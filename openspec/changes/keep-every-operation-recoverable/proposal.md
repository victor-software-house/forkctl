# Proposal

## Why

Upgrading a downstream from forkctl 0.0.38 to 0.0.43 stopped twice with an operation that no command could finish.

1. `patch edit fork-tooling --remove-scope CLAUDE.md` rewrote the patch commit, then its final `check` failed with `path in patch fork-tooling is outside scope: CLAUDE.md`.
   The `patchedit` operation stayed in phase `editing` with no next action.
2. `operation abort --yes` restored the stack exactly, then ran `check` on it.
   The restored stack still held exports in the format 0.0.38 wrote, so the check failed with `export differs`, and abort left the operation in place.
   Every later abort repeated the failure; the operator could only recover by running 0.0.38 with a repository-local `core.abbrev=8`.

Three more paths share the defects:

1. `patch remove`, `patch disable` and `patch enable` write `ready_to_publish`, then check; a failure keeps the next action `forkctl publish` without naming the fix.
2. `rebase` checks between writing the replayed stack and writing its report, so a failure leaves phase `replayed`, which `publish` refuses and `continue` would replay again.
3. `contract edit` rewrites the manifest and ledger but not the exports, so after an export format change it commits and then fails its check.

Protocol version 1 is unchanged.
The operation phases and next-action strings are existing fields.

## What Changes

- `patch edit` refuses, before writing anything, a scope that no longer covers every path the patch changes, with `invalid_request` naming the uncovered paths.
- `patch edit` handles a final check failure as `patch refresh` does: phase `checking`, with next actions naming the fix, `operation continue`, and `operation abort --yes`.
- The patch transitions and `rebase` run their final check after every write, and a failure keeps phase `ready_to_publish` with next actions naming the fix, `publish`, which checks again, and `operation abort --yes`.
- `operation abort` ends the operation once the restored stack matches the journal commit by commit, then checks; a check failure is reported as an error that says the abort completed.
- Every command that refreshes the bookkeeping patch writes the manifest, the ledger and every export through one function, `write_generated`.

## Capabilities

### New Capabilities

- `operation-recovery`: how each operation ends or stays resumable when its final check fails.

### Modified Capabilities

None.

## Impact

- [`src/app/mod.rs`][mod]: `write_generated` and `check_written_operation`.
- [`src/app/patch.rs`][patch]: `patch_edit`, `finish_patch_edit`, `finish_patch_refresh`, `finish_patch_transition`.
- [`src/app/rebase.rs`][rebase], [`src/app/contract.rs`][contract], [`src/app/operation.rs`][operation].
- [`tests/lifecycle.rs`][lifecycle]: five tests.

[mod]: ../../../src/app/mod.rs
[patch]: ../../../src/app/patch.rs
[rebase]: ../../../src/app/rebase.rs
[contract]: ../../../src/app/contract.rs
[operation]: ../../../src/app/operation.rs
[lifecycle]: ../../../tests/lifecycle.rs
