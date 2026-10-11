# Proposal

## Why

forkctl rewrites the whole YAML manifest on every command that changes it.
[`write_manifest`][mod] serializes the typed `Manifest` with serde-saphyr and replaces the file, so an operator's comments, key order, quoting, and blank lines are lost the first time `patch refresh`, `patch edit`, `contract edit`, `rebase`, `publish --set-default`, or a patch transition runs.

To make that rewrite safe, [`manifest_codec`][codec] refuses YAML features a rewrite would drop or change: anchors and aliases, odd indentation, and merge keys.
Those bans protect the rewrite, not the reader.

qctl already edits its ledger in place through [yamled][yamled] and reads it through `ctl_core::input`.
The ctl family is converging on that pair (CTC-008), with the reader's safety limits moving into ctl-core for every tool.

## What Changes

- forkctl reads a YAML manifest through `ctl_core::input` and maps its problems to `manifest_invalid`. Its own serde-saphyr reader and the anchor, alias, and even-indentation bans go away. Merge keys and duplicate keys stay refused, by ctl-core's rules for every tool. forkctl drops its own tighter budget (2 MiB, depth 32, 100,000 nodes, one document). The reader limits become serde-saphyr's library defaults, which ctl-core makes overridable in code and in `.ctl/` configuration.
- Every command that changes an existing YAML manifest edits only the nodes it changes, through yamled. Comments, key order, and formatting elsewhere in the file survive.
- An edit that would land on an alias, or inside an anchored node, fails with `manifest_invalid` instead of de-aliasing the file.
- Bootstrap `init` still writes a new manifest in full, because there is no earlier text to keep.
- JSON manifests keep a full `serde_json` rewrite: JSON has no comments to keep.

Protocol version 1 is unchanged: no request, result, or error shape changes. A YAML manifest that used anchors, aliases, or odd indentation, refused before, is now accepted.

## Capabilities

### New Capabilities

- `manifest-edits`: how forkctl reads a manifest, and how a command that changes it preserves what it does not change.

### Modified Capabilities

None.

## Impact

- [`src/manifest_codec.rs`][codec]: the YAML reader moves to `ctl_core::input`; the serializer stays for bootstrap and JSON.
- A new manifest document module: one typed edit per kind of change, applied through yamled.
- [`src/app/mod.rs`][mod]: `write_manifest` applies the recorded edits; the commands that mutate the manifest record them.
- `Cargo.toml`: ctl-core with the `input` feature at its next patch, and yamled 0.0.5.
- [`README.md`][readme]: the YAML parsing paragraph.

[codec]: ../../../../src/manifest_codec.rs
[mod]: ../../../../src/app/mod.rs
[readme]: ../../../../README.md
[yamled]: https://github.com/victor-software-house/yamled
