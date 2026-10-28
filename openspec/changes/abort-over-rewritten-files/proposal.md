# Proposal

## Why

Moving a downstream from forkctl 0.0.43 to 0.0.44, the operator staged `mise.toml` with the new pin and ran `patch refresh`.
`mise` had already rewritten the tracked `mise.lock`, so the final check failed with `dirty_worktree` and the refresh stayed in phase `checking`, as 0.0.44 intends.

The next action `operation abort --yes` then failed: `stg delete --all` refused with ``Entry 'mise.lock' not uptodate. Cannot merge.``, although the abort plan lists that path among the paths it discards.
The other next action worked: `git add mise.lock`, then `operation continue`, completed the refresh with the lockfile in the patch.
The next action's text did not say that staging an owned path is the fix.

A test of that recovery for a source patch exposed a third defect: `operation continue` refreshed the staged paths into the top of the stack, the bookkeeping patch, so the check failed with `path in patch fork-tooling is outside scope`.
It worked downstream only because the refreshed patch was the bookkeeping patch.

Protocol version 1 is unchanged.

## What Changes

- `operation abort` resets tracked changes to `HEAD` before StGit deletes the stack, so a tracked file a hook rewrote no longer blocks it.
  The plan already lists those paths and the command already requires `--yes`.
- `operation continue` refreshes staged paths into the refreshed patch with `stg refresh --patch <patch> --index`, wherever it sits in the stack.
- The first next action of an operation stopped by its final check reads: stage paths a hook rewrote that the patch owns, and restore the others.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `operation-recovery`: abort over rewritten tracked files, and the fix a stopped operation names.

## Impact

- [`src/app/operation.rs`][operation]: `restore_operation_stack`.
- [`src/app/patch.rs`][patch]: `continue_patch_refresh`.
- [`src/app/mod.rs`][mod]: `check_written_operation`.
- [`tests/lifecycle.rs`][lifecycle]: two tests.

[operation]: ../../../src/app/operation.rs
[mod]: ../../../src/app/mod.rs
[patch]: ../../../src/app/patch.rs
[lifecycle]: ../../../tests/lifecycle.rs
