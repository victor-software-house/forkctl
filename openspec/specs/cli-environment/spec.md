# cli-environment Specification

## Purpose
How forkctl reads its options from the environment: every long option under the `FORKCTL_` prefix, with the command line winning, and `FORKCTL_MANIFEST` as the one manifest variable.

## Requirements

### Requirement: Every long option reads a FORKCTL variable

forkctl SHALL read each long option, global options included, from `FORKCTL_<LONG>` when the command line does not give it, where `<LONG>` is the option's long name in upper case with `-` replaced by `_`.
A flag on the command line SHALL win over its variable, and an empty variable SHALL count as unset.
Help, version, hidden options, and the `--no-*` side of a pair SHALL NOT read a variable.

#### Scenario: A variable sets the output format

- **WHEN** the operator runs `forkctl instructions` with `FORKCTL_FORMAT=json`
- **THEN** stdout is one JSON document whose `command` is `instructions`

#### Scenario: The command line wins over the variable

- **WHEN** the operator runs `forkctl --format pretty instructions` with `FORKCTL_FORMAT=json`
- **THEN** stdout is the pretty rendering, not JSON

### Requirement: FORKCTL_MANIFEST selects the manifest

forkctl SHALL select the manifest from `--manifest`, then `FORKCTL_MANIFEST`, then `patches/fork.yaml`, for CLI commands and for shell completion.
`api call` SHALL use the invocation's `manifest`, then `--manifest` or `FORKCTL_MANIFEST`, then `patches/fork.yaml`.
forkctl SHALL NOT read any other variable for the manifest.

#### Scenario: The variable selects a manifest outside the default path

- **WHEN** the repository's manifest is `patches/other.yaml` and the operator runs `forkctl --format json status` with `FORKCTL_MANIFEST=patches/other.yaml`
- **THEN** the command succeeds

#### Scenario: Without the variable the default path applies

- **WHEN** the repository's manifest is `patches/other.yaml` and the operator runs `forkctl --format json status` with no manifest variable
- **THEN** the command fails, because `patches/fork.yaml` does not exist
