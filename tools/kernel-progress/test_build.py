"""Tests of the kernel progress page's counting, on a made-up board of two phases.

Run: python tools/kernel-progress/test_build.py (standard library only; no network).
"""
import json
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
        ops = build.operation_states(self.phases, OPS, CASES, FAULTS, build.case_results(CASES, FAULTS), {"G11"}, set())
        page = build.render(r, {"next": "Next gate: G13.", "source": "Counts what is pushed to GitHub.", "built": "today",
                                "ops": ops})
        self.assertIn("12%</span>", page)
        self.assertIn("1 on main, 1 built and waiting to land, 1 not started", page)
        self.assertNotIn("{{", page)
        self.assertNotIn(chr(0x2014), page)


# A made-up table of operations, stored cases and named faults for the board above.
OPS = {
    "operations": [
        {"piece": "G11", "family": "Make", "name": "Make numbers"},
        {"piece": "G20", "family": "Make", "name": "Make points"},
        {"piece": "G21", "family": "Combine", "name": "Join lines"},
        {"piece": "G22", "family": "Sweep", "name": "Sweep arcs"},
        {"piece": "G24", "family": "Query", "name": "Search arcs"},
        {"piece": "G98", "family": "Query", "name": "Not on the board"},
    ],
    "wrapper": {"point": {"piece": "G20"}, "join": {"piece": "G21"}, "number": {"piece": "G11"}},
    "case_pieces": {"odd-1": "G20"},
    "waiting": [
        {"case": "w-1", "name": "arc swept", "waits_for": "sweep", "piece": "G22"},
        {"case": "w-2", "name": "arc swept twice", "waits_for": "sweep", "piece": "G22"},
        {"case": "p-2", "name": "now a stored case", "waits_for": "points", "piece": "G22"},
        {"case": "w-3", "name": "for a piece not listed", "waits_for": "lines", "piece": "G23"},
    ],
}
CASES = {"cases": [
    {"id": "p-1", "operation": "point", "name": "a point"},
    {"id": "p-2", "operation": "point", "name": "another point"},
    {"id": "odd-1", "operation": "odd", "name": "linked one by one"},
    {"id": "j-1", "operation": "join", "name": "two lines joined"},
    {"id": "j-2", "operation": "join", "name": "two lines apart"},
    {"id": "n-1", "operation": "number", "name": "a number"},
    {"id": "x-1", "operation": "fly", "name": "an operation with no piece"},
]}
FAULTS = {"faults": [
    {"case": "j-2", "fault": "SC-9, lines apart are joined: the zone is too wide", "blocks": "join"},
    {"case": "n-1", "fault": "SC-8, numbers off by one: no reason", "blocks": "number"},
]}


