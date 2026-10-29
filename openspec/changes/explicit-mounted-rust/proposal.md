# Proposal

## Why

Cold macOS verification passed managed toolchain installation, then the mounted-completion test failed while attempting to add `rust-docs`.
The mounted task declared Rust as a bare version, so it inherited rustup's default profile instead of the project's minimal profile.
Completion subprocesses consequently attempted toolchain mutation while tests shared the already installed tools.

## What Changes

1. The mounted task explicitly declares `profile = "minimal"`.
2. The existing tool-pin generator reads and rewrites the version in that structured declaration.
3. The mounted-task tests assert the profile and continue exercising real mise completion under CI's managed homes.

## Capabilities

### New Capabilities

- `mounted-rust`: the installed Rust profile of the portable mounted task.

### Modified Capabilities

None.

## Impact

No API or manifest change.
The mounted catalog changes and ships a patch fragment.
CI adopts template v0.0.60 for managed homes, explicit profiles and disabled implicit toolchain downloads.
