# Forkctl Workflow Redesign — Implementation Plan

No implementation begins until `requirements.md` and `design.md` are accepted through `ask_user`.

## Execution Checkpoint — 2026-08-03

The contract is approved and released as forkctl 0.0.6 from `bde4e163f0e077c3bbe1555d34b3fb325c460b24`. PR #1 remains closed unmerged; PR #2 was reviewed, rebased onto `main`, and merged. The clean repository compiles, passes denied-warning Clippy, and passes 46 tests through `mise run verify`.

### Implemented and verified

- Incompatible schema-1 manifest, Clap command tree, typed request/result/schema graph, execute/plan adapter, centralized pretty/JSON view, and generated width-aware help.
- Top-level `check` with repository default and staged `-s`, explicit active-patch state, metadata-only draft creation, select/show/list, staged/all/path refresh, deterministic exports, bookkeeping refresh, and finish.
- One production subprocess factory that clears Git repository-local hook variables; synthetic hook-context lifecycle coverage passes.
- Typed rebase journal, exact annotated recovery object, atomic exact-lease publication, operation-level history, and fresh-clone hydration; the real rebase/publish/fresh-clone lifecycle passes.
- Clap-derived Usage KDL with dynamic patch/ref/remote candidates. `completion SHELL` now emits dynamic Clap registrations for bash, elvish, fish, PowerShell, and zsh, plus a Usage-backed dynamic Nushell registration; direct completion tests prove live patches, refs, remotes, all six shell registrations, and fail-silent lookup outside repositories.
- Typed domain errors now originate at repository, manifest, subprocess, active-patch, patch lookup, scope, operation, lease, and publication boundaries. API conversion downcasts those types rather than parsing messages; tests prove stable request/repository/patch/path details and JSON stream cleanliness.
- Patch edits and refreshes are journaled with typed intent. Kind reorder, edit/refresh conflict continuation, hook-modified capture, no-op rejection, and stale export cleanup are lifecycle-proven.
- Abort reconstructs and verifies the exact old tip/base/ordered patch commits before deleting its journal/tag/snapshot; recovery-tag deletion and substitution fail closed.
- Mise's source-backed proxy design: executable file task, `raw_args = true`, exact local tools, `exec forkctl "$@"`, and mise's documented Usage self-mount for completion. Direct mounted help and nested command completion were proven against real mise.
- Root `mise.toml` is the sole source for minimum mise, Rust, StGit, Lefthook, Usage, and GitHub CLI versions. `[workspace.package].version` remains the sole forkctl release source. `version:sync` regenerates the lockfile and all operational copies; `version:check` rejects drift.
- Real mise tests prove raw-proxy help/cwd/stream/exit parity and bash/fish/Nushell/PowerShell/zsh mounted completion with live patch/ref candidates; real Lefthook pre-push and staged pre-commit composition passes.
- Publication tests prove stale leases, conflicting tags, protected-branch rejection, and remotes without atomic capability leave both refs unchanged, retain the operation, return typed errors, and never retry without `--atomic`.
- Forkctl 0.0.6 is published on crates.io and GitHub. The macOS arm64 asset digest `c2c435184d85c214ac3ea784cd812ef230221d9de958271e4644c5ebb1346c2d` was downloaded, compared, extracted, and executed. A clean `cargo install forkctl --version 0.0.6 --locked` reports 0.0.6.
- The immutable `v0.0.6` remote task catalog loads from GitHub, provisions forkctl 0.0.6, renders direct colored help, and exposes mounted command completion through real mise/Usage.

### Remaining implementation blockers

1. **Sequential fleet migration** — rebuild Macterm, Ghostty, and zmx onto 0.0.6 one repository at a time, run every repository-specific gate, publish atomically, and prove each fresh clone before starting the next.

### Execution order

Complete the blockers above in order. Do not version-bump, publish, or migrate a consumer while any earlier blocker remains. After each blocker: run `mise run verify`, inspect the diff, update this checkpoint and the live task ledger, then commit only the coherent verified layer.

## Phase 1 — Freeze the New Contract

### Changes

- Finalize command names, global flags, manifest schema, active state, operation journal, request/response/error types, and hook composition.
- Update `AGENTS.md` and `CODING_STANDARDS.md` to remove the six-command limit and bind the new command families.
- Close PR #1 without merge after its accepted findings are represented in the new plan; retain no commit dependency on its branch.

### Verification

- Every public mutation has an explicit request/result/error type in the design.
- Every CLI command maps to exactly one dotted API command.
- No requirement references an old flag, alias, manifest field, pending state, or task shape.

