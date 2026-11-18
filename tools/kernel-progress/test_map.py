"""Tests of the kernel map: every crate drawn once, every operation once under the engine the data
says, and a build that stops when the repository has a crate, operation, class or tolerance that
map.json does not cover.

Run: python tools/kernel-progress/test_map.py from the repository root (standard library only; no
network). The Site workflow runs it before it builds the page.
"""
import copy
import json
import os
import re
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build  # noqa: E402
import build_map as bm  # noqa: E402

CLASSES = [("Exact", "decided exactly."), ("Certified", "within a proven bound."), ("Approximate", "within a measured bound.")]
LAYERS = {"1": "Numbers", "12": "Format and exchange", "13": "Facade"}

MAP = {"crates": [
    {"name": "engine-geometry", "layer": "13", "job": "The facade."},
    {"name": "geom-num", "layer": "1", "job": "Numbers."},
    {"name": "geom-format", "layer": "12", "job": "The file."},
    {"name": "geom-test", "layer": "Development", "job": "Tests."},
]}

TOOLS = [{"name": "make.box", "family": "Make", "classes": ["Exact"]},
         {"name": "query.volume", "family": "Query", "classes": ["Exact", "Certified"]}]

SOLIDCORE = {"pin": "a" * 40, "fork": "https://example.invalid/fork", "licence": "Apache-2.0", "wrapper": "geom-solidcore",
             "cases": 3, "faults": 1, "fault_cases": 1, "tolerances": {"max_residual": 1e-9}}

FILES = {"plan/features.json": json.dumps({"name": "Spare Construct"}), "plan/A.md": "**02 Civil**: the ground."}

DATA = {
    "apps": {"program": "plan/features.json", "note": "One program.",
             "items": [{"name": "Civil", "for": "The site", "source": "plan/A.md", "find": "**02 Civil**"}]},
    "placement": {"interface": "13", "formats": "12"},
    "layers": {"13": "What programs call", "12": "Files", "1": "Numbers", "Development": "Testing tools"},
    "crates": {"engine-geometry": "The door", "geom-num": "Exact numbers", "geom-format": "Our file", "geom-test": "Tests"},
    "interface": {"Make": "G1", "Query": "G1"},
    "classes": {"gloss": {"Exact": "Exactly right", "Certified": "Proven close", "Approximate": "Measured close"},
                "ours": {"G2": {"class": "Exact", "from": "chapter 11"}}},
    "engines": {"core": {"name": "Our exact core", "for": "Ours"},
                "solidcore": {"name": "solidcore", "for": "Theirs", "class": "Approximate", "why": "Measured."},
                "none": {"name": "Not answered yet", "for": "Nothing yet"},
                "tolerances": {"max_residual": "points within {} of both surfaces"}},
}


def row(piece, state, landing=False):
    return {"piece": piece, "name": "Operation " + piece, "title": piece, "family": "Make", "state": state, "cases": 2,
            "passing": 1, "faults": [{"code": "SC-1", "what": "a fault", "cases": ["c1"]}] if state == build.SC_HELD else [],
            "unnamed": [], "waiting": [], "landing": landing}


OPS = {"rows": [row("G1", build.OURS), row("G2", build.OURS), row("G3", build.SC_DONE), row("G4", build.SC_HELD),
                row("G5", build.NOT_YET, landing=True), row("G6", build.NOT_YET)]}

EXPECTED_ENGINE = {"G1": bm.CORE, "G2": bm.CORE, "G3": bm.SOLIDCORE, "G4": bm.NONE, "G5": bm.NONE, "G6": bm.NONE}


def make(data=DATA, map_json=MAP, tools=TOOLS, ops=OPS, classes=CLASSES, solidcore=SOLIDCORE, files=FILES, layers=LAYERS):
    return bm.kernel_map(map_json, layers, tools, ops, data, classes, solidcore, files)


def page(tree):
    return bm.render_map(tree, {"built": "today"})


class Tree(unittest.TestCase):
    def test_every_crate_once_in_its_place(self):
        tree = make()
        self.assertEqual([c["name"] for c in tree["interface"]["crates"]], ["engine-geometry"])
        self.assertEqual([c["name"] for c in tree["formats"]["crates"]], ["geom-format"])
        self.assertEqual([(l["layer"], [c["name"] for c in l["crates"]]) for l in tree["layers"]],
                         [("1", ["geom-num"]), ("Development", ["geom-test"])])
        html = page(tree)
        for c in MAP["crates"]:
            self.assertEqual(html.count(f'<span class="nm">{c["name"]}</span>'), 1, c["name"])

    def test_every_operation_once_under_its_engine(self):
        tree = make()
        got = {op["piece"]: eng for eng, ops in tree["engines"].items() for op in ops}
        self.assertEqual(got, EXPECTED_ENGINE)
        html = page(tree)
        for pid, eng in EXPECTED_ENGINE.items():
            self.assertEqual(re.findall(rf'data-piece="{pid}" data-engine="(\w+)"', html), [eng], pid)

    def test_classes_come_from_the_interface_map_json_and_the_engine(self):
        ops = {op["piece"]: op for ops in make()["engines"].values() for op in ops}
        self.assertEqual(ops["G1"]["classes"], ["Exact", "Certified"])  # the union, in chapter 9's order
        self.assertEqual(ops["G1"]["calls"], ["make.box", "query.volume"])
        self.assertEqual(ops["G2"]["classes"], ["Exact"])
        self.assertEqual(ops["G3"]["classes"], ["Approximate"])
        self.assertEqual(ops["G4"]["classes"], [])
        self.assertIn("SC-1", ops["G4"]["why"])
        self.assertEqual(ops["G5"]["why"], "Built by us, waiting to land.")

    def test_the_page_has_no_gap_and_the_kernels_words(self):
        html = page(make())
        self.assertNotIn("{{", html)
        self.assertIn("decided exactly.", html)
        self.assertIn("points within 1e-09 of both surfaces", html)
        self.assertIn("aaaaaaaaa", html)
        self.assertIn('href="./kernel-progress.html"', html)


