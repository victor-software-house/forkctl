# Forkctl Rust Architecture Hardening

## Invariant

Expected operator and repository states are typed by construction; failed commands prove they do not mutate; public adapters remain in parity; modules expose one coherent responsibility and cannot silently grow back into monoliths.

## Delivery rules

- Land as a linear stack of small PRs from current `main`.
- Keep every intermediate layer compiling, tested, documented, and usable.
- Preserve public CLI and JSON schema unless a defect requires a deliberate correction covered in that same layer.
- Improve local seams while touching them, but do not mix feature work or unrelated cleanup.
- Run focused tests, `mise run verify`, `mise run test:isolated`, `mise run build`, real hooks, and hosted Ubuntu/macOS CI for every layer.
- Do not release within this goal.

## 1. Lock the public failure invariants before refactoring

**Change**

- Add a schema-derived mutation-command inventory and require every public mutation command to have at least one representative expected-failure case.
- Add a reusable repository snapshot covering worktree bytes, index tree, HEAD, refs, StGit series, and Git-private forkctl state.
- For every matrix case, require a typed non-internal error and byte-for-byte unchanged snapshot.
- Add a command-set parity check between the protocol schema and Clap tree before either adapter is split.

**Proof**

- The matrix includes all current mutation commands and fails when a new schema command lacks a case.
- The exact absent-journal `operation.continue` and `operation.abort` regression remains represented.
- Every expected failure is non-mutating.

## 2. Make error classification exhaustive

**Change**

- Introduce an application error enum and result alias that distinguish `DomainError` from contextual internal failures.
- Make command execution return that typed result and make JSON error rendering an exhaustive match rather than an `anyhow` downcast plus fallback.
- Keep CLI/JSON parse failures explicitly mapped to `invalid_request` before command execution.
- Keep subprocess failures typed as `subprocess_failed`; make I/O and violated implementation invariants explicit internal variants with preserved causes.
- Provide explicit adapters for internal errors rather than a blanket opaque conversion that lets `?` silently choose `internal_error`.
- Centralize common preconditions (`require/no active patch`, `require/no operation`, clean worktree, declared branch/tracking) so sibling commands cannot improvise classification.

**Proof**

- The layer-1 matrix remains green without relying on runtime downcasts.
- Error schema snapshots and pretty/JSON parity remain stable.

## 3. Remove production panic paths and adopt selected lints

**Change**

- Deny `unwrap_used`, `expect_used`, `panic`, `todo`, and `unimplemented` in non-test production code.
- Replace fallible runtime assumptions with typed failures.
- Permit only narrow `#[expect]` sites whose reason states a construction or compile-time invariant; do not use broad module-level allowances.
- Deny selected low-noise complexity lints that already pass: `too_many_lines` and `cognitive_complexity`.
- Fix and adopt only high-signal nursery findings encountered in changed seams (`redundant_clone`, `needless_pass_by_ref_mut`); do not enable the full nursery group.

**Proof**

- Strict Clippy passes across workspace/all targets/all features.
- A source audit finds no unannotated production panic/unwrap/expect macro path.

## 4. Split application infrastructure and checks

**Change**

- Reduce `src/app/mod.rs` to the `App` type, module declarations, and narrow shared exports.
- Move repository discovery/branch/remotes/targets, generated evidence/bookkeeping, Git-private state storage, and recovery construction into cohesive modules.
- Split `src/app/check.rs` into orchestration, repository/staged checks, generated/history evidence, and operation evidence.
- Replace accidental cross-module visibility with the narrowest `pub(super)` or private API.

**Proof**

- Existing real lifecycle suite remains unchanged in behavior.
- Architecture tests prevent command/domain modules from rendering or printing and prevent new reverse dependencies into CLI/view code.

## 5. Split the patch lifecycle

**Change**

- Replace `src/app/patch.rs` with a `patch/` module family organized around query/selection, metadata editing, refresh/capture, finish, and disable/remove/enable transitions.
- Keep shared state-machine helpers private to the patch module family.
- Remove duplicated precondition plumbing in favor of the typed helpers from layer 1.
- Apply local improvements exposed by the split only when focused lifecycle evidence closes the loop.

**Proof**

- Patch create/select/edit/refresh/finish/transition tests pass with real Git/StGit.
- Conflict continuation and abort evidence remain exact.

## 6. Separate manifest types from validation

**Change**

- Split manifest data types, validation, query/accessor helpers, path/glob rules, and unit tests into a `manifest/` module family.
- Keep serialization shape and schema names unchanged through re-exports.
- Make validation phases explicit and ordered without introducing compatibility readers.

**Proof**

- YAML/JSON round trips, schema snapshots, export determinism, and lifecycle codecs remain byte-compatible.

## 7. Separate protocol requests, responses, and schema

**Change**

- Split `protocol.rs` into request arguments/metadata, response/results/errors, and schema generation.
- Re-export the existing protocol surface so adapters do not gain a second model.
- Derive command inventory and read-only/mutation metadata from one exhaustive representation where practical.

**Proof**

- JSON schema and response snapshots remain stable except for deliberate typed-error corrections.
- An automated command-set parity check compares protocol commands with the Clap command tree.

## 8. Separate CLI grammar and human rendering

**Change**

- Split `cli.rs` by root grammar, patch/operation/API arguments, parsers, and conversion into typed requests while retaining Clap as the sole CLI grammar.
- Split `view.rs` by response dispatch, command renderers, error rendering, and shared style/table primitives while retaining one semantic theme.
- Do not create generic framework abstractions; modules follow current concrete command families.

**Proof**

- Help, Usage KDL, completion, narrow-width rendering, pretty/JSON parity, and command execution tests pass.
- Architecture tests continue to permit terminal concerns only in CLI/help/layout/view/main boundaries.

## 9. Split lifecycle evidence and lock the architecture

**Change**

- Keep one integration-test crate but split `tests/lifecycle.rs` into coherent modules for bootstrap/codecs, patch lifecycle, operation recovery, publication, and declared checks.
- Move only genuinely shared fixture operations into `tests/support`; do not create a testing framework around single-use setup.
- Add semantic dependency/ownership checks and final emergency ceilings:
  - production Rust files: 500 lines;
  - test/support Rust files: 600 lines;
  - functions: Clippy `too_many_lines` default (100 lines).
- Count recursively and exclude no handwritten Rust file. A ceiling failure must be fixed by a responsibility split, not a numbered continuation file.

**Proof**

- Ordinary and isolated suites execute the same test inventory before and after the move.
- Every production module and test module satisfies the final ceilings.
- Full hosted matrix passes from a clean clone.

## Final done condition

- No command boundary can silently turn an expected state into `internal_error` through opaque propagation.
- Expected-error, no-mutation, and adapter-parity invariant suites are green.
- No unannotated production panic/unwrap/expect path remains.
- No production Rust file exceeds 500 lines, no test/support Rust file exceeds 600 lines, and no function exceeds the selected Clippy threshold.
- Every split follows a named responsibility and narrow visibility boundary.
- `main` is clean, synchronized, and green on Ubuntu and macOS; no release is performed.
