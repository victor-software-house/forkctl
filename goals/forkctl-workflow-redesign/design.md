# Forkctl Workflow Redesign — Design

## Design Summary

Forkctl remains a small Rust policy layer over Git and StGit. The redesign replaces an overloaded `new --finish` flow with an explicit local active-patch workflow, adds hook-composable checks, normalizes operation recovery, binds history to exact recovery objects, and makes the CLI and JSON API one schema-driven command graph.

The key ergonomic rule is:

> The operator chooses intent once; forkctl owns every repetitive step after that.

## Command Tree

```text
forkctl [global options] <command>

Core
  init
  status
  verify
  rebase --onto REF [--dry-run]
  publish [--dry-run]
  instructions

Patch
  patch list
  patch show [NAME]
  patch create NAME --kind KIND --purpose TEXT
                    --upstream-status TEXT --drop-when TEXT
                    --scope GLOB...
  patch select NAME
  patch edit [NAME] [metadata/scope edits]
  patch check [NAME] [--staged | --all | --path PATHSPEC...] [--require-active]
  patch refresh [NAME] [--all | --path PATHSPEC...] [--dry-run]
  patch finish [NAME]

Operation
  operation status
  operation continue
  operation abort --yes [--dry-run]

API
  api schema
  api call
```

### Why core verbs remain top-level

The repository is forkctl's implicit primary resource, so `repo status`, `stack rebase`, and `stack publish` add ceremony without disambiguation. Mature CLIs such as Git, StGit, and mise keep their primary verbs top-level and namespace secondary object families. `patch`, `operation`, and `api` are genuine families and earn subcommands.

### Removed surface

- `new` and `new --finish`
- `--output`
- `--json`
- optional per-patch `--export`
- command-specific rebase continuation by repeating `rebase`
- manual pending-file knowledge in user instructions

No aliases remain.

## Global Options

| Option | Contract |
|:--|:--|
| `--manifest PATH` | Explicit manifest path; environment fallback remains `FORK_MANIFEST` |
| `--format pretty|json` | Complete human view or complete versioned envelope; default `pretty` |
| `--color auto|always|never` | Pretty output only; default `auto`; respects `NO_COLOR` |
| `--quiet`, `-q` | Suppress successful pretty output; errors remain; rejected with JSON because JSON is already a complete contract |
| `--version`, `-V` | Clap version output |
| `--help`, `-h` | Generated command-specific help |

`--dry-run` and `--yes` are not global because only some mutations can honor them. A global flag silently ignored by read-only commands is a bad API.

## Active Patch Model

### Repository bootstrap

When the manifest does not exist, `init` requires an explicit new-contract bootstrap:

```text
forkctl --manifest patches/fork.json init \
  --upstream-remote upstream \
  --upstream-url https://github.com/example/project.git \
  --upstream-ref refs/heads/main \
  --downstream-remote origin \
  --downstream-branch main \
  --base refs/heads/main \
  --ledger PATCHES.md \
  --exports patches/downstream \
  --bookkeeping-patch fork-tooling \
  --bookkeeping-path mise.toml \
  --bookkeeping-path lefthook.yml \
  --bookkeeping-path FORK.md
```

Forkctl resolves the base to typed historical provenance, requires `HEAD` to equal that commit, initializes StGit, creates the non-empty bookkeeping patch from the files explicitly declared or generated, and verifies the initial contract. It refuses downstream commits above the base. Existing fleets are rebuilt by replaying intended changes through `patch create`/`refresh`; forkctl has no legacy commit importer.

When the manifest exists, `init` is idempotent clone hydration: fetch exact historical evidence, reconstruct StGit metadata when absent, and verify.

### State

Active patch state is clone-local and Git-private:

```json
{
  "schema": 1,
  "patch": "reliable-busy-close",
  "mode": "draft",
  "metadata": {
    "kind": "source",
    "purpose": "...",
    "upstream_status": "not-submitted",
    "drop_when": "...",
    "scope": ["src/**", "tests/**"]
  }
}
```

