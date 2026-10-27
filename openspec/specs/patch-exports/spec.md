# patch-exports Specification

## Purpose

What a generated source patch export contains, independent of the clone that writes it.

## Requirements

### Requirement: A source patch export does not depend on the clone

forkctl SHALL write each source patch export with full object ids in every `index` line, and SHALL ignore `stgit.diff-opts` from every Git configuration file when it writes or checks an export.

#### Scenario: Index lines carry full object ids

- **WHEN** the operator creates source patch `source-change` that adds `source.txt`, and runs `forkctl patch refresh` and `forkctl patch finish`
- **THEN** the `index` line of `patches/downstream/0001-source-change.patch` names two object ids of 40 hexadecimal characters

#### Scenario: Clone configuration does not change an export

- **WHEN** the operator then runs `git config core.abbrev 12` and `git config stgit.diff-opts --no-prefix` in the clone
- **AND** runs `forkctl check`
- **THEN** it succeeds
