# Proposal

## Why

Downstream, on forkctl 0.0.20 and again on 0.0.38, `operation abort --yes` failed after a conflicted `patch refresh` on a stack holding patch `resolve-deleted-and-answered-threads`, a 36-character name.
The defect is still present in 0.0.47.

Abort rebuilds the stack with `stg uncommit --to <old base> --exclusive`.
Without names, StGit names each patch from its commit subject, cut to `stgit.namelength` characters, 30 by default.
The restored series then reads `resolve-deleted-and-answered-t` where the journal holds the full name, so the series check fails and the operation stays.
Setting `git config stgit.namelength 0` works around it.

`init` hydration already passes the manifest's patch names to `stg uncommit`, so it is not affected.

Protocol version 1 is unchanged.

## What Changes

- `operation abort` passes the journal's patch names to `stg uncommit`, newest first, as `init` hydration does, so every patch comes back under its journal name whatever `stgit.namelength` says.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `operation-recovery`: abort restores full patch names.

## Impact

- [`src/app/operation.rs`][operation]: `restore_operation_stack`.
- [`tests/lifecycle.rs`][lifecycle]: one test.

[operation]: ../../../../src/app/operation.rs
[lifecycle]: ../../../../tests/lifecycle.rs
