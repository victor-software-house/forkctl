# forkctl agent instructions

`forkctl` maintains a downstream branch as an ordered StGit patch stack.

## Repository contract

- Keep repository-specific state in the manifest selected by `FORK_MANIFEST` or `--manifest`.
- Source patches have an `export` path and must precede tooling-only patches.
- The final patch should own fork guidance, the manifest, and exported-patch bookkeeping.
- The configured upstream remote is fetch-only; `forkctl init` sets its push URL to `DISABLED`.

## Workflow

1. Run `mise run fork:init` after cloning when StGit metadata is absent.
2. Run `mise run fork:verify` before changing or publishing the stack.
3. Run `mise run fork:rebase` to fetch upstream, rebase, refresh declared exports, update base SHAs, and verify.
4. Resolve a stopped StGit conflict explicitly with normal `stg` commands, then rerun the relevant gate.
5. Run repository-specific semantic and integration tests after textual verification.

## Safety

- Do not edit base SHAs or exported patches by hand after a successful rebase.
- Do not add an upstream push URL.
- Do not treat deterministic patch reconstruction as semantic compatibility.
- Do not put generic executable fork logic back into the consuming repository.

See `examples/` and the repository README for the manifest and mise include shapes.
