---
forkctl: patch
"verctl:group": fixed
---

`operation abort --yes` no longer fails when a hook rewrote a tracked file, such as `mise.lock` after a pin change; it discards that change, as its plan says. An operation stopped by its final check now names the fix: stage paths a hook rewrote that the patch owns, then `operation continue`, or restore the others.
