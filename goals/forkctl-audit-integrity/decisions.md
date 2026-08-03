# Forkctl Audit Integrity — decisions

- 2026-08-03 Protect against accidental local drift and remote concurrency; trust the local operator and do not add signing or external anchoring.
- 2026-08-03 Replace descriptive base labels with typed historical targets that record target kind, selected ref or SHA, resolved commit, and annotated tag-object identity when applicable. Moving branches are historical selectors and need not remain at the selected commit.
- 2026-08-03 Bind each generated rebase report to pending state with its Git object ID; do not add a hashing crate or couple publication to regeneration under a later renderer version.
- 2026-08-03 `new --finish` records the verified tip. Any later stack change blocks publication until `new --finish` is rerun.
- 2026-08-03 Export destinations are concrete, bookkeeping-owned files disjoint from manifest, ledger, required contracts, and all non-bookkeeping patch path policies.
- 2026-08-03 When `stg rebase --merged` produces an empty non-bookkeeping patch, forkctl drops it automatically and appends its complete metadata and resolved target to manifest-backed `PATCHES.md` history. It does not retain or repeatedly warn about empty patches.
- 2026-08-03 The 0.0.5 corrective release includes real lifecycle regression coverage for evidence binding, export safety, invalid targets, merged-patch history, conflicting remote tags, tooling insertion, exact new-patch trailers, and complete pending identity.
- 2026-08-03 Unsupported atomic-push fallback remains source-level proof: forkctl has one atomic push and no retry path. Do not add a fabricated transport framework solely for that case.
- 2026-08-03 Publish and independently verify forkctl 0.0.5 after source and released-binary lifecycle suites pass.
