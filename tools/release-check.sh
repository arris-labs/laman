#!/usr/bin/env bash
# Refuse a release unless its tag, workspace version and changelog agree.
# Usage: tools/release-check.sh vX.Y.Z (also used by release.yml).
set -euo pipefail
cd "$(dirname "$0")/.."

fail() {
  echo "release-check: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail "usage: tools/release-check.sh vX.Y.Z"
tag=$1
version=$(awk '
  /^\[workspace.package\]$/ { package = 1; next }
  /^\[/ { package = 0 }
  package && /^version[[:space:]]*=/ {
    split($0, parts, "\""); print parts[2]
  }
' Cargo.toml)
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] ||
  fail "workspace version must be a stable X.Y.Z release (found '$version')"
[ "$tag" = "v$version" ] || fail "tag '$tag' must equal 'v$version'"
awk -v heading="## $version" '
  $0 == heading || index($0, heading " — ") == 1 { found = 1 }
  END { exit !found }
' CHANGELOG.md || fail "CHANGELOG.md needs a '## $version' release heading"
echo "release-check: $tag green"
