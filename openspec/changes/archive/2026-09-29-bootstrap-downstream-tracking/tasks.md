# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Tracking

- [x] 1.1 Create a missing downstream branch at the base under an absent lease; proof: `bootstrap_from_a_fresh_upstream_clone_needs_no_manual_step` reads the base from `git ls-remote origin refs/heads/main`
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.2 Plan the creation; proof: the same test reads the exact `ref_updates` entry from `init -n` and finds no remote branch afterwards
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.3 Set tracking before StGit state; proof: the same test reads `origin/main` from `@{upstream}`, refreshes a source patch, and publishes as a fast-forward
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.4 Name the fix for missing tracking; proof: `a_branch_without_upstream_names_the_fix` reads the message and `suggested_command`
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.5 State the behaviour in the `init` paragraph of `.ctl/operator/instructions.md.jinja`; proof: both `operator_docs` tests pass
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
- [x] 1.6 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): passes on the build host; `mise run verify` runs 109 of 109 tests.
