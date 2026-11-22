# G148: passing untouched faces through, and one weld instead of two

*Piece G148, Spare Kit geometry kernel. A design note on how the mesh boolean reaches its stage budgets (`G148-stage-budgets.md`), written 2026-11-02 by Claude Opus 5.5 for the owners. It follows decisions chapter 11.3 has already made. It changes no decision.*

## The question

Both of G148's stage targets are missed by about ten times. Small steps no longer close that gap; each has given 5 to 10%. Which structural changes, inside what 11.3 already decides, can reach the budgets without changing a single result?

## Where the time goes

These are one-run callgrind profiles at commit bcbe4a73: two 972-face solids, on 1 thread.

| Step | Crossing all over (1.81G instructions) | In a strip (0.70G) |
| --- | --- | --- |
| Tracing (classify) | 33% | 19% |
| Local cuts | 13% | 7% |
| Box splits | 6% | 8% |
| Faces to convex pieces | 2% | 5% |
| Merge | 22% | 38% |
| Mesh build and its check | 8% | 10% |

Measured on a shared Linux machine (`G148-stage-budgets.md`):

- **The soup stage** takes 8.5 ms (strip) and 17 ms (crossing) on 8 threads, against 4.8 ms.
- **The merge and build** take 23 to 51 ms on 8 threads, against 2.6 to 3.5 ms. Most of the merge is serial.

## What 11.3 already says

- **How faces enter, item 3:** "Faces that can meet nothing pass through untouched, without conversion."
- **Item 2:** only "a face that may meet another operand (by a broad phase on bounding boxes)" is split into convex pieces.
- **11.6:** for flat hosts and tools, faces no tool can reach "pass through untouched, names and all", and the cost is linear in the host's faces plus local work around each tool.

G148 does not do this yet. Every face of every operand is split, arranged, traced and merged. In the strip pair, about four faces in five meet nothing.

## The options

- **A. A broad phase, and pass-through for faces that meet nothing (recommended first).** A face of operand i is touched if its box, a step wider, meets the box of any face of another operand. Otherwise it is untouched. An untouched face lies wholly inside or wholly outside each other operand, so a single exact point test settles it. G147's `Classifier` gives that test: `beside`, the winding number just beside a three-plane point, at a corner of the face. One test serves each connected patch of untouched faces, since a patch cannot cross another operand's surface. A kept patch is copied into the result with its loops, edge planes and names (11.9's "passed through untouched, under its own name"). Only touched faces go through the arrangement and the merge.
- **B. One weld instead of two.** The merge numbers every point (a sorted list of exact cells) and pairs every side by line. `build` then sorts the same points again and pairs the same half-edges again. The merge can hand over its numbering and pairing, so the result is built from welded parts and only checked. In the strip pair, the second weld is most of the build's 10%.
- **C. A merge per leaf.** Each leaf merges its own pieces on the pool, then the leaves are stitched across box faces. This parallelises the merge, but the stitching is a new algorithm with its own failure modes. It should wait until A and B are measured.
- **D. Cheaper tracing.** Corner meetings can be kept per polygon across box splits, and `covers` can read from precomputed corner points. These are steady, local gains that go on in parallel with A to C.

## Recommendation

Build A, then B, measuring each; then decide whether C is needed. D continues as before.

## What A must keep exactly

- **The same mesh, bit for bit.** A property test runs random operands through both paths and compares the meshes, as `solids_give_what_their_arrays_give` does for the solid flag. geom-bench's goldens of the boolean must not move.
- **Its own coplanar neighbours still merge.** Today's merge joins coplanar neighbouring faces of one operand into a maximal face, so a triangulated input changes shape. In the benchmark, the flat base's 968 triangles come out as one face. A pass-through face therefore cannot simply be copied. The unit that passes through is a whole sheet: the connected coplanar faces of one operand. A sheet passes through only if none of its faces is touched, and it still goes through the merge's sheet step, which is cheap without the arrangement. An operand that is itself a boolean result is already maximal, so it gives single-face sheets.
- **Flush faces still merge (11.5).** A face coplanar with any face of another operand counts as touched, whatever the boxes say, so flush walls still merge into one face with a merged name.
- **No T-junctions.** Where an untouched face shares an edge with a touched face of its own operand, the touched side's cuts can put new vertices on that edge. The untouched face's loop gains the same vertices, which come from the merge's numbering, so the result stays conforming.
- **Names and history.** An untouched face keeps its run, and its record is "unchanged" (20.3). Runs that are split between the two paths are recorded as they would be today.

## Open risks

- **The broad phase is a box test**, so its savings depend on how much of each operand lies far from the others. The crossing pair, whose tops cross everywhere, gains little from A; only B, C and D help there.
- **Stitching the edges between the two kinds of face** is new code on the path to every result. It needs the same attack tests as the merge: T-junctions, coplanar flush faces, and an untouched face whose whole boundary is touched.
- **Thingi10K pairs** (32.11) are mostly closed meshes placed to overlap. How much of them passes through is unknown until they are measured.
