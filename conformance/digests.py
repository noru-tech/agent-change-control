#!/usr/bin/env python3
"""Write, or check, CORPUS-DIGESTS.txt: the SHA-256 of every corpus file.

Usage:
    python3 digests.py           rewrite CORPUS-DIGESTS.txt from the files on disk
    python3 digests.py --check   exit 1 if CORPUS-DIGESTS.txt does not match them

One line per file, `<sha256 hex>  <path>`, sorted by path, in the format `sha256sum -c`
reads. Every file under this directory is listed except the digest list itself and its
signature material. The list is generated, never edited by hand; the release workflow signs it
at the tag. Only the Python standard library is used.
"""

import hashlib
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
LIST = os.path.join(HERE, "CORPUS-DIGESTS.txt")


def render():
    files = []
    for root, dirs, names in os.walk(HERE):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for name in names:
            rel = os.path.relpath(os.path.join(root, name), HERE).replace(os.sep, "/")
            if rel.startswith("CORPUS-DIGESTS.txt") or rel.endswith(".pyc"):
                continue
            files.append(rel)
    lines = []
    for rel in sorted(files):
        with open(os.path.join(HERE, rel), "rb") as f:
            lines.append(f"{hashlib.sha256(f.read()).hexdigest()}  {rel}\n")
    return "".join(lines)


def main(argv):
    text = render()
    if "--check" in argv:
        try:
            with open(LIST, encoding="utf-8") as f:
                current = f.read()
        except FileNotFoundError:
            current = ""
        if current != text:
            print("CORPUS-DIGESTS.txt does not match the files on disk; run digests.py", file=sys.stderr)
            return 1
        print(f"CORPUS-DIGESTS.txt matches {text.count(chr(10))} files")
        return 0
    with open(LIST, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"wrote CORPUS-DIGESTS.txt for {text.count(chr(10))} files")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
