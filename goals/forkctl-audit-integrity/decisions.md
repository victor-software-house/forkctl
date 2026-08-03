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
- 2026-08-03 Domain handlers return typed protocol data and errors only. Clap and JSON requests are equal adapters over the same handlers; command/domain modules never print, inspect output mode/TTY, or build tables.
- 2026-08-03 Expose global pretty/JSON output plus a full local JSON API with generated Schemars schema and JSON request execution.
- 2026-08-03 Use the proven composable Rust CLI stack rather than an immature all-in-one framework: Clap, Serde/Schemars, Comfy Table, Anstream/Anstyle, and Insta where each removes concrete boilerplate. Do not add Miette until diagnostics require source spans or richer structure than the unified semantic renderer provides.
- 2026-08-03 One centralized semantic theme and renderer owns every human-facing visual element so the CLI remains visually consistent rather than combining crate defaults.
- 2026-08-03 Keep the protocol/view kernel small; do not add progress, panels, or renderer abstractions that forkctl does not currently need.
- 2026-08-03 Historical upstream-merged entries record the pre-rebase patch commit preserved by the published recovery tag, never the transient empty post-rebase commit; `fork:init` hydrates recovery tags by the declared prefix before verifying history in a fresh clone.
- 2026-08-03 Disposable lifecycle subprocesses remove inherited `GIT_*` variables because Git hooks export parent-repository context; fixture commands must never resolve against or mutate the checkout running the hook.