## Phase 2 — Rebuild the Protocol and CLI Skeleton

### Changes

- Split Clap adapter definitions out of `main.rs` into `cli.rs`.
- Replace `ApiRequest`, `CommandResult`, and generic error handling with the new discriminated command graph and stable domain errors.
- Add execute/plan mode and global `--format`, `--color`, and `--quiet` semantics.
- Compose leaf parameters from subject, metadata/scope, capture, execution, and presentation `Args` groups; assign collision-audited short aliases.
- Add a centralized justpath-style width-aware colored help renderer that traverses Clap metadata and uses the existing semantic view stack.
- Add `usage-lib` Clap conversion, hidden `--usage-spec`, dynamic Clap completion, Nushell completion bridging, and `completion SHELL`.
- Materialize every request, plan, result, notice, error code/detail, manifest, active-state, and operation schema specified by `api.md`.
- Replace the manifest structs with the new schema and deterministic export configuration.
- Delete every compatibility path and old test fixture; rewrite fixtures directly in the new format.

### Verification

- CLI help snapshots cover the full command tree, parameter headings, defaults, choices, repeatable flags, and all short aliases at narrow/standard/wide widths with and without color.
- Direct completion tests cover bash, elvish, fish, Nushell, PowerShell, and zsh; mounted mise completion tests cover its Usage-supported bash, fish, Nushell, PowerShell, and zsh paths, including dynamic patches/refs, file hints, repeated values, and short forms.
- JSON Schema snapshots cover the bundle plus manifest/invocation/response/active-state/operation selections and reject unknown fields.
- JSON Schema exposes every request/result/error variant.
- CLI/API parity tests execute one handler per command.
- Architecture test keeps adapters/views out of domain modules.
- Old manifests and old commands fail as invalid input, not as migration candidates.

## Phase 3 — Make Production Subprocesses Hook-Safe

### Changes

- Centralize all production child creation in `process.rs`.
- Resolve and clear Git repository-local variables for child processes while preserving transport/auth variables.
- Remove raw `Command::new` from every production module.
- Recreate PR #1's test-process isolation only where fixtures themselves launch foreign repositories; do not sanitize forkctl process entry.

### Verification

