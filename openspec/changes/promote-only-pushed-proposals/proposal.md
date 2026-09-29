# Promote only pushed proposals

## Why

`publish --propose` moved the local `refs/heads/forkctl/proposal/<branch>` to
the new proposal commit before it pushed. When the leased push was rejected,
the local ref kept a commit that no reviewer saw. `publish --promote` reads
the local ref first, so it could promote that commit. Work row: `FRK-010` in
[`tasks.yaml`][ledger].

Protocol version: unchanged. `PublishResult.pushed_refs` keeps its type. Its
proposal entry now reads `<commit>:refs/heads/forkctl/proposal/<branch>`, the
refspec that was actually pushed.

## What Changes

1. The proposal push sends the commit by object ID.
2. The local proposal ref moves only after the remote accepted the push.

## Capabilities

### New Capabilities

- `publish-proposal`: adds one requirement to the capability that
  `update-open-proposal` introduces.

### Modified Capabilities

None.

## Impact

1. Code: `publish_propose` in [`src/app/publish.rs`][publish].
2. Tests: the lease race test asserts that no local proposal ref exists after
   the rejected push. The test fails on the previous code.

[ledger]: ../../../tasks.yaml
[publish]: ../../../src/app/publish.rs
