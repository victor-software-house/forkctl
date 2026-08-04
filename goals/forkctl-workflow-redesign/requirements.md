# Forkctl Workflow Redesign — Requirements

## Objective

Deliver a from-scratch forkctl contract that makes explicit downstream patch work fast, safe, composable, and fully scriptable. An operator chooses patch intent; forkctl validates and performs the repetitive StGit, evidence, bookkeeping, and publication work.

## Invariants

1. Git commits/refs and StGit patches remain canonical repository mechanics.
2. Forkctl never infers patch intent or owner from changed paths.
3. One clone may have at most one explicitly active patch.
4. Domain handlers return typed data only; input and output protocols never leak into repository logic.
5. Pretty CLI and JSON/API output consume the same typed results and errors.
6. Mutations never stash operator changes.
7. Full repository check, rebase, and publish fail on an unclean worktree, incomplete active patch, or unresolved operation.
8. Every source patch has a deterministic generated export; tooling patches do not reconstruct product source.
9. Recovery tags are immutable annotated objects and every historical removal names the exact tag object preserving its old stack.
10. Publication is one atomic explicit-ref push with an exact lease and no fallback.
11. Forkctl never edits provider branch-protection policy.
12. No previous format or command receives compatibility support.

## CLI Requirements

### Global behavior

- `--manifest PATH`/`-m PATH` selects the manifest; `FORK_MANIFEST` remains the environment alternative.
- `--format pretty|json`/`-f pretty|json` selects one output representation. There is no `--json` alias.
- `--color auto|always|never`/`-c ...` controls pretty output only and respects `NO_COLOR`.
- `--quiet`/`-q` suppresses non-error pretty output; JSON remains complete.
- Every mutating command exposes `--dry-run`/`-n`; planning executes all reads and validation but performs no writes, ref changes, or hooks.
- Destructive recovery requires `--yes`/`-y` in non-interactive contexts and displays the exact discarded state in pretty mode.
- Most command-local long options have a mnemonic short form. Global shorts are reserved across the tree; command-local shorts may repeat only on disjoint subcommands. Misleading abbreviations remain long-only.
- Leaf-command parameters are orthogonal and composable: subject selection, metadata/scope, capture selection, execution safety, and global presentation do not duplicate one another as mode-specific commands.
- Repeatable values use repeatable options, composed conveniences expand into the same typed option groups, and defaults remain visible in help.
- `--help` is a colored, width-aware, scan-friendly view generated solely from Clap metadata. It groups commands/arguments/options, shows short and long forms, metavars, defaults, choices, and descriptions, and uses forkctl's existing semantic theme.
- `forkctl completion SHELL` generates completions for bash, elvish, fish, Nushell, PowerShell, and zsh from the same Clap graph. Completions include commands, options, short forms, enums, paths, repository remotes/refs, and live patch names.
- Dynamic completion is local/offline and fail-silent: an absent/invalid manifest yields no domain candidates rather than terminal errors or network access.
- Hidden `--usage-spec[=BIN]` emits a full Usage KDL specification generated from Clap and augmented with the same dynamic candidates; `BIN` relabels the mounted root command. The mise task is a `raw_args` proxy, so forkctl retains authoritative validation/help, while mise requests `--usage-spec=fork` only for equivalent shell completion.
- Invalid CLI syntax exits 2; actionable repository/policy failure exits 1; success exits 0. Machine callers use typed error codes rather than additional exit-code taxonomy.

### Core lifecycle

- With no manifest, `forkctl init` bootstraps a new contract from explicit upstream/downstream/base/document/bookkeeping arguments plus repeatable allowed-base globs and required `PATH=TEXT` assertions, requires the branch to equal the selected base, creates the initial bookkeeping patch, and writes the first verified manifest/ledger. It never imports or interprets legacy commits.
- With a manifest, `forkctl init` idempotently hydrates StGit metadata and exact historical recovery refs in a fresh clone.
- `forkctl status` reports repository identity, base/target, patch series, active patch, staged/unstaged paths, check summary, and current operation without mutation.
- `forkctl check` performs the complete offline structural, audit, export, history, and reconstruction gate.
- `forkctl check --staged`/`-s` performs the narrower read-only index/scope check against `--patch NAME`/`-p NAME` or the active patch. An empty index succeeds; a nonempty index without a target fails.
- `forkctl rebase --onto REF` starts or completes a journaled upstream replay and never publishes.
- `forkctl publish` requires a successful full check and atomically publishes the branch and exact recovery tag under the recorded lease.
- `forkctl instructions` emits the generated consumer/operator contract.
- `forkctl contract edit` appends declarative allowed-base/required-text contracts, or explicitly clears and replaces the complete set; it validates required files/text before mutation and refreshes bookkeeping through the normal typed plan/execute path.

### Patch family

- `patch list` returns ordered patch summaries and active state.
- `patch show [NAME]` returns one patch; omission selects the active patch and fails if none is active.
- `patch create NAME` records complete patch metadata and selects it as the active draft without creating an empty commit; repeated `--scope GLOB` values declare ownership.
- `patch select NAME` selects an existing patch locally and performs no repository mutation.
- `patch edit [NAME]` updates purpose, upstream status, drop condition, kind, and ownership scope through explicit set/add/remove-scope arguments; it updates commit trailers and bookkeeping atomically.
- `patch refresh [NAME]` captures the index by default. `--all` captures the explicitly allowed working-tree set; repeated `--path PATHSPEC` captures only those paths. The modes are mutually exclusive.
- `patch refresh` validates ownership before invoking `stg refresh --patch ... --index`, regenerates source exports/ledger/manifest, refreshes bookkeeping, restores the fully applied series, and returns a typed result.
- `patch finish [NAME]` requires no remaining staged/unstaged changes, runs the full check, and clears active state. It does not create another commit.
- `patch delete NAME` is not in the first implementation; upstream-merged deletion remains rebase-owned. Add manual deletion only after a concrete workflow requires it.

