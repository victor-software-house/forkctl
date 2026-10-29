# Tasks

- [x] 1. Make the task and tool-pin generator declare minimal Rust; proof: `version:check` and mounted task tests pass in CI.
  - 2026-10-29 (UTC): [Linux CI][linux] passed all 141 tests in parallel and isolated runs, including real mounted completion and structured tool-version fixtures.
- [x] 2. Adopt the managed CI environment from template v0.0.60; proof: Linux CI and a cold macOS dispatch pass provider, components and full gates.
  - 2026-10-29 (UTC): Linux Clippy, docs, static gates, both suites and release build passed. The [cold macOS run][macos] installed without a cache in 98 seconds, selected rustup under `mise/cargo/bin` and a compiler sysroot under `mise/rustup/toolchains`, passed component assertions, all 141 parallel tests, all 141 isolated tests and the release build.
- [x] 3. Archive with linked test evidence after both platforms pass.
  - 2026-10-29 (UTC): this change is archived after both linked runs passed; no compatibility or retry logic was added.

[linux]: https://github.com/victor-software-house/forkctl/actions/runs/40852955441
[macos]: https://github.com/victor-software-house/forkctl/actions/runs/40852694460
