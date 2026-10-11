# Changelog

## forkctl 0.0.31

- A `patch refresh` whose final check fails after the commit is written, for example because a pre-commit hook regenerated a lockfile, now stays an operation in phase `checking` and names its recovery: fix the reported problem and run `operation continue`, or `operation abort --yes`.
`patch finish` now refuses with `operation_in_progress` while an operation is in flight, so it can no longer clear the active patch in the middle of a refresh.

## forkctl 0.0.30

- Release assets are built and published from CI.
Each release carries `forkctl_<version>_macos_arm64.tar.gz` and a static `x86_64-unknown-linux-musl` `forkctl_<version>_linux_x64.tar.gz`, and CI runs the documented mounted `fork` task against the released binary.

