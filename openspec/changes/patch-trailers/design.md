# Design

## Context

[`Patch::message`][manifest] writes the patch name and three trailers, and [`check_patch_commit`][check] reads each trailer back with `git log --format=%(trailers:key=KEY,valueonly,unfold=true)`.
The manifest has no place for any other trailer, and a hand-amended commit message is rewritten by the next `patch edit`.

## Decisions

1. **A list of key and value records.**
   A patch declares `trailers: [{key, value}]`.
   A map from key to value lost because a patch with two co-authors repeats `Co-authored-by`.
   A map from key to a list of values lost because it drops the order across keys, which the commit message keeps.
2. **The three managed keys are reserved, compared without case.**
   Git matches a trailer key without regard to case, so a declared `drop-when` would add a second value to the trailer forkctl owns.
3. **A trailer key is a Git trailer token.**
   ASCII letters, digits, and hyphens, starting with a letter.
   Accepting every key Git accepts lost because a key with a space or a colon reads differently in `git interpret-trailers` and in the manifest.
4. **`check` compares only the keys the manifest names.**
   For each declared key, the commit's values for that key, one per line, must equal the manifest's values for that key, in order.
   A commit value the manifest does not list fails, and so does a missing one.
   Requiring the whole trailer block to match lost because hooks and signing tools add trailers such as `Signed-off-by` or `Change-Id`, which forkctl does not own.
5. **`patch edit --trailer` replaces the whole list.**
   A trailer has no name of its own to replace by, so an additive edit could never remove one value.
   `--clear-trailers` removes every trailer, and conflicts with `--trailer`.
6. **The ledger lists trailers only when a patch declares one.**
   `PATCHES.md` gains a `Trailers` table, with one row per trailer in stack and manifest order.
   An always-present section lost because every existing ledger would then differ from its render and fail `check` until each consumer refreshed it, for a field none of them uses.
   The section follows the manifest: declaring the first trailer adds it, and removing the last one removes it.

## Risks

- A repository whose hooks add a trailer with a key the manifest also declares, such as a hook that appends `Co-authored-by`, fails `check`.
  The operator then declares that value in the manifest too, or removes it from the hook.

[manifest]: ../../../src/manifest.rs
[check]: ../../../src/app/check.rs
