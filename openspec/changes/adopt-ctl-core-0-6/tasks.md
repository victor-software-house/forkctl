# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.

## 1. Adopt

- [x] 1.1 Pin ctl-core `=0.6.3` in both dependency tables; proof: `Cargo.lock` resolves ctl-core 0.6.3
- [x] 1.2 Move the patch column to `Table::id_column`; proof: `rg 'token' src/presentation.rs` lists only the `next` command
- [x] 1.3 Run the suite on the new defaults; proof: `mise run verify` passes with no test changes
- [x] 1.4 Release a forkctl patch in its own pull request; proof: the release tag exists
