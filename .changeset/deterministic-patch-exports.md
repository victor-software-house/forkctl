---
forkctl: patch
"verctl:group": fixed
---

Source patch exports are now the same in every clone. Their `index` lines carry full blob ids, where git's automatic abbreviation grew from 8 to 9 characters once a clone held more objects, so `check` failed with `export differs` on a clone that had fetched more of upstream. A `stgit.diff-opts` setting in the clone's or the user's Git configuration no longer changes an export either. The first `patch refresh` after the upgrade rewrites the `index` lines of every export, so make it the refresh that moves the forkctl pin.
