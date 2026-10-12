# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Repository input limits

- [x] 1.1 Pin ctl-core 0.6.11; proof: `cargo update -p ctl-core` on the build host reports `ctl-core v0.6.10 -> v0.6.11`
  - 2026-10-12 (UTC): reported on the build host.
- [x] 1.2 Call `use_repo_limits` in `App::discover`, keep the warnings on `App`, and fold a malformed file into the manifest error; proof: `repository_input_limits_apply_and_warn_when_loosened` passes
  - 2026-10-12 (UTC): passes on the build host.
- [x] 1.3 Print each warning to stderr in `main`; proof: the same test reads the exact warning line from two runs
  - 2026-10-12 (UTC): passes on the build host.
- [x] 1.4 Document `.ctl/input.yaml` in `README.md`, `AGENTS.md`, and the operator instructions template; proof: both `operator_docs` tests pass after `UPDATE_OPERATOR_DOCS=1`
  - 2026-10-12 (UTC): both pass on the build host after regenerating `src/instructions.md`.
- [x] 1.5 Add a changeset fragment; proof: `.changeset/repo-input-limits.md` is a `forkctl: patch` fragment
- [x] 1.6 Fold a malformed file only into an existing YAML manifest, so a JSON manifest and bootstrap with an absent manifest are not blocked; proof: the same test's `--manifest patches/absent.yaml init` case reports no error naming `.ctl/input.yaml`
  - 2026-10-12 (UTC): passes on the build host, in the pre-push run of the commit that adds it.
- [x] 1.7 Run the full gate; proof: `mise run verify` and `openspec validate --all --strict` pass on the build host
  - 2026-10-12 (UTC): pre-push `mise run verify` passes on the build host; `openspec validate --all --strict` passes.
