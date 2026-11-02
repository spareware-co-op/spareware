# G148: timing the mesh boolean stage by stage against EMBER

*Piece G148, Spare Kit geometry kernel. A decision note on how G148's done-when, "Thingi10K pairs within 3 times EMBER's 1.6 ms", is measured. Written 2026-11-02 by Claude Opus 5.5 for the owners. The decision was made by the kernel lead session for the owner on the evidence below. It does not relax the target.*

## The answer in plain words

EMBER's 1.6 ms covers less work than G148's boolean does. EMBER stops at a polygon soup: every output polygon is a part of an input polygon, with no merging and no mesh. G148's `boolean` goes on to merge the kept pieces into maximal faces and build an `ExactMesh` of the result, which is checked on the way out. On the benchmark pair these last two steps take about 45% of the instructions in a one-run profile, and about two thirds of the time when the soup is timed apart. A whole-pipeline time set against EMBER's would not compare like with like.

So G148 is timed in stages:

1. **The polygon soup**, `geom_mesh::boolean::soup`: gathering the operands, the arrangement, classification and the selection. This is the stage EMBER does, on the same input. **Its target is the done-when unchanged: 3 times EMBER's 1.6 ms, 4.8 ms, on Thingi10K pairs.** If the soup stage misses it, the target stays open.
2. **The merge and the mesh build**, the rest of `boolean`, have their own budget, derived below: **0.5 µs for each kept piece and each result face.**
3. **The whole pipeline's time is always reported beside both**, so neither stage hides the other.

## The question

What is the 3x EMBER target compared against, given that EMBER's output is a polygon soup while G148's output is a merged, built and checked mesh?

## The options

- **A. The whole `boolean` against 4.8 ms.** This is the done-when read literally. It sets a merge and a mesh build, which EMBER never runs, against EMBER's time. A miss would then say nothing about the arrangement, and a pass would need a merge faster than anything measured.
- **B. Each stage against its own target (chosen).** The soup stage is set against 3 times EMBER, like for like. The merge and build are set against a budget derived from a done-when the kernel already has. The whole time is reported beside both.
- **C. Relax the target.** This was refused: the done-when stands.

## The evidence

- **EMBER's output.** The stored evidence (G01, report 01, section 5) gives EMBER's representation and output: "Each output polygon is a subset of an input polygon, so attribute and ID transfer is trivial." See [`projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md`](https://github.com/spareware-co-op/spareware/blob/7aaf4c6c71deadb43e28010d8395ecdc49ef36f1/projects/kit/notes/G01-evidence/01-manifold-mesh-booleans.md#L130-L144), lines 130 to 144, at commit 7aaf4c6c. The paper itself is Trettner, Nehring-Wirxel and Kobbelt 2022, "EMBER: Exact Mesh Booleans via Efficient & Robust Local Arrangements", ACM TOG 41(4) 39, <https://doi.org/10.1145/3528223.3530181>: its output stage emits the classified polygons of each leaf.
- **No EMBER code exists to pin.** The same report found no public repository (*unverified*; the owners' contact with the authors is 32.9). The output format therefore rests on the paper and on the report at the commit above.
- **EMBER's timing.** The geometric mean is 1.6 ms per boolean over 1,000 Thingi10K pairs of 1k to 100k faces, on an i9-9900K. The paper reports a 5.5x speed-up on 8 cores (the same report, lines 138 to 141).
- **Where G148's time goes.** In a one-run callgrind profile of the benchmark pair, two 972-face solids overlapping in a strip, on a shared Linux benchmark machine at commit cf5b2715:
  - the arrangement, classification and selection took about 55%;
  - the merge took 38%;
  - the mesh build and its check took 7%.

## The merge and build budget, and its derivation

G146's done-when is "1M faces built and checked in 0.5 s", which is 0.5 µs per face for `build` and its checker.

The merge welds n kept pieces into maximal faces. It sorts their corners and their sides by exact keys, groups the equal ones and sweeps each line. That is the same class of work `build` does on a soup of n faces when it pools their vertices and pairs their half-edges. The result, F faces, is then built and checked once more by `build` itself. So the two together are held to two builds' worth:

> merge + build ≤ 0.5 µs × (n + F)

Here n is the number of pieces the soup keeps and F is the number of faces in the result, both counted on the same run, on the same machine as the soup stage. The benchmark (`crates/geom-mesh/benches/boolean.rs` under `projects/engines/geometry/`) prints n, F, the soup's time, the whole time, their difference and this budget for every run.

## Measured

These are the benchmark's pairs as solids, 972 faces each, at commit 49b2c054, measured on a shared Linux machine. Each figure is one run, and the machine was loaded by other work, so the numbers are noisy.

| Pair | Threads | Soup | Pieces kept | Whole | Merge and build | Its budget |
| --- | --- | --- | --- | --- | --- | --- |
| Crossing | 1 | 62.8 ms | 5,075 | 128.7 ms | 65.9 ms | 3.5 ms |
| Crossing | 8 | 17.4 ms | 5,075 | 70.8 ms | 53.4 ms | 3.5 ms |
| In a strip | 1 | 16.2 ms | 3,256 | 49.1 ms | 32.9 ms | 2.6 ms |
| In a strip | 8 | 8.5 ms | 3,256 | 31.7 ms | 23.2 ms | 2.6 ms |

Neither stage meets its target yet:

- The soup misses 4.8 ms by 1.8 times on the strip pair at 8 threads.
- The merge and build miss their budget by about 9 times. Most of the merge is serial, which is why it barely speeds up on 8 threads.

## What stays open

- **The soup stage's target.** The latest measurements are in PR #478 and the build queue. The target is met only when the soup stage of Thingi10K pairs is within 4.8 ms.
- **Thingi10K pairs are not stored.** Thingi10K models carry their own licences (32.11), so the benchmark times synthetic height-field pairs as a stand-in, and the done-when's own corpus waits on that decision.
- **The machine.** EMBER's 1.6 ms was measured on an i9-9900K, and its 5.5x speed-up was on 8 cores. Ours is timed on a shared Linux machine at 1 and 8 threads, until the reference machine of 32.16 is in place.
- **The merge and build budget is new.** It is derived from G146's done-when, which G146 itself has not yet met (0.5 s is the target, 1 s is measured). If G146's target changes, this budget follows it.
