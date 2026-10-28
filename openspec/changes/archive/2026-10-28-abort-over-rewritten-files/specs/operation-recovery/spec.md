## ADDED Requirements

### Requirement: Abort discards tracked changes a hook left

`operation abort --yes` SHALL reset tracked changes in the worktree and index to `HEAD` before it deletes the StGit stack, so a tracked file a hook rewrote cannot stop the abort.

#### Scenario: A hook rewrote a tracked lockfile during a refresh

- **WHEN** source patch `locked-source` owns `tool.lock`, a pre-commit hook rewrites `tool.lock`, `forkctl patch refresh` stops in phase `checking`, and the operator runs `forkctl operation abort --yes`
- **THEN** it succeeds, `forkctl operation status` reports no operation, and HEAD equals the tip before the refresh

### Requirement: A refresh continues once the rewritten owned path is staged

When the final check of `patch refresh` fails because a hook rewrote a path the patch owns, staging that path and running `operation continue` SHALL complete the refresh with the staged content in the refreshed patch, wherever that patch sits in the stack.

#### Scenario: Staging the rewritten lockfile

- **WHEN** the same refresh stops in phase `checking` and the operator runs `git add tool.lock` and `forkctl operation continue`
- **THEN** it succeeds, `forkctl patch finish` and `forkctl check` succeed, and the patch commit holds `tool.lock` as the hook wrote it
