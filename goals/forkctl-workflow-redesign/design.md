# Forkctl Workflow Redesign — Design

## Design Summary

Forkctl remains a small Rust policy layer over Git and StGit. The redesign replaces an overloaded `new --finish` flow with an explicit local active-patch workflow, adds hook-composable checks, normalizes operation recovery, binds history to exact recovery objects, and makes the CLI and JSON API one schema-driven command graph.

The key ergonomic rule is:

> The operator chooses intent once; forkctl owns every repetitive step after that.

Companion contracts are normative, not appendices:

- `cli.md` — every command, option, short form, default, conflict, and example;
- `api.md` — every request, plan, result, notice, error/detail, envelope, and JSON Schema surface;
- `mise.md` — exact mounted task, Usage macros, completion, cwd, tools, and Lefthook composition;
- `requirements.md` — acceptance invariants;
- `plan.md` — layered implementation and verification.

## Command Tree

```text
forkctl [global options] <command>

Core
  init [bootstrap options] [-n|--dry-run]
  status
  check [-s|--staged] [-p|--patch NAME]
  rebase -o|--onto REF [-n|--dry-run]
  publish [-n|--dry-run]
  instructions
  completion SHELL

Patch
  patch list
  patch show [NAME]
  patch create NAME -k|--kind KIND -p|--purpose TEXT
                    -u|--upstream-status TEXT -d|--drop-when TEXT
                    -s|--scope GLOB... [-n|--dry-run]
  patch select NAME [-n|--dry-run]
  patch edit [NAME] [metadata/scope edits] [-n|--dry-run]
  patch refresh [NAME] [-s|--staged | -a|--all | -p|--path PATHSPEC...]
                       [-n|--dry-run]
  patch finish [NAME] [-n|--dry-run]

Contract
  contract edit [--clear] [-a|--allow-base GLOB]... [-r|--required-text PATH=TEXT]...

Operation
  operation status
  operation continue [-n|--dry-run]
  operation abort -y|--yes [-n|--dry-run]

API
  api schema
  api call
```

### Why core verbs remain top-level

The repository is forkctl's implicit primary resource, so `repo status`, `stack rebase`, and `stack publish` add ceremony without disambiguation. Mature CLIs such as Git, StGit, and mise keep their primary verbs top-level and namespace secondary object families. `patch`, `contract`, `operation`, and `api` are genuine families and earn subcommands.

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
| `-m`, `--manifest PATH` | Explicit manifest path; environment fallback remains `FORK_MANIFEST` |
| `-f`, `--format pretty|json` | Complete human view or complete versioned envelope; default `pretty` |
| `-c`, `--color auto|always|never` | Pretty output only; default `auto`; respects `NO_COLOR` |
| `-q`, `--quiet` | Suppress successful pretty output; errors remain; rejected with JSON because JSON is already a complete contract |
| `-V`, `--version` | Clap version output |
| `-h`, `--help` | Generated command-specific help |

`-h`/`--help` and `-V`/`--version` remain Clap-reserved. Global `-m`, `-f`, `-c`, and `-q` are unavailable to subcommands. `-n` consistently means dry-run on mutations and `-y` consistently means confirmed destructive execution. Command-local letters may repeat only across disjoint subcommands; composite bootstrap identities such as `--upstream-remote` remain long-only when no abbreviation is unambiguous.

### Short-flag matrix

| Command family | Short forms |
|:--|:--|
| `init` | `-u` upstream URL · `-b` base · `-l` ledger · `-e` exports · `-k` bookkeeping patch · `-p` bookkeeping path · `-a` allow base · `-r` required text · `-n` dry-run; remote/ref/branch identities remain long-only |
| `rebase` | `-o` onto · `-n` dry-run |
| `publish` | `-n` dry-run |
| `check` | `-s` staged · `-p` patch; no flag means complete repository check |
| `patch create` | `-k` kind · `-p` purpose · `-u` upstream status · `-d` drop condition · `-s` scope · `-n` dry-run |
| `patch edit` | same metadata shorts · `-s` set scope · `-a` add scope · `-r` remove scope · `-n` dry-run |
| `patch refresh` | `-s` staged · `-a` all · `-p` path · `-n` dry-run |
| `patch select` / `finish` | `-n` dry-run |
| `operation continue` | `-n` dry-run |
| `operation abort` | `-y` yes · `-n` dry-run |
| `api schema` | `-k` schema kind |

Short aliases are snapshot-tested as part of the public grammar; adding a collision is a test failure.

### Parameter composition

Each leaf command composes a small set of concern-specific Clap `Args` structs rather than one flat bag or extra mode subcommands:

| Group | Examples | Rule |
|:--|:--|:--|
| Subject | optional patch `NAME`, `--onto REF` | Identifies the object; omission may mean active patch only when help says so |
| Metadata/scope | kind, purpose, status, drop condition, repeatable scope | Persistent declarative intent; no capture behavior |
| Capture | staged, all, repeatable pathspec | Mutually exclusive source selection; staged is default |
| Execution | dry-run, yes | Side-effect policy only |
| Presentation | manifest, format, color, quiet | Global and handler-independent |

