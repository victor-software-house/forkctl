---
forkctl: patch
---

Commands that change a YAML manifest now edit only the nodes they change, so comments, key order, and formatting elsewhere in `patches/fork.yaml` survive `patch refresh`, `patch edit`, `patch disable`, `patch enable`, `patch remove`, `contract edit`, `publish --set-default`, and `rebase`.
The manifest is read through ctl-core's input reader: anchors, aliases, and any consistent indentation are accepted, and duplicate keys and merge keys are still refused. An edit that would go through an anchor or alias fails with `manifest_invalid` and leaves the file unchanged.
