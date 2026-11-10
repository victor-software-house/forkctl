# Tasks

Compile, format, lint, and test tasks run in CI with `mise run verify`.

## 1. Rebase

- [ ] 1.1 Delete the derived patch before the replay and derive in phase `deriving`; proof: the "Upstream changed the bundle" and "An upstream change outside every patch" scenarios pass in CI
- [ ] 1.2 Stop a failing build in phase `deriving`, and let `operation continue` derive again; proof: the "A failing build stops the rebase" scenario passes in CI
- [ ] 1.3 Add `rebase --verify-base` and the `derived` result field; proof: the "Upstream outputs that do not reproduce" scenario passes in CI

## 2. Surfaces and release

- [ ] 2.1 Update `src/instructions.md`, the operator templates and both renders, and `README.md`; proof: both `operator_docs` tests pass in CI, and `rg -n 'deriving' skills/forkctl/SKILL.md src/instructions.md` names the phase
- [ ] 2.2 Add a patch changeset fragment naming the phase, the flag, and the result field; proof: `mise run ver check` passes
- [ ] 2.3 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
