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
FORK_MANIFEST = "patches/fork.yaml"

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

### Remove or temporarily disable a patch

```sh
mise run fork patch remove old-policy --reason 'Upstream now provides it'
mise run fork publish

mise run fork patch disable optional-feature --reason 'Not needed on this host'
mise run fork publish
mise run fork patch enable optional-feature
mise run fork publish
```

All three transitions require a clean fully checked stack and no active patch.
They create an annotated recovery tag and publication operation, mutate the
series through StGit, regenerate the manifest/ledger/exports, and stop at the
same exact-lease atomic `publish` gate as rebase. The bookkeeping patch cannot
be removed or disabled.

Disabled patches are absent from the source tree and active StGit series. Their
metadata, former commit, original position, reason, and recovery evidence remain
under `disabled_patches`; fresh clone hydration fetches those recovery tags.
Enabling imports the preserved patch at its former position with a 3-way apply
and can continue or abort through the normal operation journal on conflict.
Permanent removal writes the same evidence to manifest history instead.

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

## Declared checks

Structural validation answers the question a clean merge cannot: does this patch still do what it says — and does it still cover everything it needs to?

A patch encodes the cases that existed when it was written. The drift that breaks a long-lived fork is upstream introducing cases it never covered: a new call site, a new trait implementor, a new enum variant, in files the patch does not own and that may not have existed. So a patch declares **checks**: ordinary commands that must succeed, scoped by glob.

```sh
mise run fork patch create terminal-guard \
  -k source \
  -p 'Route every terminal spawn through the downstream guard.' \
  -u not-submitted \
  -d 'Upstream guards terminal spawning.' \
  -s 'crates/terminal/**' -s 'fork-rules/**' \
  -C 'unguarded-spawn=ast-grep scan --rule fork-rules/unguarded-spawn.yml {files}' \
  -g 'unguarded-spawn=crates/**/*.rs'
```

| Option | Meaning |
|:--|:--|
| `-C NAME=COMMAND` | declare a check; `{files}` expands to the checked files, shell-quoted |
| `-g NAME=GLOB` | restrict a declared check; repeatable, **defaults to the declaring patch's scope** |
| `--check-at NAME=stack\|patch` | choose the tree the check observes; defaults to `stack` |
| `--clear-checks` | replace the complete check set |

**Exit status is the verdict.** Zero passes; anything else is one typed `check_failed` finding naming the patch, the check, the exit code, and the first diagnostic line. No polarity flags and no expression language — a check is a command, so any tool works: `ast-grep`, `rg`, `grep`, `cargo`, a repo script.

Globs default to the declaring patch's scope and **may reach anywhere in the repository**. Scope governs what a patch may *modify*; a check only reads, and the invariants a fork depends on are usually about upstream code it does not own.

### Covering cases the patch never saw

Write a rule matching only the *unhandled* shape and let the tool's own exit status report it — the [Coccinelle](https://coccinelle.gitlabpages.inria.fr/website/) bad-pattern idiom. With `severity: error`, ast-grep exits non-zero exactly when the bad shape exists:

```yaml
id: unguarded-spawn
language: rust
severity: error
message: spawn_terminal bypasses the downstream guard
rule:
  all:
    - pattern: spawn_terminal($$$ARGS)
    - not:
        inside:
          kind: function_item
          has: { field: name, regex: ^spawn_terminal_guarded$ }
          stopBy: end
```

A caller added upstream in a crate the patch does not own now fails the check. The same shape expresses "every implementor of this trait must carry the downstream method" (`kind: impl_item` + `has` trait + `not: has` method).

For Rust exhaustiveness the sharpest check forbids a catch-all arm in the patched `match`: a new upstream enum variant then becomes a compile error, delegated to the tool that does it best.

### Stack or patch layer

`--check-at stack` (the default) checks out the complete applied series in a disposable clone. `--check-at patch` checks out the declaring patch's own commit instead, so an invariant can be verified for that layer alone — independent of what later patches do. Forkctl invokes commands with the clone as cwd and removes its origin, protecting the operator worktree from ordinary relative writes and accidental pushes.

Declared commands remain trusted repository code running with the operator's user privileges; the clone is an execution boundary, not a security sandbox. Review declarations before running them.

### Stale globs

A check whose globs match **no tracked file fails as stale**. A command over an empty file list usually succeeds, so a moved or deleted subject would otherwise disarm the very check that was watching it:

```
check_glob_stale · guarded-spawn: unguarded-spawn
crates/**/*.rs matches no tracked file, so the check would pass over nothing;
its subject moved or was removed
```

### Execution

`check` runs declared checks after structural validation, in manifest order, so they gate `patch refresh`, `patch finish`, `rebase`, and both hook checks. Commands run through forkctl's process layer with repository-local Git variables cleared and an explicit cwd, `sh -c`, and shell-quoted `{files}`. Checks and their commands are rendered in the generated ledger, so what a fork enforces is audited alongside why each patch exists.

