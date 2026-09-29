# Update an open proposal in place

## Why

A downstream fork that publishes with `downstream.publish: propose` cannot
update its proposal. The second `publish --propose` fails with
`! [rejected] forkctl/proposal/main (non-fast-forward)`. The only workaround is
to close the pull request, delete the branch, and propose again, which loses
the review history. That breaks a daily rebase-as-pull-request flow on its
second day. Work row: `FRK-001` in [`tasks.yaml`][ledger].

Protocol version: unchanged. `PublishResult` keeps its fields.

## What Changes

1. `publish --propose` reads the remote proposal branch tip before it pushes.
2. The proposal push carries `--force-with-lease` on the proposal ref, expecting
   the tip it read, or expecting the ref to be absent when it read none.
3. The proposal commit keeps its shape: one commit whose parent is the fetched
   downstream tip and whose tree is the candidate tree. `publish --promote` is
   unchanged.

## Capabilities

### New Capabilities

- `publish-proposal`: how `publish --propose` creates and updates the
  proposal branch.

### Modified Capabilities

None.

## Impact

1. Code: `publish_propose` in [`src/app/publish.rs`][publish].
2. Tests: the proposal lifecycle test in [`tests/lifecycle.rs`][lifecycle]
   proposes twice and races a concurrent proposal update.
3. Operator surfaces: the publish paragraph of
   [`src/instructions.md`][instructions] and its
   [template][instructions-template].
4. A pull request on GitHub stays open across updates. Earlier review comments
   remain and show as outdated.

[ledger]: ../../../tasks.yaml
[publish]: ../../../src/app/publish.rs
[lifecycle]: ../../../tests/lifecycle.rs
[instructions]: ../../../src/instructions.md
[instructions-template]: ../../../.ctl/operator/instructions.md.jinja
