---
name: forkctl
description: >-
  Operate forkctl audited Git/StGit downstream patch stacks: inspect status,
  create/select/edit/refresh/finish patches, run staged or repository checks,
  recover interrupted operations, rebase onto upstream, and publish under an
  exact lease. Use when a repository has a forkctl manifest, FORK_MANIFEST,
  generated patch exports/ledger, or the user mentions forkctl, downstream
  patches, StGit fork maintenance, rebase recovery, or atomic fork publication.
license: MIT
compatibility: Requires forkctl plus supported git and stg executables; a repository-mounted mise `fork` task may provision exact versions.
---

# Forkctl

Treat forkctl as the policy owner for an explicit StGit downstream patch stack. Delegate repository and patch mechanics to `git`, `stg`, and forkctl; do not reimplement or bypass their state.

## Establish the repository contract

1. Read repository guidance before changing anything.
2. Determine the supported invocation:
   - use `mise run fork -- <args>` when the repository mounts the forkctl task;
   - otherwise use `forkctl <args>`.
3. Respect an explicit `--manifest PATH` or `FORK_MANIFEST`. Do not guess a different manifest when discovery fails.
4. Run `status`, then the complete read-only `check`, before mutation.
5. Use `<invocation> --help` and `<invocation> instructions` as the authoritative installed-version contract. Do not invent aliases for rejected commands or fields.

```sh
mise run fork -- status
mise run fork -- check
```

Use the direct binary instead when no mounted task exists:

```sh
forkctl status
forkctl check
```

The remaining examples use the direct binary for brevity. In a mounted consumer, replace `forkctl` with `mise run fork --` without changing the arguments.

## Capture a downstream change

The operator chooses patch intent; forkctl never routes changes by filename.

Create a metadata-only patch intent before editing:

```sh
forkctl patch create PATCH \
  --kind source \
  --purpose 'Why this downstream change exists.' \
  --upstream-status not-submitted \
  --drop-when 'The condition that makes this patch unnecessary.' \
  --scope 'src/**' \
  --scope 'tests/**'
```

For an existing patch, select it instead:

```sh
forkctl patch select PATCH
```

Then edit normally and use Git's index as the default explicit capture boundary:

```sh
git add -- path/to/file
forkctl check --staged
forkctl patch refresh
forkctl patch finish
```

Rules:

- `check --staged` is read-only and validates the index against the active or explicitly named patch.
- `patch refresh` captures staged content by default and owns StGit targeting, hooks, generated evidence, and bookkeeping refresh.
- Use `patch refresh --all` only when every changed owned path is intended.
- Use repeatable `patch refresh --path PATHSPEC` for explicit path-limited capture.
- Repeat refresh while editing; finish only after the worktree/index is settled.
- Never manually edit generated exports or the generated ledger. Change patch intent through forkctl and let refresh regenerate them.
- Never stash operator work to satisfy a clean-state requirement.

Inspect or update declared intent with `patch show`, `patch list`, and `patch edit`; consult command help for exact metadata and scope flags.

## Remove or disable a patch

Use forkctl transitions, never raw `stg delete`, `stg pop`, or manual manifest edits:

```sh
forkctl patch remove PATCH --reason 'Why this patch is permanently obsolete'
forkctl publish

forkctl patch disable PATCH --reason 'Why this patch is temporarily excluded'
forkctl publish
forkctl patch enable PATCH
forkctl publish
```

Each transition requires a clean checked stack, creates immutable recovery
evidence, regenerates exports and the ledger, and stops at the ordinary atomic
exact-lease publication gate. A disabled patch is absent from the source tree
but retained in manifest `disabled_patches` with its former commit and recovery
tag. `operation continue` / `operation abort` own conflicts during deletion,
replay, or re-enable. The bookkeeping patch can never be removed or disabled.

## Validate

```sh
forkctl check                  # complete clean-repository audit
forkctl check --staged         # index against active patch
forkctl check --staged --patch PATCH
```

A complete check validates repository state, branch/remotes, base evidence, StGit order, patch scopes and trailers, generated ledger/exports, reconstruction, required text, and any current operation evidence. It does not claim application-level semantic compatibility; run the repository's own tests too.

Do not replace a failing check with raw Git/StGit commands or manual generated-file edits. Fix the typed failure at its owning boundary.

## Handle operations and conflicts

Mutations that can stop partway use one Git-private operation journal:

```sh
forkctl operation status
forkctl operation continue
forkctl operation abort --dry-run
forkctl operation abort --yes
```

On conflict:

1. Run `operation status` and follow its reported next actions.
2. Resolve the actual Git/StGit conflict without deleting forkctl state or recovery refs.
3. Continue through forkctl so it can regenerate and verify evidence.
4. If abandoning the operation, inspect `abort --dry-run` before confirmed abort.

Do not delete, retarget, recreate, or substitute recovery tags. Do not clear operation files manually.

## Rebase and publish

Rebase and publication are deliberately separate.

```sh
forkctl rebase --onto refs/heads/main --dry-run
forkctl rebase --onto refs/heads/main
# review status and the generated range-diff report
forkctl publish --dry-run
forkctl publish
```

Rebase requires a clean declared branch, no active patch, no other operation, and a complete passing check. It creates immutable recovery evidence, captures the downstream lease, delegates replay to StGit, records dropped upstream-merged patches, regenerates evidence, and stops before publication.

Publish only when the user explicitly requests remote mutation and the generated report has been reviewed. Publication atomically pushes the declared branch and recovery tag under the captured exact lease.

If the remote advanced, rejects atomic push, rejects the tag, or enforces branch policy:

- stop and report the typed failure;
- retain the operation and evidence;
- never retry with plain `--force`, an implicit lease, `+` refspecs, non-atomic pushes, or a provider-policy workaround.

Forkctl does not administer GitHub rulesets or bypass permissions.

## Use the typed API for automation

Pretty output is for operators. Automation uses JSON output or the local one-request API:

```sh
forkctl status --format json
forkctl api schema --kind bundle
printf '%s' '{"protocol_version":1,"mode":"execute","request":{"command":"check","arguments":{"scope":"repository"}}}' \
  | forkctl api call
```

Keep stdout as exactly one JSON document and treat schema-derived command names, arguments, results, notices, error codes, and details as authoritative. Do not parse pretty tables or error prose.

Use API `mode: "plan"` or CLI `--dry-run` for mutation planning; read-only commands reject plan mode.

## Stop conditions

Stop and ask rather than guessing when:

- the requested patch purpose, scope, upstream status, or removal condition is unclear;
- a staged path belongs to no declared patch;
- a mutation would overwrite unrelated dirty work;
- an operation or active patch already exists unexpectedly;
- a rebase target is ambiguous;
- publication, force-with-lease, recovery-tag, or provider-policy intent is not explicit;
- installed help differs from remembered syntax.
