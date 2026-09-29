# Spec Delta

## Purpose

Defines how `forkctl publish --propose` pushes a reviewable proposal branch
for the current stack and keeps that branch current across repeated proposals.

## ADDED Requirements

### Requirement: Proposal pull requests target the downstream repository

Every `gh` call made by `publish --propose` SHALL pass `--repo` with the host,
owner, and name parsed from the configured URL of the downstream remote. When
that URL is not an `https://`, `http://`, `ssh://`, or scp-style URL with an
owner and a repository name, `publish --propose` SHALL fail before it pushes.
The host SHALL carry no port, and the error SHALL carry no user information
from the URL.

#### Scenario: Fork clone with an upstream remote

- **WHEN** `origin` is `git@github.com:example/downstream.git`, the clone also
  has a remote named `upstream`, and the operator runs
  `forkctl publish --propose`
- **THEN** every `gh` invocation includes `--repo github.com/example/downstream`

#### Scenario: Local path remote

- **WHEN** the downstream remote URL is a local path such as `/srv/git/fork.git`
- **THEN** `forkctl publish --propose` fails with `invalid_request`
- **AND** the remote has no `refs/heads/forkctl/proposal/main`

#### Scenario: Remote URL with a token

- **WHEN** the downstream remote URL is
  `https://user:token@gitlab.example.com/group/sub/repo.git`
- **THEN** `forkctl publish --propose` fails with `invalid_request`
- **AND** the error message contains `https://gitlab.example.com/group/sub/repo.git`
  and not `token`

### Requirement: Pull-request access is checked before the push

`publish --propose` SHALL look up the open pull request from the proposal
branch into the downstream branch before it pushes. When `gh` is missing or the
lookup fails, the command SHALL fail with `subprocess_failed`, carry the `gh`
stderr, and push nothing.

#### Scenario: gh cannot see the repository

- **WHEN** `gh pr list --repo github.com/example/downstream` exits non-zero with
  `HTTP 404: Not Found`
- **THEN** `forkctl publish --propose` exits non-zero with `subprocess_failed`
- **AND** the error message contains `HTTP 404: Not Found`
- **AND** the remote proposal branch is unchanged

#### Scenario: gh is not installed

- **WHEN** `PATH` holds `git` and `stg` but no `gh`
- **THEN** `forkctl publish --propose` exits non-zero with `subprocess_failed`
- **AND** the remote has no `refs/heads/forkctl/proposal/main`

### Requirement: A pull-request failure after the push is an error

When creating or editing the pull request fails after the proposal branch was
pushed, `publish --propose` SHALL exit non-zero with `subprocess_failed`. The
message SHALL name the pushed proposal branch and include the `gh` stderr, and
the error SHALL be retryable and suggest `forkctl publish --propose`.

#### Scenario: Create fails after the push

- **WHEN** no pull request is open and `gh pr create` exits non-zero
- **THEN** `forkctl --format json publish --propose` exits non-zero
- **AND** the JSON error has code `subprocess_failed` and
  `retryable: true`
- **AND** its message names `forkctl/proposal/main` as pushed

### Requirement: An open proposal pull request is reused

When a pull request from the proposal branch of the downstream repository into
the downstream branch is open, `publish --propose` SHALL update its title and
body. A pull request from a branch of the same name in another repository SHALL
not be reused. Forkctl SHALL create no other pull request and SHALL report the
reused pull request's URL as `proposal_url`.

#### Scenario: Second proposal with an open pull request

- **WHEN** a pull request from `forkctl/proposal/main` into `main` is open and
  the operator runs `forkctl --format json publish --propose`
- **THEN** forkctl runs `gh pr edit` on that pull request and never
  `gh pr create`
- **AND** `result.proposal_url` is that pull request's URL

#### Scenario: Same branch name in another repository

- **WHEN** the only open pull request with head `forkctl/proposal/main` comes from
  another repository
- **THEN** `forkctl publish --propose` runs `gh pr create` for its own branch

### Requirement: The pull request describes the candidate

A created or updated proposal pull request SHALL be a draft. Its title SHALL
name the downstream branch and the short candidate commit. Its body SHALL list
the candidate commit, the downstream tip it replaces, the upstream base
selector and commit, each patch with its kind and purpose, and the command
`mise run fork publish --promote`.

#### Scenario: New pull request text

- **WHEN** the stack has the source patch `source-change` and the operator runs
  `forkctl publish --propose` with no open pull request
- **THEN** `gh pr create` receives `--draft`
- **AND** the title starts with `forkctl: propose main at `
- **AND** the body contains `source-change` and
  `mise run fork publish --promote`

### Requirement: Planning makes no pull-request calls

`publish --propose -n` SHALL plan without running `gh`.

#### Scenario: Dry run

- **WHEN** the operator runs `forkctl publish --propose -n` with a `gh` on `PATH`
  that records every invocation
- **THEN** the command succeeds and prints the plan
- **AND** `gh` recorded no invocation
