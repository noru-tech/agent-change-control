# Rules and validation codes

Every identifier `acc` reports has a stable page here. **Rules** (`ACC…`) evaluate a change and
yield `pass`, `fail`, `unknown` or `not_applicable`; a `fail` is a finding that can carry a
disposition. **Validation codes** (`ACV…`) reject an input outright: no manifest, no findings,
exit 3. Identifiers are never reused; reserved ones are listed so they stay taken.

SARIF output links each rule to its page (`helpUri`), and error messages that name a validation
code end with a link to its page. The exact conditions are normative in
[AI Change Provenance §6.2 and §6.6](../../spec/ai-change-provenance.md#62-rules); the fixtures
that exercise each are listed in [policy](../policy.md#exact-deterministic-conditions).

## Rules

| ID | Finding | Default severity | Under |
| --- | --- | --- | --- |
| [ACC001](ACC001.md) | Agent change without independent human approval | high | every policy |
| [ACC002](ACC002.md) | Effective human author/operator approved own change | high | every policy |
| [ACC003](ACC003.md) | Merged change without independent human approval | high | every policy |
| ACC004 | Reserved (deployment rule) | — | not implemented |
| ACC005 | Reserved (bypass rule) | — | not implemented |
| [ACC006](ACC006.md) | Agent operator unknown | warning | every policy |
| [ACC007](ACC007.md) | Agent approval recorded (observation) | info | every policy |
| [ACC008](ACC008.md) | Same-vendor write and review | high | every policy |
| [ACC009](ACC009.md) | Agent approval lacks required independence | high | `agent_review` |
| [ACC010](ACC010.md) | Agent approval without signed identity | warning | `agent_review` |

Severities order `info < warning < medium < high`; with the default `fail_on: medium`, `high`
findings fail `acc check` and `warning`/`info` findings do not. A [policy](../policy.md) can change
any rule's severity or disable it.

## Validation codes

| Code | Input is rejected when |
| --- | --- |
| [ACV001](ACV001.md) | An `approved` review is later than the change's merge |
| [ACV002](ACV002.md) | Reserved (deployment validation) |
| [ACV003](ACV003.md) | A reference between facts does not resolve or is not allowed |
| [ACV004](ACV004.md) | The timeline or identifiers are inconsistent |
| [ACV005](ACV005.md) | A number is not an integer (I-JSON) |
| [ACV006](ACV006.md) | An integer is outside ±(2^53 − 1) (I-JSON) |
| [ACV007](ACV007.md) | A string contains an unpaired surrogate (I-JSON) |
| [ACV008](ACV008.md) | An object has a duplicate member name (I-JSON) |
| [ACV009](ACV009.md) | Nesting is deeper than 128 (I-JSON) |
| [ACV010](ACV010.md) | The document does not conform to its schema |

## Exit codes

The process exit codes (0 success, 1 policy threshold exceeded, 2 usage, 3 invalid input, 4
collection incomplete, 5 authentication, 6 API failure, 7 unsupported API data) are on one page:
[exit codes](../exit-codes.md).

## Recording a decision about a finding

Findings are never deleted. A human decision (`accepted`, `remediated` or `false_positive`, with
owner, date, rationale and optional expiry) goes in the finding's `disposition` in the manifest,
and `acc check MANIFEST --as-of DATE` honors it on that date. Each rule page has the details;
the full rules are in [policy and dispositions](../policy.md#recording-exceptions). Where the
findings land in SOC 2, ISO 27001, PCI DSS and NIST is in [control mapping](../control-mapping.md).
