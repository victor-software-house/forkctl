# Forkctl Rust Architecture Hardening

## Objective

Harden forkctl so expected operator and repository states are distinct from internal failures by construction, failed commands prove non-mutation, all public adapters stay synchronized, and the implementation remains navigable through cohesive Rust modules rather than responsibility-heavy files.

## Why

A final pre-merge review found that two ordinary absent-operation calls still returned `internal_error` despite a prior typed-error correction. The immediate cases were fixed, but the architecture allowed the omission: command handlers returned `anyhow::Result`, and the protocol boundary inferred domain versus internal semantics by downcasting at runtime.

The codebase also has seven production files above 500 lines and one 1,671-line lifecycle test file. Individual functions are not oversized; unrelated responsibilities have accumulated in the same modules. The goal is therefore not cosmetic line reduction. It is explicit error authority, cohesive ownership, and automated protection against recurrence.

## Non-negotiable outcomes

- Expected client, repository, patch, operation, and publication states are typed errors; opaque propagation cannot silently choose `internal_error`.
- Actual internal failures retain structured context and become one machine-readable internal response rather than a panic.
- Failed mutation requests prove that worktree, index, refs, StGit series, Git-private journals, manifest, ledger, and generated exports remain unchanged.
- CLI grammar, completion, help, Usage KDL, protocol requests, JSON schema, and handlers remain in automated parity.
- Production code contains no unannotated panic, unwrap, expect, todo, or unimplemented path.
- Modules split by responsibility and reason to change. Numeric continuation files and cosmetic extraction are forbidden.
- CI enforces semantic architecture rules plus 500-line production, 600-line test/support, and 100-line function emergency ceilings.

## Delivery

Implement the approved plan as a stack of independently green PRs: public failure/non-mutation invariants; typed error boundary; production panic/lint policy; application/check modules; patch lifecycle; manifest; protocol; CLI/view; lifecycle tests and final architecture ceilings. Local improvements are allowed inside the touched seam when they remove accidental complexity and receive focused proof in the same slice.

Every layer must pass focused evidence, `mise run verify`, `mise run test:isolated`, `mise run build`, real hooks, and GitHub-hosted Ubuntu/macOS CI. Preserve the current public contract unless a demonstrated defect requires a deliberate correction in that layer.

## Out of scope

- New forkctl features or commands.
- Compatibility readers, aliases, migration fallbacks, or a framework rewrite.
- Blanket Clippy nursery/restriction groups.
- Dependency-license/advisory policy expansion.
- Releasing a new forkctl version.

## Done

The typed error, non-mutation, and adapter-parity invariants are automated; every module satisfies semantic and size boundaries; the full real Git/StGit suite passes unchanged in meaning; and clean synchronized `main` is green on Ubuntu and macOS.
