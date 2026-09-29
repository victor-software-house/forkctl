# Bootstrap downstream tracking

## Why

Bootstrapping a downstream fork from a fresh upstream clone needed two manual
steps. First, `init` failed with `branch tracks upstream/main, expected
origin/main`, after it had already written StGit state. Second, `patch
refresh` failed with `no upstream configured for branch 'main'`. The workaround
pushed the plain upstream base to the downstream remote by hand, with the
downstream CI disabled, and set tracking by hand. Work row: `FRK-005` in
[`tasks.yaml`][ledger].

Protocol version: unchanged.

## What Changes

1. When the downstream remote has no downstream branch, bootstrap `init`
   creates it at the exact base commit. The push carries
   `--force-with-lease=refs/heads/<branch>:`, which requires the branch to
   still be absent.
2. Bootstrap `init` fetches the downstream branch and sets the current
   branch's upstream to `<downstream remote>/<branch>` before it writes any
   StGit state.
3. The dry-run plan lists the creation push under `ref_updates` and the
   tracking change under `writes`.
4. Every command that requires the declared branch reports a missing or wrong
   upstream the same way, and suggests
   `git branch --set-upstream-to=<remote>/<branch> <branch>`.

## Capabilities

### New Capabilities

- `bootstrap`: this change adds the tracking requirements. The
  `capture-bootstrap-configuration` change adds the capture requirement to the
  same capability.

### Modified Capabilities

None.

## Impact

1. Code: `bootstrap` in [`src/app/init.rs`][init], `require_declared_branch`
   in [`src/app/mod.rs`][mod], and a new `wrong_tracking` error in
   [`src/error.rs`][error].
2. Tests: the fresh-upstream-clone lifecycle test bootstraps, refreshes a
   patch, and publishes as a fast-forward. Another test removes the upstream
   and reads the suggested command.
3. Operator surfaces: the `init` paragraph of the
   [instructions template][instructions-template].
4. Creating the downstream branch runs the consumer's `pre-push` hooks and any
   downstream CI on push, once, for the unmodified upstream base.

[ledger]: ../../../tasks.yaml
[init]: ../../../src/app/init.rs
[mod]: ../../../src/app/mod.rs
[error]: ../../../src/error.rs
[instructions-template]: ../../../.ctl/operator/instructions.md.jinja