### Operation family

- `operation status` returns the current operation and exact next actions.
- `operation continue` resumes the current operation after operator conflict resolution.
- `operation abort --yes` restores the recorded recovery point through supported Git/StGit commands, verifies the restored stack, and clears current-operation state.
- Completed operations remain represented by Git refs/reports/manifest history; forkctl does not build a generalized Jujutsu-style operation database.

### Check and integration family

- Forkctl's reusable pre-commit primitive is `check --staged`; it queries the index itself and never mutates.
- Pre-push integration calls ordinary `forkctl check`.
- Commit-message validation is added only if implementation evidence shows it catches a gap not already covered by generated messages and full check.
- The repository publishes one exact `fork` mise task whose mounted Usage spec exposes the complete forkctl grammar, plus optional Lefthook install/validate helpers. It does not duplicate command arguments across wrapper tasks.
- The mounted file task uses `dir = "{{cwd}}"`, `raw_args = true`, a shebang `exec forkctl "$@"` passthrough, exact task-local tools, mise's documented self-mount for completion, and no deprecated Tera argument functions or manual `usage_*` forwarding.
- The repository publishes a small Lefthook preset or documented local snippet using `mise run fork check [-s]`. It never overwrites consumer hook configuration or claims `core.hooksPath`.

## Manifest Requirements

- The new manifest uses `schema: 1` as the first and only contract.
- Downstream and upstream identities use full remote/branch/ref values.
- Base target provenance stores kind, selector, resolved commit, and annotated tag object when applicable.
- Patch records store name, kind, purpose, upstream status, drop condition, and complete ownership `scope` globs. Scope uses documented `globset` semantics: `*` stays within one path segment and `**` crosses directories.
- Source export paths are deterministic from one configured export directory and patch order/name; they are not copied into every patch record.
- One final tooling patch owns manifest, ledger, task/hook configuration, and generated exports.
- Rebase history is operation-level: one target and one exact recovery object own a list of dropped patch snapshots/commits.
- Required-text and allowed-base contracts remain declarative.
- Direct manual manifest edits are unsupported during normal operation; `init`, `patch edit`, `rebase`, and generated bookkeeping own writes.

## Hook and Process Requirements

- Every production subprocess clears only repository-local variables returned by `git rev-parse --local-env-vars` before operating by explicit working directory. Transport/auth variables such as `GIT_SSH_COMMAND` remain available.
- Tests launch forkctl with synthetic hook variables intact and prove nested clones and foreign repositories are isolated.
- Checks never stage or rewrite files.
- `patch refresh` may intentionally invoke StGit, whose pre-commit hook may modify the index; forkctl must consume the final post-hook index exactly as StGit does.
- Hook policy is configurable by arguments/configuration. Forkctl provides strict VSH defaults without making strictness unavoidable for other operators.

## JSON API Requirements

- `api schema` emits JSON Schema 2020-12 for invocation and response.
- `api call` reads one invocation and emits exactly one response; subprocess and human output never contaminate stdout.
- Requests use a discriminated command string matching CLI paths, such as `patch.refresh` and `operation.abort`, with command-specific typed `arguments`.
- Every mutation carries `mode: execute|plan`; read-only commands reject a non-execute mode as invalid rather than ignoring it.
- Success responses include protocol version, command, typed result, notices, and optional operation ID.
- Error responses include stable code, message, causes, structured details, retryability, and an optional suggested command.
- Domain error codes distinguish invalid request, invalid manifest, dirty worktree, active-patch requirement, staged-path violation, patch absence, operation conflict, verification failure, remote advancement, publication-policy rejection, subprocess failure, and internal failure.
- CLI adapters construct the same request types executed by `api call`; no command has a CLI-only handler.

## Verification Requirements

- Unit tests cover manifest/state validation, capture-mode parsing, deterministic export names, typed errors, and renderer snapshots.
- Architecture tests reject output/view dependencies and raw subprocess construction outside the process module.
- CLI/API parity tests cover every command and plan/execute mode.
- Hook-context tests preserve synthetic repository-local `GIT_*` variables at forkctl process entry.
- Real lifecycle tests cover create/select/edit/check/refresh/finish, lower-patch refresh, hook-modified index, ownership rejection, conflict recovery, abort, rebase, merged-patch history, fresh-clone history hydration, exact lease, branch-policy rejection, and atomic publication.
- History tests delete, retarget, and substitute recovery refs and require all cases to fail.
- Released-binary tests exercise the complete disposable lifecycle, not only `--version`.

## Fleet Completion

1. Release and independently verify the new forkctl patch version.
2. Rebuild each consumer stack from its upstream base with the new `init` plus explicit `patch create`/`refresh`; do not add a generic legacy import command.
3. Migrate Macterm to the new manifest, commands, hooks, and immutable catalog.
4. Migrate Ghostty directly from its old format to the new contract with no reader or migration code retained.
5. Migrate zmx's inherited downstream commits into named source/tooling patches.
6. Verify each repository from a fresh private clone before advancing.
7. Establish durable VSH branch-rule policy for the three approved forkctl repositories.
