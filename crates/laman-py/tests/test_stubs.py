"""The typed stubs match the extension module (mypy.stubtest) and type-check strictly."""

import subprocess
import sys


def test_stubtest() -> None:
    subprocess.run([sys.executable, "-m", "mypy.stubtest", "laman"], check=True)
