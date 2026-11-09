---
forkctl: minor
"verctl:group": changed
---

forkctl no longer names anything `kind`. This breaks the manifest, the command line, the environment, and the JSON API, so manifest schema 2 and protocol version 2 replace schema 1 and protocol version 1. Run `mise run fork upgrade` once after taking this release, on a clean worktree with no operation in progress: it rewrites the manifest in place, regenerates the ledger, and refreshes the bookkeeping patch. Finish or abort an operation in progress with the release that started it first.

- Manifest: `schema: 1` → `schema: 2`; `patches[].kind` → `layer`; `base.target.kind` → `type`; `history[].kind` → `event`; inside `disabled_patches` and `history`, each patch record's `kind` → `layer` and each target's `kind` → `type`.
- `patch create` and `patch edit`: `-k/--kind` → `-l/--layer`; `FORKCTL_KIND` → `FORKCTL_LAYER`.
- `api schema`: `-k/--kind` → `-d/--document`; `FORKCTL_KIND` → `FORKCTL_DOCUMENT`.
- JSON API: `protocol_version: 1` → `2`; patch results' `kind` → `layer`; `operation.kind` → `operation.command`; error details' `kind` → `command`.
- Git-private state: the operation journal's `kind` → `command`, and its intent tag `kind` → `action`; the active patch's `metadata.kind` → `metadata.layer`.
- JSON Schema definitions: `PatchKind` → `PatchLayer`, `TargetKind` → `TargetType`, `OperationKind` → `JournalCommand`.
- Output: `patch list` and `patch show` print `layer` for `kind`, `operation status` prints `command` for `kind`, and the generated ledger's `Kind` column is `Layer`.
- Added: `upgrade`, which rewrites a schema 1 manifest as schema 2.
