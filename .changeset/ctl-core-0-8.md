---
forkctl: patch
---

forkctl now builds on ctl-core 0.8.0-rc.2, vsh-serde-saphyr 0.0.6, and yamled 0.0.14.
Every command takes its arguments from a document: `--input FILE`, or `--input -` for stdin, and `--input-format NAME` (`FORKCTL_INPUT`, `FORKCTL_INPUT_FORMAT`).
A document's keys are long option names and positional ids, and each value passes the argument's own parser.
The command line wins over the environment, the environment over the document, and the document over the default, and a key that loses is dropped.
The JSON protocol is unchanged: every response, success or error, is still the one document on stdout under `--format json` and in `api call`, with the same exit codes.
ctl-core 0.8 also changes the output around it:

- Piped output now defaults to `tsv`. `--format pretty` or `FORKCTL_FORMAT=pretty` gives the table, and `--json` is `--format json`.
- A parse error under `--format json` goes to stderr, and a parse error no longer starts with `error:`.
- A table narrows no column below its floor of 8 cells or its header width, and a table that cannot fit at its floors stacks each row as a record.
- Help gains `--json`, `--stream`, `--input`, and `--input-format`, and `--format` no longer shows `[default: pretty]`.
- An input problem in a manifest or a document uses ctl-core's new wording and positions.
- A pretty run streams the output of a publication's `git push` and hooks to stderr only when its format is `pretty`, so a piped run no longer does.
