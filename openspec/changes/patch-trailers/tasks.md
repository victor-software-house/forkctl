# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`, and in CI.

## 1. Manifest

- [ ] 1.1 Add `Patch.trailers` and `Trailer`, validate them, and append them in `Patch::message`; proof: the `manifest` unit tests `message_appends_trailers_after_drop_when` and `rejects_invalid_trailers` pass
- [ ] 1.2 Round-trip trailers through both codecs and the in-place YAML edit; proof: `yaml_and_json_round_trip_trailers` and `replacing_a_patch_writes_and_removes_its_trailers` pass

## 2. Commands

- [ ] 2.1 Add `--trailer` to `patch create` and `patch edit`, `--clear-trailers` to `patch edit`, and the protocol fields; proof: `patch_edit_replaces_and_clears_trailers` passes and the usage snapshot shows both flags
- [ ] 2.2 Verify declared trailers in `check`; proof: `check_verifies_declared_trailers` passes
- [ ] 2.3 List trailers in `patch show` and `PATCHES.md`; proof: `check_verifies_declared_trailers` reads the ledger row, and every existing ledger test still passes unchanged

## 3. Release

- [ ] 3.1 Update `README.md`, `AGENTS.md`, and the operator templates with their renders; proof: both `operator_docs` tests pass
- [ ] 3.2 Add a patch changeset fragment; proof: `mise run ver check` passes
- [ ] 3.3 Run the full gate; proof: `mise run verify` passes on the build host and in CI, and `openspec validate --all --strict` passes
