---
forkctl: patch
---

The mounted `fork` task is now `raw`, so `mise run fork --format json status | jq` receives exactly what forkctl wrote. Before, in `prefix` output mode (several tasks, or `MISE_TASK_OUTPUT=prefix`), mise relayed each output line with a `[fork]` prefix and wrapped it in colour whenever its own colour was on, which `CLICOLOR_FORCE=1` turns on even into a pipe.
