# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Abort over rewritten files

- [ ] 1.1 Reset tracked changes before StGit deletes the stack; proof: `abort_discards_a_tracked_file_a_hook_rewrote` passes
- [ ] 1.2 Name staging as the fix for an owned path; proof: `refresh_with_a_hook_rewriting_an_owned_file_continues_once_staged` passes
- [ ] 1.3 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
