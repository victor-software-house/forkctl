# Capture bootstrap configuration

## Why

`init` refuses any dirty worktree, including an untracked `mise.toml`. That
file is what provides `mise run fork`, so a downstream fork had to hide it to
bootstrap and then commit it by hand. Work row: `FRK-004` in
[`tasks.yaml`][ledger].

Protocol version: unchanged.

## What Changes

1. Bootstrap `init` accepts untracked files that match the bookkeeping patch
   scope: the manifest, the ledger, the exports directory, and every
   `--bookkeeping-path`. It adds them to the bookkeeping patch.
2. Staged or unstaged changes, and untracked files outside that scope, still
   fail with `dirty_worktree`, which lists only those paths.
3. The dry-run plan lists the captured files under `writes`.
4. Hydrating `init` with a manifest is unchanged and still requires a clean
   worktree.

## Capabilities

### New Capabilities

- `bootstrap`: how `init` without a manifest creates the downstream stack.

### Modified Capabilities

None.

## Impact

1. Code: `bootstrap` in [`src/app/init.rs`][init].
2. Tests: a lifecycle test bootstraps a fresh upstream clone with an
   untracked `mise.toml` and is refused only for an unrelated file.
3. Operator surfaces: the `init` paragraph of the
   [instructions template][instructions-template].

[ledger]: ../../../tasks.yaml
[init]: ../../../src/app/init.rs
[instructions-template]: ../../../.ctl/operator/instructions.md.jinja