class SeededWrongInputs(unittest.TestCase):
    def fails(self, expect, **kw):
        with self.assertRaises(bm.MapError) as err:
            make(**kw)
        self.assertTrue(any(expect in p for p in err.exception.args[0]), err.exception.args[0])

    def test_a_crate_with_no_words_fails(self):
        m = copy.deepcopy(MAP)
        m["crates"].append({"name": "geom-new", "layer": "1", "job": "New."})
        self.fails("crate geom-new is in MAP.json but has no plain words", map_json=m)

    def test_words_for_a_crate_that_is_gone_fail(self):
        d = copy.deepcopy(DATA)
        d["crates"]["geom-gone"] = "Gone"
        self.fails("geom-gone, which MAP.json does not list", data=d)

    def test_a_layer_chapter_27_does_not_name_fails(self):
        m = copy.deepcopy(MAP)
        m["crates"][1]["layer"] = "7"
        self.fails("layer '7', which chapter 27's layer table does not name", map_json=m)

    def test_a_placement_at_a_layer_chapter_27_does_not_name_fails(self):
        d = copy.deepcopy(DATA)
        d["placement"]["formats"] = "14"
        self.fails("places the formats at layer '14'", data=d)

    def test_an_operation_of_our_core_with_no_class_fails(self):
        d = copy.deepcopy(DATA)
        del d["classes"]["ours"]["G2"]
        self.fails("G2 is answered by our core, but neither", data=d)

    def test_an_operation_missing_from_the_data_fails(self):
        ops = copy.deepcopy(OPS)
        ops["rows"].append(row("G7", build.OURS))
        self.fails("G7 is answered by our core", ops=ops)

    def test_a_class_chapter_9_does_not_have_fails(self):
        d = copy.deepcopy(DATA)
        d["classes"]["ours"]["G2"]["class"] = "Exactish"
        self.fails("the class 'Exactish'", data=d)
        d = copy.deepcopy(DATA)
        d["engines"]["solidcore"]["class"] = "Close enough"
        self.fails("solidcore's answers the class 'Close enough'", data=d)
        self.fails("has no plain gloss", classes=CLASSES + [("Rough", "rough.")])

    def test_an_interface_family_with_no_operation_fails(self):
        self.fails("of family 'Draw', which map.json links to no operation",
                   tools=TOOLS + [{"name": "draw.section", "family": "Draw", "classes": ["Exact"]}])
        d = copy.deepcopy(DATA)
        d["interface"]["Make"] = "G99"
        self.fails("to G99, which is not an operation", data=d)

    def test_a_tolerance_with_no_words_fails(self):
        sc = dict(SOLIDCORE, tolerances={"max_residual": 1e-9, "angle": 1e-3})
        self.fails("tolerance angle has no words", solidcore=sc)

    def test_an_app_the_plan_no_longer_names_fails(self):
        self.fails("plan/A.md no longer says", files={**FILES, "plan/A.md": "**02 Survey**"})


class Repository(unittest.TestCase):
    """The real files, read as the Site workflow reads them."""

    @classmethod
    def setUpClass(cls):
        os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
        cls.classes = bm.read_classes(build.read(bm.CLASSES_MD))
        out = os.path.join(tempfile.mkdtemp(), "kernel-map.html")
        cls.tree = bm.build_map(out, offline=True)
        cls.html = build.read(out)
        cls.map = json.loads(build.read(build.MAP))

    def test_chapter_9s_classes_are_the_interfaces(self):
        names = [c for c, _ in self.classes]
        self.assertEqual(names, ["Exact", "Certified", "Approximate"])
        schema = build.read("projects/engines/geometry/crates/geom-op/src/schema.rs")
        block = schema.split("pub enum Class {", 1)[1].split("}", 1)[0]
        self.assertEqual(re.findall(r"^\s+([A-Z]\w+),", block, re.M), names)

    def test_every_crate_of_map_json_is_drawn_once(self):
        for c in self.map["crates"]:
            self.assertEqual(self.html.count(f'<span class="nm">{c["name"]}</span>'), 1, c["name"])

    def test_every_operation_is_drawn_once_under_the_engine_its_state_says(self):
        table = json.loads(build.read(build.OPERATIONS))
        engine = {build.OURS: bm.CORE, build.SC_DONE: bm.SOLIDCORE, build.SC_HELD: bm.NONE, build.NOT_YET: bm.NONE}
        drawn = {op["piece"]: op for ops in self.tree["engines"].values() for op in ops}
        self.assertEqual(sorted(drawn), sorted(o["piece"] for o in table["operations"]))
        for pid, op in drawn.items():
            self.assertEqual(re.findall(rf'data-piece="{pid}" data-engine="(\w+)"', self.html), [engine[op["state"]]], pid)

    def test_the_apps_are_in_order_and_the_link_back_is_there(self):
        self.assertEqual([a["name"] for a in self.tree["apps"]], ["Civil", "Construct", "Workshop"])
        self.assertIn('href="./kernel-progress.html"', self.html)
        self.assertIn('href="./kernel-map.html"', build.read(os.path.join(build.HERE, "page.html")))


if __name__ == "__main__":
    unittest.main(verbosity=1)
