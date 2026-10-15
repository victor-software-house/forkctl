---
forkctl: patch
---

Every long option now reads `FORKCTL_<LONG>`, such as `FORKCTL_FORMAT=json`, and a flag on the command line wins over its variable. `FORK_MANIFEST` is renamed to `FORKCTL_MANIFEST`, with no alias: set `FORKCTL_MANIFEST` in place of `FORK_MANIFEST`, for example under `[env]` in `mise.toml`. `api call` falls back to `--manifest` or `FORKCTL_MANIFEST` when the invocation names no manifest.
