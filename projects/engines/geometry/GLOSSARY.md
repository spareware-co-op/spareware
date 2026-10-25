# Glossary

Every term used in phases 1 to 3 of the kernel's work board, defined once, in plain words. Phases 1 to 3 build the ground and the numbers, the geometry kinds, and topology, names, the operation contract and the native format ([architecture/30-build-order.md](architecture/30-build-order.md)); the terms here are the ones those pieces and the chapters they rest on use, with the examples chapter 26.3 names.

How to read an entry:

- Entries are in alphabetical order word by word, ignoring case, accents and punctuation: a space, hyphen or underscore ends a word, so "Batch runner" comes before "`BatchBudget`", and "Bézier" sorts as "Bezier". Code names are in `code`. Other names an entry goes by follow its headword as "(also: ...)", so searching for any of them finds it; a word in brackets after a headword, as in **Class** (exactness), says which sense the entry defines. Each name belongs to one entry.
- A number in parentheses is a section of the kernel architecture: (6.2) is section 2 of [chapter 6](architecture/06-numbers.md). An ID such as G27 is a piece on the work board. "Proposed" marks what waits for an owner's decision (chapter 32).
- Crate names (`geom-num`, `geom-topo` and the rest) are not repeated here: each crate's job is in [MAP.json](MAP.json) and chapter 27.
- A path in backticks is written from the repository root when it starts with `projects/` or `.github/`, and from this folder, `projects/engines/geometry/`, otherwise.

## A

**Acorn**: A body kind: a body made of points only (8.1).

**Advisory**: A published notice that a version of a crate has a security or soundness problem. The merge gate's cargo-deny check refuses a dependency with an open advisory (28.8).

**Allow-list**: A list of what is allowed, where anything else is refused. The licence allow-list is chapter 29.2's SPDX identifiers; the crate allow-list is `deny-no-std.toml`'s `allow`, which names the published crates a kernel crate may depend on (`libm` today).

**`AllPieces`**: What a reference can ask for: every piece a recorded split made of its target, rather than one (20.4). Compare `One` and `Carrier`.

**`Ambiguous`**: A base error kind: more than one result fits and nothing given chooses between them (22). Also a reference's outcome when a name was split into several pieces and the reference wants `One` (20.4).

**Angular bound** (also: ε_ang, `eps_ang`): The tolerance ladder's bound on angles in curved code: 2^-34 radians, the same at every grid step, since it equals ε_fit over a frame's range (6.5).

**Antisymmetry**: The law that swapping two input points of an orientation or circle predicate flips its sign (28.6). See also law.

**Approximate** (also: Approximate(ε)): An exactness class: within a measured, not proven, deviation ε of the true shape, such as a mesh fitted to a scan (9). Nothing labelled Approximate is ever called Certified.

**Arena**: The storage of one kind of topological entity in a body: a vector addressed by a typed index, with no pointers between entities (8.2). The kernel's arenas are column-chunked (23.9).

**Arena index in name order**: The proposed rule that the builder assigns arena indices in persistent-name order, so a body's arenas depend only on what it contains, never on how it was built (8.2; owner action G76).

**Arrangement**: The division of a plane or of space into cells by a set of curves, surfaces or planes. Booleans, cells and rooms are computed from arrangements (11; 14).

**`atan2`**: The angle of a point (x, y) from the x-axis. The kernel's `geom_num::math::atan2` is a correctly rounded port of CORE-MATH's (6.4).

**Attribute**: Data hung on a body or on one of its named entities, such as a fire rating on a face (8.4). Values are keyed by persistent name, carried through edits by their class, and never decide shape.

**Attribute class**: What an attribute's values do at each event of an operation's history: keep, transform, propagate, combine, copy or delete (8.4). The kernel applies classes in one shared step in `geom-name`; there are no callbacks.

**Attribute definition**: An attribute's owner and name, version, the entity kinds that may carry it, the schema of its value, and its class (8.4). A definition never changes; a new class is a new version.

**Attribute event**: One of the eight things an operation's history can do to an entity, each of which an attribute class answers: unchanged, changed, transformed, split, merged, copied, generated or deleted (8.4).

**Audit**: A written review of a published crate before it may ship in the kernel: its notices, its `unsafe` code, its panics and its allocation (29.20, rule 3). `libm`'s audit is piece G24's; rayon's is G33's.

**Avnaim's method**: The method of Avnaim, Boissonnat, Devillers, Preparata and Yvinec for the sign of a small determinant of b-bit integers using only b-bit arithmetic (23.3); piece G42 compares it with the kernel's exact paths.

## B

**B-rep** (also: boundary representation): A shape stored as its boundary: faces bounded by loops of edges meeting at vertices, with the surfaces and curves they lie on (8; 9). The kernel's editable, drawable representation.

**Banned-source scanner** (also: acad pattern names): A CI tool that searches the repository for signs of code that may not be used: Numerical Recipes markers, statements that code was taken from a copyleft project, and the names of a proprietary program's hatch patterns (piece G16; 29.20).

**Base kind** (also: `ErrorKind`): The shared, closed set of error kinds every kernel error belongs to: `InvalidInput`, `Degenerate`, `NoSolution`, `Ambiguous`, `Undecided`, `ExactnessNotAccepted`, `ToleranceExceeded`, `FrameRangeExceeded`, `BudgetExceeded`, `Cancelled`, `OutOfMemory`, `Unsupported` and `Internal` (22). Generic code handles any error by its base kind alone.

**Batch** (also: batch call): One call that runs an operation on many items, with one result per item, so one failure fails only itself (10.1). Each item has a stable key given by the caller, and its `OpPath` is the batch's plus that key.

**Batch rule**: How budgets work in a batch: each item runs under its own budget, so whether it passes never depends on the other items; a batch total is summed in item-key order, and the item at which it is first passed, and every later one, fails (23.10).

**Batch runner**: The driver every operation's batch entry uses, under the batch rule; a batch gives the same bits and names as the same calls made one by one (piece G104).

**`BatchBudget`**: The budget of a batch call: a budget per item, and optionally a total for the whole batch (23.10).

**Benchmark**: A measured run of an operation on fixed inputs. In this kernel benchmarks are gated on counts (instructions, cache misses, heap, WebAssembly fuel), never on time, on every pull request (23.13; 28.14). See count gate.

**Bernstein bounds** (also: Bernstein arithmetic): Bounds on a polynomial piece from its coefficients in the Bernstein (Bézier) basis: the piece lies in the convex hull of its control points, which bounds it with no sampling (12.3; piece G59).

**Bézier extraction**: Splitting a B-spline into its polynomial (Bézier) pieces by knot insertion (The NURBS Book, algorithm A5.6; 12.3).

**Big-endian**: A byte order with the most significant byte first; native format round trips run on a big-endian target under emulation, so the format never depends on byte order (28.13; piece G99).

**Big-integer oracle**: A test that computes the same formula in arbitrary-precision integers or rationals (`num-bigint`, `num-rational` or `dashu`) and compares it with the kernel's answer (28.6).

**Binding crates**: `engine-geometry-c`, `engine-geometry-wasm` and `engine-geometry-py`: the C, WebAssembly and Python interfaces beside the facade, the only crates of the kernel's family allowed audited `unsafe` code at the foreign-function boundary (27; phase 13).

**Bit budget**: The most bits any intermediate value of an exact computation needs, over every input in its domain. Each predicate's module derives its bit budget in writing and tests it with every coordinate at ±(2^40 − 1) (6.2).

**Bit-range certificate**: The predicate generator's record, for every step of a predicate, of the bound on its exact value, the bits that bound needs and the integer type that holds it; an independent checker re-derives it (piece G27).

**`BLAKE3`**: A cryptographic hash function, one of the two candidates for the kernel's content hash beside SHA-256 (owner action G79; 23.8).

**Body**: One shape: the top entity of the topology (8.1). Its kind is Solid, Sheet, Wire, Acorn or General. A body is immutable once built.

**Body record**: The native format's records for one body: an index record and chunk records (24.2; piece G94).

**bolero**: A Rust test harness whose one `check!` runs as a property test, as a fuzz target and under Kani (28.2). Its test mode draws fresh seeds, so it counts as the short fuzz, not as a fixed-seed property test.

**Bounded**: A form of `TopoName`: an edge or vertex named by the faces it bounds (20.2). Whether it should also hold the faces that end an edge is open (owner action G78).

**Box tree** (also: bounding-volume hierarchy, BVH): A tree of integer boxes for finding which things are near which; the kernel has one, deterministic, in `geom-bvh` (27; piece G54). It is built top-down by the binned surface-area heuristic with integer costs, refitted or rebuilt after edits, and traversed with an explicit stack; boxes are inflated by their geometry's class (exact, certified, tolerant), and every answer is sorted by entry key.

**Brent's method**: A safeguarded method for finding a root or a minimum of a function of one variable (piece G51).

**Budget** (also: `Budget`): The limits an operation runs under: work units and logical bytes, each counted the same way on every machine, so running out fails identically everywhere (23.10).

**`BudgetExceeded`**: A base error kind: a deterministic work or logical-memory budget ran out (22). It records which limit, the phase and what was used, and may be stored as model state. `geom-num`'s `NumError::BudgetExceeded` is different: it means an exact computation exceeded its proven bit budget, which only a defect can cause, and maps to `Internal`.

