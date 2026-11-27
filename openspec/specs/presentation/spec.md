# presentation Specification

## Purpose

How forkctl presents patch-stack reports on top of the ctl-core presentation kernel: which values take the identifier role.

## Requirements

### Requirement: Patch names use the identifier role

forkctl SHALL render patch names in the stack table through ctl-core's
identifier role, and SHALL keep suggested commands as tokens.

#### Scenario: Stack status

- **WHEN** `forkctl status --color always` lists a stack with one patch named `fix-build`
- **THEN** `fix-build` is bold with no colour escape
- **AND** the `next` command keeps the token colour

### Requirement: Every JSON response is the one document on stdout

forkctl SHALL write each `ApiResponse`, success or error, to stdout when the output format is JSON or the command is `api call`, with the exit code the response sets.
A pretty failure SHALL go to stderr.
When stdout is not a terminal and no flag or variable names a format, forkctl SHALL write `tsv`.

#### Scenario: A failed request in JSON

- **WHEN** `forkctl patch show missing --format json` fails because no patch is named `missing`
- **THEN** stdout holds one JSON error response, stderr holds no JSON, and the exit code is unchanged

#### Scenario: A failed api call

- **WHEN** `forkctl api call` reads a request whose `protocol_version` is 1
- **THEN** stdout holds one error response with code `unsupported_protocol`
