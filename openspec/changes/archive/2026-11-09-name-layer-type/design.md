# Design

## Context

forkctl stores three contracts: the tracked manifest (`schema`), the Git-private operation journal and active-patch file (`schema` in the journal), and the JSON API (`protocol_version`).
Each reader accepts only the current number, with `deny_unknown_fields` on every record, and the repository kept no migration code.
This change renames `kind` in all three, so each number moves from 1 to 2.

## Decisions

### 1. The names

| Was | Now | Why |
|:--|:--|:--|
| `Patch.kind`, `PatchKind`, `-k/--kind` | `layer`, `PatchLayer`, `-l/--layer` | Source patches sit below tooling patches; the value picks the layer. |
| `BaseTarget.kind`, `TargetKind` | `type`, `TargetType` | The value is the target's ref type. |
| `HistoryEvent` tag `kind` | `event` | Each entry records one event. |
| `OperationState.kind`, `OperationKind` | `command`, `JournalCommand` | The value is the command that started the operation. |
| `OperationIntent` tag `kind` | `action` | The intent records what the operation does to its patch. |
| `api schema -k/--kind`, `SchemaKind` | `-d/--document`, `SchemaDocument` | The value picks the schema document. |
| `__candidates <KIND>`, `CandidateKind` | `<TYPE>`, `CandidateType` | The value picks the candidate type. |

`BaseTarget` keeps the Rust field `target_type` with `#[serde(rename = "type")]`, because the protocol already maps wire names with `serde(rename)` and no module uses raw identifiers.
`-l` is free on `patch create` and `patch edit`: `init` uses it for `--ledger`, and no global or shared argument group reaches either subcommand with it.
`-d` is free on `api schema`, whose only other options are the global `-m`, `-f`, `-c`, and `-q`.

Alternative considered for the journal: `OperationCommand`.
It lost because `src/cli.rs` already defines `OperationCommand` for the `operation` subcommands.

### 2. One explicit upgrade command

`forkctl upgrade` reads the manifest text as a generic value, finds each mapping that holds `kind` by record (base target, each patch, each disabled patch, each history entry and the target and patches inside it), and renames that key.
A YAML manifest is changed by splicing the renamed keys and the schema number at the byte ranges yamled reports, so comments, flow collections, quoting, and order survive.
A JSON manifest is written in full, as every JSON write already is.
The result must parse and validate as schema 2 before anything is written.
Then the command rewrites a draft active patch the same way, regenerates the ledger and exports, refreshes the bookkeeping patch, and runs the full check, as `contract edit` does.

The command runs only when an operator invokes it.
Normal reads accept only schema 2; a schema 1 manifest fails as `manifest_invalid` with a message that names `mise run fork upgrade`.
`src/manifest_upgrade.rs`, `src/app/upgrade.rs`, the `upgrade` command and request, and that message are removed together in one later change, tracked as work-queue row FRK-037.

Alternative considered: rename the keys on every read of a schema 1 manifest.
It lost because that is a permanent fallback reader, and it would leave the committed manifest at schema 1 until some unrelated command rewrote it.

Alternative considered: no migration, with a changeset that tells consumers to edit their manifests by hand.
It lost because a hand edit of nested history records is easy to get wrong, and the operator requires that existing files upgrade in place by a command.

### 3. An operation in progress is refused

An operation journal holds the old field names, a typed manifest snapshot, and the manifest text, and the stack it describes is mid-rewrite.
`upgrade` refuses while the journal exists and names the fix: finish or abort with the release that started it, which the mounted task's mise backend can still run.

Alternative considered: rewrite the journal, its manifest snapshot, and its manifest text too.
It lost because a continued or aborted operation then mixes commits that hold the old manifest with a journal that holds the new one, which needs its own tests for every operation type, for state that lasts only until the operator finishes it.

### 4. Automatic behaviour

`upgrade` makes no change on its own: an operator runs it, and `-n` shows the plan.
There is no override or disable path because nothing runs without the command.
