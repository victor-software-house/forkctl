# Forkctl Composed Upstreams and Reviewable Sync

## Objective

Extend forkctl from one full upstream repository to a composition-owned repository assembled from exact file and directory projections of multiple Git sources, followed by one ordered downstream patch stack.

The resulting repository must contain only selected upstream content and declared downstream patches. A source may advance independently, several sources may advance atomically, and the existing audited replay, recovery, range-diff, validation, and publication principles must remain intact.

Add a provider-neutral proposal protocol that lets GitHub Actions prepare an exact candidate and review PR, then promote that unchanged candidate through a separate trusted workflow after human review.

This goal defines future architecture. It does not describe behavior available in the current release.

## Why

A full fork is appropriate when the downstream needs an upstream repository as a whole. It is wasteful when the useful contract is smaller:

- one file from one repository;
- one directory from another repository;
- a third source projected under a different destination;
- a small ordered patch series spanning those projected components.

Sparse checkout does not solve that product problem. It changes which paths appear in one repository's working tree; it does not create a self-contained downstream history containing only selected material, and it does not compose several repositories.

The same source synchronization should be operable locally and from scheduled automation without turning GitHub's merge, squash, or rebase buttons into unreviewed changes to forkctl's exact patch topology.

## Product Model

```text
Git source A @ exact commit ── selected path mappings ──┐
Git source B @ exact commit ── selected path mappings ──┼─> synthetic base
Git source C @ exact commit ── selected path mappings ──┘         │
                                                                  └─> ordered StGit patches
```

A source has a stable name, Git URL, tracked ref, exact resolved commit, and one or more exact `from` → `to` mappings. The destination path set is disjoint. There is no source precedence, overlay order, glob membership, content transform, or silent collision resolution.

The first mode is composition-owned: every tracked path is either present in the synthetic base or introduced/changed by a declared forkctl patch. Importing managed roots into an otherwise independently evolving ordinary repository is a separate future mode and is not part of this goal.

## Non-Negotiable Invariants

- Keep Git and StGit as the repository and patch mechanics.
- Preserve the existing repository base-provider behavior for full upstream forks; composition is an explicit second base provider, not an implicit special case.
- Resolve every selected source target before mutating the downstream repository.
- Copy only selected source entries into downstream history.
- Never make upstream source commits parents of a synthetic base commit; doing so would make upstream histories reachable from the downstream.
- Preserve regular files, executable modes, symbolic links, and Gitlinks as Git tree data. Preserve LFS pointer blobs as pointers.
- Reject overlapping destination roots, missing projection roots, invalid paths, and source identity drift before replay.
- Treat additions, removals, and renames beneath a projected directory as normal upstream changes.
- Let patches span several projected destinations and add downstream-only files.
- Keep checks read-only. Network resolution and source-cache mutation belong to initialization and synchronization.
- Keep rewrite as the default publication history. Support append replay only as an explicit policy.
- Never publish a candidate after its captured downstream lease changes.
- Keep GitHub orchestration thin. Forkctl owns candidate construction and verification; `gh` owns PR operations.
- Never execute proposal-head code with privileged `pull_request_target` credentials.
- Do not administer provider rulesets or bypass policy.

## Synchronization

`sync` replaces the one-target mental model with a source-set operation. With no source selection it resolves all configured sources; repeatable source selection advances only those sources while retaining every other exact lock. The operation is atomic across the selected set.

Forkctl materializes a new composite base only when selected projected content or lock evidence requires a new state. It then delegates patch replay to StGit, detects empty merged patches, regenerates manifest/ledger/exports, runs full validation, and produces review evidence.

### Rewrite history

```text
B0 ── P0a ── P0b       old published tip
 └── B1 ── P1a ── P1b  new candidate
```

Rewrite is the default. It leaves one clean current patch generation and publishes under an exact force-with-lease.

### Append history

```text
B0 ── P0a ── P0b ───── E1 ── P1a ── P1b
 └── B1 ────────────────┘
```

