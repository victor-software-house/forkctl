# Forkctl Workflow Redesign — Implementation Plan

No implementation begins until `requirements.md` and `design.md` are accepted through `ask_user`.

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
- Replace the manifest structs with the new schema and deterministic export configuration.
- Delete every compatibility path and old test fixture; rewrite fixtures directly in the new format.

### Verification

- CLI help snapshots cover the full command tree.
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
- Complete init, nested export reconstruction, verify, rebase, and publish against disposable repositories.
- Prove the checkout running the hook remains byte/ref clean.
- Test SSH/transport variables remain visible to intended remote Git commands.

## Phase 4 — Implement Active Patch State and Read-Only Checks

### Changes

- Add typed Git-private active state.
- Implement `patch list`, `show`, `create`, `select`, and `check`.
- Make draft creation metadata-only until the first refresh.
- Add staged/unstaged/untracked path inventory and ownership diagnostics.

### Verification

- Create/select never changes tracked files or refs.
- Staged ownership checks accept exact/glob matches and reject overlap/escape cases deterministically.
- Strict mode fails without active state; permissive mode remains usable by custom hook policies.
- Pretty and JSON snapshots show the same active/path data.

## Phase 5 — Implement Patch Mutation Workflow

### Changes

- Implement capture planning for staged, all-owned, and explicit pathspec modes.
- Implement draft materialization and existing lower-patch refresh through StGit.
- Consume hook-modified index state.
- Implement metadata/path edits and trailer amendment.
- Generate every source export deterministically; remove optional per-patch export fields.
- Regenerate manifest/ledger/exports and refresh bookkeeping atomically.
- Implement `patch finish` as verify-plus-clear-active.

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
- Add exact mise tasks for every routine command.
- Add `fork:check:staged` and `fork:verify` hook primitives.
- Document and test concise Lefthook plus mise composition; do not mutate hook manager configuration.

### Verification

- Remote advancement, conflicting tag, unsupported atomic push, and protected-branch rejection fail without partial refs or fallback.
- Successful publication atomically updates branch and recovery tag under the exact lease.
- Real StGit pre-commit hooks may update staged content during `patch refresh`.
- Real Lefthook pre-commit and pre-push configurations pass in a disposable consumer and under inherited Git hook variables.

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
- Re-run format, lint, unit, release build, E2E, benchmark, atomic publication, and fresh-clone init/verify.

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

Each repository must be clean and synchronized, have no old contract files, and pass fresh-clone `forkctl init` plus `forkctl verify` before the next repository starts.

## Done Condition

- Forkctl 0.0.6 is released and independently proven.
- PR #1 is closed unmerged and superseded by the new layered PR.
- Macterm, Ghostty, and zmx exclusively use the new manifest/API/task/hook contract.
- Every normal patch change uses explicit active intent plus one refresh command; no manual bookkeeping or raw StGit choreography is required.
- Custom hook managers can call stable forkctl checks, while VSH's mise/Lefthook path is documented, tested, and low-friction.
- No compatibility, migration, alias, or fallback code remains.
