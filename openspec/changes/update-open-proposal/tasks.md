# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Lease the proposal push

- [x] 1.1 Read the remote proposal tip and push with `--force-with-lease` on the proposal ref; proof: the lifecycle test that proposes twice passes
  - 2026-09-29 (UTC): `publish_propose_updates_the_open_proposal_then_promotes` passes. The second proposal replaces the remote tip with a commit whose parent is `origin/main` and whose tree is `HEAD^{tree}`.
- [x] 1.2 Cover the race; proof: a lifecycle test that moves the remote proposal ref between read and push sees `publish --propose` fail and the other commit survive
  - 2026-09-29 (UTC): `publish_propose_lease_keeps_a_concurrent_proposal_update` passes. A `git` wrapper creates the remote proposal ref just before the push, the leased push fails, and the other writer's commit stays on the remote.
- [x] 1.3 Promote after an in-place update; proof: the same lifecycle test promotes and `origin/main` equals `HEAD`
  - 2026-09-29 (UTC): passes in `publish_propose_updates_the_open_proposal_then_promotes`.
  - Not verified: the new tests were not run against the pre-change code.

## 2. Operator surfaces

- [x] 2.1 State the lease in the publish paragraph of `.ctl/operator/instructions.md.jinja`; proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` regenerates `src/instructions.md` and `mise run verify` passes
  - 2026-09-29 (UTC): the template and `src/instructions.md` carry the same paragraph. `operator_docs::instructions_are_the_committed_surface_render` passes without regeneration.
- [x] 2.2 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): `mise run verify` passes with 105 of 105 tests, and `mise run test:isolated` passes with 105 of 105.
