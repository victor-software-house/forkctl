---
forkctl: patch
---

forkctl now applies a repository's YAML reader limits from `limits:` in `.ctl/input.yaml`, and prints `forkctl: warning: .ctl/input.yaml raises … above the default …` to stderr on every run for each limit raised above serde-saphyr's default. A malformed `.ctl/input.yaml` fails commands that need a YAML manifest with `manifest_invalid`; `operation status`, `continue`, and `abort` still work.
