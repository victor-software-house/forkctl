## ADDED Requirements

### Requirement: The manifest declares derived outputs and their rebuild

The manifest SHALL accept one optional `derived` block with `outputs`, a non-empty list of globs, `rebuild`, a non-empty list of shell commands, and `inputs`, an optional list of globs.
forkctl SHALL report `manifest_invalid` when an output glob matches the manifest, the ledger, or a file under the exports directory.

#### Scenario: Declare the bundle

- **WHEN** the operator runs `forkctl contract edit --derived-output 'dist/**' --derived-rebuild 'sh build.sh'`
- **THEN** `patches/fork.yaml` holds `derived.outputs` `["dist/**"]` and `derived.rebuild` `["sh build.sh"]`, and `forkctl check` reports `derived_absent`

#### Scenario: An output over the manifest

- **WHEN** `patches/fork.yaml` declares `derived.outputs: [patches/**]` and the operator runs `forkctl --format json status`
- **THEN** the command fails and `error.code` is `manifest_invalid`

### Requirement: One derived patch on top holds every output

When `derived` is declared, the derived patch SHALL be the top patch, directly above the bookkeeping patch, and SHALL change only files the output globs match.
Its commit SHALL carry the trailers `Derived-Inputs` and `Derived-Rebuild`.
A complete `forkctl check` SHALL report `derived_path_foreign` when another patch's commit touches a file the output globs match.

#### Scenario: A hand patch touches the bundle

- **WHEN** `derived.outputs` is `[dist/**]`, patch `fix` changes `dist/index.js`, and the operator runs `forkctl --format json check`
- **THEN** the command fails and a finding has code `derived_path_foreign` and subject `fix`

### Requirement: Derive builds once per input state in a clean clone

`forkctl derive` SHALL compute the input hash and the rebuild hash from the tree below the derived patch.
Without a cached result for both, it SHALL clone that tree, delete every file the output globs match, run the `rebuild` commands in order, and commit the files the output globs match as the derived patch.
With a cached result, it SHALL commit that result and run no command.

#### Scenario: Derive the first time

- **WHEN** `derived` declares a rebuild that writes `dist/out.txt` from `src/in.txt`, and the operator runs `forkctl derive`
- **THEN** the top patch is `derived`, `dist/out.txt` equals the rebuild of `src/in.txt`, and `forkctl check` succeeds

#### Scenario: A stale upstream output does not survive

- **WHEN** upstream's tree holds `dist/old-chunk.js`, the rebuild does not write it, and the operator runs `forkctl derive`
- **THEN** `dist/old-chunk.js` is absent at the new tip

#### Scenario: The same state is not built twice

- **WHEN** the operator runs `forkctl derive`, then `forkctl operation abort` on a later rebase, then `forkctl derive` again on the same tree
- **THEN** the second derive runs no rebuild command

### Requirement: The input hash covers every tracked file by default

With no `inputs`, the input hash SHALL cover every tracked file below the derived patch except files the output globs match, the manifest, the ledger, and the exports.
With `inputs`, it SHALL cover only the tracked files those globs match.

#### Scenario: An upstream change outside every patch

- **WHEN** `derived` declares no `inputs`, a rebase moves onto an upstream that changes only `rollup.config.mjs`, which no patch touches
- **THEN** the rebase rebuilds the outputs

#### Scenario: A metadata edit does not rebuild

- **WHEN** the operator runs `forkctl patch edit fix -p 'New purpose'` and then `forkctl check`
- **THEN** the check reports the derived patch fresh

### Requirement: A failed or dirty build writes nothing

forkctl SHALL report `derived_rebuild_failed`, with the command and its exit status, when a rebuild command exits non-zero.
It SHALL report `derived_rebuild_dirty`, naming each file, when the build changes a tracked file outside the output globs.
In both cases it SHALL leave the worktree and the series unchanged.

#### Scenario: The install rewrites a lockfile

- **WHEN** the rebuild rewrites the tracked `aube-lock.yaml` and the operator runs `forkctl --format json derive`
- **THEN** the command fails, `error.code` is `derived_rebuild_dirty`, the message names `aube-lock.yaml`, and `git status --porcelain` is empty

### Requirement: Rebase never conflicts on an output

`forkctl rebase` SHALL delete the derived patch before the replay, replay every other patch, and derive once in phase `deriving` before it regenerates bookkeeping.
A failed build SHALL stop the rebase in phase `deriving`; `operation continue` SHALL derive again and finish the rebase.

#### Scenario: Upstream changed the bundle

- **WHEN** upstream changes `dist/out.txt` and `src/in.txt`, the stack holds a derived patch for `dist/**`, and the operator runs `forkctl rebase -o refs/heads/main`
- **THEN** the command succeeds with no conflict, `dist/out.txt` at the new tip equals the rebuild of `src/in.txt` at the new tip, and `result.derived.rebuilt` is true

#### Scenario: A failing build stops the rebase

- **WHEN** a rebuild command exits 3 during `forkctl rebase`
- **THEN** the command fails, `operation status` reports phase `deriving`, and the error names the command and its exit status 3

### Requirement: Check reads the derived state without building

A complete `forkctl check` SHALL report `derived_absent` when `derived` is declared and the top patch is not the derived patch, and `derived_stale` when a trailer differs from the hash forkctl computes.
It SHALL run no rebuild command, unless the operator passes `--rebuild`, which SHALL build in a clone and report `derived_unreproducible` when the result differs from the derived patch.

#### Scenario: A patch changed a source

- **WHEN** patch `src` changes `src/in.txt` and is finished without a derive, and the operator runs `forkctl --format json check`
- **THEN** the command fails and a finding has code `derived_stale`, with a message that names `mise run fork derive`

#### Scenario: A build that reads the clock

- **WHEN** the rebuild writes the current time into `dist/out.txt` and the operator runs `forkctl --format json check --rebuild`
- **THEN** the command fails and a finding has code `derived_unreproducible`

### Requirement: Finishing a patch derives

`forkctl patch finish` SHALL derive before its final check, and `forkctl patch refresh` SHALL run no rebuild command.

#### Scenario: Finish after a source change

- **WHEN** the active patch changes `src/in.txt`, the operator runs `forkctl patch refresh`, then `forkctl patch finish`
- **THEN** the refresh runs no rebuild command, the finish succeeds, and `forkctl check` reports the derived patch fresh

### Requirement: Rebase can verify upstream's outputs

`forkctl rebase --verify-base` SHALL also build at the new base and report in the result and the rebase report whether upstream's committed outputs equal that build.

#### Scenario: Upstream outputs that do not reproduce

- **WHEN** upstream's committed `dist/out.txt` differs from the rebuild of upstream's `src/in.txt`, and the operator runs `forkctl rebase -o refs/heads/main --verify-base`
- **THEN** the command succeeds and `result.derived.base_reproducible` is false
