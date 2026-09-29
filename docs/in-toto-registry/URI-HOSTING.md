# Making the predicate type URIs resolve

**Done.** The redirects below went live on 2026-09-29 with noru-tech/noru#685, in
`apps/web/next.config.ts` rather than `vercel.json`, because the site keeps its redirects there.
Spec 0.3 was released as v0.5.0, and noru-tech/noru#686 pins `v0.3` to that tag, permanently.

Reviewers will follow the type URI. Before that change, every ACP URI returned 404 from
noru.tech, which is served by Vercel:

| URI | Status |
| --- | --- |
| `https://noru.tech/spec/ai-change-provenance/v0.1` | 404 |
| `https://noru.tech/spec/ai-change-provenance/v0.2` | 404 |
| `https://noru.tech/spec/ai-change-provenance/v0.3` | 404 |
| `https://noru.tech/spec/ai-change-provenance/provenance/v0.1` | 404 |
| `https://noru.tech/spec/ai-change-provenance/review/v0.1` | 404 |
| `https://noru.tech/spec/ai-change-provenance` | 404 |

This is a change to the noru.tech site, outside this repository.

## What each URI should resolve to

Each version resolves to the specification text of the release that defined it, pinned by tag,
so the text behind a URI never changes after the fact.

| Path | Target | Redirect |
| --- | --- | --- |
| `/spec/ai-change-provenance/v0.1` | `https://github.com/noru-tech/agent-change-control/blob/v0.3.1/spec/ai-change-provenance.md` | 308 |
| `/spec/ai-change-provenance/v0.2` | `https://github.com/noru-tech/agent-change-control/blob/v0.4.0/spec/ai-change-provenance.md` | 308 |
| `/spec/ai-change-provenance/v0.3` | `…/blob/main/spec/ai-change-provenance.md` until the release that ships spec 0.3, then `…/blob/vX.Y.Z/spec/ai-change-provenance.md` | 307, then 308 |
| `/spec/ai-change-provenance/provenance/v0.1` | `…/blob/v0.4.0/spec/ai-change-provenance.md#32-provenance-document` | 308 |
| `/spec/ai-change-provenance/review/v0.1` | `…/blob/v0.4.0/spec/ai-change-provenance.md#37-review-document` | 308 |
| `/spec/ai-change-provenance` | `…/blob/main/spec/ai-change-provenance.md` (latest) | 307 |

Tags were checked: v0.3.1 is the last release with spec 0.1, and v0.4.0 carries spec 0.2. The
provenance and review documents are still at 0.1 in spec 0.3 and are unchanged since v0.4.0.

## Vercel configuration

In the site's `vercel.json`, or the equivalent `redirects()` in `next.config.js`:

```json
{
  "redirects": [
    {
      "source": "/spec/ai-change-provenance/v0.1",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/v0.3.1/spec/ai-change-provenance.md",
      "permanent": true
    },
    {
      "source": "/spec/ai-change-provenance/v0.2",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/v0.4.0/spec/ai-change-provenance.md",
      "permanent": true
    },
    {
      "source": "/spec/ai-change-provenance/v0.3",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/main/spec/ai-change-provenance.md",
      "permanent": false
    },
    {
      "source": "/spec/ai-change-provenance/provenance/v0.1",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/v0.4.0/spec/ai-change-provenance.md#32-provenance-document",
      "permanent": true
    },
    {
      "source": "/spec/ai-change-provenance/review/v0.1",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/v0.4.0/spec/ai-change-provenance.md#37-review-document",
      "permanent": true
    },
    {
      "source": "/spec/ai-change-provenance",
      "destination": "https://github.com/noru-tech/agent-change-control/blob/main/spec/ai-change-provenance.md",
      "permanent": false
    }
  ]
}
```

With `"permanent": true`, Vercel answers 308; with `false`, 307. Once the release that ships
spec 0.3 is tagged, change the v0.3 entry to that tag and make it permanent. From then on, each
new spec version adds a permanent entry when it is released.

A later improvement is to serve the specification as a page on noru.tech instead of redirecting,
with the same pinning. The redirects are enough for the registry submission.

## Check

```bash
for p in v0.1 v0.2 v0.3 provenance/v0.1 review/v0.1; do
  curl -s -o /dev/null -w "%{http_code} %{redirect_url}\n" "https://noru.tech/spec/ai-change-provenance/$p"
done
```

Every line should be a 307 or 308 to the target above, and the target should be a 200.