Repeatable flags behave like justpath's includes/excludes: one value per occurrence, preserved in user order, represented as arrays in the JSON API. Convenience flags are transparent compositions—`--all` selects the complete owned worktree set; it does not enter a different handler.

### Help rendering

The visual target is the supplied justpath help: compact colored usage, a one-sentence description, and scan-friendly bordered panels with aligned flag/value/description columns.

Forkctl keeps Clap as the sole command/parameter model:

1. Derive the complete `clap::Command` graph from adapter types.
2. Assign declarative `help_heading` values such as `Patch metadata`, `Capture`, `Execution`, and `Output`.
3. A centralized `HelpRenderer` walks command, subcommand, argument, possible-value, default, and heading metadata.
4. It renders through the existing Anstyle semantic palette and Comfy Table width logic used by normal views; Comfy Table's `custom_styling` feature makes width calculation ANSI-aware without introducing another visual API.
5. Clap's own semantic styles render parse errors and usage consistently.
6. Non-TTY/`--color never` output retains the same layout without ANSI bytes.

The option panel columns are: short form, long form, metavar/choices, description plus dim default. Commands receive their own panel with name and one-line purpose. Positional arguments receive an arguments panel. Examples are styled strings attached to the Clap command and rendered from the same metadata path. The centralized theme adds semantic `command`, `option`, and `value` slots: headings/commands use the established cyan family, options use green, values/choices use yellow, descriptions use the terminal default, and defaults use muted bright black.

`clap-help` is rejected despite its mature width-aware tables because it explicitly lacks subcommand support and would introduce Termimad as a second visual system. No help text duplicates parameter spelling, defaults, or choices outside Clap declarations.

Snapshot coverage fixes the contract at narrow, standard, and wide terminal widths in colored and plain modes, including every subcommand and short alias.

### Completion

The same Clap graph owns native and mise completion:

- Pin `clap_complete` with its dynamic engine and attach `ArgValueCompleter` functions to domain values.
- `completion SHELL` emits self-correcting registration for bash, elvish, fish, PowerShell, and zsh through Clap's environment completer.
- Nushell receives an equivalent external completer backed by a hidden shell-neutral completion endpoint using `clap_complete::engine::complete`; it is not limited to static generated externs.
- `usage-lib` converts `clap::Command` directly to Usage KDL for mise. Forkctl programmatically overlays Usage `complete` entries for dynamic domain values rather than hand-writing command grammar.

Completion sources:

| Argument | Candidates |
|:--|:--|
| command/subcommand/flag | Clap graph |
| enum values | Clap `ValueEnum` |
| manifest/file/directory/pathspec | Clap value hints and filesystem/Git index |
| patch name | Manifest patches plus active draft |
| upstream/downstream remote | Local `git remote` names |
| rebase target | Local full branch/tag refs and commit completion; never network |
| operation ID/phase | Current Git-private operation |
| schema kind/shell | static enum values |

Candidate generation is read-only, local, bounded, and silent on unavailable repository state. Registration scripts are regenerated on version upgrades because the dynamic shell protocol is binary-coupled. Completion snapshots plus shell integration tests cover every supported shell, repeated options, equals syntax, stacked short flags, values with spaces, and live patch/ref candidates.

Proposed `forkctl patch refresh --help` structure:

<!-- box:begin
match-width: true
diagram:
  - text: "Usage: forkctl patch refresh [OPTIONS] [NAME]"
  - gap: 1
  - text: "Capture staged or explicitly selected changes into a declared patch."
  - gap: 1
  - box: Arguments
    width: match
    body:
      - table:
          rows:
            - ["", "NAME", "", "Patch name; defaults to the active patch"]
  - box: Capture
    width: match
    body:
      - table:
          rows:
            - ["-s", "--staged", "", "Capture the index [default]"]
            - ["-a", "--all", "", "Stage and capture all owned changes"]
            - ["-p", "--path", "PATHSPEC", "Capture a pathspec; repeatable"]
  - box: Execution
    width: match
    body:
      - table:
          rows:
            - ["-n", "--dry-run", "", "Validate and show the mutation plan"]
  - box: Output
    width: match
    body:
      - table:
          rows:
            - ["-m", "--manifest", "PATH", "Manifest [env: FORK_MANIFEST]"]
            - ["-f", "--format", "[pretty|json]", "Output format [default: pretty]"]
            - ["-c", "--color", "[auto|always|never]", "Color policy [default: auto]"]
            - ["-q", "--quiet", "", "Suppress successful pretty output"]
            - ["-h", "--help", "", "Show this help"]
