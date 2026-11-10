# Design

## Context

forkctl replays a stack of StGit patches onto an exact upstream target and regenerates its own documents, the ledger and the patch exports, into the bookkeeping patch on every refresh.
`rebase` hands the replay to one `stg rebase --merged <commit>`, and a conflict stops it until the operator resolves the files and runs `operation continue`.
Declared checks run in a disposable clone of the applied stack or of one patch's commit, and never touch the worktree.

A committed build output breaks the model in two ways.
It sits in a hand patch, so every upstream release that changes the bundle conflicts in a file nobody can read.
And nothing ties it to the sources it was built from, so it goes stale silently.

## Decisions

### 1. Derived files are a function of the stack, not content of a patch

```yaml
derived:
  outputs:
    - dist/**
  rebuild:
    - aube install --frozen-lockfile
    - aube run package
```

`outputs` are globs in the `scope` grammar.
`rebuild` is a list of commands that run in order through `sh -c`, from the clone's root, and stop at the first that exits non-zero.
`inputs` is optional and narrows what counts as an input (section 4).

A manifest declares at most one `derived` block.
forkctl reports `manifest_invalid` when an output glob also matches a forkctl document (the manifest, the ledger, or the exports directory), or when `inputs` and `outputs` share a glob.

Alternative considered: a `derived` record on one patch, which rebuilds the outputs after that patch replays.
It lost because the outputs then stay content of a hand patch: the patch needs an ownership rule, a rebuild in the operator's worktree in the middle of a replay, and a per-patch freshness check, while the bundle that ships is still built from the tree at the top.

Alternative considered: a git merge driver for the outputs, as [npm-merge-driver][npm] does for lockfiles.
It lost because a driver runs once per conflicting file, before the other files of the patch are merged, so it would build from a half-merged tree.

### 2. One derived patch, on top, owned by forkctl

When `derived` is declared, the series ends with a patch named `derived` above the bookkeeping patch.
Its diff against the patch below holds only files the output globs match, and its commit carries two trailers:

```text
Derived-Inputs: sha256:<input hash>
Derived-Rebuild: sha256:<rebuild hash>
```

No other patch may touch an output; a complete check reports `derived_path_foreign` with the patch's name when one does.
So below the derived patch every output is upstream's copy, and the derived patch is the fork's whole difference in them.
The derived patch has no export, as a tooling patch has none, and the ledger lists the declaration.

The invariant that the bookkeeping patch is on top becomes: the bookkeeping patch is on top, or directly below the derived patch.

Alternative considered: put the outputs in the bookkeeping patch.
It lost because the bookkeeping patch holds the manifest, so the hashes it would record are hashes of a tree that contains them.

### 3. Build in a clone, once per input state

`derive` computes the input hash and the rebuild hash from the tree below the derived patch.
When the cache holds a result for both, forkctl uses it.
Otherwise it clones that tree as a check does, with no origin remote, deletes every file the output globs match, runs `rebuild`, and reads every file the output globs match.

1. The outputs are deleted before the build because they are not inputs. Upstream's copy, which the tree below the derived patch holds, could otherwise feed the build or survive it, such as a chunk file the fork's build no longer emits, and the result would depend on a file the input hash does not cover. A file the build must find in place is not generated, and belongs outside `outputs`.
2. The build reads only tracked files, so it never sees the operator's ignored files, and the commands install what they need.
3. When the build changed a tracked file outside the outputs, such as a lockfile an install rewrote, forkctl reports `derived_rebuild_dirty`, names each file, and writes nothing.
4. A command that exits non-zero reports `derived_rebuild_failed` with the command, its exit status, and its first diagnostic line, and writes nothing.

On success forkctl writes the outputs into the worktree, stages them with deletions, and replaces the derived patch with one commit that carries the two trailers.
The result is cached under `$(git rev-parse --git-path forkctl/derived/)` as the output tree's object id, kept reachable by a ref under `refs/forkctl/derived/`, keyed by both hashes.

The build runs once per input state, at the top of the stack.
It never runs once per commit or once per patch that touches an input, because the outputs that ship are those built from the top.

