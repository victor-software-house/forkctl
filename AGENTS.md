# Mise Fork Tasks

Reusable downstream-fork maintenance for StGit patch stacks.

## Architecture

- `forkctl` is a small Rust policy CLI. It delegates all repository and patch mechanics to `git` and `stg`; never reimplement either data model.
- `tasks/fork.toml` is the mise-native remote task catalog. Every task declares all required tools and versions so consuming repositories need only mise.
- A consuming fork owns one JSON manifest and its exported patch files. Generic executable logic stays here.
- `src/patchexport.tmpl` pins StGit export formatting across StGit upgrades.

## Invariants

- Keep exactly three public operations: `init`, `verify`, and `rebase`.
- No runtime language, package manager, config parser, or shell-library dependency beyond the mise-provisioned `forkctl`, `git`, and `stg` executables.
- Remote task and release versions are immutable. Consumers pin the remote catalog by commit or release tag.
- `verify` must fail closed on dirty worktrees, missing tools, remote drift, patch-order drift, undeclared paths, missing source contracts, or non-reconstructable exports.
- `rebase` may update only declared base pins and exported source patches before refreshing the top tooling patch.
- Keep `main.rs` as wiring; implementation modules stay bounded by responsibility.

## Checks

```sh
mise run verify
mise run build
```

Test the released remote catalog against a real disposable fork clone before publishing a version.
