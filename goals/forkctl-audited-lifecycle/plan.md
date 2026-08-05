# Forkctl Audited Lifecycle

## Solution approach

Replace forkctl's current manifest shape in place while retaining the literal `schema: 1`. No compatibility reader or migration path will remain: existing `0.0.2` consumers stay on their immutable release, and `0.0.3` defines the first complete audited lifecycle contract.

Forkctl remains a small Rust policy layer over the supported `git` and `stg` executables. Git commits and refs remain canonical history; StGit remains responsible for stack creation, ordering, replay, conflict state, export, and clone recovery. Forkctl adds declared policy, deterministic projections, Git-private pending-operation evidence, and fail-closed command sequencing.

The planned manifest shape is:

```json
{
  "schema": 1,
  "downstream": {
    "remote": "origin",
    "branch": "main",
    "backup_tag_prefix": "vsh/pre-sync"
  },
  "upstream": {
    "remote": "upstream",
    "url": "https://github.com/example/project.git",
    "fetch_ref": "refs/heads/main"
  },
  "base": {
    "label": "refs/tags/v1.2.3",
    "canonical": "0000000000000000000000000000000000000000",
    "stack": "0000000000000000000000000000000000000000"
  },
  "ledger": "PATCHES.md",
  "bookkeeping_patch": "fork-tooling",
  "patches": [
    {
      "name": "downstream-change",
      "kind": "source",
      "purpose": "Describe why this downstream change exists.",
      "upstream_status": "not-submitted",
      "drop_when": "Upstream provides the required behavior.",
      "paths": ["src/example.rs"],
      "export": "patches/0001-downstream-change.patch"
    },
    {
      "name": "fork-tooling",
      "kind": "tooling",
      "purpose": "Own fork policy and generated bookkeeping.",
      "upstream_status": "inappropriate: downstream-only tooling",
      "drop_when": "The downstream fork is retired.",
      "paths": ["AGENTS.md", "FORK.md", "PATCHES.md", "mise.toml", "patches/*"]
    }
  ],
  "allow": { "base": [] },
  "required": [
    { "path": "FORK.md", "contains": "mise run fork:verify" }
  ]
}
```

Unknown fields remain errors. Source patches precede tooling patches; exports are optional; the declared bookkeeping patch is the final tooling patch. Every patch has one-line audit metadata and at least one path. The manifest, ledger, and exported-patch paths remain explicitly owned by the bookkeeping patch rather than gaining hidden path exceptions.

## Ordered implementation

### 1. Replace the manifest contract and deterministic projections

**Files:** `src/manifest.rs`, new focused `src/ledger.rs`, `examples/fork.yaml`, manifest unit tests.

- Replace the old upstream/bases/allow-tooling shape with the accepted schema-1 contract above.
- Add `Downstream`, `PatchKind`, per-patch audit metadata and paths, base label, ledger path, and bookkeeping-patch identity.
- Validate full SHAs, repository-relative paths, unique patch names/exports, one-line audit values, source-before-tooling order, and final bookkeeping-patch ownership.
- Make `source_top` return the stack base when no source patches exist.
- Render `PATCHES.md` deterministically from manifest order, base label/SHA, and escaped metadata; the manifest remains canonical and the ledger contains no self-referential commit hashes.
- Add pure tests for tooling-only stacks, malformed metadata, duplicate paths/exports, invalid ordering, and byte-stable ledger rendering.

**Verify:** targeted manifest/ledger unit tests pass; the example parses; rendering the same manifest twice is byte-identical.

### 2. Introduce shared repository state and safety primitives

**Files:** `src/app/mod.rs`, `src/process.rs`, new focused `src/state.rs` and `src/status.rs` data types.

- Separate upstream fetch configuration from explicit target resolution. Fetch the declared upstream branch into its remote-tracking ref; resolve `rebase --onto` through a targeted fetch and `FETCH_HEAD^{commit}` rather than deriving a branch from the rebase target.
- Add helpers for current/declared branch checks, remote URLs, exact `ls-remote` branch SHA capture, patch commit resolution, per-patch changed paths, Git-private paths, and atomic JSON/text writes.
- Store pending mutation evidence beneath the path returned by `git rev-parse --git-path forkctl`, so linked worktrees use their correct Git-private directory.
- Define one internal pending-state schema containing operation, expected downstream SHA, old base/tip, backup tag, selected target, and optional new base/tip/report path.
- Preserve direct argument passing to `git`/`stg`; add no shell execution and no VCS implementation.

**Verify:** pure state round-trip tests and disposable-repository helper tests pass; Git-private files never appear in worktree status.

### 3. Strengthen initialization and verification

**Files:** `src/app/init.rs`, `src/app/verify.rs`, `src/ledger.rs`.

