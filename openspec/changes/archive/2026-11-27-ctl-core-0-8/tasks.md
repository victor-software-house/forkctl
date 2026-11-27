# Tasks

## 1. The upgrade

- [x] 1.1 Pin ctl-core `=0.8.0-rc.2` in both dependency tables with `input-document` and `input-json` on the runtime entry, vsh-serde-saphyr `=0.0.6` and yamled `=0.0.14`, and take them past the cooldown with `mise run update:own`.
  Verify: `cargo tree -d` lists one copy of each ctl crate and of serde-saphyr.
  2026-11-27 (UTC), [PR 190][forkctl#190] merged as [633149217][forkctl-merge].
  `Cargo.toml` at that commit pins ctl-core `=0.8.0-rc.2` in both tables, with `input-document` and `input-json` on the runtime entry, vsh-serde-saphyr `=0.0.6` and yamled `=0.0.14`.
  The lock moved with one `cargo update -p ctl-core@<old> --precise 0.8.0-rc.2` under `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`, not with `mise run update:own`.
  `update:own` refuses the added crates erased-serde 0.4.11 and typeid 1.0.3, which no trusted owner publishes.
  The lock diff of the PR adds only those two and the ctl crates (`ctl-input` is new, `ctl-core`, `ctl-layout`, `ctl-term` and `ctl-text` move from 0.7.0 to 0.8.0-rc.2), and moves vsh-serde-saphyr from 0.0.3 to 0.0.6 and yamled from 0.0.12 to 0.0.14.
  The literal `cargo tree -d` did not run; CI is the only place this change compiled.
  The lock at the merge commit has no package name twice, so each ctl crate, vsh-serde-saphyr and yamled appears once, and the [CI run][forkctl-ci] of the PR head [866fdb6][forkctl-head] passed.
- [x] 1.2 Migrate every 0.8 break by the changelog's migration notes, and accept snapshots from a real run.
  Verify: `mise run verify`, `mise run test:isolated` and `mise run build` pass on the build host.
  2026-11-27 (UTC), the [CI run][forkctl-ci] of [866fdb6][forkctl-head] passed every job but the skipped `refresh`: the `verify` jobs, including `verify / Verify`, and `Isolated tests and release build`, which runs `test:isolated` and `build`.
  They ran on GitHub's hosted runner, which [AGENTS.md][agents-md] makes the gate, not on a build host.
  The migration is [a785843][forkctl-cli-commit].
  The snapshots it changed are committed in the PR, and the CI run compares them.
  Their acceptance from real runs on the build host is the author's record, not something CI shows.
  The table width check moved from 40 to 60 columns in that commit, because ctl-core 0.8 stacks `patch list` rows at 40 columns.

## 2. The protocol and the document

- [x] 2.1 Keep every JSON response on stdout, and add tests for both presentation scenarios.
  Verify: both scenarios pass as tests on the build host.
  2026-11-27 (UTC), the tests are [`a_failed_request_under_json_is_one_document_on_stdout`][test-failed] and [`an_unsupported_protocol_version_is_one_error_response_on_stdout`][test-protocol] in `tests/cli.rs`, added by [a785843][forkctl-cli-commit].
  Both passed in the [CI run][forkctl-ci] of the PR head, through `verify / Build and test`.
  They did not run on a build host.
- [x] 2.2 Add the golden test for `patch create --input`.
  Verify: the scenario passes as a test on the build host.
  2026-11-27 (UTC), [`patch_create_reads_its_arguments_from_an_input_document`][test-golden] was added by [bd89055][forkctl-golden-commit] and extended by [866fdb6][forkctl-head] after a review finding.
  It compares the `--dry-run` plan from [`patch.yaml`][input-fixture] with the plan from the matching flags and records it as a [snapshot][golden-snapshot].
  It then creates the patch for real and asserts the name, layer, purpose, upstream status, drop condition and scope on the stored patch.
  It passed in the [CI run][forkctl-ci] of the PR head, not on a build host.

## 3. Docs and release

- [x] 3.1 Describe the input document, its variables, the precedence, the piped default and the stream of each JSON document in the skill and the instructions, through their templates.
  Verify: the rendered files match their templates in `mise run verify`.
  2026-11-27 (UTC), [c8f2797][forkctl-docs-commit] changed both templates and both renders.
  `skills/forkctl/SKILL.md` and `src/instructions.md` at the merge commit state `--input`, `--input-format`, `FORKCTL_INPUT`, `FORKCTL_INPUT_FORMAT`, the precedence, the piped `tsv` default and that each JSON response is the one document on stdout.
  `verify / Verify` passed in the [CI run][forkctl-ci] of the PR head.
  `mise run verify` and the `operator_docs` test did not run outside CI.
- [x] 3.2 Write a `forkctl: patch` changeset naming the new flags and each output change.
  Verify: `openspec validate ctl-core-0-8 --strict` passes.
  2026-11-27 (UTC), [0a66c86][forkctl-changeset-commit] added `.changeset/ctl-core-0-8.md` with `forkctl: patch`.
  It names `--input`, `--input-format` and their variables, the piped `tsv` default, the parse error on stderr, table stacking, the new help flags, the new input wording and the `git push` stream.
  `openspec validate ctl-core-0-8 --strict` passed with OpenSpec 1.16.0 before the archive.
  [forkctl 0.1.2][forkctl-release] shipped the change, from [run 46038742514][forkctl-release-run].
  Its linux-arm64 job failed once on a cache-key race and passed on the rerun (attempt 2); [verctl#434][verctl#434] fixed the cause.
  The failed first attempt's log was not read here, so that account is the lead's.

[agents-md]: ../../../../AGENTS.md
[forkctl#190]: https://github.com/victor-software-house/forkctl/pull/190
[forkctl-merge]: https://github.com/victor-software-house/forkctl/commit/633149217febaf204fd63a3fdc525c5153e00580
[forkctl-ci]: https://github.com/victor-software-house/forkctl/actions/runs/46037395355
[forkctl-head]: https://github.com/victor-software-house/forkctl/commit/866fdb6b8ce5b2057d6ac0ec2f522a0af7cc03c4
[forkctl-cli-commit]: https://github.com/victor-software-house/forkctl/commit/a7858437dc4df87e7b04916c9f98f0484b952018
[forkctl-golden-commit]: https://github.com/victor-software-house/forkctl/commit/bd8905507d20b4faddc272d1d57dde7771bd3278
[forkctl-docs-commit]: https://github.com/victor-software-house/forkctl/commit/c8f27970bc54b84403d407dba7b26c97b4a66048
[forkctl-changeset-commit]: https://github.com/victor-software-house/forkctl/commit/0a66c86e3c17a8f92245405d501d0ac09c0fc8ba
[test-failed]: https://github.com/victor-software-house/forkctl/blob/633149217febaf204fd63a3fdc525c5153e00580/tests/cli.rs#L406
[test-protocol]: https://github.com/victor-software-house/forkctl/blob/633149217febaf204fd63a3fdc525c5153e00580/tests/cli.rs#L418
[test-golden]: https://github.com/victor-software-house/forkctl/blob/633149217febaf204fd63a3fdc525c5153e00580/tests/cli.rs#L449
[input-fixture]: https://github.com/victor-software-house/forkctl/blob/633149217febaf204fd63a3fdc525c5153e00580/tests/testdata/input/patch.yaml
[golden-snapshot]: https://github.com/victor-software-house/forkctl/blob/633149217febaf204fd63a3fdc525c5153e00580/tests/snapshots/cli__patch_create_reads_its_arguments_from_an_input_document.snap
[forkctl-release]: https://github.com/victor-software-house/forkctl/releases/tag/v0.1.2
[forkctl-release-run]: https://github.com/victor-software-house/forkctl/actions/runs/46038742514
[verctl#434]: https://github.com/victor-software-house/verctl/pull/434
