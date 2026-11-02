#!/usr/bin/env bash
# The Python binding: maturin builds it into crates/laman-py/.venv, then
# pytest (the binding's tests, the docstring examples and mypy.stubtest),
# then mypy --strict over the tests. Arguments go to pytest.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)/crates/laman-py"
[ -d .venv ] || uv venv -q .venv
uv pip install -q -p .venv "maturin>=1.7,<2" pytest mypy hypothesis
# uv resolves bin/ on Unix and Scripts/ on Windows, and sets VIRTUAL_ENV
# for maturin. Do not sync the project: maturin installs the binding here.
uv run --no-project --python .venv maturin develop -q --release
uv run --no-project --python .venv pytest -q "$@"
uv run --no-project --python .venv mypy --strict tests
