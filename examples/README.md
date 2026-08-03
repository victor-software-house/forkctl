# Examples

Copy `fork.json` into the consuming fork's patch directory, replace the example upstream and base SHAs, then declare every source and tooling patch in stack order.

Merge the relevant sections from `mise.toml` into the consuming repository's mise config. Keep the remote task reference immutable. If the repository has no local file tasks, remove the `mise-tasks` include; when `task_config.includes` is present, mise does not add its default task directories automatically.

The example's all-zero SHAs are placeholders and must be replaced with full existing commits before `forkctl` will verify the stack.
