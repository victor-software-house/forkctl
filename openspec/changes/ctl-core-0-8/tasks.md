# Tasks

## 1. The upgrade

- [ ] 1.1 Pin ctl-core `=0.8.0-rc.2` in both dependency tables with `input-document` and `input-json` on the runtime entry, vsh-serde-saphyr `=0.0.6` and yamled `=0.0.14`, and take them past the cooldown with `mise run update:own`.
  Verify: `cargo tree -d` lists one copy of each ctl crate and of serde-saphyr.
- [ ] 1.2 Migrate every 0.8 break by the changelog's migration notes, and accept snapshots from a real run.
  Verify: `mise run verify`, `mise run test:isolated` and `mise run build` pass on the build host.

## 2. The protocol and the document

- [ ] 2.1 Keep every JSON response on stdout, and add tests for both presentation scenarios.
  Verify: both scenarios pass as tests on the build host.
- [ ] 2.2 Add the golden test for `patch create --input`.
  Verify: the scenario passes as a test on the build host.

## 3. Docs and release

- [ ] 3.1 Describe the input document, its variables, the precedence, the piped default and the stream of each JSON document in the skill and the instructions, through their templates.
  Verify: the rendered files match their templates in `mise run verify`.
- [ ] 3.2 Write a `forkctl: patch` changeset naming the new flags and each output change.
  Verify: `openspec validate ctl-core-0-8 --strict` passes.
