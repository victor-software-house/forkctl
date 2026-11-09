## ADDED Requirements

### Requirement: Manifest fields name what varies

Manifest schema 2 SHALL name a patch's `source` or `tooling` value `layer`, a base target's `commit`, `branch`, or `tag` value `type`, and a history entry's tag `event`.
The operation journal SHALL name the command that started an operation `command` and its intent's tag `action`, and the API SHALL use protocol version 2 with the same field names.
forkctl SHALL refuse a manifest that uses `kind` for any of them.

#### Scenario: The example manifest reads

- **WHEN** `patches/fork.yaml` holds `examples/fork.yaml`, which declares `schema: 2`, `type: branch`, and `layer: source`
- **THEN** `forkctl --format json status` reports no `manifest_error`

#### Scenario: The layer flag

- **WHEN** the operator runs `forkctl patch create draft -l source -p P -u not-submitted -d D -s 'src/**'`
- **THEN** the active draft's metadata has `layer` `source`

### Requirement: A schema 1 manifest names the upgrade

When the tracked manifest declares `schema: 1` and does not read as schema 2, forkctl SHALL report `manifest_invalid` with a message that names `mise run fork upgrade`.

#### Scenario: Status names the upgrade

- **WHEN** the bookkeeping patch holds a schema 1 `patches/fork.yaml` and the operator runs `forkctl --format json status`
- **THEN** `result.manifest_error` contains `mise run fork upgrade`

### Requirement: Upgrade rewrites a schema 1 manifest in place

`forkctl upgrade` SHALL rewrite a schema 1 manifest as schema 2 in its own format.
In a YAML manifest it SHALL change only the schema number and each `kind` key it renames, and SHALL keep every other byte, comments and flow collections included.
It SHALL rewrite a draft active patch's metadata the same way, regenerate the ledger and every export, refresh the bookkeeping patch, and run the full check.
`forkctl upgrade -n` SHALL report the plan and write nothing.

#### Scenario: The upgrade restores the current text

- **WHEN** the bookkeeping patch holds the schema 1 form of a manifest with a comment, two patches, and a `patch_removed` history entry, and the operator runs `forkctl upgrade`
- **THEN** `patches/fork.yaml` equals its schema 2 form byte for byte, `PATCHES.md` has the `Layer` column, `git status --porcelain` is empty, and `forkctl check` succeeds
- **AND** `result.from_schema` is 1 and `result.to_schema` is 2

#### Scenario: A plan writes nothing

- **WHEN** the manifest is schema 1 and the operator runs `forkctl upgrade -n`
- **THEN** the command succeeds and `patches/fork.yaml` is unchanged

### Requirement: Upgrade refuses what it cannot finish

`forkctl upgrade` SHALL fail with `operation_conflict` and write nothing while an operation journal exists, because that journal belongs to the forkctl that started it.
It SHALL fail with `manifest_invalid` and write nothing for a manifest whose schema is neither 1 nor 2.
On a schema 2 manifest it SHALL succeed and change nothing.

#### Scenario: An operation is in progress

- **WHEN** the manifest is schema 1, `$(git rev-parse --git-path forkctl/operation.json)` exists, and the operator runs `forkctl --format json upgrade`
- **THEN** `error.code` is `operation_conflict` and `patches/fork.yaml` is unchanged

#### Scenario: An unknown schema

- **WHEN** `patches/fork.yaml` declares `schema: 3` and a client calls `api call` with command `upgrade`
- **THEN** `error.code` is `manifest_invalid` and the repository is unchanged

#### Scenario: Already current

- **WHEN** the manifest is schema 2 and the operator runs `forkctl --format json upgrade`
- **THEN** `result.from_schema` is 2 and `result.check` is null
