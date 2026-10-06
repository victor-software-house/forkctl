## ADDED Requirements

### Requirement: A refresh that fails its final check stays resumable

When the final `check` of `patch refresh`, or of the `operation continue` that resumes it, fails after the patch commit and bookkeeping are written, forkctl SHALL keep the `patchrefresh` operation in phase `checking`, with next actions that name `mise run fork operation continue` and `mise run fork operation abort --yes`, and SHALL report the check's own error code.

#### Scenario: A pre-commit hook leaves an untracked lockfile

- **WHEN** `.git/hooks/pre-commit` writes `tool.lock` on every run, and the operator stages a change to the active patch's `hook-lock.txt` and runs `forkctl --format json patch refresh`
- **THEN** it fails with `error.code` equal to `dirty_worktree`
- **AND** `forkctl --format json operation status` reports `result.operation.phase` equal to `checking`, with `next_actions` containing `operation continue` and `operation abort --yes`

#### Scenario: Continue after the dirty path is removed

- **WHEN** the operator deletes `tool.lock` and runs `forkctl operation continue`
- **THEN** it succeeds
- **AND** `forkctl --format json status` reports `result.active_patch.patch` equal to `hook-lock` and `result.operation` as null
- **AND** one `forkctl patch finish` leaves `result.active_patch` null and `forkctl check` succeeds

### Requirement: Patch finish waits for the operation in flight

`patch finish` SHALL fail with `operation_in_progress` while a forkctl operation is in flight, and SHALL leave the active patch unchanged.

#### Scenario: Finish during a refresh stuck in its final check

- **WHEN** a `patchrefresh` operation is in phase `checking` and the operator runs `forkctl --format json patch finish`
- **THEN** it fails with `error.code` equal to `operation_in_progress`
- **AND** the operation and the active patch are unchanged
