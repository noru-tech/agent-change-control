# Agent authorship

Technical identity and independent human judgment are different relationships. The forge opener, commit authors, effective author and human operator are stored separately.

Phase 1 accepts an exact `agent-change-control` fenced YAML/JSON declaration in the PR description, or explicit `--agent-account LOGIN=AGENT` mappings supplied by a caller who verified the account. Account mappings are a caller trust boundary, not an automatically verified account directory. Their provenance is labeled declared. An API reference to the observed opener is also retained. Conflicting declarations/mappings and malformed structured declarations are errors, never silently ignored.

Example PR body block:

````text
```agent-change-control
author: codex
operator: github:alice
```
````

Only matching GitHub human identities resolve an operator. The exporter can GET `/users/alice` to resolve a declared operator not otherwise present in the PR. An email alone does not resolve to a GitHub account. Unknown operators stay unknown. The merger, PR creator or first reviewer is never implicitly substituted as agent operator.

Evidence records `source`, `ref`, and `kind` (observed, derived, declared). API observations and user declarations are distinguishable even though both have source links. Empty evidence arrays are invalid for important observations. Known-account recognition retains both the mapping and observed PR evidence.

The separately published provenance schema defines tool-neutral agent/version, operator ID, session and change base/head fields. The library can validate its binding to an explicit head SHA. File discovery, trailers, signed assertions, app installation verification and email identity maps are deferred. No LLM, wording/style classifier or blanket bot-to-agent conversion is used.
