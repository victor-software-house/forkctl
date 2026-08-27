# Forkctl Rust Architecture Hardening — facts

- Forkctl already uses Rust 2024, Cargo resolver 3, an exact Rust toolchain, inherited workspace lints, `unsafe_code = "forbid"`, denied Clippy `all` and `pedantic`, rustfmt, ordinary and isolated tests, release builds, and Ubuntu/macOS hosted CI.
- The machine API converts typed `DomainError` values correctly, but command execution currently returns `anyhow::Result`; any expected state left as an opaque `anyhow` error falls through to `internal_error`.
- The merged no-operation defect was possible because both recovery commands used `anyhow::Context` for an ordinary absent-journal state; focused tests covered other preconditions but not those sibling commands.
- Seven production files exceed 500 lines: `src/app/patch.rs` (1,086), `src/manifest.rs` (857), `src/app/mod.rs` (795), `src/protocol.rs` (762), `src/cli.rs` (706), `src/view.rs` (582), and `src/app/check.rs` (562).
- `tests/lifecycle.rs` is 1,671 lines and mixes bootstrap, patch capture, operation recovery, publication, declared checks, manifest codecs, and shared helpers.
- A targeted Clippy run reports no function exceeding `clippy::too_many_lines` and no cognitive-complexity warning; the problem is responsibility density at module level, not giant individual functions.
- A targeted production panic audit reports nine `unwrap` or `expect` sites. Some are replaceable runtime assumptions; a smaller number describe construction or compile-time invariants and require explicit local justification if retained.
- Blanket `clippy::nursery` produces both useful findings and subjective churn. Rust Clippy recommends selecting restriction/nursery lints individually rather than enabling opinionated groups wholesale: https://doc.rust-lang.org/clippy/lint_configuration.html
- Cargo supports centrally inherited workspace lint policy through `[workspace.lints]`: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table
- Rust modules and restricted visibility provide the native mechanism for cohesive internal boundaries; file-length ceilings are project policy, not a Rust language rule: https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html
