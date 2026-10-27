# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Deterministic source patch exports

- [x] 1.1 Reproduce the report; proof: regenerating a downstream export with `git -c core.abbrev=N diff` matches the committed body only at the length the writing clone used
  - 2026-10-27 (UTC): on a fresh downstream clone with 105,851 objects, `check` failed with `export differs`; the committed body matched `core.abbrev=8` exactly and differed in 12 lines at 7 and at 9. The new test was not run against `main`.
- [x] 1.2 Pass `--diff-opt --full-index` and an empty `stgit.diff-opts` to `stg export`; proof: `patch_exports_do_not_depend_on_clone_configuration` passes, including `forkctl check` after `core.abbrev=12` and `stgit.diff-opts=--no-prefix`
  - 2026-10-27 (UTC): passes in [CI run 40487023619][run], job `verify / Build and test`.
- [x] 1.3 Keep every other export test passing; proof: `explicit_patch_workflow_captures_staged_and_generates_evidence` and `patch_kind_change_reorders_series_and_exports` pass
  - 2026-10-27 (UTC): both pass in the same [CI run][run], and in `Isolated tests and release build`.
- [x] 1.4 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
  - 2026-10-27 (UTC): every required check of [CI run 40487023619][run] passes; `openspec validate --all --strict` passes 7 of 7 items.

[run]: https://github.com/victor-software-house/forkctl/actions/runs/40487023619
