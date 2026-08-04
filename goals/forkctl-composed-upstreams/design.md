# Forkctl Composed Upstreams — Design

## Summary

Introduce an explicit composition base provider that resolves exact projections from several Git repositories into one deterministic synthetic base, then reuses forkctl's ordered StGit replay and audit lifecycle.

Add immutable proposal evidence so candidate preparation and exact promotion can occur on separate machines. Build GitHub integration as thin reusable workflows around provider-neutral forkctl commands.

This document is proposed architecture, not current behavior.

## Architecture Boundary

```text
                             ┌──────────────────────────┐
Git source caches ──────────>│ Composition materializer │
                             └────────────┬─────────────┘
                                          │ synthetic base
                                          ▼
                               ┌─────────────────────┐
                               │ StGit patch replay  │
                               └──────────┬──────────┘
                                          │ exact candidate
                      ┌───────────────────┴───────────────────┐
                      ▼                                       ▼
             direct publication                      durable proposal refs
                                                              │
                                                              ▼
                                                     provider review PR
                                                              │
                                                              ▼
                                                     trusted promotion
```

Forkctl continues to own policy, evidence, and orchestration. Git owns object/ref mechanics. StGit owns patch replay and conflict state. GitHub CLI owns PR reads/writes.

## Base Provider Model

A tagged base-provider enum prevents hidden behavior:

```text
RepositoryBase
  upstream remote/url/fetch ref
  selected exact target
  canonical merge base
  StGit stack base

CompositionBase
  named source declarations and locks
  synthetic materialized base
  StGit stack base
```

A one-source root projection remains a composition. A direct repository remains direct even if sparse checkout happens to hide paths locally.

Illustrative data shape:

```yaml
base:
  kind: composition
  sources:
    - name: pi
      git:
        url: https://github.com/badlogic/pi-mono.git
        ref: refs/heads/main
      project:
        - from: packages/coding-agent/src
          to: vendor/pi/src
      locked:
        commit: 0123456789abcdef0123456789abcdef01234567

    - name: schemas
      git:
        url: https://github.com/example/schemas.git
        ref: refs/heads/main
      project:
        - from: events
          to: schemas/events
      locked:
        commit: 89abcdef0123456789abcdef0123456789abcdef

  materialized: fedcba9876543210fedcba9876543210fedcba98
  stack: fedcba9876543210fedcba9876543210fedcba98
```

Exact field names and representation are deferred to schema design. Authored source intent and generated exact evidence remain distinguishable in the typed model even if they share one manifest file.

## Source Caches

Each source name owns a clone-local cache under the repository's Git-private forkctl directory. The tracked manifest stores no credentials or cache paths.

A cache:

- has one configured URL matching the source declaration;
- resolves the configured ref to an exact commit;
- may use shallow or partial fetch when supported;
- must produce the same projected objects regardless of fetch optimization;
- is disposable and reconstructible from tracked source declarations and locks.

Unselected source locks do not change during subset synchronization.

## Projection Plan

Before mutation, build one complete plan:

1. Validate unique source names and normalized exact mappings.
2. Reject equal or parent/child-overlapping destinations.
3. Resolve every selected source ref and freeze its exact target evidence.
4. Verify every mapped source root exists at that target.
5. Enumerate each selected tree recursively with Git, preserving mode, object kind, object ID, and source path.
6. Remap every entry to its destination and reject collisions again at entry level.
7. Sort the complete destination inventory by raw Git path order.
8. Compare source-lock and projected-tree evidence with the current materialized state.

No downstream object, index, worktree, manifest, StGit patch, or ref changes before this plan succeeds.

## Materializer

The materializer copies Git objects rather than copying a checkout:

1. Read selected blob contents in batches from source caches.
2. Write exact blob bytes into the downstream object database.
3. Populate a temporary isolated index with remapped destinations and original modes.
4. Preserve Gitlinks as Gitlink entries; do not require the submodule commit object in the downstream database.
5. Write the composite tree from the temporary index.
6. Create a deterministic synthetic commit with that tree.

The first synthetic commit has no parent. Each later synthetic commit has the previous synthetic base as its sole parent. Upstream source commits appear only as recorded provenance, never as parents.

Canonical commit content includes:

- stable forkctl materialization identity;
- sorted source name and exact commit pairs;
- canonical projection-mapping digest;
- composite tree ID;
- prior synthetic base when present.

Author/committer identity and timestamp policy must be deterministic and fixed before implementation.

## Replay

With a new synthetic base available:

1. Create the existing recovery evidence before modifying the stack.
2. Rebase the declared series with `stg rebase --merged`.
3. Preserve ordinary StGit conflict state and operation continuation.
4. Identify patches whose trees equal their parents.
5. Bind every removed patch to its exact old commit and recovery object.
6. Update current source locks and base evidence.
7. Regenerate manifest, ledger, exports, and declared generated artifacts.
8. Refresh the final bookkeeping patch.
9. Run the complete repository check.
10. Generate range-diff and source-lock review evidence.

A patch may own paths across several projected roots. Projection ownership constrains the synthetic base; patch scopes continue to constrain downstream intent.

## History Strategies

### Rewrite

