## ADDED Requirements

### Requirement: Bootstrap accepts a base outside the upstream branch

Bootstrap `init` SHALL record the canonical base as the merge base of the
selected base and the fetched upstream branch. Before it pushes, writes the
manifest, or creates StGit state, it SHALL fail with `check_failed` when the
diff from that canonical base to the selected base touches a path outside
`--allow-base`, in both execute and plan mode.

#### Scenario: Release tag off the upstream branch

- **WHEN** upstream tag `v1.0.0` is a commit changing `VERSION` on top of
  upstream `refs/heads/main`, no upstream branch contains it, and the operator
  runs `forkctl init ... --upstream-ref refs/heads/main --base refs/tags/v1.0.0 --allow-base VERSION`
  from a clone at `v1.0.0`
- **THEN** the command succeeds
- **AND** `forkctl --format json check` reports `result.canonical_base` equal
  to the upstream `main` commit and `result.stack_base` equal to the tag commit

#### Scenario: Release drift not allowed

- **WHEN** the operator runs the same `forkctl --format json init` without
  `--allow-base VERSION`
- **THEN** it fails with `check_failed` whose message contains
  `pre-stack drift is outside scope: VERSION`
- **AND** no manifest file exists and `origin` has no `refs/heads/main`

#### Scenario: Upstream branch never fetched

- **WHEN** the clone has no `refs/remotes/upstream/main`
- **THEN** bootstrap fetches it and succeeds as in the first scenario

#### Scenario: Rebase onto the next release tag

- **WHEN** upstream `main` advances and tag `v1.1.0` is cut off it the same
  way, and the operator runs `forkctl rebase --onto refs/tags/v1.1.0`
- **THEN** `forkctl --format json check` reports the new `main` commit as
  `result.canonical_base` and the `v1.1.0` commit as `result.stack_base`
