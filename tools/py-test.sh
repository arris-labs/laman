#!/usr/bin/env bash
# The Python binding: maturin builds it into crates/laman-py/.venv, then
# pytest (the binding's tests, the docstring examples and mypy.stubtest),
# then mypy --strict over the tests. Arguments go to pytest.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)/crates/laman-py"
[ -d .venv ] || uv venv -q .venv
uv pip install -q -p .venv "maturin>=1.7,<2" pytest mypy hypothesis
VIRTUAL_ENV=$PWD/.venv .venv/bin/maturin develop -q --release
.venv/bin/pytest -q "$@"
.venv/bin/mypy --strict tests
