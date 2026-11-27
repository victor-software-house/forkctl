# Proposal

## Why

forkctl pins ctl-core 0.7.0, and ctl-core 0.8.0-rc.2 is published ([changelog][changelog]).
ctl-core row CTC-043 asks each ctl CLI to take a command's arguments from an input document, with a golden test, and that needs 0.8.
`patch create` takes up to eleven kinds of flag, with `--scope` and `--check` repeated per item.
ctl-core 0.8 also writes a result whose outcome is an error to stderr, which would move forkctl's JSON error responses off stdout.
The queue row is FRK-042.
Protocol version 2 is unchanged.

## What Changes

1. Pin ctl-core `=0.8.0-rc.2` with `input-document` and `input-json`, vsh-serde-saphyr `=0.0.6` and yamled `=0.0.14`, and migrate every 0.8 break by the changelog's migration notes.
2. Every command takes `--input <FILE>` and `--input-format <NAME>`, also read from `FORKCTL_INPUT` and `FORKCTL_INPUT_FORMAT`.
3. A JSON response, success or error, stays the one document on stdout, in `api call` and under `--format json`.
4. Pretty output and the rest of ctl-core 0.8's output changes apply: piped output defaults to `tsv`, and a parse error under `--format json` goes to stderr.
5. The skill and the installed instructions describe the input document, the piped default and where each JSON document goes.

## Capabilities

### New Capabilities

- `input-document`: forkctl commands read their arguments from a document.

### Modified Capabilities

- `presentation`: the piped default format, and the stream of each JSON document.

## Impact

1. Code: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/cli.rs`, `src/presentation.rs`, the tests and their snapshots.
2. Docs: `skills/forkctl/SKILL.md`, `src/instructions.md` and their templates.
3. Release: a `forkctl: patch` changeset, as the repository's release rule asks.

[changelog]: https://github.com/victor-software-house/ctl-core/blob/4453827c9771269b450b70a18ac8fe2a4cffd4c6/CHANGELOG.md
