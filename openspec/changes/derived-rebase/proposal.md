# Proposal

## Motivation

The [`derived-paths`][derived-paths] change puts a fork's build outputs in one forkctl-owned derived patch on top of the stack and rebuilds it with `forkctl derive`.
A rebase still replays that patch like any other, so an upstream release that changes the outputs conflicts on them, as the [mise-action fork][mise-action] did on `dist/index.js` when it moved onto v5.2.7.
The derived patch is a function of the tree below it, so a rebase has nothing to merge in it: it can drop the patch and build it again from the new tree.

## Changes

- `rebase` removes the derived patch before the replay and derives once after it, in a new phase `deriving`, so a rebase never conflicts on an output.
- A failing build stops the rebase in phase `deriving`; `operation continue` derives again, and `operation abort` restores the old stack.
- The rebase result and report gain a `derived` field with the input hash, the rebuild hash, and whether the result came from the cache.
- `rebase --verify-base` also rebuilds at the new base, and the report says whether upstream's committed outputs match its own sources.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `derived-paths`: deriving during rebase and verifying upstream's outputs.

## Impact

- [`src/app/rebase.rs`][rebase] and [`src/app/operation.rs`][operation]: the `deriving` phase, its continue and abort, and `--verify-base`.
- [`src/cli.rs`][cli] and [`src/protocol.rs`][protocol]: the flag, the phase, and the result field.
- [`src/instructions.md`][instructions], the operator skill, and [`README.md`][readme].
- It ships in the same release as `derived-paths`, and no consumer must act.

[derived-paths]: ../derived-paths/proposal.md
[mise-action]: https://github.com/victor-software-house/mise-action
[rebase]: ../../../src/app/rebase.rs
[operation]: ../../../src/app/operation.rs
[cli]: ../../../src/cli.rs
[protocol]: ../../../src/protocol.rs
[instructions]: ../../../src/instructions.md
[readme]: ../../../README.md
