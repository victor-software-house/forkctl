# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Recover a refresh after its final check

- [x] 1.1 Reproduce the report; proof: `refresh_left_dirty_by_a_hook_names_its_recovery` fails on `main` at the `checking` phase assertion
  - 2026-10-06 (UTC): on the build host, with only the test applied, refresh fails with `dirty_worktree` and `operation status` reports phase `refreshing` where the test expects `checking`.
- [x] 1.2 Record phase `checking` and its next actions when the final check fails; proof: the same test passes its `operation status` assertions
  - 2026-10-06 (UTC): passes on the build host.
- [x] 1.3 Make `patch finish` refuse during an operation; proof: the same test reads `operation_in_progress` from `patch finish` before the recovery
  - 2026-10-06 (UTC): passes on the build host.
- [x] 1.4 Resume with `operation continue` and finish once; proof: the same test reads the active patch after `continue`, none after one `patch finish`, and a passing `check`
  - 2026-10-06 (UTC): passes on the build host. `patch_refresh_abort_restores_stack_and_active_patch` still passes.
- [x] 1.5 State the recovery in `.ctl/operator/instructions.md.jinja` and `.ctl/operator/SKILL.md.jinja`; proof: both `operator_docs` tests pass after `UPDATE_OPERATOR_DOCS=1`
  - 2026-10-06 (UTC): both pass on the build host after `UPDATE_OPERATOR_DOCS=1` regenerated `src/instructions.md` and `skills/forkctl/SKILL.md`.
- [x] 1.6 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-10-06 (UTC): `mise run verify` runs 117 of 117 tests on the build host, and `openspec validate --all --strict` passes.
