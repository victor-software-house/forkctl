# Forkctl Workflow Redesign — CLI Reference

## Synopsis

```text
forkctl [GLOBAL OPTIONS] <COMMAND>
```

## Global Options

| Short | Long | Value | Default | Meaning |
|:--|:--|:--|:--|:--|
| `-m` | `--manifest` | `PATH` | `FORK_MANIFEST`, then `patches/fork.json` | Manifest selection |
| `-f` | `--format` | `pretty|json` | `pretty` | Complete output representation |
| `-c` | `--color` | `auto|always|never` | `auto` | Pretty-output color policy |
| `-q` | `--quiet` | — | false | Suppress successful pretty output |
| `-V` | `--version` | — | — | Print version |
| `-h` | `--help` | — | — | Render command-specific colored help |
| — | `--usage-spec[=BIN]` | — | hidden | Emit Usage KDL from Clap; optional BIN relabels a mounted task root |

`--format json` conflicts with `--quiet`; JSON is already the complete machine contract. Global options may appear before or after subcommands because Clap marks them global.

## `init`

Create the first new-contract stack or hydrate an existing manifest in a clone.

```text
forkctl init [OPTIONS]
```

### Bootstrap options

| Short | Long | Required without manifest | Meaning |
|:--|:--|:--:|:--|
| — | `--upstream-remote` | yes | Fetch-only upstream remote name |
| `-u` | `--upstream-url` | yes | Exact upstream URL |
| — | `--upstream-ref` | yes | Full `refs/heads/*` tracking ref |
| — | `--downstream-remote` | yes | Publication remote name |
| — | `--downstream-branch` | yes | Managed branch name |
| `-b` | `--base` | yes | Full branch/tag ref or commit used as initial base |
| `-l` | `--ledger` | yes | Generated ledger path |
| `-e` | `--exports` | yes | Generated source-export directory |
| `-k` | `--bookkeeping-patch` | yes | Final tooling patch name |
| `-p` | `--bookkeeping-path` | no/repeatable | Additional ownership scope for tooling files |
| `-a` | `--allow-base` | no/repeatable | Allowed base-drift ownership glob |
| `-r` | `--required-text` | no/repeatable | Required repository assertion as `PATH=TEXT` |
| `-n` | `--dry-run` | no | Resolve and show bootstrap/hydration effects |

If the manifest exists, bootstrap options are rejected. If absent, `HEAD` must equal the resolved base.

```sh
forkctl -m patches/fork.json init \
  --upstream-remote upstream \
  -u https://github.com/example/project.git \
  --upstream-ref refs/heads/main \
  --downstream-remote origin \
  --downstream-branch main \
  -b refs/heads/main \
  -l PATCHES.md \
  -e patches/downstream \
  -k fork-tooling \
  -p mise.toml -p lefthook.yml -p FORK.md
```

## `status`

Read repository, patch, active-state, worktree/index, check-summary, and current-operation state. Never mutates and remains usable during conflicts.

```text
forkctl status
forkctl status -f json
```

## `check`

One validation command with a scope parameter.

```text
forkctl check [OPTIONS]
```

| Short | Long | Value | Default | Meaning |
|:--|:--|:--|:--|:--|
| `-s` | `--staged` | — | false | Check index scope instead of full repository |
| `-p` | `--patch` | `NAME` | active patch | Explicit staged-check target; requires `--staged` |

```sh
forkctl check            # complete clean repository audit
forkctl check -s         # staged paths against active patch
forkctl check -s -p docs # staged paths against patch docs
```

An empty index succeeds in staged mode. A nonempty index without active/explicit patch fails.

## `patch list`

List ordered patch summaries, states, commit IDs, kinds, and active marker.

```text
forkctl patch list
```

## `patch show`

Show complete metadata, scope, changed paths, commit, generated export, and active state.

```text
forkctl patch show [NAME]
```

Omitted `NAME` means active patch.

## `patch create`

Create and select metadata-only active intent. No empty commit is created.

```text
forkctl patch create [OPTIONS] NAME
```

| Short | Long | Value | Required | Meaning |
|:--|:--|:--|:--:|:--|
| `-k` | `--kind` | `source|tooling` | yes | Patch layer |
| `-p` | `--purpose` | `TEXT` | yes | Downstream reason |
| `-u` | `--upstream-status` | `TEXT` | yes | Current upstream disposition |
| `-d` | `--drop-when` | `TEXT` | yes | Objective removal condition |
| `-s` | `--scope` | `GLOB` | yes/repeatable | Persistent ownership scope |
| `-n` | `--dry-run` | — | no | Validate and show draft-state write |

```sh
forkctl patch create reliable-busy-close \
  -k source \
  -p 'Protect destructive close while daemon work runs.' \
  -u not-submitted \
  -d 'Upstream adopts equivalent daemon-aware safety.' \
  -s 'Macterm/**/*.swift' -s 'MactermTests/**/*.swift' -s 'e2e/**/*.py'
```

