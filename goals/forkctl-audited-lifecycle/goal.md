# Forkctl Audited Lifecycle

## Objective

Publish forkctl `0.0.3` as the first complete audited downstream-fork lifecycle: a small Rust policy CLI over real Git and StGit that can initialize, inspect, create, verify, rebase, and safely publish declared patch stacks with deterministic audit metadata and real disposable-repository evidence.

## Scope

This goal changes only the public `victor-software-house/forkctl` repository. It does not inspect, migrate, or modify Zed or any other consumer repository.

The manifest remains literally `schema: 1`, but its barely released `0.0.2` shape is replaced in place. There is no compatibility reader, migration path, alternate schema, or fallback. Consumers pinned to immutable `0.0.2` remain on that release; `0.0.3` defines the complete contract.

## Public contract

The commands are:

- `init` — reconstruct declared StGit metadata after clone and verify it;
- `status [--json]` — report branch, remotes, bases, stack, dirty paths, verification, lease, recovery, and rebase evidence without mutation;
- `new` — create and correctly position a documented empty patch, then update manifest and generated ledger bookkeeping;
- `verify` — fail closed on all structural, audit, export, and reconstruction drift;
- `rebase --onto <ref>` — resolve an explicit branch, tag, or commit target, create recovery evidence, replay with StGit, verify, and write range-diff review evidence without publishing;
- `publish` — atomically publish the recovery tag and declared downstream branch under an exact force-with-lease;
- `instructions` — print the complete workflow outside a repository.

Human status may use color only when stdout is a terminal and `NO_COLOR` is absent. Redirected and JSON output contain no ANSI escapes.

## Manifest and audit model

The schema declares:

- downstream remote, branch, and backup-tag prefix;
- upstream remote, fetch URL, and fetch ref independently from a rebase target;
- base label plus full canonical and stack SHAs;
- tracked ledger path and final bookkeeping patch;
- ordered source/tooling patches;
- for every patch: name, kind, purpose, upstream status, drop condition, allowed paths, and optional export;
- allowed pre-stack base drift and required path/text contracts.

Unknown fields fail. Source patches precede tooling patches. The bookkeeping patch is final. A tooling-only stack is valid and reconstructs to the stack-base tree.

Each patch commit message must contain exactly matching `Downstream-Reason`, `Upstream-Status`, and `Drop-When` trailers. The manifest is canonical; forkctl deterministically renders and byte-verifies `PATCHES.md`. Persisted exports are optional reconstruction evidence, not canonical implementation.

## Safety invariants

- Every mutating command rejects a dirty worktree; forkctl never stashes operator work.
- Current branch must equal the declared downstream branch.
- The upstream fetch URL must match and its push URL must be `DISABLED`.
- Forkctl delegates all repository, revision, patch, replay, and conflict semantics to supported `git` and `stg` commands.
- Rebase captures exact downstream remote SHA, old base, and old tip and creates a unique annotated recovery tag before replay.
- Rebase uses `stg rebase --merged`, preserves normal StGit conflict state, and never publishes.
- Rebase review evidence lives under the Git-private path returned by `git rev-parse --git-path forkctl` and contains old/new identities plus no-color `git range-diff` output.
- Publish requires matching pending evidence and full verification.
- Publish uses explicit `--force-with-lease=refs/heads/<branch>:<captured-sha>` with `--atomic` and explicit branch/tag refspecs.
- Plain `--force`, implicit leases, `+` refspecs, and non-atomic fallback are forbidden.
- Remote advancement or unsupported atomic push fails without clearing pending state.
- Successful publication is complete only after both remote refs are read back and match.

## Patch creation

`new` accepts patch name, kind, purpose, upstream status, drop condition, one or more allowed paths, and optional export. It verifies the existing stack, captures pre-mutation recovery/lease state, reorders with StGit as needed, creates an empty patch with the exact trailers, restores later patches, updates manifest and ledger in the final bookkeeping patch, and leaves implementation files untouched. The empty patch intentionally blocks later verification/publication until the operator adds and refreshes its declared implementation.

## Verification

Verification covers:

- clean worktree and correct downstream branch/tracking;
- downstream remote and upstream fetch/push policy;
- base identities and merge-base relationship;
- exact StGit patch order and all-applied state;
- per-patch allowed paths and non-empty patch content;
- exact audit trailers;
- base allowlist and required text;
- generated ledger bytes;
- regenerated export bytes;
- exported source-tree reconstruction;
- tooling-only base-tree reconstruction;
- pending state identity when present.

Structural verification never claims semantic compatibility; consumers own product-specific tests.

## Automated evidence

The test suite creates tiny real Git and StGit repositories with local bare upstream/downstream remotes. It covers clone initialization and idempotency, tooling-only stacks, dirty and wrong-branch rejection, path/trailer/ledger/export drift, status plain/JSON behavior, source/tooling patch insertion, no-op and changed rebases, merged and conflicting patches, immutable recovery tags, range-diff reports, invalid targets, concurrent remote advancement, exact-lease rejection, and successful atomic publication. Tests require only mise-provisioned Git/Rust/StGit and no network or consumer repository.

## Delivery

- Set `[workspace.package].version` to `0.0.3`; existing synchronization updates task and example references.
- Expose all six lifecycle operations through the immutable remote mise task catalog with exact existing tool provisioning.
- Align README, examples, embedded instructions, AGENTS, and coding standards to one contract.
- Pass rustfmt, workspace Clippy with warnings denied, all unit/integration tests, release build, package dry-run, and a synthetic cold-clone remote-catalog proof.
- Publish `0.0.3` through the existing local GitHub/crates.io release path and verify release ref, asset digest, crates.io metadata, mise installation, instructions, and clean Git state.

## Explicit deferrals

Do not add a GitHub Actions release workflow or native asset matrix. Those wait for the shared workflow collection and will use GitHub-hosted default runners because forkctl is public.

Do not add compatibility code, automatic conflict resolution, GitHub ruleset logic, consumer semantic tests, application build/package/signing logic, or any Zed work.

## Done condition

Forkctl `0.0.3` is published and independently verified; every selected fact has automated evidence; a synthetic repository executes the full audited lifecycle; the repository documentation and task catalog describe the same schema-1 contract; no compatibility or Zed code exists; no release workflow was added; and the repository is clean and synchronized.
