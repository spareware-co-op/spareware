# G139: the incremental rebuild of a local re-cut

Piece G139 ("Arrangement cache and incremental re-cut", chapter 11.17 item 14) re-cuts a host after
one tool moves. Its benchmark gate: one door moved in a 300-opening wall is re-cut and certified
within 16 ms native and 48 ms on wasm32. The arrangement cache (22120f47) solves again only the
boxes the moved tool touches, and the merge's sorts were made cheaper (a70362bb), but the re-cut
still takes about 280 ms on the owner's workstation. This note says why, what the rest of the
re-cut must keep from the cut before it, and the law that holds it. The owner decided on
2026-11-03 that this is G139's own work, not a new piece, and that the 16 ms target stays.

## The question

What must a re-cut keep from the previous cut, beyond the solved boxes, so that its cost follows
what the move touched rather than the size of the wall, while its result stays bit for bit the
result of a full cut?

## Where the time goes

Callgrind on D03, around one door move in the 300-opening wall (a70362bb, 1.66 billion
instructions):

| Stage | Share | What it does for the whole wall each time |
| --- | --- | --- |
| Merge (`geom_poly::merge::merge_with`) | 43% | Sorts every corner by exact coordinates (9%) and every side by line (12%), sweeps every line, rings every sheet, orders fins about every edge, nests every shell (10%) |
| Naming (`geom_bool::names`) | 26% | Names every face, edge and vertex; a name's identity is a SHA-256 of its parts (15%) |
| The bounding cell (`geom_poly::subdivide::around`) | 14% | Rounds every corner of every polygon to the grid |
| Box keys (G139's cache) | 12% | SHA-256 over every polygon of every box, to find which boxes the move left alone |
| Operand preparation | 9% | Splits every operand's faces into convex polygons |
| Publishing the body | 5% | Writes the arenas |

No stage dominates: each is paid for the whole wall, and the move changes about 1% of it. So the
gate needs every stage to work from the previous cut, not the arrangement alone.

## Options

1. **Faster full stages.** Make each stage cheaper and keep it global. Measured: the merge's sorts
   gave 19% (a70362bb); the stages left are already exact and linear or n log n. Even all of them
   twice as fast leaves the re-cut near 140 ms. Rejected as the answer; still taken where cheap.
2. **Patch the previous body.** Edit the old body's arenas around the moved door. Fast, but a
   patched body is not built the way a full cut builds it: its row order, names and certificate
   would need their own proof of equality, and any shortcut that drifted would break principle 9
   (nothing changes meaning in place) silently. Rejected.
3. **Keep each stage's state, keyed by its inputs, and redo only what changed (chosen).** Every
   stage of a full cut is a function of its inputs. Each one keeps, beside the arrangement cache,
   the state it built, keyed by what it was built from, and on a re-cut recomputes only the parts
   whose inputs changed, then assembles its output exactly as the full stage does. Equality with a
   full cut is then a property of each stage, testable stage by stage, and the cache stays an
   optimisation that can always be dropped: a missing or foreign cache gives the full cut.

## The design

The re-cut cache grows from solved boxes into one value per cut, `Recut`, returned beside the
result as the arrangement cache is now and accepted in `reuse`. Every part is keyed by a digest of
everything its answer depends on, as the boxes are, so a part whose key does not match is ignored.

1. **Operands.** Each operand's prepared polygons, keyed by its key and a digest of its body. An
   unchanged operand is not prepared again.
2. **The cell.** Each operand's rounded bounds, kept with its preparation. The cell is the least
   and greatest rounded corner over every polygon, which is the same taken operand by operand and
   then over the operands; only a changed operand is rounded again.
3. **Box keys.** Each polygon's digest, kept with its operand; a box's key is the digest of its
   polygons' digests in order (a new key domain, `SKG arrangement box v2`), so an unchanged box's
   key costs one short hash.
4. **The merge.** The kept pieces are identified by digest. The sorted corners and sorted sides are
   kept with the pieces they came from; a re-cut removes the pieces that left and inserts the ones
   that came, by binary search with the same exact comparisons, then renumbers in one linear pass.
   Sweeps, rings, radial orders and nesting are kept per line, per sheet, per edge and per shell,
   keyed by the pieces they read, and are redone only where those changed. Points' names (G138's
   `named`) are kept per point.
5. **Names.** Each output entity's name is kept, keyed by the parts its name is made from; an
   entity whose parts did not change takes its name without hashing again.
6. **Publishing and the certificate.** The arenas are written in full (they are the result, and
   linear), and the checker's verdict is kept per shell, keyed by the shell's faces, edges and
   vertices; only a changed shell is checked again.

Each step charges the meter what the full stage charges, as reused boxes do now, so a re-cut and
a full cut cost the same work units.

