# Phase 1 normalized model

The four schemas in `schemas/` are versioned public interfaces (`0.1`, JSON Schema draft 2020-12). Unknown fields and unknown rule names are rejected. `src/model` mirrors these interfaces. Schema validation occurs before CLI evaluation. The exporter depends on the model, never on rule code.

An export contains a repository, inclusive UTC collection window, actor registry and changes. Actor map keys are canonical IDs (`github:login`, `agent:tool`); GitHub logins are lowercased during collection. When the forge returns no account for a role (a deleted user), the collector records the `unknown:unavailable` actor of kind unknown rather than guessing. Each reference must resolve. Actor kinds are human, agent, bot, service and unknown. There is no email/login identity guessing.

Each change records the forge opener, commit authors, effective author, optional agent operator, complete review history, current head SHA, merger, timestamps and provenance. `reviews_complete` is distinct from overall window completeness. Missing collection never means an empty, complete review history. Null represents an unknown/unavailable optional fact, not a fabricated identity. Phase 1 omits deployment and bypass fields entirely.

A review records a stable source ID, actor, state, submission/event time, reviewed SHA and evidence. Normalized consumers should supply review-state events with their actual event times. GitHub's current dismissed state lacks its effective time, so the collector flags that history incomplete. Duplicate IDs and unresolved references are rejected. Conflicting simultaneous decisions are rejected because ordering them by an arbitrary ID would invent chronology.

Integrity errors: ACV001 approval after merge, ACV003 invalid actor relationships, ACV004 inconsistent timeline or duplicated/ambiguous events. ACV002 is reserved for Phase 2 deployment validation. A governance failure remains valid data and never prevents a manifest.

A manifest embeds the normalized events and resolved policy alongside findings, assessments, summary and generation metadata. Each enabled rule gets a `pass`, `fail`, `unknown` or `not_applicable` assessment per change. `clean`, `with_findings` and `indeterminate` are disjoint change counts; `findings` counts individual findings. A change with ACC006 is counted with findings, even though its independence is unknown. Clean means no enabled finding or uncertainty in complete inputs, not a compliance certification.

Canonical JSON uses sorted object keys, UTF-8, compact serialization and one final LF. Changes sort by ID, commits by SHA, reviews by UTC instant then ID; evidence lists sort and deduplicate. Timestamps normalize to UTC. Actor sets sort and deduplicate. No current time or random value enters evaluation. Identical semantic inputs, resolved policy and tool version yield identical output bytes. YAML is a presentation format, not the hash format. This canonicalization is project-specific, not an RFC 8785 claim.

The source digest is SHA-256 of canonical normalized events. Finding IDs are `acc-` plus the first 16 hexadecimal characters of SHA-256 over the canonical JSON tuple `[repository, change_id, rule_id, sorted_unique_actor_ids]` including its final LF. Actor IDs comprise the effective author and known effective human. Structured encoding prevents concatenation ambiguities. IDs do not change with severity or dispositions.

Validation re-evaluates the embedded events and policy and compares canonical output, allowing only structurally valid disposition edits. Evidence refs make findings explainable offline; the tool does not claim to archive API response bodies or prove those bodies were authentic.
