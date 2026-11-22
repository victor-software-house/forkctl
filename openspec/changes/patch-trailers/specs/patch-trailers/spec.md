# patch-trailers Specification

## ADDED Requirements

### Requirement: A patch commit carries its declared trailers

forkctl SHALL write each trailer a patch declares in `trailers` as a `Key: value` line after `Drop-When`, in manifest order, in the same trailer block.

#### Scenario: Two co-authors follow the managed trailers

- **WHEN** the operator runs `forkctl patch create source-change -l source -p Purpose. -u not-submitted -d Drop. -s source.txt --trailer 'Co-authored-by=Ada <ada@example.com>' --trailer 'Co-authored-by=Grace <grace@example.com>'`, stages `source.txt`, and runs `forkctl patch refresh`
- **THEN** the commit message of `source-change` ends with `Drop-When: Drop.`, `Co-authored-by: Ada <ada@example.com>`, and `Co-authored-by: Grace <grace@example.com>`, one per line

### Requirement: forkctl rejects a trailer it cannot write or check

forkctl SHALL reject a trailer whose key equals `Downstream-Reason`, `Upstream-Status`, or `Drop-When` without regard to case, whose key is not ASCII letters, digits, and hyphens starting with a letter, whose value is empty or holds a line break, or whose key and value repeat another trailer of the same patch.

#### Scenario: A managed key is refused

- **WHEN** the operator runs `forkctl patch create source-change -l source -p Purpose. -u not-submitted -d Drop. -s source.txt --trailer drop-when=never`
- **THEN** it fails with `invalid_request` and writes no active patch

#### Scenario: A repeated key with another value is accepted

- **WHEN** a patch declares `Co-authored-by` twice with different values
- **THEN** manifest validation succeeds

### Requirement: check verifies the declared trailers

For each key a patch declares, `forkctl check` SHALL fail with `check_failed` unless the patch commit's values for that key equal the manifest's values for that key, in order. It SHALL ignore commit trailers whose key the patch does not declare.

#### Scenario: A missing co-author fails

- **WHEN** a patch declares two `Co-authored-by` trailers and its commit message is rewritten with only the first
- **THEN** `forkctl check` fails with `check_failed` naming `Co-authored-by`

#### Scenario: An extra co-author fails

- **WHEN** the commit message instead carries a third `Co-authored-by` value the manifest does not list
- **THEN** `forkctl check` fails with `check_failed` naming `Co-authored-by`

#### Scenario: An undeclared key is ignored

- **WHEN** the commit message also carries `Signed-off-by: Ada <ada@example.com>`
- **THEN** `forkctl check` succeeds

### Requirement: patch edit replaces the trailer list

`forkctl patch edit --trailer KEY=VALUE` SHALL replace the patch's whole trailer list with the given trailers, and `forkctl patch edit --clear-trailers` SHALL remove every trailer.

#### Scenario: Replacing and clearing

- **WHEN** the operator runs `forkctl patch edit source-change --trailer 'Co-authored-by=Ada <ada@example.com>'`, then `forkctl patch edit source-change --clear-trailers`
- **THEN** after the first edit the manifest and commit carry only that trailer, and after the second neither carries a `Co-authored-by` trailer, and `forkctl check` succeeds after each

### Requirement: The ledger lists declared trailers

`PATCHES.md` SHALL list every declared trailer with its patch, key, and value in a `Trailers` section, and SHALL omit the section when no patch declares a trailer.

#### Scenario: A ledger without trailers keeps its bytes

- **WHEN** no patch in `patches/fork.yaml` declares `trailers`
- **THEN** `PATCHES.md` has no `Trailers` heading
