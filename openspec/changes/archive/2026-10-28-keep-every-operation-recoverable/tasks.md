# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Keep every operation recoverable

- [x] 1.1 Refuse an edit scope that drops changed paths before writing; proof: `patch_edit_keeps_the_scope_over_changed_paths` passes
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.2 Record phase `checking` when a patch edit fails its final check; proof: `patch_edit_left_dirty_by_a_hook_names_its_recovery` passes
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.3 Name the fix, `publish` and abort when a transition fails its final check; proof: `patch_remove_left_dirty_by_a_hook_names_its_recovery` passes
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.4 End abort after the restored stack is verified; proof: `abort_ends_the_operation_when_the_restored_stack_fails_check` passes
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.5 Regenerate every export in every bookkeeping write; proof: `contract_edit_regenerates_every_export` passes
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.6 Keep the existing recovery tests passing; proof: `refresh_left_dirty_by_a_hook_names_its_recovery` and `patch_refresh_abort_restores_stack_and_active_patch` pass
  - 2026-10-28 (UTC): passes in [CI run 40641193938][run], jobs `verify / Build and test` and `Isolated tests and release build`.
- [x] 1.7 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
  - 2026-10-28 (UTC): every required check of [CI run 40641193938][run] passes; `openspec validate --all --strict` passes 8 of 8 items.

[run]: https://github.com/victor-software-house/forkctl/actions/runs/40641193938
