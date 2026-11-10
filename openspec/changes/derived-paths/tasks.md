# Tasks

Compile, format, lint, and test tasks run in CI with `mise run verify`.

## 1. Declaration

- [ ] 1.1 Add the optional `derived` block with `outputs`, `rebuild`, and `inputs`, validated against forkctl's documents; proof: a `manifest` unit test rejects an output glob over the manifest, and `api schema` lists `derived`
- [ ] 1.2 Add `--derived-output`, `--derived-input`, `--derived-rebuild`, and `--clear-derived` to `contract edit`; proof: the usage snapshot test passes in CI with the new flags and their `FORKCTL_*` variables
- [ ] 1.3 Let the derived patch sit above the bookkeeping patch, and report `derived_path_foreign`; proof: the "A hand patch touches the bundle" scenario passes in CI

## 2. Derive

- [ ] 2.1 Compute the input hash and the rebuild hash from `git ls-tree -r`; proof: unit tests cover the default inputs, declared `inputs`, and the excluded documents
- [ ] 2.2 Build in a clone that deletes the outputs first, commit the derived patch with both trailers, and cache the result; proof: the "Derive the first time", "A stale upstream output does not survive", and "The same state is not built twice" scenarios pass in CI
- [ ] 2.3 Report `derived_rebuild_failed` and `derived_rebuild_dirty` and write nothing; proof: the "The install rewrites a lockfile" scenario passes in CI
- [ ] 2.4 Add `forkctl derive` with plan mode and its refusals; proof: the `derive` case in `every_mutation_has_a_typed_non_mutating_failure_case` passes in CI

## 3. Patches

- [ ] 3.1 Derive in `patch finish`, and keep `patch refresh` free of builds; proof: the "Finish after a source change" scenario passes in CI

## 4. Check

- [ ] 4.1 Report `derived_absent` and `derived_stale` with no build; proof: the "A patch changed a source", "A change to a build configuration", and "A metadata edit does not rebuild" scenarios pass in CI
- [ ] 4.2 Add `check --rebuild` and `derived_unreproducible`; proof: the "A build that reads the clock" scenario passes in CI

## 5. Surfaces and release

- [ ] 5.1 Update `src/instructions.md`, the operator templates and both renders, `README.md`, `AGENTS.md`'s product invariants, and `examples/fork.yaml`; proof: both `operator_docs` tests pass in CI, and `rg -n 'derived' skills/forkctl/SKILL.md src/instructions.md` names the derived patch, `derive`, and the clean build
- [ ] 5.2 Add a patch changeset fragment naming the new block, flags, variables, command, and finding codes; proof: `mise run ver check` passes
- [ ] 5.3 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
