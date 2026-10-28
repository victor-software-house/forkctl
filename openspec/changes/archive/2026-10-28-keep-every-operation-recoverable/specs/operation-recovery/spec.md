## ADDED Requirements

### Requirement: Patch edit keeps its scope over the paths the patch changes

`patch edit` SHALL fail with `invalid_request`, before writing any commit, manifest or operation, when the edited scope no longer owns every path the patch commit changes, and SHALL name each uncovered path.

#### Scenario: Removing the only scope entry

- **WHEN** source patch `source-change` changes `source.txt` and the operator runs `forkctl --format json patch edit source-change --remove-scope source.txt --add-scope other.txt`
- **THEN** it fails with `error.code` equal to `invalid_request` and a message containing `uncovered: source.txt`
- **AND** `forkctl operation status` reports no operation, HEAD is unchanged, and `forkctl check` succeeds

### Requirement: A patch edit that fails its final check stays resumable

When the final `check` of `patch edit` fails after its commits are written, forkctl SHALL keep the `patchedit` operation in phase `checking` with next actions naming `operation continue` and `operation abort --yes`.

#### Scenario: A pre-commit hook leaves an untracked lockfile

- **WHEN** `.git/hooks/pre-commit` writes `tool.lock` and the operator runs `forkctl --format json patch edit source-change --purpose 'Changed purpose.'`
- **THEN** it fails with `error.code` equal to `dirty_worktree`, and `operation status` reports phase `checking`
- **AND** after `tool.lock` is deleted, `forkctl operation continue` succeeds, no operation remains, and `forkctl check` succeeds

### Requirement: Transitions and rebase name the fix when their final check fails

When the final `check` of `patch remove`, `patch disable`, `patch enable` or `rebase` fails, forkctl SHALL keep the operation in phase `ready_to_publish` with next actions naming the fix, `publish`, and `operation abort --yes`.

#### Scenario: A hook dirties the worktree during a removal

- **WHEN** a pre-commit hook writes `tool.lock` while `forkctl patch remove source-change --reason obsolete` refreshes the bookkeeping patch
- **THEN** `operation status` reports phase `ready_to_publish` with next actions that include `operation abort --yes`

### Requirement: Abort ends the operation once the stack is restored

`operation abort --yes` SHALL clear the operation once HEAD, the StGit base, the series order and every patch commit match the journal, and SHALL then report a failing check as an error that says the abort completed.

#### Scenario: The stack failed check before the operation started

- **WHEN** the bookkeeping patch holds an export no patch produces, a `patch edit` stops in phase `checking`, and the operator runs `forkctl --format json operation abort --yes`
- **THEN** it fails with `error.code` equal to `check_failed` and a message containing `was aborted`
- **AND** `operation status` reports no operation and HEAD equals the tip before the edit

### Requirement: Every bookkeeping write regenerates every generated document

Every command that refreshes the bookkeeping patch SHALL write the manifest, the ledger and every source patch export, and SHALL remove each export no patch produces.

#### Scenario: Contract edit on a stack with a stale export

- **WHEN** the bookkeeping patch holds `patches/downstream/9999-stale.patch` and the operator runs `forkctl contract edit --required-text base.txt=base`
- **THEN** `patches/downstream/9999-stale.patch` no longer exists and `forkctl check` succeeds
