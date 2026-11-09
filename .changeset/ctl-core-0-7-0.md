---
forkctl: patch
---

forkctl now builds on ctl-core 0.7.0, vsh-serde-saphyr 0.0.3, and yamled 0.0.12, so it links one serde-saphyr with ctl-core.
The help of `--color` and its values, `--no-color`, and the `pretty` format takes ctl-core's colour wording, and in help tables an empty column is 2 cells wide and a wrapped column narrows to its widest line.
