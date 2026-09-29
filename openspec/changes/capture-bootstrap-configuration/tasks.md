# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Capture

- [x] 1.1 Partition untracked files by the bookkeeping scope before any mutation; proof: `bootstrap_from_a_fresh_upstream_clone_needs_no_manual_step` sees `dirty_worktree` with only `notes.txt`, no manifest, and no remote branch
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.2 Add captured files to the bookkeeping patch; proof: the same test finds `mise.toml` tracked and a clean worktree after `init`
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.3 State the capture in the `init` paragraph of `.ctl/operator/instructions.md.jinja`; proof: both `operator_docs` tests pass
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.4 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
