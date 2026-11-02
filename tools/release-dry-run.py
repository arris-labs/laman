#!/usr/bin/env python3
"""Write stable release metadata to a separate directory for a CI rehearsal.

The source tree is read only. Only the runner copies receive this metadata;
neither the development version nor the changelog is committed as a release.
Requires Python 3.11 or newer (tomllib).
"""

from pathlib import Path
import re
import sys
import tomllib


def prepare(source: Path, destination: Path) -> str:
    """Return the rehearsal tag after writing the three metadata files."""
    manifest = (source / "Cargo.toml").read_text()
    version = tomllib.loads(manifest)["workspace"]["package"]["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-dev)?", version):
        raise ValueError(f"unsupported rehearsal version: {version!r}")
    stable = version.removesuffix("-dev")
    lock = (source / "Cargo.lock").read_text()
    changelog = (source / "CHANGELOG.md").read_text()
    if version != stable:
        # Update the workspace version and its exact internal dependency pins.
        manifest = manifest.replace(f'version = "{version}"', f'version = "{stable}"')
        manifest = manifest.replace(f'version = "={version}"', f'version = "={stable}"')
        # Only local packages have no registry source. External versions stay put.
        blocks = lock.split("[[package]]")
        for index in range(1, len(blocks)):
            package = tomllib.loads("[[package]]" + blocks[index])["package"][0]
            if "source" not in package and package["version"] == version:
                blocks[index] = blocks[index].replace(
                    f'version = "{version}"', f'version = "{stable}"', 1
                )
        lock = "[[package]]".join(blocks)
        if changelog.count("\n## Unreleased\n") != 1:
            raise ValueError("CHANGELOG.md needs one Unreleased heading")
        changelog = changelog.replace("\n## Unreleased\n", f"\n## Unreleased\n\n## {stable}\n", 1)
    destination.mkdir(parents=True, exist_ok=True)
    for name, contents in (("Cargo.toml", manifest), ("Cargo.lock", lock), ("CHANGELOG.md", changelog)):
        target = destination / name
        if target.resolve() == (source / name).resolve():
            raise ValueError("rehearsal metadata must not overwrite the source")
        target.write_text(contents)
    return f"v{stable}"


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: tools/release-dry-run.py DESTINATION")
    print(prepare(Path(__file__).resolve().parent.parent, Path(sys.argv[1])))
