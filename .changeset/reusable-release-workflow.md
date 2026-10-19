---
forkctl: patch
---

Releases run through verctl's reusable release workflow at 0.5.0: each push to `main` publishes any version commit not yet fully released, builds its tarballs, and opens or updates the Version PR on `verctl/release/main`. The macOS gates and the mounted task still run against each new release tag.
