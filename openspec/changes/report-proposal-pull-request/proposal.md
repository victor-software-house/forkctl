# Report the proposal pull request

## Why

`publish --propose` can push the proposal branch, open no pull request, and
still report success. On a downstream fork, `gh` was logged in as an account
without access to the repository. The branch was pushed, no pull request
existed, and forkctl printed no notice. Work row: `FRK-002` in
[`tasks.yaml`][ledger].

The pull-request step has four defects:

1. `open_proposal_pr` discards every failure. The caller applies
   `.ok().flatten()`, and a non-zero `gh` exit returns `Ok(None)`.
2. It runs `gh pr create` without `--repo`. `gh` then picks the base
   repository from the clone's remotes and prefers a remote named `upstream`
   over `origin`, so in a fork it can target the upstream repository.
3. It never looks for an open pull request on the proposal branch, so a
   repeated proposal cannot report the pull request it updated.
4. Its title and body are fixed literals that say nothing about the candidate.

It also runs `gh` through `std::process::Command` directly, although
[AGENTS.md][agents] makes `src/process.rs` the only production child-process
factory.

Protocol version: unchanged. `PublishResult.proposal_url` keeps its type and is
always set when `publish --propose` succeeds. A pull-request failure uses the
existing `subprocess_failed` error code.

## What Changes

1. Forkctl looks up the open pull request before it pushes. A downstream remote
   that is not a GitHub URL, a missing `gh`, or an account without access
   fails `publish --propose` before anything is pushed.
2. A failed create or edit after the push fails `publish --propose` with a
   non-zero exit. The error carries the `gh` stderr and says that the proposal
   branch was pushed.
3. Every `gh` call names the repository with `--repo`, derived from the
   downstream remote URL.
4. An open pull request from the proposal branch into the downstream branch is
   reused. Forkctl refreshes its title and body and reports its URL.
5. The title names the downstream branch and the candidate. The body lists the
   candidate, the downstream tip it replaces, the upstream base, the patches,
   and the promote command. The pull request stays a draft.

## Capabilities

### New Capabilities

- `publish-proposal`: this change adds the pull-request requirements. The
  `update-open-proposal` change adds the branch requirements to the same
  capability.

### Modified Capabilities

None.

## Impact

1. Code: `open_proposal_pr` in [`src/app/publish.rs`][publish], and a new
   Askama template for the pull-request body under [`templates/`][templates].
2. Tests: the lifecycle suite runs `publish --propose` against a stub `gh` on
   `PATH`, for success, reuse, and failure.
3. Operator surfaces: the publish paragraph of the
   [instructions template][instructions-template] and the `--propose` help
   text in [`src/cli.rs`][cli]. Both drop "when `gh` is available".
4. A downstream remote that is not a GitHub URL can no longer use
   `publish --propose`. It fails before the push instead of pushing a branch
   that no pull request can reach.
5. `publish --propose -n` still plans without calling `gh`.

[ledger]: ../../../tasks.yaml
[agents]: ../../../AGENTS.md
[publish]: ../../../src/app/publish.rs
[templates]: ../../../templates/
[instructions-template]: ../../../.ctl/operator/instructions.md.jinja
[cli]: ../../../src/cli.rs
