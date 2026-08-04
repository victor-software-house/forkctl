# Forkctl Explicit Patch Workflow and Protocol

## Objective

Release forkctl 0.0.6 as an incompatible, from-scratch workflow for maintaining audited Git/StGit downstream patch stacks. The operator names patch intent once; forkctl then owns staged capture, StGit targeting, generated exports/ledger/manifest, operation recovery, audit evidence, exact-lease publication, typed CLI/API output, and integration primitives.

Macterm, Ghostty, and zmx must subsequently use this contract exclusively. No prior forkctl manifest, command, task, API, local state, alias, or migration path survives.

## Why

Forkctl 0.0.5 proves the audited lifecycle but leaves normal patch work too manual: explicit metadata exists, yet operators still choreograph staging, lower-patch refresh, bookkeeping restoration, exports, and finish. Its consumer task catalog duplicates a shallow subset of the CLI, provides no mounted grammar/completion, and does not expose a deliberately scoped pre-commit check. PR #1 identifies valid audit defects but masks production hook-environment contamination in tests and still does not bind historical commits to exact recovery tags.

The new contract removes those causes instead of adding guidance around them.

## Non-Negotiable Invariants

- Delegate all repository and patch mechanics to installed [Git](https://git-scm.com/) and [StGit](https://stacked-git.github.io/).
- Never infer patch intent or ownership from filenames.
- Never stash operator changes.
- Keep domain handlers independent from Clap, JSON, terminal state, and renderers.
- Execute CLI and local API through the same typed handlers.
- Make checks read-only; only explicit mutations stage or rewrite.
- Bind every dropped historical patch to the exact immutable annotated recovery object preserving its old stack.
- Publish branch plus recovery tag atomically under one exact lease with no fallback.
- Never administer GitHub rulesets or require one hook manager in generic forkctl.
- Provide no compatibility or migration implementation for earlier formats.

## Product Model

### Active patch

One clone has at most one explicit active patch in Git-private typed state. `patch create` records metadata-only draft intent; `patch select` chooses an existing patch. Neither creates or rewrites commits.

The operator edits and stages normally. `patch refresh` captures staged content by default, with explicit `--all` and repeatable `--path` alternatives. Forkctl validates scope, delegates targeted refresh to StGit, accepts hook-modified index state, regenerates all evidence, refreshes bookkeeping, restores the applied series, and keeps the patch active. `patch finish` requires no remaining changes, runs full check, and clears active state.

### Validation

One `check` command avoids competing strict-sounding verbs:

```text
forkctl check                    complete clean-repository audit
forkctl check -s|--staged       staged index against active patch
forkctl check -s -p PATCH       staged index against explicit patch
```

An empty staged index succeeds. A nonempty index without a patch target fails. Hook managers call `check -s`; pre-push/release/rebase/publication use full `check`.

### CLI

```text
init · status · check · rebase · publish · instructions · completion
patch list|show|create|select|edit|refresh|finish
contract edit
operation status|continue|abort
api schema|call
```

Leaf parameters follow justpath's composable design: orthogonal subject, metadata/scope, capture, execution, and presentation groups; repeatable values; visible defaults; and deliberate collision-audited short forms. `-n` consistently means dry-run; `-y` means confirmed destructive execution.

Help is a clean colored, width-aware panel view generated entirely from the Clap command graph through forkctl's existing Anstyle/Comfy Table renderer. There is no second parameter specification.

### Completion

`completion SHELL` supports bash, elvish, fish, Nushell, PowerShell, and zsh. Completion includes commands, short/long flags, enum choices, files/directories, local Git remotes/refs, live patch names, and current operation values. Candidate lookup is local, read-only, bounded, and silent outside a valid repository.

### JSON API

The local one-request stdin/stdout protocol uses `protocol_version: 1`, dotted command names, command-specific typed arguments, and `mode: execute|plan`. Success, plan, notice, error, and error-detail types are schema-derived. `api schema` selects manifest, invocation, response, active-state, operation, or bundle JSON Schema 2020-12 documents. JSON stdout is exactly one document and stderr is empty.

### Mise and hooks

One immutable remote `fork` file task is a transparent `raw_args` proxy using `dir = "{{cwd}}"`, exact task-local forkctl/Rust/StGit tools, and `exec forkctl "$@"`. Forkctl therefore owns validation and colored help unchanged. For shell completion, the task uses mise's documented self-mount to request `--usage-spec=fork`, generated through `usage-lib` directly from Clap, without duplicated wrapper arguments or deprecated Tera argument functions.

VSH's optional Lefthook composition is:

```yaml
pre-commit:
  commands:
    forkctl-staged:
      run: mise run fork check -s
pre-push:
  commands:
    forkctl-check:
      run: mise run fork check -q
```

Another hook manager calls the same commands. Forkctl does not alter `core.hooksPath` or consumer configuration.

### Operation recovery and history

One Git-private typed current-operation journal records exact phases, old/new stack evidence, lease, recovery object, report object, conflicts, and next actions. `operation status`, `continue`, and `abort` expose it. Abort plans its discarded state, requires confirmation for execution, restores through supported Git/StGit operations, verifies restoration, and only then clears state.

Completed state remains in Git refs, reports, and operation-level manifest history. A rebase history record stores target, exact recovery tag object, old base/tip, and all dropped pre-rebase patch snapshots/commits. Fresh clones fetch only named evidence refs and reject deletion, retargeting, substitution, lightweight tags, or wrong ancestry.

## Explicit Exclusions

- Automatic patch routing or intent inference.
- Mandatory Lefthook, native hook installation, or consumer hook-file mutation.
- Generic legacy commit import.
- A second Jujutsu-like operation database or generalized concurrency model.
- GitHub-specific administrative credentials or branch-ruleset changes.
- Optional per-patch export paths.
- Manual deletion command until a real workflow requires it.
- Compatibility readers, aliases, migration code, and fallback behavior.

## Delivery Phases

1. Freeze CLI/API/manifest/state/help/completion/mise contracts.
2. Rebuild typed protocol, Clap adapter, schemas, and renderer/help/completion surfaces.
3. Make production subprocesses hook-environment-safe.
4. Implement active state and parameterized check.
5. Implement create/select/edit/refresh/finish and deterministic exports.
6. Implement current-operation recovery, rebase, exact history binding, and abort.
7. Implement atomic publication plus mounted mise/Lefthook integration.
8. Independently review, publish, and prove forkctl 0.0.6.
9. Rebuild Macterm, Ghostty, then zmx sequentially onto the new contract.
10. Provision durable narrow VSH branch policy for those approved repositories.

## Acceptance Gates

- Every CLI command/flag/short/default/help group has snapshots and direct/API parity.
- Direct and mise-mounted grammar/completion cover all supported shells and dynamic patch/ref candidates.
- Every request, plan, result, notice, error/detail, manifest, active-state, and operation type appears in generated schemas.
- Hook-context tests enter forkctl with repository-local `GIT_*` variables intact and prove nested repositories are isolated.
- Real lifecycle tests cover all capture modes, hook-modified staging, lower-patch conflicts, continue/abort, rebase, exact history hydration, stale lease, protected-branch rejection, and atomic publication.
- Released GitHub and crates.io binaries—not only local builds—pass the complete disposable lifecycle.
- PR #1 is closed unmerged and superseded by the layered implementation PR.
- Each fleet repository has no old contract files and passes fresh-clone `forkctl init` plus `forkctl check` before the next migration starts.
- Repository and vault records finish clean, synchronized, and accurate.

## Source Package

- `research.md` — primary-source and local-runtime evidence.
- `decisions.md` — binding intent and approved choices.
- `facts.md` — testable outcomes.
- `requirements.md` — normative behavior.
- `cli.md` — complete command/parameter/usage reference.
- `api.md` — complete request/result/error/schema reference.
- `mise.md` — complete task macro, completion, and hook integration.
- `design.md` — architecture and state model.
- `plan.md` — implementation and verification sequence.
