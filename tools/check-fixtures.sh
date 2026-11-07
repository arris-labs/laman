#!/usr/bin/env bash
# A fixture's expectation is not loosened to pass (.agents/rules/solver.md,
# "Every failure becomes a fixture"). Run by the gate; red when an
# `expected.json` outside tests/fixtures/regression/ carries a "tolerance"
# key and its fixture directory has no README.md citing the ADR that decided
# it. A regression fixture may name one: it records the desired assertion.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
bad=()
while IFS= read -r f; do
  dir=$(dirname "$f")
  if ! grep -qsE 'ADR-[0-9]{4}' "$dir/README.md"; then
    bad+=("$f")
  fi
done < <(grep -rlE '"tolerance"[[:space:]]*:' tests/fixtures --include=expected.json \
           | grep -v '^tests/fixtures/regression/' | sort)
if [ ${#bad[@]} -gt 0 ]; then
  echo "check-fixtures: a tolerance key outside regression/ needs a README.md citing its ADR;" >&2
  echo "  a case that fails at the default is an ignored regression fixture instead:" >&2
  printf '  %s\n' "${bad[@]}" >&2
  exit 1
fi
