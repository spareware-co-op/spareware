# geom-fuzz: the kernel's fuzz targets and bounded proofs

`geom-fuzz` is a development crate of the Spare Kit geometry kernel (chapter 27). It holds the
fuzz targets of chapter 28.5 and the bounded Kani proofs of chapter 28.7 that run on the same
harnesses. Nothing here ships, and no kernel crate depends on it.

This page is the layout every target follows, chosen by piece G14 (the fuzz toolchain spike), and
how each leg runs a target, on your machine and in CI. Every leg is an ordinary test in the suite,
run on every pull request the target's code can be reached by: a harness is fuzzed by seeded random
and hostile cases sized to seconds, and every input a run ever found is saved and replayed, the way
solidcore tests (decision 32.52). There is no libFuzzer, no campaign and no run measured in hours. The first target built to it is
`orient2d` (`src/orient2d.rs`): `geom-num`'s orient2d, its exact path and its `f64` filter must
agree with the exact determinant of orient2d's definition, computed from scratch. The others:
`predicates` (`src/predicates.rs`, G11), every exact predicate answers and swapping two points
flips its sign; `wide` (`src/wide.rs`, G26), the wide integers agree with `num-bigint` on
every operation at every width, `I128` to the reserved 768 bits; `cbor` (`src/cbor.rs`, G91),
`geom-format`'s strict CBOR decoder and its text form never panic, refuse inside their input and
convert both ways bit for bit; `cache` (`src/cache.rs`, G105), `geom-format`'s memo store keeps to
its bound, forgets the entry used longest ago first and answers only where the caller's budget
covers the stored run; and, for Gate 2 (decision 32.46:
a target per phase-2 code piece), `curves` (`src/curves.rs`) and `surfaces` (`src/surfaces.rs`),
every curve and surface kind built from any bytes keeps the kind contract kit
(`geom_curve::contract`, `geom_surface::contract`) under any typed transform, and after a budget
stop still keeps the laws that need no conversion (`check_without_conversions`, under a meter of
its own); `curve_offsets` (`src/curve_offsets.rs`) and `surface_offsets`
(`src/surface_offsets.rs`), the same for the offset kinds alone, which read the same bytes with
the kind made an offset: offsets cost nearly all of what the two kinds' targets spend, so
`curves` and `surfaces` pass them unbuilt and the other kinds get the time; `arenas` (`src/arenas.rs`,
G80), `geom-topo` publishes exactly the arenas its rules allow, and a body's versions never see
each other's edits; `attributes` (`src/attributes.rs`, G90), `geom-name`'s attribute step gives
each output exactly what section 8.4's table gives; `batch` (`src/batch.rs`, G104), `geom-op`'s
batch driver gives each item its own result, and the many-point calls give the single calls' bits;
`make` (`src/make.rs`, G101), the primitives and transforms return only what the full checker
passes, keep their laws, and refuse with a typed error; `sequence` (`src/sequence.rs`, G116),
whole operation logs replay to the same hash and keep every law at every step. For Phase 5's
intersections: `intersect` (`src/intersect.rs`, G158), an intersection's items keep their
invariants against an all-pairs oracle, and a pair asked either way round makes one call; `quadric`
(`src/quadric.rs`, G159), every exact decision on a line and a quadric agrees with big integers,
and a Sturm chain counts the roots a polynomial was built from; `project` (`src/project.rs`, G162),
a nearest point is never beaten by a sample of the kind; `crossings` (`src/crossings.rs`, G163),
every point of any two curves lies on both, and two cubic graphs that change sides have an item
between them; `piercing` (`src/piercing.rs`, G164), every point of a curve and a surface lies on
both; `natural` (`src/natural.rs`, G165), every curve two natural quadrics are said to share lies
on each by its own equation; `pencils` (`src/pencils.rs`, G166), what a pencil says two curved
quadrics share is so, in either order and wherever they stand; `reductions`
(`src/reductions.rs`, G167), every curve a torus's special position gives lies on it by its own
equation; `level` (`src/level.rs`, G168), a level curve misses nothing a dense grid can see; and
`general` (`src/general.rs`, G169, G170), subdivision leaves no crossing outside its settled or
flagged pairs, and marching stays on both surfaces. For Gate 3 (24.7:
every reader is a fuzz target): `encoder` (`src/encoder.rs`, G86), `geom-op`'s canonical encoder
refuses without a trace and writes only canonical items; `records` (`src/records.rs`, G92), the
record layer reads the same twice and however its reads split, refuses inside its input and
accepts only canonical records and files; `bodies` (`src/bodies.rs`, G94), body records read the
same twice and give a body that writes and reads to itself; `names` (`src/names.rs`, G95) and
`operations` (`src/operations.rs`, G96), the names, history, recipe, attributes and operation
records accept only the one encoding of what they read; and `open` (`src/open.rs`, G108), a
container comes back from a file to the same blobs and trust on open keeps damage with the body
it is in. Their corpora start from the writers' output for the fixtures of `geom-format`'s own
tests. `p21` (`src/p21.rs`, G415), `geom-p21`'s Part 21 reader never panics, refuses inside its
input, reads the same however its bytes arrive, and what it accepts writes to a fixed point. For
the Gate 3 pieces that `make` and `sequence` do not reach (30.1: a fuzz target per
code piece): `euler` (`src/euler.rs`, G81), the Euler operators change the counts by their table,
keep Euler's formula, refuse dead rows without a trace and publish back to themselves; `naming`
(`src/naming.rs`, G87), names get the one ID their bytes hash to, the name
graph keeps each once and knows which are weak, and the duplicate detector, the reference pick and
the ordinal order agree with oracles written from 20.2; `references` (`src/references.rs`, G89),
reference resolution gives 20.4's outcome on any history from `geom-test`'s kit; `prune`
(`src/prune.rs`, G109), `PruneHistory` keeps every held reference and charges exactly its
schedule, and the pruned record reads back and reads only its one encoding; and `contract`
(`src/contract.rs`, G82), the operation contract's errors keep their detail's kind and chapter 22's
remedies and write text that names what they hold, and every schema the facade lists names details
the contract has, whose MCP tool descriptions parse and round-trip. For Gate 4's pieces (30.1: a
fuzz target per code piece): `planes` (`src/planes.rs`, G123, and through it G124's step ladder
and G125's convex faces), `geom-poly`'s canonical planes and arrangement predicates give one entry
to one plane and one order to any points, rays or flaps; `exact` (`src/exact.rs`, G146),
`geom-mesh`'s exact mesh refuses what is not a mesh and gives one mesh in any input order; `cdt`
(`src/cdt.rs`, G153), the triangulation is a true constrained Delaunay triangulation and one answer
in any input order; `solids` (`src/solids.rs`, G147 to G152, and through the mesh boolean
G126 to G129's arrangement code), `geom-mesh`'s operations on small solids agree with counted
cells; `booleans` (`src/booleans.rs`, G130 to G133, G137 to G139 and G144, with G129's
`rebuild.rs`), `geom-bool`'s boolean pipeline on small solids agrees with counted cells: every
selection of one arrangement, a half-space and a wire kept inside the boxes, rooms and their
leaks, batch cuts and a re-cut from the last cut's cache, and a local step on its full and fast
paths, every result passing the full checker and the second checker, which agree; and `commits`
(`src/commits.rs`, G142), `geom-poly`'s plane commit holds its edge exactly, stays in budget,
proves a deviation that the exact distance at no corner of its extent beats (decided in big
integers), maps a class exactly under the grid's 48 signed permutations, refuses a flip exactly
when the ideal sign is certain and differs, and refuses NaN, infinite and degenerate input by
type. `make` and `sequence` reach G134's checker groups. Which lines of each piece they reach, and
the pieces no target reaches yet (G143 and G145, which the boolean pipeline does not call), are in
`projects/kit/notes/G156-piece-map.md`. `src/adversarial.rs`
holds the generators aimed at the hard cases, and `src/robustness.rs` counts how many plain designs
succeed per family: `src/robustness.txt` records it, and a count may not fall.

## One law, three legs

A target is one law and its oracle, written once as a plain function, and run by every leg:

| Leg | What it does | Where it runs | Test it runs |
| --- | --- | --- | --- |
| bolero's test mode | Replays the target's stored corpus and crash files, then tries a few thousand random inputs | `cargo test`, every operating system (kernel.yml, kernel-dev.yml) | `<target>_fuzz` |
| The seeded leg | Runs the harness on a fixed set of byte strings from a printed seed, half of them hostile (extreme words, repeats and near-repeats, progressions, fills), sized for about two seconds of an optimised build (at most ten for the slowest targets) and a tenth of that with debug assertions | `cargo test`, every target, WebAssembly included | `<target>_seeded` |
| Kani | Proves the law for every case of a bounded domain | Linux and macOS; every pull request that can affect it (kernel-fuzz.yml) | `<target>_proof` |

The legs drive the same function on the same bytes, so a saved file replays in each, and
`tests/attack_g14.rs` replays every saved corpus and crash file of every target on every run.

## The layout of a target

Each target `<target>` has a module `src/<target>.rs` and a test file `tests/<target>.rs`.

The module holds, with a header in the grammar of `projects/engines/geometry/HEADERS.md`:

- `Case`: what one input decodes to.
- `check(&Case) -> Result<(), Finding>`: the law and its oracle. It never panics by itself; a
  broken law is a `Finding` that names the law, the input and the values. Only the kernel code
  under test can panic, and a panic fails every leg.
- `check_with(...)`: `check` on functions handed in, so unit tests can feed it broken versions
  and show it catches each one (28.4, fault injection: a check that cannot fail proves nothing).
- `decode(&[u8]) -> (Case, usize)`: any bytes to a valid case, aimed at the hard cases with
  `geom-test`'s grid sampler, reading at most `MAX_INPUT_BYTES`, a bound derived on the constant
  and reached by a unit test.
- `harness(&[u8]) -> Result<(), Finding>`: `decode`, then `check`. This is the function every
  leg drives (the crate's `Harness` type).
- For a target with a proof: `ProofDraw`, `proof_case(bound, &ProofDraw) -> Option<Case>` and
  `PROOF_BOUND`, the bounded domain the proof covers. The module header says exactly what is
  proven and what is only tested (28.7). A proof too slow as one solve is split by claim
  (orient2d's `Claim`, `CLAIMS` and `check_claim`: one harness a claim, which together prove what
  `check` checks).

The test file holds three tests (a proof split by claim has one `<target>_proof_<claim>` test
a claim in place of `<target>_proof`):

```rust
#[test]
fn <target>_fuzz() {
    bolero::check!()
        .with_iterations(ITERATIONS)
        .with_max_len(MAX_LEN)
        .for_each(|data: &[u8]| harness(data));
}

#[test]
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(kani, kani::solver(cadical))]
#[cfg_attr(kani, kani::unwind(10))]
fn <target>_proof() {
    bolero::check!()
        .with_generator(proof_draws(PROOF_BOUND))
        .for_each(|draw| holds(prove(PROOF_BOUND, draw)));
}

#[test]
fn <target>_seeded() -> Result<(), seeded::Failure> {
    seeded::run("<target> fuzz target, seeded", Seed::DEFAULT, SEEDED_CASES, MAX_LEN, harness)
}
```

Rules:

- A bolero test is a target's fuzz test exactly when its name ends in `_fuzz`.
- The fuzz test takes raw bytes and hands them to `harness`, never a typed generator, so every
  saved corpus and crash file is the harness's own input.
- `check!` is written in the `#[test]` function itself, never in a helper it calls. bolero names
  a target after the function `check!` is written in (`bolero-engine`'s `TargetLocation`), and a
  tool that selects a target (Kani's `--harness`) finds it by the test of that name, so a target
  in a helper cannot be selected ("no targets matched"). `tests/attack_g14.rs` refuses one.
- No bolero test is `#[should_panic]`. A tool that selects bolero tests by name runs one of a
  binary's bolero tests, the first it listed (in no fixed order), with `CARGO_BOLERO_SELECT` set,
  and there `check!` returns without running its body (`bolero-engine`'s
  `TargetLocation::should_run`). A `#[should_panic]` test fails in that mode. A test that shows bolero
  fails catches the panic itself and accepts a body that never ran only in that mode
  (`tests/predicates.rs`, `bolero_fails_on_a_finding`). `tests/attack_g14.rs` refuses one.
- The proof test drives `check` itself, with a typed bolero generator of the proof's domain.
  Under Kani every generated value is symbolic, so the proof covers the whole domain; under
  `cargo test` the same test is a property test over that domain, on every operating system.
  Aiming at hard cases is pointless there: Kani covers every case.
- The proof test's body panics on a broken law, through `holds`, rather than returning the
  `Finding`. Under Kani, bolero's engine turns a returned error into its `PanicError`, whose
  constructor may capture a backtrace (`bolero-engine`'s `panic.rs`), and Kani would have to
  explore that capture; a panic is a failed property and nothing more, and Kani's `panic!`
  formats nothing. Under `cargo test` bolero catches the panic and prints the finding with the
  failing draw.
- A finding is made with `Finding::new(law, || detail)`, so its detail is written only when it is
  reported, and never under Kani, which traces every value itself; formatting symbolic values
  would only enlarge the problem Kani solves.
- Each Kani bound is measured, not guessed: trial harnesses ran on the hosted runner until each
  bound was chosen, then were removed. For orient2d (PR #209) the proof's bound is 1 (bound 0
  took 3½ minutes, 1 took 19, and 3, 7 and 15 did not finish within the job's hour) and the fuzz
  target was proven on every byte string of at most 4 bytes (8 minutes; 8, 16 and 24 bytes did not
  finish). Once G28 routed orient2d to its exact paths on 64-bit targets, 4 bytes no longer
  finished within the hour, so the fuzz target's bound is 2. After G37 rewrote the wide integers
  the proof reaches, the bound-1 proof took 47 and then 75 minutes with kissat as Kani's SAT
  solver (runs 40291338626 and 40308963161: the hosted runners' speed varies), and CaDiCaL ran
  past three hours, so the proof uses kissat and its job has 120 minutes. A new
  target, or a change to the code a proof reaches, repeats the trials before its bounds are set.
- The proof test sets Kani's unwind bound above the longest loop a passing case runs (10 for
  orient2d: six coordinates), so a loop Kani cannot bound is cut rather than unwound forever.
  Kani's unwinding assertions, on by default, fail the proof if a cut loop can be reached, so
  the bound hides nothing.
- Corpus and crash files live in `tests/__fuzz__/<target>/corpus` and `tests/__fuzz__/<target>/crashes`
  (bolero names the folder after the test, without `_fuzz`). `cargo test` replays both on every
  run, so a case found by any leg and saved there stays a test forever (28.16). A seeded failure
  prints its bytes in hexadecimal; save them as a file of the target's `crashes` folder (by custom
  named after their SHA-1). Corpora only grow.
- A new target is registered in `.github/workflows/kernel-fuzz.yml`: its `_proof` test in the
  Kani matrix, and every crate it reaches in the path filters. `tests/attack_g14.rs` fails until
  it is. On a pull request the job `select` runs a target's proofs only when the change can affect
  it: when a crate its code names (or a crate that one builds) changed, or its own module, tests,
  corpus or crashes did (`geom_test::fuzz_scope`, which reads the crates a target uses from its
  `use` lines and paths, so nothing more is registered).

## Running a target

From the repository root.

**bolero's test mode and the seeded leg**, on any operating system with the workspace's compiler:

```text
cargo test -p geom-fuzz --test orient2d
```

A failure in test mode prints `BOLERO_RANDOM_SEED=<seed>`; set that variable to replay it. The
seeded leg prints `seed: <hex>`; set `SPAREKIT_SEED=<hex>` to replay it.

**Kani**, on Linux or macOS:

```text
cargo install kani-verifier --version 0.68.0 --locked
cargo kani setup
cargo kani --tests -p geom-fuzz --harness orient2d_proof_exact_path --exact
```

(and the filter's claim in steps: `orient2d_proof_filter_exact_stand_in`,
`orient2d_proof_filter_predicate_stand_in` and `orient2d_proof_filter_answer`; and the
predicate's claim in parts: `orient2d_proof_predicate_cover` and
`orient2d_proof_predicate_part_0` to `_part_63`).

`cfg(kani)`, which marks the proof tests, is declared to the compiler by `build.rs`
(`cargo::rustc-check-cfg`), not in the manifest's `[lints]` table, which repeats `geom-num`'s
exactly.

Two traps, found by the first proof. First, Kani 0.68.0 cannot compile a read of an enum's niche when the
niche starts above 2^64: `codegen_get_discriminant` in
`kani-compiler/src/codegen_cprover_gotoc/codegen/rvalue.rs` unwraps `u64::try_from(niche_start)`,
unchanged on Kani's main branch on 2026-10-25. rustc makes such a niche when it widens an enum's
tag to 128 bits, which it does for an enum whose data starts with an `i128` or `u128`; an `Option`
or `Result` around that enum then marks its other variant with the tag value 2^128 − 1.
`geom_num::NumError` was such an enum, so the proof failed to compile in `P2::new`; under Kani its tag
is now one byte (`#[cfg_attr(kani, repr(u8))]`, explained on the type; built normally its layout
is rustc's own, because a one-byte tag in every build made `interval::new` 12.7% dearer). Any other kernel enum that carries an `i128` or
`u128` and is wrapped in an `Option` or `Result` needs the same until Kani is fixed; the Kani job
fails to compile, never passes, when one is missed. Second, a proof whose body returns its
finding, or formats one, never finishes: the first orient2d proofs, the smallest domain among
them, were still in CBMC's symbolic execution when the job's hour ran out, in bolero's error
path (its backtrace capture) and in the formatting of the finding's detail. Hence `holds` and
`Finding::new`, above.

On Windows only the first runs. Kani needs Linux or macOS; CI runs it on every pull request that
touches the code a proof reaches.

## What the orient2d proof covers

`orient2d_proof_predicate`, `orient2d_proof_exact_path` and `orient2d_proof_filter` drive
`check_claim`, one claim each, which together are `check`, over every case `proof_case` gives
with bound `PROOF_BOUND`. Kani proves the predicate's claim in 64 parts, one job each
(`orient2d_proof_predicate_part_<k>`: the draws whose six coordinates have the signs k names,
`predicate_part`), with `orient2d_proof_predicate_cover` proving that every draw is in one of
them; as one harness it took 35 to 46 minutes on the hosted runner, and a part takes 4 to 9.
Kani proves the filter's claim one step of `check_with` at a time, one job each, after the one
reference it takes first (`check_filter_step`): the exact path's stand-in
(`orient2d_proof_filter_exact_stand_in`), the predicate's stand-in
(`orient2d_proof_filter_predicate_stand_in`) and the filter's answer
(`orient2d_proof_filter_answer`); as one harness it took 570 to 943 seconds on the hosted
runner, most of it in holding the reference's three copies equal (decision 32.83).
The domain:
every one of the six coordinates within `PROOF_BOUND` grid steps of 0 or of an edge
±(2^40 − 1) of the frame, in any combination. Kani proves, for every such case, that orient2d
answers with the reference's sign, that the exact path returns the reference's value, that the
filter declines or gives the reference's sign, and that nothing on the way panics or overflows.
Near the edges the filter's products round, so the proof covers the filter's error bound as well
as exact arithmetic. Everywhere else the claims are tested, not proven. Wider Kani proofs of the
numbers layer are piece G41's.

## Sources

Read for this layout, every one under a licence chapter 29.2 lets us read; no code was copied:

- bolero 0.13 (MIT), at commit `7642943b6c1f8a9b5e824dd1cd29b7c8e56068b8` of
  <https://github.com/camshaft/bolero>; from the published crates, `bolero-kani` 0.13.2 and
  `bolero-generator` 0.13.6's `kani.rs` (how a generator becomes symbolic) and `bolero-engine`
  0.13.6's `target_location.rs` (test names and corpus folders).
- solidcore (Apache-2.0), at commit `a3a8244566081e0777284dc182f77a91bb1a456d` of
  <https://github.com/tangency-labs/solidcore>: `crates/solidcore/tests/support/panic_fuzz.rs`,
  whose seeded cases mixed with a fixed list of hostile values the seeded leg follows, and whose
  `HOSTILE` list it ports (`src/seeded.rs`).
- Kani 0.68.0 (MIT OR Apache-2.0), `kani-driver/src/call_cargo.rs`, for how `cargo kani --tests`
  builds a test target, `kani-compiler/src/codegen_cprover_gotoc/codegen/rvalue.rs` and
  `statement.rs` at tag `kani-0.68.0`, for the niche trap above, and the kani-github-action's
  `src/install-kani.sh` at commit `f838096619a707b0f6b2118cf435eaccfa33e51f`, for how CI installs
  it.
- `bolero-engine` 0.13.6's `target_location.rs`, for how a target is named and selected (the rule
  that `check!` sits in the test itself).
- The Cargo book's "Build Scripts" chapter, `cargo::rustc-check-cfg`, for `build.rs`.
