# Design

## Decisions

1. **Canonical base is the merge base with the upstream branch.** `rebase` and `check` already use this definition, so bootstrap adopts it and a tag needs no new manifest field. The alternative was to treat the tag commit as canonical and teach `check` a tag mode. It lost because it would give the manifest two meanings of canonical and change the `check` contract for every existing downstream.
2. **The tag's own commits are pre-stack drift governed by `--allow-base`.** That contract already exists for commits between the canonical base and the stack base. The alternative, exempting the whole tag diff automatically, lost because an operator would no longer see or bound what the release commit changes.
3. **Bootstrap settles both before any mutation.** The push, the manifest, and the StGit state come after the drift check, so a refused bootstrap leaves the clone as it was, apart from the fetched upstream tracking ref. Keeping the check at the end, after the mutation, was the old behaviour that produced a half-initialised clone.
4. **`--upstream-ref` stays a branch.** A tag cannot serve as the moving reference that `rebase` and `check` measure against. A tag is chosen as a base with `--base` and later with `rebase --onto`.

## Automatic behaviour

Bootstrap now fetches the upstream branch into `refs/remotes/<upstream>/<branch>`. This is the same fetch `rebase` performs. There is no override: the canonical base cannot be computed without it. The drift check is governed by `--allow-base` at bootstrap and by `contract edit --allow-base` later.
