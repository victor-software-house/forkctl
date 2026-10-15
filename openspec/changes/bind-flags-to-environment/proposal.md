# Proposal

## Why

A caller that keeps its settings in the environment, such as a mise `[env]` table, a reusable workflow, or an action, could set only one forkctl option that way: the manifest, through a hand-written `FORK_MANIFEST` fallback in [`src/main.rs`][main] and [`src/completion.rs`][completion].
Every other option had to be rebuilt into argv by the caller.
[ctl-core][ctl-core] 0.6.21 binds every long option of a parser to `<PREFIX>_<LONG>` through `EnvPrefix`, and its `App` removes each variable the command line overrules before the parse, so the command line wins.
The ctl CLIs adopt one prefix per CLI, and the operator chose `FORKCTL_MANIFEST` over keeping `FORK_MANIFEST`, with no alias.

## What Changes

- The root parser carries `env_prefix = "FORKCTL"`, so every long option reads `FORKCTL_<LONG>`: `FORKCTL_FORMAT=json`, `FORKCTL_QUIET=true`, `FORKCTL_MANIFEST=patches/fork.yaml`.
- `FORK_MANIFEST` is removed, with no alias. The hand-written fallback in `src/main.rs` is gone; clap reads `FORKCTL_MANIFEST` for `--manifest`. Shell completion, which runs before the parse, reads `FORKCTL_MANIFEST`.
- `api call` falls back to `--manifest`, or `FORKCTL_MANIFEST`, when the invocation names no manifest.
- Integration tests remove every `FORKCTL_*` variable from the child, so a caller's environment cannot change what a test runs.
- ctl-core moves from 0.6.17 to 0.6.21.

Protocol version 1 is unchanged: no request, result, or error shape changes.
The breaking part is the variable name, which consumers set in their own `mise.toml`.

## Capabilities

### New Capabilities

- `cli-environment`: options read from the environment under the `FORKCTL_` prefix.

### Modified Capabilities

None.

## Impact

- [`src/cli.rs`][cli]: `env_prefix = "FORKCTL"` on the root parser.
- [`src/main.rs`][main] and [`src/completion.rs`][completion]: the manifest variable.
- [`tests/support/mod.rs`][support] and [`tests/cli.rs`][tests]: the cleared environment and the new tests.
- `Cargo.toml`: ctl-core `=0.6.21`.
- [`README.md`][readme], `AGENTS.md`, the operator instructions and skill templates, [`examples/mise.toml`][example], and the design notes under `goals/`.
- Each consumer repository that sets `FORK_MANIFEST` in its `mise.toml` renames it when it takes this release.

[ctl-core]: https://github.com/victor-software-house/ctl-core
[cli]: ../../../src/cli.rs
[main]: ../../../src/main.rs
[completion]: ../../../src/completion.rs
[support]: ../../../tests/support/mod.rs
[tests]: ../../../tests/cli.rs
[readme]: ../../../README.md
[example]: ../../../examples/mise.toml
