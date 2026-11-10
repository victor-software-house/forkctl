## ADDED Requirements

### Requirement: A patch declares derived paths and their rebuild

A patch SHALL accept an optional `derived` record with `paths`, a non-empty list of globs in the `scope` grammar, and `rebuild`, a non-empty list of shell commands.
forkctl SHALL report `manifest_invalid` when a `derived.paths` glob is not matched by the same patch's `scope`.

#### Scenario: Create a patch with derived paths

- **WHEN** the operator runs `forkctl patch create dist -l source -p P -u not-submitted -d D -s 'dist/**' -D 'dist/**' -R 'sh build.sh'`
- **THEN** the active draft's metadata has `derived.paths` `["dist/**"]` and `derived.rebuild` `["sh build.sh"]`

#### Scenario: A derived path outside the scope

- **WHEN** `patches/fork.yaml` declares a patch with `scope: [src/**]` and `derived.paths: [dist/**]`, and the operator runs `forkctl --format json status`
- **THEN** the command fails and `error.code` is `manifest_invalid`

#### Scenario: Clear the declaration

- **WHEN** the operator runs `forkctl patch edit dist --clear-derived`
- **THEN** the manifest entry for `dist` has no `derived` key

### Requirement: Each derived path has one owner

`forkctl check` SHALL report `derived_path_shared` when derived globs of two patches match the same tracked file.
It SHALL report `derived_path_foreign` when a commit of a patch other than the owner touches a file that the owner's derived globs match.

#### Scenario: Another patch touches the bundle

- **WHEN** patch `dist` declares `derived.paths: [dist/**]`, patch `fix` below it changes `dist/index.js`, and the operator runs `forkctl --format json check`
- **THEN** the command fails and a finding has code `derived_path_foreign` and subject `fix`

### Requirement: Rebase rebuilds each owner after pushing it

`forkctl rebase` SHALL push the series one patch at a time.
After it pushes a patch that declares `derived`, it SHALL run the `rebuild` commands in order in the worktree, stage every file the derived globs match, and refresh that patch.
The rebase result SHALL list the patch in `rebuilt_patches`.

#### Scenario: Upstream changed only sources

- **WHEN** the stack holds `src` and `dist`, `dist` declares `derived` with a rebuild that writes `dist/out.txt` from `src/in.txt`, upstream changes another line of `src/in.txt`, and the operator runs `forkctl rebase -o refs/heads/main`
- **THEN** the command succeeds, `dist/out.txt` at the new tip equals the rebuild of `src/in.txt` at the new tip, and `result.rebuilt_patches` is `["dist"]`

### Requirement: A conflict in derived paths alone resolves by rebuilding

When pushing an owner conflicts and every unmerged path matches its derived globs, `forkctl rebase` SHALL restore those paths from `HEAD`, rebuild and refresh the owner, and continue without stopping.

#### Scenario: Upstream changed the bundle

- **WHEN** upstream changes `dist/out.txt` and `src/in.txt`, the owner `dist` changed `dist/out.txt` too, and the operator runs `forkctl rebase -o refs/heads/main`
- **THEN** the command succeeds with no conflict, and `dist/out.txt` at the new tip equals the rebuild of `src/in.txt` at the new tip

### Requirement: Continue finishes a stopped rebuild and pushes the rest

When pushing an owner conflicts in a path outside its derived globs, `forkctl rebase` SHALL stop in phase `conflict` with that patch recorded as `rebuild_pending`.
`forkctl operation continue` SHALL rebuild and refresh the `rebuild_pending` patch, push every remaining unapplied patch through the same rebuild rule, and then finish the rebase.

#### Scenario: A source conflict in the owner

- **WHEN** a rebase stops on owner `dist` with a conflict in `src/in.txt`, the operator resolves it, runs `stg add --update` and `stg refresh`, and runs `forkctl operation continue`
- **THEN** the command succeeds, the bookkeeping patch is on top, and `dist/out.txt` at the new tip equals the rebuild of `src/in.txt` at the new tip

#### Scenario: A failing rebuild stops the rebase

- **WHEN** an owner's rebuild command exits 3 during `forkctl rebase`
- **THEN** the command fails, `operation status` reports phase `conflict`, and the error names the command and its exit status 3

### Requirement: Check reports a stale derived path

A complete `forkctl check` SHALL run each owner's `rebuild` commands in a disposable clone at the owner's commit.
It SHALL report `derived_stale` when any file the derived globs match differs from that commit afterwards, and `derived_rebuild_failed` when a rebuild command exits non-zero.

#### Scenario: A lower patch changed a source

- **WHEN** patch `src` below owner `dist` changes `src/in.txt` and is refreshed, and the operator runs `forkctl --format json check`
- **THEN** the command fails and a finding has code `derived_stale` and subject `dist`, with a message that names `mise run fork patch rebuild dist`

### Requirement: Patch rebuild refreshes an owner in place

`forkctl patch rebuild PATCH` SHALL rebuild and refresh the owner in the worktree, push the patches above it again, and run the full check.
It SHALL refuse a patch with no `derived` record, a dirty worktree, an active patch, or an operation in flight.
`forkctl patch rebuild PATCH -n` SHALL report the plan and change nothing.

#### Scenario: Rebuild after a source change

- **WHEN** `forkctl check` reports `derived_stale` for `dist` and the operator runs `forkctl patch rebuild dist`
- **THEN** the command succeeds, the bookkeeping patch is on top, and `forkctl check` succeeds

#### Scenario: A patch without the declaration

- **WHEN** the operator runs `forkctl --format json patch rebuild src` and `src` declares no `derived`
- **THEN** the command fails, nothing changes, and `error.message` names `-D/--derived`
