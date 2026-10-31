//! Exact geometric predicates on grid points: orientation in 2D and 3D, in-circle and in-sphere.
//!
//! Every topological decision in the kernel is one of these questions (chapter 6.2): which way do
//! three points turn, which side of a plane is a point on, is a point inside a circle or a sphere.
//! Each answer is a [`Sign`], never a number, or an error that no input can cause (below).
//!
//! Guarantees:
//!
//! - **Exact for every input.** No rounding ever decides a sign, and `Zero` means exactly
//!   degenerate. Each predicate documents its degenerate cases, including which sign a degenerate
//!   but non-zero answer has.
//! - **The same answer in every build profile; no overflow, no panic.** Outside its tests this
//!   module uses no native `+`, `-` or `*` on a fixed-width integer, and no cast: every operation
//!   is a checked method (`checked_sub`, `checked_mul`, `checked_add`, `checked_neg`) that returns
//!   `None` instead of wrapping. So nothing here depends on overflow checks being on, which they
//!   are not in a default release build. `tests/attack_predicate.rs` scans this file's code to
//!   keep it that way.
//! - **Deterministic.** The same answer on every target: the integer paths are exact, and the
//!   filter's `f64` operations are correctly rounded everywhere and give a sign only when it is the
//!   exact sign.
//! - **Bounded proofs, and what is only tested** (chapter 28.7; piece G41 collects them here).
//!   Verus proves that the generated exact paths of [`orient2d`], [`orient3d`], [`incircle`] and
//!   [`insphere`] compute their determinants exactly and never overflow, for every grid input
//!   (pieces G36 and G38: the twins in `verus/generated/`, token for token the shipped modules),
//!   and that the [`crate::wide`] arithmetic they run on is exact (G37). Kani proves that
//!   [`orient2d`] answers with the exact sign, nothing panicking or overflowing on the way, filter
//!   and routing included, for every coordinate within one step of 0 or of an edge of the frame
//!   (piece G14: geom-fuzz's `orient2d_proof`). The filters' error bounds are checked against
//!   exact arithmetic, not proven, until pieces G35 and G40 certify them; the routing and every
//!   other input are tested against exact oracles (`tests/attack_predicate.rs`, the verification
//!   suite).
//!
//! Where [`orient2d`]'s filter runs, a call the filter leaves first takes the narrow step of chapter
//! 23.2: when every coordinate difference is below 2^31, 335 m at q0, [`orient2d_narrow`] computes
//! the same determinant in checked `i64`, which saves the wide multiplies of an exact zero. A target
//! with a 64 × 64 → 128-bit multiply goes straight to an exact path as cheap as that (routing,
//! below). Each `_step` twin gives the same sign and names the step that answered ([`Step`]): it
//! makes the plain call, then asks each step in turn, so counting costs the plain call nothing.
//! [`insphere`] has no twin: its steps are the filter and the exact path, and a second caller of its
//! filter costs every plain call the filter's inlining (15% on general points, measured). Nor has
//! it a narrow step: 72δ⁵ stays in `i64` only for differences below 2^11, a third of a millimetre at
//! q0.
//!
//! Four evaluation steps follow, each exact; a call returns from the first that answers:
//!
//! 1. **The generated filter** ([`crate::generated`]): the determinant in `f64` with a
//!    semi-static error bound (Meyer and Pion's method, as Geogram's PCK uses it), generated with
//!    its error constant by geom-gen from a description of the predicate. It returns a sign only
//!    when the bound proves it, and `Zero` only when a scale factor is exactly zero; otherwise it
//!    leaves the call to the next step. Every predicate runs it first, except [`orient2d`] on
//!    targets where it costs more than the exact path it would save (routing, below). Every grid
//!    coordinate has magnitude at most
//!    [`COORD_MAX`](crate::grid::COORD_MAX) = 2^40 − 1, and no other coordinate can be made:
//!    [`Coord`](crate::grid::Coord) keeps its value private, and its constants and every function
//!    that makes one (`Coord::new`, `Coord::from_i128`, `refine` and `coarsen`) keep it within
//!    that range (`tests/attack2_predicate.rs` checks the functions). The filter's bound and the
//!    generated exact path's widths assume that range.
//! 2. **The generated exact path**, which answers every call the filter leaves: the same
//!    determinant with checked integer arithmetic, each step in the narrowest type its generated
//!    bit-range certificate allows (`i64` differences; `i128` products of them, each one
//!    64 × 64 → 128-bit multiply, and `i128` sums for orientation; `i128` factors and [`I256`]
//!    products for in-circle and in-sphere). The certificate proves that no step returns `None`
//!    for a grid input; geom-gen's tests re-derive it with an independent checker.
//! 3. **The hand-written fast path**, the same determinants as written here before the generator
//!    existed, now a second line of defence: it would answer only if the generated exact path
//!    returned `None`, which its certificate rules out. Orientation runs in `i128`; in-circle and
//!    in-sphere compute their factors in `i128` and sum the products in [`I256`]. The section
//!    [Bit budgets](#bit-budgets) below derives this path's budgets, proving that none of its
//!    checked operations returns `None` for any grid input; their arithmetic is checked when the
//!    crate compiles (the budget exponents from `FAST_DIFF_BITS` on), so a wrong one is a build
//!    error. The generated certificate derives the same budgets.
//! 4. **The total path**, defence in depth, which no public call reaches. It would answer only if
//!    both fast-path budgets were wrong, so such a mistake would cost time, never a wrong sign. It
//!    evaluates the same determinant from the raw `i64` coordinates in
//!    [`I384`], which holds every intermediate for any `i64` coordinates
//!    whatsoever, not only grid ones. Here the bounds are not only written: every value on the
//!    total path carries its bound in its type, and each operation asserts, when the crate is
//!    compiled, that the bound its result's type declares follows from its operands' bounds and
//!    that `I384` holds it. A wrong bound is a build error (the derivation and the rules are
//!    written where the total path begins). Only this module's unit tests and
//!    `tests/attack2_predicate.rs`, which compiles this code a second time to call it directly,
//!    run the total path.
//!
//! Routing, measured (chapter 23.2: "whether to filter is measured, not assumed"). The benchmark
//! `generated_route_against_the_hand_written_fast_path` in this module's tests times, per
//! predicate, the hand-written fast path, the generated exact path alone, and the filter followed
//! by the exact path, on random and on degenerate points. Release build, nanoseconds per call net
//! of the loop, on x86-64 natively (Windows) and on wasm32-wasip1 under wasmtime 50:
//!
//! | Predicate | Points | x86-64: hand-written, exact, filter then exact | wasm32: the same |
//! | --- | --- | --- | --- |
//! | orient2d | random | 18.7, 3.3, 16.3 | 54.1, 20.8, 7.7 |
//! | orient2d | degenerate | 19.0, 3.2, 18.9 | 79.2, 31.4, 42.7 |
//! | orient3d | random | 93.2, 27.7, 26.0 | 285.3, 163.0, 15.4 |
//! | orient3d | degenerate | 98.4, 31.0, 43.9 | 422.5, 256.7, 197.3 |
//! | incircle | random | 199.7, 110.9, 24.8 | 796.8, 554.6, 13.8 |
//! | incircle | degenerate | 160.5, 86.1, 74.1 | 946.3, 635.7, 449.7 |
//! | insphere | random | 543.7, 291.4, 48.1 | 1825.2, 1391.8, 36.7 |
//! | insphere | degenerate | 530.2, 260.3, 230.0 | 2632.8, 1826.0, 1324.9 |
//!
//! Chapter 6.2 puts the filter first, and principle 12 (G01 section 3) blocks any speed
//! regression over 5%. On a 64-bit target with a 64 × 64 → 128-bit multiply, orient2d's exact
//! path is a handful of integer instructions, five times cheaper than its filter, so filtering
//! first made exactly zero calls (the common case in buildings, chapter 23.2) slower than before
//! this module was generated. There, and only there, [`orient2d`] goes straight to the exact
//! path (`ORIENT2D_FILTER_FIRST`): switching a filter off where measurement shows it does not
//! pay is chapter 23.2's proposed amendment to 6.2, which an owner has yet to accept (section 32),
//! applied only where principle 12 leaves no other choice. The width-aware exact path does less
//! work than the hand-written one it replaced (64-bit differences and single widening products
//! instead of 128-bit differences and general overflow-checked 128-bit products), so no target
//! is slower than before. Every other predicate, and orient2d on WebAssembly and 32-bit targets,
//! filters first and is faster than before on both kinds of points on both targets measured. On
//! x86-64 orient3d's filter breaks even on random points and costs about 40% on degenerate ones,
//! still twice as fast as before; whether to switch it off too is the owner's call under the same
//! amendment. Timings move with the machine's load; the ratios within one run are what count.
//!
//! What the types cannot rule out is reported. The total path's operations are still checked
//! methods returning `Option`, because stable Rust has no multiplication that cannot fail by its
//! type other than the native `*` and the wrapping and overflowing methods, which this module
//! never uses (`u64::widening_mul` is unstable). With every bound checked at compile time, one of
//! them could return `None` only if [`Wide`](crate::wide::Wide)'s checked arithmetic reported an
//! overflow for a result that fits, a defect its own tests look for against big integers. The
//! predicate then returns [`NumError::BudgetExceeded`]: an error, never a sign, so a defect can
//! never pass for an exact degeneracy. That is why each predicate returns a `Result`; for grid
//! points it is always `Ok`.
//!
//! Method: the determinant forms of Shewchuk, *Adaptive Precision Floating-Point Arithmetic and
//! Fast Robust Geometric Predicates*, Discrete & Computational Geometry 18 (1997)
//! (<https://doi.org/10.1007/PL00009321>; reference code
//! <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>): orientation is the
//! determinant of the edge vectors, and in-circle and in-sphere are the orientation of the points
//! lifted onto the paraboloid z = |p|², written relative to the query point so every entry is a
//! small difference. Shewchuk evaluates these in floating point with adaptive expansions; on the
//! grid the inputs are integers below 2^40, so the same determinants evaluate exactly in
//! fixed-width integers. The filter and the exact path in front of the hand-written ones are
//! generated (module [`crate::generated`], whose header states their method, proof and cost). The
//! total path's bounds follow the static bit-length analysis of Fortune and Van Wyk, *Static
//! Analysis Yields Efficient Exact Integer Arithmetic for Computational Geometry*, ACM
//! Transactions on Graphics 15(3) (1996), carried in Rust's type system so that the compiler
//! checks them.
//!
//! Conventions: [`orient2d`] and [`incircle`] have the signs of `predicates.c`. [`orient3d`] and
//! [`insphere`] have the **opposite** signs to `predicates.c`, whose `orient3d` is positive when
//! `d` lies *below* the plane of `a`, `b`, `c`; here it is positive when `d` lies on the side that
//! the right-hand normal (b − a) × (c − a) points to. Code ported from Shewchuk-based libraries
//! must account for this.
//!
//! Cost: per call, the generated filter's few dozen `f64` operations where the routing runs
//! it, and the exact path only when the filter cannot decide (or first, where the filter does
//! not pay): a fixed number of checked integer operations, each module's header gives the
//! counts. No allocation and no recursion.
//!
//! Consulted: J. R. Shewchuk, `predicates.c`, public domain,
//!   <https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c>, for the determinant
//!   forms and the sign conventions of `orient2d` and `incircle` that the Method and Conventions
//!   above follow; chapter 29.3 asks for this credit line.
//!
//! # Bit budgets
//!
//! Chapter 6.2 asks each predicate's module for the written derivation of its bit budget: why
//! orient2d and orient3d fit `i128`, and in-circle and in-sphere fit 256 bits ([`I256`]), for every
//! grid input. This is that derivation, for the exact paths in front of the total path (the
//! generated one and the hand-written one evaluate the same determinants, step for step).
//!
//! Notation: M = `COORD_MAX` = 2^40 − 1, and δ = 2M = 2^41 − 2 < 2^41, the largest magnitude of a
//! difference of two grid coordinates. Every bound is on absolute values, by two rules: a product
//! of values at most X and Y is at most X·Y, and a sum or difference of k values each at most X is
//! at most k·X. A sum's bound also bounds each of its partial sums, so the order of evaluation
//! never matters. A type holds every magnitude below 2^127 (`i128`) or 2^255 ([`I256`]).
//!
//! | Predicate | Difference | Lift | Minor | Term | Result | Type | Headroom |
//! | --- | --- | --- | --- | --- | --- | --- | --- |
//! | orient2d | δ < 2^41 | — | — | δ² < 2^82 | 2δ² < 2^83 | `i128` | 44 bits |
//! | orient3d | δ < 2^41 | — | 2δ² < 2^83 | 2δ³ < 2^124 | 6δ³ < 2^126 | `i128` | 1 bit |
//! | incircle | δ < 2^41 | 2δ² < 2^83 | 2δ² < 2^83 | 4δ⁴ < 2^166 | 12δ⁴ < 2^168 | `I256` | 87 bits |
//! | insphere | δ < 2^41 | 3δ² < 2^84 | 6δ³ < 2^126 | 18δ⁵ < 2^210 | 72δ⁵ < 2^212 | `I256` | 43 bits |
//!
//! The columns: each coordinate difference; each squared length |p′|² of the lifted tests; each
//! minor (a component of orient3d's cross product, a 2 × 2 determinant p′ × q′ of in-circle, a
//! 3 × 3 determinant of in-sphere); each term the result sums; the result; the type the result and
//! its terms are computed in; and the bits between the result's bound and that type's limit.
//!
//! - **orient2d**, det[b − a; c − a] = (b.x − a.x)(c.y − a.y) − (b.y − a.y)(c.x − a.x): each
//!   difference at most δ, each of the two products at most δ², their difference at most 2δ², far
//!   below 2^127. The largest value over the whole grid is δ² = 4M², reached by a triangle that is
//!   half the bounding square, such as (−M, −M), (M, −M), (M, M): no triangle in a square has more
//!   than half its area.
//! - **orient3d**, the triple product (b − a) · ((c − a) × (d − a)): each component of the cross
//!   product a difference of two products of differences, at most 2δ²; each of the three products
//!   in the dot product at most δ · 2δ² = 2δ³ < 2^124; the sum at most 6δ³ < 2^126 (6 < 2^3,
//!   δ³ < 2^123), below 2^127, so negating it cannot overflow either. The largest value over the
//!   whole grid is 2δ³ = 16M³: the regular tetrahedron on alternate corners of the bounding cube,
//!   the largest tetrahedron in a cube (a third of its volume).
//! - **incircle**, relative to d with p′ = p − d: each lift |p′|² at most 2δ², each 2 × 2
//!   determinant p′ × q′ at most 2δ², both below 2^83 and computed in `i128`. Each of the three
//!   products of a lift and a determinant is at most 4δ⁴ < 2^166 (δ⁴ < 2^164), and the sum at most
//!   12δ⁴ < 2^168 (12 < 2^4), in [`I256`]: every product of two `i128` values fits it
//!   (|x · y| < 2^254), and the sum stays 87 bits below its limit. Over all points with
//!   coordinates in {−M, 0, M} the largest value is 8M⁴ = δ⁴ / 2, reached by three corners of the
//!   bounding square and its centre. That is not claimed to be the largest over the whole grid,
//!   which is not derived here; nothing depends on it, since the proof uses only the bound 12δ⁴.
//! - **insphere**, relative to e with p′ = p − e: each lift |p′|² at most 3δ² < 2^84; each 3 × 3
//!   determinant at most 6δ³ < 2^126 (orient3d's derivation), both computed in `i128`, so negating
//!   a determinant cannot overflow. Each of the four products is at most 18δ⁵ < 2^210 (18 < 2^5,
//!   δ⁵ < 2^205), and the sum at most 72δ⁵ < 2^212 (72 < 2^7), in [`I256`], 43 bits below its
//!   limit. Over all points with coordinates in {−M, 0, M} the largest value is 48M⁵ = 3δ⁵ / 2,
//!   reached by a regular tetrahedron on alternate corners of the bounding cube and its centre.
//!   That is not claimed to be the largest over the whole grid, which is not derived here; nothing
//!   depends on it, since the proof uses only the bound 72δ⁵.
//!
//! Three checks hold this table to the code. The hand-written path's exponents (`FAST_DIFF_BITS`
//! to `FAST_INSPHERE_BITS` below) are computed from [`COORD_BITS`] and asserted when the crate
//! compiles, against these figures and against each type: raising [`COORD_BITS`] to 41 makes
//! orient3d's exponent 129, over `i128`'s 127, and the build stops. The generated exact path's
//! bit-range certificates, in each module of [`crate::generated`], derive the same bounds step by
//! step from M, and geom-gen's tests re-derive them with an independent checker and compare them
//! with these. And `tests/attack_g28.rs` reads this table, re-derives every cell in big integers
//! from the rules above, and checks every grid configuration with every coordinate at ±M (4,096
//! for orient3d) against it, the two largest values above included; it also takes every set of
//! distinct points with coordinates in {−M, 0, M} through the public in-circle and in-sphere and
//! their generated exact values, and finds the largest values there to be the 8M⁴ and 48M⁵ above
//! (this module's unit tests find them on the hand-written and total paths). The total path's own
//! budget, for any `i64` coordinates in [`I384`], is written where that path begins.

