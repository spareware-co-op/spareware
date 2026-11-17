# G156: the standard bar's piece map for Phase 4 (G123 to G155)

*Gate 4 (piece G156), Spare Kit geometry kernel, gap 9 of its record: for every code piece of G123 to G155, the fuzz target, the benchmark, the attack tests and the golden that serve it, and, where a target or benchmark is shared, the coverage that shows it reaches the piece's code. Written 2026-11-17 by Claude Opus 5.5 for the main session, which records the gate. Gate 3's map is 32.56's standard-bar rows, with #821's coverage; this is the same for Phase 4, measured on GitHub.*

## The answer in plain words

The code pieces are the 27 a shipped build links: G123 to G134, G137 to G139 and G142 to G153 (G135, G136, G140, G141, G154 and G155 are in geom-test or geom-bench and build the tools behind the bar, Gate 4's reading R10, as 32.56).

- **22 have every part of the bar**: G123 to G134, G137 to G139, G146 to G149 and G151 to G153. Four of them have files no target or no benchmark reaches, listed under "Reached in part". G130 to G133 and G137 to G139 have their fuzz target in `booleans`, `geom-bool`'s pipeline held to counted cells and both checkers (#949).
- **5 lack more**: G142 (fuzz target and benchmark), G143 and G145 (fuzz target, benchmark and golden), G144 (benchmark and golden; `booleans` is its fuzz target) and G150 (benchmark and golden).

No figure here counts a piece as served unless the measurement shows the lines it runs; a file no unit reaches is named as such.

## The map

One row a piece. "Own" means the target's or benchmark's header or catalogue entry names the piece or measures its function; "shared" means another piece's target or benchmark, with its figure in the next section. Benchmarks are entries of `geom-bench/src/catalogue.rs`, under the 5% count gate, and their digests are in `geom-bench/src/golden.txt`, pinned in the golden store as `geom-bench/output-hashes`. Attack tests are the files whose header names the piece. Goldens are `geom-test/golden/store.json`'s.

| Piece | Its code | Fuzz target | Benchmark | Attack tests | Golden |
| --- | --- | --- | --- | --- | --- |
| G123 canonical planes, arrangement predicates | geom-poly `plane.rs`, `order.rs` | `planes` (own) | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/planes.rs` | `output-hashes` (those benchmarks' digests) |
| G124 predicate step ladder | geom-num `step.rs`, the ladder in `plane.rs` and `predicate.rs`; geom-poly `plane.rs` | shared: `planes`, `make` | shared: `plane/*` | geom-num `tests/steps.rs`; geom-bench `tests/steps.rs` | `output-hashes` (`plane/*`) |
| G125 faces into convex polygons | geom-poly `convex.rs` | shared: `planes` | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/convex.rs` | `output-hashes` |
| G126 clipping and local cuts | geom-poly `cut.rs` | shared: `solids`, `planes` | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/cut.rs` | `output-hashes` |
| G127 winding vectors, tracing, selection | geom-poly `winding.rs`, `trace.rs` | shared: `solids` | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/winding.rs`, `trace.rs` | `output-hashes` |
| G128 adaptive subdivision | geom-poly `subdivide.rs` | shared: `solids` | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/subdivide.rs` | `output-hashes` |
| G129 canonical rebuild | geom-poly `merge.rs`; geom-bool `rebuild.rs` | shared: `solids` (`merge.rs` only); `booleans` (own, `rebuild.rs`) | shared: `scenario/*`, `mesh/boolean` | geom-poly `tests/merge.rs`, `nest_box.rs`; geom-bool `tests/rebuild.rs` | `output-hashes` |
| G130 operands, selections, schema | geom-bool `operand.rs`, `half_space.rs`, `wires.rs` | `booleans` (own) | shared: `scenario/*` | geom-bool `tests/combine.rs` | `output-hashes` (`scenario/*`) |
| G131 n-ary cut and union entry | geom-bool `combine.rs` | `booleans` (own) | own: `scenario/union` (`combine::unite`), `one_selection`, `three_selections`, `storey` (`combine::combine`) | geom-bool `tests/combine.rs` | `output-hashes` |
| G132 boolean names and history | geom-bool `names.rs` | `booleans` (own) | shared: `scenario/*` | geom-bool `tests/names.rs` | `output-hashes` |
| G133 arrangement certificate | geom-bool `certificate.rs` | `booleans` (own) | shared: `scenario/*` | geom-bool `tests/certificate.rs` | `output-hashes` |
| G134 checker groups 2 and 3, plane-based | geom-check `planar.rs` (and its parts of `flat.rs`, `self_intersection.rs`) | shared: `make`, `sequence` | shared: `query/validity`, `make/*` | geom-check `tests/attack_g134.rs`; geom-topo `tests/meet.rs` | `output-hashes` (`query/validity`, `make/*`) |
| G137 cells, rooms, space boundaries | geom-bool `passage.rs` and the cells in `combine.rs` | `booleans` (own) | shared: `scenario/storey` (not `passage.rs`) | geom-bool `tests/cells.rs`, `passage.rs`; geom-check `tests/attack_g137.rs` | `output-hashes` (`scenario/storey`) |
| G138 batch cutting | geom-bool `batch.rs` | `booleans` (own) | own: `scenario/openings`, `moved_door`, `slab_holes` (`batch::cut`) | geom-bool `tests/batch.rs` | `output-hashes` |
| G139 arrangement cache, local re-cut | its parts of geom-bool `batch.rs`, `combine.rs`, `operand.rs`, `names.rs` and geom-poly `merge.rs`, `subdivide.rs` | `booleans` (own) | shared: `scenario/moved_door` | geom-bool `tests/batch.rs`; geom-poly `tests/merge.rs` | `output-hashes` (`scenario/moved_door`) |
| G142 plane commit routine | geom-poly `commit.rs` | **none** | **none** | geom-poly `tests/commit.rs`, `commit_determinism.rs` | `geom-poly/commits` (own) |
| G143 per-entity error bounds | geom-op `bounds.rs` | **none** | **none** | geom-op `tests/bounds.rs` | **none** |
| G144 local-operation driver | geom-bool `local.rs`; geom-op `local.rs` | `booleans` (own: `local_step` on its full and fast paths, with geom-op's `AllowSet`) | **none** | geom-bool `tests/attack_g144.rs`; geom-op `tests/attack_g144.rs` | **none** |
| G145 placed instances | geom-topo `placed.rs`; geom-make `placed.rs` | **none** | **none** | geom-topo `tests/placed.rs`; geom-make `tests/placed.rs` | **none** |
| G146 exact mesh | geom-mesh `exact/` | `exact` (own) | own: `mesh/exact_build` | geom-mesh `tests/exact.rs`, `exact_determinism.rs` | `geom-mesh/exact` (own) |
| G147 point classification | geom-mesh `classify.rs` | `solids` (own, its header) | own: `mesh/locate` | geom-mesh `tests/classify.rs` | `output-hashes` (`mesh/locate`) |
| G148 mesh boolean | geom-mesh `boolean.rs` | `solids` (own) | own: `mesh/boolean` | geom-mesh `tests/boolean.rs`, `boolean_determinism.rs` | `geom-mesh/boolean` (own) |
| G149 slices, hulls, faceted B-rep | geom-mesh `hull.rs`, `slice.rs`, `export.rs`, `faceted.rs`; geom-poly `publish.rs` | `solids` (own) | own: `mesh/hull`, `mesh/slice` | geom-mesh `tests/hull.rs`, `slice.rs`, `export.rs`, `faceted.rs`; geom-poly `tests/publish.rs` | `output-hashes` (`mesh/hull`, `mesh/slice`) |
| G150 Minkowski sums | geom-mesh `minkowski.rs` | `solids` (own) | **none** | geom-mesh `tests/minkowski.rs` | **none** |
| G151 distance and contact predicates | geom-num `distance.rs`; geom-mesh `contact.rs` | `solids` (own; `contact.rs` only) | own: `mesh/point_point`, `point_line`, `point_plane`, `line_line`, `closest_dots`, `triangles` | geom-mesh `tests/contact.rs`; geom-num `tests/distance.rs` | `output-hashes` (those benchmarks) |
| G152 polyhedral envelope | geom-mesh `envelope.rs` | `solids` (own) | own: `mesh/envelope` | geom-mesh `tests/envelope.rs` | `output-hashes` (`mesh/envelope`) |
| G153 constrained Delaunay | geom-mesh `cdt.rs` | `cdt` (own) | own: `mesh/cdt` | geom-mesh `tests/cdt.rs`, `cdt_determinism.rs` | `geom-mesh/cdt` (own) |

## What each shared target and benchmark reaches

Line coverage of the piece's files: lines run, of the lines the file has, by each unit alone in its own process. "Kernel" figures for a benchmark count only its measured pass: the profile counters are zeroed after its inputs are built. Where pieces share a file, the figure is the lines of that file the piece's own commits wrote (by `git blame` on main at 870022ada) that the unit runs. Runs, all on GitHub on the measurement branch `measure/g156-piece-map-coverage`: A = 43894768049 (commit ffce8efc0), B = 43896027651 (6afb3e114), C = 43897357017 (396738395).

| Piece | Shared target or benchmark | What it runs | Run |
| --- | --- | --- | --- |
| G123 | `scenario/*` (each), `mesh/boolean`, kernel | `plane.rs` 21 of 121, `order.rs` 215 of 286 (211 for `storey`) | B, C |
| G124 | `planes` seeded; `make` seeded | the ladder geom-num `plane.rs` holds (`side_of_tpi` and its helpers): 32 lines G124 wrote; 90 | C |
| G124 | `plane/*`, kernel | the same: 28 lines G124 wrote | C |
| G125 | `planes` seeded | `convex.rs` 181 of 190 | A, C |
| G125 | `scenario/*`, `mesh/boolean`, kernel | `convex.rs` 36 of 190 | B, C |
| G126 | `solids` seeded; `planes` seeded | `cut.rs` 193 of 240; 67 | A, C |
| G126 | `scenario/*`, `mesh/boolean`, kernel | `cut.rs` 196 to 200 of 240; 197 | B, C |
| G127 | `solids` seeded | `winding.rs` 98 of 132, `trace.rs` 302 of 444 | A, C |
| G127 | `scenario/*`, `mesh/boolean`, kernel | `winding.rs` 72 to 98 of 132, `trace.rs` 328 to 364 of 444; `mesh/boolean` 73 and 309 | B, C |
| G128 | `solids` seeded | `subdivide.rs` 167 of 878, 28 of them lines G128 wrote | A, C |
| G128 | `scenario/*`, `mesh/boolean`, kernel | `subdivide.rs` 458 to 512 of 878 (74 lines G128 wrote); `mesh/boolean` 165 | B, C |
| G129 | `solids` seeded | `merge.rs` 1,117 of 1,815 (214 lines G129 wrote); `rebuild.rs` none | A, C |
| G129 | `scenario/*`, `mesh/boolean`, kernel | `merge.rs` 1,132 to 1,267 of 1,815 (220 lines G129 wrote); `rebuild.rs` 15 of 63 (scenarios only) | B, C |
| G130 | `scenario/*`, kernel | `operand.rs` 284 to 300 of 432, 321 for the group (196 to 211 lines G130 wrote); `combine.rs` 37 to 74 lines G130 wrote; `half_space.rs`, `wires.rs` none | B, C |
| G131 | `scenario/*`, kernel (its own four and the cuts) | `combine.rs` 180 to 427 of 818 (`union` 217, `storey` 351, the group 427) | B, C |
| G132 | `scenario/*`, kernel | `names.rs` 577 to 667 of 978, 733 for the group (385 to 458 lines G132 wrote) | B, C |
| G133 | `scenario/*`, kernel | `certificate.rs` 56 to 67 of 85 | B, C |
| G134 | `make` seeded; `sequence` seeded | `planar.rs` 729 and 716 of 857 (485 lines G134 wrote, `make`); `flat.rs` 129 and 125 of 171; `self_intersection.rs` 242 of 270 | A, C |
| G134 | `query/validity`, kernel; `make/build_box`, `move_box`, `turn_box`, `cylinder`, kernel | `planar.rs` 81 lines G134 wrote; 81, 81, 81, 44 (the `query` group: 174 of 857; `make`: 202) | C |
| G137 | `scenario/storey`, kernel | `combine.rs` 140 lines G137 wrote (every other scenario 15 or 16); `merge.rs` 24 (as every scenario); `passage.rs` none | C |
| G139 | `scenario/moved_door`, kernel | lines G139 wrote: `subdivide.rs` 181 (the other scenarios 148), `merge.rs` 68 (others 28 to 46), `operand.rs` 74 (62 to 64), `batch.rs` 34 (28), `names.rs` 164, `combine.rs` 33 | C |

The pieces with their own target and benchmark reach them too, as their own units measure: `exact` seeded runs geom-mesh `exact/build.rs` 845 of 955 lines and `mesh/exact_build` 728 (kernel); `cdt` seeded `cdt.rs` 765 of 1,037 and `mesh/cdt` 624; `solids` seeded `boolean.rs` 420 of 507, `classify.rs` 158 of 337, `hull.rs` 410 of 446, `slice.rs` 249 of 279, `export.rs` 250 of 608, `minkowski.rs` 52 of 134, `contact.rs` 136 of 195, `envelope.rs` 170 of 608; `mesh/boolean` `boolean.rs` 435, `mesh/locate` `classify.rs` 141, `mesh/hull` 369, `mesh/slice` 177, `mesh/triangles` `contact.rs` 64, `mesh/envelope` 103 and the distance benchmarks geom-num `distance.rs` 29 to 53 of 91 each, 86 together (runs A and B).

## Reached in part

Files of a piece that has every part, which a unit of the bar does not reach (from run C's whole catalogue, every benchmark built and run once, and every target's seeded test, `_seeded` in every geom-fuzz test binary):

- G124: geom-num `step.rs` (14 lines), the narrow `i64` step of `predicate.rs` (the catalogue's `UNMEASURED` says `orient2d`'s benchmark measures it on the targets that take that path; x86-64 does not) and geom-poly `plane.rs`'s axis step `side_step`: no target and no benchmark run them here. geom-num's and geom-bench's `tests/steps.rs` do.
- G146: `exact/raw.rs` (36 lines): neither; `exact/mass.rs`: no benchmark (the `solids` target runs 41 of its 130 lines).
- G149: `faceted.rs` (68 lines): neither; `export.rs`: no benchmark; geom-poly `publish.rs`: no target (the scenarios run 261 of its 364 lines).
- G151: geom-num `distance.rs`: no target.

## Lacking, and what each needs

Code for a further lane, decided by the main session; nothing here lowers the bar.

- **A fuzz target for geom-bool**: made, `booleans` (`geom-fuzz/src/booleans.rs`), for G130, G131, G132, G133, G137, G138 and G139, with G129's `rebuild.rs` and G144's driver. It holds to counted cells, in the way `solids` holds geom-mesh's operations, the selections of one arrangement (`geom_bool::combine`, with the names, history and certificate), a half-space and a wire kept inside the boxes (`half_space.rs`, `wires.rs`), rooms in an envelope and two seeds' leak (`passage.rs`), batch cuts, many hosts and a re-cut after a move from the last cut's cache (`batch::cut`), and `local_step` on both paths; every result passes the full checker and the second checker, which agree. Its rows are "own" (its header names each piece); the lines it reaches are to be measured on GitHub as the figures above were. The boolean pipeline calls none of G142's, G143's or G145's code, so they still need their own.
- **G142, plane commit**: a fuzz target and a count benchmark for `geom_poly::commit`; it has its golden (`geom-poly/commits`) and its tests.
- **G143, per-entity error bounds**: a fuzz target, a count benchmark and a golden for `geom_op::bounds`.
- **G144, local-operation driver**: a count benchmark and a golden for `geom_bool::local`; `booleans` is its fuzz target.
- **G145, placed instances**: a fuzz target, a count benchmark and a golden for `geom_topo::placed` and `geom_make::placed`.
- **G150, Minkowski sums**: a count benchmark (`mesh/minkowski`) and a golden; `solids` already runs `minkowski.rs`.

## How it was measured

On GitHub, never locally, built as `kernel-quality.yml`'s coverage parts build: the pinned nightly (`nightly-2026-10-01`), cargo-llvm-cov 0.9.1, release with overflow checks and debug assertions, `--branch`, `-Z cross-crate-inline-threshold=never -C link-dead-code`. Each unit ran alone in its own `cargo llvm-cov --no-report` process with the old profiles deleted first, then `cargo llvm-cov report --json --summary-only` gave each file's lines, and `--lcov` each line's count for the files pieces share. Fuzz targets: their `<target>_seeded` test, `Seed::DEFAULT`, as the suite runs it (32.52). Benchmarks: two measurement tests on that branch only, which build each named benchmark with `geom_bench::catalogue::prepare_named` and run one pass of its kernel, `whole` counting the inputs' building too and `kernel` zeroing the counters first with the LLVM profile runtime's `__llvm_profile_reset_counters`. #821 measured Gate 3's on a grown corpus, locally; under 32.52 the seeded tests are the fuzzing, so they are what is measured here. The workflow and the two tests are on the measurement branch, not in this repository's main line; each run's log prints every figure as a `PM` line (and `PML` for hit lines).
