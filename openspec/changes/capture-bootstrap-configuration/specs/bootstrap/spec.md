# Spec Delta

## Purpose

Defines how `forkctl init` without a manifest turns a clone at its upstream
base into a downstream stack with a bookkeeping patch, tracking, and a
downstream branch.

## ADDED Requirements

### Requirement: Bootstrap captures untracked bookkeeping files

Bootstrap `init` SHALL add every untracked file that matches the bookkeeping
patch scope to the bookkeeping patch. The manifest and ledger paths are excluded,
because bootstrap writes them; an untracked file at either path SHALL refuse. It SHALL fail with `dirty_worktree`
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
