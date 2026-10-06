# Changelog

## forkctl 0.0.30

- Release assets are built and published from CI.
Each release carries `forkctl_<version>_macos_arm64.tar.gz` and a static `x86_64-unknown-linux-musl` `forkctl_<version>_linux_x64.tar.gz`, and CI runs the documented mounted `fork` task against the released binary.

