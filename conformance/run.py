#!/usr/bin/env python3
"""Run an ACP evaluator against the conformance corpus.

Usage:
    python3 run.py --verifier "<cmd>" [--corpus DIR] [--report FILE] [--timeout SECONDS]

The verifier is run once per vector as `<cmd> <vector-file>` (the command is split with shlex;
the vector path is appended). Its exit status and the last non-blank line of its stdout, a
single-line JSON object, are compared with the vector's expected result from MANIFEST.json as
README.md#contract describes. Only the Python standard library is used.

Before running anything the corpus is checked against CORPUS-DIGESTS.txt, so a report always
describes the published bytes; --allow-modified runs a changed corpus anyway and says so in the
report.

Exit status: 0 every vector passed, 1 at least one did not, 2 the harness could not run (usage,
a corpus that does not match its digest list, or a verifier that cannot be started).
"""

import argparse
import hashlib
import json
import os
import shlex
import subprocess
import sys

VERDICT_EXIT = {"evaluated": 0, "invalid": 3, "incomplete": 4}
OUTCOMES = {"pass", "fail", "unknown", "not_applicable"}


def sha256_file(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def check_digests(corpus):
    """Return the problems between CORPUS-DIGESTS.txt and the files on disk."""
    listed = {}
    with open(os.path.join(corpus, "CORPUS-DIGESTS.txt"), encoding="utf-8") as f:
        for line in f:
            digest, _, path = line.rstrip("\n").partition("  ")
            listed[path] = digest
    problems = []
    on_disk = set()
    for root, dirs, files in os.walk(corpus):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for name in files:
            rel = os.path.relpath(os.path.join(root, name), corpus).replace(os.sep, "/")
            if rel.startswith("CORPUS-DIGESTS.txt") or rel.endswith(".pyc"):
                continue
            on_disk.add(rel)
    for rel in sorted(on_disk | set(listed)):
        if rel not in listed:
            problems.append(f"{rel}: not in CORPUS-DIGESTS.txt")
        elif rel not in on_disk:
            problems.append(f"{rel}: listed but missing")
        elif sha256_file(os.path.join(corpus, rel)) != listed[rel]:
            problems.append(f"{rel}: digest differs")
    return problems


def parse_result(stdout):
    """The last non-blank stdout line as a result object, or (None, reason)."""
    lines = [l for l in stdout.splitlines() if l.strip()]
    if not lines:
        return None, "no output on stdout"
    try:
        obj = json.loads(lines[-1])
    except ValueError:
        return None, "last line of stdout is not JSON"
    if not isinstance(obj, dict):
        return None, "last line of stdout is not a JSON object"
    verdict = obj.get("verdict")
    if verdict not in VERDICT_EXIT:
        return None, "verdict is missing or not evaluated, invalid or incomplete"
    codes = obj.get("codes")
    if not isinstance(codes, list) or not all(isinstance(c, str) for c in codes):
        return None, "codes is missing or not a list of strings"
    assessments = obj.get("assessments")
    if assessments is None:
        if verdict != "invalid":
            return None, "assessments is missing"
    elif not isinstance(assessments, list) or not all(
        isinstance(a, dict)
        and isinstance(a.get("change"), str)
        and isinstance(a.get("rule"), str)
        and a.get("outcome") in OUTCOMES
        for a in assessments
    ):
        return None, "assessments is not a list of {change, rule, outcome}"
    digest = obj.get("manifestDigest")
    if digest is not None and not isinstance(digest, str):
        return None, "manifestDigest is not a string"
    return obj, None


def tuples(assessments):
    return sorted((a["change"], a["rule"], a["outcome"]) for a in assessments)


def compare(kind, expected, exit_code, obj):
    """Reasons the observed result disagrees with the expected one; empty when it agrees."""
    reasons = []
    want = expected["verdict"]
    if obj["verdict"] != want:
        reasons.append(f"verdict {obj['verdict']!r}, expected {want!r}")
    if exit_code != VERDICT_EXIT[want]:
        reasons.append(f"exit status {exit_code}, expected {VERDICT_EXIT[want]}")
    if exit_code != VERDICT_EXIT[obj["verdict"]]:
        reasons.append("exit status contradicts the reported verdict")
    if kind == "reject":
        if not set(obj["codes"]) & set(expected["codes"]):
            reasons.append(
                f"codes {sorted(obj['codes'])} do not intersect expected {expected['codes']}"
            )
        return reasons
    if set(obj["codes"]) != set(expected["codes"]):
        reasons.append(f"codes {sorted(set(obj['codes']))}, expected {expected['codes']}")
    if obj.get("assessments") is not None:
        got, exp = tuples(obj["assessments"]), tuples(expected["assessments"])
        if got != exp:
            missing = [t for t in exp if t not in got]
            extra = [t for t in got if t not in exp]
            reasons.append(f"assessments differ: missing {missing}, unexpected {extra}")
    digest = obj.get("manifestDigest")
    if digest is not None and digest != expected["manifestDigest"]:
        reasons.append("manifestDigest differs (spec §8.2 serialization)")
    return reasons


def main(argv=None):
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--verifier", required=True, help='command, e.g. "acc evaluate --conformance-json"')
    ap.add_argument("--corpus", default=here, help="corpus directory (default: this script's)")
    ap.add_argument("--report", default="conformance-report.json", help="report file to write")
    ap.add_argument("--timeout", type=float, default=60.0, help="seconds per vector")
    ap.add_argument("--allow-modified", action="store_true", help="run a corpus that does not match its digest list")
    args = ap.parse_args(argv)

    corpus = os.path.abspath(args.corpus)
    with open(os.path.join(corpus, "MANIFEST.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    problems = check_digests(corpus)
    if problems and not args.allow_modified:
        for p in problems:
            print(f"corpus: {p}", file=sys.stderr)
        print("the corpus does not match CORPUS-DIGESTS.txt; refusing to run", file=sys.stderr)
        return 2
    command = shlex.split(args.verifier)
    if not command:
        print("--verifier is empty", file=sys.stderr)
        return 2

    rows, executed = [], 0
    totals = {"vectors": len(manifest["vectors"]), "pass": 0, "fail": 0, "error": 0}
    for v in manifest["vectors"]:
        path = os.path.join(corpus, v["path"])
        try:
            proc = subprocess.run(
                command + [path],
                capture_output=True,
                timeout=args.timeout,
                stdin=subprocess.DEVNULL,
            )
        except FileNotFoundError:
            print(f"cannot start verifier: {command[0]}", file=sys.stderr)
            return 2
        except subprocess.TimeoutExpired:
            totals["error"] += 1
            rows.append({"id": v["id"], "kind": v["kind"], "path": v["path"],
                         "status": "error", "reasons": [f"timed out after {args.timeout}s"]})
            continue
        executed += 1
        stdout = proc.stdout.decode("utf-8", "replace")
        stderr = proc.stderr.decode("utf-8", "replace")
        obj, problem = parse_result(stdout)
        if obj is None:
            status, reasons = "error", [problem]
        else:
            reasons = compare(v["kind"], v["expected"], proc.returncode, obj)
            status = "fail" if reasons else "pass"
        totals[status] += 1
        if status != "pass":
            rows.append({
                "id": v["id"],
                "kind": v["kind"],
                "path": v["path"],
                "parent": v.get("parent"),
                "conditions": v["conditions"],
                "status": status,
                "reasons": reasons,
                "expected": v["expected"],
                "observed": {
                    "exit": proc.returncode,
                    "lastLine": ([l for l in stdout.splitlines() if l.strip()] or [None])[-1],
                    "stderrTail": stderr[-2000:],
                },
            })

    with open(os.path.join(corpus, "CORPUS-DIGESTS.txt"), "rb") as f:
        corpus_digest = "sha256:" + hashlib.sha256(f.read()).hexdigest()
    passed = totals["pass"] == totals["vectors"] and executed == totals["vectors"]
    report = {
        "suite": manifest["suite"],
        "suiteRevision": manifest["suiteRevision"],
        "specVersion": manifest["specVersion"],
        "specDigest": manifest["specDigest"],
        "corpusDigest": corpus_digest,
        "corpusModified": bool(problems),
        "verifier": {"command": args.verifier, "vectorsExecuted": executed},
        "totals": totals,
        "passed": passed and not problems,
        "disagreements": rows,
    }
    with open(args.report, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, sort_keys=True)
        f.write("\n")
    print(
        f"{manifest['suite']} revision {manifest['suiteRevision']}: "
        f"{totals['pass']}/{totals['vectors']} passed, {totals['fail']} failed, "
        f"{totals['error']} errors; report in {args.report}"
    )
    for r in rows:
        print(f"  {r['status']}: {r['id']}: {'; '.join(r['reasons'])}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