**Build mode**: Debug or release, with overflow checks on or off: the determinism matrix runs every target in each (21).

**Builder**: The only way to make a body: it checks the combinatorial rules as entities are added and assigns arena indices (8.2; piece G81).

**Bulge polyline**: A polyline of lines and arcs, each arc given by a bulge, with optional widths, as in DXF (7).

**`ByteSource`**: The `no_std` trait a reader uses to take bytes in order from wherever the program keeps them, so a result never depends on how the bytes arrive (24.7). Compare `RangeSource`.

## C

**`CacheKey`**: The hash that keys a cached operation result: the kernel edition, the operation's name and version, its `OpPath`, the precision and the canonical encoding of every input (23.8). A hit is the same bits as recomputing.

**Cancellation** (also: `Cancel`, `Cancelled`): Stopping an operation because the caller asked. The caller's `Cancel` trait is polled at fixed intervals; a cancelled operation publishes nothing and returns `Cancelled`, a base error kind that is never logged or cached (23.10; 22).

**Canonical bytes** (also: canonical encoding): The one encoding of a value, so the same value always gives the same bytes on every machine: the native format's rules on top of deterministic CBOR (24.3).

**Canonical encoder**: A `no_std`, deterministic CBOR encoder with one encoding per value, from which names and content hashes are computed (piece G86).

**Canonical null space**: A basis of a matrix's null space put in reduced echelon form over a fixed variable order, so the same matrix always gives the same basis (15; piece G49).

**Canonical output**: An operation's whole result in canonical bytes (bodies, names, history, exactness, diagnostics), or its error with its kind, entities and evidence; its SHA-256 is compared with a golden value (28.13; piece G96).

**cargo-bolero**: The tool that runs bolero harnesses under a coverage-guided fuzzer such as libFuzzer (piece G14).

**cargo-deny**: A tool that checks a dependency graph's licences, security advisories, sources and banned crates; the merge gate runs it over every crate of the kernel's family (28.8).

**cargo-llvm-cov**: A tool that measures which lines and branches the tests run (28.8; piece G21).

**`Cargo.lock`**: The file that pins the exact version of every dependency in the workspace. It is committed, and every command runs `--locked`, so CI never builds a version nobody reviewed (28.13).

**cargo-mutants**: A tool that makes small changes to the code and checks that some test fails (28.8; piece G21). See mutation testing.

**cargo-semver-checks**: A tool that finds breaking changes to a crate's public interface; it runs on `engine-geometry` before every release (25).

**Carrier**: The plane, axis or surface underneath a face or edge. A carrier reference resolves to it, so a dimension to a wall's face survives the face being split by an opening (20.4).

**Cash-Karp**: A Runge-Kutta method published by Cash and Karp (1990), also printed in Numerical Recipes; a file that cites the published method and copies nothing must pass the banned-source scanner (piece G16).

**CBOR**: Concise Binary Object Representation, an IETF standard binary encoding (RFC 8949). The native format is deterministic CBOR with a record layer on top (24.1).

**CBOR protocol tag**: The four-byte tag in a native file's label that says the file is SKG; to be registered with IANA (24.2; owner action G77).

**`cbor2`**: A Python CBOR library, used as an independent cross-check of the kernel's encoder and decoder (pieces G86 and G91).

**CDDL**: Concise Data Definition Language (RFC 8610), the schema language for CBOR: every native record type is described in it (24.1; piece G93).

**Ceiling**: The largest certified bound an operation may return without the caller explicitly accepting more: ε_fit, unless the operation states another (6.5). Passing it is `ExactnessNotAccepted`, and rolls back.

**Cell**: A region of a cellular model, such as a room bounded by shared wall faces (8.1; 11).

**Central differences**: A derivative estimated from the function on either side of a point, used to check computed derivatives (piece G52).

**Certified** (also: Certified(ε)): An exactness class: within a proven bound ε of the true shape, with ε at most ε_fit unless the caller asked for more (9). The bound comes from interval or Bernstein arithmetic, never from sampling.

**CGAL**: The Computational Geometry Algorithms Library. It is GPL, so its source is never read: its methods are studied through papers and public documentation only, and its tools may run only out of process, as CI oracles, once an owner confirms it (14.14; 28.10; owner action G75).

**Chain of title**: The record of who owns a work's copyright; a licence is only as good as the licensor's ownership (29; 29.20, rule 2).

**Charge schedule**: What an operation charges in work units and when, fixed by the operation's version, so a count never changes without a new version (23.10; piece G83).

**Check tiers**: When each check runs: on every pull request, nightly, weekly, on a toolchain or dependency change, and at each release (28.17; piece G10).

**Checked arithmetic**: Integer arithmetic through methods that report overflow (`checked_add`, `checked_mul`) instead of operators that wrap in release builds. Every fixed-width step in kernel code is checked, so no answer depends on overflow checks being on (27; 6.2).

**Checked, not proven**: The label of a filter whose error bound has been tested but not yet machine-proven by FPTaylor or Gappa (28.6). Every generated filter carries it until pieces G35 and G40.

**Checker groups**: The four groups of the validity checker: 1, combinatorial; 2, geometric consistency; 3, self-intersection; 4, the exactness record (8.3).

**Cholesky decomposition**: The factorisation of a symmetric positive definite matrix as LLᵀ with L lower triangular (piece G49).

**Chunk**: A fixed run of rows of a body's table, 4,096 by the proposal, shared between versions so an edit copies and rewrites only what changed (23.9; 24.2).

**`ChunkPlan`**: The fixed chunk size a call of the parallel helper splits its items into, chosen for the cost of one item and never for the thread count, so results do not depend on threads (23.4).

**Circular involute**: The curve traced by the end of a string unwound from a circle, used for gear teeth; a curve kind (7).

**Class** (exactness): See exactness class. For attributes, see attribute class.

**Clippy**: Rust's linter. The kernel runs it with its own denials, chapter 21's bans and the pedantic group, warnings denied (27; 28.8).

**`clippy.toml`**: A crate's clippy configuration. Every crate that ships copies `geom-num`'s: the lints tests may relax, and the disallowed methods and types of chapter 21 and the randomised-method rule.

**Closed enum**: An enum whose variants are all known, matched exhaustively. Curve and surface kinds are stored in closed enums, one variant per kind (7).

**Clothoid**: A spiral whose curvature changes linearly with length, used in road and rail alignments; a curve kind (7; piece G63).

**ClusterFuzzLite**: A service that runs fuzz targets in CI on pull requests (piece G14; piece G116).

**Coarsening**: Converting coordinates to a coarser grid step: it divides by a power of 25, rounds, never ties, and reports that it rounded (6.1; 6.3; piece G22). Compare refinement.

**Codec**: The code that writes a record as bytes and reads it back. In the native format every kind has a tag, a CDDL description and a codec, and a body read back equals the one written (24.2; piece G94).

**Collapsed-edge division**: Dividing out the factor that makes a surface's derivative vanish along a collapsed edge (control points that coincide, a sphere's pole, a cone's apex), so the normals can still be bounded there (12.3; piece G59).

**Column-chunked arena**: An arena stored as one column per field (all face boxes together, all fin links together), each column cut into chunks shared between versions (23.9; piece G80).

**Combinatorial group**: Checker group 1: every loop closes, every fin has a partner or is a recorded boundary, shells are closed and oriented, regions nest, and Euler's formula holds (8.3).

**Command layer**: Spare Kit's one layer through which programs, people, scripts, plugins and AI agents run operations, generated from the operation schemas (22; `ARCHITECTURE.md`).

**Commit** (also: commit to the grid): Making an implicit point explicit by rounding each coordinate to the nearest grid step, ties to even; it moves a coordinate by at most half a step and says whether it moved (6.3).

**Commit rounding**: The rung of the tolerance ladder for a commit: at most q/2 per coordinate, (√2/2)q in 2D and (√3/2)q in 3D as a distance (6.5).

**Complete orthogonal decomposition**: A factorisation A = Q·T·Zᵀ with Q and Z orthogonal and T triangular of the matrix's rank, used for rank-deficient least squares (piece G49).

**Completeness gate**: A CI check over `MAP.json` that refuses a merge when an operation, kind, format or toolkit lacks anything chapter 26.1 requires: tests, fuzz target, benchmark, error tests, laws, example (28.2; piece G103).

**Composite curve**: A curve made of other curves joined end to end (7).

**Conservative bound**: A bound guaranteed to contain the true value, though perhaps larger than needed. Every curve and surface kind gives one over any parameter interval (7).

**Consulted record** (also: `Consulted:`): A header line crediting an outside source read for its method, with its licence, a link pinned to a full commit, and what it was consulted for; nothing is copied (HEADERS.md; 29.20).

**Container**: A file that holds native records inside some other storage, such as a SQLite project file; integrity is each record's checksum and content ID (24.5; piece G108).

**Content hash** (also: content ID): The SHA-256 by which a record or body is identified: of a record's payload, or for a body the root of a hash tree over its chunks (24.2; 23.8). Equal bytes give equal content IDs on every machine.

**Contract**: See operation contract and kind contract.

**Contract laws**: The checks every operation passes: transactional under injected cancellation and budget exhaustion, every error kind triggered, remedies applied, exactness and tolerance never lost unasked, the range held at 2^40 (28.2; piece G103).

