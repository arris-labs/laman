#!/usr/bin/env bash
# Exercise the release guard in an isolated tree, never changing the checkout.
set -euo pipefail
cd "$(dirname "$0")/.."
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir "$fixture/tools"
cp tools/release-check.sh "$fixture/tools/"

manifest() {
  cat > "$fixture/Cargo.toml" <<EOF
[workspace.package]
version = "$1"
[workspace.dependencies]
unrelated = { version = "9.9.9" }
EOF
}

expect() {
  local expected=$1 name=$2 actual=0
  shift 2
  bash "$fixture/tools/release-check.sh" "$@" > "$fixture/output" 2>&1 || actual=$?
  if [ "$actual" -ne "$expected" ]; then
    echo "release-check-test: $name: expected exit $expected, got $actual" >&2
    cat "$fixture/output" >&2
    exit 1
  fi
}

manifest 0.1.0
printf '# Changelog\n\n## 0.1.0 — 2026-11-02\n' > "$fixture/CHANGELOG.md"
expect 0 'matching release' v0.1.0
printf '## 0.1.0\n' > "$fixture/CHANGELOG.md"
expect 0 'undated release heading' v0.1.0
expect 1 'mismatched tag' v0.2.0
expect 1 'missing tag prefix' 0.1.0
expect 1 'missing argument'
expect 1 'extra argument' v0.1.0 extra
manifest 0.1.0-dev
expect 1 'development version' v0.1.0-dev
manifest 0.1.0
printf '## Unreleased\n' > "$fixture/CHANGELOG.md"
expect 1 'missing heading' v0.1.0
printf '## 0.1.00\nA mention of ## 0.1.0\n### 0.1.0\n' > "$fixture/CHANGELOG.md"
expect 1 'version mention is not a release heading' v0.1.0
echo 'release-check-test: green'
