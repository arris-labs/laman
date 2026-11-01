#!/usr/bin/env bash
# The Python binding: maturin builds it into crates/laman-py/.venv, then
# pytest (the binding's tests and mypy.stubtest). Arguments go to pytest.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)/crates/laman-py"
[ -d .venv ] || uv venv -q .venv
uv pip install -q -p .venv "maturin>=1.7,<2" pytest mypy
VIRTUAL_ENV=$PWD/.venv .venv/bin/maturin develop -q --release
.venv/bin/pytest -q "$@"
