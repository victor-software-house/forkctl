# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Environment-bound options

- [x] 1.1 Pin ctl-core 0.6.21; proof: `cargo update -p ctl-core` on the build host reports `ctl-core v0.6.17 -> v0.6.21`
  - 2026-10-15 (UTC): reported on the build host.
- [x] 1.2 Put `env_prefix = "FORKCTL"` on the root parser; proof: `variables_set_flags_under_the_command_line` passes
  - 2026-10-15 (UTC): passes on the build host, in `mise run verify`.
- [x] 1.3 Remove the `FORK_MANIFEST` fallback and read `FORKCTL_MANIFEST` in completion; proof: `manifest_variable_selects_the_manifest` passes, and a search of the repository finds `FORK_MANIFEST` only in `CHANGELOG.md` history
  - 2026-10-15 (UTC): the test passes on the build host; `rg FORK_MANIFEST` matches only `CHANGELOG.md`.
- [x] 1.4 Remove every `FORKCTL_*` variable from test children; proof: `support::clear_forkctl_env` runs in `forkctl()` and `sandboxed_command`
  - 2026-10-15 (UTC): both helpers call it; the suite passes.
- [x] 1.5 Update the help snapshot and both operator renders; proof: `cli__usage_spec_is_the_mounted_fork_grammar` and both `operator_docs` tests pass
  - 2026-10-15 (UTC): regenerated with `INSTA_UPDATE=always` and `UPDATE_OPERATOR_DOCS=1`; both pass on the build host.
- [x] 1.6 Document the variables in `README.md`, `AGENTS.md`, the operator templates, `examples/mise.toml`, and `goals/`; proof: the same `operator_docs` tests pass
  - 2026-10-15 (UTC): pass on the build host.
- [x] 1.7 Add a changeset fragment; proof: `.changeset/env-flags.md` is a `forkctl: patch` fragment
  - 2026-10-15 (UTC): added.
- [x] 1.8 Run the full gate; proof: `mise run verify` and `openspec validate --all --strict` pass on the build host
  - 2026-10-15 (UTC): `mise run verify`, which runs `openspec validate --all --strict`, and `mise run test:isolated` (131 tests) pass on the build host.
