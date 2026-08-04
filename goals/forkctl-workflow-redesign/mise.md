# Forkctl Workflow Redesign — Mise and Lefthook Integration

## Design Goal

Mise must expose the complete forkctl CLI—not incomplete wrappers whose arguments drift from Clap. The `fork` file task is a transparent `raw_args` proxy: forkctl owns parsing, validation, and help; a mise-documented Usage self-mount supplies the same Clap-derived grammar to shell completion. Lefthook calls that task but is not required by forkctl core.

## Consumer Configuration

```toml
min_version = "<minimum-mise-version>"

[settings]
experimental = true
lockfile = true

[env]
FORK_MANIFEST = "patches/fork.yaml"

[task_config]
includes = [
  "git::https://github.com/victor-software-house/forkctl.git//tasks/fork?ref=<immutable-release>",
  "mise-tasks",
]
```

- The release tag is immutable.
- `mise-tasks` remains only when the consumer has local file tasks; declaring `task_config.includes` disables mise's automatic task-directory inclusion.
- `FORK_MANIFEST` is the sole consumer-local manifest pointer.

## Remote Catalog

`tasks/fork/` is a remote file-task directory:

```text
tasks/fork/
├── fork
└── hooks/
    ├── install
    └── validate
```

Canonical `tasks/fork/fork`:

```bash
#!/usr/bin/env bash
#MISE description="Run the complete forkctl CLI with exact Git/StGit tools and mounted shell completion"
#MISE dir="{{cwd}}"
#MISE raw_args=true
#MISE tools = { rust = "<rust-version>", "cargo:stgit" = { version = "<stgit-version>", depends = ["rust"] }, "github:victor-software-house/forkctl" = "<forkctl-version>" }
#USAGE mount "mise run --quiet fork -- --usage-spec=fork"

set -euo pipefail
exec forkctl "$@"
```

`hooks/install` and `hooks/validate` are executable file tasks with `#MISE dir="{{cwd}}"`, the Lefthook pin generated from root `mise.toml`, and direct `exec lefthook install|validate`.

## Why These Mise Features

| Mise feature | Use |
|:--|:--|
| `dir = "{{cwd}}"` | Preserve the operator's invocation directory; forkctl itself discovers the repository root. `{{config_root}}` would silently force every call to the config root and hide cwd-sensitive mistakes. |
| `#USAGE mount ...` | Import the complete generated forkctl grammar for shell completion instead of restating arguments in task config. Mise's source intentionally hoists root file-task mounts into the task spec. |
| `mise run --quiet fork -- --usage-spec=fork` | Self-mount through the task's exact tool environment while suppressing mise's task banner from interactive completion. The separator is needed only for this bootstrap call. |
| Shebang task plus `exec forkctl "$@"` | Preserve the parsed command line exactly and make forkctl the process receiving signals/exit status. |
| `raw_args = true` | Make the task a transparent CLI proxy: mise does not intercept `--help` or parse forkctl's arguments, while the mounted spec still supplies shell completion. |
| Exact task-local `tools` | Provision forkctl and StGit with no ambient/global dependency. |
| `{{cwd}}` rather than shell `cd` | Use mise's native directory macro; no audit-noisy wrapper commands. |
| Rich `description` | Make the task understandable in `mise tasks` and by agents before invocation. |

No deprecated `{{arg()}}`, `{{option()}}`, or `{{flag()}}` Tera functions appear. No manually maintained `${usage_*}` forwarding appears because `raw_args`, `exec`, and `"$@"` preserve the full command line. The generated Usage completion spec itself comes from `usage_lib::Spec::from(&Cli::command())`, so Clap remains the single source.

## Operator Usage

```sh
mise run fork --help
mise run fork status
mise run fork check
mise run fork check -s
mise run fork patch create reliable-busy-close -k source -p '...' -u not-submitted -d '...' -s 'src/**'
mise run fork patch refresh
mise run fork patch finish
mise run fork rebase -o refs/heads/main -n
mise run fork publish
```

Mise flags go before the task name:

```sh
mise run --silent fork check
```

Everything after `fork` belongs to the mounted forkctl grammar, so ordinary invocation does not need `--`.

## Mise Help and Completion

`mise run fork --help` reaches forkctl directly because the task sets `raw_args = true`; mise's own source explicitly treats such tasks as CLI proxies. `forkctl --usage-spec=fork` emits Usage KDL generated from the full Clap tree, relabeled for the mounted task, and augmented with dynamic completion entries. The mount runs only when shell completion asks for the task spec, exactly as mise documents.

Mise therefore provides:

- subcommand completion;
- short/long flag completion;
- enum choices;
- required/optional argument awareness during completion;
- repeated flag handling;
- file/directory completion;
- live patch-name completion;
- local remote/ref completion;
- direct forkctl validation and colored help with unchanged exit codes and streams.

Examples:

```sh
mise run fork patch <TAB>
mise run fork patch refresh --<TAB>
mise run fork patch show <TAB>       # live patch names
mise run fork rebase --onto <TAB>    # local refs
```

The mounted completion command self-invokes the task because completion resolution happens outside the task process; this is mise's documented pattern for applying task-local tools before forwarding `--usage-spec`. `--quiet` suppresses mise's task banner from completion. Integration tests exercise real mise against the catalog and compare direct forkctl vs mounted completion.

## Lefthook Composition

Recommended VSH consumer snippet:

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

Properties:

- No duplicated file glob: forkctl manifest scope is canonical. Lefthook `glob` remains available for operator-specific optimization but is not a safety boundary.
- No `stage_fixed`: `check -s` is read-only.
- Consumer format/lint fixers may run before forkctl's staged check using normal Lefthook ordering.
- `patch refresh` invokes StGit, which invokes pre-commit; a formatter may update the index and StGit consumes that final index.
- Pre-push full check runs under hook-exported Git variables; forkctl's production process layer clears repository-local variables for nested repositories.
- Consumers may omit these hooks or use another manager without changing forkctl commands.

## Catalog Tests

A release is blocked unless a disposable consumer proves:

1. remote catalog loads from the immutable tag;
2. `mise run fork --help` reaches forkctl's complete colored grammar;
3. direct and mounted long/short forms map to identical API requests;
4. invalid combinations fail through forkctl with the same exit code and diagnostics;
5. bash, fish, Nushell, PowerShell, and zsh task completion includes static grammar;
6. patch names and local refs complete dynamically;
7. Lefthook pre-commit runs `check -s` and pre-push runs full `check`;
8. hook-exported Git variables do not contaminate nested verification clones;
9. task exit codes and stdout/stderr match direct forkctl.
