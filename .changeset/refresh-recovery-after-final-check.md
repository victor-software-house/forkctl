---
forkctl: patch
---

A `patch refresh` whose final check fails after the commit is written, for example because a pre-commit hook regenerated a lockfile, now stays an operation in phase `checking` and names its recovery: fix the reported problem and run `operation continue`, or `operation abort --yes`.
`patch finish` now refuses with `operation_in_progress` while an operation is in flight, so it can no longer clear the active patch in the middle of a refresh.