**Copyleft**: Licences (GPL, AGPL, LGPL) that require work built from the code to carry the same licence. No copyleft code enters the kernel; copyleft tools may run only out of process (29; 29.2).

**CORE-MATH**: A project of correctly rounded mathematical functions with proofs; the kernel's `sinpi`, `cospi`, `exp`, `ln` and `atan2` are ports of its code, under its MIT licence (6.4; piece G05).

**Correctly rounded**: The result is the exact mathematical value rounded once to the nearest representable number. IEEE 754 requires it for +, −, ×, ÷, square root and fused multiply-add.

**Cost**: A module header's field stating what a function costs: time, memory, allocation and recursion (HEADERS.md).

**Cost model** (also: K06 cost model): Spare Kit's estimate of what the project's running costs will be, in note K06 (`projects/kit/notes/K06-cost-model.md`); CI machines and runners are entered in it (owner action G07; piece G10).

**Count gate**: A benchmark gate on counts that do not vary between runs (instructions and cache misses under Valgrind, heap under DHAT, WebAssembly fuel): a rise of more than 5% against the merge base blocks the pull request (23.13; piece G18). There are no stored baselines: every run measures the merge base afresh, so a baseline is re-recorded by merging the change that moves it.

**`count_then_fill`**: A parallel-helper function by which each item counts what it makes, an exclusive scan turns the counts into offsets, and each item fills its own range, so new indices are in input order on any number of threads (23.4).

**Counter-based generator**: A random number generator whose output is a pure function of a key and a counter, so any position of any stream can be computed directly; the kernel's is Philox4x32-10 (piece G32).

**Coverage**: Which lines and branches the tests run: a floor, with mutation testing as the real gate (28.8). A branch may be left uncovered only where a proof shows it cannot be reached, marked `coverage(off)` with the proof cited.

**Crate header**: The `//!` lines that open a crate's `lib.rs`: its summary, one job, layer, invariants and sections, in HEADERS.md's grammar.

**Crate table**: Chapter 27's table of every crate, its layer and its direct dependencies, held in code in `../crates/engine-geometry/tests/scaffold/table.rs` and checked against every `Cargo.toml` (owner action G08; piece G09).

**CRC-32C**: A 32-bit checksum (the Castagnoli polynomial) on every native record, to find damage cheaply (24.2).

**Curve kind**: One variant of the closed enum of curves: line, ray, infinite line, circle, arc, ellipse, parabola, hyperbola, helix, spirals, polynomial curve, involute, NURBS, bulge polyline, composite, trimmed, offset, pcurve and intersection curve (7).

## D

**`dashu`**: A pure-Rust arbitrary-precision arithmetic library (MIT or Apache-2.0), used as an exact oracle in tests (28.6).

**de Boor's algorithm**: The standard method for evaluating a B-spline from its control points and knots; the exact rational reference uses it (piece G57).

**Decision log**: Where kernel owners record decisions and gate results, with the CI runs that show them (30.1; chapter 32).

**Decision note**: A note that proposes or changes an architecture decision; nothing in `ARCHITECTURE.md` changes without one an owner has accepted (26.1).

**`Degenerate`**: A base error kind: a valid input is degenerate for this operation, such as a zero length or radius (22). A degenerate input to a predicate is one whose exact answer is `Zero`, such as three collinear points.

**Degree elevation** (also: degree reduction): Raising a curve's polynomial degree without changing its shape, or lowering it within a stated error (piece G56).

**Deleted**: A history record: an input entity the operation removed (20.3). Also the outcome of a reference whose target was deleted, naming the operation that did it (20.4).

**DEPEND**: One of chapter 29.1's ways of taking something: using a published crate as it is, pinned in `Cargo.lock`. Inside kernel crates only what the crate allow-list holds.

**Depth bound**: The stated maximum depth of a recursion, written in the module header and tested on a small WebAssembly stack (28.8; piece G13).

**Derivative bounds**: Conservative bounds on a curve's or surface's derivatives over a parameter interval; every kind gives them (7; piece G68).

**Description**: The few lines from which the predicate generator derives a predicate: its inputs, its intermediate values and the value whose sign it returns, in `geom-gen`'s description language (piece G27).

**Determinism matrix**: Every target, build mode, `target-cpu=native` and thread count the same-answer promise covers, on which every golden hash must match (21; piece G19).

**Deterministic**: The same inputs give bit-identical outputs on every supported target, build mode and thread count (principle 5; 21).

**Development crates**: `geom-test`, `geom-fuzz`, `geom-gen` and `geom-bench`: test, fuzz, generator and benchmark code that never ships. They may use `std`, and no kernel crate depends on them except for its tests (27; 28.1).

**DHAT**: Valgrind's heap profiler; the count gate measures peak heap with it (piece G18).

**Diagnostics**: Warnings an operation returns with the entities they concern, such as a short edge (10.1).

**Differential fuzzing** (also: differential testing): Running the same inputs through the kernel and an independent implementation and investigating every difference (28.10; piece G26).

**Direct dependency**: A kernel crate named in a crate's own `Cargo.toml`. The crate table lists exactly these (27).

**Disallowed methods** (also: disallowed types): Clippy's lists of paths a crate may not use. The kernel's ban platform maths, `HashMap` and `HashSet`, `f64::max` and `min`, `Iterator::sum` and `product`, clocks, thread identity and every source of randomness but `geom_num::rng` (21).

**`DocRef`**: A reference to another document: its permanent ID and the version a reference was made against (20.1).

**Domain toolkit**: A crate for one domain (sheet metal, threads, circuit boards, stairs), built only from existing operations and kinds; it enters the kernel only when two programs need it (26.1).

**Double-double**: A number held as the unevaluated sum of two doubles, giving about twice the precision of one, with stated error bounds (piece G25).

**Dry run**: Following a playbook to add a toy example and checking that it passes CI, to prove the playbook is complete (pieces G17, G117 and G480).

**Dual numbers**: Numbers a + bε with ε² = 0; computing with them gives a function's value and its derivative together (forward-mode differentiation; piece G52).

**Duplicate-name detector**: A check in every test build that fails when two entities get the same name where the operation's contract does not expect it (20.2; piece G87).

## E

**Edge**: A bounded piece of one curve, between two vertices or closed (8.1).

**Edition** (also: kernel edition): A version of the whole kernel's results. Within an edition, replaying a log gives the same model bit for bit; a document moves to a new edition only by an explicit upgrade operation that reports every difference (26.2).

**Eigensolver**: A method for the eigenvalues and eigenvectors of a matrix; the kernel's is for symmetric matrices (piece G50).

**Ellipse by parameter**: An ellipse whose points are given by its parametric angle, not its polar angle; a curve kind (7).

**Enclosure**: An interval guaranteed to contain the exact result of a computation (6.4; piece G31).

**END record**: The last record of a standalone native file: the record count and the SHA-256 of everything before it (24.2).

**Entry key**: The stable key of an entry in the box tree (a persistent name, or an arena index inside one operation), by which ties are broken and answers sorted (27; piece G54).

**Epoch** (also: coordinate epoch): The date survey coordinates refer to. In a dynamic or semi-dynamic datum such as NZGD2000 the ground moves, so coordinates are fixed at a reference date and a deformation model carries observations made on other dates to it; two frames at different epochs are never the same frame (17.1; piece G48).

**Equivalent mutant**: A change to the code that cannot alter its behaviour, so no test could catch it; marked with a written reason (28.8).

**Error catalogue**: The list of every error the kernel can return, with an example of each, generated from the code into `MAP.json` (26.3; piece G82).

**Error constant**: The bound of a predicate's floating-point filter, derived exactly by the generator and rounded up: when the computed value's magnitude exceeds it times the scale, the sign is certain (piece G27).

**Error enum** (also: error kind): A public enum that is an error type; each variant is one error kind. The map lists every error kind of every kernel crate, each with its doc comment (HEADERS.md; 22).

**Error-free transformation**: A computation that returns a rounded result and its exact rounding error as a second double, so the pair is exact: `TwoSum` and `TwoProduct` (`geom_num::eft`).

**Euler operators**: Operations that change a body's topology while keeping Euler's formula, the building blocks of the builder (piece G81; piece G111).

**Euler's formula**: The relation between the numbers of vertices, edges, faces, loops and shells of a valid body and its genus (the Euler-Poincaré formula); checker group 1 checks it (8.3).

**Evidence**: The points, parameters, distances and bounds an error carries to say where and why it failed (22).

**Exact**: An exactness class: decided entirely by exact predicates on exact data (9).

**Exact path**: The part of a predicate computed exactly in integers, run when the filter cannot be sure (6.2; piece G27).

**Exact peak test**: A test of an interval maths function at an argument where the function reaches a maximum or minimum, such as `sinpi` at 1/2, so its enclosure must contain the exact peak (piece G47).

**Exactness class**: What a result says about how close it is to the true shape: Exact, Certified(ε) or Approximate(ε). A result's class is the least exact of its inputs', and bounds add (9; `geom_num::Exactness`).

**Exactness record**: Checker group 4: every body's exactness class and bounds are consistent with its geometry (8.3).

**`ExactnessNotAccepted`**: A base error kind: the result would be less exact than the caller accepted, such as a certified bound over ε_fit (22).

