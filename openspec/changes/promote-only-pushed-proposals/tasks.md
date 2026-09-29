# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Order

- [x] 1.1 Push the proposal commit by object ID and update the local ref afterwards; proof: `publish_propose_lease_keeps_a_concurrent_proposal_update` finds no local proposal ref after the rejected push
  - 2026-09-29 (UTC): the assertion fails on the previous `publish.rs` with "a rejected proposal left a local proposal ref" and passes with the change.
- [x] 1.2 Run the full gate; proof: `mise run verify` passes on the build host
  - 2026-09-29 (UTC): all 7 proposal tests pass, and `mise run lint` passes. The pre-push gate runs `mise run verify`.