Forkctl ships no checking tooling and embeds no parser, query language, or scripting runtime. A check names whatever the consumer's own `mise.toml` provides.

> ▲ A declared check is a command from a tracked manifest, executed by `check` — including in hooks and during fresh-clone `init` hydration. This is the same trust shape as repository-declared Lefthook hooks or mise tasks. Review a fork's manifest before hydrating a clone you do not control.

Checks describe what tools can see. Whether patched code is still reached belongs to the consumer's build and tests.

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

A patch whose complete effect disappears into the new base is dropped into history. A surviving patch that touches fewer paths records those lost paths as `path_changes` history bound to the same recovery tag and rendered in the ledger. This audits the replay delta without claiming whether upstream, conflict resolution, or another replay effect caused it.

`operation status`, `continue`, and `abort` expose the typed in-flight journal. `operation abort -n` reports the restoration plan; `operation abort -y` restores and checks old state before clearing the journal. While an operation is in flight, `status`, `operation status`, `operation continue`, and `operation abort` read the Git-private manifest snapshot, so an unreadable tracked manifest cannot block recovery.

Publish covers every unpublished downstream state, not only rebases:

| Local state versus published branch | Publish behavior |
|:--|:--|
| identical | reports `already_published` and pushes nothing |
| published tip is an ancestor | fast-forward push under an exact lease; no recovery tag required |
| rewritten history | creates an immutable annotated recovery tag at the **overwritten published tip**, then pushes tag plus branch atomically under an exact lease |
| rewritten history after a reviewed rebase | reuses the rebase recovery tag when it already preserves the overwritten tip, otherwise publishes both |

If the branch already matches while a ready operation remains, `publish` verifies every recorded recovery ref before clearing the local journal and snapshot. Exact refs make the retry idempotent; missing required evidence is published atomically under the unchanged branch lease, while mismatched evidence fails closed.

Every rewrite requires exact evidence that it was computed against the current published tip: the reviewed rebase lease when an operation journal exists, otherwise the fetched downstream tracking ref. A remote that advanced beyond that evidence fails with `remote_advanced` and changes no ref. There is no force, lease, or atomic fallback and no provider ruleset administration.

Pretty `publish` streams `git push` and hook output to stderr as it happens (pre-push Jest, husky, remote progress). `--format json` and `api call` still capture that output and keep stderr empty.

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
printf '%s' '{"protocol_version":1,"manifest":"patches/fork.yaml","mode":"execute","request":{"command":"check","arguments":{"scope":"repository"}}}' \
  | forkctl api call
```

The API already accepts complete typed command arguments from JSON on stdin; `manifest` selects either a YAML or JSON manifest path. This is the generic programmatic interface — no second stdin argument grammar exists.

Requests use dotted command names such as `patch.refresh` and `operation.abort`, command-specific typed arguments, and `mode: execute|plan`. Success, plan, notice, error, and error-detail types are schema-derived. JSON stdout is exactly one response and stderr is empty.

Schema kinds: `bundle`, `manifest`, `invocation`, `response`, `active-state`, `operation`.

## Manifest

YAML is the default human-authored format:

```yaml
%YAML 1.2
---
schema: 1
downstream:
  remote: origin
  branch: main
  recovery_tag_prefix: forkctl/recovery
upstream:
  remote: upstream
  url: https://github.com/example/project.git
  fetch_ref: refs/heads/main
base:
  target:
    kind: branch
    selector: refs/heads/main
    commit: '0000000000000000000000000000000000000000'
  canonical: '0000000000000000000000000000000000000000'
  stack: '0000000000000000000000000000000000000000'
documents:
  ledger: PATCHES.md
  exports: patches/downstream
bookkeeping_patch: fork-tooling
patches:
  - name: downstream-change
    kind: source
    purpose: Describe why this downstream change exists.
    upstream_status: not-submitted
    drop_when: Upstream provides the required behavior.
    scope:
      - src/**
      - tests/**
    checks:
      - name: downstream-hook
        run: grep -q downstream_hook {files}
        glob: []
        at: stack
      - name: unguarded-call
        run: ast-grep scan --rule fork-rules/unguarded-call.yml {files}
        glob:
          - crates/**/*.rs
        at: stack
disabled_patches: []
history: []
contracts:
  allow_base: []
  required_text: []
