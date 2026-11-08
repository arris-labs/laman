"""The README's Python examples run as written."""

import re
from pathlib import Path

README = Path(__file__).resolve().parents[3] / "README.md"
BLOCKS = re.findall(r"```python\n(.*?)```", README.read_text(), re.S)


def test_the_readme_has_the_sketch_the_profiles_and_the_rigidity_examples() -> None:
    assert len(BLOCKS) == 3
    assert "laman.Sketch()" in BLOCKS[0]
    assert "profiles(" in BLOCKS[1] and "check_profiles" in BLOCKS[1]
    assert "laman.rigidity" in BLOCKS[2]


def test_the_readme_examples_run() -> None:
    for block in BLOCKS:
        exec(compile(block, "README.md", "exec"), {})
