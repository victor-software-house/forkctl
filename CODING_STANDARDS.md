# Coding Standards

## 1. Scope

### 1.1 Delegate mechanics

Use the installed `git` and `stg` CLIs. Do not reproduce their storage, revision, patch, or conflict semantics.

### 1.2 Keep the public surface narrow

The lifecycle operations are `init`, `status`, `new`, `verify`, `rebase`, and `publish`. New commands require another demonstrated cross-fork contract.

## 2. Dependencies

### 2.1 Runtime isolation

All runtime tools must be declared by the remote mise tasks. Never require Homebrew, a global Cargo install, or an activated language environment.

### 2.2 Rust dependencies

Use current stable releases, pin them exactly, and add a crate only when it removes more code or risk than it adds.

## 3. Safety

### 3.1 Fail closed

A failed command, malformed manifest, dirty worktree, drifted remote, or undeclared path is an error. Never print success after a failed prerequisite.

### 3.2 Atomic writes

Write manifests and exported patches atomically. Stage only files declared by the manifest.

### 3.3 No hidden migrations

The current manifest contract is `schema: 1`. Do not add compatibility readers or migrations for the pre-0.0.3 shape.

## 4. Verification

### 4.1 Repository checks

Formatting, Clippy with denied warnings, and tests must pass before release.

### 4.2 Consumer proof

Every release must be exercised through mise against disposable real Git/StGit remotes, including metadata initialization, deterministic reconstruction, rebase evidence, exact-lease rejection, and successful publication.
