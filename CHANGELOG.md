# Changelog

## forkctl 0.0.33

- forkctl now applies a repository's YAML reader limits from `limits:` in `.ctl/input.yaml`, and prints `forkctl: warning: .ctl/input.yaml raises … above the default …` to stderr on every run for each limit raised above serde-saphyr's default. A malformed `.ctl/input.yaml` fails commands that need a YAML manifest with `manifest_invalid`; `operation status`, `continue`, and `abort` still work.

## forkctl 0.0.32

- Commands that change a YAML manifest now edit only the nodes they change, so comments, key order, and formatting elsewhere in `patches/fork.yaml` survive `patch refresh`, `patch edit`, `patch disable`, `patch enable`, `patch remove`, `contract edit`, `publish --set-default`, and `rebase`.
The manifest is read through ctl-core's input reader: anchors, aliases, and any consistent indentation are accepted, and duplicate keys and merge keys are still refused. An edit that would go through an anchor or alias fails with `manifest_invalid` and leaves the file unchanged.

## forkctl 0.0.31

- A `patch refresh` whose final check fails after the commit is written, for example because a pre-commit hook regenerated a lockfile, now stays an operation in phase `checking` and names its recovery: fix the reported problem and run `operation continue`, or `operation abort --yes`.
`patch finish` now refuses with `operation_in_progress` while an operation is in flight, so it can no longer clear the active patch in the middle of a refresh.

## forkctl 0.0.30

- Release assets are built and published from CI.
Each release carries `forkctl_<version>_macos_arm64.tar.gz` and a static `x86_64-unknown-linux-musl` `forkctl_<version>_linux_x64.tar.gz`, and CI runs the documented mounted `fork` task against the released binary.

