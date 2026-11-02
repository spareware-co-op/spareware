# G139a: shared bodies for a local re-cut

Piece G139a is a part of G139 ("Arrangement cache and incremental re-cut", chapter 11.17 item 14). G139's gate is one door moved in a 300-opening wall, re-cut and certified within 16 ms native, about 94 million instructions on the owner's workstation. After G139's incremental rebuild (`G139-incremental-rebuild.md`) the move costs 432 million. The main session decided on 2026-11-03 that bodies should share their unchanged parts between cuts, in geom-topo, and that G139 stays open on 16 ms with its measured state recorded. This note says what the code does today, what a door move changes, what sharing can buy, and the law that holds it.

## The question

Can a re-cut publish its body by writing only what the move changed, sharing the rest with the body it replaces, and what does that buy against G139's gate?

## What the code does today

- **Columns are already chunked and shareable.** Every table of a body is a `Column` of `Arc`-shared chunks of 4,096 rows (`geom-topo/src/column.rs`, `CHUNK_ROWS`), the fixed runs of 24.2. Chapter 8.2 asks for it ("large bodies are split into shared chunks so a new version copies only what changed"), and 23.9 keeps chunk sharing out of the content hash.
- **No version shares a chunk yet.** A boolean writes every row from the merge (`geom-bool::rebuild::arenas`), then `Builder::publish_named` sorts every table into name order (`Moves::new`, `Moves::arenas`). Each cut builds its body from nothing.

## What a door move changes

Measured on c11d6d73, the 300-opening wall cut fresh, then cut fresh again with door 150 moved five steps along it:

| Table | Rows | Rows that differ |
| --- | --- | --- |
| Faces | 1,206 | 4 |
| Loops | 1,806 | 0 |
| Fins | 7,224 | 0 |
| Edges | 3,612 | 0 |
| Vertices | 2,408 | 0 |
| Points | 2,408 | 8 |
| Curves | 3,612 | 12 |
| Surfaces | 608 | 2 |

Every name and every count stays: the moved door's faces, edges and vertices keep their names, as names follow topology and not position (chapter 20). So every row stays where it was, and 26 rows of 23,094 change, all geometry: the moved door's two jamb planes, its eight corners and the twelve lines through them. Each table fits in one or two chunks.

The body as a value costs, a move (Callgrind on D03, three moves averaged): writing the arenas 27 million, sorting them into name order 25 million, the checker 9 million, the certificate 3 million and dropping the last result 13 million. That is 76 million, 18% of the 432 million.

## Options

1. **Bodies patched by row, sharing every chunk they do not touch (chosen).** A re-cut that keeps every name and count in the same order takes the last body and writes only the rows whose entities changed, into copies of their chunks. It shares every other chunk, keeps the last cut's name order instead of sorting again, and checks only what the changed rows reach. In this move it writes 4 chunks of 13 and sorts nothing.
2. **Content-defined chunk boundaries.** A chunk ends after a row whose name hashes a set way, so an edit that adds or removes entities shifts only its own chunk. A move adds and removes nothing, so it does not need this. It would change 24.2's fixed runs, which are proposed and versioned. It is kept as an open risk for edits that add or remove entities.
3. **Leave publishing whole.** 76 million a move stays.

## The law

A patched body equals the body a full publish gives, bit for bit: the arenas, their name order, the names, the chunk contents and so the content hash (23.8). It is G139's law carried one stage further, tested the same way, on every scenario and on random sequences of moves, additions and removals. A re-cut that cannot patch publishes in full, so the law holds whatever the move.

## How it runs

- **geom-topo.** A column takes a list of rows to replace and returns a new column sharing every chunk the list does not touch. A body takes the rows to replace in each table and returns the new body, running the builder's checks on the rows it replaced and on what links to them.
- **geom-bool.** A re-cut keeps, in its cache, the last body and the permutation that put the merge's rows into name order. When the merge kept every entity in the same order, and naming gave the same names, the permutation is the same. The rows that changed are the rows of entities made from pieces that changed, which the merge's memo already tells apart. Those rows are written; every other row is the last body's.
- **The checker.** The changed rows reach a known set of faces, edges and vertices. The cheap combinatorial check runs on those, and the full check of a shell on the shells they lie in (G139's item 6).

## What it buys

About 60 of the 76 million a move, by estimate from the profile above (unmeasured until built), taking the move from 432 million to about 370 million. It does not reach 16 ms alone. The rest is the merge's and naming's passes over the whole wall: the merge 160 million a move and naming 120 million, each linear in the wall. G139 keeps those open. Its as-built table records the state, and the gate stays 16 ms.

## Open risks

- **Moves that change names or counts.** A door pushed into another door, or one that splits a face in two, changes names or counts. It publishes in full, at today's cost.
- **Edits that add or remove entities.** Rows are in name order, and names are hashes, so an added or removed entity shifts every later row. With fixed 4,096-row runs, every chunk after the first change is written again. Content-defined boundaries (option 2) fix this, and they are a format decision for 24.2.
- **The checker's reach.** Checking only what changed rows reach is sound only if every check is local to those rows. The checks that read a whole shell (Euler's formula, region nesting) run on the whole shell when any of its rows changed.