use crate::error::NumError;
use crate::generated;
use crate::grid::{COORD_BITS, P2, P3};
use crate::sign::Sign;
use crate::step::Step;
use crate::wide::{I256, I384};

use bounded::Bounded;

// The fast path's bit budget as exponents, for coordinates of B = `COORD_BITS` bits: each quantity
// has magnitude below 2 to its constant. A product of values below 2^a and 2^b is below 2^(a + b);
// a sum or difference of two values below 2^a is below 2^(a + 1), and of three or four below
// 2^(a + 2). The derivations in words are on the predicates. Even here every step is a checked
// method (this file has no native arithmetic), and a step that overflowed would give `u32::MAX`,
// which every assertion below refuses.

/// a + b for budget exponents, or `u32::MAX` if that overflows.
const fn exponent_sum(a: u32, b: u32) -> u32 {
    match a.checked_add(b) {
        Some(sum) => sum,
        None => u32::MAX,
    }
}

/// A difference of two grid coordinates: |p − q| ≤ δ = 2M < 2^(B + 1).
const FAST_DIFF_BITS: u32 = exponent_sum(COORD_BITS, 1);

/// A product of two differences, δ² < 2^(2B + 2).
const FAST_PRODUCT_BITS: u32 = exponent_sum(FAST_DIFF_BITS, FAST_DIFF_BITS);

/// A 2 × 2 determinant of differences, a difference of two products: 2δ² < 2^(2B + 3).
const FAST_DET2_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 1);

/// A squared length in 2D, two squares: 2δ² < 2^(2B + 3).
const FAST_LIFT2_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 1);

/// A squared length in 3D, three squares: 3δ² < 2^(2B + 4).
const FAST_LIFT3_BITS: u32 = exponent_sum(FAST_PRODUCT_BITS, 2);

/// A 3 × 3 determinant of differences, three terms each a difference times a 2 × 2 determinant:
/// 6δ³ < 2^(3B + 6).
const FAST_DET3_BITS: u32 = exponent_sum(exponent_sum(FAST_DIFF_BITS, FAST_DET2_BITS), 2);

/// The in-circle determinant, three products of a 2D lift and a 2 × 2 determinant:
/// 12δ⁴ < 2^(4B + 8).
const FAST_INCIRCLE_BITS: u32 = exponent_sum(exponent_sum(FAST_LIFT2_BITS, FAST_DET2_BITS), 2);

/// The in-sphere determinant, four products of a 3D lift and a 3 × 3 determinant:
/// 72δ⁵ < 2^(5B + 12).
const FAST_INSPHERE_BITS: u32 = exponent_sum(exponent_sum(FAST_LIFT3_BITS, FAST_DET3_BITS), 2);

/// `i128` holds every magnitude below 2^127.
const I128_MAGNITUDE_BITS: u32 = match i128::BITS.checked_sub(1) {
    Some(bits) => bits,
    None => 0,
};

/// [`I256`] holds every magnitude below 2^255: four limbs of 64 bits (checked below against the
/// type itself), less the sign bit.
const I256_MAGNITUDE_BITS: u32 = match u64::BITS.checked_mul(4) {
    Some(bits) => match bits.checked_sub(1) {
        Some(magnitude) => magnitude,
        None => 0,
    },
    None => 0,
};

/// Checks, at compile time, the arithmetic of every fast-path budget, the figures the predicates'
/// documentation states (at B = 40), and that each value fits the type it is computed in.
const _: () = {
    assert!(I256::ZERO.to_le_limbs().len() == 4, "I256 has four limbs");
    assert!(I128_MAGNITUDE_BITS == 127 && I256_MAGNITUDE_BITS == 255);
    assert!(
        FAST_DIFF_BITS == 41 && FAST_PRODUCT_BITS == 82,
        "the figures at 40 bits"
    );
    assert!(FAST_DET2_BITS == 83 && FAST_LIFT2_BITS == 83 && FAST_LIFT3_BITS == 84);
    assert!(FAST_DET3_BITS == 126 && FAST_INCIRCLE_BITS == 168 && FAST_INSPHERE_BITS == 212);
    // `i128`: orient2d (a 2 × 2 determinant), orient3d (a 3 × 3 one, and its negation, since
    // |v| < 2^126 leaves room for it) and every factor of the in-circle and in-sphere sums.
    assert!(FAST_DET2_BITS <= I128_MAGNITUDE_BITS, "orient2d fits i128");
    assert!(FAST_DET3_BITS <= I128_MAGNITUDE_BITS, "orient3d fits i128");
    assert!(FAST_LIFT2_BITS <= I128_MAGNITUDE_BITS && FAST_LIFT3_BITS <= I128_MAGNITUDE_BITS);
    // `I256`: the in-circle and in-sphere sums of products.
    assert!(
        FAST_INCIRCLE_BITS <= I256_MAGNITUDE_BITS,
        "incircle fits I256"
    );
    assert!(
        FAST_INSPHERE_BITS <= I256_MAGNITUDE_BITS,
        "insphere fits I256"
    );
};

// ---------------------------------------------------------------------------------------------
// The fast path: `i128`, then `I256` for the in-circle and in-sphere sums.
// ---------------------------------------------------------------------------------------------

/// p − q for two coordinates, as an `i128`.
///
/// Budget: on the grid |p|, |q| ≤ M, so |p − q| ≤ δ < 2^41. For any `i64` values at all
/// |p − q| ≤ 2^64 − 1, so `None` cannot occur even off the grid.
fn diff(p: i64, q: i64) -> Option<i128> {
    i128::from(p).checked_sub(i128::from(q))
}

/// a·d − b·c, the 2 × 2 determinant with rows (a, b) and (c, d), or `None` if it does not fit
/// `i128`. Each caller bounds it.
fn det2(a: i128, b: i128, c: i128, d: i128) -> Option<i128> {
    a.checked_mul(d)?.checked_sub(b.checked_mul(c)?)
}

/// The difference p − q of two 2D points, components in `i128`.
#[derive(Clone, Copy)]
struct D2 {
    x: i128,
    y: i128,
}

impl D2 {
    /// p − q. Budget: each component at most δ < 2^41 ([`diff`]).
    fn sub(p: [i64; 2], q: [i64; 2]) -> Option<D2> {
        let ([px, py], [qx, qy]) = (p, q);
        Some(D2 {
            x: diff(px, qx)?,
            y: diff(py, qy)?,
        })
    }

    /// The 2 × 2 determinant with rows `self` and `other`: self.x · other.y − self.y · other.x.
    ///
    /// Budget: each product at most δ² < 2^82, the difference at most 2δ² < 2^83.
    fn cross(self, other: D2) -> Option<i128> {
        det2(self.x, self.y, other.x, other.y)
    }

    /// The squared length x² + y². Budget: at most 2δ² < 2^83.
    fn norm2(self) -> Option<i128> {
        let xx = self.x.checked_mul(self.x)?;
        xx.checked_add(self.y.checked_mul(self.y)?)
    }
}

/// The difference p − q of two 3D points, components in `i128`.
#[derive(Clone, Copy)]
struct D3 {
    x: i128,
    y: i128,
    z: i128,
}

impl D3 {
    /// p − q. Budget: each component at most δ < 2^41 ([`diff`]).
    fn sub(p: [i64; 3], q: [i64; 3]) -> Option<D3> {
        let ([px, py, pz], [qx, qy, qz]) = (p, q);
        Some(D3 {
            x: diff(px, qx)?,
            y: diff(py, qy)?,
            z: diff(pz, qz)?,
        })
    }

