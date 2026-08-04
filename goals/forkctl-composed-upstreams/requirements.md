# Forkctl Composed Upstreams — Requirements

## 1. Base Providers

1. Forkctl shall model a direct full-repository upstream and a multi-source composition as explicit base-provider variants.
2. Existing direct-provider ancestry shall not be silently converted into synthetic composition history.
3. A composed base shall contain selected source entries only and shall not parent upstream source commits.
4. Every synthetic base after the first shall name the previous synthetic base as its sole parent.

## 2. Source Contract

1. Every source shall have a unique stable name, Git URL, tracked ref, exact resolved commit, and one or more exact projections.
2. A projection shall identify one exact source file or directory prefix and one exact destination path.
3. All destination paths shall be normalized repository-relative paths.
4. Destination equality and parent/child overlap shall fail before downstream mutation.
5. A missing mapped source root shall fail as stale configuration.
6. Additions, removals, and renames beneath an existing mapped directory shall be accepted as source changes.
7. The first milestone shall not support globs, include/exclude rules, content transforms, overlay order, or non-Git source kinds.
8. Credentials shall not be stored in the tracked manifest.

## 3. Projection Integrity

1. The downstream object graph reachable from synthetic bases shall contain no unselected source blobs or source commit histories.
2. Projection shall preserve Git entry bytes and modes, including executable files, symbolic links, and Gitlinks.
3. Git LFS pointer blobs shall remain pointers unless a future explicit policy changes that behavior.
4. Forkctl shall delegate source tree enumeration, raw object access, blob insertion, tree writing, and commit creation to Git plumbing.
5. Source-cache depth or object filtering shall not alter the resulting synthetic tree.
6. Network transfer reduction shall be documented as transport-dependent rather than a strict path-only guarantee.

## 4. Synchronization

1. Synchronization with no source selection shall resolve all sources.
2. Repeatable source selection shall update exactly the named sources while retaining other locks.
3. All selected targets shall be resolved before the first downstream mutation.
4. Source resolution, materialization, replay, generated bookkeeping, and validation shall form one recoverable operation.
5. A failure shall retain sufficient evidence to continue or abort without partially publishing downstream refs.
6. Patches shall remain ordered and may touch several projected destinations or downstream-only paths.
7. Forkctl shall use StGit replay semantics and shall bind every removed empty patch to exact pre-sync recovery evidence.
8. Read-only repository checks shall not fetch or mutate source caches.
9. A fresh downstream clone shall reconstruct and validate current/historical projected states without complete upstream histories.

## 5. History Strategies

### 5.1 Rewrite

1. Rewrite shall be the default.
2. The new stack base shall be the new synthetic base.
3. Publication shall require the captured downstream lease and exact candidate identity.
4. No plain force, implicit lease, or publication fallback shall exist.

### 5.2 Append

1. Append shall require explicit selection.
2. Forkctl shall create an epoch commit with the old published tip and new synthetic base as parents.
3. The epoch tree shall equal the new synthetic-base tree exactly.
4. Fresh patch commits shall replay above the epoch.
5. The old published tip shall be an ancestor of the append candidate.
6. Append publication shall be fast-forward-only.
7. Rewrite and append from the same old state, source locks, and patch inputs shall produce identical final trees.
8. Documentation shall state that append retains previous patch generations and is not a normal merge.

## 6. Durable Proposals

1. Proposal preparation shall not move the downstream publication ref.
2. A proposal shall bind the downstream ref, expected old SHA, exact candidate SHA, history strategy, recovery object, source-lock delta, synthetic base, report object, and manifest object.
3. Proposal preparation shall create a single-parent review commit whose parent is the expected old downstream SHA and whose tree equals the exact candidate tree.
4. Recovery and proposal evidence shall be immutable annotated Git objects; the proposal tag shall keep the exact candidate reachable.
5. Review branch, recovery tag, and proposal tag shall be pushed atomically under explicit leases.
6. Proposal verification shall work in a fresh clone without local operation state.
7. Verification shall prove review-parent identity and review-tree/candidate-tree equality without treating the review commit as the publication candidate.
8. Promotion shall fetch and verify every bound object, rerun the complete repository check on the exact candidate, and reject a changed downstream lease.
9. Promotion shall update the downstream ref to the exact candidate and verify the remote effect.
10. Review-branch cleanup shall happen only after verified promotion and provider-PR closeout.
11. Provider branch or tag names alone shall never be trusted as proposal identity.

## 7. Presentation

1. Pretty source, sync, proposal, status, and error prose/tables shall fit the detected terminal width, with `COLUMNS` as the non-TTY fallback.
2. JSON and generated machine contracts shall never reflow.
3. Narrow, standard, and wide subprocess tests shall enforce maximum visible line width without making wide output consume unnecessary space.

## 8. GitHub Actions

1. Forkctl shall expose typed JSON results sufficient for provider automation without parsing pretty output.
2. GitHub integration shall use versioned reusable proposal and promotion workflows.
3. Consumer callers shall own schedule/manual triggers, permissions, source credentials, and repository policy.
4. Proposal runs shall serialize by downstream repository and branch.
5. Proposal PRs shall be draft review surfaces and render the net old-tip → candidate-tree diff, source-lock changes, patch changes/drops, validation, review evidence, proposal identity, and promotion instructions.
6. Promotion shall require explicit `workflow_dispatch` in the first release.
7. Trusted promotion shall execute workflow code from the downstream default branch.
8. Workflows shall not execute proposal-head code through `pull_request_target`.
9. The documented default-token path shall describe GitHub's approval-required PR checks.
10. The unattended path shall recommend a least-privilege GitHub App installation token.
11. Forkctl shall report ruleset rejection but shall not create or modify bypass policy.

## 9. Verification

Automated disposable Git/StGit fixtures shall prove:

- composition from at least two independent bare source repositories;
- file and directory remapping;
- regular/executable files, symlinks, and Gitlinks;
- destination collision and stale-root rejection before mutation;
- one-source, subset, and all-source synchronization;
- deletion and rename beneath mapped directories;
- no downstream change for irrelevant source content changes according to the settled no-op policy;
- one patch spanning multiple source destinations;
- upstream incorporation and recovery-bound empty-patch removal;
- absence of unrelated source content and unreachable source commits in downstream history;
- rewrite/append final-tree equivalence and ancestry differences;
- proposal preparation and promotion in separate clean clones;
- stale lease, candidate tampering, proposal substitution, unsupported atomic push, and protected-branch rejection;
- generated documentation/schema/help/completion parity.

## 10. Deferred Requirements

This goal shall not require mixed ordinary-repository ownership, canonical patch-file mode, bidirectional source pushback, SemVer source selection, automatic promotion on approval, or provider policy administration.
