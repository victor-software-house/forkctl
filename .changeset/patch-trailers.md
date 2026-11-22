---
forkctl: patch
---

A patch can declare extra commit trailers, such as `Co-authored-by` to credit the author of upstream code it carries. `patch create` and `patch edit` take a repeatable `--trailer KEY=VALUE` (`FORKCTL_TRAILER`), and `patch edit --clear-trailers` (`FORKCTL_CLEAR_TRAILERS`) removes them. forkctl writes each trailer after `Drop-When` in manifest order, and `check` fails when the commit's values for a declared key differ from the manifest's. Trailers under keys a patch does not declare are not compared. The manifest field is `patches[].trailers`, a list of `key` and `value`; the JSON API takes `trailers` on `patch.create` and a replacement list on `patch.edit`. `patch show` prints the trailers, and `PATCHES.md` lists them in a `Trailers` section only when a patch declares one, so existing manifests and ledgers are unchanged.