- `mode: draft` means metadata exists locally but no StGit patch/manifest record exists yet.
- `mode: existing` selects a manifest patch for amendment; metadata is read from the manifest rather than duplicated.
- One active patch maximum removes routing ambiguity.
- `status`, `patch list`, and API results always expose active state.

### Create

`patch create` validates metadata and writes only the Git-private draft. It does not create an empty StGit commit, dirty the tracked manifest, or leave the repository structurally invalid.

If an active patch exists, creation fails with `active_patch_exists` and suggests `forkctl patch finish` or `forkctl patch select` after finishing.

### Select

`patch select NAME` requires the patch to exist and records only its name/mode. It can run with a clean or dirty worktree because it performs no capture. The next check/refresh determines whether changed paths belong.

### Check

`patch check` is read-only:

1. Resolve the explicit patch or active patch.
2. Read staged paths from the index by default.
3. Apply optional incoming path arguments as an intersection, never as replacement evidence.
4. Validate every staged path against patch scope globs.
5. Report staged, unstaged, untracked, owned, and rejected paths separately.
6. Return `staged_path_violation` when any staged path is outside scope.
7. With `--require-active`, return `active_patch_required` when no patch is active.

It never stages, refreshes, formats, or rewrites.

### Refresh

Capture modes are mutually exclusive:

| Invocation | Capture source |
|:--|:--|
| `patch refresh` | Current index; equivalent to the safe intent of `stg refresh --index` |
| `patch refresh --all` | All tracked and untracked changes matching the active patch's declared patterns; forkctl stages that explicit owned set before refresh |
| `patch refresh --path A --path B` | Only named Git pathspecs after ownership validation; forkctl stages that set |

Refresh pipeline:

1. Resolve active/explicit patch and capture mode.
2. Compute a typed plan: paths read, paths staged, StGit target, generated files, hooks expected, and recovery behavior.
3. On `--dry-run`, return the plan and stop.
4. Reject changed paths that the selected capture would ambiguously leave partly staged in the same file.
5. For a draft, create the StGit patch at the kind-appropriate insertion point with generated trailers.
6. For an existing patch, target it directly with StGit's `--patch` support.
7. Invoke StGit with repository-local Git environment removed and the explicit repository cwd.
8. Let StGit run the consumer's pre-commit hook; consume its final index exactly as StGit does.
9. Revalidate the resulting patch paths and trailers.
10. Regenerate deterministic source exports, manifest, and ledger.
11. Refresh the final bookkeeping patch through the same safe command factory.
12. Restore all patches applied and keep the selected patch active.
13. Return captured paths, commit IDs before/after, generated evidence, and notices.

A lower-patch refresh conflict creates a typed `patch_refresh` operation; it never leaves an unnamed `refresh-temp` without status/recovery guidance.

### Edit

Metadata flags are explicit:

```text
--kind source|tooling
--purpose TEXT
--upstream-status TEXT
--drop-when TEXT
--set-scope GLOB...       # replace full ownership set
--add-scope GLOB...
--remove-scope GLOB...
```

`--set-scope` conflicts with add/remove. Empty ownership is invalid. Scope uses `globset` semantics (`*` does not cross `/`; `**` does), while `patch refresh --path` remains a one-shot Git pathspec selector. Editing metadata updates the manifest, patch commit trailers, deterministic export presence/name when kind changes, and bookkeeping in one journaled operation.

### Finish

`patch finish` requires:

- active patch exists and is materialized;
- index and worktree have no remaining changes;
- all patches applied;
- complete `verify` passes.

It clears active state and returns verification data. It does not amend commits again.

## Manifest

### Shape

```json
{
  "schema": 1,
  "downstream": {
    "remote": "origin",
    "branch": "main",
    "recovery_tag_prefix": "forkctl/recovery"
  },
  "upstream": {
    "remote": "upstream",
    "url": "https://github.com/example/project.git",
    "fetch_ref": "refs/heads/main"
  },
  "base": {
    "target": {
      "kind": "branch",
      "selector": "refs/heads/main",
      "commit": "<sha>",
      "tag_object": null
    },
    "canonical": "<sha>",
    "stack": "<sha>"
  },
  "documents": {
    "ledger": "PATCHES.md",
    "exports": "patches/downstream"
  },
  "bookkeeping_patch": "fork-tooling",
  "patches": [
    {
      "name": "downstream-change",
      "kind": "source",
      "purpose": "...",
      "upstream_status": "not-submitted",
      "drop_when": "...",
      "scope": ["src/**", "tests/**"]
    }
  ],
  "history": [],
  "contracts": {
    "allow_base": [],
    "required_text": [
      {"path": "FORK.md", "contains": "forkctl verify"}
    ]
  }
}
```

