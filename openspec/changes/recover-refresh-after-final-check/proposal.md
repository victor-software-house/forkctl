# Proposal

## Why

A downstream on forkctl 0.0.30 lost a refresh to its own pre-commit hook.
During `patch refresh`, the bookkeeping refresh ran the consumer's hook, and a `mise` run inside it rewrote `mise.lock`.
Refresh had already written the new patch commit and the bookkeeping, but its final `check` found the worktree dirty and failed with `dirty_worktree`.
The `patchrefresh` operation stayed in phase `refreshing` with no next action, so `operation status` named no way out.

The operator then restored `mise.lock` and ran `patch finish`, which succeeded although an operation was still in flight, and cleared the active patch.
`operation continue` finished the refresh and set the patch active again.
`publish` then failed with `active_patch_exists` until a second `patch finish`.

Two defects caused this:

1. `finish_patch_refresh` returns a final-check failure without recording the operation's recovery.
2. `patch finish` is the only patch command without the in-flight operation guard every other patch command has.

## What Changes

- When the final check of `patch refresh`, or of the `operation continue` that resumes it, fails after the commit is written, the operation moves to phase `checking`. Its next actions name the fix, `mise run fork operation continue`, and `mise run fork operation abort --yes`.
- `operation continue` resumes such a refresh and ends exactly where a successful refresh ends: the patch is active and no operation remains.
- `patch finish` fails with `operation_in_progress` while any operation is in flight, and leaves the active patch unchanged.

Protocol version 1 is unchanged: `operation status` already carries `phase` and `next_actions`, and `operation_in_progress` already exists.

## Capabilities

### New Capabilities

- `patch-refresh`: how a refresh that fails after writing its commit is recovered, and when `patch finish` may run.

### Modified Capabilities

None.

## Impact

- [`src/app/patch.rs`][patch]: the final-check failure in `finish_patch_refresh`, and the guard in `patch_finish`.
- [`tests/lifecycle.rs`][lifecycle]: a pre-commit hook that leaves an untracked lockfile reproduces the report.
- [`.ctl/operator/instructions.md.jinja`][instructions] and [`.ctl/operator/SKILL.md.jinja`][skill]: the recovery for a hook that leaves files dirty.

[patch]: ../../../src/app/patch.rs
[lifecycle]: ../../../tests/lifecycle.rs
[instructions]: ../../../.ctl/operator/instructions.md.jinja
[skill]: ../../../.ctl/operator/SKILL.md.jinja
