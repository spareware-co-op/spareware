"""Tests of the kernel progress page's counting, on a made-up board of two phases.

Run: python tools/kernel-progress/test_build.py (standard library only; no network).
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build  # noqa: E402

BOARD_MD = """### Phase 1: Ground (4 pieces)

| Phase | What it builds | Starts after | Gate |
| --- | --- | --- | --- |
| 1. Ground | The ground | Nothing | G13: ground proven |
| 2. Shapes | Shapes | Gate 1 | G25: shapes proven |

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G10 | Hosting (owner) | owner action | S | Owner | none |
| G11 | Numbers | geom-num | M | B | G10 |
| G12 | Patent search | owner action | S | Owner | none |
| G13 | Gate 1 | review | S | Gate | G11 |

### Phase 2: Shapes (6 pieces)

| ID | Piece | Crate | Size | Lane | Needs |
| --- | --- | --- | --- | --- | --- |
| G20 | Points | geom-math | M | A | G13 |
| G21 | Lines | geom-curve | M | A | G20 |
| G22 | Arcs | geom-curve | M | A | G20 |
| G23 | Licence check | owner action | S | Owner | none |
| G24 | Patent search: arcs | owner action | S | Owner | none |
| G25 | Shapes proven | engine | M | A | G21, G22 |

**Patent pieces, closed by the owner.** G12 and G24 are closed. G24 keeps a licence review, so G22 waits for it.
"""

DECISIONS_MD = """# 32. Decisions

## 32.1 Hosting (owner action G10)

Decided.

## 32.2 Licence check (owner action G23)

This action stays open until the reply.

## 32.3 Gate 1: ground proven (G13)

**Result: passed (2026-11-01, on commit abc).**
"""

BOARD_JSON = [
    {"id": i, "title": t, "phase": f"Phase {ph}", "depends_on": d}
    for i, t, ph, d in [
        ("G10", "Hosting", 1, []), ("G11", "Numbers", 1, ["G10"]), ("G12", "Patent search", 1, []),
        ("G13", "Gate 1", 1, ["G11"]), ("G20", "Points", 2, ["G13"]), ("G21", "Lines", 2, ["G20"]),
        ("G22", "Arcs", 2, ["G20"]), ("G23", "Licence check", 2, []), ("G24", "Patent search: arcs", 2, []),
        ("G25", "Shapes proven", 2, ["G21", "G22"])]
]


class Counting(unittest.TestCase):
    def setUp(self):
        self.phases = build.read_board(BOARD_JSON, BOARD_MD)
        self.decisions = build.read_decisions(DECISIONS_MD)

    def test_board(self):
        self.assertEqual([(p["n"], p["name"], p["gate"]["id"]) for p in self.phases],
                         [(1, "Ground", "G13"), (2, "Shapes", "G25")])
        # A gate in a lane of its own (G25 sits in lane A) is still the gate, not a piece.
        self.assertEqual([p["id"] for p in self.phases[1]["pieces"]], ["G20", "G21", "G22", "G23", "G24"])

    def test_decisions_and_closed_rows(self):
        owner, still_open, gates = self.decisions
        self.assertEqual((owner, still_open, gates), ({"G10"}, {"G23"}, {"G13": "2026-11-01"}))
        self.assertEqual(build.read_closed_owner_rows(BOARD_MD), {"G12"})

    def test_sources(self):
        self.assertEqual(build.ids_from_subjects([
            "G20: points", "Merge pull request #5 from org/piece/G21", "G22a: arcs, first part",
            "Merge pull request #6 from org/fix/g20-bug", "Board: G24 added"]), {"G20", "G21", "G22"})
        status = {"crates": [
            {"status": "scaffold only; built by pieces G21, G22."},
            {"status": "points (G20) built; arcs to come in G22."},
            {"status": "built by pieces G21, G22; G20 is in."},
            {"status": None}]}
        self.assertEqual(build.ids_from_map(status), {"G20"})
        self.assertEqual(build.ids_from_file_names(["crates/x/tests/attack_g21.rs", "src/g2d.rs", "src/log.rs"]), {"G21"})
        self.assertEqual(build.ids_in("G234 to G237, and G12"), {"G234", "G235", "G236", "G237", "G12"})

    def test_count(self):
        main = build.on_main(self.phases, history={"G20", "G23"}, map_ids=set(), file_ids=set(),
                             decisions=self.decisions, closed_rows=build.read_closed_owner_rows(BOARD_MD))
        # G11 is on main because passed gate G13 needs it; G23 stays open whatever the history says.
        self.assertEqual(main, {"G10", "G11", "G12", "G13", "G20"})
        r = build.count(self.phases, main, {"G21", "G25", "G99"} - main, self.decisions[2])
        p1, p2 = r["phases"]
        self.assertEqual(p1["counts"], {"main": 3, "waiting": 0, "none": 0})
        self.assertEqual(p2["counts"], {"main": 1, "waiting": 1, "none": 3})
        self.assertEqual((r["size"], r["total"]["main"], r["pct_main"], r["pct_built"]), (8, 4, 50, 62))
        self.assertEqual(p1["gate_passed"], "2026-11-01")
        self.assertEqual(build.next_gate(r), "Next gate: G25, which closes phase 2 (Shapes). It waits on 4 pieces "
                         "(1 built and waiting to land and 3 not started): G21, G22, G23, G24.")

    def test_page(self):
        r = build.count(self.phases, {"G10"}, {"G11"}, {})
        page = build.render(r, {"next": "Next gate: G13.", "source": "Counts what is pushed to GitHub.", "built": "today"})
        self.assertIn("12%</span>", page)
        self.assertIn("1 on main, 1 built and waiting to land, 1 not started", page)
        self.assertNotIn("{{", page)
        self.assertNotIn(chr(0x2014), page)


if __name__ == "__main__":
    unittest.main()
