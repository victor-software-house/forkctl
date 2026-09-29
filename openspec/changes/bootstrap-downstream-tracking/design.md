# Design

## Context

Bootstrap wrote the manifest and StGit state, then ran the full check, whose
`require_declared_branch` resolves `@{upstream}`. A fresh upstream clone
tracks `upstream/main`, and an empty downstream remote has no branch to
track, so the check failed after mutation. See [proposal.md](proposal.md).

## Decisions

1. **Create the downstream branch in `init`, at the base.** The alternative
   was to let the first `publish` create the branch. It lost because every
   publication path assumes an existing remote tip: the exact lease, the
   fast-forward test, append bridges, recovery evidence, and the post-push
   read-back. Rebase also captures the remote tip as its lease. Creating the
   branch at the exact base keeps all of those unchanged, and the first
   publish becomes a plain fast-forward. The operator was not available for
   this choice on 2026-09-28; it is recorded here for review.
2. **Lease the creation on absence.** `--force-with-lease=<ref>:` makes the
   push fail if another writer created the branch meanwhile.
3. **Set tracking before StGit state.** The fetch and
   `git branch --set-upstream-to` run right after the creation push, so a
   later failure leaves a clone that a rerun of `init` can bootstrap again.
4. **One tracking error.** `require_declared_branch` treats "no upstream"
   like a wrong upstream, with a suggested command. The alternative was to
   configure tracking silently on every command. It lost because commands
   other than `init` must not rewrite branch configuration.
5. **No override and no disable path.** An operator who wants the downstream
   branch to start elsewhere pushes it before `init`; bootstrap then only
   sets tracking.

## Risks / Trade-offs

1. [The creation push runs the consumer's `pre-push` hooks.] → The `init`
   instructions say to install hooks that call forkctl after bootstrap.
2. [Downstream CI runs once on the unmodified base.] → That is the upstream
   commit the fork starts from, and the push is shown in the dry-run plan.
