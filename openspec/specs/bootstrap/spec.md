# bootstrap Specification

## Purpose
Defines how `forkctl init` without a manifest turns a clone at its upstream
base into a downstream stack with a bookkeeping patch, tracking, and a
downstream branch.

## Requirements

### Requirement: Bootstrap captures untracked bookkeeping files

Bootstrap `init` SHALL add every untracked file that matches the bookkeeping
patch scope to the bookkeeping patch. The manifest path, the ledger path, and
everything under the exports directory are excluded, because bootstrap writes
them; an untracked file at any of those paths SHALL refuse. It SHALL fail with `dirty_worktree`
before any mutation when a staged change, an unstaged change, or an untracked
file outside that scope exists, and the error SHALL list only those paths.

#### Scenario: Untracked mise configuration

- **WHEN** the only change in the clone is an untracked `mise.toml` and the
  operator runs `forkctl init ... --bookkeeping-path mise.toml`
- **THEN** the command succeeds
- **AND** `git ls-files --error-unmatch mise.toml` succeeds
- **AND** the worktree is clean

#### Scenario: Unrelated untracked file

- **WHEN** the clone has an untracked `mise.toml` and an untracked `notes.txt`,
  and `notes.txt` matches no bookkeeping scope
- **THEN** `forkctl init ...` fails with `dirty_worktree` whose paths are
  exactly `["notes.txt"]`
- **AND** no manifest file exists and the downstream remote is unchanged

#### Scenario: Untracked ledger

- **WHEN** the clone has an untracked `PATCHES.md` and the ledger is `PATCHES.md`
- **THEN** `forkctl init ...` fails with `dirty_worktree` naming `PATCHES.md`
- **AND** `PATCHES.md` keeps its content

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
