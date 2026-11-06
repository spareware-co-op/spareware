"""Broad-phase counts for live and batch clash, from element bounding boxes.

Part of the A13 note (../A13-clash.md). It measures work, not time: how many
candidate pairs a batch run and a single edit produce at each clearance, and how
many box tests a bounding-volume hierarchy needs to find them. Those counts
don't depend on the language, so they carry over to the Rust build.

Input: two JSON files.
  boxes.json  {"<element id>": [minx, miny, minz, maxx, maxy, maxz], ...}
  cats.json   {"<category>": [<element id>, ...], ...}
Units are given with --unit (feet by default, as the Revit API reports them).
Output is totals only: no element ids, names or coordinates are printed.

    python broadphase_count.py boxes.json cats.json --unit ft --rounding 0.01
"""

import argparse
import json
import statistics
from collections import Counter

UNITS = {"ft": 0.3048, "m": 1.0, "mm": 0.001}

# Which discipline each category belongs to. Categories not listed count as
# architecture. Hosted categories are cut into their host (doors, windows).
STRUCTURE = {"framing", "columns", "foundations"}
SERVICES = {"pipes", "ducts", "trays", "conduits", "equipment"}
HOSTED = {"doors", "windows"}

CLEARANCES_MM = [0, 25, 50, 100, 300, 600]


def load(boxes_path, cats_path, unit, rounding):
    scale = UNITS[unit]
    grow = rounding / 2 * scale  # undo the report's rounding, outward
    raw = json.load(open(boxes_path))
    cats = json.load(open(cats_path))
    cat_of = {}
    for cat, ids in cats.items():
        for i in ids:
            cat_of[str(i)] = cat
    elems = []
    for key in sorted(raw, key=int):
        b = [v * scale for v in raw[key]]
        b = [b[0] - grow, b[1] - grow, b[2] - grow, b[3] + grow, b[4] + grow, b[5] + grow]
        elems.append((cat_of.get(key, "other"), b))
    return elems


def discipline(cat):
    if cat in STRUCTURE:
        return "structure"
    if cat in SERVICES:
        return "services"
    return "architecture"


class Bvh:
    """Median-split tree, leaves of up to 4 boxes. Counts every box test."""

    LEAF = 4

    def __init__(self, boxes):
        self.boxes = boxes
        self.nodes = []  # (box, left, right, items)
        self.root = self._build(list(range(len(boxes))))
        self.tests = 0

    def _enclose(self, items):
        bs = [self.boxes[i] for i in items]
        return [min(b[0] for b in bs), min(b[1] for b in bs), min(b[2] for b in bs),
                max(b[3] for b in bs), max(b[4] for b in bs), max(b[5] for b in bs)]

    def _build(self, items):
        box = self._enclose(items)
        if len(items) <= self.LEAF:
            self.nodes.append((box, -1, -1, items))
            return len(self.nodes) - 1
        axis = max(range(3), key=lambda a: box[a + 3] - box[a])
        items.sort(key=lambda i: (self.boxes[i][axis] + self.boxes[i][axis + 3], i))
        mid = len(items) // 2
        left = self._build(items[:mid])
        right = self._build(items[mid:])
        self.nodes.append((box, left, right, None))
        return len(self.nodes) - 1

    def near(self, q, gap):
        """Indices whose boxes come within `gap` of box q (closed test)."""
        out, stack = [], [self.root]
        while stack:
            box, left, right, items = self.nodes[stack.pop()]
            self.tests += 1
            if not within(box, q, gap):
                continue
            if items is None:
                stack += (left, right)
            else:
                for i in items:
                    self.tests += 1
                    if within(self.boxes[i], q, gap):
                        out.append(i)
        return out


def within(a, b, gap):
    return (a[0] - gap <= b[3] and b[0] - gap <= a[3] and
            a[1] - gap <= b[4] and b[1] - gap <= a[4] and
            a[2] - gap <= b[5] and b[2] - gap <= a[5])


