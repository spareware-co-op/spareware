# 16 Where each unbuilt piece starts: the solidcore unit or SMLib file

*Evidence for [G01](../G01-geometry-kernel.md). Written 2026-11-10 by a Claude Fable 5.2 research agent that read the outside repositories through GitHub's API and their public pages. Nothing was cloned, built, run or tested. It follows [13](13-vetting-smlib.md) and [15](15-take-map-solidcore-craft.md) and does not repeat them. Licences are settled by the owner ([32.44](../../../engines/geometry/architecture/32-decisions.md)) and are not discussed here.*

## 1. What this note is for

The kernel's rule is existing code first: every new piece starts from solidcore's Rust, then from SMLib's C++ converted to Rust, and says so in a `Started from:` line in its pull request ([AGENTS.md](../../../engines/geometry/AGENTS.md), section 1). Until now nothing told a lane which unit to open. This note does, for the 211 code pieces of the [board](../../../engines/geometry/architecture/30-build-order.json) that had no branch, no pull request and no merge on 2026-11-10. It is for those pieces only. A built piece that passes its gates is never reworked to change where its method came from.

Report 15 says, for about thirty of these pieces, what to take and what to leave. This note is wider and shallower: one row for every unbuilt piece, naming the file to open first.

### Pins

| Project | Commit read | Note |
| --- | --- | --- |
| solidcore | [90f7f963](https://github.com/tangency-labs/solidcore/tree/90f7f9637d48627e6af82017211dce9060dccb6e), 2026-11-10 04:44 UTC | Apache-2.0 by its own `LICENSE` (NVIDIA's text, copied from upstream) and `NOTICE`. The repository is a read-only mirror of the `solidcore/` folder of tangency-labs/solidcraft. It moves fast: read it at its latest commit and record that one |
| SMLib (USD BRep) | [3fd5b954](https://github.com/NVIDIA-Omniverse/usd-brep/tree/3fd5b9548d19aa8c12c29bc9637a96d42a06f8a5), v1.0.1, 2026-10-22 | Apache-2.0. The commit solidcore pins |
| solidcore-viewer | [5fc270b6](https://github.com/tangency-labs/solidcore-viewer/tree/5fc270b628340199d20f132c9dcb1ae128a910e0), 2026-10-23 | Apache-2.0 |

### What changed in solidcore since report 15

- **Its status site no longer measures against the C++.** The [overview](https://tangency-labs.github.io/solidcore/) says green and partial are "historical port-plan statuses", not test results: 254 units green, 27 partial, 1 blocked (`blend.feature_executive`), 99.3% of upstream lines ported.
- **`crates/difftest` is gone.** Commit [96b1a986](https://github.com/tangency-labs/solidcore/commit/96b1a98697) (2026-11-06) replaced it with `crates/goldens`: each unit's cases are compared with outputs solidcore itself recorded. Its README says a golden "proves a result unchanged, not correct", and that the C++ is never built or run. Section 5 says what that means for us.
- **Mass properties now have a certified enclosure** (`mass/enclosure.rs`), which report 15 could not find.
- **Much of what helps us is not a port.** Many files say "not upstream" in their header: they come from the SolidCraft fork of SMLib or were written for solidcore. They have no unit in `plan/units/`, so the status site says nothing about them.

## 2. How to read the table

- **Start from.** A path that starts with a module name (`sweep/`, `blend/`, `nurbs/` and so on) is under `crates/solidcore/src/` in solidcore. Other solidcore paths start with `crates/`, `plan/`, `docs/` or `scripts/`. A path that starts with `source/` is in usd-brep. `none found` says where I looked.
- **Their status.** The unit's status in solidcore's `plan/units/<unit>.toml` at the pinned commit: green, partial or blocked. "Native, no unit" means the file is not a port, so it has no status. Read the first bullet above before trusting "green".
- **What our rules need.** solidcore decides by tolerance in `f64`, uses the platform's `sin` and `cos`, and is `std`. So nothing is used unchanged. The first word says how far it can go:
  - **Inside.** The method goes inside our exact or certified design. Port the construction and its case structure; every decision becomes an exact predicate or a certified bound on our interval type, maths comes from `geom_num::math`, and the code is `no_std`, panic-free and bounded.
  - **As is.** No geometry is decided, so the port is close to a translation, with our error types and `no_std`.
  - **Oracle.** Our method is different by design. Use theirs as test inputs or as an out-of-process comparison only.
- **How sure.** `read the code` means I read the file's header and its table of items at the pinned commit, not the whole file. `matched by name only` means I matched a file or unit name and read nothing. About two in five of the rows that name a file are by name only, so check before you rely on one.

Every port keeps the origin's notice as rule 11 of AGENTS.md says, and the pull request's `Started from:` line names the path and the commit you read, which will be later than the one here.

### Counts

| | Pieces |
| --- | --- |
| Start from a solidcore file | 82 (58 to port inside our design, 1 close to as is, 23 as an oracle or test source) |
| Start from an SMLib file solidcore has not ported | 2 (G205, G351) |
| None found | 124 |
| Not a code piece (G480 to G482) | 3 |
| **Total** | **211** |

Of the rows that name a file, 60 are `read the code` and 41 are `matched by name only`.

Phases 9 and 10 are almost all `none found`: neither project has subdivision surfaces, fields, terrain, alignments, point clouds, IFC, DXF or mesh formats. The help is in phases 7, 11 and 12.

### The five places it saves the most

1. **Blends, G263 to G269.** `blend/` is the largest module (142 files), with the exact classes, a certified section, overflows, law corners, setbacks and corners already separated.
2. **Sweeps and lofts, G255 to G260.** Extrude, analytic revolve, stations with frame laws, a procedural sweep with a certified bound, the screw sweep, skinning, and NLib's Coons, Gordon and N-sided fills.
3. **STEP, G415, G418, G427 to G429.** A hardened Part 21 reader, Part 42 geometry, product structure, topology-first body building and a writer.
4. **Offsets, shells, draft and face edits, G270, G277 to G279.** Certified offset sheets, the reach margin, draft in kind, the local operation and fillet removal.
5. **Queries with proofs, G246, G247, G293, G296, G346.** A certified volume enclosure, a certified distance bound, a proven contact bracket and a continuity measure, none of them from SMLib.

## 3. The map

### Phase 6: 2D drafting and sketches

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G197** Arrangement with certified curves (geom-2d) | none found (looked in `prism/`, `boolean/`, `curve/`); nearest is the loop split in `prism/profile.rs` | native, no unit | Oracle. No arrangement exists there. Use its profile cases as inputs only | read the code |
| **G201** Drafting entities to bodies and back (geom-2d) | `prism/sheet.rs`, `prism/profile.rs`, `primitive/api_trim_ops.rs` | native, no unit; `primitive.api_trim_ops` partial | Inside. Regions read back by exact tests, names carried both ways, typed errors | read the code |
| **G202** 2D booleans (geom-2d) | `boolean/api_brep.rs` (the 2D boolean), `crates/solidcore-ops/src/prism.rs` | `boolean.api_brep` partial | Oracle. Ours runs on the exact arrangement with fill rules; theirs is tolerance-based, so compare results only | matched by name only |
| **G203** 2D offsets (geom-2d) | `primitive/primitive_creation/p1/offset_profile.rs`, `curve/offset_curve.rs`, `nurbs/crv_offset.rs` | `primitive.primitive_creation_p1`, `curve.offset_curve`, `nlib.crv_offset` green | Inside. Only the certified procedural offset can start here; joins, end styles and winding cleanup are exact and ours (report 15, section 3) | read the code |
| **G204** Trim, extend, break, join and cleanup (geom-2d) | `primitive/api_curve_ops.rs`, `primitive/api_trim_ops.rs`, `nurbs/crv_tool/` | green; `primitive.api_trim_ops` partial | Inside. Split and join points decided exactly; names; OutOfRange at the grid limit | matched by name only |
| **G205** Fillet, chamfer and blend curves (geom-2d) | `source/SMLib/src/SmCurveFilletExecutive.cpp` (in no solidcore unit); then `primitive/api_curve_ops.rs` | not ported | Inside. Tangent circles built exactly on exact fractions; its trim and join order and side picking carry over | read the code |
| **G207** 2D distance and clearance (geom-2d) | `query/distance_bound.rs`, `curve/curve/` (closest point) | native, no unit; `curve.curve_p*` green | Inside. Exact pairs are ours; its hull lower bound is the pattern for the certified pairs, on our interval type | read the code |
| **G208** Blocks, inserts, clipping and viewports (geom-2d) | none found (looked in `prism/`, `curve/`, `primitive/` and the ops crate) | n/a | Nothing to port | n/a |
| **G209** Dimensions, leaders and text anchors (geom-2d) | none found (looked in `prism/`, `curve/`, `primitive/` and the ops crate) | n/a | Nothing to port | n/a |
| **G210** Hatches and linetypes (geom-2d) | none found (looked in `prism/`, `curve/`, `primitive/` and the ops crate) | n/a | Nothing to port | n/a |
| **G211** Our own pattern and linetype library (geom-2d (data)) | none found (authored data, by the piece's own rule) | n/a | Nothing to port | n/a |
| **G212** Wide polylines and snap rounding (geom-2d) | none found (looked in `prism/`, `curve/`, `primitive/` and the ops crate) | n/a | Nothing to port | n/a |
| **G214** 2D corpus, oracles and fuzzing (geom-test, geom-fuzz) | `crates/goldens/src/units/` (the `boolean.api_brep` 2D cases), `crates/goldens/tests/fuzz_corpus/` | test data | Oracle. Inputs rebuilt in our API; expected values from exact truth | matched by name only |
| **G229** Profiles from sketches (geom-profile) (geom-profile) | `prism/profile.rs`, `sweep/extrude.rs` (its profile checks) | native, no unit | Inside. Closure, self-crossing and even-odd decided exactly; regions come from our arrangement; edges keep sketch names | read the code |
| **G230** Unsolved sketches and project datum systems (geom-sketch, geom-profile) | none found (no sketch solver in either tree) | n/a | Nothing to port | n/a |
| **G232** Law catalogue: 2D and sketches (geom-test) | none found; the invariants of `crates/solidcore/tests/kernel_cases/` are candidates for laws | test data | Oracle. Each law as an exact or certified number | matched by name only |

### Phase 7: curved modelling

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G245** Face splitting: NURBS and procedural carriers (geom-bool) | `boolean/topology_intersector/` (p1 to p3, `coincident_strip.rs`), `crates/solidcore-ops/src/split.rs` | `boolean.topology_intersector_p1` to `p3` green | Inside. Splits follow certified intersection curves; coincidence proven, never judged by tolerance; NearCoincident with evidence | matched by name only |
| **G246** Certified ray classification (geom-bool) | `intersect/ray_tracer.rs`, `query/brep/ray.rs`, `query/brep/ray/live.rs`, `intersect/point_class.rs` | `intersect.ray_tracer`, `intersect.point_class` green | Inside. Its grid finds candidates; each hit certified by intervals; fixed ray sequence; NearContact where none qualifies | read the code |
| **G247** Queries: classify a point and certified mass properties (geom-bool, geom-topo) | `mass/enclosure.rs`, then `mass/integrator/properties.rs` | native, no unit; `mass.integrator` partial | Inside. The enclosure method on our interval type and our maths; exact rationals for flat bodies; drop the adaptive integrator | read the code |
| **G248** Curved path integration and Tolerant operands (geom-bool) | `boolean/merge/` (p1 to p5), `boolean/api_brep.rs` | `boolean.merge_p1` to `p5` green; `boolean.api_brep` partial | Oracle. Our curved builder feeds the shared exact back end; use its case structure and compare results | matched by name only |
| **G249** close_near_contacts repair operation (geom-bool) | `heal/gap.rs`, `heal/stitch/bounds.rs`, `topology/brep/validation/coincidence.rs` | `heal.gap` green | Oracle. Ours adopts one carrier within an explicit cap and reports each move; theirs grows tolerances | read the code |
| **G250** Research spike: certified winding number for trimmed NURBS (geom-bool) | none found (looked in `intersect/point_class.rs`, `mass/`) | n/a | A research spike from the paper | n/a |
| **G252** Curved boolean corpus and gates (geom-test, geom-bench) | `crates/solidcore/tests/kernel_cases/booleans/`, the goldens of `boolean.*` | test data | Oracle. Bodies rebuilt in our files; statuses from our own runs | matched by name only |
| **G255** Extrude and revolve (geom-sweep) | `sweep/extrude.rs`, `sweep/analytic_revolution.rs`, `primitive/primitive_creation/p2/taper.rs`, `sweep/trans_sweep_geometry.rs`, `sweep/rotational_sweep_geometry.rs` | first two native; `sweep.trans_sweep_geometry`, `sweep.rotational_sweep_geometry` green | Inside. Kind chosen by exact tests, not by tolerance; flatness and closure exact; faces named side and cap; poles and seams by rule | read the code |
| **G256** General sweeps and swept disks (geom-sweep) | `sweep/stations.rs`, `surface/procedural_sweep/path.rs`, `sweep/topology_sweep/`, `sweep/directed.rs` | first two native; `sweep.topology_sweep` green | Inside. Station frames stored bit for bit (32.28); cache bound on our intervals; line or arc decided by kind, not by 12 samples; mitre and self-overlap tests are ours | read the code |
| **G257** Rail and helical sweeps (geom-sweep) | `sweep/screw.rs`, `sweep/stations.rs` (its two guide rails) | native, no unit | Inside. Helix edges Exact, only the swept face carries a cache; rail fit certified; RailMismatch typed | read the code |
| **G258** geom-thread toolkit (geom-thread) | `sweep/screw.rs`; cases in `crates/solidcore/tests/kernel_cases/threads/` | native, no unit | Inside. Built only from G257; no thread form shipped | read the code |
| **G259** Lofts and swept blends (geom-sweep, geom-nurbs) | `primitive/primitive_creation/p2/skin.rs`, `nurbs/srf_adv/` (skinning), `sweep/section_blend.rs`; SMLib `source/SMLib/src/SmBlendSurface.cpp` | `primitive.primitive_creation_p2`, `nlib.srf_adv_p1` to `p6` green | Inside. Linear solves on our solvers; knot merging certified; ruled lofts stay Exact when planar | read the code |
| **G260** Network surfaces, Coons and N-sided fills (geom-sweep, geom-surface) | `nurbs/srf_adv/` (the ports of `N_CreateGordonSrf`, `N_CreateCoonsSrf`, `N_FillNSidedHole`) | `nlib.srf_adv_p1` to `p6` green | Inside. G1 angle certified; curve network crossings found by certified intersection | matched by name only |
| **G261** Developable lofts (geom-sweep) | `primitive/primitive_creation/p3/ruled.rs` | `primitive.primitive_creation_p3` green | Inside. Only the ruled surface starts here; Aumann patches and the developability certificate are from the papers | read the code |
| **G263** Constant fillets: analytic blends and chamfers (geom-blend) | `blend/fillet_exact.rs` and `blend/fillet_exact/`, `surface/procedural_blend/chamfer.rs`, `blend/chamfer_side.rs` | native, no unit (behind an opt-in fix flag) | Inside. Class chosen by exact tests on kinds and placements; closed-form volumes; no fix flags | read the code |
| **G264** Constant fillets: rolling-ball blends (geom-blend, geom-surface) | `blend/fillet_section/` (with `certificate.rs`), `blend/fillet_solver/`, `surface/procedural_blend/` | solver units green; section native | Inside. Rails traced by our certified tracer; Krawczyk and Hermite bounds on our intervals; tangency within the ladder's angle | read the code |
| **G265** Constant fillets: overflows and clusters (geom-blend) | `blend/fillet_overflow.rs`, `blend/fillet_cliff_overflow.rs`, `blend/fillet_tangent_rollover.rs` | native, no unit | Inside. Overflow order as a stated policy; cluster edits as transactions; creases from certified intersection | read the code |
| **G266** Variable-radius blends and trimlines (geom-blend) | `blend/law_corners.rs`, `blend/fillet_executive/law_radii.rs` | native, no unit | Inside. Radius laws in our law-curve kind; deviation certified against the definition | read the code |
| **G267** Class-A Gn sections and bridge blends (geom-blend) | `blend/fillet_section.rs` (the Gn section), `sweep/section_blend.rs`, `query/continuity.rs` | native, no unit | Inside. Hartmann sections with fullness are from the paper; continuity recorded as certified numbers | read the code |
| **G268** Vertex blends: setbacks and sphere corners (geom-blend) | `blend/blend_setback.rs` and `blend/blend_setback/` (with `certify.rs`) | native, no unit | Inside. Sphere corners exact; G1 certified on our Bernstein bounds | read the code |
| **G269** Vertex blends: remaining corner classes (geom-blend) | `blend/blend_corner.rs` and `blend/blend_corner/`; the old classes in `blend/fillet_corner/` | native; `blend.fillet_corner_p2`, `p3` partial | Inside. Every class covered or CornerUnsupported; G2 only where proven; do not port the old corner code they deleted | read the code |
| **G270** Surface offsets with certified caches (geom-offset) | `offset/construction/sheets.rs`, `offset/construction/bernstein.rs`, `surface/offset_surface.rs`, `nurbs/srf_offset.rs` | first two native; `surface.offset_surface`, `nlib.srf_offset` green | Inside. Offsets in kind by exact tests; cache bound on our intervals; cusps from known curvature; waits for G235 | read the code |
| **G271** Exact weighted straight skeleton (geom-skeleton) | none found (no skeleton file or unit in either tree) | n/a | From the papers named in the piece | n/a |
| **G272** Roof generation (geom-skeleton) | none found (no skeleton or roof file) | n/a | Nothing to port | n/a |
| **G274** 3D skeleton: vertex splits and event simulation (geom-skeleton) | none found (no skeleton file) | n/a | Nothing to port | n/a |
| **G275** Body offset, shell and thicken: flat bodies (geom-offset) | `offset/offset_executive/` (with `reach.rs`), `offset/offset_faces.rs`, `offset/wall_room.rs` | `offset.offset_executive_p1`, `p2` partial | Oracle. Our join is the 3D skeleton (G274), exact on polyhedra; take the reach margin derivation and the cases; waits for G235 | read the code |
| **G276** Roof build-ups by thickening (geom-offset) | the thicken path of `offset/offset_executive/` | partial | Oracle. Built from G275 on roof sheets; cases only | matched by name only |
| **G277** Body offset, shell and thicken: curved bodies (geom-offset) | `offset/offset_executive/` (p1, p2, `crossing_loops.rs`, `step_gaps.rs`), `offset/offset_geometry_creation.rs`, `offset/construction/sheets.rs` | all three offset units partial | Inside. Joins by certified re-intersection; no tolerance-driven vertex and edge repair | read the code |
| **G278** Draft (geom-offset) | `offset/draft.rs`, `primitive/primitive_creation/p2/taper.rs`, `p2/taper_walls.rs` | native; `primitive.primitive_creation_p2` green | Inside. Planes and cones drafted in kind exactly; draft angle certified on every face | read the code |
| **G279** Face edits and defeaturing (geom-offset) | `topology/brep/local_operation.rs`, `offset/offset_faces.rs`, `blend/fillet_removal.rs`, `blend/identify_fillet.rs` | `topology.brep_local_operation` partial; `blend.fillet_removal`, `blend.identify_fillet` green; `blend.feature_executive` blocked | Inside. Faces regrown by certified intersection; fillets recognised by role and history first, by shape second; all-or-nothing edits | read the code |
| **G280** Swept volumes: exact routes (geom-sweep) | `sweep/screw.rs` for helical moves only; nothing for Minkowski sums | native, no unit | Inside. Flat-body sums are exact and ours; the screw wall carries a certified cache | read the code |
| **G281** geom-stair toolkit (geom-stair) | none found (a composition of our own operations) | n/a | Nothing to port | n/a |
| **G282** Chapter 13 corpus and Class-A corpus (geom-test) | the models behind solidcore-viewer (`TestFiles/pt_TestFiles` of usd-brep: Fillet 96, Shell 25, Defeature 12), read by `topology/part_file.rs` and `topology/database_io.rs` | `topology.database_io` green | Oracle. Converted offline with the notice; never copied in as they are (see section 5) | matched by name only |
| **G283** Law catalogue: sweeps, blends, offsets, manufacturing and curved mass (geom-test) | `crates/solidcore/tests/kernel_cases/ladder/` and the invariants of the kernel cases | test data | Oracle. Each law exact or certified; their 1e-3 pass rule is not ours | matched by name only |

### Phase 8: tessellation, spatial index and clash

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G288** Edge-first certified tessellation (geom-mesh) | `crates/solidcore-tess/src/mesher/` (`edges.rs`, `mod.rs`, `sag.rs`); SMLib's own in `query/tess.rs` | native crate; `query.tess` green | Inside. Edges first is the same idea; the sag check samples points, so replace it with a proven bound; closure certificate; no_std | read the code |
| **G289** Thin-part check and incremental tessellation (geom-mesh) | none found (looked in `crates/solidcore-tess/src/`: `plan.rs`, `mesher/tangle.rs`) | n/a | Thin-part and incremental rules are ours | matched by name only |
| **G290** Display boundary and level-of-detail ladder (geom-mesh) | `crates/solidcore-tess/src/simplify.rs`, `preview.rs` | native crate | Inside. Display-only by type; levels tied to the fit bound; no cracks proven | read the code |
| **G291** Display simplifier and meshlets (geom-mesh) | none needed: the piece names meshoptimizer; `crates/solidcore-tess/src/simplify.rs` as a comparison | native crate | Oracle. Port meshoptimizer as the piece says | read the code |
| **G292** Analytic closed-form distances (geom-intersect) | `query/api_queries.rs`, `curve/curve/` and `surface/surface/` (closest point), `geom/geom_utility/` | green | Inside. Closed forms kept; every Touch or Apart answer exact or certified, never a Newton result | matched by name only |
| **G293** Certified distance for curved bodies (geom-spatial) | `query/distance_bound.rs`, `query/contact.rs` | native, no unit | Inside. Hull bound and bracket on our interval type; refinement budgeted; NearLimit typed | read the code |
| **G296** Clash pipeline (geom-spatial) | `query/contact.rs`, `crates/solidcore-ops/src/contact.rs` | native, no unit | Inside. Its proven bracket is the narrow phase pattern; broad phase, six-valued contact and names are ours | read the code |
| **G297** Time of first contact (geom-spatial) | none found (the piece names Tight Inclusion) | n/a | Port that, as the piece says | n/a |
| **G300** Envelope-bounded decimation and remeshing (geom-mesh) | `query/brep.rs` (its unit lists `source/SMLib/src/SmPolyDecimate.cpp`); `crates/solidcore-tess/src/simplify.rs` | `query.brep` green | Inside. Collapses accepted only inside the two-way envelope, with rollback | matched by name only |
| **G302** Volume mesher: insertion (geom-mesh) | none found (no tetrahedral mesher in either tree) | n/a | Nothing to port | n/a |
| **G303** Volume mesher: improvement, tags and regions (geom-mesh) | none found (no tetrahedral mesher) | n/a | Nothing to port | n/a |
| **G304** Mesh offsets, hollow and thicken (geom-mesh) | none found (the piece names the Topological Offsets code) | n/a | Port that, as the piece says | n/a |
| **G305** Reconstruct by envelope remesh (geom-mesh) | none found | n/a | Nothing to port | n/a |
| **G306** Prism and hexahedral meshing of 2.5D bodies (geom-mesh) | none found | n/a | Nothing to port | n/a |
| **G307** Law catalogue for meshes, spatial and contact (geom-test) | `crates/solidcore/tests/kernel_cases/tessellation/`, `crates/solidcore-tess/tests/data/` | test data | Oracle. Laws stated exactly; cases as inputs | matched by name only |

### Phase 9: free-form

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G314** OpenSubdiv oracle driver (geom-test) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G315** SubD net, options and uniform refinement (geom-surface, geom-subd) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G316** Creases, darts and local refinement (geom-surface, geom-subd) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G317** Exact SubD oracle and eigen-tables (geom-gen, geom-surface, geom-test) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G318** SubD limit surface kind (geom-surface, geom-format) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G319** Face-varying evaluation and SubD tessellation (geom-subd, geom-mesh) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G320** Intersections at extraordinary vertices (geom-intersect) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G321** SubD face intersections with every kind (geom-intersect) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G322** SubD to B-rep with SubD faces (geom-convert, geom-check) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G323** SubD to NURBS: packed patches and PCCM (geom-subd, geom-convert) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G324** Face splitting on subdivision-limit carriers (geom-bool) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G325** SubD net editing (geom-subd) | none found (neither file list has a subdivision file) | n/a | Nothing to port; OpenSubdiv is the named reference | n/a |
| **G326** Field graph, bytecode and node names (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G327** Field interpreter (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G328** Field body contract (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G329** Field opcodes: primitives, blends and shells (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G330** Field opcodes: lattices (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G331** Field mesher: octree and dual contouring (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G332** Field mesher: certificate, names, 2D and slices (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G333** Bodies as fields (geom-convert) | `intersect/point_class.rs` as a second opinion on inside or outside | `intersect.point_class` green | Oracle. Winding numbers are from the named sources; compare classifications only | matched by name only |
| **G334** Field primitive recognition to B-rep booleans (geom-convert) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G335** Conversion graph and route query (geom-convert) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G336** One boolean across representations (geom-combine) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G337** Representation dispatch for modelling operations (geom-combine) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G338** Swept volumes as fields: space-time opcode and grid (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G339** Swept volumes as fields: contouring (geom-field) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G340** Swept volumes as fields: extraction and the general route (geom-convert) | none found (neither file list has a field or lattice file; `surface/implicit.rs` is implicit forms of surfaces) | n/a | Nothing to port | n/a |
| **G341** Discrete parameterisation and flattening (geom-mesh) | none found (no mesh parameterisation file) | n/a | Nothing to port | n/a |
| **G342** Surface division and pattern mapping (geom-panel) | none found (looked at `curve/iso_curve.rs`: iso curves only) | n/a | Nothing to port | matched by name only |
| **G343** Unrolling with strain maps (geom-panel) | `surface/unbend_volume.rs`, `surface/bend_volume.rs`; cases in `crates/solidcore/tests/kernel_cases/sheet-metal/` | `surface.unbend_volume`, `surface.bend_volume` green | Inside. Only the cylinder-to-plane map starts here; strain limits and seams are ours; flat maps of developable kinds exact | read the code |
| **G344** Panelisation: planar quads and exact panels (geom-panel) | none found | n/a | Nothing to port | n/a |
| **G345** Panelisation: strips and mould reuse (geom-panel) | none found | n/a | Nothing to port | n/a |
| **G346** Class-A analysis: continuity across edges (geom-analysis) | `query/continuity.rs`, `crates/solidcore-ops/src/continuity.rs`; cases in `kernel_cases/continuity/` | native, no unit | Inside. It measures at one parameter; ours needs a certified maximum per edge with a witness | read the code |
| **G347** Class-A analysis: curvature extremes (geom-analysis) | `eval/normal_section.rs`, `query/normal.rs` | native, no unit | Inside. Curvature formulas only; extremes by Bernstein root isolation with proofs | matched by name only |
| **G348** Class-A analysis: light lines (geom-analysis) | `intersect/surface_silhouette.rs`, `intersect/surface_tracer.rs` | `intersect.surface_silhouette`, `intersect.surface_tracer` green | Inside. A silhouette is one isophote; trace with our certified tracer; kink angles per edge | read the code |
| **G349** Class-A analysis: draft, gap, flush and deviation (geom-analysis) | `offset/draft.rs`, `query/continuity.rs` (gap), `heal/gap.rs`, `query/distance_bound.rs` | native; `heal.gap` green | Inside. Minima and maxima certified, not sampled | read the code |
| **G350** Reverse engineering: primitives and quad layout (geom-cloud, geom-mesh, geom-convert) | none found for seeded RANSAC or quad layout; `eval/form.rs` recognises analytic forms of one NURBS face | native, no unit | Oracle. Recognition cases as inputs | matched by name only |
| **G351** Reverse engineering: surface fitting and Poisson (geom-convert) | `source/SMLib/src/NL_SrfLstSqFit.cpp` (in no solidcore unit), with `nurbs/srf_fit.rs` and `nurbs/crv_fit/` | `nlib.srf_fit`, `nlib.crv_fit_p1` to `p4` green | Inside. Random sampling through our seeded generator (SMLib's hidden one differs by platform); solves on our solvers; result Approximate with measured deviation | read the code |
| **G352** Signed heat reconstruction (geom-field) | none found | n/a | Nothing to port | n/a |
| **G353** Law catalogue for free-form and conversions (geom-test) | none found | n/a | Nothing to port | n/a |
| **G354** SubD exchange: USD and OBJ (geom-meshio) | none found (usd-brep's USD layers carry B-reps, not subdivision nets) | n/a | Nothing to port | matched by name only |

### Phase 10: survey, terrain and civil

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G360** COGO intersections, arcs and parcels (geom-2d) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G362** Exact Delaunay core (geom-terrain) | `crates/solidcore-tess/src/mesher/cdt.rs` as a comparison only | native crate | Oracle. Ours is integer Delaunay with exact predicates; theirs is floating point in a face's parameters | matched by name only |
| **G363** Breaklines, crossings and given triangulations (geom-terrain) | `crates/solidcore-tess/src/mesher/cdt.rs` as a comparison only | native crate | Oracle. Constraint insertion with exact crossings is ours | matched by name only |
| **G364** Parallel, chunked and incremental TIN build (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G365** TIN surface operations (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G366** Contours, cut and fill, volumes (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G367** Toposolid (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G368** Grading: straight pieces, rounded corners and the envelope (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G369** Grading: cut and fill switching, targets and benches (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G370** Grading: arcs, helices and skeleton mitres (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G372** Spiral intersections with every kind (geom-intersect) | `nurbs/spiral.rs`, `intersect/curve_class/` | `nlib.spiral`, `intersect.curve_class_p1` to `p9` green | Inside. Our spiral kinds have their own certified evaluators; only the case structure of the curve intersector carries over | matched by name only |
| **G373** Alignment model, stations and projection (geom-align) | none found; nearest `nurbs/spiral.rs` | `nlib.spiral` green | Nothing to port | matched by name only |
| **G374** Terrain and station reference forms (geom-name, geom-terrain, geom-align) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G375** Corridor sectioned solid and faceting (geom-sweep) | `sweep/stations.rs`, `sweep/section_blend.rs`, `primitive/primitive_creation/p2/skin.rs` | native; `primitive.primitive_creation_p2` green | Inside. Sections placed by the caller's stations; faceted solid exact, smooth one Certified | read the code |
| **G376** Corridor ranges, names and junctions (geom-align) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G377** Flow, sight and raster DEM input (geom-terrain) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G378** geom-civil compositions (geom-civil) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G379** Point-cloud reference queries and snaps (geom-cloud) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |
| **G380** Point-cloud fitting and TIN from a cloud (geom-cloud, geom-terrain) | `nurbs/crv_fit/`, `nurbs/srf_fit.rs`, `geom/point_set.rs`; SMLib `source/SMLib/src/NL_SrfLstSqFit.cpp` | green; the SMLib file is not ported | Inside. Seeded sampling through our generator; deviation certified; plane and primitive fits from the sources the piece names | read the code |
| **G381** Law catalogue and oracles for ground (geom-test) | none found (neither file list has a terrain, alignment or point-cloud file) | n/a | Nothing to port | n/a |

### Phase 11: drawings

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G386** Drawing corpus and benchmark scenes (geom-test, geom-bench) | none found | n/a | Nothing to port | n/a |
| **G387** View and scene input (geom-draw) | `blend/hcr.rs` (its view transform), `intersect/surface_silhouette.rs` (parallel and eye-point views) | `blend.hcr`, `intersect.surface_silhouette` green | Oracle. Our view is rational data with typed InvalidView; use theirs to cross-check | read the code |
| **G388** Exact view predicates (geom-num) | none found (no exact predicate in either tree) | n/a | Generated predicates, as section 6 of AGENTS.md says | n/a |
| **G389** Exact plane sections of every body kind (geom-draw) | `topology/brep/planar_section.rs`, `topology/brep/cutting.rs` | `topology.brep_planar_section`, `topology.brep_cutting` partial | Inside. Plane-based bodies cut exactly; curved ones by certified plane intersection; regions keyed by name | read the code |
| **G390** Closed-form sections and outlines of quadrics, pipes and tori (geom-intersect, geom-draw) | `intersect/surface_drop_lib/cylinder_section.rs`, `surface/plane/`, `surface/cone/`, `surface/sphere.rs`, `surface/torus.rs` | green; `surface.plane_p1`, `surface.torus` partial | Inside. Conic kind decided by exact tests; cusps certified | matched by name only |
| **G391** Certified free-form sections and composite sections (geom-draw) | `topology/brep/planar_section.rs`, `intersect/surface_intersector/` | partial; `intersect.surface_intersector_p2` partial | Inside. Through our certified plane intersection | read the code |
| **G392** Certified contour generators (geom-intersect) | `intersect/surface_silhouette.rs`, `intersect/surface_tracer.rs`; SMLib `source/SMLib/src/SmDirectedSilhouette.cpp` (in no solidcore unit) | green; the SMLib file is not ported | Inside. Loop tests by cone of normals; cusps isolated; tracing certified; branch keys by name | read the code |
| **G393** Contours near extraordinary vertices (geom-intersect) | none found (neither file list has a subdivision file) | n/a | Nothing to port | n/a |
| **G394** Exact hidden lines for plane-based bodies (geom-draw) | `blend/hcr.rs`; SMLib `source/SMLib/src/SmDirectedSilhouette.cpp` | `blend.hcr` green | Oracle. Ours counts exact events on plane-based bodies; compare visible pieces only | read the code |
| **G395** Bodies that run into each other (geom-draw) | none found (`blend/hcr.rs` takes one body) | `blend.hcr` green | Oracle. Compare with a union drawn by G394 | read the code |
| **G396** Certified hidden lines for curved bodies (geom-draw) | `blend/hcr.rs`, `intersect/surface_silhouette.rs` | green | Inside. Events certified and refined to the bound; CertificationFailed with remedies | read the code |
| **G397** Projected curve kinds and canonical drawing form (geom-draw) | `topology/api_projection.rs`, `curve/projected_curve.rs` | `topology.api_projection` partial; `curve.projected_curve` green | Inside. Kind-preserving cases exact; rational picture coordinates; the rest certified NURBS with provenance | matched by name only |
| **G398** Line classes, bands and continuity records (geom-draw, geom-topo) | `query/tangent_edge.rs`, the silhouette edges of `blend/hcr.rs` | native; `blend.hcr` green | Inside. Tangent edges proven all along, as theirs does; classes from construction history first | read the code |
| **G399** Drawing checker and regression (geom-draw, geom-test) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |
| **G400** Visible-face map and shadows (geom-draw) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |
| **G401** Drawing references: dimensions and overrides (geom-draw) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |
| **G402** Incremental drawing engine and parallel views (geom-draw) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |
| **G403** Perspective and oblique projections (geom-draw) | `intersect/surface_silhouette.rs` (its eye-point form) | `intersect.surface_silhouette` green | Inside. Perspective predicates exact (G388); near clip required | read the code |
| **G404** Developed and civil sections (geom-draw) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |
| **G405** Drawing storage, explode and export (geom-draw, geom-format) | none found (no drawing model in either tree) | n/a | Nothing to port | n/a |

### Phase 12: native format and exchange

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G410** Records for meshes, SubD, fields, 2D, display and clouds (geom-format) | `crates/solidcore-record/src/native/binary.rs`, `brep.rs` as a comparison only | native crate | Oracle. Our records follow chapter 24; nothing of theirs covers meshes, SubD, fields or clouds | matched by name only |
| **G411** Stored display data and safe cold open (geom-format, geom-mesh) | none found | n/a | Nothing to port | n/a |
| **G412** Permanence harness: independent reader (geometry/tools/format-reader, geom-test) | none found | n/a | Nothing to port | n/a |
| **G415** ISO 10303-21 codec (geom-p21) | `crates/solidcore-record/src/step/p21.rs`, then `crates/step-import/src/part21.rs`; writer side in `crates/step-export/src/lib.rs` | native crates | As is. No tolerance in a parser. Needs streaming over ByteSource, no_std, typed errors with line numbers, a canonical writer; keep its hostile-input caps | read the code |
| **G416** EXPRESS code generation and validator (geom-gen (output in the format crates)) | none found (their entity map, `crates/step-import/src/map.rs`, is written by hand) | n/a | Generated from the schemas | matched by name only |
| **G417** Rounding imported polyhedra (geom-heal) | none found; `crates/step-brep/tests/issue_2581_faceted_brep.rs` is a case | test data | Oracle. Rounding once onto the grid is ours | matched by name only |
| **G418** ISO 10303-42 geometry mapping (geom-exchange) | `crates/solidcore-record/src/step/read.rs`, `step/geom.rs`, `step/units.rs`; `crates/step-import/src/map.rs`; `crates/step-brep/src/pcurves.rs` | native crates | Inside. Entity coverage as a cross-check of our generated types; its pcurve checks become TrimDisagreement; gaps certified | read the code |
| **G419** Healing: curve certification and re-intersection (geom-heal) | `heal/heal_data/` (p1 to p3), `heal/gap.rs`, `crates/step-brep/src/pcurves.rs` | `heal.*` green | Inside. Sender curves certified and re-intersected; a tolerance is never raised, which is how theirs heals | read the code |
| **G420** Healing: tangent edges, vertices, sewing and orientation (geom-heal) | `heal/stitch/` (p1, p2, `bounds.rs`), `query/tangent_edge.rs`, `crates/step-brep/src/lib.rs` (topology first) | `heal.stitch_p1`, `p2` green | Inside. Ambiguous on a non-unique match; orientation exact; tangency proven | read the code |
| **G421** IFC sweep mapping and sectioned sweeps (geom-ifc) | none found (no IFC file in either tree); the sweeps are G255 to G257 | n/a | Nothing to port | n/a |
| **G422** IFC import: profiles, CSG, clipping and openings (geom-ifc) | none found (no IFC file in either tree) | n/a | Nothing to port | n/a |
| **G423** IFC import: B-reps, tessellations, mapped items and placements (geom-ifc) | none found (no IFC file in either tree) | n/a | Nothing to port | n/a |
| **G424** IFC 4.3 terrain, alignment and corridor mapping (geom-ifc) | none found (no IFC file in either tree) | n/a | Nothing to port | n/a |
| **G425** Kind-by-format export matrix (geom-convert, geom-test) | none found | n/a | Nothing to port | n/a |
| **G426** IFC export (geom-ifc) | none found (no IFC file in either tree) | n/a | Nothing to port | n/a |
| **G427** STEP import: B-rep and Tolerant bodies (geom-step) | `crates/step-brep/src/` (`lib.rs`, `constructor.rs`, `pcurves.rs`), `crates/solidcore-ops/src/file.rs` | native crates | Inside. Topology first, as theirs; Tolerant bodies with certified gaps; every failure typed; AP242 is ours | read the code |
| **G428** STEP import: assemblies, tessellation and properties (geom-step) | `crates/solidcore-record/src/step/product.rs`, `crates/solidcore-ops/src/step_assembly.rs` | native crates | Inside. Placements composed exactly; tessellated geometry, validation properties and persistent IDs are ours | read the code |
| **G429** STEP export (geom-step) | `crates/step-export/src/lib.rs`, `crates/step-brep/src/brep_to_model.rs` | native crates | Inside. Canonical order and byte-identical round trips; AP242 and validation properties are ours; refuse rather than approximate, as theirs | read the code |
| **G430** DXF read: 2D curves, hatches, text and thickness (geom-dxf) | none found (no DXF file in either tree; report 15 section 17 covers CADCraft's) | n/a | Nothing to port from these two | n/a |
| **G431** DXF read: blocks, inserts, dimensions and leaders (geom-dxf) | none found (no DXF file in either tree; report 15 section 17 covers CADCraft's) | n/a | Nothing to port from these two | n/a |
| **G432** DXF read: 3D faces, meshes and references (geom-dxf) | none found (no DXF file in either tree; report 15 section 17 covers CADCraft's) | n/a | Nothing to port from these two | n/a |
| **G433** DXF write, binary DXF and code pages (geom-dxf) | none found (no DXF file in either tree; report 15 section 17 covers CADCraft's) | n/a | Nothing to port from these two | n/a |
| **G434** Mesh formats: STL, OBJ, PLY, glTF and 3MF (geom-meshio) | none found (no STL, OBJ, PLY, glTF or 3MF file in either tree) | n/a | Nothing to port | n/a |
| **G435** 3MF manufacturing extensions (geom-meshio) | none found | n/a | Nothing to port | n/a |
| **G436** Point-cloud exchange rules (geom-exchange, geom-cloud) | none found | n/a | Nothing to port | n/a |
| **G437** LandXML 1.2 mapping (geom-landxml) | none found | n/a | Nothing to port | n/a |
| **G438** Rhino .3dm (geom-3dm) | none found (the piece names openNURBS) | n/a | Port that, as the piece says | n/a |
| **G439** Decision notes for formats not yet owned (notes) | `nurbs/iges.rs` for the IGES note only | `nlib.iges` green | Oracle. It reads entities 126 and 128 only; a fact for the note | read the code |
| **G440** Exchange conformance harness (geom-test) | `scripts/fetch-steptools-corpus.sh`, `scripts/fetch-nist-pmi-corpus.sh` and their `.sha256` lists; the goldens crate's `--step-corpus` report | test tooling | Oracle. Fetch scripts as a pattern: pinned hashes, nothing committed | matched by name only |
| **G441** Oracles: IfcConvert and web-ifc (geom-test) | none found | n/a | Nothing to port | n/a |
| **G442** Oracle: OpenCascade DRAW (geom-test) | none found (usd-brep's OpenCascade translators were not read) | n/a | Nothing to port | n/a |
| **G443** Oracles: NIST STEP analyser, ezdxf and LibreCAD (geom-test) | `scripts/fetch-nist-pmi-corpus.sh` | test tooling | Oracle. The file list only | matched by name only |

### Phase 13: release

| Piece | Start from | Their status | What our rules need | How sure |
| --- | --- | --- | --- | --- |
| **G450** Spare Construct scenario generator and calibration (geom-bench) | none found (their `scripts/perf/` replays SolidCraft sessions) | n/a | Nothing to port | matched by name only |
| **G451** Spare Construct target scenarios (geom-bench) | none found (their `scripts/perf/` replays SolidCraft sessions) | n/a | Nothing to port | matched by name only |
| **G452** Wall-clock and scaling gates (geom-bench, CI) | none found (their `scripts/perf/` replays SolidCraft sessions) | n/a | Nothing to port | matched by name only |
| **G453** C interface (engine-geometry-c) | none found (the C API lives in the SolidCraft fork of SMLib, not in solidcore) | n/a | Generated from our schemas | n/a |
| **G454** WebAssembly bindings (engine-geometry-wasm) | `scripts/wasm-digest.sh` and the goldens crate's `--digest-dir` | test tooling | Oracle. A second pattern for the native-against-wasm check we already run | read the code |
| **G455** Python bindings (engine-geometry-py) | none found (usd-brep's Python layer binds the C++) | n/a | Generated from our schemas | n/a |
| **G456** Browser builds: threads, memory ceiling and cancellation (geom-pool, engine-geometry-wasm) | none found | n/a | Nothing to port | n/a |
| **G457** Cross-program scenarios: 2D, clouds, design, civil (geom-bench) | none found (their `scripts/perf/` replays SolidCraft sessions) | n/a | Nothing to port | matched by name only |
| **G458** Cross-program scenarios: mechanical, boards, manufacturing, meshes (geom-bench) | none found (their `scripts/perf/` replays SolidCraft sessions) | n/a | Nothing to port | matched by name only |
| **G459** Preview precision (geom-op, geom-format) | none found; nearest `crates/solidcore-tess/src/tolerance.rs`, `preview.rs` | native crate | Nothing to port | matched by name only |
| **G460** Revit truth-test harness (geom-test) | none found | n/a | Nothing to port | n/a |
| **G461** Revit batch: walls, openings, floors and roofs (geom-test) | none found | n/a | Nothing to port | n/a |
| **G462** Revit batch: stairs, curtain grids, structure, MEP and rebar (geom-test) | none found | n/a | Nothing to port | n/a |
| **G463** Revit batch: toposolids and Revit's failure cases (geom-test) | none found | n/a | Nothing to port | n/a |
| **G464** AutoCAD-class drafting truth tests (geom-test) | none found | n/a | Nothing to port | n/a |
| **G465** Outside failure catalogues as fixtures (geom-test) | `plan/upstream-issues/`, `crates/goldens/expected/known_failing.json`, `crates/solidcore/tests/kernel_cases/known_failing.json`; usd-brep `KNOWN_ISSUES.md` | test data | Oracle. Each failure rebuilt in our own files with a status (section 5) | matched by name only |
| **G466** Certify mode: candidates and design note (geom-intersect) | `crates/ssi/` (`algebraic.rs`, `taylor.rs`, `trace2d.rs`, `quadric.rs`), `docs/ssi.md`, `reports/ssi-corpus.json` | native crate (MIT or Apache-2.0) | Inside. A fourth candidate beside the three papers; measured on the same corpus | read the code |
| **G467** Certify mode: implementation (geom-intersect) | `crates/ssi/src/intersect.rs`, `intersect/adv_surface_intersector.rs` | native crate; `intersect.adv_surface_intersector` green | Inside. Topology certified or a structured error; their 78 of 119 is not above SMLib's 81 | matched by name only |
| **G468** Certify mode: local escalation and default policy (geom-intersect) | none found | n/a | Nothing to port | n/a |
| **G469** Research: certifying field meshes at sharp CSG features (geom-field) | none found (research) | n/a | Nothing to port | n/a |
| **G470** Research: eigen-tables for dart and corner vertices (geom-surface) | none found (research) | n/a | Nothing to port | n/a |
| **G471** Research: curved-eave skeletons (geom-skeleton) | none found (research) | n/a | Nothing to port | n/a |
| **G472** geom-sheet toolkit (geom-sheet) | `surface/bend_volume.rs`, `surface/unbend_volume.rs`; cases in `crates/solidcore/tests/kernel_cases/sheet-metal/` | `surface.bend_volume`, `surface.unbend_volume` green | Inside. The bend maps only; flanges, reliefs and recognition are ours; flat lengths within a certified bound | read the code |
| **G473** geom-board copper operations (geom-board) | none found (no board file in either tree) | n/a | Nothing to port | n/a |
| **G474** Board exchange: Gerber X2 and X3 (geom-gerber) | none found (no board file in either tree) | n/a | Nothing to port | n/a |
| **G475** Board exchange: IPC-2581 (geom-ipc2581) | none found (no board file in either tree) | n/a | Nothing to port | n/a |
| **G476** STEP AP242 semantic PMI (geom-step) | `scripts/fetch-nist-pmi-corpus.sh` (the corpus); nothing reads PMI | test tooling | Oracle. The file list only | matched by name only |
| **G477** Placement variables at D = 3 and D = 6 (geom-sketch) | none found | n/a | Nothing to port | n/a |
| **G478** 3D sketch entities (geom-sketch) | none found | n/a | Nothing to port | n/a |
| **G479** Assembly mates and mechanism stepping (geom-assembly) | cases in `crates/solidcore/tests/kernel_cases/assemblies/`; `crates/solidcore-ops/src/step_assembly.rs` | test data | Oracle. Mates and stepping are ours | matched by name only |
| **G480** Playbooks: formats, representations, toolkits (geometry/AGENTS.md) | not a code piece (playbooks) | n/a | n/a | n/a |
| **G481** Third-party notices for shipped artefacts (CI) | not a code piece (CI notices) | n/a | n/a | n/a |
| **G482** First Stable release of engine-geometry (engine-geometry) | not a code piece (the release) | n/a | n/a | n/a |


## 4. SMLib files that solidcore has not ported

By the `upstream` lists of solidcore's 282 unit files, these SMLib sources belong to no unit. The ones that matter to an unbuilt piece:

| SMLib file | What it is | Piece |
| --- | --- | --- |
| `source/SMLib/src/SmCurveFilletExecutive.cpp` | Fillet between two curves, with trim, join and a choice of side | G205 |
| `source/SMLib/src/NL_SrfLstSqFit.cpp` | Least-squares surface fits to scattered points and normals | G351, G380 |
| `source/SMLib/src/SmDirectedSilhouette.cpp` | Silhouette spans sorted across a view, for visibility | G392, G394 |
| `source/SMLib/src/SmBlendSurface.cpp` | A blend surface through sections along rails | G259 |
| `source/SMLib/src/NL_SrfShape.cpp`, `NL_CrvShape.cpp` | Shape edits: bend, warp, flatten, constraint fits | none on the board |
| `source/SMLib/src/SmSLC.cpp` | Slices of a body as point loops | the slicing pieces are built |

## 5. Test material

**The defect list as attack tests.** `plan/upstream-issues/` holds 1,150 files. Its [page](https://tangency-labs.github.io/solidcore/issues.html) counts 1,096 entries, 368 confirmed by a C++ run. Each gives the function, the trigger and the right answer. We rebuild the input from the description in our own API and assert the exact answer or a typed refusal; no fixture is copied. Several batches are already open as `attack/smlib-*` branches. For a new piece, search the folder for the SMLib class its row names before writing the piece's attack entries.

**The differential oracle.** `crates/difftest`, which report 15 points to, no longer exists. What we have in its place:

- *Its ideas*, still in report 15, section 4, for our own runner (G113): perturb the output to test the comparator, pad the heap to find address-dependent output, a waiver file with reasons. `plan/difftest-waivers.toml` is still there as an example.
- *The goldens*, `crates/goldens/expected/<unit>.jsonl`, are solidcore's own recorded outputs. They are a regression record, not truth, so we never assert them. They are useful as a list of cases per unit, and as a second opinion where our result is exact or certified and theirs disagrees. The crate's `--digest-dir` and `scripts/wasm-digest.sh` compare native with wasm32 output by hash, the same check we run.
- To compare with SMLib's C++ itself, the route is our own oracle container (decisions 32.23 and 32.44), not solidcore.

**Kernel cases.** `crates/solidcore/tests/kernel_cases/` holds operation cases as data, by area: fillets, booleans, sweeps, shell, surfaces, threads, sheet-metal, import, history, assemblies, tessellation, continuity and a ladder. The [cases page](https://tangency-labs.github.io/solidcore/cases.html) shows 311 with pictures, 15 of them known failing. These are inputs for G252, G282, G283 and G465; the expected values come from our own exact truth.

**STEP in and out.** `crates/solidcore-record/src/step/` (Part 21, Part 42 geometry, product structure), `crates/step-import`, `crates/step-brep` and `crates/step-export`. Besides being the start for G415 to G429, they give us a way to bring SMLib's models to our kernel: solidcore, run out of process, reads an `.smb` file and writes STEP, and our importer reads that. `scripts/fetch-steptools-corpus.sh` and `scripts/fetch-nist-pmi-corpus.sh` fetch public STEP corpora by pinned hash and commit nothing, a pattern for G440.

**solidcore-viewer.** The [page](https://tangency-labs.github.io/solidcore-viewer/) and its [repository](https://github.com/tangency-labs/solidcore-viewer) hold three things:

- *A model list.* The 262 B-rep part files of SMLib's own test corpus (`TestFiles/pt_TestFiles` in usd-brep), 2,631 faces, by folder: Fillet 96, Shell 25, Defeature 12, Solids 37, Boolean 4, Tess 2, MakeFace 1, and 85 at the top level, most of them surface intersection pairs. These are the "25 shell fixtures" and "12 defeature fixtures" that G277, G279 and G282 name. The repository holds only meshes made from them, not the models. Under 32.44 we read the models from usd-brep at its pinned commit during a test run and do not copy them into our repository.
- *A comparison method.* Both kernels sample every edge at 48 parameters, every face on an 88 by 88 grid kept where the point is inside the face, and every trim loop. A model matches when the face and edge counts agree, the inside mask agrees at every grid point and every sample is within 1e-5. `manifest.json` records that all 262 match. This is the shape of comparison our oracle runs need (G113, G442): counts, then classification, then distance. Ours would compare against certified bounds, not 1e-5.
- *A viewer for differences.* A static page with side by side, overlay and difference views, colouring each point by how far the two results are apart. `tools/build.py` documents the data format and `tools/wire.rs` is the solidcore side of the sampler. It is a ready pattern for looking at a failed oracle comparison, attached to a CI report and never gating, like the drawing overlays of G399. It loads three.js from a CDN, so a copy of ours would vendor that.

The viewer was last pushed on 2026-10-23, before solidcore stopped comparing with the C++, so its "all match" describes that date.

## 6. What I did not check

- I read headers, not whole files. No row says a method is correct, only where it is.
- The rows marked `matched by name only` are guesses from names. A name can mislead: `blend/hcr.rs` is hidden-line removal, not blending.
- "Green" was not measured by me and is no longer measured by solidcore against SMLib.
- I did not read our own chapters for each piece beyond its board entry, so "what our rules need" is a phrase, not a design. The piece's chapter decides.
- Which pieces are unbuilt came from branch names, open pull requests and commit subjects on main on 2026-11-10. A piece built under another name would be missed.
- I did not run solidcore, its goldens, the STEP crates or the viewer's tools, and did not confirm that solidcore can write every `.smb` model as STEP.
- usd-brep's OpenCascade translators and Python layer were not read.
- The SolidCraft fork of SMLib (tangency-labs/smlib), which many "not upstream" files cite, was not opened.
