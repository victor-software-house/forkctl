# Design

## Context

`open_proposal_pr` in [`src/app/publish.rs`][publish] builds its own
`std::process::Command` for `gh pr create`, turns every failure into `None`,
and uses fixed text. See [proposal.md](proposal.md) for the observed failure.
The branch update itself belongs to the
[`update-open-proposal`](../2026-09-29-update-open-proposal/proposal.md) change, which
lands first.

## Goals / Non-Goals

**Goals:**

1. An operator or CI job can never miss a missing pull request.
2. Failures that need no push are caught before the push.

**Non-Goals:**

1. Forges other than GitHub. Forkctl uses `gh` only for pull-request
   operations, as the [composed-upstreams decisions][decisions] state.
2. A configurable title, body, or draft state. No request names one.

## Decisions

1. **Fail after the push, not warn.** A create or edit failure after the push
   exits non-zero with `subprocess_failed` and `retryable: true`, and the
   message says the branch was pushed. The alternative was exit 0 with a
   status field and a warning notice. It lost because a CI job reading only
   the exit code would miss it, which is the reported failure. The operator
   chose this on 2026-09-28. A rerun is safe because the proposal push is
   leased and idempotent in shape.
2. **Look up before the push.** `gh pr list --repo R --head <proposal>
   --base <branch> --state open --json url` runs before `git push`. It proves
   that `gh` exists, is logged in, and can read the repository. The
   alternative was to push first and handle every `gh` failure afterwards. It
   lost because it leaves an orphan branch for failures that were knowable in
   advance.
3. **Derive `--repo` from the downstream remote URL.** Forkctl reads
   `remote.<name>.url` and accepts `https://HOST/OWNER/REPO`,
   `ssh://[USER@]HOST[:PORT]/OWNER/REPO`, and `[USER@]HOST:OWNER/REPO`, each
   with an optional `.git`. It passes `HOST/OWNER/REPO`. The alternative was
   to let `gh` pick the repository. It lost because `gh` prefers a remote named
   `upstream`, which every forkctl clone has.
4. **Reuse by editing.** An open pull request gets `gh pr edit --title --body`.
   The alternative was to leave its text alone. It lost because the text would
   then describe an older candidate.
5. **Keep the draft state.** A proposal is promoted by forkctl, never merged by
   GitHub, and a draft cannot be merged by accident. The alternative was a
   ready pull request, which lost for that reason. `gh pr edit` does not
   change the draft state of a reused pull request.
6. **Render the body with Askama.** A new `templates/proposal-pr.md` owns the
   body structure, as [AGENTS.md][agents] requires for generated Markdown.
7. **Run `gh` through `src/process.rs`.** The existing `capture` helper already
   produces `subprocess_failed` with the program, arguments, exit code, and
   stderr. Forkctl adds a message prefix after the push; it adds no error code.
8. **No override and no disable path.** `publish --propose` exists to open a
   pull request. An operator who wants only a branch can push a ref by hand.

## Risks / Trade-offs

1. [GitHub Enterprise hosts need `gh` logged in to that host.] → `--repo`
   carries the host, and the `gh` error says which host failed.
2. [A non-GitHub downstream loses `publish --propose`.] → It never produced a
   pull request there. `publish --rewrite` and `publish --append` are
   unchanged.

[publish]: ../../../../src/app/publish.rs
[decisions]: ../../../../goals/forkctl-composed-upstreams/decisions.md
[agents]: ../../../../AGENTS.md
