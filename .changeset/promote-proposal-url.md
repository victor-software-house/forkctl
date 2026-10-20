---
forkctl: patch
---

`publish --promote --proposal` takes the proposal pull request's URL, as its help says: forkctl reads the pull request's head with `gh`, fetches it from the downstream remote, and refuses a URL of another repository or a proposal that moved after `gh` read it.
