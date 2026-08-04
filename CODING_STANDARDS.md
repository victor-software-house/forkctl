# Coding Standards

## 1. Scope

### 1.1 Delegate mechanics

Use installed Git and StGit CLIs. Never reproduce their storage, revision, patch, rebase, hook, or conflict semantics.

### 1.2 Explicit intent

Forkctl never infers patch ownership. An operator creates/selects one active patch; capture modes and scope are explicit typed inputs.

### 1.3 Separate data, adapters, and views

Domain/App operations return typed protocol values and errors. Clap and JSON are input adapters; pretty and JSON are output adapters. Domain modules never print, inspect TTY/color/output mode, construct tables, or import renderer crates.

`src/protocol.rs` owns all wire/schema types. `src/cli.rs` owns the one command graph. `src/view.rs` and `src/help.rs` own the one visual system. Generated Markdown remains in Askama templates.

## 2. Dependencies

### 2.1 Runtime isolation

Remote mise tasks declare exact forkctl, Rust, StGit, and optional Lefthook tools. Root `mise.toml` is the sole source for toolchain versions; `[workspace.package].version` is the sole forkctl release source. `mise run version:sync` generates every operational copy and lock entry, and `version:check` rejects drift. No Homebrew/global installation or activated language environment is required.

### 2.2 Rust dependencies

Use current stable pinned releases. Add a crate only when it removes more duplicated contract or correctness risk than it adds. Clap, Schemars, usage-lib, completion, and pretty rendering all consume one authoritative command/type graph.

## 3. Safety

### 3.1 Fail closed

Malformed manifests, dirty clean-only operations, scope drift, patch drift, history drift, operation drift, stale leases, and remote policy failures are typed errors. Never print success after a failed prerequisite.

### 3.2 Atomic state

Write tracked and Git-private JSON atomically. Refresh only explicitly selected patch and bookkeeping files. Never stash.

### 3.3 Hook environment

Every production child comes from `process.rs`, clears only repository-local variables reported by `git rev-parse --local-env-vars`, applies an explicit cwd, and preserves transport/auth variables.

### 3.4 No compatibility

The current manifest/API/local-state contract is `schema: 1`. There are no previous formats from the implementation's perspective and no migration or fallback code.

## 4. CLI/API discipline

- Modes of one action are flags/typed parameters, not optional subcommands.
- Most long options have deliberate collision-tested shorts; misleading shorts are omitted.
- Every mutation has execute/plan parity and `-n`/`--dry-run`.
- CLI and JSON execute the same handler and return the same typed result.
- JSON stdout is exactly one envelope; pretty errors use stderr.
- Help, Usage spec, completions, and schemas are generated from Clap/Schemars types.
- Stable error codes/details are chosen at domain boundaries, never by parsing arbitrary error strings when a typed classification is available.

## 5. Verification

Formatting, workspace Clippy `all`/`pedantic` with warnings denied, architecture boundaries, unit tests, CLI/API/schema/help/completion snapshots, and real disposable Git/StGit lifecycle tests must pass.

Tests invoking forkctl from hook context preserve inherited repository-local `GIT_*` variables at process entry. Fixture helper commands may isolate themselves, but must never hide production contamination.

Every release is tested through direct and mounted mise grammar, real Lefthook, published native binary, fresh-clone recovery hydration, exact lease rejection, protected-branch rejection, and successful atomic publication.
