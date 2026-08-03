# Forkctl Audit Integrity

## Objective

Publish forkctl 0.0.5 as a narrow audit-integrity correction. Bind every recovery and review artifact to the pending operation it represents, prevent export configuration from overwriting unrelated repository content, represent historical targets according to Git ref semantics, and close the highest-value real Git/StGit lifecycle test gaps without adding commands, compatibility code, signing, or a fabricated transport layer.

## Implementation

1. **Typed historical target and append-only patch history**
   - Replace `base.label` with a typed target containing `kind` (`commit`, `branch`, or `tag`), `selector`, resolved `commit`, and optional annotated tag-object ID.
   - Require full `refs/heads/*`, full `refs/tags/*`, or a full commit SHA at the CLI boundary.
   - Persist `target.commit == base.stack`; exact commit selectors equal that commit; annotated tag objects must exist, contain the selected tag name, and peel to the resolved commit. Branch selectors remain historical and are never required to follow a moving live branch during offline verification.
   - Add manifest `history` entries preserving a dropped patch's complete metadata, former commit, resolved target, and `upstream-merged` reason.
   - Render current patches and append-only history through Askama `PATCHES.md`.

2. **Pending evidence binding**
   - Record old patch count when recovery state is created.
   - Verify the annotated recovery tag exists and peels exactly to `old_tip`; verify `old_tip~old_patch_count == old_base`.
   - For completed new/rebase operations, bind `new_tip` to `HEAD`, `new_base` to manifest/StGit base, and rebase target to manifest target.
   - Store rebase report path plus `git hash-object` object ID and compare current report bytes during ordinary verification and immediately before push.
   - Make `new --finish` record `new_tip`; later changes fail verification/publication until finish is rerun.

3. **Safe generated exports**
   - Reject export paths containing glob metacharacters.
   - Require every export to match the final bookkeeping patch's path policy.
   - Reject equality with manifest, ledger, or required paths and reject matches against every non-bookkeeping patch path policy.
   - Keep uniqueness and repository-relative checks. Add table-driven unit tests before exercising writes.

4. **Upstream-merged patch history**
   - After successful `stg rebase --merged`, detect empty non-bookkeeping patches before final verification.
   - Capture each patch's metadata and pre-deletion commit, delete it through `stg delete`, remove its manifest row and obsolete export, and append an `upstream-merged` history event associated with the resolved target.
   - Regenerate ledger/exports/bookkeeping, emit one operation-time notice, and continue normally. Future status/verify output does not repeat the notice; `PATCHES.md` is the durable record.

5. **Real regression evidence**
   - Add lifecycle tests for local recovery-tag retargeting, report modification, all pending identity mismatches, unsafe export ownership/collisions, invalid target before state mutation, merged patch deletion/history, conflicting remote recovery tag, real tooling-patch insertion, and exact trailers from `new`.
   - Retain existing real bare upstream/downstream fixtures and add focused helper abstractions only where they reduce duplication. No mocks replace Git/StGit behavior.
   - Keep unsupported atomic fallback as source-level proof: a single `git push --atomic` invocation propagates failure and has no retry path.

6. **Protocol-first CLI and unified view**
   - Define versioned typed request, success, error, warning, and command-result envelopes deriving Serde and Schemars from the same Rust types.
   - Map Clap commands and `api call` JSON requests into the same request enum and execute each handler once without output concerns.
   - Add global `--output pretty|json`, `api schema`, and `api call`; JSON stdout contains exactly one schema-valid envelope and diagnostics never contaminate it.
   - Make every App operation return typed data and notices. Capture Git/StGit subprocess output rather than inheriting stdout/stderr.
   - Centralize human rendering in one view module using one semantic Anstyle theme and one Comfy Table configuration. Do not expose crate-default styles or construct tables in handlers.
   - Render typed protocol errors through the same semantic view. Defer Miette until diagnostics require source spans; JSON errors retain stable codes and structured details.
   - Add schema contract tests, JSON round-trips, pretty/JSON parity tests, NO_COLOR/pipe snapshots, and a source guard that rejects printing/table/view dependencies outside the boundary.

7. **Release and proof**
   - Update manifest examples, README, instructions, project guidance, Askama templates, and durable vault reference.
   - Bump the Cargo single source to 0.0.5 and synchronize six remote task pins plus example ref.
   - Run clean rustfmt, strict workspace Clippy, all unit/CLI/lifecycle/xtask tests, release build, package dry-run, and local release.
   - Install both GitHub and crates.io variants and run the published GitHub binary through the complete real lifecycle suite.

## Non-goals

- No local-tamper resistance, signing keys, remote evidence service, schema split, compatibility reader, new public command, live branch equality check, or automatic conflict interpretation.
- No release workflow or native build matrix.