**`ExactSum`** (also: superaccumulator, `add_slice`, `add_iter`): An exact sum of doubles in a wide fixed-point accumulator, rounded once at the end, so its bits never depend on the order of the terms or how they were split (piece G25; 23.4). `add_slice`, `add_iter` and `collect` are its entry points for whole collections.

**`Exec`**: The parallel helper's executor: a pool, or none for serial running with the same answer (23.4).

**Explicit point**: A point stored as grid coordinates. Compare implicit point.

**Explicit-point predicates**: `orient2d`, `orient3d`, `incircle` and `insphere`: predicates on grid points (6.2; piece G28).

**Extreme-value tests** (also: maximal-term corners): Tests with every coordinate at ±(2^40 − 1), every sign combination for explicit-point predicates, and the configurations that maximise each term for implicit ones (28.6).

## F

**`f64`**: A 64-bit IEEE 754 floating-point number (binary64). Rust computes its +, −, ×, ÷ and square root exactly to IEEE rules on every supported target (21).

**Facade** (also: `engine-geometry`): The kernel's one stable public interface: operation schemas, stable value types and re-exports, and no kernel logic. Programs depend only on it (25; 27).

**Face**: A bounded piece of one surface, with a sense saying which side is material (8.1).

**Family** (also: catalogue family): One of chapter 10.2's groups of operations: Make, 2D, Sketch, Sweep, Combine, Blend, Offset, Transform, Repair, Query, Draw, Mesh, Manufacture, Free-form and Ground. A crate header states its family.

**Fat line**: The thinnest strip containing a curve, found from the Bernstein coefficients of its distance to a line; Bézier clipping cuts curves with it. The surface form is a fat-plane slab (12; piece G59).

**Fault injection**: Breaking valid bodies on purpose, one fault at a time (a flipped fin, an open loop, a vertex moved just off its surface), to show that the validity checker catches every fault with the right error kind while still passing every valid body (28.4; pieces G110 and G135). The contract laws inject cancellation and budget exhaustion into operations in the same spirit.

**Filter** (also: semi-static filter): The fast first step of a predicate: the formula in `f64` with an error bound derived ahead of time, which gives the sign when the computed value is larger than the bound, and otherwise leaves the decision to the exact path (6.2).

**Filter-bound certificate**: A machine-checked proof that a filter's error constant is at least the true bound, by FPTaylor or Gappa (28.6; piece G40).

**Filter boundary**: Inputs placed just inside and just outside a filter's error bound, so tests run both the filter and the exact path (28.6).

**`filter_ordered`**: A parallel-helper function that keeps the items a test accepts, in input order (23.4).

**Fin**: The use of an edge by a loop, in one direction. In non-manifold models, the fins around an edge are kept in radial order (8.1).

**`find_first`**: A parallel-helper function that returns the first match in input order, never any match (23.4).

**Fit bound** (also: ε_fit, `eps_fit`): The largest certified error of any curved construction that feeds topology: 64 grid steps, 0.01 mm at q0 (6.5). It is every operation's default ceiling.

**Fixture**: A permanent test case: an operation log, its expected result per edition and set of operation versions (`Expect`), and its status, `Pass` or `Known` (28.16; piece G97). A run reports Pass, Fail or Improved.

**Forbid unsafe code** (also: `forbid(unsafe_code)`, `unsafe_code`): The lint setting that refuses every `unsafe` block. Every kernel crate and the facade carry it (27).

**Format museum**: An archive of files and operation logs from every released format version and edition, each of which must open and replay to its recorded hashes forever (28.13; piece G99).

**Foundation set**: `geom-num`, `geom-poly`, `geom-check`, `geom-name`, the builder in `geom-topo` and the arrangement module of `geom-2d`: the code everything else rests on, held to no unexplained surviving mutant and full coverage (28.8).

**FPG**: Meyer and Pion's generator of fast, certified floating-point filters for geometric predicates; the kernel's predicate generator follows its method (6.2).

**FPTaylor**: A tool that proves bounds on floating-point round-off error, with certificates HOL Light can check (28.6; piece G35).

**Frame**: A local coordinate system that holds part of the real world, with its own origin, grid step and link to a map projection (6.1).

**`FrameMismatch`**: The error when values from two different frames are combined without converting between them (piece G48).

**`FrameRangeExceeded`**: A base error kind: a coordinate would leave ±(2^40 − 1) steps of its frame's origin (22).

**`FrameRecord`** (also: frame record): The record of a frame: its kind (Map or Ground), coordinate system, origin in grid steps, rotation, scale and lattice (6.1; 17.1; piece G48).

**FreeCAD**: An open-source modeller whose long fight with naming faces after edits taught the kernel what not to do; its lessons are a method note of piece G16 (20).

**Fresnel integrals**: The integrals of sin(t²) and cos(t²) that give a clothoid's position; the kernel encloses them with proven remainders (12.3; pieces G63 and G67).

**Fuel** (also: WebAssembly fuel): Wasmtime's count of WebAssembly operators executed, deterministic for a given binary; a count gate (piece G18).

**Fused multiply-add** (also: `mul_add`, fma): a × b + c rounded once. Kernel crates are `no_std`, so they write it as `geom_num::math::fma` (6.4).

**Fuzz target** (also: fuzzing): A test harness fed generated inputs for as long as a fuzzer runs, looking for panics, hangs and broken laws (28.5).

## G

**Gappa**: A tool that proves bounds on floating-point computations, writing proofs the Rocq proof assistant checks (28.6; piece G35).

**Gate** (also: gate review): The last piece of a phase: a review that runs the phase's gate condition and has a kernel owner record the result (30.1). Also any CI check that blocks a merge.

**Gauss-Legendre quadrature** (also: Gauss-Kronrod): Methods of numerical integration from weighted sums at chosen nodes; Gauss-Kronrod nests a second rule to estimate the error (piece G51).

**General**: A body kind: mixed and non-manifold (8.1).

**Generated** (history record): An output entity made from inputs, with a role, rather than modified from one (20.3).

**Generator**: A function in `geom-gen` whose output is committed to the repository; CI reruns every generator and fails on any difference (27). For random numbers, see counter-based generator.

**Geogram PCK**: Geogram's predicate construction kit, which writes one description per predicate and generates its filter and exact path; the kernel follows its method (6.2).

**`geomdl`**: NURBS-Python, an MIT-licensed NURBS library run out of process as the first outside oracle (28.10; piece G113).

**Geometry pool**: A body's tables of surfaces, curves and points, which faces, edges and vertices refer to by index (8.1).

**Golden hash** (also: golden digest, golden): The recorded SHA-256 of a canonical output, which every target must reproduce (28.13).

**Golden store**: Where golden hashes are kept, keyed by edition and operation version, with an owner's approval needed to change one (piece G19).

**Golub-Kahan SVD**: A method for the singular value decomposition by bidiagonalisation (piece G50).

**Golub-Welsch**: The method that finds Gaussian quadrature nodes and weights as eigenvalues and eigenvectors of a tridiagonal matrix (piece G51).

**Grid**: The integer lattice every stored coordinate lies on (6.1).

**Grid point**: A point whose coordinates are whole numbers of grid steps within ±(2^40 − 1) (6.1).

**Grid step** (also: `GridStep`): The distance between neighbouring grid points: q0, q1 or q2, each exactly 25 times finer than the one above (6.1; piece G22).

**Grid translation**: A move by whole grid steps, which keeps every grid point on the grid exactly (7; piece G45).

**Ground frame**: A site grid: a frame whose coordinates are map coordinates rotated, and scaled horizontally, by exact values (17.1; piece G48). Compare Map frame.

**Growth check**: A benchmark run at n, 2n and 4n, whose count must grow no faster than its stated cost (28.14; piece G18).

**gungraun**: A Rust benchmark harness (formerly iai-callgrind) that runs benchmarks under Valgrind's Callgrind and DHAT (piece G18).

## H

**h_min** (also: smallest triangle altitude): A proposed rung of the tolerance ladder: 8 grid steps, the smallest altitude tessellation may give a triangle, so commit rounding cannot flip it (6.5).

