# Forkctl Development Guide

Forkctl is a Rust policy CLI for explicit audited StGit downstream patch stacks.

## Architecture

- Delegate repository and patch mechanics to installed `git` and `stg`; never reproduce either model.
- `src/cli.rs` is the Clap adapter and sole command/parameter grammar.
- `src/protocol.rs` is the versioned Serde/Schemars request/result/notice/error/schema contract.
- `src/app/` owns typed repository operations and never prints or chooses a view.
- `src/view.rs` owns pretty command output; `src/help.rs` owns help derived from Clap metadata; `src/layout.rs` owns shared terminal-width detection and wrapping. All use one semantic Anstyle/Comfy Table system.
- `src/process.rs` is the only production child-process factory and clears Git repository-local hook variables for explicit cwd execution.
- Git-private active/operation state lives under `$(git rev-parse --git-path forkctl/)`.
- Askama templates own generated Markdown document structure.

## Product invariants

- Patch intent is explicit; forkctl never routes changes by filename inference.
- One clone has at most one active patch.
- `check` is the sole validation command: full repository by default, staged index with `-s`.
- `patch refresh` captures staged by default and owns StGit targeting plus all generated bookkeeping.
- Checks never stage or mutate.
- Every source patch has one deterministic generated export; tooling patches have none.
- One typed current-operation journal exposes status/continue/abort.
- Operation-scoped commands read the Git-private manifest snapshot, so an unreadable tracked manifest never blocks status, continue, or abort.
- Historical dropped patches are bound to the exact annotated recovery tag object preserving the old stack.
- A rebase that leaves a surviving patch touching fewer paths records the lost paths as recovery-bound replay evidence; path disappearance is never silent and never attributed to upstream without proof.
- A patch may declare checks: commands that must exit zero, with `{files}` expanded from globs that default to the declaring patch's scope and may reach anywhere. Scope governs what a patch may modify, never what it may check.
- A check whose globs match no tracked file fails as stale rather than passing vacuously, so a moved or deleted subject can never silently disarm it.
- Checks observe the applied stack by default, or the declaring patch's own commit when declared `at: patch`; both stages run with a disposable clone as cwd and no origin remote. Declared commands are trusted user-level code, not a hostile-code sandbox.
- Forkctl ships no checking tooling and embeds no parser, query language, or scripting runtime; a check names whatever the consumer provides.
- Publish covers every unpublished downstream state: idempotent when identical, fast-forward when the published tip is an ancestor, and otherwise an annotated recovery tag at the overwritten published tip plus one atomic explicit-ref push under one exact lease, with no fallback.
- An identical branch never clears a ready operation until every required remote recovery ref matches; missing evidence is repaired atomically and mismatched evidence fails closed.
- Every rewrite publication requires exact evidence of the tip it overwrites: the reviewed rebase lease, or the fetched downstream tracking ref when no operation is in flight.
- Forkctl does not install hooks, edit `core.hooksPath`, or administer provider branch policy.
- No compatibility reader, alias, migration, or fallback exists for older forkctl contracts.

## CLI and integration

- Keep core verbs top-level. Use `patch`, `operation`, and `api` subcommands only for distinct actions; modes of one operation are parameters, not optional subcommands.
- Leaf parameters are orthogonal and composable, with repeatable values, visible defaults, and collision-audited short forms.
- Help, Usage KDL, shell completion, CLI requests, and JSON Schema derive from the authoritative Clap/protocol types rather than copied literals.
- Pretty help, result tables, notices, and errors fit the detected terminal width; redirected output may use the standard `COLUMNS` fallback. JSON and generated machine contracts never reflow.
- The remote catalog exposes one mounted `fork` file task using `dir = "{{cwd}}"`, `raw_args = true`, exact task tools, the mise-documented self-mount `mise run --quiet fork -- --usage-spec=fork`, and `exec forkctl "$@"`.
- VSH Lefthook defaults call `mise run fork check -s` on pre-commit and `mise run fork check -q` on pre-push; other managers call the same commands.
- `skills/forkctl/SKILL.md` is the portable skills.sh-installable operator workflow. Keep it concise and synchronized with released CLI behavior; installed help/instructions remain authoritative, so the skill must not duplicate a full flag grammar.

## Versioning

`[workspace.package].version` is the only forkctl release-version source. Root `mise.toml` is the only source for the minimum mise, Rust, StGit, Lefthook, Usage, and GitHub CLI versions. `mise run version:sync` regenerates `mise.lock` and every operational copy; `version:check` rejects drift. Continue patch releases; do not introduce a minor bump without explicit operator direction.

## Checks

```sh
mise run verify
mise run build
```

Every release must additionally be exercised through the immutable mise catalog and the published binary against disposable real Git/StGit remotes, including bootstrap, active patch capture, hooks, abort/continue, rebase history hydration, stale lease, protected-branch rejection, and successful atomic publication. Supply `CARGO_REGISTRY_TOKEN` before invoking the release task; it must fail before creating GitHub state when an unpublished version lacks auth and must resume only an exact-target draft.
