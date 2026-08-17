# Forkctl Rust Architecture Hardening — decisions

- 2026-08-05 Prefer explicit Rust types over lighter plumbing when that makes expected client failures and internal failures impossible to confuse.
- 2026-08-05 Split modules by distinct responsibilities and reasons to change, not by arbitrary numbered chunks or line-count theater.
- 2026-08-05 Enforce semantic architecture boundaries in CI, backed by generous emergency ceilings rather than using raw file length as the primary design rule.
- 2026-08-05 Improve a seam while it is being split when the improvement is local, removes accidental complexity, and is proven in the same slice; do not preserve awkward shapes merely to call a refactor mechanical.
- 2026-08-05 Repository/input state must never panic the release binary. Supposedly impossible runtime states become structured internal failures with context.
- 2026-08-05 Give equal automated weight to error semantics, non-mutation on failure, and parity among CLI, JSON schema, help, completion, and handlers.
- 2026-08-05 Deliver the full hardening as small stacked PRs. Each layer must remain usable and green; feature work, dependency-policy expansion, and a release are outside this goal.