Append creates an explicit epoch commit `E1` whose parents are the old published tip and new composite base and whose tree equals the new composite base. Fresh patch commits replay above it. The final tree must equal rewrite mode, while the old published tip remains an ancestor and publication is fast-forward-only.

Append intentionally retains prior patch generations. It is not a normal content merge and must not be described as one.

## Review Proposals

A proposal separates review from exact publication:

1. A trusted preparation run synchronizes sources, validates the exact candidate, and creates immutable recovery and proposal evidence.
2. It creates a review commit whose parent is the captured old downstream tip and whose tree equals the exact candidate tree.
3. It atomically pushes the review branch, recovery tag, and proposal tag; the proposal tag keeps the exact candidate reachable and binds it to the review commit.
4. A draft GitHub PR from the review branch presents the net old-tip → candidate-tree change, source-lock changes, patch changes, dropped patches, range-diff summary, validation, and proposal identity.
5. After review, an explicit trusted promotion run fetches the proposal into a fresh clone, verifies the review/candidate tree identity and every bound object, confirms the unchanged downstream lease, reruns checks on the exact candidate, and moves the downstream ref to that candidate.
6. Promotion verifies remote state, comments on and closes the review PR, and only then removes the mutable review branch.

The separate review commit matters for rewrite proposals: a PR directly from the divergent replay candidate would use an older merge base and show the whole downstream patch stack instead of the net synchronization delta. The review commit has the candidate tree but is never the publication target.

A normal GitHub merge, squash, or rebase is not publication because each would publish the review topology rather than the exact forkctl candidate.

## GitHub Actions Experience

Ship versioned reusable proposal and promotion workflows. Consumer workflows remain thin and own their schedule or manual trigger, permissions, credentials, and repository policy.

The proposal workflow serializes runs per downstream branch, installs an exact forkctl release, prepares or updates one stable proposal PR, and emits typed outputs. The promotion workflow is `workflow_dispatch`-driven and runs trusted default-branch code.

The default `GITHUB_TOKEN` path may require manual approval before PR CI starts. Fully unattended operation uses a least-privilege GitHub App installation token and, where repository rules require it, an explicitly configured app bypass. Forkctl reports missing permission and never changes the policy itself.

## Explicit Exclusions

- Independently evolving ordinary destination content outside forkctl's stack.
- Hidden component-stack refs for mixed repositories.
- Globs, include/exclude filters, overlays, transforms, templates, or precedence.
- Non-Git sources.
- Bidirectional synchronization or pushing patches back to source repositories.
- SemVer/tag-range source selection.
- Source credentials in the tracked manifest.
- Provider-specific source APIs as the generic materialization path.
- Automatic promotion on review approval.
- Provider branch-policy administration.
- Compatibility aliases or fallback behavior for any future incompatible contract.

## First Milestone Done Condition

- Two or more disposable Git sources project exact files/directories into one composition-owned downstream.
- One patch spans more than one source destination and replays successfully after a subset source update.
- The downstream object graph and worktree contain no unrelated source content.
- Collision, stale-root, source-resolution, replay, and lease failures leave downstream refs and tracked files unchanged.
- Rewrite synchronization preserves current forkctl audit/recovery guarantees.
- A fresh clone reconstructs and checks the projected stack without requiring complete upstream histories.

## Broader Goal Done Condition

- Rewrite and append produce identical final trees with their documented ancestry guarantees.
- Proposal preparation and promotion succeed from separate clean runners and reject stale, substituted, or tampered evidence.
- Versioned reusable GitHub workflows open/update review PRs and explicitly promote exact candidates.
- Documentation, manifest/API schemas, help, completion, and lifecycle fixtures all describe the same contract.
- Existing full-repository fork behavior remains explicit and independently verified.

## Source Package

- `facts.md` — accepted testable outcomes.
- `decisions.md` — binding product and scope decisions.
- `research.md` — primary-source evidence and tool assessment.
- `requirements.md` — normative behavioral and verification contract.
- `design.md` — proposed architecture, state, topology, CLI, and workflow model.
- `plan.md` — coherent implementation and release slices.
