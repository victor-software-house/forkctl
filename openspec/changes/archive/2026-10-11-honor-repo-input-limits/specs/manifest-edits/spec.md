## ADDED Requirements

### Requirement: A repository's input limits apply, with a warning when loosened

forkctl SHALL apply the limits a repository sets under `limits:` in `.ctl/input.yaml` to every manifest parse, and SHALL print one `forkctl: warning:` line to stderr on every run for each limit set above serde-saphyr's default.
A `.ctl/input.yaml` that cannot be read SHALL make commands that need an existing YAML manifest fail with `manifest_invalid`, naming the file, and SHALL NOT block a JSON manifest or bootstrap with an absent manifest.

#### Scenario: A loosened limit warns on every run

- **WHEN** `.ctl/input.yaml` holds `limits: { max_depth: 128 }` and the operator runs `forkctl --format json status` twice
- **THEN** each run succeeds and writes `forkctl: warning: .ctl/input.yaml raises max_depth to 128, above the default 64` to stderr

#### Scenario: A tightened limit applies to the manifest

- **WHEN** `.ctl/input.yaml` holds `limits: { max_depth: 2 }` and the operator runs `forkctl --format json check`
- **THEN** it fails with `error.code` equal to `manifest_invalid`, naming `max_depth`, and writes nothing to stderr

#### Scenario: An unknown limit is refused

- **WHEN** `.ctl/input.yaml` holds `limits: { max_widgets: 1 }` and the operator runs `forkctl --format json check`
- **THEN** it fails with `error.code` equal to `manifest_invalid`, naming `.ctl/input.yaml`

#### Scenario: An absent manifest is not blocked

- **WHEN** `.ctl/input.yaml` holds `limits: { max_widgets: 1 }` and the operator runs `forkctl --format json --manifest patches/absent.yaml init`
- **THEN** no error it reports names `.ctl/input.yaml`
