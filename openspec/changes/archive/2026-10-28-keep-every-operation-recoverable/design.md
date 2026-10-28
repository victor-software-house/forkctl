# Design

## Context

Each mutating command writes commits and generated documents, refreshes the bookkeeping patch, which runs the consumer's pre-commit hook, and then runs the full `check`.
Since 0.0.31, `patch refresh` records phase `checking` when that check fails.
The other commands either left a phase with no next action or a next action that cannot succeed.

## Decisions

### 1. Validate what can be validated before the first write

`patch edit` compares the new scope with the paths the patch commit changes, using the same `Patch::owns` rule as `check`, and fails with `invalid_request` before creating an operation.

Alternative considered: let the final check catch it and record phase `checking`.
It lost because continuing cannot fix a scope the operator just narrowed; only abort could, after a rewrite that never had to happen.

### 2. One function ends every written operation

`check_written_operation` runs the check and, on failure, records the phase and three next actions: fix what the check reported, the command that resumes, and `operation abort --yes`.
Refresh and edit resume with `operation continue`; transitions and rebase resume with `publish`, whose preflight runs the check again.

Alternative considered: a `checking` phase for transitions and rebase too, resumed by `continue`.
It lost because their commits are complete and `publish` already checks; `continue` would replay a rebase and push its history event twice.

### 3. Rebase checks after its report

The report depends only on the range-diff, so writing it before the check leaves a complete `ready_to_publish` candidate when the check fails.

### 4. Abort ends the operation before it checks

Abort already verifies HEAD, the StGit base, the series order and every patch commit against the journal.
That verification is the abort's contract; the check describes the state the operation started from, which may never have passed, for example after a forkctl upgrade changed a generated document.

Alternative considered: skip the check after abort.
It lost because the operator still needs to know the restored stack fails, so the error stays, and says that the abort completed.

### 5. Every bookkeeping write regenerates every generated document

`write_generated` writes the manifest, the ledger and every source export.
`contract edit` used to skip exports, so after a format change it was the one command that committed a stack its own check rejects.

No compatibility code is added: the first command that writes the bookkeeping patch after an upgrade brings every generated document to the new form.
No automatic behaviour is added, so there is no override or disable path to name.
