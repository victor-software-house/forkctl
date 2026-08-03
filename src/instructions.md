# forkctl agent instructions

`forkctl` maintains an audited downstream branch as an ordered StGit patch stack.

## Sources of truth

- Git commits and refs are canonical repository history.
- The manifest selected by `FORK_MANIFEST` or `--manifest` is canonical policy and patch audit metadata.
- `PATCHES.md` and optional patch exports are generated evidence; never edit them independently.
- StGit metadata is recoverable clone-local state.

## Workflow

1. Run `mise run fork:init` after cloning when StGit metadata is absent.
2. Use `mise run fork:status` at any time, including dirty or conflicted states. Add `--json` for machine-readable output.
3. Run `mise run fork:new -- NAME --kind source|tooling --purpose ... --upstream-status ... --drop-when ... --path ...` to scaffold a documented empty patch.
4. Add only the declared implementation paths, refresh that patch with StGit, restore the bookkeeping patch, and run `mise run fork:new -- --finish` to regenerate exports and verify.
5. Run `mise run fork:rebase -- --onto REF` to capture an exact lease and recovery tag, replay with `stg rebase --merged`, refresh generated evidence, and write a Git-private range-diff report.
6. If replay conflicts, resolve with `stg add --update`, `stg refresh`, and `stg goto <bookkeeping-patch>`, then rerun the same `fork:rebase -- --onto REF` command to finish.
7. Review the range-diff and run consumer-specific semantic tests.
8. Run `mise run fork:publish` only after review. It verifies again and atomically pushes the recovery tag and branch under the captured exact lease.

## Safety

- Mutating commands reject dirty worktrees. Forkctl never stashes operator changes.
- Keep the configured upstream remote fetch-only; its push URL must remain `DISABLED`.
- Rebase never publishes and publish never uses plain `--force` or an implicit lease.
- A remote advance or unsupported atomic push is a hard failure without fallback.
- Do not treat structural reconstruction as semantic compatibility.
- Keep consumer build, packaging, hosting policy, and semantic checks outside forkctl.
- Do not put generic executable fork lifecycle logic back into consuming repositories.

See `examples/` and the repository README for the complete manifest and immutable mise include shapes.
