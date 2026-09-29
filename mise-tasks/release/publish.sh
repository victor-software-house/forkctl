#!/bin/sh
#MISE description="From the Release workflow: attach the assets, publish the crate, then finalize the release"
set -eu

: "${TAG:?TAG names the release tag}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY names the repository}"
version=${TAG#v}
repo=$GITHUB_REPOSITORY

set -- "dist/forkctl_${version}_linux_x64.tar.gz" "dist/forkctl_${version}_macos_arm64.tar.gz"
for asset in "$@"; do
  [ -f "$asset" ] || { printf 'release: missing asset %s\n' "$asset" >&2; exit 1; }
done

work=$(mktemp -d "${TMPDIR:-/tmp}/forkctl-publish.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
# Probe the registry outside this workspace: inside it, `cargo info forkctl@VERSION`
# can succeed by reading the local package when that version is not on crates.io.
published=false
if (cd "$work" && cargo info "forkctl@$version" >/dev/null 2>&1); then
  published=true
fi
if [ "$published" = false ] && [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  printf 'release: the CARGO_REGISTRY_TOKEN secret is required before creating release state\n' >&2
  exit 1
fi

if gh release view "$TAG" --repo "$repo" >/dev/null 2>&1; then
  draft=$(gh release view "$TAG" --repo "$repo" --json isDraft --jq .isDraft)
  gh release upload "$TAG" "$@" --repo "$repo" --clobber
else
  draft=true
  gh release create "$TAG" "$@" --repo "$repo" --verify-tag --draft \
    --title "forkctl $version" --notes "forkctl $version."
fi

# Registry and GitHub release state are independent. Repair a missing crate even when
# the release is already final, and never finalize a draft before crates.io serves the
# exact version.
if [ "$published" = false ]; then
  cargo publish --locked
  attempts=0
  until (cd "$work" && cargo info "forkctl@$version" >/dev/null 2>&1); do
    attempts=$((attempts + 1))
    [ "$attempts" -lt 12 ] || { printf 'release: crates.io did not expose forkctl@%s\n' "$version" >&2; exit 1; }
    sleep 5
  done
fi
if [ "$draft" = true ]; then
  gh release edit "$TAG" --repo "$repo" --draft=false
fi

mkdir "$work/download"
for asset in "$@"; do
  name=$(basename "$asset")
  gh release download "$TAG" --repo "$repo" --pattern "$name" --dir "$work/download"
  cmp -s "$asset" "$work/download/$name" || { printf 'release: downloaded %s differs\n' "$name" >&2; exit 1; }
done
printf 'release: published forkctl %s\n' "$version"
