# Design

## Context

ctl-core 0.8 routes a result by its outcome: success to stdout, error to stderr ([changelog][changelog]).
forkctl's `Report::outcome` marks an `ApiResponse::Error` as an error, so under 0.8 a failed `api call` would leave stdout empty.
The protocol promises one JSON document on stdout for every request.

## Decisions

### 1. A JSON response is a delivered document

When the output format is JSON, or the command is `api call`, `Report` reports success for every `ApiResponse`, and its exit code stays the one the response sets today.
Pretty output keeps the error outcome, so a pretty failure goes to stderr.

Alternative considered: take ctl-core's routing and move JSON errors to stderr.
It lost because it breaks the protocol's one-document rule for every caller of `api call`.

### 2. Parse errors follow ctl-core

A Clap parse error is ctl-core's envelope, written before forkctl runs, and goes to stderr under `--format json`, as in every ctl CLI.
`api call` reads its request from stdin, so its request errors are forkctl responses and stay on stdout.

### 3. Take ctl-core's piped default

Piped pretty output becomes `tsv`.
`--format pretty` or `FORKCTL_FORMAT=pretty` gives the table.

Alternative considered: `Formats::builtin().piped_default(FormatName::PRETTY)`.
It lost because ctl-core chose `tsv` so that a script reads the columns a person sees.

### 4. Every command takes the document

`App` adds `--input` and `--input-format` to every command, and forkctl removes them from none.
The golden test drives `patch create`, whose positional name and repeated `--scope` a document sets.
The losing key of a conflict is dropped, ctl-core's default; `App::input_conflicts(InputConflicts::Fail)` is the override forkctl does not take.

### 5. One serde-saphyr

vsh-serde-saphyr moves to `=0.0.6`, the release ctl-core 0.8.0-rc.2 and yamled 0.0.14 pin, so the lock holds one copy.

[changelog]: https://github.com/victor-software-house/ctl-core/blob/4453827c9771269b450b70a18ac8fe2a4cffd4c6/CHANGELOG.md