- `init` requires a clean worktree on the declared downstream branch, adds or verifies upstream fetch URL, sets its push URL to `DISABLED`, fetches the configured upstream ref and base label, and reconstructs the exact declared linear patch count with `stg uncommit`.
- Preserve idempotent initialized-stack behavior; reject partial foreign stacks, merges, extra commits, and wrong order.
- `verify` checks downstream branch/tracking, downstream remote existence, upstream fetch/push policy, base identities, exact StGit order, all-applied state, per-patch path ownership, exact audit trailers, base allowlist, required text, generated ledger bytes, fresh export bytes, and reconstructed trees.
- Read trailers from Git's commit-format/trailer support rather than hand-parsing arbitrary commit messages.
- For tooling-only stacks, compare reconstruction directly with the stack-base tree.
- Reject empty declared patches during verification; this intentionally leaves a newly scaffolded patch incomplete until the operator adds its implementation.

StGit documents that `stg uncommit` converts ordinary single-parent commits into patches without modifying the commits, which is the clone-recovery mechanism forkctl continues to delegate to ([StGit uncommit](https://stacked-git.github.io/man/stg-uncommit/)).

**Verify:** real integration tests prove clean-clone initialization, initialized idempotency, tooling-only reconstruction, wrong-branch/dirty rejection, path/trailer/ledger/export failures, and valid source reconstruction.

### 4. Add read-only status and documented patch creation

**Files:** `src/main.rs`, new `src/app/status.rs`, new `src/app/new.rs`, `src/status.rs`.

- Add `status [--json]`. Collect state without mutation even when verification fails: repository/branch, remotes, base label/SHAs, applied/unapplied stack, exports, dirty paths, pending lease/rebase state, backup tag, and a verification result.
- Human status uses minimal ANSI color only when stdout is a terminal and `NO_COLOR` is absent. Redirected output and JSON never contain ANSI escapes.
- Add `new <name> --kind ... --purpose ... --upstream-status ... --drop-when ... --path ... [--export ...]`.
- `new` verifies the existing stack, captures the exact downstream lease, creates a recovery tag for the pre-mutation tip, pops to the layer-correct insertion point, creates an empty patch with `stg new --message`, reapplies later patches, atomically inserts manifest metadata, renders the ledger, stages only bookkeeping files, and refreshes the final bookkeeping patch.
- Leave implementation files untouched. Print the declared paths and the commands needed to refresh and verify the new patch.

StGit explicitly defines `stg new` as creating an empty patch on top of the currently applied stack and accepts a supplied commit message, while `stg goto` and `stg push --all` provide the ordering operations forkctl needs ([StGit new](https://stacked-git.github.io/man/stg-new/)).

**Verify:** CLI and real-stack tests cover plain/JSON status, color policy as a pure decision, source/tooling insertion, exact trailers, bookkeeping ownership, dirty rejection, and the expected incomplete verification state before implementation changes are refreshed.

### 5. Add targeted rebase, recovery, and review evidence

**Files:** `src/main.rs`, `src/app/rebase.rs`, `src/state.rs`, new `src/report.rs`.

- Change the public contract to `rebase --onto <ref>` and require cleanliness before any fetch, lease capture, or tag creation. Never invoke StGit's `--autostash` and never stash operator work.
- Capture the downstream branch's exact remote SHA, old base, and old tip before replay.
- Resolve only the requested upstream target, compute the canonical merge base against the fetched upstream tracking ref, and record the target label plus full commit identity.
- Create an annotated `backup_tag_prefix-<epoch>-<old-tip-abbrev>` tag before replay. Do not use `-f`; Git already fails if a tag exists, preserving immutability ([Git tag](https://git-scm.com/docs/git-tag)).
- Persist pending state before `stg rebase --merged <target-sha>`. On conflict, preserve StGit's normal state, backup tag, and pending file and print StGit's documented recovery sequence; do not interpret or auto-resolve conflicts ([StGit rebase](https://stacked-git.github.io/man/stg-rebase/)).
- After successful replay, refresh declared exports, base state, and ledger, and refresh the bookkeeping patch only when staged bytes changed so no-op rebases preserve commit identity.
- Run full verification, then write a no-color Git-private report containing identities, export hashes, structural result, and `git range-diff old-base..old-tip new-base..new-tip`. Git defines range-diff specifically for comparing two versions of a patch series ([Git range-diff](https://git-scm.com/docs/git-range-diff)).
- Rebase prints evidence and next checks but never pushes.

**Verify:** real-stack tests cover no-op commit stability, changed clean rebase, upstream-merged empty patch rejection, conflict state and recovery tag persistence, invalid target rejection, and report identities/range-diff content.

### 6. Add exact-lease atomic publication

**Files:** new `src/app/publish.rs`, `src/state.rs`, `src/process.rs`.

- `publish` requires clean full verification and pending state matching current HEAD/base/report.
- Read the downstream branch directly with `git ls-remote` and require it still equals the captured SHA.
- Push the annotated recovery tag and declared branch in one `git push --atomic` transaction with exact `--force-with-lease=refs/heads/<branch>:<expected-sha>` and explicit source/destination refspecs.
- Never use plain `--force`, a leading `+`, or lease inference from remote-tracking state. Git documents that exact leases compare against the supplied SHA and that `--atomic` fails rather than partially updating when unsupported ([Git push](https://git-scm.com/docs/git-push)).
- Do not retry non-atomically. After success, verify both remote refs through `ls-remote`, then remove pending state atomically.

**Verify:** disposable bare-remotes prove concurrent remote advancement rejects without changing either ref, conflicting backup tags reject, unsupported atomic behavior does not trigger fallback, successful publication updates both refs, and pending state clears only after remote verification.

### 7. Build the real Git/StGit integration harness

**Files:** new `tests/support/mod.rs`, new focused integration files under `tests/`, `mise.toml`.

- Add exact `cargo:stgit` 2.6.1 to forkctl's development mise tools so the canonical test task provides the real executable.
- Build fixtures with bare upstream/downstream remotes, working clones, local Git identity, upstream branch/tag evolution, ordinary downstream commits, generated schema-1 manifest/ledger, and real StGit commands.
- Keep test helpers in support code and scenarios in bounded files instead of one monolithic integration test.
- Cover all automated facts: init/idempotency, dirty/wrong branch, tooling-only, path/trailer/ledger/export drift, status JSON, patch insertion, no-op/changed/conflicting rebase, recovery evidence, lease rejection, and successful publish.

**Verify:** `mise run test` executes all unit and integration scenarios against installed Git and StGit; no test depends on Zed or network access.

### 8. Align documentation, task catalog, version, and local release

**Files:** `README.md`, `AGENTS.md`, `CODING_STANDARDS.md`, `src/instructions.md`, `examples/`, `tasks/fork.toml`, `Cargo.toml`, `Cargo.lock`, `xtask/src/main.rs` only if synchronization markers change.

- Update all guidance to the single audited schema-1 lifecycle and remove the obsolete three-operation restriction.
- Expose `fork:init`, `fork:status`, `fork:new`, `fork:verify`, `fork:rebase`, and `fork:publish` from the remote task catalog with exact existing tool provisioning.
- Keep consumer semantic tests outside forkctl and make publication explicitly separate.
- Set `[workspace.package].version` to `0.0.3`; run `mise run version:sync` so task and example release references update automatically.
- Run strict formatting, workspace Clippy, all unit/integration tests, release build, package dry-run, and a synthetic cold-clone remote-task proof.
- Publish `0.0.3` through the existing local GitHub/crates.io release path and verify the tag, asset digest, crates.io metadata, both mise installation backends, embedded instructions, and repository cleanliness.
- Add no GitHub Actions workflow and no native asset matrix. Those remain deferred to the future shared workflow collection, which will use GitHub-hosted default runners for this public repository.

**Verify:** `mise run verify`, `mise run build`, `cargo publish --dry-run --locked`, released catalog invocation, cold-clone lifecycle proof, release/ref inspection, and clean synchronized Git state all pass.

## Risks and controls

| Risk | Control |
|:--|:--|
| `new` or rebase fails after stack reordering | Create recovery tag and pending state before history mutation; stop without hidden rollback when StGit reports conflicts. |
| Empty patch scaffold conflicts with normal verification | `new` deliberately ends with actionable incomplete status; `publish` remains blocked until implementation is refreshed and verification passes. |
| Generated ledger becomes a second source of truth | Manifest is canonical; ledger is deterministic and byte-verified, with no commit hashes. |
| Background fetch weakens publication safety | Use the captured explicit remote SHA, not a remote-tracking ref or implicit lease. |
| Atomic tag/branch publication is unsupported | Fail without fallback; leave pending state and local recovery tag intact. |
| Real integration tests become slow or brittle | Use tiny local repositories and supported CLI contracts only; no network, Zed, or copied VCS logic. |
| Current `0.0.2` consumers use the old shape | They remain pinned to immutable `0.0.2`; `0.0.3` intentionally has no compatibility reader. |
| Release scope expands into CI design | Do not add workflows or matrix assets in this goal; use the current local release task only. |

## Done condition

The goal is complete when forkctl `0.0.3` is published and independently verified, all accepted facts have automated evidence where selected, a synthetic repository can execute the full audited lifecycle without network access beyond release installation, no Zed file was touched, no compatibility code remains, and the repository is clean and synchronized.
