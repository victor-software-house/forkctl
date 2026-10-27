# Design

## Context

[`export_patch`][mod] runs `stg export --stdout --template <template> <patch>`.
StGit builds the body with `git diff-tree -p --color=never <stgit.diff-opts> <--diff-opt values> --binary` and the diffstat with `git apply --stat --summary`.
`check` regenerates every source export and compares it with the tracked file; `patch refresh` and the stack transitions write them.

## Decisions

### 1. Full object ids through `--full-index`

`stg export --diff-opt --full-index` makes each `index` line carry the full blob ids, which depend only on file contents.

Alternative considered: a fixed abbreviation, through `--abbrev=N` or `core.abbrev`.
It lost because git lengthens an abbreviation that is ambiguous in the clone, so the output still depends on which objects the clone holds.

### 2. An empty `stgit.diff-opts` through `GIT_CONFIG_COUNT`

`export_patch` sets `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=stgit.diff-opts`, and an empty `GIT_CONFIG_VALUE_0`.
Configuration from the environment outranks every configuration file, and StGit reads the empty value as no options.
`process::command` already removes an inherited `GIT_CONFIG_COUNT`, so a caller's environment cannot add entries.

Alternative considered: `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`.
It lost because it leaves the repository's own `.git/config` in force, and it drops unrelated settings StGit may need.

Alternative considered: build the export from `git diff-tree` and `git apply --stat` in forkctl, without `stg export`.
It lost because forkctl delegates patch mechanics to StGit and would have to reproduce StGit's template fields.

### 3. No migration step

The first `patch refresh` after the upgrade rewrites every export, as any refresh already does.
The project keeps no compatibility reader for older forkctl contracts, so `check` does not accept the abbreviated form.

No automatic behaviour is added.
Exports are generated evidence and have no override or disable path by design.

[mod]: ../../../../src/app/mod.rs
