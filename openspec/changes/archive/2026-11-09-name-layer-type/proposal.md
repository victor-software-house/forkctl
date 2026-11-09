# Proposal

## Why

forkctl used `kind` as the name of five different things: a patch's layer (`source` or `tooling`), a base target's type (`commit`, `branch`, or `tag`), a history entry's event, an operation's starting command, and the schema document `api schema` emits.
The word says only that a value comes from a set, so a reader of the manifest, the help, or the JSON cannot tell which set without the docs.
The ctl CLIs name a field, flag, or message by what it does, and the operator ruled that `kind` names nothing and must go.

Three of the five are stored or exchanged: the manifest (`patches[].kind`, `base.target.kind`, `history[].kind`, and the patch and target records inside history and `disabled_patches`), the Git-private operation journal and active-patch file, and the JSON API.
Renaming them changes the manifest, local state, and protocol, all at version 1 today.
forkctl has a schema number and no upgrade path, so every consumer manifest written by forkctl 0.0.48 or earlier needs a way forward that keeps its history.

Protocol version 1 becomes protocol version 2, and manifest schema 1 becomes schema 2.

## What Changes

- Manifest schema 2: `patches[].layer`, `base.target.type`, `history[].event`, and `layer` and `type` inside every nested patch and target record.
- Operation journal: `command` names the command that started the operation, and the intent's tag is `action`. Error details for an operation carry `command`.
- CLI: `patch create` and `patch edit` take `-l/--layer` (`FORKCTL_LAYER`) in place of `-k/--kind` (`FORKCTL_KIND`); `api schema` takes `-d/--document` (`FORKCTL_DOCUMENT`) in place of `-k/--kind`; the hidden `__candidates` positional is `<TYPE>`.
- Output: `patch list` and `patch show` print `layer`, `operation status` prints `command`, and the generated ledger's column is `Layer`. JSON results use the same names.
- `forkctl upgrade` rewrites a schema 1 manifest in place as schema 2, regenerates the ledger, refreshes the bookkeeping patch, and runs the full check. Any command reading a schema 1 manifest names it.
- Rust types: `PatchKind` → `PatchLayer`, `TargetKind` → `TargetType`, `OperationKind` → `JournalCommand`, `SchemaKind` → `SchemaDocument`, `CandidateKind` → `CandidateType`.

## Capabilities

### New Capabilities

- `manifest-upgrade`: schema 2 field names, and the one upgrade from schema 1.

### Modified Capabilities

- `publish-proposal`: the pull request lists each patch with its layer.

## Impact

- [`src/manifest.rs`][manifest], [`src/state.rs`][state], [`src/protocol.rs`][protocol], [`src/cli.rs`][cli], and every module that reads them.
- New [`src/manifest_upgrade.rs`][upgrade] and [`src/app/upgrade.rs`][app-upgrade].
- [`templates/patches.md`][ledger] and [`templates/proposal-pr.md`][proposal-template].
- [`examples/fork.yaml`][example], [`README.md`][readme], `AGENTS.md`, `CODING_STANDARDS.md`, and the operator templates under `.ctl/operator/`.
- Every consumer runs `mise run fork upgrade` once after taking the release, and renames `FORKCTL_KIND` in its environment.

[manifest]: ../../../../src/manifest.rs
[state]: ../../../../src/state.rs
[protocol]: ../../../../src/protocol.rs
[cli]: ../../../../src/cli.rs
[upgrade]: ../../../../src/manifest_upgrade.rs
[app-upgrade]: ../../../../src/app/upgrade.rs
[ledger]: ../../../../templates/patches.md
[proposal-template]: ../../../../templates/proposal-pr.md
[example]: ../../../../examples/fork.yaml
[readme]: ../../../../README.md
