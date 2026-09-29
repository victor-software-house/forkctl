#!/bin/sh
#MISE description="Tag the pushed main commit; the Release workflow builds and publishes it"
#MISE confirm={message="Push the forkctl release tag?",default="no"}
set -eu

[ -z "$(git status --porcelain)" ] || { printf 'release: worktree is not clean\n' >&2; exit 1; }
[ "$(git branch --show-current)" = main ] || { printf 'release: current branch is not main\n' >&2; exit 1; }

git fetch origin main
head=$(git rev-parse HEAD)
[ "$head" = "$(git rev-parse origin/main)" ] || { printf 'release: main is not pushed exactly\n' >&2; exit 1; }

# [workspace.package].version is the only release-version source.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)
case "$version" in
  ''|*[!0-9.]*) printf 'release: cannot read the workspace version from Cargo.toml\n' >&2; exit 1 ;;
esac
tag="v$version"

remote=$(git ls-remote origin "refs/tags/$tag^{}" | cut -f1)
if [ -n "$remote" ]; then
  [ "$remote" = "$head" ] || { printf 'release: %s already tags another commit\n' "$tag" >&2; exit 1; }
  printf 'release: %s already tags %s; rerun the Release workflow to resume it\n' "$tag" "$head"
  exit 0
fi

git tag --annotate "$tag" --message "forkctl $version" "$head"
git push origin "refs/tags/$tag"
printf 'release: pushed %s at %s; the Release workflow builds, publishes, and finalizes it\n' "$tag" "$head"
