# Tasks

Compile, format, lint, and test tasks run in CI with `mise run verify`.

## 1. Declaration

- [ ] 1.1 Add the optional `derived` record with `paths` and `rebuild` to `Patch`, validated against the patch's scope; proof: a `manifest` unit test rejects a derived glob outside the scope, and `api schema` lists `derived`
- [ ] 1.2 Add `-D/--derived` and `-R/--rebuild` to `patch create` and `patch edit`, and `--clear-derived` to `patch edit`; proof: the usage snapshot test passes in CI with the new flags and their `FORKCTL_*` variables
- [ ] 1.3 Report `derived_path_shared` and `derived_path_foreign` from a complete check; proof: lifecycle tests for both findings pass in CI

## 2. Replay

- [ ] 2.1 Replace the single `stg rebase --merged` with `--nopush` and a push loop that rebuilds and refreshes each owner; proof: the "Upstream changed only sources" scenario passes as a lifecycle test in CI
- [ ] 2.2 Resolve a conflict confined to derived paths by restoring them from `HEAD` and rebuilding; proof: the "Upstream changed the bundle" scenario passes in CI
- [ ] 2.3 Record `rebuild_pending`, and let `operation continue` rebuild it and push the remaining patches; proof: the "A source conflict in the owner" and "A failing rebuild stops the rebase" scenarios pass in CI
- [ ] 2.4 Refresh an owner only with its derived files, and stop with `derived_rebuild_dirty` on any other change; proof: the "The install rewrites a lockfile" scenario passes in CI
- [ ] 2.5 Add `rebuilt_patches` to the rebase result and name each rebuilt patch in the rebase report; proof: the rebase lifecycle test reads `result.rebuilt_patches`

## 3. Freshness and rebuild

- [ ] 3.1 Rebuild each owner in a disposable clone at its commit and report `derived_stale` and `derived_rebuild_failed`; proof: the "A lower patch changed a source" scenario passes in CI
- [ ] 3.2 Record passing results by owner commit and rebuild hash, skip them in staged checks, and add `check --no-cache`; proof: the "An unchanged owner is not rebuilt again" scenario passes in CI
- [ ] 3.3 Add `patch rebuild PATCH` with plan mode and the refusals; proof: both `patch rebuild` scenarios pass in CI, and the `patch rebuild` case in `every_mutation_has_a_typed_non_mutating_failure_case` passes

## 4. Surfaces and release

- [ ] 4.1 Update `src/instructions.md`, the operator templates and both renders, `README.md`, `AGENTS.md`'s product invariants, and `examples/fork.yaml`; proof: both `operator_docs` tests pass in CI, and `rg -n 'derived' skills/forkctl/SKILL.md src/instructions.md` names the owner rule and `patch rebuild`
- [ ] 4.2 Add a patch changeset fragment naming the new field, flags, variables, command, and finding codes; proof: `mise run ver check` passes
- [ ] 4.3 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
