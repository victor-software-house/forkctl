# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Bootstrap on an off-branch tag

- [x] 1.1 Record the canonical base as the merge base with the fetched upstream branch; proof: `bootstrap_and_rebase_accept_an_off_branch_release_tag` reads `canonical_base` and `stack_base` from `check`
  - 2026-09-29 (UTC): passes on the build host. Without the change the same test fails with `subprocess_failed`.
- [x] 1.2 Refuse unallowed drift with `check_failed` before any push or write; proof: the same test finds no manifest and no `origin` branch after the refusal
  - 2026-09-29 (UTC): passes on the build host.
- [x] 1.3 Rebase onto the next off-branch tag; proof: the same test reads the new `canonical_base` and `stack_base` after `rebase --onto refs/tags/v1.1.0`
  - 2026-09-29 (UTC): passes on the build host.
- [x] 1.4 State the behaviour in the `init` paragraph of `.ctl/operator/instructions.md.jinja`; proof: both `operator_docs` tests pass
  - 2026-09-29 (UTC): both pass on the build host after `UPDATE_OPERATOR_DOCS=1` regenerated `src/instructions.md`.
- [x] 1.5 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): `mise run verify` runs 116 of 116 tests on the build host.
