# Configurable Conventional Commit subjects

## Outcome

Forkctl generates and verifies Conventional Commit subjects independently from immutable StGit patch names. New repositories use Conventional subjects by default, and existing repositories can migrate their complete stack through one recoverable operation.

## Non-goals

- Do not rename StGit patch identities or generated export filenames.
- Do not infer semantic commit types from changed files.
- Do not retain a permanent compatibility mode for slug-only subjects.
- Do not publish or rewrite a downstream remote during migration.

## Contract

1. Add tracked commit-message policy to the manifest.
   - Source default: `feat: <normalized patch name>`.
   - Tooling default: `chore(tooling): <normalized patch name>`.
   - Validate Conventional Commit type, scope, and one-line description.
2. Add an optional complete per-patch override for type, optional scope, and description.
   - `patch create` and `patch edit` expose the override through typed CLI/API fields.
   - Patch name remains the stable stack identity.
3. Verify every applied patch subject against its effective policy in `check`. Historical recovery commits and tags remain immutable evidence; re-enabled patches adopt the current subject before repository validation.
4. Add `contract migrate-commit-messages`.
   - Require a clean declared branch with no active patch or operation.
   - Capture immutable recovery evidence and the old patch commits.
   - Apply the configured policy to every patch through StGit.
   - Regenerate manifest, ledger, and exports, then run the complete repository check.
   - Support operation status, continue, and confirmed abort.
   - Never publish automatically.
5. Keep operator surfaces synchronized.
   - CLI and JSON schema.
   - `src/instructions.md`, `skills/forkctl/SKILL.md`, `README.md`, and `AGENTS.md`.
   - Forkctl currently has no changeset or release-fragment lane, so do not invent one in this feature.

## Proof

1. Pure manifest tests cover default rendering, overrides, and invalid Conventional fields.
2. Lifecycle tests prove create/edit subjects and subject drift rejection.
3. Migration tests prove success, exact recovery evidence, interruption/continue, abort restoration, and no remote mutation.
4. Existing YAML and JSON fixtures can invoke migration, while normal checks reject unmigrated subjects with an actionable command.
5. `mise run verify`, `mise run test:isolated`, and `mise run build` pass on `macbook-portable`.
6. Packed/bundled operator docs and schemas match the implemented surface.
