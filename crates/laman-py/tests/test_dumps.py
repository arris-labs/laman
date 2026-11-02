"""The dumps from Python: text, SVG and a solution's text name what the
sketch holds."""

import json
import xml.etree.ElementTree as ET
from typing import Any

import pytest

from laman import Sketch
from laman.check import check
from laman.debug import solution_text, svg, text
from test_c1 import NAMES, load


def tags(form: dict[str, Any]) -> list[str]:
    found = [e["tag"] for e in form["entities"] if e.get("tag")]
    return found + [c["tag"] for c in form["constraints"] if c.get("tag")]


@pytest.mark.parametrize("name", NAMES)
def test_text_and_svg_name_every_tag(name: str) -> None:
    fx = load(name)
    values = fx.sketch.coordinates()
    report = check(fx.sketch, values)
    out = text(fx.sketch, values, report)
    drawing = svg(fx.sketch, values, report)
    root = ET.fromstring(drawing)  # the SVG parses as XML
    assert root.tag.endswith("svg")
    # The tags are SVG text, with XML's escapes undone by the parser.
    shown = "".join(root.itertext())
    wanted = tags(json.loads(fx.text))
    assert wanted
    for tag in wanted:
        assert tag in out
        assert tag in shown


def test_dumps_are_repeatable() -> None:
    fx = load("belt")
    values = fx.sketch.coordinates()
    report = check(fx.sketch, values)
    assert text(fx.sketch, values, report) == text(fx.sketch, values, report)
    assert svg(fx.sketch, values, report) == svg(fx.sketch, values, report)


def test_solution_text_has_the_status_and_the_diagnostics() -> None:
    sk = Sketch()
    a = sk.point(0.0, 0.0, fixed=True)
    b = sk.point(3.0, 1.0)
    sk.distance(a, b, 5.0)
    out = solution_text(sk, sk.solve())
    assert "solution: UnderConstrained dof=1" in out
