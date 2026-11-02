# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Full patch names on abort

- [ ] 1.1 Pass the journal's patch names to `stg uncommit` in `restore_operation_stack`; proof: `abort_restores_patch_names_longer_than_stgit_namelength` passes
- [ ] 1.2 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