## The law

A re-cut from any cache equals a full cut of the same inputs, bit for bit: the bodies, their names
and history, the certificate, the diagnostics, the cache returned, and the work charged. It is
tested on every scenario of G140's harness (the wall's openings, the moved door, the slab's
holes, the union of many boxes, the storey) and on random sequences of moves, additions and
removals of tools, from a fresh cache, a stale one and a foreign one.

## As built

Instructions for three door moves in the 300-opening wall (Callgrind on D03, around the re-cut
alone); the gate is about 94 million a move (N42 runs about 5.9 billion a second on this work).

| Commit | What it keeps or stops redoing | Three moves |
| --- | --- | --- |
| 22120f47 | Solved boxes, keyed by what they depend on | 6.134G |
| a70362bb | The merge's sorts compare lazily, in machine words where they fit | 4.969G |
| 6e07979b | Each unchanged operand's preparation and rounded bounds (items 1, 2) | 4.295G |
| 1ab76935, b465f32e | Nesting's shell boxes from six roundings a shell; a name hashed once | 3.424G |
| 9f401556 | Each merged vertex rounded to the grid once for all lookups | 3.341G |
| c04c5c87 | The merge's memo: each piece's corners and lines, and the sorts' orders (item 4, part) | 2.488G |
| adb2d160 to ef8b6806 | Names' ids kept (item 5, part); corners and lines in machine words, taken once; the names a result reaches found in one sweep, and vertices' grid keys kept; the arrangement's tree of divided boxes kept, so a held box is neither clipped nor hashed again (item 3, by another road) | 1.758G |
| d42c8995 | The merge's union-find keeps its paths short; runs are cut only at ends between their own | 1.569G |
| 3eb52ca0 | Publishing takes each vertex's grid rounding from naming | 1.478G |
| 1970a163 | The names' ids a list by name, merged rather than sorted again | 1.439G |

Tried and dropped: sorting on reduced planes (3.496G against 3.424G).

Item 4 as built: the merge keeps each piece's corners and line keys by the piece's planes, and the
two sorts' orders by piece and place. A re-cut starts each sort from the old order, the new
pieces' entries after it, and the sorts break ties by place, so they end in the one total order a
fresh sort gives. Rust's stable sort finds the old order as one run and merges the new entries
in, which is linear in the comparisons rather than the binary insertion first planned; it needs no
second code path to keep equal. A piece held twice starts the sorts from no order, as its one
place in the old orders would stand for two.

What is left at 480 million a move (1970a163): the merge's passes over the whole wall (40%: nesting's
outer test traces every polygon of the wall's shell, radial orders, the sorts' merging, numbering,
rings), naming per entity (29%: edges, vertices, the names a result reaches), publishing the body
(11%, about 51 million a move), the arrangement along the moved door's path (9%), and allocation.
Publishing alone is over half the gate, which is why the gate needs bodies that share their
unchanged parts, a geom-topo change outside this piece.

Item 3 as built: rather than per-polygon digests, the arrangement keeps the tree it divided. A box
at the same place in the next tree with the same bounds, vector and polygons divides as it did, so
the boxes below it are the last tree's boxes, their keys and answers taken as they were; only the
boxes on the path from the root to the moved door are clipped and hashed again. A tree divided for
other tests or another leaf size is not taken.

## Evidence

- The profile above, and G139's earlier timings: in the 300-opening wall 127 of 128 boxes are
  reused and the arrangement falls from 317 ms to 37 ms, while the selection and rebuild are paid
  in full.
- Chapter 11.6's arrangement cache, "keyed by a content hash of the host" with "only boxes the
  moved tool touches re-solved", generalised here from boxes to every stage.
- Incremental computation with memoised, input-keyed stages: Acar, Blelloch and Harper,
  "Adaptive functional programming" (POPL 2002, <https://doi.org/10.1145/503272.503296>), whose
  correctness criterion, that a change-propagated result equals a from-scratch one, is this
  note's law.

## Open risks

- **Memory.** The cache keeps every stage's state: roughly the body's size again in sorted
  corners, sides and names. It is returned to the caller, who drops it to free it.
- **Stages that are not local.** Nesting and shells can change far from the move (a door that
  splits a wall in two). Those parts are redone in full when their keys change, which keeps the
  law but not the 16 ms; the gate is a door moved within a wall, which keeps its shells.
- **The certificate.** Keeping a verdict per shell is sound only if the checker's verdict is a
  function of the shell alone; the checks that read the whole body (Euler's formula is per shell,
  region nesting is not) are redone in full.
- **wasm32.** 48 ms on wasm32 has not been measured; hashing and the exact comparisons cost more
  there.
