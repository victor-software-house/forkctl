# Research

Each source below treats generated output as a function of its inputs, never as content to merge.

1. [Debian][debian] regenerates build-system output at build time and keeps it out of its patches.
2. [`jj fix`][jj-fix] runs a deterministic tool on a commit's tree and promises no new conflicts.
3. [git-branchless `git test fix`][branchless] does the same, and [keys its results by tree][tree-key] so they survive a rebase.
4. [Bazel][bazel] keys an action by its declared inputs and command, and its [sandboxing page][sandbox] shows why an undeclared input makes an incremental result wrong.
5. [npm-merge-driver][npm] resolves lockfile conflicts by regenerating the file, one conflicting file at a time, before the rest of the patch has merged.

[debian]: https://www.debian.org/doc/manuals/debmake-doc/ch10.en.html
[jj-fix]: https://github.com/jj-vcs/jj/blob/9ff73e7f/cli/src/commands/fix.rs
[branchless]: https://github.com/arxanas/git-branchless/discussions/803
[tree-key]: https://docs.rs/git-branchless-lib/latest/branchless/git/fn.get_test_tree_dir.html
[bazel]: https://bazel.build/remote/caching
[sandbox]: https://bazel.build/docs/sandboxing
[npm]: https://github.com/npm/npm-merge-driver/
