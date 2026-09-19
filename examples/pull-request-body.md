Add rate limiting to the token endpoint

Limits requests per client to 60/min using the existing Redis-backed limiter.

```agent-change-control
author: claude-code
operator: alice
```