```text
B0 ── P0a ── P0b
 └── B1 ── P1a ── P1b
```

The StGit base is `B1`. Direct publication uses exact force-with-lease and immutable recovery evidence.

### Append

```text
B0 ── P0a ── P0b ───── E1 ── P1a ── P1b
 └── B1 ────────────────┘
```

Construct `E1` with:

- parent 1: old published tip `P0b`;
- parent 2: new synthetic base `B1`;
- tree: `B1^{tree}`.

The StGit base is `E1`. Replayed patches are `P1*`. Validation proves `E1`'s tree identity and both parents, proves old-tip ancestry, and permits fast-forward publication only.

The append epoch deliberately resets the visible tree to the unpatched composite base before fresh patch commits. This trade-off is recorded rather than hidden.

## CLI Direction

Proposed command families:

```text
forkctl sync [-s|--source NAME]... [--history rewrite|append] [-n|--dry-run]
forkctl publish [-n|--dry-run]

forkctl proposal push [--branch REF] [-n|--dry-run]
forkctl proposal verify PROPOSAL
forkctl proposal promote PROPOSAL [-n|--dry-run]
```

`sync` owns source resolution, materialization, replay, generated artifacts, validation, and the local operation journal. Direct `publish` remains the local one-step publication path.

Proposal commands are provider-neutral:

- `proposal push` publishes review refs and immutable evidence without moving downstream;
- `proposal verify` validates a proposal in a fresh clone;
- `proposal promote` enforces the proposal lease and moves the exact downstream ref.

Detailed CLI design must decide whether direct-provider `rebase` remains or is cleanly replaced by `sync`. No alias is implied.

Every command receives one request/result/schema path through the existing typed protocol. Automation consumes JSON results rather than human text.

## Proposal Objects

A local operation journal cannot survive runner boundaries. A durable proposal uses Git objects:

```text
old downstream tip ──> review commit ──> draft review PR
                         tree = candidate tree

proposal annotated tag ────────────────> exact candidate commit
recovery annotated tag ────────────────> old downstream tip
```

The review commit is a single-parent commit built from the exact candidate tree with the captured old downstream tip as parent. This makes the provider PR show the net synchronization delta even when the rewrite candidate diverges below the old patch stack. It is review and CI material only; the immutable proposal tag keeps the exact candidate reachable.

The canonical proposal payload binds:

- protocol version and proposal identity;
- downstream remote/ref and expected old SHA;
- exact candidate SHA and history strategy;
- review branch, commit, and candidate-equal tree;
- recovery tag name and object ID;
- source old/new locks;
- synthetic base and tree;
- manifest object ID;
- report object ID;
- creation identity/time policy.

`proposal push` atomically pushes the mutable review branch, immutable recovery tag, and immutable proposal tag under exact leases. Promotion starts from the proposal object ID, not the review branch name or commit.

`proposal verify` fetches required refs, validates tag/object types and targets, proves that the review commit's parent is the expected old downstream tip and its tree equals the exact candidate tree, verifies the manifest and report object IDs, runs the complete repository check at the candidate, and confirms downstream still equals the expected old SHA.

`proposal promote` then moves downstream to the exact candidate with force-with-lease for rewrite or fast-forward-only for append. It reads the remote ref back before reporting success. The GitHub integration comments on and closes the draft review PR rather than merging it.

## GitHub Workflows

### Proposal reusable workflow

Inputs should remain typed and minimal: manifest path, history strategy, optional source names, proposal branch, and optional app token secret. The caller owns schedule/manual triggers and grants `contents: write` and `pull-requests: write`.

The called workflow:

1. serializes by caller repository and downstream branch;
2. checks out complete trusted downstream history;
3. installs an exact forkctl release;
4. hydrates and synchronizes;
5. exits successfully without a PR on a true no-op;
6. pushes proposal evidence and the review branch;
7. creates or updates one stable draft PR through `gh`;
8. exposes proposal ID, candidate SHA, changed sources, and PR URL as outputs.

### Promotion reusable workflow

The consumer exposes a `workflow_dispatch` caller taking a proposal or PR identity. The called workflow resolves that input to an immutable proposal object, checks out trusted default-branch workflow code, verifies and promotes the proposal, verifies remote state, comments the result, and only then removes the candidate branch.

Neither workflow checks out and executes mutable PR-head workflow code with privileged credentials.

## Error Boundaries

Add typed errors for:

- duplicate source identity;
- unknown selected source;
- source URL/ref mismatch;
- source resolution/fetch failure;
- stale projection root;
- invalid or colliding destination;
- unsupported Git entry kind;
- materialization/object mismatch;
- invalid synthetic-base ancestry;
- invalid append epoch;
- proposal object mismatch;
- stale proposal lease;
- candidate tampering;
- promotion rejection.

Pretty and JSON views consume the same error details.

## Deferred Embedded Mode

An ordinary repository with independently evolving unmanaged paths cannot share this live repository-level StGit stack without a second history owner.

If pursued, embedded mode should instead treat exact source locks plus tracked patch files as canonical input and compile managed destination roots into ordinary Git commits. It may share the source graph and materializer, but not patch persistence, recovery topology, or publication semantics.
