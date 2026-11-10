## ADDED Requirements

### Requirement: Rebase never conflicts on an output

`forkctl rebase` SHALL delete the derived patch before the replay, replay every other patch, and derive once in phase `deriving` before it regenerates bookkeeping.
A failed build SHALL stop the rebase in phase `deriving`; `operation continue` SHALL derive again and finish the rebase.

#### Scenario: Upstream changed the bundle

- **WHEN** upstream changes `dist/out.txt` and `src/in.txt`, the stack holds a derived patch for `dist/**`, and the operator runs `forkctl rebase -o refs/heads/main`
- **THEN** the command succeeds with no conflict, `dist/out.txt` at the new tip equals the rebuild of `src/in.txt` at the new tip, and `result.derived.rebuilt` is true

#### Scenario: A failing build stops the rebase

- **WHEN** a rebuild command exits 3 during `forkctl rebase`
- **THEN** the command fails, `operation status` reports phase `deriving`, and the error names the command and its exit status 3

#### Scenario: An upstream change outside every patch

- **WHEN** `derived` declares no `inputs`, a rebase moves onto an upstream that changes only `rollup.config.mjs`, which no patch touches
- **THEN** the rebase rebuilds the outputs

### Requirement: Rebase can verify upstream's outputs

`forkctl rebase --verify-base` SHALL also build at the new base and report in the result and the rebase report whether upstream's committed outputs equal that build.

#### Scenario: Upstream outputs that do not reproduce

- **WHEN** upstream's committed `dist/out.txt` differs from the rebuild of upstream's `src/in.txt`, and the operator runs `forkctl rebase -o refs/heads/main --verify-base`
- **THEN** the command succeeds and `result.derived.base_reproducible` is false
