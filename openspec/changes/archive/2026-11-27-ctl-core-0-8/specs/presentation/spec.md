## ADDED Requirements

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
