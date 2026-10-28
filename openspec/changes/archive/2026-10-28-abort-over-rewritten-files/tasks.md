# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Abort over rewritten files

- [x] 1.1 Reset tracked changes before StGit deletes the stack; proof: `abort_discards_a_tracked_file_a_hook_rewrote` passes
  - 2026-10-28 (UTC): passes in [CI run 40646535477][run], jobs `verify / Build and test` and `Isolated tests and release build`; `lower_patch_refresh_conflict_can_continue_from_typed_journal` still passes.
- [x] 1.2 Refresh staged paths into the refreshed patch on continue, and name staging as the fix for an owned path; proof: `refresh_with_a_hook_rewriting_an_owned_file_continues_once_staged` passes
  - 2026-10-28 (UTC): passes in [CI run 40646535477][run], jobs `verify / Build and test` and `Isolated tests and release build`; `lower_patch_refresh_conflict_can_continue_from_typed_journal` still passes.
- [x] 1.3 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
  - 2026-10-28 (UTC): every required check of [CI run 40646535477][run] passes; `openspec validate --all --strict` passes 9 of 9 items.

[run]: https://github.com/victor-software-house/forkctl/actions/runs/40646535477
