# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`, and in CI.

## 1. Manifest

- [x] 1.1 Add `Patch.trailers` and `Trailer`, validate them, and append them in `Patch::message`; proof: the `manifest` unit tests `message_appends_trailers_after_drop_when` and `rejects_invalid_trailers` pass
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests.
- [x] 1.2 Round-trip trailers through both codecs and the in-place YAML edit; proof: `yaml_and_json_round_trip_trailers` and `replacing_a_patch_writes_and_removes_its_trailers` pass
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests.

## 2. Commands

- [x] 2.1 Add `--trailer` to `patch create` and `patch edit`, `--clear-trailers` to `patch edit`, and the protocol fields; proof: `patch_edit_replaces_and_clears_trailers` passes and the usage snapshot shows both flags
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests. The snapshot was accepted from that run with `INSTA_UPDATE=always`.
- [x] 2.2 Verify declared trailers in `check`; proof: `check_verifies_declared_trailers` passes
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests.
- [x] 2.3 List trailers in `patch show` and `PATCHES.md`; proof: `check_verifies_declared_trailers` reads the ledger row, and every existing ledger test still passes unchanged
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests. `ledger_lists_trailers_only_when_declared` also passes.

## 3. Release

- [x] 3.1 Update `README.md`, `AGENTS.md`, and the operator templates with their renders; proof: both `operator_docs` tests pass
  - 2026-11-22 (UTC): passes in `mise run -c verify` on the build host at [d75f67e][head], 155 of 155 tests.
- [x] 3.2 Add a patch changeset fragment; proof: `mise run ver check` passes
  - 2026-11-22 (UTC): `mise run ver check` reports one fragment ok on the build host.
- [x] 3.3 Run the full gate; proof: `mise run verify` passes on the build host and in CI, and `openspec validate --all --strict` passes
  - 2026-11-22 (UTC): `mise run -c verify` passes every task on the build host at [d75f67e][head], with `check:versions` run again once the clone had an `origin/main` to read; `openspec validate --all --strict` passes 11 of 11 items. Every check of [CI run 45000476054][run] passes on [508bba2][pushed], `Build and test` and `Isolated tests and release build` included, and Kody leaves no open thread on [PR 182][pr].

[head]: https://github.com/victor-software-house/forkctl/commit/d75f67ec5eb18631fee7dd4364a9ee590480bff5
[pushed]: https://github.com/victor-software-house/forkctl/commit/508bba2c63e610ea70a79e98028ba1cda41b73e3
[run]: https://github.com/victor-software-house/forkctl/actions/runs/45000476054
[pr]: https://github.com/victor-software-house/forkctl/pull/182
