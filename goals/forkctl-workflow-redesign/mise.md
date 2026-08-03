# Forkctl Workflow Redesign — Mise and Lefthook Integration

## Design Goal

Mise must expose the complete forkctl CLI—not six incomplete wrappers whose arguments drift from Clap. The mounted task validates arguments, renders task help, and completes the same grammar as direct forkctl. Lefthook calls that task but is not required by forkctl core.

## Consumer Configuration

```toml
min_version = "2026.7.7"

[settings]
experimental = true
lockfile = true

[env]
FORK_MANIFEST = "patches/fork.json"

[task_config]
includes = [
  "git::https://github.com/victor-software-house/forkctl.git//tasks/fork.toml?ref=v0.0.6",
  "mise-tasks",
]
```

- The release tag is immutable.
- `mise-tasks` remains only when the consumer has local file tasks; declaring `task_config.includes` disables mise's automatic task-directory inclusion.
- `FORK_MANIFEST` is the sole consumer-local manifest pointer.

## Remote Catalog

```toml
["fork"]
description = "Run the complete forkctl patch-stack CLI with exact Git/StGit tooling, mounted argument validation, help, and completion"
dir = "{{cwd}}"
usage = 'mount "mise run fork -- --usage-spec=fork"'
run = '''
#!/usr/bin/env bash
set -euo pipefail
exec forkctl "$@"
'''
tools = { rust = "1.97.1", "cargo:stgit" = { version = "2.6.1", depends = ["rust"] }, "github:victor-software-house/forkctl" = "0.0.6" }

["fork:hooks:install"]
description = "Install the consumer's existing Lefthook configuration; forkctl does not edit or replace it"
dir = "{{cwd}}"
run = "lefthook install"
tools = { lefthook = "2.1.10" }

["fork:hooks:validate"]
description = "Validate the consumer's composed Lefthook configuration, including forkctl checks"
dir = "{{cwd}}"
run = "lefthook validate"
tools = { lefthook = "2.1.10" }
```

## Why These Mise Features

| Mise feature | Use |
|:--|:--|
| `dir = "{{cwd}}"` | Preserve the operator's invocation directory; forkctl itself discovers the repository root. `{{config_root}}` would silently force every call to the config root and hide cwd-sensitive mistakes. |
| `usage = 'mount ...'` | Import the complete generated forkctl grammar instead of restating args/flags in task TOML. |
| `mise run fork -- --usage-spec=fork` | Official self-mount pattern: the task's exact tool environment exists while the hidden bootstrap spec is emitted. The separator is needed only for this bootstrap call. |
| Shebang task plus `exec forkctl "$@"` | Preserve the parsed command line exactly and make forkctl the process receiving signals/exit status. |
| Exact task-local `tools` | Provision forkctl and StGit with no ambient/global dependency. |
| `{{cwd}}` rather than shell `cd` | Use mise's native directory macro; no audit-noisy wrapper commands. |
| Rich `description` | Make the task understandable in `mise tasks` and by agents before invocation. |

No deprecated `{{arg()}}`, `{{option()}}`, or `{{flag()}}` Tera functions appear. No manually maintained `${usage_*}` forwarding appears because the mounted spec and `"$@"` preserve the full command line. The generated Usage spec itself comes from `usage_lib::Spec::from(&Cli::command())`, so Clap remains the single source.

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

`forkctl --usage-spec=fork` emits Usage KDL generated from the full Clap tree, relabeled for the mounted task, and augmented with dynamic completion entries. Mise therefore provides:

- subcommand completion;
- short/long flag completion;
- enum choices;
- required/optional argument validation;
- repeated flag handling;
- file/directory completion;
- live patch-name completion;
- local remote/ref completion;
- task help matching direct `forkctl --help` semantics.

Examples:

```sh
mise run fork patch <TAB>
mise run fork patch refresh --<TAB>
mise run fork patch show <TAB>       # live patch names
mise run fork rebase --onto <TAB>    # local refs
```

The mounted completion command runs inside the task's exact tool environment. Integration tests invoke real mise against the immutable catalog and compare direct forkctl vs mounted parsing/completion.

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
2. `mise run fork --help` displays mounted grammar;
3. direct and mounted long/short forms map to identical API requests;
4. invalid combinations fail in mise before forkctl execution where the Usage spec can decide them;
5. bash, fish, Nushell, PowerShell, and zsh task completion includes static grammar;
6. patch names and local refs complete dynamically;
7. Lefthook pre-commit runs `check -s` and pre-push runs full `check`;
8. hook-exported Git variables do not contaminate nested verification clones;
9. task exit codes and stdout/stderr match direct forkctl.
