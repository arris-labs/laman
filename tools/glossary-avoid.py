#!/usr/bin/env python3
"""Print every word GLOSSARY.md lists under _Avoid_ found in the given files.

Advisory, not a gate stage: an avoided word is often ordinary English
("result", "name") and only a hit that names one of laman's own concepts is
replaced by its term. Run by /plan on the plan and by /work on any ADR or
design doc a step writes (.agents/skills), because an ADR is append-only and
keeps its words for good.

Usage: tools/glossary-avoid.py FILE...
Prints `file:line: word -> Term` per hit; exit 0 always.
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
avoid = {}  # avoided word -> the term that replaces it
term = None
for line in (root / "GLOSSARY.md").read_text(encoding="utf-8").splitlines():
    m = re.match(r"\*\*(.+?)\*\*:", line)
    if m:
        term = m.group(1)
    elif line.startswith("_Avoid_:") and term:
        words = re.sub(r"\([^)]*\)", "", line[len("_Avoid_:"):])
        for w in words.split(","):
            w = w.strip()
            if w:
                avoid.setdefault(w.lower(), term)

patterns = [
    (re.compile(r"\b" + re.escape(w) + r"(s|es)?\b", re.IGNORECASE), w, t)
    for w, t in sorted(avoid.items())
]
for name in sys.argv[1:]:
    path = pathlib.Path(name)
    if path.resolve() == (root / "GLOSSARY.md"):
        continue
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        for pat, w, t in patterns:
            if pat.search(line):
                print(f"{name}:{n}: {w} -> {t}")
