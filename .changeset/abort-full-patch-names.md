---
forkctl: patch
"verctl:group": fixed
---

`operation abort --yes` restores every patch under its full name. It used to let StGit name patches from commit subjects cut to `stgit.namelength` characters, so a patch name over 30 characters failed the abort unless `stgit.namelength` was 0.