```

The extension selects the codec: `.yaml`/`.yml` reads and rewrites canonical YAML; `.json` reads and rewrites canonical JSON. Both deserialize into the same typed `Manifest`, execute the same handlers, and produce the same generated evidence. Unknown or missing extensions are rejected — forkctl never sniffs bytes or silently changes a file's format.

YAML parsing rejects duplicate keys, merge keys, aliases/anchors, multiple documents, odd indentation, ambiguous booleans, excessive nesting, and manifests over 2 MiB. JSON remains available for generated consumers; YAML is the readable default. Git-private operation/active state, API stdin/stdout, pretty/JSON output selection, and JSON Schema remain JSON.

Every patch commit carries matching `Downstream-Reason`, `Upstream-Status`, and `Drop-When` trailers. Source patches precede tooling patches. Every source export is generated deterministically as `<exports>/<order>-<name>.patch`; tooling patches have no export. The final tooling patch owns manifest, ledger, exports, and integration files.

## Bootstrap and clone hydration

Without a manifest, `init` requires explicit repository/base/document/bookkeeping arguments and `HEAD` exactly at the resolved base. Repeatable `--allow-base GLOB` and `--required-text PATH=TEXT` options initialize declarative contracts without manual manifest edits. It creates the initial bookkeeping patch and never imports legacy commits.

With a manifest, `init` idempotently reconstructs StGit metadata, skips history recovery tags already present locally at the recorded object, fetches only the exact missing recovery refs named by history, and reports an actionable error naming any recovery tag the downstream remote no longer serves before running the full check.

## Agent skill

The portable [`forkctl` Agent Skill](skills/forkctl/SKILL.md) teaches compatible coding agents to use the installed CLI safely: explicit patch intent, staged capture, checks, operation recovery, rebase review, exact-lease publication, and typed automation.

List or install it with the [Skills CLI](https://skills.sh/):

```sh
npx skills add victor-software-house/forkctl --list
npx skills add victor-software-house/forkctl --skill forkctl
```

Add `--global` when the skill should be available outside one project, and use the agent selector offered by the installer when targeting a specific runtime.

## Direct installation

```sh
cargo install forkctl --locked
```

Forkctl supports Linux and macOS. Windows is not currently supported: the mounted mise tasks use a POSIX shell, and declared checks execute through `sh -c`. Shell completion generation for PowerShell remains available for compatible Unix-hosted PowerShell environments.

Successful interactive commands check crates.io at most once every 24 hours and print one concise notice when a newer version is available. Checks time out after one second, fail silently, never run for JSON/completion/non-TTY output, and can be disabled with `FORKCTL_NO_UPDATE_CHECK=1`. Forkctl never overwrites its own executable: Cargo or the repository's pinned mise catalog remains the update owner.

Provide supported `git` and `stg` executables on `PATH`, plus whatever tools your declared checks invoke; the mounted mise task provisions exact Git/StGit versions automatically.

## Future architecture

The reviewed [composed-upstreams goal](goals/forkctl-composed-upstreams/goal.md) proposes composition-owned repositories assembled from exact file/directory projections of multiple Git sources, followed by one ordered patch stack. Its [facts](goals/forkctl-composed-upstreams/facts.md), [design](goals/forkctl-composed-upstreams/design.md), [requirements](goals/forkctl-composed-upstreams/requirements.md), [research](goals/forkctl-composed-upstreams/research.md), [decisions](goals/forkctl-composed-upstreams/decisions.md), and [implementation plan](goals/forkctl-composed-upstreams/plan.md) also define rewrite/append history and reviewable GitHub Actions proposals.

That package is future architecture only; the current release continues to implement the single-upstream lifecycle documented above.

## Development

```sh
mise install --locked
mise exec -- lefthook install
mise run verify
mise run test:isolated
mise run build
```

Pull requests and pushes to `main` run those gates on `blacksmith-4vcpu-ubuntu-2404` (`CARGO_BUILD_JOBS=4`) with `mise-action` cache and `Swatinem/rust-cache`. External actions are pinned to immutable commits; CI has read-only repository permissions. Native macOS assets are produced by the local release task, not a PR matrix.

`mise run test` uses Rust's standard parallel harness. `test:isolated` runs the same suite with cargo-nextest, one test per process. Real Git/StGit lifecycle tests use a fresh `tempfile` sandbox with private HOME/XDG/Git configuration/templates, deterministic identity/time/locale, and command-local environment; they never read operator aliases, credential helpers, hooks, or global config. Mounted-task tests reuse only the caller's mise installation store for the repository's exact pinned tools. Containers are reserved for future scenarios that actually require another OS, daemon, network, or toolchain image.

`[workspace.package].version` is the sole forkctl release source. Root `mise.toml` is the sole source for the minimum mise, Rust, StGit, Lefthook, Usage, and GitHub CLI versions. After changing either source, run `mise run version:sync`; it regenerates `mise.lock` and every operational pin. The verification gate rejects drift and runs rustfmt, denied-warning workspace Clippy, API/schema/help/completion tests, and disposable real Git/StGit lifecycle tests.
