# Examples

| File | What it shows |
| --- | --- |
| `pull-request-body.md` | The inline declaration a producer puts in a pull request description (spec §3.1) |
| `provenance.json` | A standalone provenance document bound to a head commit (spec §3.2); validate with the `provenance` schema |
| `policy.yml` | A policy that fails on unknown operators |
| `workflow-required-check.yml` | The GitHub Action as a merge gate with SARIF upload |

Synthetic inputs for the offline commands live in `tests/fixtures/`; for instance:

```bash
acc evaluate tests/fixtures/claude-operator-self-approved/events.json --format table
acc evaluate tests/fixtures/claude-clean/events.json --format in-toto
acc evaluate tests/fixtures/claude-clean/events.json -o clean.intoto.jsonl && acc validate clean.intoto.jsonl
```
