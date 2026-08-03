# forkctl

Reusable, mise-provisioned maintenance for downstream forks carried as [StGit](https://stacked-git.github.io/) patch stacks.

The repository publishes:

- `forkctl` — a self-contained Rust binary that validates fork policy and delegates patch operations to `stg`;
- `tasks/fork.toml` — remote mise tasks exposing `fork:init`, `fork:verify`, and `fork:rebase` with every tool dependency declared.

## Consumer setup

A fork keeps one manifest and its exported patches, then pins this task catalog:

```toml
min_version = "2026.7.7"

[settings]
experimental = true
lockfile = true

[env]
FORK_MANIFEST = "patches/embedding/fork.json"

[task_config]
includes = [
  "git::https://github.com/victor-software-house/forkctl.git//tasks/fork.toml?ref=<immutable-ref>",
  "mise-tasks",
]
```

`task_config.includes` replaces mise's default task directories, so list local `mise-tasks` explicitly when the repository has unrelated local tasks.

Then use:

```sh
mise run fork:init
mise run fork:verify
mise run fork:rebase
```

Mise installs the pinned `forkctl`, Rust, and `cargo:stgit` tools in its isolated store before running a task. No global Cargo or Homebrew installation is required.

For direct use outside mise, install the CLI from crates.io and provide `git` and `stg` on `PATH`:

```sh
cargo install forkctl --locked
```

## Development

```sh
mise install --locked
mise exec -- lefthook install
mise run verify
mise run build
```

The gate runs rustfmt check, Clippy across all targets/features with warnings denied, and all tests. Lefthook runs formatting and lint checks before commits and tests before pushes; hooks skip in CI.

## Manifest

```json
{
  "schema": 1,
  "upstream": {
    "remote": "upstream",
    "url": "https://github.com/example/project.git",
    "ref": "upstream/main"
  },
  "bases": {
    "canonical": "0000000000000000000000000000000000000000",
    "stack": "0000000000000000000000000000000000000000"
  },
  "patches": [
    { "name": "downstream-change", "export": "patches/0001-downstream-change.patch" },
    { "name": "fork-tooling" }
  ],
  "allow": {
    "base": [],
    "tooling": ["AGENTS.md", "FORK.md", "mise.toml", "mise.lock", "patches/*"]
  },
  "required": [
    { "path": "include/project.h", "contains": "required_symbol" }
  ]
}
```

Patches with `export` are source patches and must precede tooling-only patches. The last exported patch defines the source tree reconstructed by verification. The final patch is refreshed after a successful rebase.

## StGit template

`forkctl` embeds a pinned `patchexport.tmpl` matching StGit's stable mail-style export. StGit templates can interpolate patch descriptions, authors, dates, and diffstats; base commit SHAs remain explicit manifest state updated by `fork:rebase`.
