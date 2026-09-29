# Design

## Context

Bootstrap called `require_clean` first, which counts every untracked file. See
[proposal.md](proposal.md).

## Decisions

1. **Use the bookkeeping scope as the allow list.** The operator already
   declares with `--bookkeeping-path` which files the bookkeeping patch owns,
   and forkctl checks that scope on every refresh. The alternative was a fixed
   list of known files such as `mise.toml`. It lost because forkctl never
   routes changes by filename inference, as [AGENTS.md][agents] requires.
2. **Capture untracked files only.** A staged or unstaged change to a tracked
   file is an edit to upstream content and still refuses. The alternative was
   to capture any change in scope. It lost because a tracked file at the base
   belongs to upstream until a patch claims it.
3. **Check before any mutation.** The dirty check runs after argument
   parsing and before remotes, pushes, or StGit state change.
4. **No override and no disable path.** An operator who does not want a file
   captured leaves it out of `--bookkeeping-path` or removes it first.

## Risks / Trade-offs

1. [A broad `--bookkeeping-path` glob captures more than intended.] → The
   dry-run plan lists every captured file under `writes`.

[agents]: ../../../AGENTS.md
