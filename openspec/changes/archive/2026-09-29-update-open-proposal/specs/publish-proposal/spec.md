# Spec Delta

## Purpose

Defines how `forkctl publish --propose` pushes a reviewable proposal branch
for the current stack and keeps that branch current across repeated proposals.

## ADDED Requirements

### Requirement: Proposal commit shape

`publish --propose` SHALL push to `refs/heads/forkctl/proposal/<downstream branch>`
one commit whose only parent is the fetched downstream tip and whose tree is
the tree of `HEAD`.

#### Scenario: First proposal

- **WHEN** the downstream branch `main` has no proposal branch on `origin` and
  the operator runs `forkctl publish --propose`
- **THEN** `origin` has `refs/heads/forkctl/proposal/main`
- **AND** that commit's parent is the tip of `origin/main`
- **AND** its tree equals `HEAD^{tree}`

### Requirement: Repeated proposals update the branch in place

A later `publish --propose` SHALL replace the existing proposal branch with a
new proposal commit of the same shape. It SHALL push with
`--force-with-lease` on the proposal ref, expecting the proposal tip it read
from the remote immediately before the push, or expecting the ref to be absent
when it read none.

#### Scenario: Second proposal after the stack changed

- **WHEN** `origin` already has `refs/heads/forkctl/proposal/main` from an
  earlier `forkctl publish --propose`
- **AND** the stack changed and the operator runs `forkctl publish --propose`
  again
- **THEN** the command succeeds
- **AND** `refs/heads/forkctl/proposal/main` on `origin` points at a new commit
  whose parent is the tip of `origin/main` and whose tree equals
  `HEAD^{tree}`

#### Scenario: Proposal branch moved between read and push

- **WHEN** another writer moves `refs/heads/forkctl/proposal/main` on `origin`
  after forkctl read it and before forkctl pushed
- **THEN** `forkctl publish --propose` fails and leaves the other writer's
  commit on `origin`

### Requirement: Promotion checks the proposal it promotes

`publish --promote` SHALL refuse a proposal whose tree differs from
`HEAD^{tree}` or whose parent differs from the current downstream tip.

#### Scenario: Promote after an in-place update

- **WHEN** the operator ran `forkctl publish --propose` twice and then runs
  `forkctl publish --promote` on the same `HEAD`
- **THEN** `origin/main` equals `HEAD`
