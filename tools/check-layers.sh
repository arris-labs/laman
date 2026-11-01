#!/usr/bin/env bash
# The layer rule (docs/ARCHITECTURE.md §Crates): laman-check names only items
# under `laman::model`, never the solver or the residuals it checks. Run by
# the gate; red when any other `laman::` path appears in the checker's
# sources, comments and doctests included.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
bad=$(grep -rnoE 'laman::[A-Za-z_{]*' crates/laman-check/src | grep -vE 'laman::model$')
if [ -n "$bad" ]; then
  echo "check-layers: laman-check may name only laman::model:" >&2
  echo "$bad" >&2
  exit 1
fi
echo "check-layers: green"
