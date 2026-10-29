# mounted-rust Specification

## Purpose

How the mounted task selects its Rust profile without inheriting machine settings.

## Requirements

### Requirement: The mounted task declares a minimal Rust profile

The mounted fork task SHALL declare Rust with its exact version and `profile = "minimal"`, so an invocation never inherits the caller's rustup profile.

#### Scenario: Completion against a minimal installed toolchain

- **WHEN** stable Rust has only the minimal components and Clippy, and the mounted task produces shell completions
- **THEN** completion succeeds without attempting to install `rust-docs`
