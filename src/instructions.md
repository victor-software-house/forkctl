# forkctl agent instructions

`forkctl` maintains one explicit audited StGit downstream patch stack.

## Invocation

Always use the consumer's mise-provisioned task. Never call a PATH-global
`forkctl` or `stg` binary.

```
mise run fork -- status
mise run fork -- operation status
```

If a mid-stack refresh popped later patches that own `mise.toml`, the
mounted task disappears. Do not hunt install paths. Use the same mise
backend the task pins:

```
mise x github:victor-software-house/forkctl -- forkctl operation status
mise x github:victor-software-house/forkctl -- forkctl operation abort --yes
```

forkctl snapshots `mise.toml` and `mise.lock` into
`.git/forkctl/workspace/` at operation start. If they vanish from the
worktree, it restores them as temporary files so `mise run fork` still
works. Continue/abort remove those temps before StGit reapplies later
patches, so they cannot block `stg push`. The tracked manifest is not
copied into the worktree; the Git-private JSON snapshot already covers
operation continue/abort.

Everyday follow-up is a **new top patch**. `patch refresh` on a non-top
patch is refused unless you pass `--rewrite-below`. That flag unapplies
everything above the patch (including tooling that owns mise). It is
recoverable via `operation continue` / `abort`; it is not the everyday
edit path.

## Sources of truth

- Git commits and refs are canonical repository history.
- The manifest selected by `FORK_MANIFEST` or `--manifest` is canonical policy, target provenance, patch metadata, and recovery-bound history.
- `PATCHES.md` and source exports are generated evidence; edit metadata with forkctl, not generated files.
- Active patch and current operation are typed Git-private clone state.

## Patch workflow

1. `mise run fork patch create NAME -k source|tooling -p PURPOSE -u STATUS -d DROP_CONDITION -s SCOPE...` records explicit active intent.
2. Edit normally and stage with Git.
3. `mise run fork check -s` validates the index against the active patch without mutation.
4. `mise run fork patch refresh` captures staged files by default, refreshes the targeted StGit patch, regenerates evidence, and refreshes bookkeeping.
5. Repeat edit/stage/check/refresh as needed.
6. `mise run fork patch finish` runs the full check and clears active state.

`patch refresh -a` explicitly stages all changed paths owned by the patch. Repeated `-p PATHSPEC` limits capture to explicit Git pathspecs. Use `-n` on mutations to inspect the effect plan.

## Declared checks

A patch may declare commands that must succeed for it to still hold, so a rebase or refactor that quietly neutralizes it fails instead of passing:

- `-C NAME=COMMAND` declares a check; `{files}` expands to the checked files, shell-quoted. Exit 0 passes.
- `-g NAME=GLOB` restricts it; repeatable, and defaults to the declaring patch's scope.
- `--check-at NAME=stack|patch` selects the applied stack (default) or the patch's own commit in a disposable clone.
- `--clear-checks` replaces the complete set.

Globs are deliberately not limited to the patch's scope. Use a repository-wide check to catch cases the patch never covered — a new upstream call site bypassing a downstream wrapper, or a new trait implementor missing a downstream method. Write the rule to match the unhandled shape and let the tool exit non-zero. In Rust, forbidding a catch-all arm in a patched `match` delegates exhaustiveness to the compiler.

A check whose globs match no tracked file fails as stale, so a moved or deleted subject cannot silently disarm it.

Checks run during `check` after structural validation, so they gate `patch refresh`, `patch finish`, `rebase`, and both hook checks. They execute `sh -c` with repository-local Git variables cleared; provide their tools through the consumer's own mise configuration.

## Stack lifecycle

- `mise run fork init` hydrates a fresh clone with StGit metadata and exact historical recovery refs. Without a manifest it requires explicit bootstrap arguments and a branch exactly at its selected base.
- `mise run fork status` is read-only and remains usable during conflicts.
- `mise run fork check` is the complete clean-repository audit.
- `mise run fork rebase -o REF` records exact recovery/lease evidence and delegates replay to StGit; it never publishes.
- A rebase that leaves a surviving patch touching fewer paths records the lost paths as recovery-bound history and reports them as `path_changed_patches`; inspect them before publishing. This is path-delta evidence, not a claim of upstream causality.
- Resolve conflicts with supported Git/StGit commands, then run `mise run fork operation continue`.
- `mise run fork operation status` reports the exact phase and next actions; `operation abort -n` plans restoration and `operation abort -y` performs it. These remain usable when an in-flight conflict leaves the tracked manifest unreadable.
- Review the range-diff report and consumer semantic checks before `mise run fork publish`.
- `mise run fork publish` publishes any unpublished downstream state. It reports `already_published` when nothing changed, fast-forwards when the published tip is an ancestor, and otherwise creates an annotated recovery tag at the overwritten published tip before one atomic explicit-ref push. Pretty mode streams `git push` and hook output to stderr; JSON captures it.
- Publish is one atomic explicit-ref push with an exact lease and no fallback. A rewrite requires the published tip to equal the reviewed rebase lease, or the fetched downstream tracking ref when no operation is in flight; otherwise it fails with `remote_advanced`.

## Hook integration

Forkctl exposes commands; it does not own a hook manager:

- pre-commit: `mise run fork check -s`
- pre-push: `mise run fork check -q`

Checks never stage or rewrite. StGit refresh invokes the consumer's pre-commit hook and consumes its final index.

## Protocol

- `--format pretty|json` selects human or complete versioned output.
- `api call` reads one typed invocation and emits one JSON response.
- `api schema -k bundle|manifest|invocation|response|active-state|operation` emits JSON Schema 2020-12.
- `--usage-spec=fork` emits the mounted mise grammar from the Clap tree.
- `completion SHELL` supports bash, elvish, fish, Nushell, PowerShell, and zsh.
- JSON stdout never contains subprocess or human output.

## Safety

- Forkctl never guesses patch intent, stashes changes, edits hook configuration, or administers remote branch policy.
- Upstream push URL must remain `DISABLED`.
- Dirty worktrees, incomplete active patches, unresolved operations, scope drift, evidence drift, stale leases, and non-atomic publication fail closed.
- Keep consumer build, packaging, signing, hosting, and semantic compatibility outside forkctl.
