#!/usr/bin/env bash
# The layer rule (docs/ARCHITECTURE.md §Crates): laman-check names only items
# under `laman::model`, never the solver or the residuals it checks. Run by
# the gate; red when any other `laman::` path appears in the checker's
# sources, comments and doctests included. The oracle (tools/oracle) holds to
# the same rule: model types and the serde form only, never the solver.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
bad=$(grep -rnoE 'laman::[A-Za-z_{]*' crates/laman-check/src tools/oracle/src | grep -vE 'laman::model$')
if [ -n "$bad" ]; then
  echo "check-layers: laman-check and tools/oracle may name only laman::model:" >&2
  echo "$bad" >&2
  exit 1
fi
# ADR-0007: the binding carries laman-debug without its strategies, so
# `proptest` stays out of the extension module.
if ! tree=$(cargo tree -p laman-py -e normal --prefix none); then
  echo "check-layers: cargo tree -p laman-py failed" >&2
  exit 1
fi
if grep -q '^proptest ' <<<"$tree"; then
  echo "check-layers: proptest is in laman-py's dependencies (ADR-0007)" >&2
  exit 1
fi
echo "check-layers: green"
