#!/usr/bin/env bash
# Refuse a release unless its tag, workspace version and changelog agree.
# Usage: tools/release-check.sh vX.Y.Z [NOTES] (also used by release.yml).
# With NOTES, the version's CHANGELOG.md section is written there as the
# GitHub Release's notes, and an empty section is refused.
set -euo pipefail
cd "$(dirname "$0")/.."

fail() {
  echo "release-check: $*" >&2
  exit 1
}

[ "$#" -eq 1 ] || [ "$#" -eq 2 ] || fail "usage: tools/release-check.sh vX.Y.Z [NOTES]"
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
if [ "$#" -eq 2 ]; then
  awk -v heading="## $version" '
    /^## / { inside = ($0 == heading || index($0, heading " — ") == 1); next }
    inside
  ' CHANGELOG.md > "$2"
  grep -q '[^[:space:]]' "$2" || fail "CHANGELOG.md's '## $version' section is empty"
fi
echo "release-check: $tag green"
