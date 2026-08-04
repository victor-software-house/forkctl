# Forkctl Composed Upstreams — Research

## Scope

This research asks how forkctl can compose selected files and directories from several Git repositories into one audited downstream patch stack, and how the resulting synchronization can be reviewed and promoted through GitHub Actions.

The current implementation assumes one upstream remote, one upstream tracking ref, one direct base target, and one StGit replay operation. See the pinned current [`Manifest`](https://github.com/victor-software-house/forkctl/blob/d1923d8d88615cbac25d5e7152f5d249cc05837e/src/manifest.rs), [`init`](https://github.com/victor-software-house/forkctl/blob/d1923d8d88615cbac25d5e7152f5d249cc05837e/src/app/init.rs), [`rebase`](https://github.com/victor-software-house/forkctl/blob/d1923d8d88615cbac25d5e7152f5d249cc05837e/src/app/rebase.rs), [`check`](https://github.com/victor-software-house/forkctl/blob/d1923d8d88615cbac25d5e7152f5d249cc05837e/src/app/check.rs), and [`publish`](https://github.com/victor-software-house/forkctl/blob/d1923d8d88615cbac25d5e7152f5d249cc05837e/src/app/publish.rs) boundaries.

## Primary Evidence

| Source | Observed contract | Consequence |
|:--|:--|:--|
| [Vendir](https://carvel.dev/vendir/) and its [versioned v0.46 configuration](https://carvel.dev/vendir/docs/v0.46.x/vendir-spec/) | A declarative file can assemble any number of sources into managed destination directories, select portions, and generate a lock with exact Git SHAs | Named sources, authored selection policy, generated locks, and collision-free destination ownership are proven concepts; forkctl's differentiator is ordered patch replay, recovery, and publication evidence |
| [Git partial clone](https://git-scm.com/docs/partial-clone) | Object filters omit some objects and fetch missing objects on demand; partial clone is independent from shallow commit selection | Promise strict downstream contents, not portable path-only network transfer; shallow/blobless source caches are optimization, not correctness |
| [Git sparse checkout](https://git-scm.com/docs/git-sparse-checkout) | Sparse checkout controls working-tree population and has significant behavior/performance constraints, especially outside cone-mode directories | Sparse checkout alone cannot create the desired self-contained multi-repository downstream history |
| [Josh filtered views](https://josh-project.github.io/josh/guide/gettingstarted.html) | Client filtering can present a path-composed repository and reverse it, but the client still downloads the full object database unless a Josh proxy performs server-side filtering | Josh is appropriate when reversible source pushback or a projection proxy is desired; it is unnecessary infrastructure for one-way exact snapshot composition |
| [git-filter-repo at `d7b75ac`](https://github.com/newren/git-filter-repo/blob/d7b75aca907380f608892cc289e616f195427b99/Documentation/git-filter-repo.txt) | Path extraction is a destructive history rewrite designed for fresh clones; partial use deliberately risks mixing old and new histories | It fits one-time extraction, not repeatable multi-source materialization into an audited patch lifecycle |
| [Git `ls-tree`](https://git-scm.com/docs/git-ls-tree), [`cat-file --batch`](https://git-scm.com/docs/git-cat-file), [`hash-object`](https://git-scm.com/docs/git-hash-object), [`write-tree`](https://git-scm.com/docs/git-write-tree), and [`commit-tree`](https://git-scm.com/docs/git-commit-tree) | Git exposes exact tree entries/modes, batched raw object bytes, object insertion, tree construction from an index, and commits with caller-selected trees and parents | Forkctl can compose exact projected Git content while continuing to delegate object semantics to Git instead of copying through lossy filesystem abstractions |
| [Git `read-tree --prefix`](https://git-scm.com/docs/git-read-tree) | Git can place a tree under a destination prefix and refuses to overwrite existing index entries | It is useful for non-remapped directory projections and demonstrates fail-closed collision semantics; per-file remapping still requires explicit index construction |
| [StGit `rebase --merged`](https://stacked-git.github.io/man/stg-rebase/) | StGit replays the stack onto a new base and leaves upstream-merged patches empty rather than deleting them | Composition can reuse the current replay primitive; forkctl remains responsible for binding and removing empty patches with recovery evidence |
| [GitHub reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows) | `workflow_call` supports typed inputs/secrets and is invoked as a complete job; caller permissions can be maintained or reduced, never elevated | Proposal and promotion are reusable workflow jobs, while each consumer owns triggers, permissions, and credentials |
| [GitHub workflow triggering](https://docs.github.com/actions/using-workflows/triggering-a-workflow) | PRs created or updated with `GITHUB_TOKEN` start PR workflows in an approval-required state; GitHub App or PAT events can trigger normally | Support the default token honestly and recommend a GitHub App for unattended synchronization |
| [GitHub ruleset bypass](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository#granting-bypass-permissions-for-your-branch-or-tag-ruleset) | Rulesets may grant bypass to GitHub Apps, roles, teams, and users, including pull-request-only bypass | Exact promotion may use an explicitly authorized app, but generic forkctl must not create or modify that policy |

## Tool Assessment

### Vendir

Vendir is the closest product analogue for source composition. It already handles multiple Git sources, destination layout, selected paths, and resolved locks. Delegating the complete operation to Vendir would nevertheless create a second manifest/lock contract, materialize through its filesystem policy, and leave forkctl to reverse-engineer Git mode/provenance and patch-layer evidence afterward.

Forkctl should borrow the source/lock distinction, not depend on Vendir for its Git-only first milestone.

### Josh

Josh is stronger when a filtered repository must preserve projected history and push changes back into the source. The selected forkctl milestone is one-way composition with independent downstream patches. Requiring a Josh proxy to guarantee path-only transport would add service infrastructure that the agreed strict-downstream guarantee does not require.

### Copybara

[Copybara at `d21fac4`](https://github.com/google/copybara/tree/d21fac43e951ef989b3c7b18c8bc8e9f0649ecac) is a broad migration and transformation system with GitHub destinations, change-request workflows, Starlark configuration, and one authoritative origin per ordinary workflow. It is appropriate for cross-repository code movement and reversible transformations, but adopting it would replace forkctl's narrow Git/StGit policy boundary rather than extend it.

### git-filter-repo

`git-filter-repo` is excellent for a one-time extraction that preserves selected history. Re-running destructive history filters for every source update and then combining the results would produce exactly the mixed-history and force-update complexity forkctl is meant to make explicit.

## Findings

### Keep

- Existing Git/StGit delegation.
- Explicit patch intent and ordered source/tooling patches.
- Recovery tags, current-operation journal, range-diff evidence, deterministic exports, and exact leases.
- Typed CLI/API handlers and read-only checks.

### Add

- Explicit direct-repository and composition base providers.
- Named Git sources with exact locked commits.
- Exact file/directory source-to-destination mappings.
- A deterministic synthetic base chain containing projected content only.
- Repeatable source selection for atomic subset sync.
- Rewrite and append history strategies with distinct ancestry checks.
- Durable remote proposal evidence usable by a fresh promotion runner.
- A review-only commit based on the old downstream tip with a tree equal to the exact candidate, so rewrite PRs show the net synchronization delta rather than the complete replayed patch stack.
- Versioned reusable GitHub proposal and promotion workflows.

### Reject

- Sparse checkout as the product model.
- Upstream source commits as synthetic-base parents.
- Globs, transforms, or source precedence in the first milestone.
- Hidden StGit refs to mix forkctl-managed roots with unrelated ordinary destination history.
- Normal GitHub merge/squash/rebase as exact candidate publication.
- Privileged execution of proposal-head code.
- Provider policy administration.

## Open Questions for Detailed Design

- Exact manifest enum and field names for the two base providers.
- Whether `sync` replaces direct-repository `rebase` or the two remain explicit verbs.
- Whether a source ref may initially be branch/tag/commit or branch-only.
- Deterministic synthetic-commit identity and timestamp policy.
- Whether a source ref advance with an identical projected tree is a no-op or records metadata-only provenance.
- Proposal tag payload encoding, signing policy, retention, and review-branch naming.
- Whether append replay ships in the first implementation PR or follows proven rewrite composition.
- Whether StGit can replay the existing named patches onto an epoch commit that already has the old patch tip as an ancestor without stack-state or merged-detection distortion; this requires a disposable prototype before append enters the public contract.
