## ADDED Requirements

### Requirement: A command reads its arguments from a document

Every forkctl command SHALL take `--input <FILE>` and `--input-format <NAME>`, also read from `FORKCTL_INPUT` and `FORKCTL_INPUT_FORMAT`.
A document's keys SHALL be long option names and positional ids, and each value SHALL pass the argument's own parser.
The command line SHALL win over the environment, the environment over the document, and the document over the default.

#### Scenario: Create a patch from a YAML file

- **WHEN** `forkctl patch create --dry-run --format json --input patch.yaml` runs where `patch.yaml` holds `name`, `layer`, `purpose`, `upstream-status`, `drop-when` and a two-item `scope` list
- **THEN** stdout is the same plan as the matching flags write
