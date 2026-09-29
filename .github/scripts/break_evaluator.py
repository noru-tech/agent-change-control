"""Deliberately break acc so the conformance corpus can prove it catches the breakage.

Usage: python break_evaluator.py ACC002 | serialization

A rule code turns every `fail` of that rule into `pass`, keeping its reason text, which is what
an evaluator that never detects the violation would report. `serialization` appends the newline that ACP 0.2 put in
every preimage back into the RFC 8785 serializer. The script fails if the code it patches has
moved, so the CI job cannot pass by patching nothing.
"""

import pathlib
import re
import sys


def patch(path, old, new):
    p = pathlib.Path(path)
    text = p.read_text()
    if text.count(old) != 1:
        sys.exit(f"{path}: patch site not found exactly once: {old!r}")
    p.write_text(text.replace(old, new))


target = sys.argv[1]
if re.fullmatch(r"ACC\d{3}", target):
    rule = "Acc" + target[3:]
    signature = "    fn assess(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, &'static str) {\n"
    patch(
        "src/rules/mod.rs",
        signature,
        f"""    fn assess(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, &'static str) {{
        match self.assess_unbroken(rule, c, p) {{
            (Status::Fail, reason) if rule == RuleId::{rule} => (Status::Pass, reason),
            other => other,
        }}
    }}

    fn assess_unbroken(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, &'static str) {{
""",
    )
elif target == "serialization":
    patch(
        "src/canonical/mod.rs",
        "    Ok(serde_json_canonicalizer::to_string(value)?)\n",
        "    Ok(serde_json_canonicalizer::to_string(value)? + \"\\n\")\n",
    )
else:
    sys.exit(f"unknown target {target!r}")
print(f"broke {target}")