-->
```text
Usage: forkctl patch refresh [OPTIONS] [NAME]

Capture staged or explicitly selected changes into a declared patch.

┌─ Arguments ────────────────────────────────────────────────────────────┐
│   NAME    Patch name; defaults to the active patch                     │
└────────────────────────────────────────────────────────────────────────┘
┌─ Capture ──────────────────────────────────────────────────────────────┐
│ -s  --staged            Capture the index [default]                    │
│ -a  --all               Stage and capture all owned changes            │
│ -p  --path    PATHSPEC  Capture a pathspec; repeatable                 │
└────────────────────────────────────────────────────────────────────────┘
┌─ Execution ────────────────────────────────────────────────────────────┐
│ -n  --dry-run    Validate and show the mutation plan                   │
└────────────────────────────────────────────────────────────────────────┘
┌─ Output ───────────────────────────────────────────────────────────────┐
│ -m  --manifest  PATH                 Manifest [env: FORK_MANIFEST]     │
│ -f  --format    [pretty|json]        Output format [default: pretty]   │
│ -c  --color     [auto|always|never]  Color policy [default: auto]      │
│ -q  --quiet                          Suppress successful pretty output │
│ -h  --help                           Show this help                    │
└────────────────────────────────────────────────────────────────────────┘
```
<!-- box:end -->

The actual renderer wraps descriptions and rebalances columns to terminal width; the mock fixes information hierarchy, not literal spacing.

`--dry-run` and `--yes` are not global because read-only commands cannot honor them. A global flag silently ignored by commands is a bad API.

## Active Patch Model

### Repository bootstrap

When the manifest does not exist, `init` requires an explicit new-contract bootstrap:

```text
forkctl --manifest patches/fork.yaml init \
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
  --bookkeeping-path FORK.md \
  --allow-base 'vendor/**' \
  --required-text 'FORK.md=forkctl check'
```

Forkctl resolves the base to typed historical provenance, requires `HEAD` to equal that commit, validates repeatable allowed-base globs and required `PATH=TEXT` assertions, initializes StGit, creates the non-empty bookkeeping patch from the files explicitly declared or generated, and verifies the initial contract. It refuses downstream commits above the base. Existing fleets are rebuilt by replaying intended changes through `patch create`/`refresh`; forkctl has no legacy commit importer.

When the manifest exists, `init` is idempotent clone hydration: fetch exact historical evidence, reconstruct StGit metadata when absent, and run the full check.

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

`check` is one read-only command with one scope parameter, not two strict-sounding commands or an optional mode subcommand:

```text
forkctl check                    # complete clean-repository audit
forkctl check -s                 # staged index against active patch
forkctl check -s -p PATCH        # staged index against an explicit patch
```

Repository mode is the default. It requires no active draft, requires a clean worktree and completed operation, and runs the complete stack, provenance, trailer, scope, ledger, export, history, report, and reconstruction gate.

Staged mode:

1. Resolve `-p`/`--patch` or the active patch.
2. Read staged paths from the index.
3. Succeed immediately when the index is empty.
4. If paths exist and no patch resolves, return `active_patch_required`.
5. Validate every staged path against patch scope globs.
6. Report staged, unstaged, untracked, owned, and rejected paths separately.
7. Return `staged_scope_violation` when any staged path is outside scope.

It never stages, refreshes, formats, or rewrites. Hook managers call `check -s`; pre-push/release automation calls plain `check`.

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
- complete `check` passes.

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
      {"path": "FORK.md", "contains": "forkctl check"}
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
forkctl check --staged
forkctl check --quiet
```

Forkctl does not set `core.hooksPath`, install scripts into `.git/hooks`, or require Lefthook.

### VSH Lefthook default

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

The exact remote catalog exposes one canonical task:

```text
mise run fork <complete forkctl grammar>
```

Its Usage specification mounts `mise run fork -- --usage-spec=fork`, generated by `usage-lib` directly from Clap. The task is a shebang wrapper ending in `exec forkctl "$@"`, uses `dir = "{{cwd}}"`, and declares exact task-local forkctl/Rust/StGit tools. It does not use deprecated Tera argument functions or manually mirror `usage_*` variables. Optional `fork:hooks:install` and `fork:hooks:validate` helpers invoke the consumer's existing Lefthook configuration; they never edit it. The full exact catalog and macro contract are specified in `mise.md`.

A mounted command task is preferable to separate `fork:status`, `fork:check`, and patch wrappers: one task exposes every current/future subcommand, short form, validation rule, help panel, and completion without task drift.

### Hook environment

`process.rs` owns the only subprocess constructor. It obtains the repository-local variable names from `git rev-parse --local-env-vars` once per invocation context and removes them for every child command before applying an explicit cwd. It preserves nonlocal Git variables such as SSH transport, tracing, and credential configuration.

Architecture tests reject `Command::new` outside `process.rs` in production modules.

## CLI/API Mapping

### Request envelope

```json
{
  "protocol_version": 1,
  "manifest": "patches/downstream/fork.yaml",
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
    check.rs
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
