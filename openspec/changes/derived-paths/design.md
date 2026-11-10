# Design

## Context

`rebase` hands the whole replay to one `stg rebase --merged <commit>`, and any conflict stops it with the operator's next actions in the journal.
`operation continue` then requires every patch to be applied and the bookkeeping patch on top, so the operator pushes the rest by hand with `stg goto`.
Declared checks run in a disposable clone of the applied stack or of one patch's commit, and never touch the worktree.

## Decisions

### 1. The declaration

```yaml
patches:
  - name: dist
    layer: source
    scope:
      - dist/**
    derived:
      paths:
        - dist/**
      rebuild:
        - aube install --frozen-lockfile
        - aube run package
```

`paths` are globs with the same grammar as `scope`, and each must be covered by the patch's own scope, since the rebuild writes them into that patch.
`rebuild` is a list of commands that run in order through `sh -c`, with the repository root as the working directory, and stop at the first that exits non-zero.
A list keeps each step a separate command with its own exit status and diagnostics.

Alternative considered: one `rebuild` string, like a check's `run`.
It lost because a rebuild usually installs dependencies and then builds, and one string forces the two into a shell chain whose failing step the finding cannot name.

Alternative considered: a named list of derived records per patch, like `checks`.
It lost because no known fork needs two rebuilds in one patch, and a patch whose outputs need two separate builds can split into two owner patches.

### 2. One owner per derived path

forkctl validates three rules when it reads the manifest or checks the repository:

1. Each `derived.paths` glob of a patch is matched by that patch's `scope`, or the manifest is invalid (`manifest_invalid`).
2. No two patches declare derived globs that match the same tracked file (`derived_path_shared`).
3. No patch other than the owner has a commit that touches a file matched by another patch's derived globs (`derived_path_foreign`).

With one owner, the derived files change in exactly one commit of the stack, so only that patch can conflict on them, and its rebuild sees every source change below it.
A fork whose bundle depends on several source patches puts the owner above all of them, often as a patch that holds nothing but the derived files.

Alternative considered: let every patch that touches a derived path rebuild it after its own push.
It lost because each such patch would carry its own copy of a 5 MB file, every one of them would conflict with the next on every rebase, and the stack's diff would be unreadable.

### 3. Replay pushes one patch at a time

`rebase` runs `stg rebase --merged --nopush <commit>`, then pushes the series with `stg push --merged <patch>`, one patch at a time.
After each push of an owner, forkctl rebuilds it in the worktree: it runs the `rebuild` commands, stages every path the derived globs match, added and deleted files included, and runs `stg refresh` limited to those paths.
When the rebuild also changed a tracked file outside the derived globs, such as a lockfile an install rewrote, forkctl stops with `derived_rebuild_dirty`, names each such file, and refreshes nothing.
The operator restores those files or adds them to `derived.paths`, then runs `operation continue`.
A rebuild therefore never folds an unrelated change into the owner.
The worktree is the right place because it holds the operator's installed dependencies, which a disposable clone lacks.

When a push conflicts, forkctl reads the unmerged paths.

1. When the pushed patch is an owner and every unmerged path matches its derived globs, forkctl restores each from `HEAD` (the base plus the patches below), rebuilds, refreshes, and pushes on.
2. Otherwise it records the patch in the journal as `rebuild_pending` when it is an owner, sets phase `conflict`, and stops with next actions `resolve conflicts`, `stg add --update`, `stg refresh`, and `forkctl operation continue`.

`operation continue` for a rebase rebuilds the `rebuild_pending` patch when it is on top, then pushes each remaining unapplied patch through the same loop, then finishes as today.
A continue after the operator already pushed every patch, the old procedure, still works: there is nothing left to push.

A rebuild command that fails stops the rebase in phase `conflict` with the command, its exit status, and its first diagnostic line, and the patch stays `rebuild_pending`.
The operator fixes the cause and runs `operation continue`, or runs `operation abort`.

Alternative considered: keep `stg rebase` and pass `--exec` for the rebuild.
It lost because `--exec` runs after every patch rather than the owner, cannot stage or refresh, and rolls back the whole rebase on failure, which discards the conflict resolutions already made.

