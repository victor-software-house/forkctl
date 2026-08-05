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
version_output=$(target/release/forkctl --version)
version=${version_output#forkctl }
[ "$version" != "$version_output" ] || { printf 'release: unexpected version output: %s\n' "$version_output" >&2; exit 1; }
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
published=false
draft=false
# Run registry probes outside this package workspace. Inside the workspace,
# `cargo info forkctl@VERSION` can succeed by inspecting the local package even
# when that version does not exist on crates.io.
if (cd "$work" && cargo info "forkctl@$version" >/dev/null 2>&1); then
  published=true
fi
if [ "$published" = false ] && [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  printf 'release: CARGO_REGISTRY_TOKEN is required before creating release state\n' >&2
  exit 1
fi

if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  draft=$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft)
  if [ "$draft" = true ]; then
    target=$(gh release view "$tag" --repo "$repo" --json targetCommitish --jq .targetCommitish)
    [ "$target" = "$head" ] || { printf 'release: existing draft targets another commit\n' >&2; exit 1; }
  else
    [ "$(gh api "repos/$repo/commits/$tag" --jq .sha)" = "$head" ] || { printf 'release: existing tag targets another commit\n' >&2; exit 1; }
  fi
  gh release upload "$tag" "$asset" --repo "$repo" --clobber
else
  draft=true
  gh release create "$tag" "$asset" --repo "$repo" --target "$head" --title "forkctl $version" --notes "Native forkctl release $version." --draft
fi

# Registry and GitHub release state are independent. Repair a missing crate even
# when a matching GitHub release was already finalized, and never finalize a new
# draft until crates.io confirms the exact version is readable.
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
  gh release edit "$tag" --repo "$repo" --draft=false
fi

download="$work/download"
mkdir "$download"
gh release download "$tag" --repo "$repo" --pattern "$(basename "$asset")" --dir "$download"
cmp -s "$asset" "$download/$(basename "$asset")"
printf 'release: published %s at %s\n' "$(basename "$asset")" "$head"
