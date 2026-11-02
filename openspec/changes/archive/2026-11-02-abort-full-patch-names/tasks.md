# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Full patch names on abort

- [x] 1.1 Pass the journal's patch names to `stg uncommit` in `restore_operation_stack`; proof: `abort_restores_patch_names_longer_than_stgit_namelength` passes
  - 2026-11-02 (UTC): passes in [CI run 41420880651][run], jobs `verify / Build and test` and `Isolated tests and release build`; `patch_refresh_abort_restores_stack_and_active_patch` still passes.
- [x] 1.2 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
  - 2026-11-02 (UTC): every required check of [CI run 41420880651][run] passes; `openspec validate --all --strict` passes 10 of 10 items.

[run]: https://github.com/victor-software-house/forkctl/actions/runs/41420880651