def overlap_depth(a, b):
    """Smallest per-axis overlap of two boxes; negative means apart."""
    return min(min(a[k + 3], b[k + 3]) - max(a[k], b[k]) for k in range(3))


def pct(values, p):
    if not values:
        return 0
    s = sorted(values)
    return s[min(len(s) - 1, int(round(p / 100 * (len(s) - 1))))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("boxes")
    ap.add_argument("cats")
    ap.add_argument("--unit", default="ft", choices=UNITS)
    ap.add_argument("--rounding", type=float, default=0.0,
                    help="rounding step of the reported box values, in --unit")
    ap.add_argument("--large", type=float, default=60.0,
                    help="an element whose box is longer than this (metres) on any axis counts as site-scale")
    args = ap.parse_args()

    elems = load(args.boxes, args.cats, args.unit, args.rounding)
    cats = [c for c, _ in elems]
    boxes = [b for _, b in elems]
    n = len(boxes)
    disc = [discipline(c) for c in cats]
    size = [max(b[3] - b[0], b[4] - b[1], b[5] - b[2]) for b in boxes]
    degenerate = sum(1 for b in boxes if min(b[3] - b[0], b[4] - b[1], b[5] - b[2]) <= args.rounding * UNITS[args.unit] + 1e-12)
    large = [i for i in range(n) if size[i] > args.large]

    print(f"elements: {n}")
    print("by category: " + ", ".join(f"{k} {v}" for k, v in sorted(Counter(cats).items(), key=lambda kv: -kv[1])))
    print("by discipline: " + ", ".join(f"{k} {v}" for k, v in sorted(Counter(disc).items())))
    print(f"box longest side, metres: median {statistics.median(size):.2f}, p95 {pct(size, 95):.2f}, max {max(size):.1f}")
    print(f"boxes no thicker than the rounding on some axis (points, lines, sheets): {degenerate}")
    print(f"site-scale elements (> {args.large:g} m): {len(large)}")

    tree = Bvh(boxes)
    print(f"tree: {len(tree.nodes)} nodes, leaves of up to {Bvh.LEAF}")
    print()
    header = ("clearance", "batch pairs", "struct x arch", "hosted x wall", "box-touch only",
              "pairs w/o site-scale", "edit: median", "p95", "max", "max w/o site", "box tests/edit")
    print(" | ".join(header))
    for c_mm in CLEARANCES_MM:
        gap = c_mm / 1000
        tree.tests = 0
        neighbours = []
        pairs = set()
        for i in range(n):
            hits = [j for j in tree.near(boxes[i], gap) if j != i]
            neighbours.append(len(hits))
            for j in hits:
                pairs.add((min(i, j), max(i, j)))
        tests_per_edit = tree.tests / n
        cross = sum(1 for i, j in pairs if {disc[i], disc[j]} == {"structure", "architecture"})
        hosted = sum(1 for i, j in pairs if {cats[i], cats[j]} & HOSTED and "walls" in (cats[i], cats[j]))
        # With c = 0, a pair whose boxes overlap by no more than the rounding on
        # some axis is most likely a designed contact (wall on slab, beam on column).
        touch = 0
        if c_mm == 0:
            eps = args.rounding * UNITS[args.unit] * 2 + 1e-9
            touch = sum(1 for i, j in pairs if overlap_depth(boxes[i], boxes[j]) <= eps)
        big = set(large)
        no_site = sum(1 for i, j in pairs if i not in big and j not in big)
        small_nb = [neighbours[i] - sum(1 for j in large if j != i and within(boxes[j], boxes[i], gap))
                    for i in range(n) if i not in big]
        print(" | ".join(str(x) for x in (
            f"{c_mm} mm", len(pairs), cross, hosted, touch if c_mm == 0 else "-", no_site,
            int(statistics.median(neighbours)), pct(neighbours, 95), max(neighbours),
            max(small_nb) if small_nb else 0, f"{tests_per_edit:.0f}")))


if __name__ == "__main__":
    main()
