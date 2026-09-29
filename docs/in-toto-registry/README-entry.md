# Listing entry for `spec/predicates/README.md`

Upstream's predicate README has only a "Vetted Predicates" section today. The two-tier proposal
(in-toto/ITE#63, "ITE-0000: tiers and an evidence criterion for predicates") is still an open
draft, and its tier names are undecided ("contrib", "community"). A first submission should not
list itself as vetted. Vetting needs review, and under the draft it also needs testing by
parties other than the predicate's author. So the change adds a section after "Vetted
Predicates" and puts the entry there.

If maintainers would rather wait for the ITE, the fallback is to add only the document and ask
where to list it.

```markdown
## Community Contributed Predicates

These predicates were contributed by their authors and follow the
[New Predicate Guidelines]. They have not been through the [vetting process].

-   [AI Change Provenance]: Whether a human independent of a software change's
    effective author approved it, where the author may be a coding agent and
    its human operator, with the facts and a re-evaluable verdict.

[AI Change Provenance]: ai-change-provenance.md
```

The reference definitions `[New Predicate Guidelines]` and `[vetting process]` already exist in
that file.