    /// The squared length x² + y² + z². Budget: at most 3δ² < 2^84 (3 < 2^2, δ² < 2^82).
    fn norm2(self) -> Option<i128> {
        let xx = self.x.checked_mul(self.x)?;
        let yy = self.y.checked_mul(self.y)?;
        let zz = self.z.checked_mul(self.z)?;
        xx.checked_add(yy)?.checked_add(zz)
    }
}

/// The 3 × 3 determinant with rows p, q, r: the scalar triple product p · (q × r).
///
/// Budget: each component of q × r is a difference of two products of differences, at most
/// 2δ² < 2^83; each of the three terms of the dot product is at most δ · 2δ² = 2δ³ < 2^124; the sum
/// is at most 6δ³ < 2^126 (6 < 2^3, δ³ < 2^123). Every intermediate fits `i128` (|v| < 2^127), and
/// so does the negation of the result.
fn det3(p: D3, q: D3, r: D3) -> Option<i128> {
    let tx = p.x.checked_mul(det2(q.y, q.z, r.y, r.z)?)?;
    let ty = p.y.checked_mul(det2(q.z, q.x, r.z, r.x)?)?;
    let tz = p.z.checked_mul(det2(q.x, q.y, r.x, r.y)?)?;
    tx.checked_add(ty)?.checked_add(tz)
}

/// x₀·y₀ + x₁·y₁ + … for `i128` factors, evaluated in [`I256`] (range ±2^255), or `None` if a
/// product or a partial sum does not fit: the wide step of [`incircle`] and [`insphere`].
///
/// Every product of two `i128` values fits (|x · y| ≤ 2^127 · 2^127 = 2^254), and the callers' bit
/// budgets keep every partial sum below 2^212, so for every grid input the result is `Some`. Off
/// the grid the sum can leave `I256` while every factor still fits `i128`: in-sphere with a, b, c
/// and d near (±(2^63 − 1), ±(2^63 − 1)) at small heights, around e at the origin, reaches about
/// −2^256, which wrapped to 256 bits would be positive, and only the checked addition here stops
/// it (`tests/attack2_predicate.rs`, `insphere_where_the_i256_sum_overflows_off_the_grid`).
/// Because the exact total path stands behind it, a failure here could never show up as a wrong
/// sign, so this module's unit tests check the budget itself. They cover every choice of points
/// with coordinates in {−M, 1 − M, 0, M − 1, M} for in-circle and in {−M, 0, M} for in-sphere
/// (each set of points once, with every choice of query point: the order of the others only
/// permutes the terms), and random points; every factor, product and sum stays within its bound.
fn sum_of_products(terms: &[(i128, i128)]) -> Option<I256> {
    terms.iter().try_fold(I256::ZERO, |sum, &(x, y)| {
        let product = I256::from_i128(x).checked_mul(&I256::from_i128(y))?;
        sum.checked_add(&product)
    })
}

/// orient2d's determinant on the fast path. Budget on [`orient2d`].
fn orient2d_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<i128> {
    D2::sub(b, a)?.cross(D2::sub(c, a)?)
}

/// orient3d's determinant on the fast path. Budget on [`orient3d`].
fn orient3d_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<i128> {
    det3(D3::sub(b, a)?, D3::sub(c, a)?, D3::sub(d, a)?)
}

/// The (lift, 2 × 2 determinant) pairs whose products sum to [`incircle`]'s determinant. Budgets
/// as derived there: every factor below 2^83, every product below 2^166, the sum below 2^168.
fn incircle_terms(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<[(i128, i128); 3]> {
    let a = D2::sub(a, d)?;
    let b = D2::sub(b, d)?;
    let c = D2::sub(c, d)?;
    Some([
        (a.norm2()?, b.cross(c)?),
        (b.norm2()?, c.cross(a)?),
        (c.norm2()?, a.cross(b)?),
    ])
}

/// incircle's determinant on the fast path. Budget on [`incircle`].
fn incircle_fast(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<I256> {
    sum_of_products(&incircle_terms(a, b, c, d)?)
}

/// The (lift, signed 3 × 3 cofactor) pairs whose products sum to [`insphere`]'s determinant.
/// Budgets as derived there: lifts below 2^84, cofactors below 2^126, every product below 2^210,
/// the sum below 2^212.
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points of the documented formula"
)]
fn insphere_terms(
    a: [i64; 3],
    b: [i64; 3],
    c: [i64; 3],
    d: [i64; 3],
    e: [i64; 3],
) -> Option<[(i128, i128); 4]> {
    let a = D3::sub(a, e)?;
    let b = D3::sub(b, e)?;
    let c = D3::sub(c, e)?;
    let d = D3::sub(d, e)?;
    Some([
        (a.norm2()?, det3(b, c, d)?),
        (b.norm2()?, det3(a, c, d)?.checked_neg()?),
        (c.norm2()?, det3(a, b, d)?),
        (d.norm2()?, det3(a, b, c)?.checked_neg()?),
    ])
}

/// insphere's determinant on the fast path. Budget on [`insphere`].
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points of the documented formula"
)]
fn insphere_fast(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) -> Option<I256> {
    sum_of_products(&insphere_terms(a, b, c, d, e)?)
}

// ---------------------------------------------------------------------------------------------
// The total path: the same determinants in `I384`, for any `i64` coordinates, every bound
// checked by the compiler.
//
// Budget for any `i64` coordinates. Each coordinate is in [−2^63, 2^63 − 1], so each difference
// has magnitude at most 2^64 − 1 < 2^64. Two rules give every other bound: for |x| < 2^a and
// |y| < 2^b, the product has |x · y| < 2^(a + b), and the sum or difference has
// |x ± y| ≤ |x| + |y| < 2^a + 2^b ≤ 2^(max(a, b) + 1). So:
//
// - each product of two differences, inside a 2 × 2 determinant or as a square, is below 2^128;
// - each 2 × 2 determinant, a difference of two such products, is below 2^129;
// - each squared length is below 2^129 in 2D; in 3D, below 2^129 after two squares and 2^130
//   after the third;
// - each term of a 3 × 3 determinant, a difference times a 2 × 2 determinant, is below 2^193; the
//   first two terms together are below 2^194, and the determinant, all three, below 2^195;
// - each in-circle product, a 2D squared length times a 2 × 2 determinant, is below 2^258; the
//   first two together are below 2^259, and the sum of all three below 2^260;
// - each in-sphere product, a 3D squared length times a 3 × 3 determinant, is below 2^325; the
//   sum is formed as (t₀ − t₁) + (t₂ − t₃), whose halves are below 2^326 and whole below 2^327.
//
// Everything stays at least 56 bits below the I384 limit of 2^383. None of this rests on the
// text alone: each bound above is the `BITS` of the `Bounded<BITS>` type its value has in the code
// below, and each operation asserts, when the crate is compiled, that the bound its result's type
// declares follows from its operands' bounds by these rules and that I384 holds it.
// ---------------------------------------------------------------------------------------------

/// The total path's integer type: holds every intermediate of every predicate for any `i64`
/// coordinates (budget above).
type Total = I384;

/// Total-path values that carry their bound in their type. A module of its own, so that nothing
/// else in this file can make a [`Bounded`] except through the operations that check its bound.
mod bounded {
    use super::Total;

    /// The bits of a difference of any two `i64` values: |p − q| ≤ 2^64 − 1 < 2^64. Not the
    /// fast path's bound, `FAST_DIFF_BITS`, which is for grid coordinates (41 bits).
    const ANY_I64_DIFFERENCE_BITS: usize = 64;

    /// 64L − 1 for a [`Wide`](crate::wide::Wide) of L limbs of 64 bits, which holds every value
    /// from −2^(64L − 1) to 2^(64L − 1) − 1, so every magnitude below 2^(64L − 1). Every step is
    /// checked; an overflow gives 0, which no bound fits.
    const fn magnitude_bits(limbs: usize) -> usize {
        let Some(bits) = limbs.checked_mul(64) else {
            return 0;
        };
        let Some(magnitude) = bits.checked_sub(1) else {
            return 0;
        };
        magnitude
    }

    /// [`Total`] holds every magnitude below 2^`TOTAL_BITS`, computed from its limb count: 2^383
    /// for [`I384`](crate::wide::I384).
    pub(super) const TOTAL_BITS: usize = magnitude_bits(Total::ZERO.to_le_limbs().len());

    /// The product rule: values below 2^`left` and 2^`right` have a product below
    /// 2^(left + right). `None` if the exponent overflows.
    pub(super) const fn product_bits(left: usize, right: usize) -> Option<usize> {
        left.checked_add(right)
    }

    /// The sum rule: values below 2^`left` and 2^`right` have a sum and a difference below
    /// 2^left + 2^right ≤ 2^(max(left, right) + 1). `None` if the exponent overflows.
    pub(super) const fn sum_bits(left: usize, right: usize) -> Option<usize> {
        let larger = if left < right { right } else { left };
        larger.checked_add(1)
    }

    /// Whether a value that a rule bounds by 2^`needed` may be declared below 2^`declared`: the
    /// rule's bound is no larger, and [`Total`] holds every magnitude below 2^`declared`.
    pub(super) const fn declarable(needed: Option<usize>, declared: usize) -> bool {
        matches!(needed, Some(bits) if bits <= declared) && declared <= TOTAL_BITS
    }

    // The figures the documentation states, and the bound of a difference.
    const _: () = assert!(TOTAL_BITS == 383, "I384 holds magnitudes below 2^383");
    const _: () = assert!(declarable(
        Some(ANY_I64_DIFFERENCE_BITS),
        ANY_I64_DIFFERENCE_BITS
    ));

    /// The compile-time checks, one constant per rule. Each is evaluated for every choice of
    /// bounds an operation is used with, when the crate is compiled, so a declared bound that does
    /// not follow from its operands' bounds, or that [`Total`] cannot hold, is a build error.
    /// (`cargo check` stops before this step; `cargo build` and `cargo test` do not.)
    struct Rule<const LEFT: usize, const RIGHT: usize, const OUT: usize>;

    impl<const LEFT: usize, const RIGHT: usize, const OUT: usize> Rule<LEFT, RIGHT, OUT> {
        /// A product of values below 2^LEFT and 2^RIGHT may be declared below 2^OUT.
        const PRODUCT: () = assert!(
            declarable(product_bits(LEFT, RIGHT), OUT),
            "a product's declared bound does not follow from its factors' bounds"
        );

        /// A sum or difference of values below 2^LEFT and 2^RIGHT may be declared below 2^OUT.
        const SUM: () = assert!(
            declarable(sum_bits(LEFT, RIGHT), OUT),
            "a sum's declared bound does not follow from its terms' bounds"
        );
    }

    /// A total-path value v with |v| < 2^BITS, where BITS ≤ 383.
    ///
    /// Only this module can make one, in four ways: [`Bounded::difference`] from two `i64`
    /// coordinates, and [`times`](Bounded::times), [`plus`](Bounded::plus) and
    /// [`minus`](Bounded::minus) from two bounded values, each checking the bound it declares at
    /// compile time ([`Rule`]). So every `Bounded<BITS>` that exists has |v| < 2^BITS.
    #[derive(Clone, Copy)]
    pub(super) struct Bounded<const BITS: usize>(Total);

    impl Bounded<ANY_I64_DIFFERENCE_BITS> {
        /// p − q for two `i64` coordinates. Each lies in [−2^63, 2^63 − 1], so
        /// |p − q| ≤ 2^63 + (2^63 − 1) = 2^64 − 1 < 2^64 for every value of the argument types.
        pub(super) fn difference(p: i64, q: i64) -> Option<Self> {
            let (p, q) = (Total::from_i64(p), Total::from_i64(q));
            p.checked_sub(&q).map(Bounded)
        }
    }

    impl<const BITS: usize> Bounded<BITS> {
        /// self · other, declared below 2^OUT: [`Rule::PRODUCT`] checks BITS + OTHER ≤ OUT ≤ 383.
        pub(super) fn times<const OTHER: usize, const OUT: usize>(
            &self,
            other: &Bounded<OTHER>,
        ) -> Option<Bounded<OUT>> {
            let () = Rule::<BITS, OTHER, OUT>::PRODUCT;
            self.0.checked_mul(&other.0).map(Bounded)
        }

