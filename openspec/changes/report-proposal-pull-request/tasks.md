# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Pull-request step

- [x] 1.1 Parse `--repo` from the downstream remote URL; proof: unit tests accept the https, ssh, and scp forms and reject a local path
  - 2026-09-29 (UTC): `proposal::tests::parses_github_remote_forms` and `proposal::tests::rejects_urls_that_name_no_repository` pass.
- [x] 1.2 Look up the open pull request before the push through `src/process.rs`; proof: a lifecycle test with a failing stub `gh` sees `subprocess_failed` and no remote proposal branch
  - 2026-09-29 (UTC): `publish_propose_fails_before_the_push_when_gh_cannot_read_the_repository` and `publish_propose_fails_before_the_push_when_gh_is_missing` pass. `publish_propose_refuses_a_remote_that_names_no_github_repository` covers the local-path remote.
- [x] 1.3 Create or edit after the push and fail on error; proof: lifecycle tests cover create, reuse by edit, and a failing create with `retryable: true`
  - 2026-09-29 (UTC): `publish_propose_updates_the_open_proposal_then_promotes` covers create and then edit. `publish_propose_reports_a_failed_create_after_the_push` covers the failure.
- [x] 1.4 Render the body from `templates/proposal-pr.md`; proof: the create test reads `--draft`, the title prefix, the patch name, and the promote command from the stub's log
  - 2026-09-29 (UTC): passes in `publish_propose_updates_the_open_proposal_then_promotes`. Every recorded call carries `--repo github.com/example/downstream`.
- [x] 1.5 Plan without `gh`; proof: a lifecycle test runs `publish --propose -n` with no `gh` on `PATH`
  - 2026-09-29 (UTC): `publish_propose_dry_run_makes_no_gh_calls` passes with a recording `gh` on `PATH` that logs no call. The test puts a recorder on `PATH` rather than removing `gh`, which proves the same condition.

## 2. Operator surfaces

- [x] 2.1 Drop "when `gh` is available" from the `--propose` help and the instructions template; proof: `git grep -n "when .gh. is available"` finds nothing, and the usage snapshot and operator docs are regenerated
  - 2026-09-29 (UTC): `git grep` finds no match. `usage_spec_is_the_mounted_fork_grammar` and both `operator_docs` tests pass.
- [x] 2.2 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): `mise run verify` passes with 105 of 105 tests, and `mise run test:isolated` passes with 105 of 105.