Alternative considered: build in the worktree, which already holds installed dependencies.
It lost because the worktree holds files the commit does not, so a build there can pass and still not be reproducible from the commit, and because it runs upstream's build scripts beside the operator's uncommitted work.

### 4. The input hash

The input hash is SHA-256 over the sorted lines `<mode> <object id> <path>` of every tracked file that counts as an input.
It reads object ids from `git ls-tree -r`, so it reads no file content.

With no `inputs`, every tracked file counts, except files the output globs match and forkctl's own documents.
A fork that lists `inputs` counts only those, which rebuilds less often at the risk of a missed input: the [Bazel sandboxing page][sandbox] describes that failure.
The default makes the common case right: an upstream change to a file no patch touches, such as a build configuration, still marks the outputs stale.

The rebuild hash is SHA-256 over the `rebuild` commands, each followed by a newline.

Alternative considered: hash the whole tree.
It lost because every manifest or ledger rewrite, such as a patch edit that changes only a purpose, would then rebuild the bundle.

### 5. Rebase never conflicts on an output

`rebase` deletes the derived patch from the series before the replay; the recovery tag already preserves it with the old tip.
It replays every other patch as today, so no patch it replays touches an output.
After the last push it enters phase `deriving`, derives once, then regenerates bookkeeping and writes the report.

A failing build stops the rebase in phase `deriving` with the finding.
`operation continue` derives again, for a cause outside the manifest such as the network; `operation abort` restores the old stack from the recovery tag.
A change to `rebuild` needs `operation abort`, `contract edit`, and a new rebase, because an operation in flight reads the manifest snapshot taken at its start.

The report names the input hash, the rebuild hash, and whether the result came from the cache.
`rebase --verify-base` also builds at the new base and reports whether upstream's committed outputs equal that build.
A difference there means upstream's outputs do not reproduce from its own sources, so the derived patch will carry that noise.

### 6. States and the everyday flow

The derived state is read from the stack with no build.

| State | Condition | Complete check |
|:--|:--|:--|
| `absent` | `derived` is declared and the top patch is not `derived` | fails `derived_absent` |
| `stale` | a trailer differs from the hash forkctl computes | fails `derived_stale` |
| `fresh` | both trailers match | passes |

1. `patch refresh` keeps its speed: it rebuilds nothing, and its final check reports a stale derived patch as a notice while the patch stays active.
2. `patch finish` derives before its final check, so a finished patch never leaves the outputs stale.
3. `forkctl derive` derives on request, records an operation, and supports `-n`.
4. The staged check of the pre-commit hook skips the derived state; the complete check of the pre-push hook and of `publish` fails on it.
5. `check --rebuild` also builds the input state in a clone and reports `derived_unreproducible` when the result differs from the derived patch; it ignores the cache.

Alternative considered: derive on every refresh.
It lost because one build of the mise-action bundle installs every dependency and runs rollup, which the editing loop would pay on every refresh.

### 7. Automatic behaviour

The build runs only when the operator starts `rebase`, `patch finish`, or `derive`, and only when `derived` is declared.
`rebase -n` and `derive -n` list the rebuild commands before anything runs.
The override is the declaration: `contract edit --derived-rebuild` replaces the commands, and `--derived-input` and `--derived-output` replace the globs.
The disable path is `contract edit --clear-derived`, followed by `patch remove derived` when the stack still holds the patch.

### 8. Protocol and manifest

The `derived` block is optional and skipped when absent, so every existing schema 2 manifest reads unchanged, and the schema number stays 2.
An older forkctl rejects a manifest that declares `derived`, as `deny_unknown_fields` requires, and the changeset names the first release that reads it.
The protocol stays at version 2: the `derive` request and result, the `deriving` phase, the rebase result's `derived` field, and the finding codes are additions.

## Non-goals

1. Sandboxing the build. Like checks, rebuild commands are trusted user-level code, and they run upstream's build scripts; the clone keeps them away from the worktree, not from the operator's account.
2. More than one `derived` block. A fork with two builds lists both in one `rebuild`.
3. Detecting undeclared inputs. The default counts every tracked file, and `check --rebuild` finds a build that reads something outside the tree.

[npm]: https://github.com/npm/npm-merge-driver/
[sandbox]: https://bazel.build/docs/sandboxing
