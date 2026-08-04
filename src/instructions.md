# forkctl agent instructions

`forkctl` maintains one explicit audited StGit downstream patch stack.

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

## Stack lifecycle

- `mise run fork init` hydrates a fresh clone with StGit metadata and exact historical recovery refs. Without a manifest it requires explicit bootstrap arguments and a branch exactly at its selected base.
- `mise run fork status` is read-only and remains usable during conflicts.
- `mise run fork check` is the complete clean-repository audit.
- `mise run fork rebase -o REF` records exact recovery/lease evidence and delegates replay to StGit; it never publishes.
- Resolve conflicts with supported Git/StGit commands, then run `mise run fork operation continue`.
- `mise run fork operation status` reports the exact phase and next actions; `operation abort -n` plans restoration and `operation abort -y` performs it. These remain usable when an in-flight conflict leaves the tracked manifest unreadable.
- Review the range-diff report and consumer semantic checks before `mise run fork publish`.
- `mise run fork publish` publishes any unpublished downstream state. It reports `already_published` when nothing changed, fast-forwards when the published tip is an ancestor, and otherwise creates an annotated recovery tag at the overwritten published tip before one atomic explicit-ref push.
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
