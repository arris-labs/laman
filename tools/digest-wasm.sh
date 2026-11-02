#!/usr/bin/env bash
# The determinism check (docs/ARCHITECTURE.md §Determinism): the digest of
# every C1 fixture and twin and each step of the drag, computed natively and
# on wasm32-wasip1 under wasmtime, printed and diffed. Exits non-zero if any line differs or a run
# fails. Needs `wasmtime` on PATH and the wasm32-wasip1 target.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
command -v wasmtime >/dev/null || { echo "digest-wasm: wasmtime not on PATH" >&2; exit 2; }
digests() { { grep '^digest ' || true; } | sort; }  # tests run in parallel: compare sorted
native=$(cargo test -p laman-debug --test determinism --quiet -- --nocapture | digests)
wasm=$(CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run" \
  cargo test -p laman-debug --test determinism --target wasm32-wasip1 --quiet -- --nocapture | digests)
[ -n "$native" ] || { echo "digest-wasm: no native digests" >&2; exit 1; }
echo "$native"
if [ "$native" != "$wasm" ]; then
  echo "digest-wasm: DIFFER, wasm32-wasip1 printed:" >&2
  echo "$wasm" >&2
  exit 1
fi
echo "digest-wasm: $(echo "$native" | wc -l) digests identical natively and on wasm32-wasip1"
