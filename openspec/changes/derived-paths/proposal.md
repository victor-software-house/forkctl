# Proposal

## Motivation

A fork of a JavaScript action commits a build output, such as `dist/index.js`, because the runner executes the committed file and never builds it.
That file is one generated bundle of every source file, so any upstream release that touches a dependency or a source file changes it.
The patch that carries the fork's version of the bundle then conflicts on every such upstream release, even when its source changes replay cleanly.

In the [mise-action fork][mise-action], the bundle is 5,127,133 bytes and its source map 26,366,422 bytes.
Upstream changed both in its v5.2.7 release, so the rebase onto v5.2.7 stopped on `dist/index.js` and `dist/index.js.map` while `src/index.ts` merged.
The resolution is always the same and never needs judgment: take upstream's copy, run the build, and refresh the patch.
forkctl has no way to declare that, so the operator resolves a conflict in a file nobody can read, by hand, on every rebase.

A rebuilt file can also go stale without any conflict.
When a lower patch changes a source file, the committed bundle no longer matches the sources, and nothing in `forkctl check` notices.

The change is additive to manifest schema 2 and protocol version 2: one optional patch field, one command, new finding codes, and one new result field.

## Changes

- A patch may declare `derived`: the paths it does not edit by hand (`paths`) and the commands that rebuild them (`rebuild`), in the order they run.
- Each derived path has one owner. A patch's derived paths lie inside its scope, no two patches declare overlapping derived paths, and no other patch's commit may touch a derived path.
- `rebase` pushes the stack one patch at a time. After it pushes a patch that declares `derived`, it runs the rebuild in the worktree, stages the derived paths, and refreshes the patch.
  A conflict confined to that patch's derived paths is resolved this way with no stop.
  A conflict in any other path stops as today, and `operation continue` rebuilds the patch and pushes the remaining patches.
- `forkctl check` rebuilds each owner's derived paths in a disposable clone at the owner's commit, and reports `derived_stale` when the result differs from the commit.
  A pass is recorded by owner commit and the whole `derived` record, so an unchanged owner is not rebuilt again; `check --no-cache` (`FORKCTL_NO_CACHE`) ignores the records, and a staged check skips the rebuild.
- A rebuild that changes a tracked file outside the derived paths stops with `derived_rebuild_dirty` and refreshes nothing.
- `patch rebuild PATCH` rebuilds an owner's derived paths in the worktree and refreshes the owner, for the stale case outside a rebase.
- `patch create` and `patch edit` take `-D/--derived GLOB` and `-R/--rebuild COMMAND`, both repeatable, and `patch edit` takes `--clear-derived`.
  Their variables are `FORKCTL_DERIVED`, `FORKCTL_REBUILD`, and `FORKCTL_CLEAR_DERIVED`.
- The rebase result lists `rebuilt_patches`, and the rebase report names each rebuilt patch.

## Capabilities

### New Capabilities

- `derived-paths`: declaring derived paths, their single owner, rebuilding them during rebase and on request, and checking that they match their sources.

### Modified Capabilities

None.

## Impact

- [`src/manifest.rs`][manifest] and [`src/manifest_edit.rs`][manifest-edit]: the `derived` record and its validation.
- [`src/app/rebase.rs`][rebase] and [`src/app/operation.rs`][operation]: the push loop, the rebuild, and the continuation.
- [`src/app/declared.rs`][declared] and [`src/app/check.rs`][check]: the freshness check and the ownership findings.
- [`src/app/patch.rs`][patch] and [`src/cli.rs`][cli]: `patch rebuild` and the new flags.
- [`src/protocol.rs`][protocol]: the new request, result field, and finding codes.
- [`src/instructions.md`][instructions], the operator skill, [`README.md`][readme], and [`examples/fork.yaml`][example].
- No consumer must act. A fork adopts the field when it declares `derived` on a patch.

[mise-action]: https://github.com/victor-software-house/mise-action
[manifest]: ../../../src/manifest.rs
[manifest-edit]: ../../../src/manifest_edit.rs
[rebase]: ../../../src/app/rebase.rs
[operation]: ../../../src/app/operation.rs
[declared]: ../../../src/app/declared.rs
[check]: ../../../src/app/check.rs
[patch]: ../../../src/app/patch.rs
[cli]: ../../../src/cli.rs
[protocol]: ../../../src/protocol.rs
[instructions]: ../../../src/instructions.md
[readme]: ../../../README.md
[example]: ../../../examples/fork.yaml