Alternative considered: resolve the conflict by taking the patch's own copy of the derived files.
It lost because that copy was built from the old upstream sources, so the result would pass review and ship a stale bundle.

### 4. The freshness check

A complete `check`, and every command that runs it, materializes each owner's commit in a disposable clone, runs the `rebuild` commands there, and reports `derived_stale` when `git status --porcelain` over the derived globs is not empty.
The clone holds no ignored files, so the `rebuild` commands must install what they need; the example installs dependencies first for that reason.
A rebuild command that fails reports `derived_rebuild_failed` with the command, its exit status, and its first diagnostic line.
A derived glob that matches no tracked file at the owner's commit reports `check_glob_stale`, as a check's glob does.

The finding's next action is `mise run fork patch rebuild <patch>`.

A passing result is recorded under `$(git rev-parse --git-path forkctl/derived/)`, keyed by the owner's commit id and a hash of its `rebuild` list.
A commit id fixes the whole tree, so a later check with the same key skips the rebuild; only a changed owner commit or changed commands rebuild again.
A failing result is never recorded.
A staged check (`check -s`), the pre-commit hook, does not run the freshness check, so editing stays fast; the complete check of `patch finish`, `rebase`, and the pre-push hook does.
`check --no-cache` ignores the records, which is the override when a build reads something outside the tree, such as the network.

Alternative considered: rebuild on every complete check.
It lost because one rebuild of the mise-action bundle installs every dependency and runs rollup, and the pre-push hook would pay that on every push of an unchanged owner.

Alternative considered: leave freshness to an ordinary declared check that the operator writes.
It lost because every fork that declares derived paths would write the same check, and a fork that forgot it would ship a bundle that does not match its sources.

Alternative considered: check at the top of the stack.
It lost because the rebuild runs at the owner's commit, so a check at the top would report a change above the owner that no rebuild can take in.
A patch that changes the build's inputs belongs below the owner, and the skill says so.

### 5. `patch rebuild`

`patch rebuild PATCH` requires a clean worktree, no active patch, and no operation in flight.
It records an operation, moves the stack to the owner with `stg goto`, rebuilds and refreshes as in section 3, pushes the patches above it again, and runs the full check before it ends.
A conflict while pushing the patches above stops in phase `conflict`, and `operation continue` and `operation abort` own it, as for a rebase.
`patch rebuild -n` reports the plan and changes nothing.

Alternative considered: no command, with the finding naming `stg goto`, the rebuild commands, and `patch refresh --rewrite-below`.
It lost because that is four manual steps whose order matters, repeated after every source change in a lower patch.

### 6. Automatic behaviour

The rebuild during `rebase` and the freshness check run only for a patch that declares `derived`.
The override is the declaration: `patch edit PATCH -R COMMAND` replaces the commands, and `-D GLOB` replaces the paths.
The disable path is `patch edit PATCH --clear-derived`, which returns the patch to ordinary conflict handling and drops its freshness check.

### 7. Protocol and manifest

The `derived` field is optional and skipped when absent, so every existing schema 2 manifest reads unchanged, and the schema number stays 2.
An older forkctl rejects a manifest that declares `derived`, as `deny_unknown_fields` requires; that is the expected effect of a new field, and the changeset names the first release that reads it.
The protocol stays at version 2: the `patch rebuild` request, the `rebuilt_patches` result field, the journal's `rebuild_pending` field, and the finding codes are additions.

## Non-goals

1. Generated exports of an owner patch keep showing the derived files as Git shows them; a fork that marks them `-diff`, as the mise-action fork does, sees one line per file.
2. Detecting which source files a build reads. The single-owner rule and the freshness check make that unnecessary.
3. Sandboxing rebuild commands. Like checks, they are trusted user-level code.
   During `rebase` they run the new upstream's build in the operator's worktree, which is the step the operator would run by hand to resolve the same conflict.
   `rebase -n` lists the rebuild commands before anything runs, and a fork that will not run upstream's build there omits `derived`.
