# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Dependencies

- [x] 1.1 Pin [ctl-core][ctl-core] 0.6.10 with feature `input`, yamled 0.0.5, and serde-saphyr 1.3.0 with only `serialize`; proof: `cargo tree -i serde-saphyr` and `cargo tree -i yamled` on the build host each show one version
  - 2026-10-11 (UTC): on a Linux build host, `cargo tree -i serde-saphyr -e normal` shows `serde-saphyr v1.3.0` and `cargo tree -i yamled -e normal` shows `yamled v0.0.5`.

## 2. Reader

- [x] 2.1 Parse YAML through `ctl_core::input` in `manifest_codec`, and remove forkctl's budget, alias, anchor, and indentation options and `MAX_YAML_BYTES`; proof: `rejects_ambiguous_yaml_features` refuses duplicate keys, merge keys, and a second document with no `max_documents` override, and `accepts_anchors_and_aliases` passes
  - 2026-10-11 (UTC): both pass on the build host. serde-saphyr refuses the second document on its own when parsing into one type, so forkctl sets no limit.
- [x] 2.2 Keep the source text beside the typed manifest in `App`, and snapshot it at operation start as `forkctl/manifest.source`; proof: `lower_patch_refresh_conflict_can_continue_from_typed_journal` asserts the tracked manifest is absent before `operation continue`, and the manifest comment survives it
  - 2026-10-11 (UTC): passes on the build host.
- [x] 2.3 Fail with `operation_conflict` naming `operation abort --yes` when an operation has no manifest.source snapshot; proof: `continue_without_a_manifest_source_snapshot_names_its_recovery` passes
  - 2026-10-11 (UTC): passes on the build host; before this fix the same write failed as an internal error.

## 3. Edits

- [x] 3.1 Add `ManifestEdit` with typed and yamled appliers, both matched exhaustively, and the parse-back comparison; proof: the `manifest_edit` unit tests compare the edited text with the original outside the edited node
  - 2026-10-11 (UTC): seven `manifest_edit` tests pass on the build host. Publish mode and a patch removed then reinserted restore the original bytes exactly.
- [x] 3.2 Refuse an edit through an alias with `manifest_invalid`; proof: `an_edit_through_an_alias_is_refused` passes
  - 2026-10-11 (UTC): passes on the build host.
- [x] 3.3 Move `contract edit`, `patch edit`, `patch refresh`, the disable, remove, and enable transitions, `publish --set-default`, and `rebase` onto edits, and remove `manifest_mut`; proof: `grep -rn manifest_mut src` finds nothing, and lifecycle tests keep a manifest comment through disable, enable, remove, and rebase
  - 2026-10-11 (UTC): `grep` finds nothing; `yaml_manifest_preserves_disable_enable_and_remove_history` and `rebase_publish_and_fresh_clone_hydrate_exact_recovery` pass on the build host with the comment assertions.
- [x] 3.4 Keep the full serializer for bootstrap and JSON; proof: `yaml_and_json_round_trip_the_same_manifest_deterministically` and `bootstrap_and_fresh_clone_hydration_are_idempotent` pass
  - 2026-10-11 (UTC): both pass on the build host.

## 4. Docs and gate

- [x] 4.1 Update the YAML paragraph in `README.md` and the manifest line in `AGENTS.md`; the operator templates state no YAML rule; proof: `grep -n "aliases/anchors" README.md` finds nothing, and both `operator_docs` tests pass unchanged
  - 2026-10-11 (UTC): `grep` finds nothing; both `operator_docs` tests pass on the build host.
- [x] 4.2 Add a changeset fragment; proof: `.changeset/manifest-edits-in-place.md` is a `forkctl: patch` fragment
- [x] 4.3 Run the full gate; proof: `mise run verify` and `openspec validate --all --strict` pass on the build host
  - 2026-10-11 (UTC): `mise run verify` runs 126 of 126 tests on the build host; `openspec validate --all --strict` passes.

[ctl-core]: https://github.com/victor-software-house/ctl-core