## `patch select`

Select an existing patch locally without capturing changes.

```text
forkctl patch select [-n|--dry-run] NAME
```

## `patch edit`

Change metadata or persistent scope on explicit/active patch.

```text
forkctl patch edit [OPTIONS] [NAME]
```

Metadata uses create's `-k`, `-p`, `-u`, and `-d` forms. Scope operations:

| Short | Long | Value | Meaning |
|:--|:--|:--|:--|
| `-s` | `--set-scope` | `GLOB` repeatable | Replace complete scope |
| `-a` | `--add-scope` | `GLOB` repeatable | Add patterns |
| `-r` | `--remove-scope` | `GLOB` repeatable | Remove exact patterns |
| `-n` | `--dry-run` | — | Show metadata/commit/evidence effects |

`--set-scope` conflicts with add/remove. At least one edit is required.

```sh
forkctl patch edit -a README.md -r 'docs/old/**'
forkctl patch edit release-policy -u upstream-discussion-123 -n
```

## `patch refresh`

Capture changes into explicit/active patch and update all evidence/bookkeeping.

```text
forkctl patch refresh [OPTIONS] [NAME]
```

| Short | Long | Value | Default | Meaning |
|:--|:--|:--|:--|:--|
| `-s` | `--staged` | — | selected | Capture index |
| `-a` | `--all` | — | false | Stage/capture all owned changes |
| `-p` | `--path` | `PATHSPEC` repeatable | none | Stage/capture explicit Git pathspecs |
| `-n` | `--dry-run` | — | false | Show capture, hook, StGit, and generated effects |

Capture selectors are mutually exclusive. `--staged` is optional because it is the default, but remains explicit for hook/task readability.

```sh
git add Macterm/Model/Pane.swift e2e/test_panes.py
forkctl patch refresh
forkctl patch refresh reliable-busy-close --dry-run
forkctl patch refresh --all
forkctl patch refresh -p src/model.rs -p tests/model.rs
```

## `patch finish`

Require no remaining changes, run full `check`, and clear active state.

```text
forkctl patch finish [-n|--dry-run] [NAME]
```

## `rebase`

Resolve an exact target, create recovery evidence, and replay the stack without publication.

```text
forkctl rebase -o|--onto REF [-n|--dry-run]
```

Targets are full `refs/heads/*`, full `refs/tags/*`, or full commit SHAs.

## `publish`

Check and atomically push branch plus recovery tag under the current operation's exact lease.

```text
forkctl publish [-n|--dry-run]
```

Dry-run reports exact remote, refspecs, lease, and atomic requirement without pushing.

## `operation status`

Read current operation ID, kind, phase, evidence, conflict, and exact next actions.

```text
forkctl operation status
```

## `operation continue`

Revalidate completed phases and resume the current operation.

```text
forkctl operation continue [-n|--dry-run]
```

## `operation abort`

Plan or restore the recorded old state.

```text
forkctl operation abort -y|--yes [-n|--dry-run]
```

`--dry-run` does not require `--yes`. Execution requires confirmation in noninteractive contexts.

## `instructions`

Print the generated repository/operator workflow contract.

```text
forkctl instructions
```

## `completion`

Generate a self-correcting shell registration script from the Clap command graph.

```text
forkctl completion SHELL
```

Supported shells: `bash`, `elvish`, `fish`, `nu`, `powershell`, `zsh`.

```sh
source <(forkctl completion zsh)
forkctl completion fish > ~/.config/fish/completions/forkctl.fish
forkctl completion nu | save --force ~/.config/nushell/completions-forkctl.nu
```

Completions include the full command/flag grammar, enum choices, files/directories, local remotes/refs, live patch names, and active operation values. Domain candidate lookup is offline and returns no candidates—not errors—outside a valid repository.

## `api schema`

Emit JSON Schema 2020-12.

```text
forkctl api schema [-k|--kind KIND]
```

Kinds: `bundle`, `manifest`, `invocation`, `response`, `active-state`, `operation`.

## `api call`

Read one invocation from stdin and emit one response.

```text
forkctl api call
```

```sh
printf '%s' '{"protocol_version":1,"mode":"execute","request":{"command":"check","arguments":{"scope":"repository","patch":null}}}' \
  | forkctl api call
```

## Mise-native usage

The remote catalog exposes one canonical mounted task, not a duplicated set of shallow wrappers:

```sh
mise run fork status
mise run fork check -s
mise run fork patch create ...
mise run fork patch refresh
mise run fork rebase -o refs/heads/main
mise run fork publish -n
```

Mise's own flags appear before the task name; everything after `fork` belongs to the mounted forkctl grammar:

```sh
mise run --silent fork check
```

No `--` separator is required unless explicitly disambiguating a mise flag from a task flag.
