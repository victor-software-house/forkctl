# Examples

Copy `fork.json` into the consuming fork's patch directory and replace the example remote, base label, SHAs, patch metadata, paths, and required source contracts. The final bookkeeping patch must own the manifest's actual repository path, generated ledger, task configuration, and any export paths.

Each existing patch commit needs matching `Downstream-Reason`, `Upstream-Status`, and `Drop-When` trailers. Generate `PATCHES.md` from the manifest before committing the bookkeeping patch; afterward `forkctl verify` enforces it byte-for-byte.

Merge the relevant sections from `mise.toml` into the consuming repository's mise config. Keep the remote task reference immutable. If the repository has no local file tasks, remove the `mise-tasks` include; when `task_config.includes` is present, mise does not add its default task directories automatically.

The all-zero SHAs are placeholders and must be replaced with full existing commits. `forkctl init` then reconstructs StGit metadata from ordinary linear commits and runs the full structural gate.
