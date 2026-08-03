# forkctl

Audited, mise-provisioned lifecycle control for downstream forks carried as [StGit](https://stacked-git.github.io/) patch stacks.

Forkctl is a small Rust policy CLI over real `git` and `stg` commands. Git remains canonical history; StGit remains responsible for patch mechanics and conflicts. Forkctl declares policy, verifies reproducibility, records review evidence, and publishes rewritten history only under an exact lease.

Every command handler returns a typed protocol value. Clap commands and JSON requests are equal adapters over those handlers; command/domain modules never print, detect terminals, or construct tables. One centralized Anstyle/Comfy Table view renders consistent pipe-safe human output, while Serde emits the same values as JSON and Schemars generates the full local API schema. Generated ledger and rebase-report documents remain compile-time, type-checked Askama templates.

The repository publishes:

- `forkctl` — the CLI;
- `tasks/fork.toml` — immutable remote mise tasks for `fork:init`, `fork:status`, `fork:new`, `fork:verify`, `fork:rebase`, and `fork:publish`;
- `examples/` — copy-ready manifest and mise configuration.

## Consumer setup

A fork owns one manifest, generated `PATCHES.md`, and any declared patch exports. Pin the task catalog to an immutable release or commit:

```toml
min_version = "2026.7.7"

[settings]
experimental = true
lockfile = true

[env]
FORK_MANIFEST = "patches/downstream/fork.json"

[task_config]
includes = [
  "git::https://github.com/victor-software-house/forkctl.git//tasks/fork.toml?ref=<immutable-ref>",
  "mise-tasks",
]
```

`task_config.includes` replaces mise's default task directories, so list `mise-tasks` when the repository also has local file tasks.

## Workflow

```sh
mise run fork:init
mise run fork:status
mise run fork:new -- downstream-change \
  --kind source \
  --purpose "Describe why the downstream change exists." \
  --upstream-status "not-submitted" \
  --drop-when "Upstream provides the required behavior." \
  --path src/example.rs
mise run fork:verify
mise run fork:rebase -- --onto refs/tags/v1.2.4
mise run fork:publish
```

`fork:new` creates a documented empty patch. Add and refresh its implementation, restore the bookkeeping patch, then run `mise run fork:new -- --finish` to regenerate declared exports and verify it. Rebase creates an annotated recovery tag and Git-private no-color range-diff report but never publishes. Publish verifies again and atomically pushes the recovery tag plus branch with an explicit `--force-with-lease=<ref>:<sha>`.

`forkctl instructions` prints the agent/operator contract without requiring a Git repository.

## Output and local API

```sh
forkctl status --output pretty
forkctl status --output json
forkctl status --json                 # compatibility alias
forkctl api schema
printf '%s' '{"protocol_version":1,"manifest":"patches/downstream/fork.json","request":{"command":"status"}}' \
  | forkctl api call
```

Pretty and JSON output consume the same typed result. JSON stdout is exactly one versioned success/error envelope; Git/StGit subprocess output is captured and cannot contaminate it. `api schema` emits JSON Schema 2020-12 for invocation and response envelopes.

## Manifest

```json
{
  "schema": 1,
  "downstream": {
    "remote": "origin",
    "branch": "main",
    "backup_tag_prefix": "vsh/pre-sync"
  },
  "upstream": {
    "remote": "upstream",
    "url": "https://github.com/example/project.git",
    "fetch_ref": "refs/heads/main"
  },
  "base": {
    "target": {
      "kind": "tag",
      "selector": "refs/tags/v1.2.3",
      "commit": "0000000000000000000000000000000000000000",
      "tag_object": "1111111111111111111111111111111111111111"
    },
    "canonical": "0000000000000000000000000000000000000000",
    "stack": "0000000000000000000000000000000000000000"
  },
  "ledger": "PATCHES.md",
  "bookkeeping_patch": "fork-tooling",
  "patches": [
    {
      "name": "downstream-change",
      "kind": "source",
      "purpose": "Describe why this downstream change exists.",
      "upstream_status": "not-submitted",
      "drop_when": "Upstream provides the required behavior.",
      "paths": ["src/example.rs"],
      "export": "patches/downstream/0001-downstream-change.patch"
    },
    {
      "name": "fork-tooling",
      "kind": "tooling",
      "purpose": "Own downstream fork policy and generated bookkeeping.",
      "upstream_status": "inappropriate: downstream-only tooling",
      "drop_when": "The downstream fork is retired.",
      "paths": ["FORK.md", "PATCHES.md", "mise.toml", "patches/downstream/*"]
    }
  ],
  "history": [],
  "allow": { "base": [] },
  "required": [
    { "path": "FORK.md", "contains": "mise run fork:verify" }
  ]
}
```

Every patch commit must carry trailers matching its manifest metadata:

```text
Downstream-Reason: Describe why this downstream change exists.
Upstream-Status: not-submitted
Drop-When: Upstream provides the required behavior.
```

Source patches precede tooling patches. The final tooling patch owns manifest, ledger, export, and task bookkeeping. A tooling-only stack is valid. Export destinations are concrete, bookkeeping-owned, and disjoint from source/contract paths.

Targets are typed historical selections: full commit SHAs, full `refs/heads/*` branch refs, or full `refs/tags/*` tag refs. Resolved commits remain immutable even when a selected branch later advances. Annotated tag objects are persisted and verified when available.

When `stg rebase --merged` produces an empty non-bookkeeping patch, forkctl removes it and appends its full metadata, former commit, and target to manifest-backed `PATCHES.md` history.

## Verification and recovery

Verification fails closed on dirty state, wrong branch/tracking, remote drift, base drift, patch order, unapplied or empty patches, undeclared per-patch paths, trailer drift, ledger drift, export drift, reconstruction drift, and missing source contracts.

If rebase conflicts, forkctl preserves the normal StGit state, pending lease, Git-private manifest snapshot, and recovery tag. Resolve explicitly:

```sh
stg add --update
stg refresh
stg goto <bookkeeping-patch>
mise run fork:rebase -- --onto <same-ref>
```

Forkctl never stashes or resolves conflict content.

## Direct installation

Mise tasks provision exact Rust, StGit, and forkctl versions. For direct use:

```sh
cargo install forkctl --locked
```

Provide supported `git` and `stg` executables on `PATH`.

## Version synchronization

`[workspace.package].version` in `Cargo.toml` is the sole version source. `mise run version:sync` updates the six task tool pins and `examples/mise.toml`. Lefthook synchronizes and stages them before commits; `mise run verify` rejects drift.

## Development

```sh
mise install --locked
mise exec -- lefthook install
mise run verify
mise run build
```

The gate runs rustfmt, workspace-wide Clippy `all` and `pedantic` with warnings denied, unit tests, and disposable real Git/StGit lifecycle tests. No test requires a consumer repository or network access.