        /// self + other, declared below 2^OUT: [`Rule::SUM`] checks
        /// max(BITS, OTHER) + 1 ≤ OUT ≤ 383.
        pub(super) fn plus<const OTHER: usize, const OUT: usize>(
            &self,
            other: &Bounded<OTHER>,
        ) -> Option<Bounded<OUT>> {
            let () = Rule::<BITS, OTHER, OUT>::SUM;
            self.0.checked_add(&other.0).map(Bounded)
        }

        /// self − other, declared below 2^OUT: [`Rule::SUM`] checks
        /// max(BITS, OTHER) + 1 ≤ OUT ≤ 383.
        pub(super) fn minus<const OTHER: usize, const OUT: usize>(
            &self,
            other: &Bounded<OTHER>,
        ) -> Option<Bounded<OUT>> {
            let () = Rule::<BITS, OTHER, OUT>::SUM;
            self.0.checked_sub(&other.0).map(Bounded)
        }

        /// The value.
        pub(super) fn value(self) -> Total {
            self.0
        }
    }
}

/// A coordinate difference on the total path: below 2^64.
type Diff = Bounded<64>;

/// A 2 × 2 determinant of differences: below 2^129.
type Det2 = Bounded<129>;

/// A squared length in 2D: below 2^129.
type Lift2 = Bounded<129>;

/// A squared length in 3D: below 2^130.
type Lift3 = Bounded<130>;

/// A 3 × 3 determinant of differences: below 2^195.
type Det3 = Bounded<195>;

/// p − q for two 2D points.
fn total_sub2(p: [i64; 2], q: [i64; 2]) -> Option<[Diff; 2]> {
    let ([px, py], [qx, qy]) = (p, q);
    Some([Diff::difference(px, qx)?, Diff::difference(py, qy)?])
}

/// p − q for two 3D points.
fn total_sub3(p: [i64; 3], q: [i64; 3]) -> Option<[Diff; 3]> {
    let ([px, py, pz], [qx, qy, qz]) = (p, q);
    Some([
        Diff::difference(px, qx)?,
        Diff::difference(py, qy)?,
        Diff::difference(pz, qz)?,
    ])
}

/// a·d − b·c: each product below 2^128, the difference below 2^129.
fn total_det2(a: &Diff, b: &Diff, c: &Diff, d: &Diff) -> Option<Det2> {
    let ad: Bounded<128> = a.times(d)?;
    let bc: Bounded<128> = b.times(c)?;
    ad.minus(&bc)
}

/// x² + y²: each square below 2^128, the sum below 2^129.
fn total_norm2_2d([x, y]: &[Diff; 2]) -> Option<Lift2> {
    let xx: Bounded<128> = x.times(x)?;
    let yy: Bounded<128> = y.times(y)?;
    xx.plus(&yy)
}

/// x² + y² + z²: each square below 2^128, two below 2^129, all three below 2^130.
fn total_norm2_3d([x, y, z]: &[Diff; 3]) -> Option<Lift3> {
    let xx: Bounded<128> = x.times(x)?;
    let yy: Bounded<128> = y.times(y)?;
    let zz: Bounded<128> = z.times(z)?;
    let xy: Bounded<129> = xx.plus(&yy)?;
    xy.plus(&zz)
}

/// The 3 × 3 determinant p · (q × r): each term below 2^193, two below 2^194, all three below
/// 2^195.
fn total_det3(p: &[Diff; 3], q: &[Diff; 3], r: &[Diff; 3]) -> Option<Det3> {
    let ([px, py, pz], [qx, qy, qz], [rx, ry, rz]) = (p, q, r);
    let tx: Bounded<193> = px.times(&total_det2(qy, qz, ry, rz)?)?;
    let ty: Bounded<193> = py.times(&total_det2(qz, qx, rz, rx)?)?;
    let tz: Bounded<193> = pz.times(&total_det2(qx, qy, rx, ry)?)?;
    let txy: Bounded<194> = tx.plus(&ty)?;
    txy.plus(&tz)
}

/// orient2d's determinant (b − a) × (c − a) for any `i64` coordinates: below 2^129.
fn orient2d_total(a: [i64; 2], b: [i64; 2], c: [i64; 2]) -> Option<Det2> {
    let [ux, uy] = total_sub2(b, a)?;
    let [vx, vy] = total_sub2(c, a)?;
    total_det2(&ux, &uy, &vx, &vy)
}

/// orient3d's determinant ((b − a) × (c − a)) · (d − a) for any `i64` coordinates: below 2^195.
/// `ab` is b − a, and so on.
fn orient3d_total(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) -> Option<Det3> {
    let ab = total_sub3(b, a)?;
    let ac = total_sub3(c, a)?;
    let ad = total_sub3(d, a)?;
    total_det3(&ab, &ac, &ad)
}

/// incircle's determinant, the same formula as on [`incircle`], for any `i64` coordinates: each
/// product below 2^258, two below 2^259, all three below 2^260.
fn incircle_total(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Option<Bounded<260>> {
    let a = total_sub2(a, d)?;
    let b = total_sub2(b, d)?;
    let c = total_sub2(c, d)?;
    let ([ax, ay], [bx, by], [cx, cy]) = (&a, &b, &c);
    let ta: Bounded<258> = total_norm2_2d(&a)?.times(&total_det2(bx, by, cx, cy)?)?;
    let tb: Bounded<258> = total_norm2_2d(&b)?.times(&total_det2(cx, cy, ax, ay)?)?;
    let tc: Bounded<258> = total_norm2_2d(&c)?.times(&total_det2(ax, ay, bx, by)?)?;
    let tab: Bounded<259> = ta.plus(&tb)?;
    tab.plus(&tc)
}

/// insphere's determinant, the same formula as on [`insphere`], for any `i64` coordinates: each
/// product below 2^325, summed as (t₀ − t₁) + (t₂ − t₃), whose halves are below 2^326 and whole
/// below 2^327. The same value as the formula's order t₀ − t₁ + t₂ − t₃, since nothing rounds.
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points of the documented formula"
)]
fn insphere_total(
    a: [i64; 3],
    b: [i64; 3],
    c: [i64; 3],
    d: [i64; 3],
    e: [i64; 3],
) -> Option<Bounded<327>> {
    let a = total_sub3(a, e)?;
    let b = total_sub3(b, e)?;
    let c = total_sub3(c, e)?;
    let d = total_sub3(d, e)?;
    let ta: Bounded<325> = total_norm2_3d(&a)?.times(&total_det3(&b, &c, &d)?)?;
    let tb: Bounded<325> = total_norm2_3d(&b)?.times(&total_det3(&a, &c, &d)?)?;
    let tc: Bounded<325> = total_norm2_3d(&c)?.times(&total_det3(&a, &b, &d)?)?;
    let td: Bounded<325> = total_norm2_3d(&d)?.times(&total_det3(&a, &b, &c)?)?;
    let first: Bounded<326> = ta.minus(&tb)?;
    let second: Bounded<326> = tc.minus(&td)?;
    first.plus(&second)
}

/// The fast path's sign when it answered, otherwise the sign of the total path's value.
///
/// The `None` arm of the total path cannot be reached through any argument of the four
/// `*_total` functions: their arguments are `i64` coordinates, and every bound from those to the
/// result is checked at compile time (above). It could be reached only through a defect in
/// [`Wide`](crate::wide::Wide)'s checked arithmetic, and then it is reported as
/// [`NumError::BudgetExceeded`], never answered (see the module documentation).
fn fast_else_total<const BITS: usize>(
    fast: Option<Sign>,
    total: impl FnOnce() -> Option<Bounded<BITS>>,
) -> Result<Sign, NumError> {
    match fast {
        Some(sign) => Ok(sign),
        None => total()
            .map(|exact| exact.value().signum())
            .ok_or(NumError::BudgetExceeded),
    }
}

/// A 2D grid point's coordinates.
fn xy(p: P2) -> [i64; 2] {
    [p.x.get(), p.y.get()]
}

/// A 3D grid point's coordinates.
fn xyz(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

// ---------------------------------------------------------------------------------------------
// The narrow step: `i64`, after a magnitude check (chapter 23.2).
// ---------------------------------------------------------------------------------------------

/// Differences below 2^31 (335 m at q0) keep [`orient2d`] within `i64`: two products below 2^62,
/// their difference below 2^63.
const NARROW_2D_BITS: u32 = 31;

/// What `i64` holds in magnitude: |v| < 2^63.
const I64_MAGNITUDE_BITS: u32 = 63;

const _: () = {
    let products = exponent_sum(NARROW_2D_BITS, NARROW_2D_BITS);
    assert!(
        exponent_sum(products, 1) <= I64_MAGNITUDE_BITS,
        "orient2d's narrow step fits i64"
    );
};

/// p − q when it is below 2^`bits` in magnitude: the narrow step's magnitude check.
fn narrow(p: i64, q: i64, bits: u32) -> Option<i64> {
    let d = p.checked_sub(q)?;
    (d.unsigned_abs() < u64::pow(2, bits)).then_some(d)
}

/// The narrow step of [`orient2d`]: the same sign, computed in `i64`, or `None` when a difference
/// is out of range. Budget on `NARROW_2D_BITS`; every step is checked all the same.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "a, b and c are the points, and p, q, r and s the differences of the formula"
)]
pub fn orient2d_narrow(a: P2, b: P2, c: P2) -> Option<Sign> {
    let [ax, ay] = xy(a);
    let [bx, by] = xy(b);
    let [cx, cy] = xy(c);
    let (p, q) = (
        narrow(bx, ax, NARROW_2D_BITS)?,
        narrow(cy, ay, NARROW_2D_BITS)?,
    );
    let (r, s) = (
        narrow(by, ay, NARROW_2D_BITS)?,
        narrow(cx, ax, NARROW_2D_BITS)?,
    );
    Some(Sign::of_i64(
        p.checked_mul(q)?.checked_sub(r.checked_mul(s)?)?,
    ))
}

// ---------------------------------------------------------------------------------------------
// The predicates.
// ---------------------------------------------------------------------------------------------

/// Which way the points a, b, c turn.
///
/// The sign of det[b − a; c − a] = (b.x − a.x)(c.y − a.y) − (b.y − a.y)(c.x − a.x), twice the
/// signed area of the triangle a, b, c. `Positive` when a, b, c turn counter-clockwise (c lies to
/// the left of the directed line from a to b), `Negative` when they turn clockwise, `Zero` exactly
/// when they are collinear, which includes any two coinciding. Swapping any two points flips the
/// sign. Same sign as `orient2d` in Shewchuk's `predicates.c`.
///
/// Bit budget: the determinant is at most 2δ² < 2^83, in `i128` (δ = 2^41 − 2; the derivation is
/// in the [module documentation](crate::predicate#bit-budgets)).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only if every exact path failed, which no grid points can cause
/// (the module documentation): never a sign that was not computed.
pub fn orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    if ORIENT2D_FILTER_FIRST {
        match generated::orient2d::filter(a, b, c) {
            Some(sign) => Ok(sign),
            None => orient2d_unfiltered(a, b, c),
        }
    } else {
        orient2d_exact(a, b, c)
    }
}

/// [`orient2d`], and the [`Step`] that settled it.
///
/// # Errors
///
/// As [`orient2d`].
pub fn orient2d_step(a: P2, b: P2, c: P2) -> Result<(Sign, Step), NumError> {
    let sign = orient2d(a, b, c)?;
    let filtered = generated::orient2d::filter(a, b, c).is_some();
    let narrowed = orient2d_narrow(a, b, c).is_some();
    Ok((sign, Step::route(ORIENT2D_FILTER_FIRST, filtered, narrowed)))
}

/// Whether [`orient2d`] runs its generated filter first on this target: not on a 64-bit target
/// other than WebAssembly, whose 64 × 64 → 128-bit multiply makes the exact path five times
/// cheaper than the filter, so the filter would only slow every call it cannot decide (module
/// documentation, routing); everywhere else, as chapter 6.2 asks. Either way the sign is the
/// exact one.
const ORIENT2D_FILTER_FIRST: bool = !cfg!(all(
    target_pointer_width = "64",
    not(target_family = "wasm")
));

/// [`orient2d`] when its filter cannot decide: the narrow step, then the exact paths. Out of line,
/// so that a call the filter answers does not pay for the exact paths' stack frame.
#[inline(never)]
fn orient2d_unfiltered(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    match orient2d_narrow(a, b, c) {
        Some(sign) => Ok(sign),
        None => orient2d_exact(a, b, c),
    }
}