### Deterministic exports

Every source patch exports to:

```text
<documents.exports>/<zero-padded-order>-<patch-name>.patch
```

The width is at least four digits and expands when required. Tooling patches have no export. A kind/order/name change atomically renames/removes generated exports inside bookkeeping.

This removes duplicated export paths, guarantees source reconstruction coverage, and keeps one configured export directory.

### History

History is rebase-operation-level rather than one flat record per dropped patch:

```json
{
  "kind": "rebase",
  "target": {"kind": "branch", "selector": "refs/heads/main", "commit": "<new-base>"},
  "recovery": {
    "tag": "forkctl/recovery/<id>",
    "tag_object": "<annotated-tag-object>",
    "old_base": "<sha>",
    "old_tip": "<sha>"
  },
  "dropped": [
    {"patch": {"name": "old-change", "kind": "source", "purpose": "...", "upstream_status": "...", "drop_when": "...", "scope": ["..."]}, "commit": "<pre-rebase-commit>"}
  ]
}
```

Fresh-clone initialization fetches only exact history tag refs. Verification proves:

- tag exists as the recorded annotated object;
- tag peels to `old_tip`;
- `old_base..old_tip` contains the recorded commit at the expected series position;
- patch paths/trailers match the snapshot;
- target evidence exists;
- deleted, substituted, lightweight, or retargeted tags fail.

## Operation Journal

Current operation state lives at `.git/forkctl/operation.json` and is the only in-flight operation. It includes:

- schema and stable operation ID;
- kind and phase;
- started time;
- expected downstream lease;
- old base/tip and ordered patch names/commits;
- exact annotated recovery tag object;
- target where applicable;
- new base/tip where reached;
- generated report object where reached;
- conflict/next-action details.

### Status

`operation status` is read-only and available even when the manifest patch is popped or the worktree conflicts. It loads the Git-private manifest snapshot when necessary.

### Continue

`operation continue` dispatches by kind/phase. It verifies every already-completed phase before advancing; it cannot skip stages.

### Abort

`operation abort --dry-run` reports the exact reset/undo actions and paths that would be discarded. Execution requires `--yes` when stdin is not an interactive terminal.

Abort delegates to supported StGit undo/recovery behavior, restores the recorded old branch/stack/manifest state, verifies it, and only then clears the journal. If restoration fails, the journal remains and returns a typed next action.

Completed operations are not copied into a generalized operation database. Git refs, reports, and manifest history are the durable audit record.

## Hook and Tooling Integration

### Core commands, not hook ownership

Forkctl exposes reusable commands; hook managers call them:

```text
forkctl patch check --staged --require-active
forkctl verify --quiet
```

Forkctl does not set `core.hooksPath`, install scripts into `.git/hooks`, or require Lefthook.

### VSH Lefthook default

```yaml
pre-commit:
  commands:
    forkctl-staged:
      run: mise run fork:check:staged

pre-push:
  commands:
    forkctl-verify:
      run: mise run fork:verify -- --quiet
```

The exact remote mise catalog provides:

```text
fork:status
fork:verify
fork:patch:create
fork:patch:check
fork:patch:refresh
fork:patch:finish
fork:rebase
fork:publish
```

A small documented Lefthook snippet is preferable to a mandatory remote config because the mise catalog is already the single exact tool-version boundary and local hooks remain fully composable. If repeated fleet use proves a remote preset valuable, add one independently without changing the forkctl command contract.

### Hook environment

`process.rs` owns the only subprocess constructor. It obtains the repository-local variable names from `git rev-parse --local-env-vars` once per invocation context and removes them for every child command before applying an explicit cwd. It preserves nonlocal Git variables such as SSH transport, tracing, and credential configuration.

