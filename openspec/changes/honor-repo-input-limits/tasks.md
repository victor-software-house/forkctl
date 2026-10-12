# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Repository input limits

- [ ] 1.1 Pin ctl-core 0.6.11; proof: `cargo tree -i ctl-core -e normal` on the build host shows `ctl-core v0.6.11`
- [ ] 1.2 Call `use_repo_limits` in `App::discover`, keep the warnings on `App`, and fold a malformed file into the manifest error; proof: `repository_input_limits_apply_and_warn_when_loosened` passes
- [ ] 1.3 Print each warning to stderr in `main`; proof: the same test reads the exact warning line from two runs
- [ ] 1.4 Document `.ctl/input.yaml` in `README.md`, `AGENTS.md`, and the operator instructions template; proof: both `operator_docs` tests pass after `UPDATE_OPERATOR_DOCS=1`
- [ ] 1.5 Add a changeset fragment; proof: `.changeset/repo-input-limits.md` is a `forkctl: patch` fragment
- [ ] 1.6 Run the full gate; proof: `mise run verify` and `openspec validate --all --strict` pass on the build host
