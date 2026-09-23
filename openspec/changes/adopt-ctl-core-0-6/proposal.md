# Adopt the ctl-core 0.6 presentation defaults

## Why

forkctl pins ctl-core 0.5.0. ctl-core 0.6.2 ships the look chosen in its
`choose-visual-identity` change: borderless records, an identifier role, pretty
JSON, a two-column automatic-width buffer, and an 80-column fallback when no
width is detected. The ctl CLIs should present the same way, and piped forkctl
output on 0.5.0 has no width limit.

Protocol version: unchanged. The JSON protocol keeps its fields; only its
layout on stdout is indented.

## What changes

1. Pin ctl-core `=0.6.2` in both dependency tables.
2. The patch column of the stack table renders through the identifier role
   (`Table::id_column`). The `next` command stays a token.

## Impact

Pretty output changes shape: records lose their box and JSON is indented.
The test suite passes without changes. A forkctl patch release follows in its own pull
request, as the versioning rule in AGENTS.md requires.
