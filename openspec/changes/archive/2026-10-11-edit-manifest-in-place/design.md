# Design

## Context

[`App::discover`][mod] reads the tracked manifest bytes, treats `NotFound` as "no manifest yet", and parses the rest through [`ManifestFormat::parse`][codec].
Seven call sites change the typed `Manifest` and then call `write_manifest`, which serializes the whole value:

1. `init` bootstrap writes a new file.
2. `contract edit` replaces `contracts`.
3. `patch edit` replaces one patch, or moves it when its kind changes.
4. `patch refresh` inserts a new patch at its insertion index.
5. `patch disable` and `patch drop` remove a patch and append a disabled record or a history event; `patch enable` removes the disabled record, inserts the patch, and appends a history event.
6. `publish --set-default` sets `downstream.publish`, which is absent when it is `rewrite`.
7. `rebase` sets `base.target`, `base.stack`, and `base.canonical`, removes dropped patches, and may append a history event.

Operation-scoped commands (`rebase`, `patch disable`, `patch drop`, `patch enable`, and a resumed `patch refresh`) take their manifest from the Git-private JSON snapshot written when the operation starts, not from the tracked file.

ctl-core's `input` feature parses text into a typed value with line and column problems, and refuses duplicate keys, merge keys, and booleans other than `true` and `false`.
It allows anchors and aliases under an alias-ratio guard.
[yamled][yamled] 0.0.5 edits a parsed document by path (`replace`, `insert`, `insert_item`, `push`, `remove`) and leaves every other byte alone.

## Goals and non-goals

Goals:

1. A command that changes an existing YAML manifest leaves comments, key order, quoting, and blank lines outside the changed nodes byte for byte.
2. The YAML reader is ctl-core's, with forkctl's own bans removed.
3. The written file always parses back to the typed manifest the command intended.

Non-goals:

1. Reader limits. They are serde-saphyr's library defaults, overridable through ctl-core in code and in `.ctl/` configuration. ctl-core owns that API; forkctl passes no budget of its own.
2. Keeping formatting inside a node the command replaces.
3. Any change to JSON manifests, the JSON snapshot, or protocol version 1.

## Decisions

### 1. One closed edit type, applied to both the typed manifest and the document

A new `ManifestEdit` enum names every kind of change a command makes:

| Edit | Typed effect | Document effect |
|:--|:--|:--|
| `SetContracts(Contracts)` | replace `contracts` | replace or insert the `contracts` key |
| `SetPublishMode(PublishMode)` | set `downstream.publish` | replace, insert, or (for `rewrite`) remove `downstream.publish` |
| `SetBase { target, stack, canonical }` | set the three fields | replace each scalar under `base` |
| `InsertPatch { index, patch }` | insert into `patches` | insert an item at `index` |
| `ReplacePatch { name, patch }` | replace that patch | replace that item |
| `RemovePatch { name }` | remove that patch | remove that item |
| `PushDisabled(DisabledPatch)` | append to `disabled_patches` | push an item, inserting the key if absent |
| `RemoveDisabled { name }` | remove that record | remove that item |
| `PushHistory(HistoryEvent)` | append to `history` | push an item, inserting the key if absent |

Both appliers match the enum exhaustively, so a new edit kind does not compile until both sides handle it.
Commands that validate before writing (`contract edit`, `patch edit`, `publish --set-default`) apply the edits to a clone, validate it, and then commit the same edits.
A kind-changing `patch edit` is `RemovePatch` then `InsertPatch`.

Alternative considered: diff the old and new typed manifests and turn the difference into document edits.
It lost because a generic diff of lists cannot tell a moved patch from a removed and re-added one, and it hides which command meant which change.

Alternative considered: keep the full rewrite and copy comments across.
It lost because no YAML serializer here can re-attach comments to nodes it regenerated.

### 2. The written text must parse back to the intended manifest

`edit_manifest` applies the edits to the document, parses the edited text through the same reader, and compares it with the typed manifest after the same edits (as `serde_json::Value`).
A mismatch fails with `manifest_invalid`, and nothing is written.
The reachable cause is the manifest's own form, such as an alias that shares an edited value, so the error names the file and says to expand it by hand.

This is the proof that an edit and its document effect agree.
It has no override, because writing a manifest that does not match the command's result is never correct.

### 3. Operations edit the source text they started from

When an operation starts, forkctl already writes the JSON snapshot.
It now also copies the tracked manifest bytes to `$(git rev-parse --git-path forkctl/manifest.source)`, and `load_operation_manifest` reads both.
An operation's edits apply to that text, which parses to the same manifest as the snapshot by construction.
An operation that an earlier forkctl started has no such snapshot. Its write fails with `operation_conflict` and says to run `operation abort --yes` and start the command again; there is no fallback to a full rewrite, as for every older forkctl contract.

Alternative considered: edit whatever text is on disk when the operation writes.
It lost because a stack in flight may have popped or changed the file, so the edits would apply to a document that no longer matches the snapshot.

### 4. Aliases are refused at the edited node

yamled edits the text it is given; an edit that lands on an alias, or inside a node another node aliases, would silently change both or neither.
forkctl refuses such an edit with `manifest_invalid` and says to expand the anchor or alias by hand.
No separate alias check is needed: an alias whose anchor an edit removed no longer parses, so yamled refuses the edit, and an anchored value that changes in more places than the edit fails the parse-back comparison of decision 2.
The same refusal covers a flow collection yamled cannot edit in place.
Reading a manifest with anchors and aliases is allowed.

Alternative considered: expand every alias before editing.
It lost because it rewrites nodes the command did not change, which is the behaviour this change removes.

### 5. Bootstrap and JSON keep full serialization

`init` writes a file that does not exist yet, so the serde-saphyr serializer stays, now with only its `serialize` feature.
JSON manifests have no comments, so they keep a full `serde_json` rewrite after the same typed edits.

### 6. Missing manifest stays forkctl's check

ctl-core's `Input::read` folds I/O errors into a message, so `NotFound` is not matchable there.
forkctl keeps reading the bytes itself, keeps its `NotFound` branch, checks UTF-8, and calls `Input::new(path, text).parse()`.

## Risks

1. Moving serde-saphyr from 1.0.0 to the version ctl-core resolves may change what `init` writes. The YAML round-trip test and the lifecycle fixtures prove the bootstrap output.
2. Dropping the budget raises the accepted size, depth, and node count to serde-saphyr's defaults. A repository that wants tighter limits sets them through ctl-core's `.ctl/` configuration.
3. A new edit kind added later without a document effect cannot compile, but a wrong document effect can. Decision 2 catches it at write time, and each edit has a test that checks the bytes outside the change.

[codec]: ../../../../src/manifest_codec.rs
[mod]: ../../../../src/app/mod.rs
[yamled]: https://github.com/victor-software-house/yamled
