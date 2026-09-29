"""Recompute acc's golden digests with an independent RFC 8785 implementation.

Spec §8.2 says every ACP digest is SHA-256 over the RFC 8785 (JCS) bytes of the normalized
value. This script checks that claim against the committed goldens without any of acc's code:
it parses each golden with Python's json module, serializes it with the `rfc8785` package, and
recomputes

- the file bytes of every expected-manifest.json, expected.intoto.json and each line of
  expected.intoto.jsonl, which must equal their JCS serialization exactly (no trailing newline);
- every manifest's generated.source_digest, over the JCS bytes of its events;
- every finding identifier, over the JCS bytes of [repository, change_id, rule_id, actor_ids];
- the sha256 subject form, SHA-256 over the JCS bytes of a Statement's predicate, which must equal
  the digest of the manifest file itself.

Usage: python jcs_crosscheck.py [FIXTURES_DIR]
"""

import hashlib
import json
import pathlib
import sys

import rfc8785


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def check_manifest(m: dict, where: str, errors: list) -> int:
    checked = 0
    if m["generated"]["source_digest"] != sha256(rfc8785.dumps(m["events"])):
        errors.append(f"{where}: source_digest")
    checked += 1
    for f in m["findings"]:
        preimage = rfc8785.dumps(
            [m["events"]["repository"], f["change_id"], f["rule_id"], f["actor_ids"]]
        )
        if f["id"] != "acc-" + hashlib.sha256(preimage).hexdigest()[:16]:
            errors.append(f"{where}: finding {f['id']}")
        checked += 1
    return checked


def main() -> int:
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "tests/fixtures")
    errors: list = []
    checked = 0
    for path in sorted(root.glob("*/expected-manifest.json")):
        raw = path.read_bytes()
        m = json.loads(raw)
        if raw != rfc8785.dumps(m):
            errors.append(f"{path}: file is not its JCS serialization")
        checked += 1 + check_manifest(m, str(path), errors)
    for path in sorted(root.glob("*/expected.intoto.json")):
        raw = path.read_bytes()
        statement = json.loads(raw)
        if raw != rfc8785.dumps(statement):
            errors.append(f"{path}: file is not its JCS serialization")
        manifest = (path.parent / "expected-manifest.json").read_bytes()
        if sha256(rfc8785.dumps(statement["predicate"])) != sha256(manifest):
            errors.append(f"{path}: sha256 subject form")
        checked += 2 + check_manifest(statement["predicate"], str(path), errors)
    for path in sorted(root.glob("*/expected.intoto.jsonl")):
        for n, line in enumerate(path.read_bytes().split(b"\n")[:-1], 1):
            statement = json.loads(line)
            if line != rfc8785.dumps(statement):
                errors.append(f"{path}:{n}: line is not its JCS serialization")
            checked += 1 + check_manifest(statement["predicate"], f"{path}:{n}", errors)
    for e in errors:
        print(f"mismatch: {e}", file=sys.stderr)
    print(f"{checked} digests and serializations recomputed, {len(errors)} mismatches")
    return 1 if errors or checked == 0 else 0


if __name__ == "__main__":
    sys.exit(main())
