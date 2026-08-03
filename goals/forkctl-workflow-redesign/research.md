# Forkctl Workflow Redesign — Research

## Scope

This research grounds an incompatible, from-scratch forkctl contract. The target remains a policy CLI over real [Git](https://git-scm.com/) and [StGit](https://stacked-git.github.io/); it does not reproduce their storage, patch, rebase, or conflict mechanics.

## Primary Evidence

| Source | Proven pattern | Forkctl consequence |
|:--|:--|:--|
| [Git hooks](https://git-scm.com/docs/githooks) | Git exports repository-local variables such as `GIT_DIR` and `GIT_WORK_TREE`; commands operating in another repository must clear the variables reported by `git rev-parse --local-env-vars` | Every production Git/StGit subprocess must run through one environment-safe command factory; tests must invoke forkctl with synthetic hook variables rather than sanitizing them before forkctl starts |
| [StGit refresh](https://stacked-git.github.io/man/stg-refresh/) | `stg refresh --index` captures the index, `--patch` targets a non-top patch, and path arguments constrain capture; refreshing another patch may leave a temporary patch on conflict | Staged capture is the safe default; `patch refresh` must expose explicit all/path alternatives and journal conflict recovery instead of hiding StGit state |
| [StGit 2.6.1 refresh source](https://github.com/stacked-git/stgit/blob/v2.6.1/src/cmd/refresh.rs) | Refresh invokes the configured pre-commit hook before finalizing the patch and re-reads the index if the hook changed it | Forkctl commands and external hook managers can compose naturally; forkctl does not need to own `core.hooksPath` |
| [Lefthook run configuration](https://lefthook.dev/configuration/run/) | Lefthook exposes staged, pushed, tracked, custom, and hook-argument inputs; commands remain ordinary shell-visible CLI calls | Forkctl should expose validation commands that accept normal path arguments/stdin and can be called by Lefthook, native Git hooks, mise tasks, or another manager |
| [Lefthook remotes](https://lefthook.dev/configuration/remotes/) | Versioned remote configs merge with local and local-override configuration | A reusable VSH preset is viable, but the core contract must remain hook-manager-neutral and consumer-local configuration must retain priority |
| [Jujutsu operation log](https://docs.jj-vcs.dev/latest/operation-log/) | Mutating operations are named, inspectable, and restorable; users can see why repository state changed | Forkctl needs typed current-operation status plus bounded continue/abort, but not a second VCS or a speculative full snapshot database |
| [GitHub CLI formatting](https://cli.github.com/manual/gh_help_formatting) | Human output is the default; structured JSON is explicit and formatting happens after typed data exists | CLI pretty and JSON views consume the same typed result; the local API uses the same command handlers and schema-derived request/result types |
| [mise CLI source at `4329507`](https://github.com/jdx/mise/blob/432950772aeb7f45420b514c6202d7b8880b1744/src/cli/mod.rs) | Common verbs remain top-level, object families receive subcommands, and global flags are orthogonal | Keep `init`, `status`, `verify`, `rebase`, and `publish` top-level; use namespaces only for patch, operation, check, and API families |
| [Current forkctl `229247d`](https://github.com/victor-software-house/forkctl/tree/229247d6f6789a21201f1778439caa0b9b1ba102) | Typed protocol/view separation, exact leases, generated evidence, and fail-closed verification already work | Preserve proven architecture, not command or manifest compatibility |
| [PR #1](https://github.com/victor-software-house/forkctl/pull/1) | Correctly identifies pre-rebase history and JSON error classification, but test-only environment sanitization masks the production hook failure and history remains unbound to one recovery tag | Reimplement useful findings in clean layers; do not merge the PR |

## Local Verification

### StGit hook behavior

A disposable repository with executable `pre-commit`, `commit-msg`, and `post-commit` probes established:

| Operation | Observed hook |
|:--|:--|
| `stg new -m ...` | `commit-msg` |
| `stg refresh --index` | `pre-commit` |
| Neither operation | `post-commit` |

This confirms that consumer hook integration should call ordinary forkctl checks from the existing hook pipeline. Forkctl must not install an exclusive native hook directory.

### PR #1 hook failure

PR #1 passed 44 tests and `cargo publish --dry-run --locked`. A fresh disposable Macterm clone invoked with synthetic hook-style `GIT_DIR` and `GIT_WORK_TREE` failed during forkctl's nested reconstruction clone:

```text
forkctl · error
git clone --shared --quiet --no-checkout ...:
fatal: working tree '.../macterm' already exists.
```

The PR removes `GIT_*` before launching forkctl in tests; production `src/process.rs` still inherits them. The new design must fix production command execution and add an end-to-end regression that leaves the inherited variables intact.

## Existing Fleet State

| Repository | State before redesign | Required clean cut |
|:--|:--|:--|
| Macterm | Current 0.0.5 manifest and four-patch stack | Replace manifest/tasks/hooks with the new contract after release proof |
| Ghostty | Superseded 0.0.2 manifest and commit-pinned catalog | Recreate its three source patches plus bookkeeping in the new contract; no reader for the old file |
| zmx | Ordinary private commits, no forkctl manifest | Classify inherited downstream source/tooling commits and initialize the new contract from scratch |

## Findings

### Keep

- Git and StGit as the only repository/patch mechanics.
- Typed handlers shared by Clap and the local JSON API.
- Central semantic pretty renderer and schema-derived JSON.
- Exact remote lease, atomic branch-plus-recovery publication, generated ledger/exports, target provenance, and report object binding.
- Clean-worktree requirements for repository-wide verify, rebase, and publish.

### Rewrite

- Top-level `new --finish` into a complete `patch` command family.
- Git-private single pending file into a typed current-operation journal with explicit status/continue/abort.
- Optional per-patch export paths into deterministic generated exports for every source patch.
- Ad hoc process spawning into one hook-safe command factory.
- Unbound flat dropped-patch history into operation-level history bound to exact recovery-tag objects.
- Generic `operation_failed` strings into stable typed domain error codes and structured hints.

### Add

- Explicit active patch state per clone.
- Staged-path validation and capture commands usable directly or from any hook manager.
- Patch metadata/path editing without direct manifest edits.
- Dry-run plans for every mutation.
- First-class mise tasks and a concise Lefthook preset that call normal forkctl commands.
- Protected-branch readiness documentation and external VSH ruleset policy; no privileged GitHub mutation in generic forkctl.

### Reject

- Automatic intent inference or path-based patch selection.
- Mandatory Lefthook ownership or `core.hooksPath` replacement.
- Automatic staging during ordinary checks.
- A second VCS-style operation database, lock-free concurrency, or generalized undo history.
- Compatibility readers, aliases, migration commands, or fallback support for any previous manifest/API/CLI shape.
- GitHub-specific ruleset administration in forkctl.
