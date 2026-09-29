# Spec Delta

## Purpose

Defines how `forkctl publish --propose` pushes a reviewable proposal branch
for the current stack and keeps that branch current across repeated proposals.

## ADDED Requirements

### Requirement: The local proposal ref follows the remote

`publish --propose` SHALL move the local `refs/heads/forkctl/proposal/<branch>`
only after the remote accepted the proposal push.

#### Scenario: Rejected proposal push

- **WHEN** another writer creates `refs/heads/forkctl/proposal/main` on
  `origin` between forkctl's read and its push, and no local proposal ref
  existed
- **THEN** `forkctl publish --propose` fails
- **AND** `git for-each-ref refs/heads/forkctl/proposal/main` prints nothing
