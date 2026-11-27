# Design

## Context

`rebase` hands the replay to one `stg rebase --merged <commit>`, and a conflict stops it until the operator resolves the files and runs `operation continue`.
The [`derived-paths`][derived-paths] design adds a derived patch on top of the bookkeeping patch, built in a clone from the tree below it and keyed by its input hash and rebuild hash.

## Decisions

### 1. Rebase never conflicts on an output

`rebase` deletes the derived patch from the series before the replay; the recovery tag already preserves it with the old tip.
It replays every other patch as today, so no patch it replays touches an output.
After the last push it enters phase `deriving`, derives once, then regenerates bookkeeping and writes the report.

A failing build stops the rebase in phase `deriving` with the finding.
`operation continue` derives again, for a cause outside the manifest such as the network; `operation abort` restores the old stack from the recovery tag.
A change to `rebuild` needs `operation abort`, `contract edit`, and a new rebase, because an operation in flight reads the manifest snapshot taken at its start.

The report names the input hash, the rebuild hash, and whether the result came from the cache.
`rebase --verify-base` also builds at the new base and reports whether upstream's committed outputs equal that build.
A difference there means upstream's outputs do not reproduce from its own sources, so the derived patch will carry that noise.

Alternative considered: replay the derived patch and resolve its conflicts by taking either side and rebuilding.
It lost because the replay of the derived patch carries nothing a build at the top does not produce, and every conflict it raised would be resolved the same way.

### 2. Protocol

The protocol stays at version 2: the `deriving` phase, the `--verify-base` argument, and the rebase result's `derived` field are additions.

[derived-paths]: ../derived-paths/design.md