Architecture tests reject `Command::new` outside `process.rs` in production modules.

## CLI/API Mapping

### Request envelope

```json
{
  "protocol_version": 1,
  "manifest": "patches/downstream/fork.json",
  "mode": "execute",
  "request": {
    "command": "patch.refresh",
    "arguments": {
      "patch": null,
      "capture": {"source": "staged"}
    }
  }
}
```

- `command` is the dotted CLI path.
- `arguments` is a Schemars-derived discriminated variant, not an untyped map internally.
- `mode` is `execute` or `plan` and is accepted only by mutating requests.
- CLI `--dry-run` maps to `plan`.

### Success envelope

```json
{
  "status": "success",
  "protocol_version": 1,
  "command": "patch.refresh",
  "operation_id": null,
  "result": {
    "patch": "reliable-busy-close",
    "captured_paths": ["src/model.rs"],
    "old_commit": "<sha>",
    "new_commit": "<sha>",
    "generated": ["PATCHES.md", "patches/downstream/0001-reliable-busy-close.patch"],
    "verification": {"ok": true}
  },
  "notices": []
}
```

### Error envelope

```json
{
  "status": "error",
  "protocol_version": 1,
  "command": "patch.refresh",
  "error": {
    "code": "staged_path_violation",
    "message": "2 staged paths are outside patch reliable-busy-close",
    "causes": [],
    "details": {"paths": ["README.md", "mise.toml"]},
    "retryable": false,
    "suggested_command": "forkctl patch edit --add-scope README.md --add-scope mise.toml"
  }
}
```

JSON mode writes exactly one envelope to stdout and nothing to stderr. Pretty errors use stderr. Subprocess output is always captured and incorporated into typed details/causes.

### Error codes

| Code | Meaning |
|:--|:--|
| `invalid_request` | CLI/API argument or mode is invalid |
| `unsupported_protocol` | API version is unsupported |
| `repository_not_found` | No repository root can be resolved |
| `manifest_invalid` | Manifest parse or structural validation failed |
| `dirty_worktree` | A clean-only operation found changes |
| `active_patch_required` | Patch command requires explicit active/target patch |
| `active_patch_exists` | New draft conflicts with an existing active patch |
| `patch_not_found` | Named patch is absent |
| `staged_path_violation` | Staged/captured paths exceed ownership |
| `operation_in_progress` | Another journaled mutation is active |
| `operation_conflict` | Operator resolution is required before continue |
| `verification_failed` | Structural/audit/reconstruction contract failed |
| `remote_advanced` | Exact downstream lease no longer matches |
| `publication_rejected` | Remote policy rejected the atomic push |
| `subprocess_failed` | Git/StGit command failed outside a classified condition |
| `internal_error` | Forkctl invariant or unexpected implementation failure |

Error construction is typed at domain boundaries; no string-prefix classification.

## Module Boundaries

```text
src/
  main.rs                 thin process entry and exit mapping
  cli.rs                  Clap-only adapter types → protocol requests
  protocol.rs             request/result/error/schema contract
  manifest.rs             tracked policy types and validation
  app/
    mod.rs                shared repository facade
    init.rs
    status.rs
    verify.rs
    patch.rs              create/select/show/list orchestration
    patch_refresh.rs      capture pipeline
    operation.rs          status/continue/abort dispatch
    rebase.rs
    publish.rs
  state/
    active_patch.rs       Git-private active state
    operation.rs          Git-private journal
  process.rs              sole child-process factory and Git-local env isolation
  evidence/
    export.rs
    ledger.rs
    report.rs
  view.rs                 sole pretty renderer
```

Domain modules do not import Clap, Anstream, Anstyle, Comfy Table, or terminal state.

## Publication Policy

Forkctl's generic contract ends at the atomic exact-lease `git push`. A protected-branch rejection returns `publication_rejected` with remote stderr and no fallback.

VSH separately configures a durable, narrow exception or bypass for approved Macterm, Ghostty, and zmx default branches. Forkctl never acquires organization administration credentials or temporarily edits rulesets.
