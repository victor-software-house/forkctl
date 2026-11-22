# Proposal

## Why

forkctl writes four things into a patch commit message: the patch name and the `Downstream-Reason`, `Upstream-Status`, and `Drop-When` trailers.
`check` then requires those three trailers, and the manifest has no field for any other.
A downstream patch that carries code from another author's upstream pull request cannot credit that author with a `Co-authored-by` trailer.
A trailer added by amending the commit is lost at the next `patch edit`, because forkctl rewrites the message from the manifest.
The operator asked for manifest-declared extra trailers so that such a patch can credit its co-author (operator, 2026-11-22).

Protocol version 2 and manifest schema 2 stay as they are.
The new field is optional, so a manifest without it reads, writes, and checks exactly as before.

## What Changes

- Manifest: a patch may declare `trailers`, a list of `key` and `value` records.
- Commit message: each declared trailer follows `Drop-When` as `Key: value`, in manifest order, in the same trailer block.
- `check`: for each key a patch declares, the commit's values for that key must equal the manifest's values, in order. Trailers with keys the manifest never names stay out of scope.
- Validation: a key that names one of the three managed trailers, a key that is not a Git trailer token, an empty or multi-line value, and a repeated key and value pair are rejected.
- CLI: `patch create` and `patch edit` take a repeatable `--trailer KEY=VALUE` (`FORKCTL_TRAILER`). On `patch edit` it replaces the whole list, and `--clear-trailers` (`FORKCTL_CLEAR_TRAILERS`) removes it.
- JSON API: `patch.create` takes `trailers`, `patch.edit` takes a replacement `trailers` list, and patch results carry the field.
- Output: `patch show` prints the trailers, and `PATCHES.md` gains a `Trailers` section when any patch declares one.

## Capabilities

### New Capabilities

- `patch-trailers`: extra commit trailers a patch declares, how forkctl writes them, and how `check` verifies them.

### Modified Capabilities

None.

## Impact

- [`src/manifest.rs`][manifest], [`src/app/check.rs`][check], [`src/protocol.rs`][protocol], [`src/cli.rs`][cli], [`src/app/patch.rs`][patch], [`src/ledger.rs`][ledger], and [`src/presentation.rs`][presentation].
- [`templates/patches.md`][ledger-template].
- [`README.md`][readme], `AGENTS.md`, and the operator templates under `.ctl/operator/` with their renders.
- Consumers change nothing: an existing manifest, ledger, and commit stay valid.

[manifest]: ../../../src/manifest.rs
[check]: ../../../src/app/check.rs
[protocol]: ../../../src/protocol.rs
[cli]: ../../../src/cli.rs
[patch]: ../../../src/app/patch.rs
[ledger]: ../../../src/ledger.rs
[presentation]: ../../../src/presentation.rs
[ledger-template]: ../../../templates/patches.md
[readme]: ../../../README.md
