# Tasks

Compile, format, lint, and test tasks run in CI with `mise run verify`.

## 1. Names

- [x] 1.1 Rename the manifest, journal, protocol, and CLI fields and types; proof: `rg -n -i kind` over `src`, `tests`, docs, and templates matches only the upgrade path, its fixtures, ast-grep rule keys, and library identifiers
- [x] 1.2 Move manifest schema and protocol version to 2; proof: `manifest::SCHEMA` and `PROTOCOL_VERSION` are 2, and `examples/fork.yaml` declares `schema: 2`
- [ ] 1.3 Regenerate the usage snapshot and both operator renders; proof: `cli__usage_spec_is_the_mounted_fork_grammar` and both `operator_docs` tests pass in CI

## 2. Upgrade

- [ ] 2.1 Rewrite a schema 1 manifest; proof: `manifest_upgrade` unit tests compare the YAML upgrade of `tests/testdata/upgrade/schema-1.yaml` byte for byte with `schema-2.yaml`, and the JSON upgrade with its typed form
- [ ] 2.2 Add `forkctl upgrade` and the schema 1 hint; proof: `upgrade_rewrites_a_schema_1_manifest_in_place` passes in CI
- [ ] 2.3 Refuse an unknown schema without writing; proof: the `upgrade` case in `every_mutation_has_a_typed_non_mutating_failure_case` passes in CI
- [x] 2.4 Track the removal of the upgrade; proof: horizon row FRK-037 names it
- [ ] 2.5 Refuse an uncommitted rewrite after a failed refresh; proof: `upgrade_after_a_failed_refresh_refuses_the_uncommitted_rewrite` passes in CI
- [ ] 2.6 Name the undo when writing the rewrite fails; proof: `upgrade_stopped_while_writing_names_the_undo` passes in CI

## 3. Release

- [x] 3.1 Add a changeset fragment naming every old and new flag, variable, and stored key; proof: `mise run ver check` passes
- [ ] 3.2 Run the full gate; proof: `mise run verify` passes in CI, and `openspec validate --all --strict` passes
