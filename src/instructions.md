# forkctl agent instructions

`forkctl` maintains an audited downstream branch as an ordered StGit patch stack.

## Sources of truth

- Git commits and refs are canonical repository history.
- The manifest selected by `FORK_MANIFEST` or `--manifest` is canonical policy, typed target provenance, current patch metadata, and append-only dropped-patch history.
- `PATCHES.md` and optional patch exports are generated evidence; never edit them independently.
- StGit metadata and pending review evidence are recoverable clone-local state.

## Protocol boundary

- Every command handler returns typed data; it does not print or choose a view.
- Use `--output pretty` for the unified terminal view and `--output json` for one versioned JSON envelope. `--json` remains an alias.
- `forkctl api schema` emits JSON Schema for the full local invocation/response protocol.
- `forkctl api call` reads one versioned invocation from stdin and emits one JSON response.
- JSON stdout must never contain Git/StGit progress or human diagnostics.

## Workflow

1. Run `mise run fork:init` after cloning when StGit metadata is absent.
2. Use `mise run fork:status` at any time, including dirty or conflicted states.
3. Run `mise run fork:new -- NAME --kind source|tooling --purpose ... --upstream-status ... --drop-when ... --path ...` to scaffold a documented empty patch.
4. Add only declared implementation paths, refresh that patch with StGit, restore the bookkeeping patch, and run `mise run fork:new -- --finish` to bind the verified tip and regenerate exports.
5. Run `mise run fork:rebase -- --onto REF` with a full `refs/heads/*`, full `refs/tags/*`, or full commit SHA. Forkctl captures exact lease/recovery evidence, replays with `stg rebase --merged`, records upstream-merged patch removals in `PATCHES.md`, and writes a Git-private range-diff report bound by Git object ID.
6. If replay conflicts, resolve with `stg add --update`, `stg refresh`, and `stg goto <bookkeeping-patch>`, then rerun the same rebase command.
7. Review the range-diff and run consumer-specific semantic tests.
8. Run `mise run fork:publish`. It verifies bound pending evidence and atomically pushes the recovery tag and branch under the captured exact lease.

## Safety

- Mutating commands reject dirty worktrees. Forkctl never stashes operator changes.
- Keep the upstream remote fetch-only; its push URL must remain `DISABLED`.
- Rebase never publishes; publish never uses plain `--force`, an implicit lease, or a non-atomic fallback.
- A remote advance, retargeted recovery tag, modified report, post-finish stack change, unsafe export path, or unsupported atomic push is a hard failure.
- Do not treat structural reconstruction as semantic compatibility.
- Keep consumer build, packaging, hosting policy, and semantic checks outside forkctl.
- Do not put generic executable fork lifecycle or presentation logic into consuming repositories.

See `examples/` and the repository README for the complete manifest and immutable mise include shapes.
