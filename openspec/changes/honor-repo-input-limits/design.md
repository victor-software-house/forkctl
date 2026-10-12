# Design

## Decisions

### 1. Install the limits in `App::discover`, print the warnings in `main`

`App::discover` is where forkctl first knows the repository root, and every manifest parse follows it.
It keeps the warnings as strings on `App`, and `main` prints them after discovery, because `src/app/` never prints.

Alternative considered: return the warnings inside the protocol result as notices.
It lost because ctl-core asks for one stderr line per loosened limit on every run, whatever the command, and a notice would only reach commands that render one.

### 2. A malformed file blocks the manifest, not the process

A `.ctl/input.yaml` that does not parse becomes the manifest error, so commands that need the manifest fail with `manifest_invalid` naming the file.
`operation status`, `continue`, and `abort` read the Git-private snapshot instead, so a broken limits file never blocks recovery, as for an unreadable manifest.

Alternative considered: fail `App::discover` outright.
It lost because the limits file may sit in a patch that an operation in flight has popped or left in conflict.

### 3. Shell completion keeps the default limits

Completion runs in its own process and must not write to stderr, so it does not install the repository's limits.
A manifest that only parses under loosened limits completes no patch names.

There is no override: the limits are the repository's to set in `.ctl/input.yaml`, and removing a field there disables it.
