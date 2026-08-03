#!/bin/sh
#MISE description="Build and publish this machine's native forkctl asset"
#MISE depends=["verify"]
#MISE confirm={message="Publish this machine's forkctl asset?",default="no"}
set -eu

[ -z "$(git status --porcelain)" ] || { printf 'release: worktree is not clean\n' >&2; exit 1; }
[ "$(git branch --show-current)" = main ] || { printf 'release: current branch is not main\n' >&2; exit 1; }

git fetch origin main
head=$(git rev-parse HEAD)
[ "$head" = "$(git rev-parse origin/main)" ] || { printf 'release: main is not pushed exactly\n' >&2; exit 1; }

cargo build --release
version=$(target/release/forkctl --version | awk '{print $2}')
tag="v$version"

case "$(uname -s)" in
  Darwin) os=macos ;;
  Linux) os=linux ;;
  *) printf 'release: unsupported operating system\n' >&2; exit 1 ;;
esac
case "$(uname -m)" in
  arm64|aarch64) arch=arm64 ;;
  x86_64|amd64) arch=x64 ;;
  *) printf 'release: unsupported architecture\n' >&2; exit 1 ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/forkctl-release.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
cp target/release/forkctl "$work/forkctl"
asset="$work/forkctl_${version}_${os}_${arch}.tar.gz"
tar czf "$asset" -C "$work" forkctl
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)

if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  [ "$(gh api "repos/$repo/commits/$tag" --jq .sha)" = "$head" ] || { printf 'release: existing tag targets another commit\n' >&2; exit 1; }
  gh release upload "$tag" "$asset" --repo "$repo"
else
  gh release create "$tag" "$asset" --repo "$repo" --target "$head" --title "forkctl $version" --notes "Native forkctl release $version." --draft
  cargo publish --locked
  gh release edit "$tag" --repo "$repo" --draft=false
fi

download="$work/download"
mkdir "$download"
gh release download "$tag" --repo "$repo" --pattern "$(basename "$asset")" --dir "$download"
cmp -s "$asset" "$download/$(basename "$asset")"
printf 'release: published %s at %s\n' "$(basename "$asset")" "$head"
