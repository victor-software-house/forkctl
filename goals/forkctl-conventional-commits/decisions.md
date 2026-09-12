# Forkctl Conventional Commit decisions

- 2026-09-12 Conventional Commit subjects are the default for every new forkctl patch.
- 2026-09-12 Source patches default to `feat`; tooling patches default to `chore(tooling)`.
- 2026-09-12 The default description is the normalized patch name. A patch may override its complete type, optional scope, and description when the kind default is not semantically correct.
- 2026-09-12 Commit-message policy is repository-configurable and remains part of the tracked manifest contract.
- 2026-09-12 Existing non-Conventional stacks migrate through one explicit atomic local operation. The operation creates recovery evidence, rewrites the complete StGit stack, validates generated evidence, and leaves remote publication to the existing separate exact-lease command.
- 2026-09-12 Historical recovery commits and tags remain immutable. Subject policy applies to the active stack; a disabled patch receives its current configured subject when re-enabled.
- 2026-09-12 Forkctl does not retain a permanent legacy message mode. Missing policy data is accepted only so the migration command can upgrade an existing manifest.
- 2026-09-12 The three completed HerDL isolation patches keep their audited patch identifiers until the new migration is available.
