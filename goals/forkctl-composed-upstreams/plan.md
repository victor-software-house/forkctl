# Forkctl Composed Upstreams — Implementation Plan

This plan is not active implementation authorization. It orders future work after the architecture package is reviewed and remaining CLI/schema questions are settled.

## Phase 1 — Freeze the Source and Base Contract

### Changes

- Design the tagged direct-repository and composition base-provider types.
- Define named Git sources, exact target evidence, exact file/directory mappings, and generated lock fields.
- Decide current-manifest migration as a clean break with all known consumers updated in the same slice; add no compatibility reader.
- Settle deterministic synthetic commit identity/timestamp and no-op lock semantics.
- Settle whether `sync` replaces or complements direct-provider `rebase`.

### Verification

- JSON Schema rejects unknown base-provider/source/mapping fields.
- Destination equality and parent/child overlap fail deterministically.
- Current direct-provider fixtures remain representable without implicit conversion.
- Every command/API/help/completion change has one documented contract.

## Phase 2 — Build the Projection Planner

### Changes

- Add clone-local source caches keyed by stable source identity.
- Resolve all selected targets before mutation.
- Enumerate exact file/directory entries and preserve Git modes/object kinds.
- Remap source paths into a complete sorted destination inventory.
- Reject stale roots, collisions, invalid paths, and unsupported objects before writing downstream state.
- Produce a typed dry-run plan and JSON result.

### Verification

- Hermetic bare sources cover files, directories, executable modes, symlinks, Gitlinks, deletes, and renames.
- One-source, subset, and all-source plans freeze the expected target set.
- Failure leaves downstream objects, refs, index, worktree, manifest, and operation state unchanged.
- Source caches with different fetch optimizations produce identical plans.

## Phase 3 — Materialize Synthetic Bases

### Changes

- Batch-read selected source blobs and write exact bytes into the downstream object database.
- Build the composite tree through an isolated temporary index.
- Create deterministic root/successor synthetic commits with source/mapping/tree evidence.
- Record current source locks, synthetic base, and tree in the manifest/bookkeeping patch.
- Extend full checks for synthetic tree, evidence, and ancestry.

### Verification

- Repeated materialization of identical inputs yields the same objects.
- The reachable downstream graph contains selected blobs and synthetic commits but no unrelated source blobs or source commit history.
- A fresh clone can inspect every historical synthetic base without source caches.
- Materialized tree bytes and modes equal the selected source entries after remapping.

## Phase 4 — Synchronize and Replay

### Changes

- Implement `sync` planning and execution around the current operation journal.
- Replay the declared stack onto the new synthetic base with StGit.
- Preserve conflict continuation/abort behavior.
- Bind and remove empty upstream-merged patches.
- Regenerate manifest, ledger, exports, and range-diff/source-lock reports.
- Support atomic repeatable source selection.

### Verification

- A patch spanning two source destinations survives an update to one source.
- Multiple selected sources update as one operation.
- Conflict, continue, and abort restore exact old source locks/base/stack.
- An incorporated patch is dropped only with exact recovery-bound evidence.
- Existing direct-provider lifecycle remains green.

## Phase 5 — Rewrite Publication

### Changes

- Publish composition rewrite candidates through the current exact force-with-lease and recovery model.
- Extend status/result/report views with source-lock and synthetic-base identities.
- Keep unsupported atomic push and provider rejection fail-closed.

### Verification

- Stale lease, conflicting recovery tag, unsupported atomic push, and protected branch leave all publication refs unchanged.
- Successful publication updates branch plus required recovery refs and reads them back.
- Fresh-clone hydration reconstructs the exact composed stack.

### First Coherent Release Gate

A release after this phase is useful: multi-source composition, strict downstream projection, normal patch editing/replay, and direct audited rewrite publication all work end to end. Do not block it on append history or GitHub automation if those slices are not independently proven.

## Phase 6 — Append Replay

### Changes

- Prototype StGit replay onto an epoch commit whose ancestry already contains the old patch commits; inspect stack metadata, merged detection, conflicts, and clone hydration before freezing any public API.
- If the prototype preserves the named patch stack and tree semantics, add explicit `history: append` selection; otherwise stop and redesign or defer append rather than bypassing StGit.
- Create and validate epoch commits with old-tip/new-base parents and new-base trees.
- Replay fresh patches above the epoch.
- Restrict append publication to fast-forward-only.

### Verification

- Rewrite and append from identical inputs produce identical final trees.
- Append preserves the old published tip as an ancestor.
- Epoch parent/tree substitution fails.
- Append publication rejects every non-fast-forward remote change.

## Phase 7 — Durable Proposal Protocol

### Changes

- Define the canonical versioned proposal payload.
- Build one review commit from the exact candidate tree with the captured old downstream tip as its parent.
- Implement immutable proposal/recovery tags and a mutable review branch.
- Add `proposal push`, `verify`, and `promote` with CLI/API/schema parity.
- Make verification independent from clone-local operation state.
- Bind reports, manifests, source locks, recovery objects, exact candidate, review commit/tree, downstream ref, and lease.

### Verification

- Prepare and verify one proposal in separate clean clones.
- Prove the review commit shows the old-tip → candidate-tree delta while the proposal tag retains the different exact candidate topology.
- Candidate mutation, review-tree mismatch, tag retargeting/substitution, report/manifest mismatch, stale lease, and wrong history strategy fail closed.
- Proposal evidence and review-branch push is atomic.
- Promotion publishes the exact candidate—not the review commit—and verifies remote state before review-branch cleanup.

## Phase 8 — GitHub Reusable Workflows

### Changes

- Add versioned reusable proposal and promotion workflows.
- Add minimal scheduled/manual caller examples.
- Install exact forkctl releases and pin every external Action by full release SHA.
- Set least-privilege permissions and safe concurrency.
- Render/update proposal PRs with typed forkctl JSON and `gh`.
- Document `GITHUB_TOKEN` approval behavior and GitHub App unattended setup without administering rulesets.

### Verification

- Validate workflow YAML and policy directly rather than reimplementing its graph in tests.
- Exercise no-op, new draft PR, update draft PR, stale proposal, explicit promotion, PR closeout, and cleanup in a bounded live test repository.
- Confirm no privileged workflow executes proposal-head code.
- Confirm proposal CI behavior for both `GITHUB_TOKEN` and an authorized test GitHub App.

## Phase 9 — Release and Consumer Proof

### Changes

- Review architecture, security, release, and silent-failure boundaries.
- Update README, generated instructions, examples, schemas, and task catalog.
- Release through the existing forkctl publication procedure.
- Migrate each known consumer as one clean contract slice if the manifest changed incompatibly.

### Verification

- `mise run verify`
- `mise run build`
- `cargo publish --dry-run --locked`
- Released-binary disposable multi-source lifecycle.
- Immutable remote catalog proof.
- GitHub proposal/promotion proof with exact release artifacts.
- Every migrated consumer passes fresh-clone initialization and complete checks.

## Deferred Plan

Do not begin embedded ordinary-repository mode until composition-owned mode and the shared projection engine are proven. Its separate design must choose tracked patch artifacts, managed-root ownership, normal Git merge semantics, and independent recovery requirements without introducing hidden parallel stack refs.
