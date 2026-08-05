# Examples

Prefer `forkctl init` to create a new manifest rather than copying JSON. Bootstrap requires a branch exactly at the selected upstream base and explicit upstream/downstream/document/bookkeeping arguments; it never imports legacy commits.

`fork.yaml` shows the complete tracked shape after source and tooling patches exist. Source exports are deterministic under `documents.exports`; do not declare per-patch export paths. Persistent ownership uses `scope` globs where `*` stays within a path segment and `**` crosses directories.

Merge the relevant sections from `mise.toml` into the consumer. The immutable remote catalog exposes one mounted `fork` task whose Usage grammar comes directly from forkctl's Clap tree:

```sh
mise run fork status
mise run fork check -s
mise run fork patch refresh
```

Keep `mise-tasks` only when the repository has local file tasks because explicit `task_config.includes` disables mise's default task-directory discovery.

A concise optional Lefthook integration is:

```yaml
pre-commit:
  commands:
    forkctl-staged:
      run: mise run fork check -s
pre-push:
  commands:
    forkctl-check:
      run: mise run fork check -q
```

Forkctl never installs or rewrites hook-manager configuration.
