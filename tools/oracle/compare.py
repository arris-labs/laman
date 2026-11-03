#!/usr/bin/env python3
"""Print the comparison table of docs/COMPARISON.md: for each C1 fixture,
laman's expected status, DoF, redundant and conflict sets beside what each
solver's committed report (tools/oracle/results/<solver>/<fixture>.json)
says, a disagreement marked. Stdlib only; reads, never runs, a solver.

    python3 tools/oracle/compare.py

Conflict sets compare as sets of laman tags: a solver's set that contains
laman's minimal one and more is "superset", recorded, not called wrong. A
field a solver does not report (null) is not compared. The verdict column
says `agrees` or lists the fields that differ; a disagreement is explained by
hand in COMPARISON.md.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests" / "fixtures" / "c1"
RESULTS = ROOT / "tools" / "oracle" / "results"
SOLVERS = ["sketch-solver", "acs-solver", "ezpz"]


def tags(sets):
    return [frozenset(s) for s in sets]


def conflict_relation(want, got):
    """'=' when the same sets, 'superset' when every expected set lies inside
    one of the solver's (and the counts agree), else 'differs'."""
    want, got = tags(want), tags(got)
    if sorted(map(sorted, want)) == sorted(map(sorted, got)):
        return "="
    if len(want) == len(got) and all(any(w <= g for g in got) for w in want):
        return "superset"
    return "differs"


def fmt_set(names):
    return "{" + ", ".join(sorted(names)) + "}"


def cells(expected, r):
    """The row of one report: its four cells and the differing fields."""
    if r["outcome"] != "solved":
        why = r["detail"] or ", ".join(u["kind"] for u in r["unsupported"])
        return [r["outcome"], "", "", ""], [f"outcome {r['outcome']}: {why}"]
    differs = []
    solver_word = r["solver_status"] or ""
    if r["status"] is not None:
        status = r["status"]
        if status != expected["status"]:
            differs.append("status")
    elif solver_word.startswith("unsatisfied"):
        # One word for a conflict and for a failure to converge.
        status = "unsatisfied"
        if expected["status"] != "Unsatisfiable":
            differs.append("status")
    else:
        status = solver_word or "not reported"
        differs.append("status")
    if r["dof"] is None:
        dof = "not reported"
    else:
        dof = str(r["dof"])
        if r["dof"] != expected["dof"]:
            differs.append("dof")
    if r["redundant"] is None:
        red = "not reported"
    else:
        red = fmt_set(r["redundant"]) if r["redundant"] else "none"
        if set(r["redundant"]) != set(expected["redundant"]):
            differs.append("redundant")
    if r["conflicts"] is None:
        con = "not reported"
    elif not r["conflicts"]:
        con = "none"
        if expected["conflicts"]:
            differs.append("conflicts")
    else:
        con = "; ".join(fmt_set(s) for s in r["conflicts"])
        rel = conflict_relation(expected["conflicts"], r["conflicts"]) if expected["conflicts"] else "differs"
        if rel == "superset":
            extra = sorted(set().union(*tags(r["conflicts"])) - set().union(*tags(expected["conflicts"])))
            con += f" (superset: +{', '.join(extra)})"
            differs.append("conflicts (superset)")
        elif rel == "differs":
            differs.append("conflicts")
    return [status, dof, red, con], differs


def main():
    out = []
    versions = {}
    for s in SOLVERS:
        for f in sorted((RESULTS / s).glob("*.json")):
            versions[s] = json.loads(f.read_text())["version"]
            break
    out.append("| Fixture | Solver | Status | DoF | Redundant | Conflicts | Verdict |")
    out.append("|---|---|---|---|---|---|---|")
    disagreements = 0
    for d in sorted(p for p in FIXTURES.iterdir() if p.is_dir()):
        expected = json.loads((d / "expected.json").read_text())
        e_red = fmt_set(expected["redundant"]) if expected["redundant"] else "none"
        e_con = "; ".join(fmt_set(s) for s in expected["conflicts"]) or "none"
        out.append(f"| `{d.name}` | laman (expected) | {expected['status']} | {expected['dof']} | {e_red} | {e_con} | |")
        for s in SOLVERS:
            path = RESULTS / s / f"{d.name}.json"
            if not path.exists():
                out.append(f"| | {s} | no report | | | | |")
                disagreements += 1
                continue
            r = json.loads(path.read_text())
            row, differs = cells(expected, r)
            verdict = "agrees" if not differs else "differs: " + ", ".join(differs)
            disagreements += bool(differs)
            out.append(f"| | {s} | " + " | ".join(row) + f" | {verdict} |")
    print("Versions: " + ", ".join(f"{s} {v}" for s, v in versions.items()) + ".")
    print()
    print("\n".join(out))
    print()
    print(f"{disagreements} rows differ from laman's expectation.")


if __name__ == "__main__":
    sys.exit(main())
