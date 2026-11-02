#!/usr/bin/env python3
"""Exercise release rehearsal metadata without touching the checkout."""

from pathlib import Path
import runpy
import tempfile
import tomllib
import unittest

prepare = runpy.run_path(str(Path(__file__).with_name("release-dry-run.py")))["prepare"]


class Rehearsal(unittest.TestCase):
    def test_prepares_only_copy_and_preserves_external_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source"
            source.mkdir()
            originals = {
                "Cargo.toml": '[workspace.package]\nversion = "0.1.0-dev"\n'
                '[workspace.dependencies]\nlaman = { path = "crates/laman", version = "=0.1.0-dev" }\n',
                "Cargo.lock": 'version = 4\n[[package]]\nname = "laman"\nversion = "0.1.0-dev"\n'
                '[[package]]\nname = "external"\nversion = "0.1.0-dev"\nsource = "registry+example"\n',
                "CHANGELOG.md": "# Changelog\n\n## Unreleased\n\n- Change.\n",
            }
            for name, contents in originals.items():
                (source / name).write_text(contents)
            destination = Path(directory) / "metadata"
            self.assertEqual(prepare(source, destination), "v0.1.0")
            for name, contents in originals.items():
                self.assertEqual((source / name).read_text(), contents)
            manifest = tomllib.loads((destination / "Cargo.toml").read_text())
            self.assertEqual(manifest["workspace"]["package"]["version"], "0.1.0")
            self.assertEqual(manifest["workspace"]["dependencies"]["laman"]["version"], "=0.1.0")
            packages = tomllib.loads((destination / "Cargo.lock").read_text())["package"]
            self.assertEqual([package["version"] for package in packages], ["0.1.0", "0.1.0-dev"])
            self.assertIn("\n## 0.1.0\n", (destination / "CHANGELOG.md").read_text())
            with self.assertRaisesRegex(ValueError, "must not overwrite"):
                prepare(source, source)
            # A real stable checkout is copied byte-for-byte, without invented headings.
            stable_copy = Path(directory) / "stable"
            self.assertEqual(prepare(destination, stable_copy), "v0.1.0")
            for name in originals:
                self.assertEqual((destination / name).read_bytes(), (stable_copy / name).read_bytes())


if __name__ == "__main__":
    unittest.main()
