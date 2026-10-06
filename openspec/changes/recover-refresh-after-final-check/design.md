# Design

## Context

`patch refresh` creates a `patchrefresh` operation before it touches StGit.
[`finish_patch_refresh`][patch] then writes the manifest, exports, and ledger, refreshes the bookkeeping patch through `stg refresh` (which runs the consumer's pre-commit hook), marks the patch active, and runs the full `check`.
Only after that check does it complete the operation.
`operation continue` re-enters the same function, so a resumed refresh repeats those steps; with nothing left to stage, the bookkeeping refresh is a no-op.

## Decisions

### 1. Record the recovery where the final check fails

The failure keeps its error code (`dirty_worktree`, `check_failed`, or another), and the operation moves to phase `checking` with three next actions: fix what the check reported, `operation continue`, or `operation abort --yes`.

Alternative considered: run the check before the bookkeeping refresh, so a failure happens before any commit.
It lost because the dirt in the report is produced by the hook that the bookkeeping refresh itself runs; an earlier check cannot see it.

### 2. Resume with `operation continue`, not a new verb

`operation continue` already resumes a refresh by re-running `finish_patch_refresh`.
Once the worktree is clean, the bookkeeping is unchanged, so no hook runs again, the check passes, and the operation completes.
`operation abort --yes` already restores the recovery tag with `git reset --hard`, which also discards the dirty paths.

Alternative considered: a `patch refresh --resume` mode.
It lost because the product keeps one typed operation journal behind `status`, `continue`, and `abort`.

### 3. `patch finish` refuses while an operation is in flight

`patch finish` calls `require_no_operation` like `patch create`, `select`, `edit`, `refresh`, `remove`, `disable`, and `enable`, and fails with `operation_in_progress`.

Alternative considered: let `patch finish` complete the pending operation.
It lost because completing a refresh is the operation's job, including its check; a finish that completed it would be a second, partial continue.

No automatic behaviour is added, so there is no override or disable path to name.

[patch]: ../../../src/app/patch.rs
