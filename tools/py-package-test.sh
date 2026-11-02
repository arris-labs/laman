#!/usr/bin/env bash
# Test the distributed package in a fresh venv. No develop/editable install:
# for an sdist, uv builds the extension from the archive in isolation.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo 'usage: tools/py-package-test.sh <wheel-or-sdist>' >&2
  exit 2
fi
venv=$(mktemp -d)
trap 'rm -rf "$venv"' EXIT
uv venv -q "$venv"
uv pip install -q -p "$venv" "$1" pytest mypy hypothesis
# Assert both Python and native module come from the installed artefact.
uv run --no-project --python "$venv" python -c '
from pathlib import Path
import sys
import laman
import laman._laman
for module in (laman, laman._laman):
    assert Path(module.__file__).resolve().is_relative_to(Path(sys.prefix).resolve()), module.__file__
'
uv run --no-project --python "$venv" pytest -q crates/laman-py/tests
uv run --no-project --python "$venv" mypy --strict crates/laman-py/tests