class Operations(unittest.TestCase):
    def setUp(self):
        self.phases = build.read_board(BOARD_JSON, BOARD_MD)

    def states(self, results=None, main=frozenset({"G11"}), landing=frozenset({"G24"})):
        results = build.case_results(CASES, FAULTS) if results is None else results
        ops = build.operation_states(self.phases, OPS, CASES, FAULTS, results, set(main), set(landing))
        return ops, {r["piece"]: r for r in ops["rows"]}

    def test_the_four_states(self):
        ops, rows = self.states()
        # G11 is ours on main, though solidcore has a named fault on it: our own work counts first.
        # G20: every case passes on both targets. G21: a named fault. G22: cases waiting. G24: none.
        self.assertEqual({p: r["state"] for p, r in rows.items()},
                         {"G11": build.OURS, "G20": build.SC_DONE, "G21": build.SC_HELD, "G22": build.SC_HELD,
                          "G24": build.NOT_YET})
        self.assertEqual(ops["total"], {"ours": 1, "solidcore": 1, "held": 2, "none": 1})
        self.assertEqual((ops["size"], ops["share"]), (5, {"ours": 20, "solidcore": 20, "held": 40, "none": 20}))
        self.assertEqual(ops["not_on_board"], ["G98"])
        # G24 is not started by solidcore but built by us and waiting to land, and says so.
        self.assertTrue(rows["G24"]["landing"])
        self.assertEqual(ops["landing"], 1)
        self.assertEqual(build.headline(ops), (
            "40% of the kernel's 5 operations work today: 20% (1) by our own exact core and 20% (1) by solidcore. "
            "40% (2) are solidcore's but held back, and 20% (1) do not work yet: 1 built by us and waiting to land, "
            "and 0 not started.", 40))

    def test_a_named_fault_holds_its_operation_back(self):
        _, rows = self.states()
        g21 = rows["G21"]
        self.assertEqual([(f["code"], f["cases"]) for f in g21["faults"]], [("SC-9", ["j-2"])])
        self.assertEqual(build.held_reason(g21), "1 of 2 cases pass. Held back: fault SC-9 (lines apart are joined; 1 case).")
        # Even a result that says the named case passed does not make it done: the list names it as failing.
        results = {c["id"]: {"linux": True, "wasm": True} for c in CASES["cases"]}
        self.assertEqual(self.states(results)[1]["G21"]["state"], build.SC_HELD)

    def test_a_case_failing_with_no_named_fault_is_not_done(self):
        good = build.case_results(CASES, FAULTS)
        for target in ("linux", "wasm"):
            results = {k: dict(v) for k, v in good.items()}
            results["p-1"][target] = False
            g20 = self.states(results)[1]["G20"]
            self.assertEqual((g20["state"], g20["unnamed"], g20["faults"]), (build.SC_HELD, ["p-1"], []), target)
            self.assertIn("1 case fails with no named fault (p-1)", build.held_reason(g20))
        # A case with no result at all has not been shown to pass.
        results = {k: v for k, v in good.items() if k != "odd-1"}
        self.assertEqual(self.states(results)[1]["G20"]["state"], build.SC_HELD)

    def test_waiting_cases_are_counted_and_a_stored_case_is_never_waiting(self):
        _, rows = self.states()
        g22 = rows["G22"]
        self.assertEqual(([w["case"] for w in g22["waiting"]], g22["cases"]), (["w-1", "w-2"], 0))
        self.assertEqual(build.held_reason(g22), "No case can run yet. Held back: 2 cases wait for the link to "
                         "solidcore to offer the operation they need: sweep (2).")
        # p-2 is stored now, so it is judged as a case of point (G20), not as waiting.
        self.assertEqual((rows["G20"]["cases"], rows["G20"]["passing"]), (3, 3))

    def test_every_case_belongs_to_an_operation_or_is_listed(self):
        ops, _ = self.states()
        self.assertEqual(sorted(u["case"] for u in ops["unlinked"]), ["w-3", "x-1"])
        r = build.count(self.phases, {"G10"}, set(), {})
        page = build.render(r, {"next": "", "source": "", "built": "", "ops": ops})
        self.assertIn("Cases linked to no operation (2)", page)
        self.assertIn("an operation with no piece", page)
        self.assertIn("missing from the board (1)", page)
        self.assertIn("40%</span>", page)
        self.assertIn("Held back: fault SC-9", page)

    def test_shares_add_up(self):
        self.assertEqual(build.shares({"a": 1, "b": 1, "c": 1}, 3), {"a": 34, "b": 33, "c": 33})
        self.assertEqual(sum(build.shares({"a": 11, "b": 1, "c": 5, "d": 66}, 83).values()), 100)
        self.assertEqual(build.shares({"a": 0}, 0), {"a": 0})


class RepositoryTable(unittest.TestCase):
    """The real operations.json against the real board: a typo in it would drop an operation."""

    def test_the_table_names_board_pieces(self):
        root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
        with open(os.path.join(root, build.BOARD_JSON), encoding="utf-8") as f:
            board = {p["id"] for p in json.load(f)}
        with open(build.OPERATIONS, encoding="utf-8") as f:
            table = json.load(f)
        ops = [o["piece"] for o in table["operations"]]
        self.assertEqual(sorted(set(ops) - board), [])
        self.assertEqual(len(ops), len(set(ops)))
        linked = [w["piece"] for w in table["wrapper"].values()] + list(table["case_pieces"].values()) + [
            w["piece"] for w in table["waiting"]]
        self.assertEqual(sorted(set(linked) - set(ops)), [])
        self.assertTrue(all(o["name"] and o["family"] for o in table["operations"]))


if __name__ == "__main__":
    unittest.main()
