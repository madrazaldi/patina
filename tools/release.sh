#!/usr/bin/env bash
# tools/release.sh 0.2.0 — bump the workspace version, commit and tag `v0.2.0`.
# Pushing the tag starts .github/workflows/release.yml, which builds a draft release.
set -euo pipefail

version=${1:?usage: tools/release.sh <version, e.g. 0.2.0 or 0.2.0-rc.1>}
version=${version#v}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "not a semver version: $version" >&2; exit 1; }

cd "$(git rev-parse --show-toplevel)"
[[ $(git branch --show-current) == main ]] || { echo "release from main" >&2; exit 1; }
[[ -z $(git status --porcelain) ]] || { echo "working tree is not clean" >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$version" >/dev/null && { echo "tag v$version already exists" >&2; exit 1; }

# Only the version line inside [workspace.package].
sed -i.bak "/^\[workspace.package\]/,/^\[/ s/^version = \".*\"/version = \"$version\"/" Cargo.toml && rm Cargo.toml.bak
cargo update --workspace --quiet

git commit -qam "Release v$version"
git tag -a "v$version" -m "Patina v$version"
echo "Tagged v$version. Push it with:"
echo "  git push origin main v$version"
