# forkctl

Explicit, audited lifecycle control for downstream forks carried as [StGit](https://stacked-git.github.io/) patch stacks.

Forkctl is a Rust policy CLI over real `git` and `stg` commands. The operator declares patch intent; forkctl owns staged capture, targeted refresh, generated exports/ledger/manifest, recovery evidence, exact-lease publication, and typed CLI/API output.

Clap and the local JSON API execute the same typed handlers. Domain modules never print, detect terminals, or construct tables. A centralized Anstyle/Comfy Table renderer owns pretty output and colored width-aware help; Serde/Schemars own the versioned JSON contract and JSON Schema 2020-12.

## Consumer setup

```toml
min_version = "2026.7.7"

[settings]
experimental = true
lockfile = true

[env]
FORK_MANIFEST = "patches/fork.json"

[task_config]
includes = [
  "git::https://github.com/victor-software-house/forkctl.git//tasks/fork?ref=<immutable-ref>",
  "mise-tasks",
]
```

The remote catalog exposes one mounted task with the full forkctl grammar:

```sh
mise run fork --help
mise run fork status
mise run fork check
mise run fork patch refresh
```

The task uses `dir = "{{cwd}}"`, exact task-local tools, `exec forkctl "$@"`, and a Usage spec generated directly from Clap. `task_config.includes` replaces mise's default task directories, so retain `mise-tasks` only when the repository has local file tasks.

## Patch workflow

```sh
mise run fork patch create downstream-change \
  -k source \
  -p 'Describe why this downstream change exists.' \
  -u not-submitted \
  -d 'Upstream provides the required behavior.' \
  -s 'src/**' -s 'tests/**'

# edit normally
git add src/example.rs tests/example.rs
mise run fork check -s
mise run fork patch refresh
mise run fork patch finish
```

`patch create` records metadata-only active intent. `patch refresh` captures the index by default, targets the correct StGit patch, runs the consumer pre-commit hook, regenerates deterministic evidence, refreshes bookkeeping, and leaves the patch active for more edits. `patch finish` requires no remaining changes, runs the full check, and clears active state.

Explicit alternatives:

```sh
mise run fork patch refresh --all
mise run fork patch refresh -p src/example.rs -p tests/example.rs
mise run fork patch refresh -n             # semantic dry-run plan
```

Persistent ownership uses `scope` globs (`*` stays within a segment; `**` crosses directories). One-shot capture uses Git pathspecs. Forkctl never guesses intent.

Declarative contracts may be added after their files exist:

```sh
mise run fork contract edit -a 'vendor/**' -r 'FORK.md=forkctl check'
mise run fork contract edit --clear -r 'FORK.md=forkctl check'
```

Without `--clear`, entries append uniquely. `--clear` explicitly replaces the complete contract set after validating all supplied required text.

## Check, rebase, and publish

```sh
mise run fork status
mise run fork check                         # full clean-repository audit
mise run fork check -s                      # staged index against active patch
mise run fork rebase -o refs/heads/main
mise run fork operation status
mise run fork operation continue
mise run fork publish
```

Rebase creates an immutable annotated recovery tag, captures the remote lease and old ordered stack, delegates to `stg rebase --merged`, generates a Git-private range-diff report, and records dropped patches in operation-level recovery-bound history. It never publishes.

`operation status`, `continue`, and `abort` expose the typed in-flight journal. `operation abort -n` reports the restoration plan; `operation abort -y` restores and checks old state before clearing the journal.

Publish performs one atomic explicit-ref push of branch plus recovery tag with an exact lease. There is no force, lease, or atomic fallback and no provider ruleset administration.

## Hooks

Forkctl exposes ordinary read-only checks and does not own a hook manager:

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

Checks never stage or rewrite. Forkctl's production process layer clears Git repository-local hook variables before nested/foreign repository commands while preserving transport and authentication variables.

## CLI, help, and completion

```sh
forkctl patch refresh --help
forkctl completion zsh
forkctl completion nu
forkctl --usage-spec=fork
```

Most long options have collision-audited mnemonic shorts. Leaf parameters are grouped by subject, metadata/scope, capture, execution, and output. Help is generated from Clap metadata into colored width-aware panels; no second parameter specification exists. Pretty help, result tables, notices, and errors wrap to the detected terminal width; captured non-TTY output may provide the standard `COLUMNS` fallback. JSON, schemas, Usage KDL, and completion scripts never reflow.

Completion supports bash, elvish, fish, Nushell, PowerShell, and zsh, including commands, flags, enum values, files, local Git remotes/refs, live patch names, and current operation values. Candidate lookup is local and fail-silent.

## Local JSON API

```sh
forkctl status --format json
forkctl api schema --kind bundle
printf '%s' '{"protocol_version":1,"mode":"execute","request":{"command":"check","arguments":{"scope":"repository"}}}' \
  | forkctl api call
```

Requests use dotted command names such as `patch.refresh` and `operation.abort`, command-specific typed arguments, and `mode: execute|plan`. Success, plan, notice, error, and error-detail types are schema-derived. JSON stdout is exactly one response and stderr is empty.

Schema kinds: `bundle`, `manifest`, `invocation`, `response`, `active-state`, `operation`.

## Manifest

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
      "commit": "0000000000000000000000000000000000000000"
    },
    "canonical": "0000000000000000000000000000000000000000",
    "stack": "0000000000000000000000000000000000000000"
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
      "purpose": "Describe why this downstream change exists.",
      "upstream_status": "not-submitted",
      "drop_when": "Upstream provides the required behavior.",
      "scope": ["src/**", "tests/**"]
    }
  ],
  "history": [],
  "contracts": {
    "allow_base": [],
    "required_text": []
  }
}
```

Every patch commit carries matching `Downstream-Reason`, `Upstream-Status`, and `Drop-When` trailers. Source patches precede tooling patches. Every source export is generated deterministically as `<exports>/<order>-<name>.patch`; tooling patches have no export. The final tooling patch owns manifest, ledger, exports, and integration files.

## Bootstrap and clone hydration

Without a manifest, `init` requires explicit repository/base/document/bookkeeping arguments and `HEAD` exactly at the resolved base. Repeatable `--allow-base GLOB` and `--required-text PATH=TEXT` options initialize declarative contracts without manual manifest edits. It creates the initial bookkeeping patch and never imports legacy commits.

With a manifest, `init` idempotently reconstructs StGit metadata and fetches only exact recovery refs named by history before running the full check.

## Direct installation

```sh
cargo install forkctl --locked
```

Provide supported `git` and `stg` executables on `PATH`; the mounted mise task provisions exact versions automatically.

## Development

```sh
mise install --locked
mise exec -- lefthook install
mise run verify
mise run build
```

`[workspace.package].version` is the sole forkctl release source. Root `mise.toml` is the sole source for the minimum mise, Rust, StGit, Lefthook, Usage, and GitHub CLI versions. After changing either source, run `mise run version:sync`; it regenerates `mise.lock` and every operational pin. The verification gate rejects drift and runs rustfmt, denied-warning workspace Clippy, API/schema/help/completion tests, and disposable real Git/StGit lifecycle tests.
