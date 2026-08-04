# Coding Standards

## 1. Scope

### 1.1 Delegate mechanics

Use installed Git and StGit CLIs. Never reproduce their storage, revision, patch, rebase, hook, or conflict semantics.

### 1.2 Explicit intent

Forkctl never infers patch ownership. An operator creates/selects one active patch; capture modes and scope are explicit typed inputs.

### 1.3 Separate data, adapters, and views

Domain/App operations return typed protocol values and errors. Clap and JSON are input adapters; pretty and JSON are output adapters. Domain modules never print, inspect TTY/color/output mode, construct tables, or import renderer crates.

`src/protocol.rs` owns all wire/schema types. `src/cli.rs` owns the one command graph. `src/view.rs`, `src/help.rs`, and `src/layout.rs` own the one visual system. Pretty prose and tables constrain themselves to the detected terminal width, with `COLUMNS` as the non-TTY fallback used by captured output and tests. Generated Markdown remains in Askama templates.

## 2. Dependencies

### 2.1 Runtime isolation

Remote mise tasks declare exact forkctl, Rust, StGit, and optional Lefthook tools. Root `mise.toml` is the sole source for toolchain versions; `[workspace.package].version` is the sole forkctl release source. `mise run version:sync` generates every operational copy and lock entry, and `version:check` rejects drift. No Homebrew/global installation or activated language environment is required.

### 2.2 Rust dependencies

Use current stable pinned releases. Add a crate only when it removes more duplicated contract or correctness risk than it adds. Clap, Schemars, usage-lib, completion, and pretty rendering all consume one authoritative command/type graph.

## 3. Safety

### 3.1 Fail closed

Malformed manifests, dirty clean-only operations, scope drift, patch drift, history drift, operation drift, stale leases, and remote policy failures are typed errors. Never print success after a failed prerequisite.

Failing closed must never disable recovery. When an operation journal exists, operation-scoped commands resolve declared state from the Git-private snapshot rather than refusing to run because tracked files are mid-conflict.

### 3.2 Atomic state

Write tracked and Git-private JSON atomically. Refresh only explicitly selected patch and bookkeeping files. Never stash.

### 3.3 Hook environment

Every production child comes from `process.rs`, clears only repository-local variables reported by `git rev-parse --local-env-vars`, applies an explicit cwd, and preserves transport/auth variables.

### 3.4 No compatibility

The current manifest/API/local-state contract is `schema: 1`. There are no previous formats from the implementation's perspective and no migration or fallback code.

### 3.5 Publication evidence

Leases and recovery points come from remote evidence, never from convenience. Capture the published tip with `ls-remote`, publish an annotated recovery tag for any tip a rewrite overwrites, and require exact evidence — the reviewed operation lease or the fetched downstream tracking ref — that the rewrite was computed against that exact tip.

### 3.6 Declared checks

A patch may declare commands that must succeed for it to still hold. Globs default to the declaring patch's scope and may reach anywhere: the drift that breaks a long-lived fork is upstream introducing cases the patch never covered, in files it does not own and that may not have existed. Scope governs what a patch may modify; a check only reads.

Exit status is the verdict, so any tool works and forkctl ships no checking tooling, embeds no parser or query language, and never rewrites source. Commands come from `process.rs` with repository-local Git variables cleared, an explicit cwd, and shell-quoted `{files}`; an expansion past the command-length budget is a typed error rather than a truncated command.

A check whose globs match no tracked file is a failure. A command over an empty file list usually succeeds, which would let a rename disarm the check watching it.

Checks observe the applied stack by default or the declaring patch's own commit with `at: patch`. Both stages execute with a disposable clone as cwd and no origin remote, so ordinary relative writes and accidental pushes cannot mutate the operator worktree. Declared commands remain trusted user-level code, not sandboxed hostile input.

A rebase that leaves a surviving patch touching fewer paths records the lost paths as recovery-bound replay history. This evidence proves only that the patch's path set changed across replay; do not attribute the cause to upstream without stronger evidence.

## 4. CLI/API discipline

- Modes of one action are flags/typed parameters, not optional subcommands.
- Most long options have deliberate collision-tested shorts; misleading shorts are omitted.
- Every mutation has execute/plan parity and `-n`/`--dry-run`.
- CLI and JSON execute the same handler and return the same typed result.
- JSON stdout is exactly one envelope; pretty errors use stderr.
- Help, Usage spec, completions, and schemas are generated from Clap/Schemars types.
- Stable error codes/details are chosen at domain boundaries, never by parsing arbitrary error strings when a typed classification is available.
- The portable `skills/forkctl/SKILL.md` teaches workflow and safety invariants while deferring exact syntax to installed help/instructions. Validate it as an Agent Skill and through Skills CLI discovery whenever its content changes.

## 5. Verification

Formatting, workspace Clippy `all`/`pedantic` with warnings denied, architecture boundaries, unit tests, CLI/API/schema/help/completion snapshots, and real disposable Git/StGit lifecycle tests must pass.

Tests invoking forkctl from hook context preserve inherited repository-local `GIT_*` variables at process entry. Fixture helper commands must obtain their child processes from the single shared isolated factory in `tests/support`, never from a local `Command::new`, so a real `pre-push` run operates on the disposable repository; forkctl invocations stay unisolated so production contamination cannot be hidden.

Every lifecycle fixture owns a private `HOME`, XDG config/cache/data tree, empty Git global/system configs, empty Git template directory, deterministic identity and commit date, `C` locale, and UTC timezone. These are command-local environment values — tests never mutate the test runner's global environment. Fixture hooks create their own directories and files explicitly; no test relies on host Git templates, aliases, credential helpers, user config, or hooks. Mounted-task tests may reuse the caller's mise installation store solely to resolve the repository's exact pinned tools.

`mise run test` preserves the standard `cargo test` gate. `mise run test:isolated` additionally runs the complete suite with cargo-nextest, one test per process, proving the fixture has no thread/process-global assumptions while retaining parallel execution. Containers are reserved for scenarios requiring a distinct OS, network, daemon, or toolchain image; ordinary Git/StGit lifecycle coverage uses real pinned binaries inside `tempfile` sandboxes.

Every release is tested through direct and mounted mise grammar, real Lefthook, published native binary, fresh-clone recovery hydration, exact lease rejection, protected-branch rejection, and successful atomic publication. Release preparation verifies crates.io publication/auth before GitHub mutation; retries may resume only an exact-target draft, clobber its native asset deterministically, publish the crate only when absent, and expose the release only after both distributions exist.