/// [`orient2d`]'s exact steps: the generated exact path, then the hand-written fast path and the
/// total path (module documentation).
#[inline]
fn orient2d_exact(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
    let exact = generated::orient2d::exact(a, b, c);
    let [a, b, c] = [a, b, c].map(xy);
    let fast = exact.or_else(|| orient2d_fast(a, b, c).map(Sign::of_i128));
    fast_else_total(fast, || orient2d_total(a, b, c))
}

/// Which side of the plane through a, b, c the point d lies on.
///
/// The sign of det[b − a; c − a; d − a] = ((b − a) × (c − a)) · (d − a), six times the signed
/// volume of the tetrahedron a, b, c, d. `Positive` when d lies on the side the right-hand normal
/// (b − a) × (c − a) points to, so that a, b, c turn counter-clockwise seen from d; `Negative` on
/// the other side; `Zero` exactly when the four points are coplanar, which includes a, b, c being
/// collinear. Swapping any two points flips the sign. This is the opposite sign to `orient3d` in
/// Shewchuk's `predicates.c` (see the module documentation).
///
/// Bit budget: the determinant is at most 6δ³ < 2^126, in `i128` (δ = 2^41 − 2; the derivation is
/// in the [module documentation](crate::predicate#bit-budgets)).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only if every exact path failed, which no grid points can cause
/// (the module documentation): never a sign that was not computed.
pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
    match generated::orient3d::filter(a, b, c, d) {
        Some(sign) => Ok(sign),
        None => orient3d_unfiltered(a, b, c, d),
    }
}

/// [`orient3d`], and the [`Step`] that settled it. It has no narrow step: 6δ³ stays in `i64` only for
/// differences below 2^20, 16 cm at q0, too little to earn the check.
///
/// # Errors
///
/// As [`orient3d`].
pub fn orient3d_step(a: P3, b: P3, c: P3, d: P3) -> Result<(Sign, Step), NumError> {
    let filtered = generated::orient3d::filter(a, b, c, d).is_some();
    Ok((orient3d(a, b, c, d)?, Step::route(true, filtered, false)))
}

/// [`orient3d`] when its filter cannot decide: the generated exact path, then the hand-written
/// fast path and the total path (module documentation). Out of line, so that a call the filter
/// answers does not pay for the exact paths' stack frame.
#[inline(never)]
fn orient3d_unfiltered(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
    let exact = generated::orient3d::exact(a, b, c, d);
    let [a, b, c, d] = [a, b, c, d].map(xyz);
    let fast = exact.or_else(|| orient3d_fast(a, b, c, d).map(Sign::of_i128));
    fast_else_total(fast, || orient3d_total(a, b, c, d))
}

/// Whether d lies inside the circle through a, b, c.
///
/// The sign of the lifted determinant, written relative to d with p′ = p − d:
///
/// ```text
/// | |a′|²  a′.x  a′.y |
/// | |b′|²  b′.x  b′.y |  =  |a′|²·(b′ × c′) + |b′|²·(c′ × a′) + |c′|²·(a′ × b′)
/// | |c′|²  c′.x  c′.y |
/// ```
///
/// where p′ × q′ = p′.x·q′.y − p′.y·q′.x. When a, b, c are not collinear this equals orient2d's
/// determinant of (a, b, c) times (r² − |d − o|²), o and r being the circle's centre and radius.
/// So: `Positive` when d lies strictly inside the circle and a, b, c turn counter-clockwise,
/// `Negative` when strictly outside; both flip if a, b, c turn clockwise; `Zero` exactly when d is
/// on the circle. Swapping any two of the four points flips the sign. Same sign as `incircle` in
/// Shewchuk's `predicates.c`.
///
/// Degenerate cases, signs included: when a, b, c are collinear the determinant equals orient2d's
/// determinant of (a, b, d) times (c − a) · (c − b). So if two of a, b, c coincide every d gives
/// `Zero`. If they are distinct, the circle degenerates to their line: `Zero` exactly when d is on
/// it, and otherwise the sign of `orient2d(a, b, d)` when c lies outside the segment from a to b,
/// the opposite sign when c lies strictly between a and b. (Derivation: swapping c and d flips the
/// sign; apply the identity above to the circle through a, b, d; and a point c on the line through
/// a and b has power (c − a) · (c − b) with respect to every circle through a and b.)
///
/// Bit budget: lifts and 2 × 2 determinants at most 2δ² < 2^83, in `i128`, and the determinant at
/// most 12δ⁴ < 2^168, in [`I256`] (δ = 2^41 − 2; the derivation is in the
/// [module documentation](crate::predicate#bit-budgets)).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only if every exact path failed, which no grid points can cause
/// (the module documentation): never a sign that was not computed.
pub fn incircle(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
    match generated::incircle::filter(a, b, c, d) {
        Some(sign) => Ok(sign),
        None => incircle_unfiltered(a, b, c, d),
    }
}

/// [`incircle`], and the [`Step`] that settled it. It has no narrow step: 12δ⁴ stays in `i64` only
/// for differences below 2^14, 2.5 mm at q0.
///
/// # Errors
///
/// As [`incircle`].
pub fn incircle_step(a: P2, b: P2, c: P2, d: P2) -> Result<(Sign, Step), NumError> {
    let filtered = generated::incircle::filter(a, b, c, d).is_some();
    Ok((incircle(a, b, c, d)?, Step::route(true, filtered, false)))
}

/// [`incircle`] when its filter cannot decide: the generated exact path, then the hand-written
/// fast path and the total path (module documentation). Out of line, so that a call the filter
/// answers does not pay for the exact paths' stack frame.
#[inline(never)]
fn incircle_unfiltered(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
    let exact = generated::incircle::exact(a, b, c, d);
    let [a, b, c, d] = [a, b, c, d].map(xy);
    let fast = exact.or_else(|| incircle_fast(a, b, c, d).map(|sum| sum.signum()));
    fast_else_total(fast, || incircle_total(a, b, c, d))
}

/// Whether e lies inside the sphere through a, b, c, d.
///
/// The sign of the lifted determinant, written relative to e with p′ = p − e, lift column first:
///
/// ```text
/// | |a′|²  a′.x  a′.y  a′.z |
/// | |b′|²  b′.x  b′.y  b′.z |  =  |a′|²·[b′ c′ d′] − |b′|²·[a′ c′ d′]
/// | |c′|²  c′.x  c′.y  c′.z |     + |c′|²·[a′ b′ d′] − |d′|²·[a′ b′ c′]
/// | |d′|²  d′.x  d′.y  d′.z |
/// ```
///
/// where [p′ q′ r′] is the 3 × 3 determinant with those rows. When a, b, c, d are not coplanar this
/// equals orient3d's determinant of (a, b, c, d) times (r² − |e − o|²), o and r being the sphere's
/// centre and radius. So: `Positive` when e lies strictly inside the sphere and
/// `orient3d(a, b, c, d)` is `Positive`, `Negative` when strictly outside; both flip if
/// `orient3d(a, b, c, d)` is `Negative`; `Zero` exactly when e is on the sphere. Swapping any two
/// of the five points flips the sign. This is the opposite sign to `insphere` in Shewchuk's
/// `predicates.c`, matching this module's [`orient3d`] (see the module documentation).
///
/// Degenerate cases, signs included. If a, b, c, d are coplanar there is no single sphere through
/// them, and:
///
/// - If a, b, c are not collinear, the determinant equals orient3d's determinant of (a, b, c, e)
///   times (|d − o|² − r²), o and r now being the centre and radius of the circle through a, b, c.
///   So if d is on that circle every e gives `Zero`. Otherwise `Zero` means exactly that e is in
///   their plane, and off it the sign is that of `orient3d(a, b, c, e)` when d lies strictly
///   outside the circle, the opposite when strictly inside.
/// - If a, b, c are collinear (which makes a, b, c, d coplanar for every d), the determinant equals
///   minus orient3d's determinant of (a, b, d, e) times (c − a) · (c − b). So if two of a, b, c
///   coincide, or d is on their line, every e gives `Zero`. Otherwise `Zero` means exactly that e
///   is in the plane of a, b, d, and off it the sign is that of `orient3d(a, b, d, e)` when c lies
///   strictly between a and b, the opposite when c lies outside the segment.
///
/// (Derivation: swapping d and e flips the sign; apply the identity above to the sphere through
/// a, b, c, e; a point in the plane of a, b, c has the same power with respect to that sphere as
/// with respect to the circle where the sphere meets the plane. For collinear a, b, c swap c and d
/// first, and use the power (c − a) · (c − b) as for [`incircle`].)
///
/// Bit budget: lifts at most 3δ² < 2^84 and 3 × 3 determinants at most 6δ³ < 2^126, in `i128`, and
/// the determinant at most 72δ⁵ < 2^212, in [`I256`] (δ = 2^41 − 2; the derivation is in the
/// [module documentation](crate::predicate#bit-budgets)).
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] only if every exact path failed, which no grid points can cause
/// (the module documentation): never a sign that was not computed.
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points of the documented formula"
)]
pub fn insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
    match generated::insphere::filter(a, b, c, d, e) {
        Some(sign) => Ok(sign),
        None => insphere_unfiltered(a, b, c, d, e),
    }
}

