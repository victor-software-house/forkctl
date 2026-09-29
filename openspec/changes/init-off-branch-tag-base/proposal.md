# Proposal

## Why

Some upstream projects cut each release as a version-bump commit on top of their main branch and tag that commit without merging it back. A downstream that bootstraps on such a tag fails. Bootstrap `init` records the tag commit itself as the canonical base, but `check` defines the canonical base as the merge base of the stack base and the upstream branch. On one real downstream the two values differed by one commit, and every later `check` and `contract edit` failed with `canonical merge base is <branch commit>, expected <tag commit>`.

Bootstrap also ran its first check only after it had pushed the downstream branch, written the manifest, and created the bookkeeping patch. The failure therefore left a half-initialised clone. In a clone that had never fetched the upstream branch, it failed as `subprocess_failed` instead.

## What Changes

- Bootstrap `init` fetches the declared upstream branch and records the canonical base as `git merge-base <base> <upstream tracking ref>`, the same value `rebase` records.
- Before it pushes or writes anything, bootstrap checks that the diff from the canonical base to the base touches only paths in `--allow-base`. It fails with `check_failed` (`pre-stack drift is outside scope: <path>`) otherwise, in both execute and plan mode.
- When the base lies on the upstream branch, the merge base is the base itself and nothing changes.
- `--upstream-ref` stays a branch ref. A tag is selected with `--base`, and later with `rebase --onto`.

Protocol version 1 is unchanged: no request, result, or error shape changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `bootstrap`: bootstrap accepts a base outside the upstream branch and settles the canonical base and its drift before any mutation.

## Impact

- [`src/app/init.rs`][init]: canonical base, upstream fetch, and drift check before the push.
- [`src/app/mod.rs`][mod]: the upstream tracking ref and fetch become usable before a manifest exists.
- [`src/app/check.rs`][check]: the drift check is shared with bootstrap.
- [`tests/lifecycle.rs`][lifecycle]: bootstrap and rebase on off-branch release tags.
- [`.ctl/operator/instructions.md.jinja`][instructions]: the `init` paragraph.

[init]: ../../../src/app/init.rs
[mod]: ../../../src/app/mod.rs
[check]: ../../../src/app/check.rs
[lifecycle]: ../../../tests/lifecycle.rs
[instructions]: ../../../.ctl/operator/instructions.md.jinja
