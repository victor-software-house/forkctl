# Mise Fork Tasks

Reusable downstream-fork maintenance for StGit patch stacks.

## Architecture

- `forkctl` is a small Rust policy CLI. It delegates all repository and patch mechanics to `git` and `stg`; never reimplement either data model.
- `tasks/fork.toml` is the mise-native remote task catalog. Every task declares all required tools and versions so consuming repositories need only mise.
- A consuming fork owns one JSON manifest and its exported patch files. Generic executable logic stays here.
- `src/patchexport.tmpl` pins StGit export formatting across StGit upgrades.
- `templates/*.md` holds Askama-compiled editorial text for generated ledgers and rebase reports; Rust renderers own typed context and format-specific escaping.
- `src/protocol.rs` is the versioned request/result/error/schema contract. `src/view.rs` is the only terminal renderer and owns one semantic visual theme.

## Invariants

- Keep exactly these public lifecycle operations: `init`, `status`, `new`, `verify`, `rebase`, and `publish`; `instructions` is read-only workflow guidance.
- `schema: 1` is the single manifest contract. Do not add a compatibility reader or migration fallback for the pre-0.0.3 shape.
- No runtime language, package manager, config parser, or shell-library dependency beyond the mise-provisioned `forkctl`, `git`, and `stg` executables.
- Remote task and release versions are immutable. Consumers pin the remote catalog by commit or release tag.
- `verify` must fail closed on dirty worktrees, wrong branch/tracking, missing tools, remote drift, base drift, patch-order drift, unapplied or empty patches, undeclared per-patch paths, trailer/ledger/export drift, missing source contracts, or non-reconstructable exports.
- Mutating commands never stash. Rebase creates recovery and exact-lease state before replay, never publishes, and preserves normal StGit conflict state.
- `publish` requires verification and exact pending evidence, uses atomic explicit-ref publication with an exact force-with-lease, and has no non-atomic or plain-force fallback.
- Keep `main.rs` as adapter wiring; implementation modules stay bounded by responsibility.
- App/domain modules return typed protocol data only. They never print, construct tables, inspect TTY/color/output mode, or depend on view crates. The architecture test enforces this boundary.
- Clap and JSON requests execute the same handlers. Pretty and JSON outputs consume the same typed results; Schemars derives the published API schema from those types.
- All human output goes through the centralized Anstream/Anstyle/Comfy Table renderer. Do not mix library-default themes or command-local visual styles.
- Keep examples, generated instructions, and the task catalog synchronized with the manifest and protocol contracts.
- `[workspace.package].version` is the only version source. Never hand-edit task/example version copies; run `mise run version:sync` and let Lefthook stage the result.

## Checks

```sh
mise run verify
mise run build
```

Test the released remote catalog against a real disposable fork clone before publishing a version.
