# Spec Delta

## Purpose

Defines how `forkctl init` without a manifest turns a clone at its upstream
base into a downstream stack with a bookkeeping patch, tracking, and a
downstream branch.

## ADDED Requirements

### Requirement: Bootstrap creates a missing downstream branch at the base

When the downstream remote has no `refs/heads/<downstream branch>`, bootstrap
`init` SHALL push the base commit to it with
`--force-with-lease=refs/heads/<downstream branch>:` before it writes any
StGit state. When the branch exists, bootstrap SHALL not push.

#### Scenario: Empty downstream remote

- **WHEN** `origin` has no `refs/heads/main` and the operator runs
  `forkctl init ... --downstream-remote origin --downstream-branch main`
- **THEN** `git ls-remote origin refs/heads/main` names the base commit

#### Scenario: Dry run

- **WHEN** the operator runs the same `forkctl --format json init ... -n`
- **THEN** `result.ref_updates` is exactly
  `["<base> -> origin refs/heads/main (create)"]`
- **AND** `origin` still has no `refs/heads/main`

### Requirement: Bootstrap sets downstream tracking

Bootstrap `init` SHALL set the upstream of the downstream branch to
`<downstream remote>/<downstream branch>`, whatever it tracked before, so the
next `patch refresh` and `publish` need no manual step.

#### Scenario: Fresh upstream clone

- **WHEN** `main` tracks `upstream/main` and the operator runs bootstrap
  `forkctl init`
- **THEN** `git rev-parse --abbrev-ref @{upstream}` prints `origin/main`
- **AND** the next `forkctl patch refresh` succeeds
- **AND** the next `forkctl publish` is a fast-forward

### Requirement: Missing tracking names its fix

A command that requires the declared branch SHALL fail with `invalid_request`
when the branch tracks nothing or the wrong ref, and SHALL suggest
`git branch --set-upstream-to=<remote>/<branch> <branch>`.

#### Scenario: Unset upstream

- **WHEN** the operator runs `git branch --unset-upstream` and then
  `forkctl --format json check`
- **THEN** the error message is
  `branch main tracks no upstream, expected origin/main`
- **AND** `error.suggested_command` is
  `git branch --set-upstream-to=origin/main main`