/// [`insphere`] when its filter cannot decide: the generated exact path, then the hand-written
/// fast path and the total path (module documentation). Out of line, so that a call the filter
/// answers does not pay for the exact paths' stack frame.
#[inline(never)]
#[allow(
    clippy::many_single_char_names,
    reason = "a to e name the points of the documented formula"
)]
fn insphere_unfiltered(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
    let exact = generated::insphere::exact(a, b, c, d, e);
    let [a, b, c, d, e] = [a, b, c, d, e].map(xyz);
    let fast = exact.or_else(|| insphere_fast(a, b, c, d, e).map(|sum| sum.signum()));
    fast_else_total(fast, || insphere_total(a, b, c, d, e))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    //! The two paths on their own. On the grid the fast path always succeeds, so no public call
    //! reaches the total path, and since the total path is exact, taking it could never show up as
    //! a wrong sign. So these tests check the fast path's bit budgets directly (every factor,
    //! product and sum within its bound, on the point sets each test names and on random points),
    //! check that the total path gives the same value wherever the fast path runs, and drive the
    //! total path with coordinates far off the grid, up to the limits of `i64`, against an
    //! independent `num-bigint` determinant. They also check the rules the compiler applies to the
    //! total path's bounds: sound on every small case, and refusing what does not follow.

    use num_bigint::BigInt;
    use proptest::prelude::*;

    use super::bounded::{TOTAL_BITS, declarable, product_bits, sum_bits};
    use super::exponent_sum;
    use super::{
        Bounded, Diff, Total, fast_else_total, incircle_fast, incircle_terms, incircle_total,
        insphere_fast, insphere_terms, insphere_total, orient2d_fast, orient2d_total,
        orient3d_fast, orient3d_total, sum_of_products,
    };
    use crate::grid::COORD_MAX;
    use crate::sign::Sign;

    /// `exponent_sum`, which only the budget constants call (so only the compiler ran it): the sum,
    /// or `u32::MAX` when the sum overflows, which no budget fits.
    #[test]
    fn exponent_sums_add_or_saturate_to_the_impossible_budget() {
        assert_eq!(exponent_sum(40, 1), 41);
        assert_eq!(exponent_sum(0, 0), 0);
        assert_eq!(exponent_sum(u32::MAX - 1, 1), u32::MAX);
        assert_eq!(exponent_sum(u32::MAX, 1), u32::MAX);
        assert_eq!(exponent_sum(1, u32::MAX), u32::MAX);
    }
    use crate::wide::{I256, Wide};

    const M: i64 = COORD_MAX;

    /// Coordinates at an edge of the grid, one step inside it, or at its centre.
    const LEVELS_5: [i64; 5] = [-M, 1 - M, 0, M - 1, M];
    /// Coordinates at an edge of the grid or at its centre.
    const LEVELS_3: [i64; 3] = [-M, 0, M];

    const INCIRCLE_BITS: [u32; 4] = [83, 83, 166, 168];
    const INSPHERE_BITS: [u32; 4] = [84, 126, 210, 212];

    /// 2^bits as an `I256`, for bits < 255.
    fn pow2(bits: u32) -> I256 {
        let mut limbs = [0u64; 4];
        limbs[(bits / 64) as usize] = 1 << (bits % 64);
        I256::from_le_limbs(limbs)
    }

    /// Whether |v| < 2^bits, for bits < 255.
    fn below(v: I256, bits: u32) -> bool {
        let bound = pow2(bits);
        bound.checked_neg().unwrap() < v && v < bound
    }

    /// |v|.
    fn magnitude(v: I256) -> I256 {
        if v.is_negative() {
            v.checked_neg().unwrap()
        } else {
            v
        }
    }

    /// k · M^n as an `I256`.
    fn times_m_to_the(k: i64, n: u32) -> I256 {
        let mut v = I256::from_i64(k);
        for _ in 0..n {
            v = v.checked_mul(&I256::from_i64(M)).unwrap();
        }
        v
    }

    /// The fast path's value widened to the total path's type.
    fn widen(v: I256) -> Total {
        v.resize::<6>().unwrap()
    }

    /// A total path's value, without the bound its type carries.
    fn total_value<const BITS: usize>(v: Option<Bounded<BITS>>) -> Option<Total> {
        v.map(Bounded::value)
    }

    /// The fast path's (lift, determinant) pairs against their budget: for bits = [x, y, p, s], the
    /// factors below 2^x and 2^y, every product below 2^p, and the sum, which must fit `I256`,
    /// below 2^s. Returns the sum.
    fn assert_within(terms: &[(i128, i128)], bits: [u32; 4]) -> I256 {
        let [x_bits, y_bits, product_bits, sum_bits] = bits;
        for &(x, y) in terms {
            assert!(x.unsigned_abs() < 1 << x_bits, "{terms:?}");
            assert!(y.unsigned_abs() < 1 << y_bits, "{terms:?}");
            let product = sum_of_products(&[(x, y)]).unwrap();
            assert!(below(product, product_bits), "{terms:?}");
        }
        let sum = sum_of_products(terms).expect("I256 must hold the sum");
        assert!(below(sum, sum_bits), "{terms:?}");
        sum
    }

    /// Every 2D point with both coordinates in `levels`.
    fn square(levels: &[i64]) -> Vec<[i64; 2]> {
        let mut points = Vec::new();
        for &x in levels {
            for &y in levels {
                points.push([x, y]);
            }
        }
        points
    }

    /// Every 3D point with all three coordinates in `levels`.
    fn cube(levels: &[i64]) -> Vec<[i64; 3]> {
        let mut points = Vec::new();
        for &x in levels {
            for &y in levels {
                for &z in levels {
                    points.push([x, y, z]);
                }
            }
        }
        points
    }

    /// Whether every coordinate of every point is in {−M, 0, M}.
    fn on_levels_3(points: &[&[i64]]) -> bool {
        let on = |p: &&[i64]| p.iter().all(|v| LEVELS_3.contains(v));
        points.iter().all(on)
    }

    /// orient2d on every ordered triple of points with coordinates in {−M, 1 − M, 0, M − 1, M}, and
    /// incircle with every d and every unordered a, b, c from the same points (the order of a, b, c
    /// only permutes the three terms, up to sign). The fast path succeeds within its budget, the
    /// total path gives the same value, the largest orient2d value is the documented 4M², and the
    /// largest incircle value over the points in {−M, 0, M} is the documented 8M⁴.
    #[test]
    fn fast_path_budgets_hold_at_the_extremes_in_2d() {
        let points = square(&LEVELS_5);
        let mut largest = 0u128;
        for &a in &points {
            for &b in &points {
                for &c in &points {
                    let det = orient2d_fast(a, b, c).expect("i128 must hold orient2d");
                    assert!(det.unsigned_abs() < 1 << 83, "{a:?} {b:?} {c:?}");
                    let total = total_value(orient2d_total(a, b, c));
                    assert_eq!(total, Some(Total::from_i128(det)), "{a:?} {b:?} {c:?}");
                    largest = largest.max(det.unsigned_abs());
                }
            }
        }
        assert_eq!(largest, 4 * (M as u128).pow(2));

        let mut largest = I256::ZERO;
        for &d in &points {
            for (i, &a) in points.iter().enumerate() {
                for (j, &b) in points.iter().enumerate().skip(i) {
                    for &c in points.iter().skip(j) {
                        let terms = incircle_terms(a, b, c, d).expect("i128 holds the terms");
                        let sum = assert_within(&terms, INCIRCLE_BITS);
                        assert_eq!(incircle_fast(a, b, c, d), Some(sum));
                        let total = total_value(incircle_total(a, b, c, d));
                        assert_eq!(total, Some(widen(sum)), "{:?}", [a, b, c, d]);
                        if on_levels_3(&[&a, &b, &c, &d]) {
                            largest = largest.max(magnitude(sum));
                        }
                    }
                }
            }
        }
        assert_eq!(largest, times_m_to_the(8, 4));
    }

    /// orient3d with every base point a and every unordered b, c, d, and insphere with every e and
    /// every unordered a, b, c, d (the order of a, b, c, d only permutes the four terms, up to
    /// sign), all from the 27 points with coordinates in {−M, 0, M}: the bounding cube's corners,
    /// edge midpoints, face centres and centre. The fast path succeeds within its budget, the total
    /// path gives the same value, and the largest values are the documented 16M³ and 48M⁵.
    #[test]
    fn fast_path_budgets_hold_at_the_extremes_in_3d() {
        let points = cube(&LEVELS_3);
        let mut largest = 0u128;
        for &a in &points {
            for (i, &b) in points.iter().enumerate() {
                for (j, &c) in points.iter().enumerate().skip(i) {
                    for &d in points.iter().skip(j) {
                        let det = orient3d_fast(a, b, c, d).expect("i128 must hold orient3d");
                        assert!(det.unsigned_abs() < 1 << 126, "{:?}", [a, b, c, d]);
                        let total = total_value(orient3d_total(a, b, c, d));
                        assert_eq!(total, Some(Total::from_i128(det)), "{:?}", [a, b, c, d]);
                        largest = largest.max(det.unsigned_abs());
                    }
                }
            }
        }
        assert_eq!(largest, 16 * (M as u128).pow(3));

        let mut largest = I256::ZERO;
        for &e in &points {
            for (i, &a) in points.iter().enumerate() {
                for (j, &b) in points.iter().enumerate().skip(i) {
                    for (k, &c) in points.iter().enumerate().skip(j) {
                        for &d in points.iter().skip(k) {
                            let terms = insphere_terms(a, b, c, d, e).expect("i128 holds terms");
                            let sum = assert_within(&terms, INSPHERE_BITS);
                            assert_eq!(insphere_fast(a, b, c, d, e), Some(sum));
                            let total = total_value(insphere_total(a, b, c, d, e));
                            assert_eq!(total, Some(widen(sum)), "{:?}", [a, b, c, d, e]);
                            largest = largest.max(magnitude(sum));
                        }
                    }
                }
            }
        }
        assert_eq!(largest, times_m_to_the(48, 5));
    }

    /// A coordinate anywhere on the grid or at an edge.
    fn coord() -> impl Strategy<Value = i64> {
        prop_oneof![-M..=M, Just(M), Just(-M)]
    }

    proptest! {
        #[test]
        fn fast_path_budgets_hold_on_random_points(
            v in proptest::array::uniform::<_, 15>(coord()),
        ) {
            let p2 = |k: usize| [v[k], v[k + 1]];
            let (a, b, c, d) = (p2(0), p2(2), p2(4), p2(6));
            let det = orient2d_fast(a, b, c).unwrap();
            prop_assert_eq!(total_value(orient2d_total(a, b, c)), Some(Total::from_i128(det)));
            let sum = assert_within(&incircle_terms(a, b, c, d).unwrap(), INCIRCLE_BITS);
            prop_assert_eq!(total_value(incircle_total(a, b, c, d)), Some(widen(sum)));

            let p3 = |k: usize| [v[k], v[k + 1], v[k + 2]];
            let (a, b, c, d, e) = (p3(0), p3(3), p3(6), p3(9), p3(12));
            let det = orient3d_fast(a, b, c, d).unwrap();
            prop_assert_eq!(total_value(orient3d_total(a, b, c, d)), Some(Total::from_i128(det)));
            let sum = assert_within(&insphere_terms(a, b, c, d, e).unwrap(), INSPHERE_BITS);
            prop_assert_eq!(total_value(insphere_total(a, b, c, d, e)), Some(widen(sum)));
        }
    }

    // -----------------------------------------------------------------------------------------
    // The total path against an independent determinant, up to the limits of i64.
    // -----------------------------------------------------------------------------------------

    const LO: i64 = i64::MIN;
    const HI: i64 = i64::MAX;

    /// The value a `Wide` holds, read from its two's-complement limbs.
    fn big<const L: usize>(w: &Wide<L>) -> BigInt {
        let limbs = w.to_le_limbs();
        let bytes: Vec<u8> = limbs.iter().flat_map(|l| l.to_le_bytes()).collect();
        BigInt::from_signed_bytes_le(&bytes)
    }

    /// The determinant by cofactor expansion along the first row.
    fn laplace(m: &[Vec<BigInt>]) -> BigInt {
        let Some((first, rest)) = m.split_first() else {
            return BigInt::from(1);
        };
        let mut det = BigInt::from(0);
        for (j, x) in first.iter().enumerate() {
            let minor: Vec<Vec<BigInt>> = rest.iter().map(|row| without(row, j)).collect();
            let term = x * laplace(&minor);
            det += if j % 2 == 0 { term } else { -term };
        }
        det
    }

    /// `row` without its entry `j`.
    fn without(row: &[BigInt], j: usize) -> Vec<BigInt> {
        let mut out = row.to_vec();
        out.remove(j);
        out
    }

    /// The determinant of the homogeneous rows [p, 1], or [p, |p|², 1] with the lift, negated if
    /// asked: orient2d is det[a 1; b 1; c 1], orient3d −det[a 1; …; d 1], incircle
    /// det[a |a|² 1; …; d |d|² 1] and insphere −det[a |a|² 1; …; e |e|² 1].
    fn oracle(points: &[&[i64]], lift: bool, negate: bool) -> BigInt {
        let rows: Vec<Vec<BigInt>> = points
            .iter()
            .map(|p| {
                let mut row: Vec<BigInt> = p.iter().map(|&v| BigInt::from(v)).collect();
                if lift {
                    row.push(p.iter().map(|&v| BigInt::from(v) * v).sum::<BigInt>());
                }
                row.push(BigInt::from(1));
                row
            })
            .collect();
        let det = laplace(&rows);
        if negate { -det } else { det }
    }

    /// The total path's value equals the oracle's and stays below 2^bits in magnitude.
    fn assert_exact(value: Option<Total>, expected: &BigInt, bits: u64, what: &str) {
        let value = big(&value.unwrap_or_else(|| panic!("{what}: I384 overflowed")));
        assert_eq!(&value, expected, "{what}");
        assert!(value.bits() <= bits, "{what}: over 2^{bits}");
    }

    fn exact_orient2d(a: [i64; 2], b: [i64; 2], c: [i64; 2]) {
        let expected = oracle(&[&a, &b, &c], false, false);
        let what = format!("orient2d {:?}", [a, b, c]);
        let value = total_value(orient2d_total(a, b, c));
        assert_exact(value, &expected, 129, &what);
    }

    fn exact_orient3d(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3]) {
        let expected = oracle(&[&a, &b, &c, &d], false, true);
        let what = format!("orient3d {:?}", [a, b, c, d]);
        let value = total_value(orient3d_total(a, b, c, d));
        assert_exact(value, &expected, 195, &what);
    }

    fn exact_incircle(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) {
        let expected = oracle(&[&a, &b, &c, &d], true, false);
        let what = format!("incircle {:?}", [a, b, c, d]);
        let value = total_value(incircle_total(a, b, c, d));
        assert_exact(value, &expected, 260, &what);
    }

    fn exact_insphere(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) {
        let expected = oracle(&[&a, &b, &c, &d, &e], true, true);
        let what = format!("insphere {:?}", [a, b, c, d, e]);
        let value = total_value(insphere_total(a, b, c, d, e));
        assert_exact(value, &expected, 327, &what);
    }

    /// Every ordering of `items`.
    fn orderings<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
        if items.is_empty() {
            return vec![Vec::new()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let first = rest.remove(i);
            for mut tail in orderings(&rest) {
                tail.insert(0, first);
                out.push(tail);
            }
        }
        out
    }

    /// p with the axes whose bit is set in `mask` reflected about −1/2 (v ↦ −1 − v), which swaps
    /// `i64::MIN` and `i64::MAX` and keeps every coordinate an `i64`.
    fn reflect<const D: usize>(p: [i64; D], mask: usize) -> [i64; D] {
        std::array::from_fn(|k| if (mask >> k) & 1 == 1 { !p[k] } else { p[k] })
    }

    /// The configurations where the total path's intermediates peak, at the limits of `i64`: for
    /// orient2d half the bounding square, for orient3d the regular tetrahedron on alternate
    /// corners, for incircle and insphere a corner and the opposite corner (largest lift) with the
    /// corners adjacent to the opposite one (largest determinant). Every order and reflection, and
    /// orient2d and incircle on every choice of points with coordinates in
    /// {`i64::MIN`, 0, `i64::MAX`}.
    #[test]
    fn total_path_is_exact_where_its_intermediates_peak_at_the_limits_of_i64() {
        for mask in 0..4 {
            for p in orderings(&[[LO, LO], [HI, LO], [HI, HI]]) {
                let p: Vec<[i64; 2]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_orient2d(p[0], p[1], p[2]);
            }
            for p in orderings(&[[HI, HI], [HI, LO], [LO, HI], [LO, LO]]) {
                let p: Vec<[i64; 2]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_incircle(p[0], p[1], p[2], p[3]);
            }
        }
        for mask in 0..8 {
            for p in orderings(&[[LO, LO, LO], [HI, HI, LO], [HI, LO, HI], [LO, HI, HI]]) {
                let p: Vec<[i64; 3]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_orient3d(p[0], p[1], p[2], p[3]);
            }
            let base = [
                [HI, HI, HI],
                [HI, HI, LO],
                [HI, LO, HI],
                [LO, HI, HI],
                [LO, LO, LO],
            ];
            for p in orderings(&base) {
                let p: Vec<[i64; 3]> = p.into_iter().map(|q| reflect(q, mask)).collect();
                exact_insphere(p[0], p[1], p[2], p[3], p[4]);
            }
        }
        let points = square(&[LO, 0, HI]);
        for &a in &points {
            for &b in &points {
                for &c in &points {
                    exact_orient2d(a, b, c);
                    for &d in &points {
                        exact_incircle(a, b, c, d);
                    }
                }
            }
        }
    }

    /// Any `i64` coordinate, with the extremes and their neighbours often.
    fn any_coord() -> impl Strategy<Value = i64> {
        prop_oneof![
            Just(LO),
            Just(HI),
            Just(LO + 1),
            Just(HI - 1),
            Just(0i64),
            any::<i64>(),
        ]
    }

    proptest! {
        #[test]
        fn total_path_is_exact_for_any_i64_coordinates(
            v in proptest::array::uniform::<_, 15>(any_coord()),
        ) {
            let p2 = |k: usize| [v[k], v[k + 1]];
            exact_orient2d(p2(0), p2(2), p2(4));
            exact_incircle(p2(0), p2(2), p2(4), p2(6));
            let p3 = |k: usize| [v[k], v[k + 1], v[k + 2]];
            exact_orient3d(p3(0), p3(3), p3(6), p3(9));
            exact_insphere(p3(0), p3(3), p3(6), p3(9), p3(12));
        }
    }

    /// The fast path's answer is used whenever it has one, without evaluating the total path;
    /// otherwise the total path's sign. The last case cannot arise unless `Wide`'s checked
    /// arithmetic is wrong (module documentation), and pins what happens if it ever did: an
    /// error, not a sign.
    #[test]
    fn fast_else_total_routes_as_documented() {
        let never = || -> Option<Diff> { panic!("the total path must not run") };
        for sign in [Sign::Negative, Sign::Zero, Sign::Positive] {
            assert_eq!(fast_else_total(Some(sign), never), Ok(sign));
        }
        let cases = [(-3, Sign::Negative), (0, Sign::Zero), (7, Sign::Positive)];
        for (v, sign) in cases {
            assert_eq!(fast_else_total(None, || Diff::difference(v, 0)), Ok(sign));
        }
        let failed = fast_else_total::<64>(None, || None);
        assert_eq!(failed, Err(crate::error::NumError::BudgetExceeded));
    }

    /// The two rules the compiler applies to the total path's bounds are sound: on every pair of
    /// values with magnitudes below 2^a and 2^b, for a and b up to 6, the product is below the
    /// product rule's bound and the sum and difference below the sum rule's. Each rule's bound is
    /// also reached as closely as the rule claims: (2^a − 1)(2^b − 1) ≥ 2^(a + b − 1) once a and
    /// b are at least 2, and (2^a − 1) + (2^b − 1) ≥ 2^max(a, b) once both are at least 1.
    #[test]
    fn the_bound_rules_are_sound() {
        let below = |v: i64, bits: Option<usize>| v.unsigned_abs() < 1u64 << bits.unwrap();
        for a in 0..=6usize {
            for b in 0..=6usize {
                let (ra, rb) = (1i64 << a, 1i64 << b);
                for x in 1 - ra..ra {
                    for y in 1 - rb..rb {
                        let at = format!("{x} and {y}, below 2^{a} and 2^{b}");
                        assert!(below(x * y, product_bits(a, b)), "{at}");
                        assert!(below(x + y, sum_bits(a, b)), "{at}");
                        assert!(below(x - y, sum_bits(a, b)), "{at}");
                    }
                }
                let top = (ra - 1) * (rb - 1);
                if a >= 2 && b >= 2 {
                    assert!(!below(top, product_bits(a, b).map(|p| p - 1)), "{a}, {b}");
                }
                if a >= 1 && b >= 1 {
                    let sum = (ra - 1) + (rb - 1);
                    assert!(!below(sum, sum_bits(a, b).map(|s| s - 1)), "{a}, {b}");
                }
            }
        }
    }

    /// The check accepts a declared bound exactly when the rule's bound is no larger and `I384`
    /// holds it, which is every magnitude below 2^383; an exponent that overflows is refused.
    #[test]
    fn declared_bounds_are_accepted_only_when_they_follow_and_fit() {
        assert_eq!(TOTAL_BITS, 383);
        assert!(declarable(product_bits(64, 129), 193));
        assert!(declarable(product_bits(64, 129), 200));
        assert!(!declarable(product_bits(64, 129), 192));
        assert!(declarable(sum_bits(194, 193), 195));
        assert!(!declarable(sum_bits(194, 193), 194));
        assert!(!declarable(sum_bits(193, 194), 194));
        assert!(declarable(sum_bits(382, 381), 383));
        assert!(!declarable(sum_bits(383, 0), 384));
        assert!(!declarable(Some(0), 384));
        assert!(!declarable(product_bits(usize::MAX, 1), usize::MAX));
        assert!(!declarable(sum_bits(usize::MAX, 0), usize::MAX));
    }

    /// Every product of two `i128` values fits `I256`, including the largest, 2^254.
    #[test]
    fn i256_holds_every_single_product() {
        let extremes = [i128::MIN, i128::MAX];
        for x in extremes {
            for y in extremes {
                let product = sum_of_products(&[(x, y)]).unwrap();
                assert_eq!(big(&product), BigInt::from(x) * BigInt::from(y));
            }
        }
        assert_eq!(sum_of_products(&[]), Some(I256::ZERO));
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod generated_tests {
    //! The generated exact path against the hand-written fast path, value for value, and the
    //! benchmark of the generated route (filter, then exact path) against the hand-written fast
    //! path that answered every public call before it. `tests/generated_predicates.rs` checks the
    //! generated code against an independent oracle; these need the private fast path.

    use std::hint::black_box;
    use std::time::Instant;

    use super::{
        fast_else_total, incircle_fast, incircle_total, insphere_fast, insphere_total,
        orient2d_fast, orient2d_total, orient3d_fast, orient3d_total, xy, xyz,
    };
    use crate::error::NumError;
    use crate::generated::{incircle, insphere, orient2d, orient3d};
    use crate::grid::{COORD_MAX, P2, P3};
    use crate::sign::Sign;

    const M: i64 = COORD_MAX;

    /// Coordinates at an edge of the grid, one step inside it, or at its centre.
    const LEVELS_5: [i64; 5] = [-M, 1 - M, 0, M - 1, M];
    /// Coordinates at an edge of the grid or at its centre.
    const LEVELS_3: [i64; 3] = [-M, 0, M];

    /// `SplitMix64`, from a fixed seed (printed).
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Rng {
            println!("seed: {seed:016x}");
            Rng(seed)
        }

        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// A grid coordinate, uniform over the whole grid.
        fn coord(&mut self) -> i64 {
            (self.next() % (2 * M as u64 + 1)) as i64 - M
        }
    }

    fn p2([x, y]: [i64; 2]) -> P2 {
        P2::new(x, y).unwrap()
    }

    fn p3([x, y, z]: [i64; 3]) -> P3 {
        P3::new(x, y, z).unwrap()
    }

    fn square(levels: &[i64]) -> Vec<[i64; 2]> {
        levels
            .iter()
            .flat_map(|&x| levels.iter().map(move |&y| [x, y]))
            .collect()
    }

    fn cube(levels: &[i64]) -> Vec<[i64; 3]> {
        levels
            .iter()
            .flat_map(|&x| square(levels).into_iter().map(move |[y, z]| [x, y, z]))
            .collect()
    }

    fn same_2d(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) {
        let [pa, pb, pc, pd] = [a, b, c, d].map(p2);
        let what = [a, b, c, d];
        assert_eq!(
            orient2d::value(pa, pb, pc),
            orient2d_fast(a, b, c),
            "{what:?}"
        );
        let hand = incircle_fast(a, b, c, d);
        assert_eq!(incircle::value(pa, pb, pc, pd), hand, "{what:?}");
    }

    fn same_3d(a: [i64; 3], b: [i64; 3], c: [i64; 3], d: [i64; 3], e: [i64; 3]) {
        let [pa, pb, pc, pd, pe] = [a, b, c, d, e].map(p3);
        let what = [a, b, c, d, e];
        assert_eq!(
            orient3d::value(pa, pb, pc, pd),
            orient3d_fast(a, b, c, d),
            "{what:?}"
        );
        let hand = insphere_fast(a, b, c, d, e);
        assert_eq!(insphere::value(pa, pb, pc, pd, pe), hand, "{what:?}");
    }

    /// The generated exact path gives the hand-written fast path's value, bit for bit: on every
    /// triple of points with coordinates in {−M, 1 − M, 0, M − 1, M} (with a fourth point for
    /// incircle), on every b, c, d with coordinates in {−M, 0, M} after a fixed corner a (with a
    /// fifth point for insphere), and on random points.
    #[test]
    fn the_generated_exact_path_equals_the_hand_written_fast_path() {
        let mut rng = Rng::new(0x6727_0000_0000_0001);
        let points = square(&LEVELS_5);
        for &a in &points {
            for &b in &points {
                for &c in &points {
                    let d = points[(rng.next() % points.len() as u64) as usize];
                    same_2d(a, b, c, d);
                }
            }
        }
        let points = cube(&LEVELS_3);
        for &b in &points {
            for &c in &points {
                for &d in &points {
                    let e = points[(rng.next() % points.len() as u64) as usize];
                    same_3d(points[0], b, c, d, e);
                }
            }
        }
        let n = if cfg!(debug_assertions) {
            20_000
        } else {
            400_000
        };
        let r2 = |rng: &mut Rng| [rng.coord(), rng.coord()];
        let r3 = |rng: &mut Rng| [rng.coord(), rng.coord(), rng.coord()];
        for _ in 0..n {
            same_2d(r2(&mut rng), r2(&mut rng), r2(&mut rng), r2(&mut rng));
            same_3d(
                r3(&mut rng),
                r3(&mut rng),
                r3(&mut rng),
                r3(&mut rng),
                r3(&mut rng),
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // The benchmark.
    // -----------------------------------------------------------------------------------------

    /// orient2d's public body before the generator: the hand-written fast path, then the total
    /// path. Never inlined, so each call pays its own entry as a real caller's would.
    #[inline(never)]
    fn before_orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
        let [a, b, c] = [a, b, c].map(xy);
        let fast = orient2d_fast(a, b, c).map(Sign::of_i128);
        fast_else_total(fast, || orient2d_total(a, b, c))
    }

    /// orient3d's public body before the generator.
    #[inline(never)]
    fn before_orient3d(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
        let [a, b, c, d] = [a, b, c, d].map(xyz);
        let fast = orient3d_fast(a, b, c, d).map(Sign::of_i128);
        fast_else_total(fast, || orient3d_total(a, b, c, d))
    }

    /// incircle's public body before the generator.
    #[inline(never)]
    fn before_incircle(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
        let [a, b, c, d] = [a, b, c, d].map(xy);
        let fast = incircle_fast(a, b, c, d).map(|sum| sum.signum());
        fast_else_total(fast, || incircle_total(a, b, c, d))
    }

    /// insphere's public body before the generator.
    #[inline(never)]
    fn before_insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
        let [a, b, c, d, e] = [a, b, c, d, e].map(xyz);
        let fast = insphere_fast(a, b, c, d, e).map(|sum| sum.signum());
        fast_else_total(fast, || insphere_total(a, b, c, d, e))
    }

    /// The public function itself, behind the same kind of never-inlined entry as `before_*`.
    #[inline(never)]
    fn now_orient2d(a: P2, b: P2, c: P2) -> Result<Sign, NumError> {
        super::orient2d(a, b, c)
    }

    #[inline(never)]
    fn now_orient3d(a: P3, b: P3, c: P3, d: P3) -> Result<Sign, NumError> {
        super::orient3d(a, b, c, d)
    }

    #[inline(never)]
    fn now_incircle(a: P2, b: P2, c: P2, d: P2) -> Result<Sign, NumError> {
        super::incircle(a, b, c, d)
    }

    #[inline(never)]
    fn now_insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Result<Sign, NumError> {
        super::insphere(a, b, c, d, e)
    }

    /// Nanoseconds per call of `f` over `inputs`, one timed pass.
    fn pass<T: Copy>(inputs: &[T], mut f: impl FnMut(T) -> Sign) -> f64 {
        let start = Instant::now();
        let mut acc = 0i64;
        for &x in inputs {
            acc += i64::from(f(black_box(x)).as_i8());
        }
        black_box(acc);
        start.elapsed().as_nanos() as f64 / inputs.len() as f64
    }

    /// Keeps the smaller time.
    fn keep_best(best: &mut f64, ns: f64) {
        if ns < *best {
            *best = ns;
        }
    }

    /// The variants timed for each predicate, in this order.
    const VARIANTS: usize = 5;

    /// One line of the benchmark, in nanoseconds per call net of the timing loop's own cost
    /// (`overhead`, a call that only reads its input). "kernel" is the evaluation alone, inlined
    /// into the timing loop: the hand-written fast path that answered every call before this
    /// piece, the generated exact path alone, and the generated filter followed, when it cannot
    /// decide, by the generated exact path. "public" is the public function, called: its body
    /// before this piece against the function as it is now. A ratio below 1 means faster than
    /// the hand-written path or the old body.
    fn report(name: &str, inputs: &str, best: [f64; VARIANTS], overhead: f64) {
        let [hand, exact, filtered, before, now] = best.map(|ns| ns - overhead);
        println!(
            "{name:<9} {inputs:<11} kernel: hand-written {hand:7.2} ns, exact {exact:7.2} ns \
             ({:.3}), filter then exact {filtered:7.2} ns ({:.3}); public: before {before:7.2} ns, \
             now {now:7.2} ns ({:.3}) (loop {overhead:.2} ns)",
            exact / hand,
            filtered / hand,
            now / before
        );
    }

    /// The benchmark: each predicate's generated routes against the hand-written fast path that
    /// answered every public call before the generator, on random points (general position, where
    /// the filter answers) and on degenerate ones (points of one lattice line or plane, so the
    /// value is exactly zero two times in three and one step off otherwise: left to the exact
    /// path, the common case in buildings, chapter 23.2). Its figures are what the routing in
    /// this module is decided on (module documentation). Every variant is timed in turn in every
    /// round, and each keeps its best round, so a busy machine slows them alike. Run with
    /// `cargo test -p geom-num --release --lib generated_route -- --ignored --nocapture`, and on
    /// WebAssembly with `--target wasm32-wasip1` under wasmtime.
    #[test]
    #[ignore = "a benchmark: run in release with --ignored --nocapture"]
    #[allow(
        clippy::too_many_lines,
        reason = "four predicates, each timed five ways, in one place"
    )]
    fn generated_route_against_the_hand_written_fast_path() {
        const N: usize = 1 << 14;
        const ROUNDS: usize = 60;
        let mut rng = Rng::new(0x6727_0000_0000_0002);
        let r2 = |rng: &mut Rng| [rng.coord(), rng.coord()];
        let r3 = |rng: &mut Rng| [rng.coord(), rng.coord(), rng.coord()];
        // A small lattice vector: each component below 2^19 in magnitude.
        let small = |rng: &mut Rng| (rng.next() % (1 << 20)) as i64 - (1 << 19);
        // A base point within 2^39 of the origin, so base + k·u + l·v (|k|, |l| < 2^19) stays on
        // the grid.
        let base = |rng: &mut Rng| (rng.next() % (1 << 40)) as i64 - (1 << 39);
        for degenerate in [false, true] {
            let label = if degenerate { "degenerate" } else { "random" };
            // 2D: random points, or four points of one lattice line (every value exactly zero),
            // the third moved one step in x one time in three.
            let pts2: Vec<[[i64; 2]; 4]> = (0..N)
                .map(|_| {
                    if !degenerate {
                        return [r2(&mut rng), r2(&mut rng), r2(&mut rng), r2(&mut rng)];
                    }
                    let a = [base(&mut rng), base(&mut rng)];
                    let u = [small(&mut rng), small(&mut rng)];
                    let at = |rng: &mut Rng| {
                        let k = small(rng);
                        [a[0] + k * u[0], a[1] + k * u[1]]
                    };
                    let (b, mut c, d) = (at(&mut rng), at(&mut rng), at(&mut rng));
                    if rng.next() % 3 == 0 {
                        c[0] += 1;
                    }
                    [a, b, c, d]
                })
                .collect();
            let gp2: Vec<[P2; 4]> = pts2.iter().map(|p| p.map(p2)).collect();
            // 3D: random points, or five points of one lattice plane (every value exactly zero),
            // the fourth moved one step in x one time in three.
            let pts3: Vec<[[i64; 3]; 5]> = (0..N)
                .map(|_| {
                    if !degenerate {
                        return std::array::from_fn(|_| r3(&mut rng));
                    }
                    let a = [base(&mut rng), base(&mut rng), base(&mut rng)];
                    let u = [small(&mut rng), small(&mut rng), small(&mut rng)];
                    let v = [small(&mut rng), small(&mut rng), small(&mut rng)];
                    let at = |rng: &mut Rng| {
                        let (k, l) = (small(rng) >> 1, small(rng) >> 1);
                        std::array::from_fn(|i| a[i] + k * u[i] + l * v[i])
                    };
                    let (b, c, mut d, e) = (at(&mut rng), at(&mut rng), at(&mut rng), at(&mut rng));
                    if rng.next() % 3 == 0 {
                        d[0] += 1;
                    }
                    [a, b, c, d, e]
                })
                .collect();
            let gp3: Vec<[P3; 5]> = pts3.iter().map(|p| p.map(p3)).collect();

            let mut best = [[f64::INFINITY; VARIANTS]; 4];
            let mut overhead = [f64::INFINITY; 2];
            for _ in 0..ROUNDS {
                let [o2, ic, o3, is] = &mut best;
                keep_best(
                    &mut overhead[0],
                    pass(&gp2, |[a, _, _, _]| Sign::of_i64(a.x.get())),
                );
                keep_best(
                    &mut overhead[1],
                    pass(&gp3, |[a, _, _, _, _]| Sign::of_i64(a.x.get())),
                );
                // orient2d.
                keep_best(
                    &mut o2[0],
                    pass(&pts2, |[a, b, c, _]| {
                        let fast = orient2d_fast(a, b, c).map(Sign::of_i128);
                        fast_else_total(fast, || orient2d_total(a, b, c)).unwrap()
                    }),
                );
                keep_best(
                    &mut o2[1],
                    pass(&gp2, |[a, b, c, _]| orient2d::exact(a, b, c).unwrap()),
                );
                keep_best(
                    &mut o2[2],
                    pass(&gp2, |[a, b, c, _]| {
                        orient2d::filter(a, b, c)
                            .or_else(|| orient2d::exact(a, b, c))
                            .unwrap()
                    }),
                );
                keep_best(
                    &mut o2[3],
                    pass(&gp2, |[a, b, c, _]| before_orient2d(a, b, c).unwrap()),
                );
                keep_best(
                    &mut o2[4],
                    pass(&gp2, |[a, b, c, _]| now_orient2d(a, b, c).unwrap()),
                );
                // incircle.
                keep_best(
                    &mut ic[0],
                    pass(&pts2, |[a, b, c, d]| {
                        let fast = incircle_fast(a, b, c, d).map(|s| s.signum());
                        fast_else_total(fast, || incircle_total(a, b, c, d)).unwrap()
                    }),
                );
                keep_best(
                    &mut ic[1],
                    pass(&gp2, |[a, b, c, d]| incircle::exact(a, b, c, d).unwrap()),
                );
                keep_best(
                    &mut ic[2],
                    pass(&gp2, |[a, b, c, d]| {
                        incircle::filter(a, b, c, d)
                            .or_else(|| incircle::exact(a, b, c, d))
                            .unwrap()
                    }),
                );
                keep_best(
                    &mut ic[3],
                    pass(&gp2, |[a, b, c, d]| before_incircle(a, b, c, d).unwrap()),
                );
                keep_best(
                    &mut ic[4],
                    pass(&gp2, |[a, b, c, d]| now_incircle(a, b, c, d).unwrap()),
                );
                // orient3d.
                keep_best(
                    &mut o3[0],
                    pass(&pts3, |[a, b, c, d, _]| {
                        let fast = orient3d_fast(a, b, c, d).map(Sign::of_i128);
                        fast_else_total(fast, || orient3d_total(a, b, c, d)).unwrap()
                    }),
                );
                keep_best(
                    &mut o3[1],
                    pass(&gp3, |[a, b, c, d, _]| orient3d::exact(a, b, c, d).unwrap()),
                );
                keep_best(
                    &mut o3[2],
                    pass(&gp3, |[a, b, c, d, _]| {
                        orient3d::filter(a, b, c, d)
                            .or_else(|| orient3d::exact(a, b, c, d))
                            .unwrap()
                    }),
                );
                keep_best(
                    &mut o3[3],
                    pass(&gp3, |[a, b, c, d, _]| before_orient3d(a, b, c, d).unwrap()),
                );
                keep_best(
                    &mut o3[4],
                    pass(&gp3, |[a, b, c, d, _]| now_orient3d(a, b, c, d).unwrap()),
                );
                // insphere.
                keep_best(
                    &mut is[0],
                    pass(&pts3, |[a, b, c, d, e]| {
                        let fast = insphere_fast(a, b, c, d, e).map(|s| s.signum());
                        fast_else_total(fast, || insphere_total(a, b, c, d, e)).unwrap()
                    }),
                );
                keep_best(
                    &mut is[1],
                    pass(&gp3, |[a, b, c, d, e]| {
                        insphere::exact(a, b, c, d, e).unwrap()
                    }),
                );
                keep_best(
                    &mut is[2],
                    pass(&gp3, |[a, b, c, d, e]| {
                        insphere::filter(a, b, c, d, e)
                            .or_else(|| insphere::exact(a, b, c, d, e))
                            .unwrap()
                    }),
                );
                keep_best(
                    &mut is[3],
                    pass(&gp3, |[a, b, c, d, e]| {
                        before_insphere(a, b, c, d, e).unwrap()
                    }),
                );
                keep_best(
                    &mut is[4],
                    pass(&gp3, |[a, b, c, d, e]| now_insphere(a, b, c, d, e).unwrap()),
                );
            }
            let [o2, ic, o3, is] = best;
            report("orient2d", label, o2, overhead[0]);
            report("orient3d", label, o3, overhead[1]);
            report("incircle", label, ic, overhead[0]);
            report("insphere", label, is, overhead[1]);
        }
    }
}
