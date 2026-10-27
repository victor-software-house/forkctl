# Proposal

## Why

`AGENTS.md` states that every source patch has one deterministic generated export.
On forkctl 0.0.38 through 0.0.42, an export depends on the clone that writes it.

A downstream's exports were written in a clone where git abbreviated object ids to 8 characters.
A fresh clone of the same downstream, after fetching upstream's branches, held 105,851 packed objects, so git's automatic abbreviation used 9 characters.
`init` hydrated the stack, and `check` then failed with `export differs for patch target-projection-core`.
Regenerating the export with `core.abbrev=8` reproduces the committed body byte for byte; 7 and 9 differ in every `index` line.

The cause is [`export_patch`][mod], which runs `stg export` with git's default abbreviation.
`stg export` also reads `stgit.diff-opts` from any Git configuration and passes it to `git diff-tree`.
Setting `stgit.diff-opts=-U1` changed the same export, while `diff.algorithm`, `diff.context`, `diff.interHunkContext`, `diff.noprefix`, `diff.renames`, `diff.indentHeuristic`, and `core.quotePath` did not, because `git diff-tree` is plumbing and ignores them.

Measured on that downstream's largest export, 21 files, 20 runs each with StGit 2.6.1:

| Mode | Time per export | Size |
|:--|--:|--:|
| automatic abbreviation | 31.2 to 31.8 ms | 101,451 bytes |
| `--full-index` | 30.8 to 31.8 ms | 102,753 bytes |

Full ids add 62 bytes per `index` line and no measurable time; `git diff-tree` alone takes 8.8 ms of each export.

## What Changes

- `export_patch` passes `--diff-opt --full-index` to `stg export`, so every `index` line carries full object ids.
- `export_patch` runs `stg export` with `stgit.diff-opts` set to an empty value through `GIT_CONFIG_COUNT`, which outranks the system, user, and repository configuration files.
- `src/process.rs` gains `output_with_config`, the one way to run a child with such configuration.

Every existing export changes once: the first `patch refresh` after the upgrade rewrites its `index` lines.
Until that refresh, `check` reports `export differs`, as it does today in a clone with a different abbreviation length.

Protocol version 1 is unchanged.

## Capabilities

### New Capabilities

- `patch-exports`: what a generated source patch export contains, independent of the clone that writes it.

### Modified Capabilities

None.

## Impact

- [`src/app/mod.rs`][mod]: `export_patch`.
- [`src/process.rs`][process]: `output_with_config`.
- [`tests/lifecycle.rs`][lifecycle]: `patch_exports_do_not_depend_on_clone_configuration`.

[mod]: ../../../../src/app/mod.rs
[process]: ../../../../src/process.rs
[lifecycle]: ../../../../tests/lifecycle.rs
