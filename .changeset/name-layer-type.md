---
forkctl: patch
"verctl:group": changed
---

forkctl no longer names anything `kind`. Manifest schema 2 and protocol version 2 replace schema 1 and protocol version 1. Run `mise run fork upgrade` once after taking this release, on a clean worktree with no operation in progress: it rewrites the manifest in place, regenerates the ledger, and refreshes the bookkeeping patch. Finish or abort an operation in progress with the release that started it first.

- Manifest: `patches[].kind` is `layer`, `base.target.kind` is `type`, `history[].kind` is `event`, and the patch and target records inside `disabled_patches` and `history` use `layer` and `type` too.
- `patch create` and `patch edit`: `-k/--kind` is `-l/--layer`, and `FORKCTL_KIND` is `FORKCTL_LAYER`.
- `api schema`: `-k/--kind` is `-d/--document`, and `FORKCTL_KIND` is `FORKCTL_DOCUMENT`.
- Operation journal and JSON: `operation.kind` is `operation.command`, the operation intent's tag `kind` is `action`, error details carry `command` where they carried `kind`, and patch results carry `layer`.
- Output: `patch list` and `patch show` print `layer`, `operation status` prints `command`, and the generated ledger's `Kind` column is `Layer`.