**Handover**: Stopping a piece part-way: committing, pushing, and opening a draft pull request that says what is done, what is left and the next step (the repository's AGENTS.md, section 6).

**Hansen-Sengupta operator**: An interval Newton operator: applied to a box, it can prove that a system of equations has exactly one root there, or none (piece G52).

**Harness**: The code that feeds inputs to the function under test, as a property test, a fuzz target or a proof.

**Hash-consed name graph**: The shared graph of persistent names, in which each name is stored once and refers to its parents by `NameId` (20.1; piece G87).

**Hash-tree content ID**: A body's content ID: the root of a tree of hashes over its index and chunk records, so a new version rehashes only what changed (24.2; piece G94).

**`HashMap`** (also: `HashSet`): The standard library's hash tables, whose iteration order is random per process; banned in kernel crates (21). Use ordered containers.

**Header**: See crate header and module header.

**Healing bound** (also: ε_heal, `eps_heal`): The largest gap import healing may close: 640 grid steps by default, capped at 6,400 (6.5).

**Helix**: A curve kind, exact; neither STEP nor IFC has one (7).

**Hex float**: A floating-point number written in hexadecimal, which reads back bit for bit; the native format's text form uses it (24.6).

**History**: What an operation says about every output entity: unchanged, modified from, generated from with a role, split from, or merged from, and which inputs it deleted (20.3).

**History laws**: The four rules every history keeps: a deleted input has no modified outputs; an input's generated and modified sets do not overlap; every output traces to an input or is generated; histories compose across operations (20.3; piece G88).

**`HistoryPruned`**: A reference's outcome when the history it needs was pruned on purpose, never `Deleted` (20.4; 24.4).

**Hodograph**: The derivative of a Bézier curve, itself a Bézier curve; its control points bound the derivative (12.3; piece G59).

**HOL Light**: A proof assistant that checks FPTaylor's certificates (28.6; piece G35).

**Householder QR**: The factorisation of a matrix into an orthogonal Q and a triangular R by Householder reflections, with column pivoting to reveal the rank (piece G49).

**Hyperbola** (also: parabola): Conic curve kinds (7).

## I

**`I128`** (also: `I256`, `I320`, `I384`, `I512`, `I576`, `I768`): Fixed-width signed integers of that many bits. `I256`, `I320` and `I384` hold the boolean predicates' budgets; `I512` is the total path; `I576` and `I768` are reserved for proposed predicates (6.2; piece G26).

**`i64`** (also: `i128`): Rust's 64-bit and 128-bit signed integers. A grid coordinate is an `i64`; orientation predicates fit in `i128` (6.2).

**IANA**: The Internet Assigned Numbers Authority, which registers protocol tags (owner action G77).

**IfcConvert**: IfcOpenShell's converter, an LGPL tool run only out of process as an oracle (28.10; owner action G75).

**Implicit point**: A constructed point kept as its definition, such as "where these three planes meet", in homogeneous integer coordinates, and decided by exact predicates without rounding (6.3).

**Implicit-point predicates**: Predicates on implicit points: classifying a three-plane point or a line-plane point against a plane, and 2D segment crossings (6.2; piece G29).

**`ImplicitBound`**: The trait in `geom-surface` through which a surface gives an implicit equation's bound (piece G60).

**Improved**: A fixture outcome: a `Known` fixture now gives its wanted result, and moves to `Pass` (28.16).

**In-process repeat**: Running each test case twice in one process, to catch hidden caches (28.13; piece G19).

**`incircle`**: The predicate saying whether a point lies inside, on or outside the circle through three others (6.2).

**Index** (also: typed index): A 32-bit position in a body's arena, typed by entity kind (`FaceIx`, `EdgeIx`). Valid only inside one body version, never persisted; persistent names are what references hold (8.2; 20.1).

**Infinite line** (also: line, ray): Straight curve kinds: unbounded both ways, bounded both ways, and bounded at one end (7).

**Input-shuffle hook**: Running a case with its inputs shuffled, wherever the contract says order does not matter (28.13; piece G19).

**`insphere`**: The predicate saying whether a point lies inside, on or outside the sphere through four others (6.2).

**Integer box** (also: box): An axis-aligned box whose corners are grid points (piece G45; piece G54).

**`Internal`**: A base error kind: an invariant broke, which is a bug; it carries what reproduces it and is never a panic (22).

**Interval** (also: interval arithmetic): Arithmetic on ranges [lo, hi] rounded outward, so the exact result is always inside (6.4; piece G31).

**Interval Newton**: Methods that prove or exclude the roots of equations in a box, such as the Krawczyk and Hansen-Sengupta operators (piece G52).

**Interval version**: A maths function evaluated over an interval, giving an enclosure of every value it takes there (6.4; piece G47).

**`InvalidInput`**: A base error kind: an input breaks the operation's preconditions (22).

**Invariant**: A property a crate or module keeps for every input; crate headers list theirs, and code checks them (27; HEADERS.md).

## K

**Kahan matrix**: A classic test matrix on which QR with column pivoting fails to reveal the rank, used to test rank-revealing factorisations (piece G49).

**Kani**: A bit-precise model checker for Rust that proves functions cannot panic or overflow for inputs up to a stated size (28.7; piece G41).

**Kernel crate**: A crate of the kernel that ships: `no_std`, no `unsafe`, `geom-num`'s lint posture (27). Compare development crates.

**Kernel owner** (also: owner): A person who may accept decisions about the kernel (32.5). Owner actions are board pieces only an owner can settle (30.1).

**Kind**: See curve kind, surface kind, body kind, base kind.

**Kind contract**: The traits and test kit every curve and surface kind passes: evaluation, derivatives, conservative bounds, conversion to NURBS with an error, split, reverse and transform (piece G60).

**kit-core**: Spare Kit's core crate, which holds permanent IDs and units; the facade converts them at its boundary (27; piece G100).

**Knot insertion** (also: knot refinement, knot removal): Adding or removing knots of a B-spline; insertion keeps the shape exactly, removal within a stated error (piece G56).

**`Known`**: A fixture status: a typed error today, with the success wanted recorded beside it. Only a typed error can be `Known` (28.16).

**Krawczyk operator**: An interval Newton operator that proves or excludes roots in a box (piece G52).

## L

**Label**: The 12 bytes that open a standalone native file and say what it is (RFC 9277; 24.2).

**Lattice**: Frames placed on a shared lattice of origins, so moving between neighbouring frames is an exact integer shift (6.1; piece G48).

**Law**: A property that must hold for every input, checked by property tests, such as antisymmetry and translation for predicates (28.3; 28.6).

**Law catalogue**: Chapter 28.3's laws for each family and kind, run as property tests with an independent refutation sampler (piece G69).

**Layer**: One of chapter 27's thirteen levels of crates; a crate depends only on lower layers, or along an edge the table allows inside its own (27).

**Leg**: One run of the test suite on one target, build mode and thread count of the determinism matrix (21).

**Lemon and apple**: The degenerate forms of a torus whose tube is wider than its centre circle; surface kinds (7).

**libFuzzer**: LLVM's coverage-guided fuzzer, run through cargo-bolero (piece G14).

**`libm`**: The pure-Rust maths library (MIT), the one published crate a kernel crate depends on today, for square roots, fused multiply-add, rounding and the unproven functions (6.4; 29.1).

**Licence choice** (also: `LICENCE_CHOICES`): The record of which licence the kernel takes where a crate offers several (`MIT OR Apache-2.0`): the first of chapter 29.2's identifiers it offers (29.20, rule 3).

**Line-plane point**: An implicit point where a line meets a plane (6.3; piece G29).

**Linear extrusion**: A surface swept by moving a curve along a straight line; a surface kind (7).

**Linear octree** (also: Morton-sorted cell index): Cells stored as a list sorted by Morton key, for finding neighbouring points (piece G55).

**Logical bytes**: Memory counted from entity and scratch counts at fixed sizes per kind, the same on every machine; the memory budget counts them (23.10).

**Loop**: A closed boundary of a face: one outer loop and any number of holes (8.1).

**Lossy cast**: A conversion between number types that can lose part of a value: a truncation, a wrap to the other sign, or a lost sign. Clippy denies them in kernel code, so a cast that cannot lose a value has its bound written beside it (23.3; piece G12).

**LU decomposition**: The factorisation of a matrix into lower and upper triangular factors, with partial pivoting for stability (piece G49).

## M

**`map`** (also: `map_each`, `try_map`): The parallel helper's mapping functions: `map` and `try_map` run one phase of an operation, reporting the lowest-numbered failure; `map_each` runs a batch with a budget per item (23.4).

**Map frame**: A frame whose coordinates are a map projection's grid coordinates, with no rotation and no scale (17.1; piece G48). Compare Ground frame.

**`MAP.json`**: The machine-readable map of every crate, module, error kind and outside source, generated from the headers and checked in CI (26.3; piece G15).

**Maths module**: `geom_num::math`, the only source of maths functions in the kernel: correctly rounded CORE-MATH ports and `libm` (6.4).

**MCP tool descriptions**: Descriptions of operations for AI agents through the Model Context Protocol, generated from the operation schemas, never written by hand (22; piece G100).

**Merge base**: The commit a pull request branched from; the count gates compare the pull request with it (piece G18).

**Merge gate**: The checks that block a merge: licences, advisories and sources; clippy with the kernel's bans; no global state; `no_std` and WebAssembly builds; the crate graph, each shown to fail on a seeded violation (28.8; piece G12).

**Merged**: A history record: one output made from several inputs (20.3). Also a form of `TopoName`.

**Method**: A module header's field naming the published method the module follows, with a link (HEADERS.md; principle 10).

**Method note**: A note in the kernel's `methods/` folder, which piece G16 creates, recording what was learnt from a source read under the method-only procedure, such as an LGPL library (29.20, rule 6; piece G16).

**Method-only procedure**: How an LGPL source may be read: for its method only, with nothing copied, recorded in a method note (29.20, rule 6).

**Minimiser** (also: shrinking): A tool that reduces a failing input to a smaller one that still fails, before it becomes a fixture: a fuzzer's minimiser, a property tool's shrinking, or, for operation logs, `geom-test` removing operations and inputs one at a time while the failure persists (28.16; piece G97).

**Miri**: The Rust interpreter that detects undefined behaviour; it runs the test suites weekly (28.8).

**Model-based test**: A test that drives the real code and a simple model of it with the same long random sequence of operations and requires them to agree at every step (28.7; pieces G81 and G111).

**Modified**: A history record: an output entity changed one-to-one from an input; it keeps its name (20.2; 20.3).

**Module header**: The `//!` lines that open a module: its job, guarantees, method with a link, cost, and provenance records, in HEADERS.md's grammar.

**Morton key**: An integer that interleaves the bits of a point's coordinates, so sorting by it keeps nearby points nearby (piece G55).

**MPL-2.0**: The Mozilla Public License 2.0, Spare Kit's licence: copyleft per file only (29).

**Multiprecision oracle**: A test that computes a function in much higher precision to check the kernel's error bound (piece G47).

**Mutation testing** (also: mutant, surviving mutant, survivor): Making small changes to the code and checking that a test fails; a change no test catches survives and marks a gap (28.8). Each pull request mutates its own diff; a weekly run mutates everything, split across machines (sharded).

## N

**`NameId`**: A persistent name: a 128-bit hash of a canonical encoding of how an entity was made (20.1).

**`NameScheme`** (also: NameScheme v1): The versioned hash function and canonical encoding that turn names into `NameId`s, with its text normalisation and Unicode version (20.1; owner action G78).

**NaN**: The floating-point "not a number". No public kernel function returns one, and none makes one from finite inputs (21).

**NaN-free scalar** (also: `Real`): `geom_num::real::Real`, a double that refuses NaN and the infinities at construction (21; piece G24).

**Native format** (also: SKG): The kernel's own file format: deterministic CBOR records with checksums, content IDs, chunks and versions, readable forever (24). SKG, Spare Kit Geometry, is its proposed name, with the extension `.skg` (24.1; owner action G77).

**`next_up`** (also: `next_down`): The next representable double above or below a value; outward rounding uses them (6.4).

**NFC** (also: NFC normalisation): Unicode Normalization Form C. Text hashed into a name must be in it (24.3; owner action G78).

**Nightly**: What CI runs once a night: property tests with fresh seeds, long fuzzing, outside oracles, all proofs and wall-clock timings (28.17). A pinned nightly compiler measures branch coverage only (28.8).

**NLib**: The NURBS layer of SMLib (Apache-2.0), followed as method with its notices kept (32.8; piece G56).

**`no_std`**: A Rust crate that uses only `core`, without the standard library: no files, threads, clocks or platform maths. Every kernel crate is `no_std` except `geom-pool` (27).

**Node** (also: Node.js): A JavaScript runtime; CI runs the WebAssembly tests under its WASI as well as under Wasmtime (21).

**Normal dependency**: A dependency used by a crate's own code, as opposed to a development dependency used only by its tests.

**Normal patch**: A surface patch of a surface's normal vectors, made exactly as a symbolic product of B-splines (12.3; piece G59).

**`NoSolution`**: A base error kind: the input is valid, but no result meets it (22).

**NOTICE**: The file holding the licence text and credit of every outside source the kernel ports code from, shipped with every binary (29.20, rule 4).

**`num-bigint`** (also: `num-rational`): Arbitrary-precision integer and rational crates (MIT or Apache-2.0), used as exact oracles in tests (28.6).

**Numerical Recipes**: A book whose code may never be used, from any source or from memory (29.2).

**NURBS**: Non-uniform rational B-splines: curves and surfaces given by control points, weights and knots (7; piece G56).

**NZTM** (also: NZTM2000): New Zealand Transverse Mercator, a map projection; an example of the projections frames link to (6.1).

## O

**One**: What a reference usually asks for: one target. A split target then gives `Ambiguous` (20.4).

**One-sided Jacobi SVD**: A singular value decomposition by plane rotations applied to one side of the matrix (piece G50).

**One-step degenerate**: An exactly degenerate input moved by one grid step, so a predicate's answer is barely non-zero (28.6).

**`OpContext`**: The context every operation receives: its `OpId` and `OpPath`, budgets, cancellation and precision; no randomness and no clock (10.1; piece G82).

**Open question** (also: OQ): A question a chapter leaves for an owner, listed in chapter 31.7; OQ-13.3 means chapter 13's open question 3.

**OpenCascade** (also: OCCT): An open-source B-rep kernel (LGPL); its history contract is adopted as a tested invariant and its tools run only out of process as oracles (20.3; 28.10).

**Operation**: A pure function that makes or changes shapes, with the shape `fn operation(ctx: &OpContext, input: &OpInput) -> Result<OpOutput, OpError>` (10.1).

**Operation contract**: What every operation keeps: pure, transactional, budgeted and cancellable, batch first, history and names, exactness stated, result checked (10.1; piece G82).

**Operation log**: The record of every operation applied to a model, with its version and inputs, from which the model replays (26.2; piece G96).

**Operation schema**: An operation described as data: inputs with units, outputs, roles, errors with their base kinds, exactness and an example (22; piece G100).

**Operation version**: Each operation's version number in the log; a changed result is a new version, and the old one keeps its behaviour (26.2).

**`OpError`**: The error every operation returns: a base kind, the operation's own detail, the operation, the entities, evidence, a message and remedies (22).

**`OpErrorDetail`**: The operation family's own typed error inside an `OpError`, versioned with its schema (22).

**`OpId`**: The permanent ID of one operation, stored in the operation log (20.1).

**`OpOutput`**: What an operation returns: new bodies, history, names, exactness and diagnostics (10.1).

**`OpPath`**: An `OpId` with sub-step labels, naming one step of one operation; a batch item's is the batch's plus its key (20.1; 10.1).

**Oracle**: An outside program or library whose answers are compared with the kernel's in tests; never the truth (29; 28.10).

**Oracle runner**: The CI tooling that runs oracles in containers built from pinned source and kept in a private registry, never linked or published, compares results and triages differences (piece G113).

**Ordered containers**: Maps and sets iterated in the order of stable keys, the only iterable containers in the kernel (8.2).

**Ordinal** (also: `Ordinal`): The n-th of a set of entities sharing a name, used only in `Weak` names, by the fixed order of 20.2.

**`orient2d`** (also: `orient3d`): The orientation predicates: which way three points turn in 2D, and which side of the plane through three points a fourth lies on in 3D (6.2).

**Out of process**: Run as a separate program that exchanges only files with the kernel, never linked, which is how copyleft tools may be used as oracles (29.20, rule 8).

**`OutOfMemory`**: A base error kind: the machine refused a large allocation, which the kernel reserves fallibly; never logged or cached (22; 23.10).

**Outward rounding**: Rounding a computed interval's lower bound down and upper bound up, so it always contains the exact result (6.4).

## P

**Patent register** (also: `PATENTS.md`): The record of every known patent and every family's patent search status (29.17; piece G115).

**Patent search**: A recorded search for patents that may cover a method, reviewed by an owner (29.17; owner actions G73 and G74).

**Pave block**: In a boolean, the piece of an edge between two neighbouring paves, the points where something touches it (11.4).

**Payload cap**: The largest payload one native record may hold, 64 MiB by the proposal (24.2).

**Pcurve**: A curve in a face's parameter space, the use of an edge on a curved face; derived from the edge's 3D curve and checked against it (8.1; 7).

**PENDING**: The lists of what waits for another piece: the map's folders not yet in the tree, and the scaffold check's dependencies on crates not yet built.

**Permissive**: Licences that let code be copied and changed if the notice is kept, such as MIT and Apache-2.0 (29).

**Perturbing allocator**: A test allocator that shifts and fragments memory, so any ordering by address shows (28.13; piece G19).

**Phase**: One of the work board's thirteen stages, each ending at a gate (30.2). Inside an operation, a phase is a step whose budget is checked at its end (23.10).

**Phase rule**: How budgets work inside an operation: each item runs with a cap of the remaining budget, the lowest-numbered failure wins, and the total is checked at the phase's end (23.10).

**Philox4x32-10**: The kernel's one random generator: a counter-based generator of ten rounds on four 32-bit words (Salmon and others, 2011; piece G32).

**Piece**: One pull request's worth of work on the board, checked out as the branch `piece/<ID>` (30.1).

**`PieceKey`**: What tells the pieces of a split apart in their names: the cutting faces and the side (20.2).

**Piegl-Tiller cache**: An offset surface's cached NURBS approximation, made by Piegl and Tiller's method and certified by interval bounds (piece G66).

**Pinned link**: A link to one commit, so what it shows can never change (29; HEADERS.md).

**Plane** (in `geom-num`): A plane through three grid points, with integer coefficients within bit budgets checked at construction: a normal of at most 83 bits and an offset of at most 125 (6.2).

**Platform maths**: The operating system's or compiler's `sin`, `cos`, `exp` and the rest, which differ between platforms; banned in the kernel (21).

**Playbook**: A step-by-step guide in AGENTS.md for adding one kind of thing to the kernel, proven by a dry run (26.3).

**Polynomial curve**: A curve kind given by polynomials in its parameter (7).

**Pool** (also: `Pool`): The trait that runs the parallel helper's tasks on threads; `geom-pool` implements it with rayon on native targets (23.4).

**Pool off**: Running benchmarks with no thread pool, so counts are single-threaded (piece G18).

**PORT** (also: PORT METHOD): Two of chapter 29.1's ways of taking something: PORT translates permissive code into our Rust, keeping its notice; PORT METHOD implements a published method in our own words, copying nothing.

**Precision**: The level an operation runs at: final, or a coarser preview the caller asked for (10.1).

**Predicate**: A yes, no or "exactly on it" question, such as which side of a plane a point is on, answered as a `Sign`. Topology is decided only by predicates, and every one is exact (6.2).

**Predicate generator**: `geom-gen`'s generator that turns a predicate's description into its filter, error constant, exact path and bit-range certificate (piece G27). Its output is committed, CI regenerates it bit for bit, and a hand-edited constant fails.

**Preview** (tier): See tier.

**Primitive**: A basic solid made from parameters: box, prism, cylinder, cone, sphere, torus (10.2; piece G101).

**Principles**: The thirteen rules at the top of `ARCHITECTURE.md` that every crate, operation and pull request keeps. AGENTS.md turns them into a checklist.

**Procedural kind**: A kind stored as its definition with an optional cached approximation and its error, such as an offset curve or an intersection curve (7).

**Proof**: A machine-checked argument that covers every input: Verus for integer code and data structures, Kani for bounded proofs, FPTaylor or Gappa for filter bounds (28.7).

**Property test**: A test that states a law and checks it on many generated inputs, shrinking a failure to the smallest input that still fails (28.2).

**proptest**: A Rust property-testing crate (MIT or Apache-2.0), used where its input strategies fit (28.2).

**Proven**: The label of a filter once its bound is machine-proven (28.6). Compare checked, not proven.

**`PROVENANCE.md`**: The ledger of every outside source the kernel took, generated from the headers' `Source:` and `Consulted:` records (29.20; piece G16).

**`PruneHistory`**: The logged operation that prunes history no held name needs, identically on every replica (24.4; piece G109).

**Pythagorean unit vector**: A direction (a/c, b/c) with whole numbers a² + b² = c², exactly of length one; rational rotations are built from them (piece G46).

## Q

**q0** (also: q1, q2): The three grid steps: 156.25 nm for buildings and sites, 6.25 nm for parts and boards, and 0.25 nm reserved (6.1).

**Quadrature**: Numerical integration (piece G51).

**Quarter turn**: A rotation by a multiple of 90 degrees, which keeps grid points on the grid exactly (7; piece G45).

**Query**: An operation that asks rather than changes: classify, measure, distance, exactness, validity (22; 26.1).

## R

**Radial fin order**: The order of the fins around an edge in a non-manifold body (8.1).

**Randomised-method rule**: The rule every kernel method that uses random numbers follows: one generator, the seed an input, one stream per consumer numbered by position, the draws part of the version, honest results, tests apart ([AGENTS.md](AGENTS.md), section 8).

**`RangeSource`**: The `no_std` trait a reader uses to read bytes at any offset, so a result never depends on how the bytes arrive (24.7). Compare `ByteSource`.

**Rank threshold**: The size below which a singular value counts as zero, named in the tolerance ladder (piece G50).

**Rational rotation**: A rotation whose matrix has rational entries, so it maps grid points to rational points exactly (Canny, Donald and Ressler; owner action G44; piece G46).

**Recipe** (also: recipe envelope): The stored operation behind a shape, in an envelope every reader knows: the owner, the schema version, the `OpPath`, inputs, an opaque payload and the bodies it produced (24.4).

**`RecipeOwnerMissing`**: The refusal to re-run a recipe whose owner (a program or plugin) is absent; its evaluated bodies still open (24.4).

**Record** (also: record envelope, record layer): One unit of the native format: a tag, flags, schema version, dependencies, payload and CRC-32C (24.2; piece G92).

**Recursion check**: A tool that lists every recursive cycle in kernel crates and fails CI on one without a stated depth bound (piece G13).

**Reference**: A stored pointer to an entity by name, with what it wants (`One`, `AllPieces` or `Carrier`), which resolves exactly, follows history, or fails with a reason (20.4).

**Reference implementation**: An independent, simple implementation used as truth in tests, such as the exact rational NURBS reference (28.9; piece G57).

**Reference machine** (also: reference runner): The dedicated machine on which benchmarks are timed nightly against the kernel's shares of Spare Construct's targets (23.1; owner action G72; piece G107).

**Reference memo store**: A cache the caller creates and owns, bounded by bytes, keyed by `CacheKey` (23.8; piece G105).

**Reference mode**: A build with caches, filters and pools removed; CI runs the replay corpus both ways and blocks on any difference (23.13; piece G106).

**Refinement**: Converting coordinates to a finer grid step by multiplying by 25, which loses nothing (6.1; piece G22).

**Region**: A connected volume of a body, solid or void; every body has one infinite void region outside it (8.1).

**Registry** (also: `REGISTRY.md`): The native format's list of every record type, its schema versions and its CDDL (24.2). In `geom-gen`, the list of every generator.

**Remedy**: What an error says would make the operation succeed, such as "radius at most 4.2 mm here" (22).

**Replay corpus**: Every fixture and a seeded operation log per family, run on every leg of the determinism matrix (28.13).

**Replica**: Another copy of a model, such as another user's in a shared session, which must reach the same state from the same log (26.2).

**Resolution** (also: `Resolution`): A reference's outcome: `Exact`, `FollowedHistory`, `Ambiguous`, `WeakChanged`, `Deleted`, `HistoryPruned` or `Unresolved` (20.4).

**Resolution floor**: A proposed rung of the tolerance ladder: one grid step, the box size below which subdivision stops and undecided boxes are flagged (6.5).

**Revit**: A proprietary building-modelling program. The kernel copies nothing from it; exports from a licensed copy may serve as truth data for tests, on terms chapter 32.10 leaves to an owner (28.11; piece G10).

**`robust`**: A crate of Shewchuk's adaptive predicates (MIT or Apache-2.0), used as a second method to check the kernel's (28.6).

**Robustness scoreboard**: The share of a fixed, seeded set of generated designs in each operation family that succeed. A new operation version may not lower it, except where an old success is shown to be wrong; that case becomes an honest error and a fixture (28.5; piece G116).

**Rocq**: The proof assistant (formerly Coq) that checks Gappa's proofs (28.6).

**Role**: The name an operation gives each entity it makes, such as `cap.start` or `exterior`, part of the operation's contract (20.2).

**Rounded input**: A predicate input whose `f64` copy is rounded from a wider integer; the generator accounts for its rounding in the filter's bound (piece G27).

**Ruled surface**: A surface swept by a straight line between two curves; a surface kind (7).

**`rust-toolchain.toml`**: The file that pins the Rust compiler for the workspace (Rust 1.99.0); the toolchain guard watches it (28.13).

## S

**Salvage read**: A read that keeps every intact record of a damaged file and names the damaged ones (24.2).

**Scaffold**: A crate that exists with its header and no code yet, waiting for the pieces that fill it (piece G09); its header's `Status:` line names them.

**`scan_exclusive`**: A parallel-helper function giving the exclusive prefix sums of integers (23.4).

**Seed**: The number that starts a random sequence. Test seeds are fixed or printed and replayable with `SPAREKIT_SEED`; a randomised method's seed is an explicit, logged input (28.17; AGENTS.md, section 8).

**Seeded violation**: A deliberately broken input or probe that shows a check really fails on what it claims to catch (piece G12).

**Segment crossing**: The 2D predicate deciding whether and how two segments cross, on grid points (6.2; piece G29).

**Sequence fuzzing**: Fuzzing with whole generated operation logs rather than single calls, with the checker, the laws and replay as oracles (28.5; piece G116).

**Serial executor**: The parallel helper's executor with no pool, which gives the same answer as any pool (23.4).

**Session attestation**: A statement in the pull-request template that the session made the change within the legal rules, beside the table of outside sources; its wording is piece G16's.

**SHA-256**: A cryptographic hash (FIPS 180-4): every golden digest, content ID and test digest uses it.

**Sheet**: A body kind: open surfaces (8.1).

**Shell**: A connected set of faces bounding a region (8.1).

**Shipped-checker leg**: A run of the replay corpus with the checker configured as it ships, not the full test configuration (28.4; piece G98).

**Short-edge warning**: A rung of the tolerance ladder: an edge shorter than 3,200 grid steps gives a warning, never an error (6.5).

**Sign** (also: `Sign`): A predicate's answer: `Negative`, `Zero` or `Positive`, never a number (6.2).

**Signature**: In a reference, the stored description of what it pointed at, used only to suggest a repair in the interface, never to resolve the reference (20.4). In differential testing, the key a difference from an oracle is grouped by: operation, oracle, kind of difference and input family; the nightly run fails only on a new signature (28.10).

**Simulation of Simplicity**: Edelsbrunner and Mücke's symbolic perturbation, which breaks ties as if inputs were moved by infinitesimals in a fixed order; the kernel orders by persistent keys, never addresses (6.2; piece G30).

**`sinpi`** (also: `cospi`): sin(πx) and cos(πx), with angles in half-turns, correctly rounded; the kernel's proven angle functions (6.4).

**`sort_stable`**: A parallel-helper sort by a total key, one answer whatever the chunks (23.4).

**Source record** (also: `Source:`): A header line crediting code ported from an outside source, with its licence, a link pinned to a full commit, the origin's copyright lines and what changed for Spare Kit (HEADERS.md; 29.20, rule 4).

**Spare Construct**: Spareware's program for buildings, survey and civil work, whose performance targets the kernel's shares come from (23.1).

**SPDX identifier** (also: `SPDX-License-Identifier:`): A licence's standard short name, such as `BSD-3-Clause`, and the header line that states a file's licence (29).

**`SPEC.md`**: The native format's written specification, with the registry and the CDDL (24.6; piece G93).

**Spike**: A small piece that tries a method to decide whether it works before the board depends on it (pieces G14, G35 and G36).

**Spiral kinds**: The clothoid and IFC 4.3's other transition curves, for road and rail alignments (7; piece G63).

**Split**: A history record: one input became several outputs (20.3). Also a form of `TopoName`, named by the cutting faces and the side.

**Stable** (tier): See tier.

**Standard bar**: What every code piece meets besides its own "done when": tests, a benchmark, determinism, readable headers and safe Rust (30.1).

**Statics ban**: The merge gate's rule that no crate that ships names a `static`, a thread-local or a lazy global, test modules included (28.8; piece G12).

**Stream** (also: `StreamId`, `StreamRange`): In `geom_num::rng`, one of a seed's 2^64 independent sequences of random numbers, its id, and a run of ids handed to one randomised method (piece G32).

**Surface kind**: One variant of the closed enum of surfaces: plane, cylinder, cone, sphere, torus, revolution, extrusion, ruled, sweep, offset, blend, NURBS and subdivision limit surface (7).

**Surface of revolution**: A surface swept by turning a curve about an axis; a surface kind (7).

**Symbolic perturbation**: Breaking a predicate's exact tie as if the inputs had moved infinitesimally, so every sign is non-zero where an algorithm needs it (6.2). See Simulation of Simplicity.

## T

**Target**: A platform the kernel is built for: x86-64 and aarch64 Linux, Windows and macOS, and 32-bit WebAssembly (21). See also `wasm32`.

**`target-cpu=native`**: A build that uses every instruction the build machine's processor has; the determinism matrix includes it (21).

**Test vector**: A fixed input and its expected output (piece G19).

**Three-plane point** (also: TPI, `Tpi`): An implicit point where three planes meet, with an `I256` weight and `I320` coordinates (6.3; piece G29).

**Tier**: The stability promise of an interface: Stable (semantic versioning, read forever), Preview (may change, graduate or drop within 12 months) or Internal (every `geom-*` crate) (25).

**Ties to even** (also: round half to even, half-even rounding): Rounding a value exactly halfway between two candidates to the even one; the kernel's rule for commits and conversions, and for parsing decimal survey text (6.1; 6.3; piece G48).

**Time budget**: A deadline, which the caller's cancellation enforces; never a replayable outcome (23.10; owner action G71).

**Tolerance ladder**: The one fixed set of tolerances, each derived from the grid step: commit rounding, ε_fit, ε_ang, ε_heal and the short-edge warning (6.5; `geom_num::ladder`).

**`ToleranceExceeded`**: A base error kind, only for Tolerant data: an operation would need a larger tolerance than the body carries (22). `geom-num`'s `NumError::ToleranceExceeded` instead reports a certified bound over its ceiling and maps to `ExactnessNotAccepted`.

**Tolerant**: The import state of a body whose edges do not quite meet, carrying per-entity tolerances up to ε_heal; no operation ever raises a tolerance (6.5).

**Toolchain guard**: The rule that any change to the compiler pin, `Cargo.lock` or vendored numeric code runs every golden across the matrix and must change no hash (28.13; piece G20).

**`TopoName`**: How an operation names what it makes: `Created`, `Split`, `Merged`, `Bounded` or `Weak` (20.2).

**Total order**: An order in which any two values compare one way; the kernel's helpers give doubles one, with named tie rules, in place of `f64::max` and `min` (21; piece G24).

**Toy predicate** (also: toy operation): A predicate, or operation, made only to prove that a playbook in AGENTS.md is enough, on a throwaway branch that is never merged (pieces G17 and G117).

**Transactional**: Nothing is published until the result passes the checker; a failed or cancelled operation changes nothing (10.1).

**Transform** (also: typed transform): A move, rotation, mirror or scale, typed as exact, rational or certified, so a change of kind (a circle scaled into an ellipse) is explicit (7; piece G45).

**Translation law**: Moving every input point by one grid vector does not change a predicate's answer (28.6).

**Triage**: Sorting a difference from an oracle into one of four outcomes, each kept as a fixture: our bug, their bug, both within bounds, or undecided (28.10).

**Trimmed curve**: A curve kind: part of another curve between two parameters (7).

**Tripwire** (also: debug tripwire): A check in debug builds that stops at once on a value that must never occur, such as NaN. The board asks for them on the NaN-free scalar; where to aim them is open, since behind that scalar's one constructor a tripwire could never fire (21; 31.7; piece G24).

**Trust on open**: The program states whether what it opens is Known or Unknown; unknown bodies are checked in full, lazily, before use (24.7; piece G108).

**Twice-area vector** (also: six-times volume): For a flat face whose vertices are grid points, the sum of the cross products of consecutive vertices: twice the face's area along its normal, an exact integer vector. Six times a flat solid's volume is likewise a sum of integer determinants. Both fit 256 bits, so neither is ever rounded (19.2; 23.4; piece G102).

**`TwoSum`** (also: `TwoProduct`): Error-free transformations: the rounded sum or product of two doubles and its exact error (piece G25).

**Typed array**: A CBOR array of raw little-endian numbers, used for coordinate columns (RFC 8746; 24.1).

## U

**ulp**: A unit in the last place, the gap between neighbouring doubles at a value; maths functions' error bounds are stated in ulps.

**`Undecided`**: A base error kind: a decision can be neither made exactly nor certified at the data's resolution (22).

**`UndecidedOrder`**: The error when the fixed ordinal order of 20.2 cannot decide between two entities; it fails rather than guesses (20.2; piece G87).

**Unit ratio**: A unit's exact size in grid steps, such as 6,400 steps per millimetre at q0 (6.1; piece G22).

**Unit test**: A test of one function on chosen inputs (28.2).

**UNMEASURED**: `geom-bench`'s list of public `geom-num` functions with no benchmark of their own, each with the reason (principle 12).

**Unproven maths** (also: `UnprovenMaths`): A maths function without a proven error bound, kept apart in `geom_num::math::unproven`; an enclosure that needs one is refused with `UnprovenMaths` (6.4; piece G31).

**`Unresolved`** (also: `UnresolvedRef`): A reference's outcome when none of the others applies, with a reason; shown as model state, the same on every machine (20.4; piece G89).

**`Unsupported`**: A base error kind: no route exists yet for this input (22).

**Unwinding**: Ending a computation by a panic that climbs the stack. The kernel never unwinds: WebAssembly builds cannot catch it (10.1).

## V

**Validation stamp**: The record in a body's index of which checker version and groups it passed, under which edition (24.7).

**Validity checker** (also: checker): `geom-check`, the checks every result passes before an operation returns it, in four groups (8.3; pieces G84 and G85). Passing it is every operation's postcondition.

**Value schema**: The schema of an attribute's value (8.4).

**Vendored**: Third-party code kept in the repository; vendored numeric code is under the toolchain guard (piece G20).

**Verdict**: An item's outcome in a phase or batch; it never depends on the other items (piece G23).

**Version**: See operation version, edition, and native format.

**Vertex**: A point of the topology (8.1).

**Verus**: A tool that proves Rust code correct with annotations; it proves the wide integers, the predicate formulas and the topology model (28.7).

**Verus twin**: A Verus-annotated copy of a predicate's integer path, emitted by the generator, which CI proves and checks token for token against the shipped code (piece G36). The alternative writes the code inside Verus's `verus!` macro, whose ghost code is erased at compile time.

## W

**Wall clock**: Time measured by a clock. It varies between runs, so it never gates a pull request; nightly timings on the reference runner report against the kernel's shares (23.13).

**`wasm32`** (also: `wasm32-wasip1`, `wasm32-unknown-unknown`, `wasm32v1-none`, WebAssembly): 32-bit WebAssembly targets: with the WASI system interface (tests run under wasmtime and Node), for browsers, and with no `std` at all, which proves `no_std` (21; 28.8).

**Wasmtime**: A WebAssembly runtime; CI runs the WebAssembly tests and the fuel gate in it (21; piece G18).

**Weak** (also: weak name): The `TopoName` form for names that would otherwise collide, told apart by an ordinal; a name is weak if any part of it is. A reference to one resolves with a flag, or `WeakChanged` (20.2; 20.4).

**`WeakChanged`**: A reference's outcome when a weak name is gone while its base still names entities, so a person or AI confirms the choice (20.4).

**Weight**: The homogeneous coordinate w of an implicit point, by which its integer coordinates are divided (6.3).

**Wide-arithmetic build**: A WebAssembly build using the proposed wide-arithmetic instructions for 64 × 64 → 128-bit products; one of three variants piece G42 measures (23.3).

**Wide integers** (also: `Wide`): `geom_num::wide`'s fixed-width integers of several 64-bit limbs, with every step checked (6.2; piece G26).

**Width-aware limbs**: Generated exact paths that use only as many 64-bit limbs as each step's certificate needs (piece G42).

**Wire**: A body kind: curves only (8.1).

**Work list**: The explicit list of boxes the root-isolation engine still has to examine, used in place of recursion (12.3; piece G53).

**Work units** (also: `WorkMeter`): The deterministic count of an operation's work, charged only for what fast paths cannot change; the work meter counts them and polls cancellation (23.10; piece G23).

**Workspace**: The Cargo workspace at the repository root that builds every crate together, with one `Cargo.lock`.

## X

**`x86-64`** (also: aarch64): The 64-bit Intel and AMD, and 64-bit Arm, processor families; native targets of the determinism matrix (21).
