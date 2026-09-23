# Presentation

## ADDED Requirements

### Requirement: Patch names use the identifier role

forkctl SHALL render patch names in the stack table through ctl-core's
identifier role, and SHALL keep suggested commands as tokens.

#### Scenario: Stack status

- **WHEN** `forkctl status --color always` lists a stack with one patch named `fix-build`
- **THEN** `fix-build` is bold with no colour escape
- **AND** the `next` command keeps the token colour
