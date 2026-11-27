# Proposal

## Motivation

A fork of a JavaScript action commits a build output, such as `dist/index.js`, because the runner executes the committed file and never builds it.
That file is one generated bundle of every source file, so any upstream release that touches a dependency or a source file changes it.
Today the bundle lives inside a hand patch, and that patch conflicts on every such release, even when its source changes replay cleanly.

In the [mise-action fork][mise-action], the bundle is 5,127,133 bytes and its source map 26,366,422 bytes.
Upstream changed both in its v5.2.7 release, so the rebase onto v5.2.7 stopped on `dist/index.js` and `dist/index.js.map` while `src/index.ts` merged.
The resolution never needs judgment: take upstream's copy, run the build, and refresh the patch.
forkctl has no way to declare that, so the operator resolves a conflict in a file nobody can read, by hand, on every rebase.

A committed bundle can also go stale without any conflict.
When a patch changes a source file, the bundle no longer matches the sources, and nothing in `forkctl check` notices.

Prior art treats generated output as a function of its inputs, never as content to merge; [research.md][research] lists it.

The change is additive to manifest schema 2 and protocol version 2.
The rebase that drops the derived patch and derives after the replay is the [`derived-rebase`][derived-rebase] change, and both ship in one release.

## Changes

- The manifest may declare one `derived` block: `outputs` (globs), `rebuild` (commands that run in order), and optional `inputs` (globs).
  With no `inputs`, every tracked file outside `outputs` and outside forkctl's own documents is an input.
- forkctl owns one derived patch on top of the stack, above the bookkeeping patch.
  It holds exactly the outputs, nobody edits it, and no other patch may touch an output.
- forkctl builds the outputs in a disposable clone of the stack below the derived patch, once per input state, and commits them as the derived patch.
  The commit records the input hash and the rebuild hash as trailers, and a result is cached by both, so a state built once is never built again.
- `forkctl derive` rebuilds on request, and `patch finish` derives before its final check.
- A complete `check` compares the recorded hashes with the stack and reports `derived_stale` with no build; `check --rebuild` also rebuilds in a clone and reports `derived_unreproducible` when the result differs.
- `contract edit` takes `--derived-output`, `--derived-input`, `--derived-rebuild`, and `--clear-derived`.

## Capabilities

### New Capabilities

- `derived-paths`: declaring derived outputs, the derived patch, deriving during rebase and on request, and checking freshness.

### Modified Capabilities

None.

## Impact

- [`src/manifest.rs`][manifest] and [`src/manifest_edit.rs`][manifest-edit]: the `derived` block and its validation.
- A new `src/app/derive.rs`: input hashing, the clone build, the cache, and the derived patch.
- [`src/app/operation.rs`][operation] and [`src/app/patch.rs`][patch]: the `derive` operation and the derive in `patch finish`.
- [`src/app/check.rs`][check]: the derived findings and `--rebuild`.
- [`src/app/contract.rs`][contract], [`src/cli.rs`][cli], and [`src/protocol.rs`][protocol]: the `derive` command, the new flags, results, and finding codes.
- The rule that the bookkeeping patch is on top becomes: on top, or directly below the derived patch.
- [`src/instructions.md`][instructions], the operator skill, [`AGENTS.md`][agents], [`README.md`][readme], and [`examples/fork.yaml`][example].
- No consumer must act. A fork adopts the block when it declares `derived`; a fork that carries outputs in a hand patch moves them out first.

[mise-action]: https://github.com/victor-software-house/mise-action
[derived-rebase]: ../derived-rebase/proposal.md
[research]: research.md
[manifest]: ../../../src/manifest.rs
[manifest-edit]: ../../../src/manifest_edit.rs
[operation]: ../../../src/app/operation.rs
[patch]: ../../../src/app/patch.rs
[check]: ../../../src/app/check.rs
[contract]: ../../../src/app/contract.rs
[cli]: ../../../src/cli.rs
[protocol]: ../../../src/protocol.rs
[instructions]: ../../../src/instructions.md
[agents]: ../../../AGENTS.md
[readme]: ../../../README.md
[example]: ../../../examples/fork.yaml
