## ADDED Requirements

### Requirement: Abort restores every patch under its journal name

`operation abort --yes` SHALL restore every patch under the name the operation journal records, whatever `stgit.namelength` is set to.

#### Scenario: A patch name longer than 30 characters

- **WHEN** `stgit.namelength` is unset, source patch `resolve-deleted-and-answered-threads` sits below source patch `upper`, both change `shared.txt`, `forkctl patch refresh --rewrite-below` on the lower patch stops on a conflict, and the operator runs `forkctl operation abort --yes`
- **THEN** it succeeds, HEAD equals the tip before the refresh, `stg series --all --no-prefix` matches the series before the refresh, `resolve-deleted-and-answered-threads` included, and `forkctl patch finish` and `forkctl check` succeed
