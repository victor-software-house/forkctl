# Design

## Context

`publish_propose` in [`src/app/publish.rs`][publish] builds the proposal with
`git commit-tree HEAD^{tree} -p <downstream tip>`, points the local
`refs/heads/forkctl/proposal/<branch>` at it, and pushes that ref with a plain
`git push`. A second proposal is a new root-level sibling of the first, so the
remote rejects it as non-fast-forward. See [proposal.md](proposal.md) for the
failure.

## Goals / Non-Goals

**Goals:**

1. Update an existing proposal branch without deleting it.
2. Never replace a remote proposal tip that forkctl did not read.

**Non-Goals:**

1. Pull-request creation and reporting. That is the
   [`report-proposal-pull-request`](../2026-09-29-report-proposal-pull-request/proposal.md)
   change.
2. Recovery tags for replaced proposal commits. A proposal is a review surface,
   not published history, and the candidate stays reachable from the stack.

## Decisions

1. **Rewrite under a lease, not append.** The new proposal commit keeps the
   shape `parent = downstream tip, tree = candidate`, and the push carries
   `--force-with-lease=refs/heads/forkctl/proposal/<branch>:<read tip>`.
   The alternative was to parent the new commit on the current proposal tip,
   which a plain fast-forward push accepts. It lost for three reasons. First,
   `publish --promote` checks that the proposal's parent is the downstream
   tip, and an appended chain breaks that check. Second, a proposal made before
   the downstream branch advanced would keep an old base. Third, the
   [composed-upstreams decisions][decisions] already fix the review commit as
   a single commit whose parent is the old downstream tip. The operator chose
   the lease rewrite on 2026-09-28.
2. **Read the tip with `git ls-remote` right before the push.** An absent ref
   becomes the lease `refs/heads/forkctl/proposal/<branch>:`, which makes git
   require that the ref still does not exist. The alternative was the local
   remote-tracking ref. It lost because a clone that never fetched the
   proposal branch would hold no tracking ref, or a stale one.
3. **No override and no disable path.** The lease is part of the
   `publish --propose` contract, like the exact lease on the downstream branch.
   Forkctl has no fallback push, as [AGENTS.md][agents] requires.

## Risks / Trade-offs

1. [A pull request loses its visible commit history on each update.] →
   GitHub keeps the pull request, its conversation, and its outdated review
   comments. The net delta against the downstream branch stays exact.
2. [Two operators proposing at once.] → The loser's push fails on the lease,
   and a rerun reads the new tip.

[publish]: ../../../../src/app/publish.rs
[decisions]: ../../../../goals/forkctl-composed-upstreams/decisions.md
[agents]: ../../../../AGENTS.md
