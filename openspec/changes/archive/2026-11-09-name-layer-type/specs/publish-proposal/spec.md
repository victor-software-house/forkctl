## MODIFIED Requirements

### Requirement: The pull request describes the candidate

A created or updated proposal pull request SHALL be a draft. Its title SHALL
name the downstream branch and the short candidate commit. Its body SHALL list
the candidate commit, the downstream tip it replaces, the upstream base
selector and commit, each patch with its layer and purpose, and the command
`mise run fork publish --promote`.

#### Scenario: New pull request text

- **WHEN** the stack has the source patch `source-change` and the operator runs
  `forkctl publish --propose` with no open pull request
- **THEN** `gh pr create` receives `--draft`
- **AND** the title starts with `forkctl: propose main at` and a space
- **AND** the body contains `source-change` and
  `mise run fork publish --promote`
