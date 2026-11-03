#!/usr/bin/env bash
# Run every solver on every C1 fixture, each in its own process under a
# timeout, and write tools/oracle/results/<solver>/<fixture>.json. A panic,
# a signal or a hang in a foreign solver becomes a report (`crashed`,
# `timeout`), never a red run. By hand, never in the gate or in CI.
#
#   tools/oracle/run.sh [solver ...]      default: the three solvers
#   ORACLE_TIMEOUT=30 tools/oracle/run.sh
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
solvers=("$@")
[ ${#solvers[@]} -eq 0 ] && solvers=(sketch-solver acs-solver ezpz)
limit=${ORACLE_TIMEOUT:-60}
cargo build --release --quiet --manifest-path tools/oracle/Cargo.toml || exit 1
bin=tools/oracle/target/release/laman-oracle
for solver in "${solvers[@]}"; do
  mkdir -p "tools/oracle/results/$solver"
  for dir in tests/fixtures/c1/*/; do
    dir=${dir%/}
    name=${dir##*/}
    out="tools/oracle/results/$solver/$name.json"
    err=$(mktemp)
    if timeout "$limit" "$bin" "$solver" "$dir" >"$out.tmp" 2>"$err"; then
      mv "$out.tmp" "$out"
    else
      status=$?
      rm -f "$out.tmp"
      if [ "$status" -eq 124 ]; then
        "$bin" crashed "$solver" "$dir" timeout "no result in ${limit} s" >"$out"
      else
        "$bin" crashed "$solver" "$dir" crashed \
          "exit $status: $(tail -c 300 "$err" | tr '\n' ' ')" >"$out"
      fi
    fi
    rm -f "$err"
  done
done
