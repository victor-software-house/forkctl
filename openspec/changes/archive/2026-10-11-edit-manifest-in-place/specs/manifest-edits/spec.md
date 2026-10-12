## ADDED Requirements

### Requirement: A YAML manifest is read through ctl-core

forkctl SHALL parse a YAML manifest through ctl-core's input reader and SHALL report its problems as `manifest_invalid` with the line and column ctl-core gives.
forkctl SHALL accept anchors, aliases, and any consistent indentation, and SHALL refuse duplicate keys and merge keys. forkctl SHALL set no reader limits of its own; the limits are those ctl-core applies.

#### Scenario: An anchored check is accepted

- **WHEN** `patches/fork.yaml` declares a check under `&lint` on one patch and `*lint` on another, and the operator runs `forkctl --format json check`
- **THEN** it does not fail with `manifest_invalid`

#### Scenario: A duplicate key is refused with its line

- **WHEN** `patches/fork.yaml` repeats `schema: 1` on line 2 and the operator runs `forkctl --format json status`
- **THEN** `result.manifest_error` names `patches/fork.yaml` and line 2

### Requirement: A command changes only the manifest nodes it changes

When a command changes an existing YAML manifest, forkctl SHALL rewrite only the nodes that command changes, and SHALL leave every other byte of the file unchanged, including comments, key order, quoting, and blank lines.
The written file SHALL parse back to the manifest the command reports; otherwise forkctl SHALL fail with `manifest_invalid` and SHALL NOT write the file.

#### Scenario: Refresh of a new patch keeps comments

- **WHEN** `patches/fork.yaml` has a comment line `# owned by the platform team` above `downstream:` and a trailing comment after `upstream:`, and the operator creates patch `docs-banner`, stages a change in its scope, and runs `forkctl patch refresh`
- **THEN** the new manifest equals the old one with one item inserted under `patches:`, and both comments are unchanged

#### Scenario: Setting the publish default edits one key

- **WHEN** the operator runs `forkctl publish --set-default fast-forward` on a manifest with no `publish` key
- **THEN** the only change to `patches/fork.yaml` is a `publish: fast-forward` line under `downstream:`
- **AND** `forkctl publish --set-default rewrite` removes that line and restores the original bytes

#### Scenario: A rebase edits the base and nothing else

- **WHEN** the operator runs `forkctl rebase` to a new upstream commit and no patch is dropped or loses paths
- **THEN** the only changes to `patches/fork.yaml` are the values of `base.target`, `base.stack`, and `base.canonical`

### Requirement: Operations edit the manifest text they started from

An operation SHALL apply its manifest edits to the tracked manifest text captured when the operation started, so `operation continue` writes the same bytes whatever the stack in flight left on disk.

#### Scenario: Disable after the bookkeeping patch is popped

- **WHEN** the manifest carries comments and the operator runs `forkctl patch disable docs-banner --reason obsolete --yes`
- **THEN** the written `patches/fork.yaml` keeps every comment, lacks the `docs-banner` patch item, and gains one `disabled_patches` item

#### Scenario: An operation started without the snapshot

- **WHEN** `forkctl/manifest.source` is missing under the Git directory while a `patchrefresh` operation is stopped on a conflict, and the operator resolves it and runs `forkctl --format json operation continue`
- **THEN** it fails with `error.code` equal to `operation_conflict`, and the message names `operation abort --yes`

### Requirement: An edit through an alias is refused

forkctl SHALL fail with `manifest_invalid`, and SHALL NOT write the manifest, when an edit would change an alias or a node that an alias refers to.

#### Scenario: Editing an anchored patch

- **WHEN** patch `docs-banner` is anchored as `&banner` and aliased elsewhere, and the operator runs `forkctl --format json patch edit docs-banner --description new`
- **THEN** it fails with `error.code` equal to `manifest_invalid`, saying to expand the anchor or alias by hand
- **AND** `patches/fork.yaml` is unchanged

### Requirement: Bootstrap and JSON manifests are written in full

`forkctl init` bootstrap SHALL write a new manifest in full, and a command that changes a `.json` manifest SHALL rewrite it in full with `serde_json`.

#### Scenario: Bootstrap writes a parseable manifest

- **WHEN** the operator runs `forkctl init --bootstrap` on a repository with no manifest
- **THEN** `patches/fork.yaml` exists and `forkctl check` reads it without `manifest_invalid`
