# Privacy

Manifests and normalized exports can contain names, usernames, email addresses, approval history, merge history and work activity. Future deployment collection would also expose deployment history. PR titles, evidence URLs and disposition rationale may contain sensitive information.

Do not publish real employee manifests by default. Keep raw exports outside Git, restrict access, and configure retention appropriate to your organization. Use synthetic fixtures for reports and debugging. Pseudonymize actor IDs consistently before sharing when appropriate, including references and free text; the CLI does not yet offer an automatic pseudonymization mode. Re-evaluate after changing normalized identities rather than editing derived hashes.

Only explicit collection commands (`scan`, `export`, `pr`) contact GitHub. `evaluate`, `validate` and `check` operate offline. No telemetry or proprietary reporting endpoint is present.