- Launch forkctl with synthetic `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, and object-directory variables intact.
- Complete init, nested export reconstruction, full check, rebase, and publish against disposable repositories.
- Prove the checkout running the hook remains byte/ref clean.
- Test SSH/transport variables remain visible to intended remote Git commands.

## Phase 4 — Implement Active Patch State and Read-Only Checks

### Changes

- Add typed Git-private active state.
- Implement top-level `check` with full-repository default and staged `-s` mode.
- Implement `patch list`, `show`, `create`, and `select`.
- Make draft creation metadata-only until the first refresh.
- Add staged/unstaged/untracked path inventory and ownership diagnostics.
- A nonempty staged index fails without an active/explicit patch; an empty index succeeds.

### Verification

- Create/select never changes tracked files or refs.
- Staged ownership checks accept exact/glob matches and reject overlap/escape cases deterministically.
- Staged mode fails without a target only when the index is nonempty; operators customize hook policy by choosing when to call it.
- Pretty and JSON snapshots show the same active/path data.

## Phase 5 — Implement Patch Mutation Workflow

### Changes

- Implement capture planning for staged, all-owned, and explicit pathspec modes.
- Implement draft materialization and existing lower-patch refresh through StGit.
- Consume hook-modified index state.
- Implement metadata/scope edits and trailer amendment.
- Generate every source export deterministically; remove optional per-patch export fields.
- Regenerate manifest/ledger/exports and refresh bookkeeping atomically.
- Implement `patch finish` as full-check-plus-clear-active.

### Verification

- Real StGit tests cover top/lower patch refresh, partial staging, untracked files, path-limited refresh, hook formatting/staging, no-op refresh, conflict creation, and finish.
- Hooks that modify the index are reflected in the final patch and generated export.
- Unowned paths never enter any patch or bookkeeping.
- Source reconstruction exactly matches the last source patch tree.

## Phase 6 — Rebuild Recovery, Rebase, and History

### Changes

- Replace pending state with the typed current-operation journal.
- Implement operation status/continue/abort.
- Rebase records ordered old patch evidence and exact annotated recovery object before replay.
- Store operation-level rebase history with exact tag object and dropped pre-rebase commits.
- Fresh init fetches only exact history refs.
- Reimplement report newline/object binding and correct API error classification from PR #1.
- Patch metadata edits and refreshes are now journaled with typed intent. Kind changes validate/reorder the real StGit series, regenerate deterministic exports, and delete stale generated exports. Real lifecycle tests cover clean kind reorder, multi-stage kind-reorder conflict continuation, lower-patch refresh conflict continuation through StGit's documented `refresh-temp` squash path, hook-modified index capture, and no-op rejection without journal debris.
- Operation abort now requires confirmation, reconstructs the exact old tip/base/ordered patch commits through Git/StGit, verifies the restored repository before deleting its journal/tag/snapshot, and keeps evidence on failure. Tests cover dry-run, unconfirmed execution, exact rebase restoration, and deleted/lightweight/wrong-tip recovery-tag rejection.

### Verification

- Conflicting rebase and lower-patch refresh expose typed next actions.
- Continue revalidates every completed phase.
- Abort dry-run is non-mutating; abort execution restores exact old branch/stack/manifest and verifies before clearing state.
- Deleted, retargeted, lightweight, substituted, or wrong-tip recovery tags fail locally and in fresh clones.
- Multiple dropped patches from one rebase share one recovery record without duplicated evidence.

## Phase 7 — Publication and Integration Points

### Changes

- Rebuild publish against the new operation journal and error taxonomy.
- Add typed `publication_rejected` without parsing provider policy into generic logic beyond preserving remote diagnostics.
- Replace shallow wrappers with one `fork` shebang file task: `dir = "{{cwd}}"`, `raw_args = true`, mise-documented self-mounted `--usage-spec` for completion, `exec forkctl "$@"`, and exact task-local tools.
- Add optional Lefthook install/validate helpers and document `mise run fork check -s` / `mise run fork check -q` composition without mutating hook manager configuration.

### Verification

- Remote advancement, conflicting tag, unsupported atomic push, and protected-branch rejection fail without partial refs or fallback.
- Successful publication atomically updates branch and recovery tag under the exact lease.
- Real StGit pre-commit hooks may update staged content during `patch refresh`.
- Real Lefthook pre-commit and pre-push configurations pass in a disposable consumer and under inherited Git hook variables.
- A real immutable remote catalog proves direct forkctl help/argument rejection through the raw proxy, short/long parity, dynamic mounted completion, cwd preservation, and direct-vs-mise stdout/stderr/exit parity.

## Phase 8 — Independent Review and Release 0.0.6

### Changes

- Run focused architecture, security, release, and silent-failure reviews over the complete diff.
- Resolve every blocking finding before version preparation.
- Set `[workspace.package].version` to `0.0.6`; synchronize generated catalog/example copies from that source only.
- Publish GitHub and crates.io artifacts through the existing local release procedure.

### Verification

- `mise run verify`
- `mise run build`
- `cargo publish --dry-run --locked`
- Full released-binary lifecycle against disposable real remotes, including hooks, abort, history hydration, and protected-branch rejection.
- GitHub asset digest, release commit, crates.io install, GitHub install, API schema, and immutable remote task catalog verified directly.

## Phase 9 — Sequential Fleet Cutover

### Macterm

- Replace 0.0.5 manifest/task shape directly.
- Configure local Lefthook/mise checks.
- Preserve four patch intents and regenerate source evidence.
- Re-run format, lint, unit, release build, E2E, benchmark, atomic publication, and fresh-clone init/check.

### Ghostty

- Delete the 0.0.2 manifest/catalog with no converter.
- Recreate command-wrapper, output-activity, sync-environ, and bookkeeping patches in the new contract.
- Run Ghostty verifier, local artifact build, and Macterm compatibility gates before publication.

### zmx

- Classify inherited macOS source patches and private tooling as explicit patch records.
- Add the new manifest, evidence, task catalog, and hook composition.
- Run Zig formatting/check/test/release-safe build plus Macterm attach/detach protocol gates.

### Fleet policy

- Configure a durable narrow VSH branch-ruleset exception/bypass for exactly the approved forkctl-managed repositories.
- Never embed organization credentials or ruleset mutation into forkctl.

### Verification

Each repository must be clean and synchronized, have no old contract files, and pass fresh-clone `forkctl init` plus `forkctl check` before the next repository starts.

## Done Condition

- Forkctl 0.0.6 is released and independently proven.
- PR #1 is closed unmerged and superseded by the new layered PR.
- Macterm, Ghostty, and zmx exclusively use the new manifest/API/task/hook contract.
- Every normal patch change uses explicit active intent plus one refresh command; no manual bookkeeping or raw StGit choreography is required.
- Custom hook managers can call stable forkctl checks, while VSH's mise/Lefthook path is documented, tested, and low-friction.
- No compatibility, migration, alias, or fallback code remains.
