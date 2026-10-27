# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Deterministic source patch exports

- [ ] 1.1 Reproduce the report; proof: `patch_exports_do_not_depend_on_clone_configuration` fails on `main` at the 40-character `index` assertion
- [ ] 1.2 Pass `--diff-opt --full-index` and an empty `stgit.diff-opts` to `stg export`; proof: the same test passes, including `forkctl check` after `core.abbrev=12` and `stgit.diff-opts=--no-prefix`
- [ ] 1.3 Keep every other export test passing; proof: `explicit_patch_workflow_captures_staged_and_generates_evidence` and `patch_kind_change_reorders_series_and_exports` pass
- [ ] 1.4 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
