# Forkctl Audited Lifecycle — decisions

- 2026-08-03 Scope is forkctl only; do not inspect, migrate, or otherwise change Zed.
- 2026-08-03 Treat the manifest as a first contract: keep `schema: 1`, replace its current shape in place, and add no compatibility reader or migration path for the barely released 0.0.2 shape.
- 2026-08-03 Continue patch releases; implement and publish `0.0.3` through the existing local release path.
- 2026-08-03 `forkctl new` creates and positions a documented empty StGit patch, then updates manifest and ledger bookkeeping while leaving implementation edits to the operator.
- 2026-08-03 Discard the uncommitted 0.1/Zed proposal and its README link rather than preserving a contradictory design.
- 2026-08-03 Defer a release workflow and native asset matrix until the shared workflow collection exists; because forkctl is public, that future workflow uses GitHub-hosted default runners.
