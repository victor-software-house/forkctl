# Forkctl Workflow Redesign — Decisions

- 2026-08-03 Treat the redesign as a new product contract: no compatibility readers, aliases, migrations, or fallback behavior for any prior manifest, CLI, local state, task, or JSON API shape.
- 2026-08-03 Preserve the proven protocol/view architecture and Git/StGit delegation, but reconsider every public command, argument, state object, and manifest field from first principles.
- 2026-08-03 Patch ownership is explicit. Forkctl never guesses intent or automatically chooses a patch from filenames.
- 2026-08-03 Forkctl provides smart, efficient commands and customization points; it does not monopolize Git, StGit, hook management, file globs, or operator choice.
- 2026-08-03 Hook behavior is manager-neutral. VSH uses mise plus Lefthook and receives a first-class preset, while another consumer may call the same forkctl checks from another manager.
- 2026-08-03 Staged capture is the proposed safe default because the index is an explicit snapshot and StGit natively supports `refresh --index`; all-worktree and path-scoped capture remain explicit alternatives.
- 2026-08-03 Design the complete workflow, CLI, JSON protocol, manifest, hook composition, recovery semantics, verification, release, and fleet cutover before implementation code begins.
- 2026-08-03 Use `ask_user` only for final design choices or real surviving ambiguity; do not use Plannotator for this initiative.
- 2026-08-03 PR #1 is evidence only. Reimplement its valid history, API error, report newline, and test-isolation findings in coherent layers; never merge it as the implementation base.
- 2026-08-03 Generic forkctl never mutates GitHub organization rulesets. VSH owns a narrow durable protected-branch policy for approved fork repositories.
