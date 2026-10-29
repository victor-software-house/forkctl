---
forkctl: patch
"verctl:group": fixed
---

The mounted task now requests Rust's minimal profile explicitly. It no longer inherits a machine's default profile or tries adding `rust-docs` during completion against a minimal toolchain.
