# Proposal

## Why

[ctl-core][ctl-core] 0.6.11 lets a repository change the YAML reader's limits under `limits:` in `.ctl/input.yaml`, through `ctl_core::input::use_repo_limits`.
A tool that does not call it ignores the repository's file, so a repository whose manifest needs more than serde-saphyr's default depth (64) or node count cannot use forkctl, and a repository that wants tighter limits gets none.
ctl-core returns each limit the file raises above the default, and asks every tool to print it on every run: that warning is the operator's only signal that a repository loosened its reader.

## What Changes

- `App::discover` calls `use_repo_limits` once the repository root is known, so every later parse applies the repository's limits.
- forkctl prints each loosened limit to stderr on every run, as `forkctl: warning: .ctl/input.yaml raises max_depth to 128, above the default 64`.
- A malformed `.ctl/input.yaml` blocks the manifest with `manifest_invalid`, the way an unreadable manifest does, so `operation status`, `continue`, and `abort` still run from the Git-private snapshot.
- ctl-core moves from 0.6.10 to 0.6.11.

Protocol version 1 is unchanged: no request, result, or error shape changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `manifest-edits`: the reader applies the repository's input limits and warns when they loosen the default.

## Impact

- [`src/app/mod.rs`][mod]: `App::discover` installs the limits and keeps the warnings.
- [`src/main.rs`][main]: prints the warnings, since `src/app/` never prints.
- `Cargo.toml`: ctl-core `=0.6.11`.
- [`README.md`][readme], `AGENTS.md`, and the operator instructions template.

[ctl-core]: https://github.com/victor-software-house/ctl-core
[mod]: ../../../../src/app/mod.rs
[main]: ../../../../src/main.rs
[readme]: ../../../../README.md
