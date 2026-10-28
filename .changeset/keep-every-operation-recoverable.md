---
forkctl: patch
"verctl:group": fixed
---

No operation can stop without a way forward. `patch edit` refuses a scope that no longer covers the paths the patch changes before it writes anything, and a `patch edit` whose final check fails stays resumable in phase `checking`, as `patch refresh` does. `patch remove`, `disable`, `enable` and `rebase` name the fix, `publish` and `operation abort --yes` when their final check fails. `operation abort` ends the operation once the stack is restored, even when the restored stack fails `check`, and says so. `contract edit` now regenerates every export, like every other command that writes the bookkeeping patch.
