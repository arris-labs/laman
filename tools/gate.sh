#!/usr/bin/env bash
# The gate (.agents/rules/git.md): every stage runs, and the last lines name
# each red one. Run by .githooks/pre-commit and by CI's `check` job.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
red=()
stage() {
  local name=$1; shift
  echo "gate: $name"
  "$@" || red+=("$name")
}
stage fmt cargo fmt --all -- --check
stage clippy cargo clippy --workspace --all-targets -- -D warnings
stage test cargo test --workspace --quiet
stage rustdoc env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet
stage wasm32 cargo build -p laman --target wasm32-unknown-unknown --quiet
if [ ${#red[@]} -gt 0 ]; then
  echo "gate: RED: ${red[*]}" >&2
  exit 1
fi
echo "gate: green"
