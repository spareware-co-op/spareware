//! A Verus-annotated twin of `geom-num`'s plane-based geometry (`src/plane.rs`), for `verus` to
//! verify (chapter 28.7 of the kernel architecture; piece G39).
//!
//! It proves, for grid points (|v| ≤ 2^40 − 1):
//!
//! - `Plane::through` gives the plane n · p + d = 0 with n = (b − a) × (c − a) and d = −n · a,
//!   exactly, within the budget |`n_i`| < 2^83, |d| < 2^125; `Degenerate` exactly when n = 0; and
//!   never `BudgetExceeded`.
//! - `Plane::side` gives the sign of n · p + d, and never an error, for a plane within the budget.
//! - `Tpi::new` makes the point of three planes, `Degenerate` exactly when the weight
//!   w = n₁ · (n₂ × n₃) is zero, and never `BudgetExceeded`; `Tpi::homogeneous` gives
//!   X = −(d₁ (n₂ × n₃) + d₂ (n₃ × n₁) + d₃ (n₁ × n₂)) and w exactly, in `I320` and `I256`, and
//!   X / w lies on all three planes.
//! - `Lpi::new` makes the point where the line p q meets a plane, `Degenerate` exactly when the
//!   weight w = s(q) − s(p) is zero (s the plane's side value), and never `BudgetExceeded`;
//!   `Lpi::homogeneous` gives X = s(q) p − s(p) q and w exactly, and X / w lies on the plane.
//! - The hand-written classifications of both points against another plane give the sign of
//!   n · X + d w times the sign of w, never an error: in `I384` for every plane and point within
//!   the budget, and on the total path in `I512` for any values the types hold.
//!
//! The functions below the `verus!` block, with their `#[verus_spec]` attributes and their `proof!`
//! and `proof_decl!` blocks removed, are token for token those of `src/plane.rs`: `geom-gen`'s
//! `tests/verus_twin.rs` compares them. `Plane::through`, `Tpi::new` and `Lpi::new`, associated
//! functions without `self`, which `#[verus_spec]` cannot annotate, are written inside the `verus!`
//! block instead, and the test compares each one's doc comment, attributes, visibility, parameters
//! and body. Also inside the block are models of `Plane`, `Tpi` and `Lpi` with the same fields, the
//! budget's limits (whose derivation `src/plane.rs` asserts when it compiles, and the test reads),
//! and the definitions and lemmas the proofs use.
//!
//! It is a module of `geom-num`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-num/verus/lib.rs`, with the Verus release `kernel-dev.yml`
//! pins; its `verus` job does this on every pull request.

use vstd::prelude::*;

use crate::model::*;
use crate::sign::*;
use crate::wide::*;

verus! {

/// The budget for each plane normal component, `NORMAL_LIMIT` in `src/plane.rs`: 2^83.
const NORMAL_LIMIT: u128 = 0x800000000000000000000;

/// The budget for a plane offset, `OFFSET_LIMIT` in `src/plane.rs`: 2^125.
const OFFSET_LIMIT: u128 = 0x20000000000000000000000000000000;

/// Whether v fits an `i128`.
pub open spec fn fits128(v: int) -> bool {
    i128::MIN as int <= v <= i128::MAX as int
}

/// Whether every coordinate of p is on the grid.
pub open spec fn on_grid3(p: P3) -> bool {
    p.x.on_grid() && p.y.on_grid() && p.z.on_grid()
}

/// n · p for a normal n and a grid point p, over the integers.
pub open spec fn dot3(n: (int, int, int), p: P3) -> int {
    n.0 * p.x@ + n.1 * p.y@ + n.2 * p.z@
}

/// The normal (b − a) × (c − a) of the plane through a, b and c, over the integers.
pub open spec fn normal(a: P3, b: P3, c: P3) -> (int, int, int) {
    let (ux, uy, uz) = (b.x@ - a.x@, b.y@ - a.y@, b.z@ - a.z@);
    let (vx, vy, vz) = (c.x@ - a.x@, c.y@ - a.y@, c.z@ - a.z@);
    (uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx)
}

/// A model of `geom_num::plane::Plane`: the points p with n · p + d = 0.
#[derive(Clone, Copy)]
pub struct Plane {
    n: [i128; 3],
    d: i128,
}

impl Plane {
    /// The normal, over the integers.
    pub closed spec fn normal(self) -> (int, int, int) {
        (self.n@[0] as int, self.n@[1] as int, self.n@[2] as int)
    }

    /// The offset, over the integers.
    pub closed spec fn offset(self) -> int {
        self.d as int
    }

    /// Whether the plane is within the budget `Plane::through` checks: |`n_i`| < 2^83, |d| < 2^125.
    pub closed spec fn wf(self) -> bool {
        &&& -0x800000000000000000000 < self.n@[0] < 0x800000000000000000000
        &&& -0x800000000000000000000 < self.n@[1] < 0x800000000000000000000
        &&& -0x800000000000000000000 < self.n@[2] < 0x800000000000000000000
        &&& -0x20000000000000000000000000000000 < self.d < 0x20000000000000000000000000000000
    }
}

/// A coordinate difference of grid points is at most 2M in magnitude, and a product of two at most
/// (2M)².
pub proof fn lemma_difference_product(u: int, v: int)
    requires
        -2 * 1099511627775 <= u <= 2 * 1099511627775,
        -2 * 1099511627775 <= v <= 2 * 1099511627775,
    ensures
        -(4 * 1099511627775 * 1099511627775) <= u * v <= 4 * 1099511627775 * 1099511627775,
{
    lemma_product_bound(u, v, 2 * 1099511627775int, 2 * 1099511627775int);
}

/// The bounds of every step of `coefficients` for grid points: each product of coordinate
/// differences at most 4M², each normal component at most 8M² (below 2^83), each product of a
/// normal component and a coordinate at most 8M³, and n · a at most 24M³ (below 2^125).
pub proof fn lemma_coefficient_bounds(a: P3, b: P3, c: P3)
    requires
        on_grid3(a),
        on_grid3(b),
        on_grid3(c),
    ensures
        ({
            let m = 1099511627775int;
            let (ux, uy, uz) = (b.x@ - a.x@, b.y@ - a.y@, b.z@ - a.z@);
            let (vx, vy, vz) = (c.x@ - a.x@, c.y@ - a.y@, c.z@ - a.z@);
            let n = normal(a, b, c);
            &&& -4 * m * m <= uy * vz <= 4 * m * m
            &&& -4 * m * m <= uz * vy <= 4 * m * m
            &&& -4 * m * m <= uz * vx <= 4 * m * m
            &&& -4 * m * m <= ux * vz <= 4 * m * m
            &&& -4 * m * m <= ux * vy <= 4 * m * m
            &&& -4 * m * m <= uy * vx <= 4 * m * m
            &&& -8 * m * m <= n.0 <= 8 * m * m
            &&& -8 * m * m <= n.1 <= 8 * m * m
            &&& -8 * m * m <= n.2 <= 8 * m * m
            &&& -8 * m * m * m <= n.0 * a.x@ <= 8 * m * m * m
            &&& -8 * m * m * m <= n.1 * a.y@ <= 8 * m * m * m
            &&& -8 * m * m * m <= n.2 * a.z@ <= 8 * m * m * m
        }),
{
    let m = 1099511627775int;
    lemma_on_grid(a.x);
    lemma_on_grid(a.y);
    lemma_on_grid(a.z);
    lemma_on_grid(b.x);
    lemma_on_grid(b.y);
    lemma_on_grid(b.z);
    lemma_on_grid(c.x);
    lemma_on_grid(c.y);
    lemma_on_grid(c.z);
    let (ux, uy, uz) = (b.x@ - a.x@, b.y@ - a.y@, b.z@ - a.z@);
    let (vx, vy, vz) = (c.x@ - a.x@, c.y@ - a.y@, c.z@ - a.z@);
    lemma_difference_product(uy, vz);
    lemma_difference_product(uz, vy);
    lemma_difference_product(uz, vx);
    lemma_difference_product(ux, vz);
    lemma_difference_product(ux, vy);
    lemma_difference_product(uy, vx);
    let n = normal(a, b, c);
    lemma_product_bound(n.0, a.x@, 8 * m * m, m);
    lemma_product_bound(n.1, a.y@, 8 * m * m, m);
    lemma_product_bound(n.2, a.z@, 8 * m * m, m);
}

/// The bounds of every step of n · p + d for a plane within the budget and a grid point: each
/// product below 2^123, every partial sum below 2^126.
pub proof fn lemma_side_bounds(n: (int, int, int), d: int, p: P3)
    requires
        -0x800000000000000000000 < n.0 < 0x800000000000000000000,
        -0x800000000000000000000 < n.1 < 0x800000000000000000000,
        -0x800000000000000000000 < n.2 < 0x800000000000000000000,
        -0x20000000000000000000000000000000 < d < 0x20000000000000000000000000000000,
        on_grid3(p),
    ensures
        -0x8000000000000000000000000000000 < n.0 * p.x@ < 0x8000000000000000000000000000000,
        -0x8000000000000000000000000000000 < n.1 * p.y@ < 0x8000000000000000000000000000000,
        -0x8000000000000000000000000000000 < n.2 * p.z@ < 0x8000000000000000000000000000000,
{
    lemma_on_grid(p.x);
    lemma_on_grid(p.y);
    lemma_on_grid(p.z);
    lemma_product_bound(n.0, p.x@, 0x7FFFFFFFFFFFFFFFFFFFFint, 1099511627775int);
    lemma_product_bound(n.1, p.y@, 0x7FFFFFFFFFFFFFFFFFFFFint, 1099511627775int);
    lemma_product_bound(n.2, p.z@, 0x7FFFFFFFFFFFFFFFFFFFFint, 1099511627775int);
}



// ---- Three-plane points (piece G39).

/// The limbs of `I384`, the budgeted width of `Plane::side_of_tpi`: `BUDGET_LIMBS` in `src/plane.rs`.
pub(crate) const BUDGET_LIMBS: usize = 6;

/// The limbs of `I512`, the total path's width: `TOTAL_LIMBS` in `src/plane.rs`.
pub(crate) const TOTAL_LIMBS: usize = 8;

/// 2^125, the offset budget.
pub open spec fn bound_125() -> int {
    42535295865117307932921825928971026432int
}

/// 2^167, the bound of a cross product of two normals within the budget.
pub open spec fn bound_167() -> int {
    187072209578355573530071658587684226515959365500928int
}

/// 2^252, the bound of a three-plane weight.
pub open spec fn bound_252() -> int {
    7237005577332262213973186563042994240829374041602535252466099000494570602496int
}

/// 2^294, the bound of a three-plane numerator.
pub open spec fn bound_294() -> int {
    31828687130226345097944463881396533766429193651030253916189694521162207808802136034115584int
}

/// a × b over the integers.
pub open spec fn cross3(a: (int, int, int), b: (int, int, int)) -> (int, int, int) {
    (a.1 * b.2 - a.2 * b.1, a.2 * b.0 - a.0 * b.2, a.0 * b.1 - a.1 * b.0)
}

/// a · b over the integers.
pub open spec fn dot_t(a: (int, int, int), b: (int, int, int)) -> int {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}

/// The weight w = `n_p` · (`n_q` × `n_r`) of the point where planes p, q and r meet.
pub open spec fn tpi_w(p: Plane, q: Plane, r: Plane) -> int {
    dot_t(p.normal(), cross3(q.normal(), r.normal()))
}

/// The numerators X = −(`d_p` (`n_q` × `n_r`) + `d_q` (`n_r` × `n_p`) + `d_r` (`n_p` × `n_q`)).
pub open spec fn tpi_x(p: Plane, q: Plane, r: Plane) -> (int, int, int) {
    let (a, b, c) = (cross3(q.normal(), r.normal()), cross3(r.normal(), p.normal()), cross3(p.normal(), q.normal()));
    let (dp, dq, dr) = (p.offset(), q.offset(), r.offset());
    (-(dp * a.0 + dq * b.0 + dr * c.0), -(dp * a.1 + dq * b.1 + dr * c.1), -(dp * a.2 + dq * b.2 + dr * c.2))
}

/// Whether x and w are the homogeneous coordinates of t's point, within their bounds.
pub open spec fn tpi_coordinates(t: Tpi, x: [I320; 3], w: I256) -> bool {
    let (p, q, r) = (t.plane(0), t.plane(1), t.plane(2));
    &&& w.value() == tpi_w(p, q, r)
    &&& (x@[0].value(), x@[1].value(), x@[2].value()) == tpi_x(p, q, r)
    &&& -bound_252() < w.value() < bound_252()
    &&& -bound_294() < x@[0].value() < bound_294()
    &&& -bound_294() < x@[1].value() < bound_294()
    &&& -bound_294() < x@[2].value() < bound_294()
}

/// The side of plane s that t's point lies on: the sign of n · X + d · w times the sign of w.
pub open spec fn tpi_side_sign(s: Plane, t: Tpi) -> Sign {
    let (p, q, r) = (t.plane(0), t.plane(1), t.plane(2));
    sign_mul(sign_of(s.offset() * tpi_w(p, q, r) + dot_t(s.normal(), tpi_x(p, q, r))), sign_of(tpi_w(p, q, r)))
}

/// A triple product with a repeated vector is zero: a · (a × b) = 0.
pub proof fn lemma_triple_repeated_left(a: (int, int, int), b: (int, int, int))
    ensures
        dot_t(a, cross3(a, b)) == 0,
{
    let (a0, a1, a2, b0, b1, b2) = (a.0, a.1, a.2, b.0, b.1, b.2);
    assert(a0 * (a1 * b2 - a2 * b1) + a1 * (a2 * b0 - a0 * b2) + a2 * (a0 * b1 - a1 * b0) == 0) by (nonlinear_arith);
}

/// A triple product with a repeated vector is zero: b · (a × b) = 0.
pub proof fn lemma_triple_repeated_right(a: (int, int, int), b: (int, int, int))
    ensures
        dot_t(b, cross3(a, b)) == 0,
{
    let (a0, a1, a2, b0, b1, b2) = (a.0, a.1, a.2, b.0, b.1, b.2);
    assert(b0 * (a1 * b2 - a2 * b1) + b1 * (a2 * b0 - a0 * b2) + b2 * (a0 * b1 - a1 * b0) == 0) by (nonlinear_arith);
}

/// The triple product is unchanged by a cyclic shift: b · (c × a) = c · (a × b) = a · (b × c).
/// Each product of three is expanded on its own, so the solver only adds the six terms.
pub proof fn lemma_triple_cyclic(a: (int, int, int), b: (int, int, int), c: (int, int, int))
    ensures
        dot_t(b, cross3(c, a)) == dot_t(a, cross3(b, c)),
        dot_t(c, cross3(a, b)) == dot_t(a, cross3(b, c)),
{
    let (a0, a1, a2, b0, b1, b2, c0, c1, c2) = (a.0, a.1, a.2, b.0, b.1, b.2, c.0, c.1, c.2);
    assert(a0 * (b1 * c2 - b2 * c1) == a0 * b1 * c2 - a0 * b2 * c1) by (nonlinear_arith);
    assert(a1 * (b2 * c0 - b0 * c2) == a1 * b2 * c0 - a1 * b0 * c2) by (nonlinear_arith);
    assert(a2 * (b0 * c1 - b1 * c0) == a2 * b0 * c1 - a2 * b1 * c0) by (nonlinear_arith);
    assert(b0 * (c1 * a2 - c2 * a1) == a2 * b0 * c1 - a1 * b0 * c2) by (nonlinear_arith);
    assert(b1 * (c2 * a0 - c0 * a2) == a0 * b1 * c2 - a2 * b1 * c0) by (nonlinear_arith);
    assert(b2 * (c0 * a1 - c1 * a0) == a1 * b2 * c0 - a0 * b2 * c1) by (nonlinear_arith);
    assert(c0 * (a1 * b2 - a2 * b1) == a1 * b2 * c0 - a2 * b1 * c0) by (nonlinear_arith);
    assert(c1 * (a2 * b0 - a0 * b2) == a2 * b0 * c1 - a0 * b2 * c1) by (nonlinear_arith);
    assert(c2 * (a0 * b1 - a1 * b0) == a0 * b1 * c2 - a1 * b0 * c2) by (nonlinear_arith);
}

/// The dot product is linear: n · −(p u + q v + r t) = −(p (n · u) + q (n · v) + r (n · t)).
pub proof fn lemma_dot_combination(n: (int, int, int), p: int, u: (int, int, int), q: int, v: (int, int, int), r: int, t: (int, int, int))
    ensures
        dot_t(n, (-(p * u.0 + q * v.0 + r * t.0), -(p * u.1 + q * v.1 + r * t.1), -(p * u.2 + q * v.2 + r * t.2)))
            == -(p * dot_t(n, u) + q * dot_t(n, v) + r * dot_t(n, t)),
{
    let (n0, n1, n2) = (n.0, n.1, n.2);
    assert(n0 * -(p * u.0 + q * v.0 + r * t.0) == -(p * (n0 * u.0) + q * (n0 * v.0) + r * (n0 * t.0))) by (nonlinear_arith);
    assert(n1 * -(p * u.1 + q * v.1 + r * t.1) == -(p * (n1 * u.1) + q * (n1 * v.1) + r * (n1 * t.1))) by (nonlinear_arith);
    assert(n2 * -(p * u.2 + q * v.2 + r * t.2) == -(p * (n2 * u.2) + q * (n2 * v.2) + r * (n2 * t.2))) by (nonlinear_arith);
    assert(p * (n0 * u.0 + n1 * u.1 + n2 * u.2) == p * (n0 * u.0) + p * (n1 * u.1) + p * (n2 * u.2)) by (nonlinear_arith);
    assert(q * (n0 * v.0 + n1 * v.1 + n2 * v.2) == q * (n0 * v.0) + q * (n1 * v.1) + q * (n2 * v.2)) by (nonlinear_arith);
    assert(r * (n0 * t.0 + n1 * t.1 + n2 * t.2) == r * (n0 * t.0) + r * (n1 * t.1) + r * (n2 * t.2)) by (nonlinear_arith);
}

/// The point X / w lies on each of its three planes: `n_k` · X + `d_k` w = 0 (Cramer's rule).
pub proof fn lemma_tpi_on_planes(p: Plane, q: Plane, r: Plane)
    ensures
        dot_t(p.normal(), tpi_x(p, q, r)) + p.offset() * tpi_w(p, q, r) == 0,
        dot_t(q.normal(), tpi_x(p, q, r)) + q.offset() * tpi_w(p, q, r) == 0,
        dot_t(r.normal(), tpi_x(p, q, r)) + r.offset() * tpi_w(p, q, r) == 0,
{
    let (a, b, c) = (p.normal(), q.normal(), r.normal());
    let (dp, dq, dr) = (p.offset(), q.offset(), r.offset());
    let (u, v, t) = (cross3(b, c), cross3(c, a), cross3(a, b));
    lemma_triple_cyclic(a, b, c);
    lemma_triple_repeated_left(a, b);
    lemma_triple_repeated_right(c, a);
    lemma_triple_repeated_left(b, c);
    lemma_triple_repeated_right(a, b);
    lemma_triple_repeated_left(c, a);
    lemma_triple_repeated_right(b, c);
    lemma_dot_combination(a, dp, u, dq, v, dr, t);
    lemma_dot_combination(b, dp, u, dq, v, dr, t);
    lemma_dot_combination(c, dp, u, dq, v, dr, t);
}

/// Bounds of the three-plane quantities for planes within the budget: each cross-product component
/// below 2^167, w below 2^252, each numerator below 2^294.
pub proof fn lemma_tpi_bounds(p: Plane, q: Plane, r: Plane)
    requires
        p.wf(),
        q.wf(),
        r.wf(),
    ensures
        ({
            let (a, b, c) = (cross3(q.normal(), r.normal()), cross3(r.normal(), p.normal()), cross3(p.normal(), q.normal()));
            &&& -bound_167() < a.0 < bound_167() &&& -bound_167() < a.1 < bound_167() &&& -bound_167() < a.2 < bound_167()
            &&& -bound_167() < b.0 < bound_167() &&& -bound_167() < b.1 < bound_167() &&& -bound_167() < b.2 < bound_167()
            &&& -bound_167() < c.0 < bound_167() &&& -bound_167() < c.1 < bound_167() &&& -bound_167() < c.2 < bound_167()
            &&& -bound_252() < tpi_w(p, q, r) < bound_252()
            &&& -bound_294() < tpi_x(p, q, r).0 < bound_294()
            &&& -bound_294() < tpi_x(p, q, r).1 < bound_294()
            &&& -bound_294() < tpi_x(p, q, r).2 < bound_294()
        }),
{
    let n83 = 9671406556917033397649407int;
    let d125 = 42535295865117307932921825928971026431int;
    let (np, nq, nr) = (p.normal(), q.normal(), r.normal());
    lemma_product_bound(nq.1, nr.2, n83, n83);
    lemma_product_bound(nq.2, nr.1, n83, n83);
    lemma_product_bound(nq.2, nr.0, n83, n83);
    lemma_product_bound(nq.0, nr.2, n83, n83);
    lemma_product_bound(nq.0, nr.1, n83, n83);
    lemma_product_bound(nq.1, nr.0, n83, n83);
    lemma_product_bound(nr.1, np.2, n83, n83);
    lemma_product_bound(nr.2, np.1, n83, n83);
    lemma_product_bound(nr.2, np.0, n83, n83);
    lemma_product_bound(nr.0, np.2, n83, n83);
    lemma_product_bound(nr.0, np.1, n83, n83);
    lemma_product_bound(nr.1, np.0, n83, n83);
    lemma_product_bound(np.1, nq.2, n83, n83);
    lemma_product_bound(np.2, nq.1, n83, n83);
    lemma_product_bound(np.2, nq.0, n83, n83);
    lemma_product_bound(np.0, nq.2, n83, n83);
    lemma_product_bound(np.0, nq.1, n83, n83);
    lemma_product_bound(np.1, nq.0, n83, n83);
    let (a, b, c) = (cross3(nq, nr), cross3(nr, np), cross3(np, nq));
    let b167 = 2 * (n83 * n83);
    lemma_product_bound(np.0, a.0, n83, b167);
    lemma_product_bound(np.1, a.1, n83, b167);
    lemma_product_bound(np.2, a.2, n83, b167);
    let (dp, dq, dr) = (p.offset(), q.offset(), r.offset());
    lemma_product_bound(dp, a.0, d125, b167);
    lemma_product_bound(dq, b.0, d125, b167);
    lemma_product_bound(dr, c.0, d125, b167);
    lemma_product_bound(dp, a.1, d125, b167);
    lemma_product_bound(dq, b.1, d125, b167);
    lemma_product_bound(dr, c.1, d125, b167);
    lemma_product_bound(dp, a.2, d125, b167);
    lemma_product_bound(dq, b.2, d125, b167);
    lemma_product_bound(dr, c.2, d125, b167);
}

/// The steps of `classify` fit `Wide<L>`: in the budget for L ≥ 6, for any values of the types
/// for L ≥ 8.
pub proof fn lemma_classify_bounds<const L: usize>(n: &[i128; 3], d: i128, x: &[I320; 3], w: &I256)
    requires
        6 <= L <= 4096,
    ensures
        ({
            let h = base(L as nat) / 2;
            let (p0, p1, p2) = (n@[0] as int * x@[0].value(), n@[1] as int * x@[1].value(), n@[2] as int * x@[2].value());
            let dw = d as int * w.value();
            (L >= 8 || (-0x800000000000000000000 < n@[0] < 0x800000000000000000000
                && -0x800000000000000000000 < n@[1] < 0x800000000000000000000
                && -0x800000000000000000000 < n@[2] < 0x800000000000000000000
                && -0x20000000000000000000000000000000 < d < 0x20000000000000000000000000000000
                && -bound_294() < x@[0].value() < bound_294() && -bound_294() < x@[1].value() < bound_294()
                && -bound_294() < x@[2].value() < bound_294() && -bound_252() < w.value() < bound_252()))
            ==> {
                &&& fits(w.value(), L as nat) &&& fits(x@[0].value(), L as nat)
                &&& fits(x@[1].value(), L as nat) &&& fits(x@[2].value(), L as nat)
                &&& fits(dw, L as nat) &&& fits(p0, L as nat) &&& fits(p1, L as nat) &&& fits(p2, L as nat)
                &&& fits(dw + p0, L as nat) &&& fits(dw + p0 + p1, L as nat) &&& fits(dw + p0 + p1 + p2, L as nat)
            }
        }),
{
    lemma_half_base_4();
    lemma_half_base_5();
    lemma_half_base_6();
    lemma_half_base_8();
    lemma_base_monotone(6, L as nat);
    lemma_value_bound(*w);
    lemma_value_bound(x@[0]);
    lemma_value_bound(x@[1]);
    lemma_value_bound(x@[2]);
    if L >= 8 {
        lemma_base_monotone(8, L as nat);
        let (m127, m319, m255) = (
            170141183460469231731687303715884105728int,
            1067993517960455041197510853084776057301352261178326384973520803911109862890320275011481043468288int,
            57896044618658097711785492504343953926634992332820282019728792003956564819968int,
        );
        lemma_product_bound(n@[0] as int, x@[0].value(), m127, m319);
        lemma_product_bound(n@[1] as int, x@[1].value(), m127, m319);
        lemma_product_bound(n@[2] as int, x@[2].value(), m127, m319);
        lemma_product_bound(d as int, w.value(), m127, m255);
    } else if -0x800000000000000000000 < n@[0] < 0x800000000000000000000
        && -0x800000000000000000000 < n@[1] < 0x800000000000000000000
        && -0x800000000000000000000 < n@[2] < 0x800000000000000000000
        && -0x20000000000000000000000000000000 < d < 0x20000000000000000000000000000000
        && -bound_294() < x@[0].value() < bound_294() && -bound_294() < x@[1].value() < bound_294()
        && -bound_294() < x@[2].value() < bound_294() && -bound_252() < w.value() < bound_252() {
        let (n83, d125) = (9671406556917033397649408int, 42535295865117307932921825928971026432int);
        lemma_product_bound(n@[0] as int, x@[0].value(), n83, bound_294());
        lemma_product_bound(n@[1] as int, x@[1].value(), n83, bound_294());
        lemma_product_bound(n@[2] as int, x@[2].value(), n83, bound_294());
        lemma_product_bound(d as int, w.value(), d125, bound_252());
    }
}

/// A model of `geom_num::plane::Tpi`: the point where three planes meet.
#[derive(Clone, Copy)]
pub struct Tpi {
    planes: [Plane; 3],
    weight: Sign,
}

impl Tpi {
    /// Plane k of the three.
    pub closed spec fn plane(self, k: int) -> Plane {
        self.planes@[k]
    }

    /// The stored sign of the weight.
    pub closed spec fn weight(self) -> Sign {
        self.weight
    }

    /// What `Tpi::new` makes: three planes within the budget meeting in one point, w ≠ 0, with
    /// the sign of w stored.
    pub closed spec fn wf(self) -> bool {
        &&& self.planes@[0].wf()
        &&& self.planes@[1].wf()
        &&& self.planes@[2].wf()
        &&& tpi_w(self.planes@[0], self.planes@[1], self.planes@[2]) != 0
        &&& self.weight == sign_of(tpi_w(self.planes@[0], self.planes@[1], self.planes@[2]))
    }
}



// ---- Line-plane points (piece G39).

/// 2^126, the bound of n · p + d and of n · (q − p) for a plane within the budget and grid points.
pub open spec fn bound_126() -> int {
    85070591730234615865843651857942052864int
}

/// The weight w = n · (q − p) of the point where the line from p to q meets plane pl.
pub open spec fn lpi_w(pl: Plane, p: P3, q: P3) -> int {
    dot3(pl.normal(), q) - dot3(pl.normal(), p)
}

/// n · p + d: the plane's value at grid point p.
pub open spec fn plane_at(pl: Plane, p: P3) -> int {
    dot3(pl.normal(), p) + pl.offset()
}

/// The side of plane s that l's point lies on: the sign of w₁ s₂ₚ − w₂ s₁ₚ times the sign of w₁.
pub open spec fn lpi_side_sign(s: Plane, l: Lpi) -> Sign {
    let (w1, s1p) = (lpi_w(l.plane(), l.p(), l.q()), plane_at(l.plane(), l.p()));
    let (w2, s2p) = (lpi_w(s, l.p(), l.q()), plane_at(s, l.p()));
    sign_mul(sign_of(w1 * s2p - w2 * s1p), sign_of(w1))
}

/// Bounds for a plane within the budget and grid points p, q: |n · p + d| and |n · (q − p)| below
/// 2^126.
pub proof fn lemma_lpi_bounds(pl: Plane, p: P3, q: P3)
    requires
        pl.wf(),
        on_grid3(p),
        on_grid3(q),
    ensures
        -bound_126() < plane_at(pl, p) < bound_126(),
        -bound_126() < plane_at(pl, q) < bound_126(),
        -bound_126() < lpi_w(pl, p, q) < bound_126(),
{
    lemma_side_bounds(pl.normal(), pl.offset(), p);
    lemma_side_bounds(pl.normal(), pl.offset(), q);
    let n = pl.normal();
    lemma_on_grid(p.x);
    lemma_on_grid(p.y);
    lemma_on_grid(p.z);
    lemma_on_grid(q.x);
    lemma_on_grid(q.y);
    lemma_on_grid(q.z);
    let n83 = 0x7FFFFFFFFFFFFFFFFFFFFint;
    lemma_product_bound(n.0, q.x@ - p.x@, n83, 2 * 1099511627775int);
    lemma_product_bound(n.1, q.y@ - p.y@, n83, 2 * 1099511627775int);
    lemma_product_bound(n.2, q.z@ - p.z@, n83, 2 * 1099511627775int);
    assert(n.0 * (q.x@ - p.x@) == n.0 * q.x@ - n.0 * p.x@) by (nonlinear_arith);
    assert(n.1 * (q.y@ - p.y@) == n.1 * q.y@ - n.1 * p.y@) by (nonlinear_arith);
    assert(n.2 * (q.z@ - p.z@) == n.2 * q.z@ - n.2 * p.z@) by (nonlinear_arith);
    assert(lpi_w(pl, p, q) == n.0 * (q.x@ - p.x@) + n.1 * (q.y@ - p.y@) + n.2 * (q.z@ - p.z@));
}

/// The point X / w, with X = `s_q` p − `s_p` q and w = `s_q` − `s_p`, lies on the plane (n · X + d w
/// = 0), and against a second plane n₂ · X + d₂ w = w s₂ₚ − w₂ s₁ₚ. Each product is expanded on its
/// own, so the solver only adds terms.
pub proof fn lemma_lpi_on_plane(pl: Plane, s: Plane, p: P3, q: P3)
    ensures
        ({
            let (sp, sq) = (plane_at(pl, p), plane_at(pl, q));
            let x = (sq * p.x@ - sp * q.x@, sq * p.y@ - sp * q.y@, sq * p.z@ - sp * q.z@);
            &&& dot_t(pl.normal(), x) + pl.offset() * (sq - sp) == 0
            &&& dot_t(s.normal(), x) + s.offset() * (sq - sp)
                == lpi_w(pl, p, q) * plane_at(s, p) - lpi_w(s, p, q) * sp
        }),
{
    let (n, d, m, e) = (pl.normal(), pl.offset(), s.normal(), s.offset());
    let (p0, p1, p2, q0, q1, q2) = (p.x@, p.y@, p.z@, q.x@, q.y@, q.z@);
    let (sp, sq) = (plane_at(pl, p), plane_at(pl, q));
    let (tp, tq) = (plane_at(s, p), plane_at(s, q));
    assert(sp == n.0 * p0 + n.1 * p1 + n.2 * p2 + d);
    assert(sq == n.0 * q0 + n.1 * q1 + n.2 * q2 + d);
    assert(tp == m.0 * p0 + m.1 * p1 + m.2 * p2 + e);
    assert(tq == m.0 * q0 + m.1 * q1 + m.2 * q2 + e);
    // n · X + d w = sq sp − sp sq = 0.
    assert(n.0 * (sq * p0 - sp * q0) == sq * (n.0 * p0) - sp * (n.0 * q0)) by (nonlinear_arith);
    assert(n.1 * (sq * p1 - sp * q1) == sq * (n.1 * p1) - sp * (n.1 * q1)) by (nonlinear_arith);
    assert(n.2 * (sq * p2 - sp * q2) == sq * (n.2 * p2) - sp * (n.2 * q2)) by (nonlinear_arith);
    assert(d * (sq - sp) == sq * d - sp * d) by (nonlinear_arith);
    assert(sq * (n.0 * p0) + sq * (n.1 * p1) + sq * (n.2 * p2) + sq * d == sq * sp) by (nonlinear_arith)
        requires sp == n.0 * p0 + n.1 * p1 + n.2 * p2 + d;
    assert(sp * (n.0 * q0) + sp * (n.1 * q1) + sp * (n.2 * q2) + sp * d == sp * sq) by (nonlinear_arith)
        requires sq == n.0 * q0 + n.1 * q1 + n.2 * q2 + d;
    assert(sq * sp == sp * sq) by (nonlinear_arith);
    // m · X + e w = sq tp − sp tq = (sq − sp) tp − (tq − tp) sp.
    assert(m.0 * (sq * p0 - sp * q0) == sq * (m.0 * p0) - sp * (m.0 * q0)) by (nonlinear_arith);
    assert(m.1 * (sq * p1 - sp * q1) == sq * (m.1 * p1) - sp * (m.1 * q1)) by (nonlinear_arith);
    assert(m.2 * (sq * p2 - sp * q2) == sq * (m.2 * p2) - sp * (m.2 * q2)) by (nonlinear_arith);
    assert(e * (sq - sp) == sq * e - sp * e) by (nonlinear_arith);
    assert(sq * (m.0 * p0) + sq * (m.1 * p1) + sq * (m.2 * p2) + sq * e == sq * tp) by (nonlinear_arith)
        requires tp == m.0 * p0 + m.1 * p1 + m.2 * p2 + e;
    assert(sp * (m.0 * q0) + sp * (m.1 * q1) + sp * (m.2 * q2) + sp * e == sp * tq) by (nonlinear_arith)
        requires tq == m.0 * q0 + m.1 * q1 + m.2 * q2 + e;
    assert(lpi_w(pl, p, q) == sq - sp);
    assert(lpi_w(s, p, q) == tq - tp);
    assert((sq - sp) * tp - (tq - tp) * sp == sq * tp - sp * tq) by (nonlinear_arith);
}

/// The steps of `lpi_classify` fit `Wide<L>`: in the budget for L ≥ 4, for any `i128`s for L ≥ 5.
pub proof fn lemma_lpi_classify_bounds<const L: usize>(w1: i128, s2p: i128, w2: i128, s1p: i128)
    requires
        4 <= L <= 4096,
    ensures
        (L >= 5 || (-bound_126() < w1 < bound_126() && -bound_126() < s2p < bound_126()
            && -bound_126() < w2 < bound_126() && -bound_126() < s1p < bound_126())) ==> {
            &&& fits(w1 as int * s2p as int, L as nat)
            &&& fits(w2 as int * s1p as int, L as nat)
            &&& fits(w1 as int * s2p as int - w2 as int * s1p as int, L as nat)
        },
{
    lemma_half_base_4();
    lemma_half_base_5();
    lemma_base_monotone(4, L as nat);
    if L >= 5 {
        lemma_base_monotone(5, L as nat);
        let m = 0x80000000000000000000000000000000int;
        lemma_product_bound(w1 as int, s2p as int, m, m);
        lemma_product_bound(w2 as int, s1p as int, m, m);
    } else if -bound_126() < w1 < bound_126() && -bound_126() < s2p < bound_126()
        && -bound_126() < w2 < bound_126() && -bound_126() < s1p < bound_126() {
        lemma_product_bound(w1 as int, s2p as int, bound_126(), bound_126());
        lemma_product_bound(w2 as int, s1p as int, bound_126(), bound_126());
    }
}

/// A model of `geom_num::plane::Lpi`: the point where the line from p to q meets a plane.
#[derive(Clone, Copy)]
pub struct Lpi {
    p: P3,
    q: P3,
    plane: Plane,
}

impl Lpi {
    /// The line's first point.
    pub closed spec fn p(self) -> P3 {
        self.p
    }

    /// The line's second point.
    pub closed spec fn q(self) -> P3 {
        self.q
    }

    /// The plane the line meets.
    pub closed spec fn plane(self) -> Plane {
        self.plane
    }

    /// What `Lpi::new` makes: grid points, a plane within the budget, and w ≠ 0.
    pub closed spec fn wf(self) -> bool {
        &&& self.plane.wf()
        &&& on_grid3(self.p)
        &&& on_grid3(self.q)
        &&& lpi_w(self.plane, self.p, self.q) != 0
    }
}


impl Plane {
    /// The plane through three grid points, oriented so that [`Plane::side`] is positive on the
    /// side (b − a) × (c − a) points to: a, b, c turn counter-clockwise seen from there. For any
    /// grid point p, `Plane::through(a, b, c)?.side(p)` is the 3D orientation of a, b, c, p.
    ///
    /// n = (b − a) × (c − a) and d = −n · a, exactly.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when a, b and c are collinear (repeated points included), decided
    /// exactly: n = 0. [`NumError::BudgetExceeded`] if a coefficient were over its budget
    /// (|`n_i`| ≥ 2^83 or |d| ≥ 2^125) or an `i128` step did not fit. That cannot happen for grid
    /// points, by the bit budget below; it is checked because every exact computation on planes
    /// relies on the budget, and chapter 6.2 requires the plane type to check it at construction.
    ///
    /// Bit budget (every coordinate |x| ≤ M = 2^40 − 1), every step checked, in `i128` (limit
    /// 2^127):
    /// - each difference `u_i` = `b_i` − `a_i`, `v_i` = `c_i` − `a_i` has |`u_i`| ≤ 2M = 2^41 − 2;
    /// - each product `u_j` · `v_k` is at most (2^41 − 2)^2 < 2^82 in magnitude, so each normal
    ///   component `n_i` = `u_j` · `v_k` − `u_k` · `v_j` has |`n_i`| ≤ 2 (2^41 − 2)^2 < 2^83;
    /// - each product `n_i` · `a_i` is below 2^83 · 2^40 = 2^123, so every partial sum of
    ///   d = −(`n_x` `a_x` + `n_y` `a_y` + `n_z` `a_z`) stays below 3 · 2^123 < 2^125.
    ///
    /// The largest values grid points reach are smaller. Each `n_i` is twice the signed area of the
    /// triangle a, b, c projected onto a coordinate plane, which lies in a square of side 2M, so
    /// |`n_i`| ≤ (2M)^2 = 4M² < 2^82. And d = −det[a; b; c], at most 4M³ < 2^122 in magnitude,
    /// because the largest 3 × 3 determinant with entries in [−1, 1] is 4 (reasoning on
    /// [`Tpi::homogeneous`]). Both are reached (tested).
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b and c are the points and n and d the coefficients of the formulas above"
    )]
    pub const fn through(a: P3, b: P3, c: P3) -> (r: Result<Plane, NumError>)
        requires
            on_grid3(a),
            on_grid3(b),
            on_grid3(c),
        ensures
            match r {
                Ok(pl) => pl.wf() && pl.normal() == normal(a, b, c) && pl.offset() == -dot3(normal(a, b, c), a)
                    && normal(a, b, c) != (0int, 0int, 0int),
                Err(e) => e == NumError::Degenerate && normal(a, b, c) == (0int, 0int, 0int),
            },
    {
        proof {
            lemma_coefficient_bounds(a, b, c);
        }
        match coefficients(a, b, c) {
            Some(([0, 0, 0], _)) => Err(NumError::Degenerate),
            Some((n, d)) if within_budget(&n, d) => Ok(Plane { n, d }),
            _ => Err(NumError::BudgetExceeded),
        }
    }

    /// The plane n · p + d = 0 from its coefficients, as a stored or reduced plane gives them.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] for a zero normal, and [`NumError::BudgetExceeded`] for a
    /// coefficient over its budget (|`n_i`| ≥ 2^83 or |d| ≥ 2^125): chapter 6.2 has the type refuse
    /// it here, so no plane over budget reaches an exact computation.
    pub const fn new(n: [i128; 3], d: i128) -> (r: Result<Plane, NumError>)
        ensures
            match r {
                Ok(pl) => pl.wf() && pl.normal() == (n@[0] as int, n@[1] as int, n@[2] as int)
                    && pl.offset() == d as int,
                Err(_) => true,
            },
    {
        if n[0] == 0 && n[1] == 0 && n[2] == 0 {
            Err(NumError::Degenerate)
        } else if within_budget(&n, d) {
            Ok(Plane { n, d })
        } else {
            Err(NumError::BudgetExceeded)
        }
    }
}

impl Tpi {
    /// The point where planes `p`, `q` and `r` meet.
    ///
    /// Computes the point's homogeneous coordinates once, every step checked (budget on
    /// [`Tpi::homogeneous`]), so every `Tpi` is known to have them.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the normals are linearly dependent, so the planes meet in no
    /// point or in a whole line or plane (two parallel planes, three planes through one line, a
    /// plane given twice). Decided exactly: w = det(`n_p`, `n_q`, `n_r`) = 0.
    /// [`NumError::BudgetExceeded`] if a step did not fit its type, which the budget rules out for
    /// planes made by [`Plane::through`] (module docs, "No answer is silently wrong").
    #[allow(
        clippy::many_single_char_names,
        reason = "p, q and r are the planes, and t, w and x the point, as in the formulas"
    )]
    pub fn new(p: Plane, q: Plane, r: Plane) -> (res: Result<Tpi, NumError>)
        requires
            p.wf(),
            q.wf(),
            r.wf(),
        ensures
            match res {
                Ok(t) => t.wf() && t.plane(0) == p && t.plane(1) == q && t.plane(2) == r,
                Err(e) => e == NumError::Degenerate && tpi_w(p, q, r) == 0,
            },
    {
        let mut t = Tpi {
            planes: [p, q, r],
            weight: Sign::Zero,
        };
        match t.homogeneous_checked() {
            Some((_, w)) if w.is_zero() => Err(NumError::Degenerate),
            Some((_, w)) => {
                t.weight = w.signum();
                Ok(t)
            }
            None => Err(NumError::BudgetExceeded),
        }
    }
}

impl Lpi {
    /// The point where the line from `p` to `q` meets `plane`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the line meets the plane in no point or in the whole line:
    /// p = q, or the line is parallel to the plane or lies in it. Decided exactly: w = n · (q − p)
    /// = 0. [`NumError::BudgetExceeded`] if w or n · p + d does not fit an `i128`, which no plane
    /// made by [`Plane::through`] causes ([`Plane::side_of_lpi`]'s budget).
    pub fn new(p: P3, q: P3, plane: Plane) -> (res: Result<Lpi, NumError>)
        requires
            plane.wf(),
            on_grid3(p),
            on_grid3(q),
        ensures
            match res {
                Ok(l) => l.wf() && l.p() == p && l.q() == q && l.plane() == plane,
                Err(e) => e == NumError::Degenerate && lpi_w(plane, p, q) == 0,
            },
    {
        match plane.weight_and_offset(p, q) {
            Some((0, _)) => Err(NumError::Degenerate),
            Some(_) => Ok(Lpi { p, q, plane }),
            None => Err(NumError::BudgetExceeded),
        }
    }
}

} // verus!

/// q − p for two coordinates, in `i128`. Checked like every step here, although it always fits:
/// |q − p| < 2^64 for any two `i64` values.
#[verus_spec(r =>
    ensures
        r matches Some(v) && v as int == q@ - p@,
)]
pub(crate) const fn difference(q: Coord, p: Coord) -> Option<i128> {
    (q.get() as i128).checked_sub(p.get() as i128)
}

/// p · q − r · s in `i128`, every step checked: `None` if a step does not fit.
#[verus_spec(res =>
    ensures
        match res {
            Some(v) => v as int == p as int * q as int - r as int * s as int,
            None => !(fits128(p as int * q as int) && fits128(r as int * s as int)
                && fits128(p as int * q as int - r as int * s as int)),
        },
)]
const fn mul_sub(p: i128, q: i128, r: i128, s: i128) -> Option<i128> {
    match (p.checked_mul(q), r.checked_mul(s)) {
        (Some(pq), Some(rs)) => pq.checked_sub(rs),
        _ => None,
    }
}

/// `n_x` `p_x` + `n_y` `p_y` + `n_z` `p_z` for a plane normal and a grid point, in `i128`, every
/// step checked: `None` if a step does not fit. Budgets on [`Plane::through`] and [`Plane::side`].
#[verus_spec(r =>
    ensures
        ({
            let (a, b, c) = (n@[0] as int * p.x@, n@[1] as int * p.y@, n@[2] as int * p.z@);
            match r {
                Some(v) => v as int == a + b + c,
                None => !(fits128(a) && fits128(b) && fits128(c) && fits128(a + b) && fits128(a + b + c)),
            }
        }),
)]
const fn dot_point(n: &[i128; 3], p: P3) -> Option<i128> {
    let [nx, ny, nz] = *n;
    let (Some(tx), Some(ty), Some(tz)) = (
        nx.checked_mul(p.x.get() as i128),
        ny.checked_mul(p.y.get() as i128),
        nz.checked_mul(p.z.get() as i128),
    ) else {
        return None;
    };
    match tx.checked_add(ty) {
        Some(s) => s.checked_add(tz),
        None => None,
    }
}

/// n = (b − a) × (c − a) and d = −n · a in `i128`, every step checked: `None` if a step does not
/// fit, which the budget on [`Plane::through`] rules out.
#[allow(
    clippy::many_single_char_names,
    reason = "a, b and c are the points and n and d the coefficients of the formulas above"
)]
#[verus_spec(r =>
    requires
        on_grid3(a),
        on_grid3(b),
        on_grid3(c),
    ensures
        r matches Some((n, d)) && (n@[0] as int, n@[1] as int, n@[2] as int) == normal(a, b, c)
            && d as int == -dot3(normal(a, b, c), a),
)]
const fn coefficients(a: P3, b: P3, c: P3) -> Option<([i128; 3], i128)> {
    proof! {
        lemma_coefficient_bounds(a, b, c);
    }
    let (Some(ux), Some(uy), Some(uz), Some(vx), Some(vy), Some(vz)) = (
        difference(b.x, a.x),
        difference(b.y, a.y),
        difference(b.z, a.z),
        difference(c.x, a.x),
        difference(c.y, a.y),
        difference(c.z, a.z),
    ) else {
        return None;
    };
    let (Some(nx), Some(ny), Some(nz)) = (
        mul_sub(uy, vz, uz, vy),
        mul_sub(uz, vx, ux, vz),
        mul_sub(ux, vy, uy, vx),
    ) else {
        return None;
    };
    let n = [nx, ny, nz];
    match dot_point(&n, a) {
        // Checked like every step: only `i128::MIN` has no negation in `i128`.
        Some(na) => match na.checked_neg() {
            Some(d) => Some((n, d)),
            None => None,
        },
        None => None,
    }
}

/// Whether plane coefficients obey the budget of chapter 6.2: |`n_i`| < 2^83 and |d| < 2^125.
#[verus_spec(r =>
    ensures
        r == (-0x800000000000000000000 < n@[0] < 0x800000000000000000000
            && -0x800000000000000000000 < n@[1] < 0x800000000000000000000
            && -0x800000000000000000000 < n@[2] < 0x800000000000000000000
            && -0x20000000000000000000000000000000 < d < 0x20000000000000000000000000000000),
)]
const fn within_budget(n: &[i128; 3], d: i128) -> bool {
    let [nx, ny, nz] = *n;
    nx.unsigned_abs() < NORMAL_LIMIT
        && ny.unsigned_abs() < NORMAL_LIMIT
        && nz.unsigned_abs() < NORMAL_LIMIT
        && d.unsigned_abs() < OFFSET_LIMIT
}

impl Plane {
    /// Which side of the plane a grid point is on: the sign of n · p + d. `Zero` means exactly on
    /// the plane; `Positive` means on the side the normal points to.
    ///
    /// Bit budget: |`n_i`| < 2^83 and |d| < 2^125, checked when the plane was made, and
    /// |`p_i`| < 2^40, checked when the point was made. So each product `n_i` · `p_i` is below
    /// 2^123 and their sum below 3 · 2^123; with |d| < 2^125 = 4 · 2^123, every partial sum stays
    /// below 7 · 2^123 < 2^126, in `i128`.
    ///
    /// Reach: n · p + d is −det[[a, 1], [b, 1], [c, 1], [p, 1]] for the points a, b, c the plane
    /// was made from. Over coordinates in [−M, M] its largest magnitude lies at a corner of that
    /// box (reasoning on [`Tpi::homogeneous`]), where it is M³ times a 4 × 4 determinant of
    /// entries ±1, at most 16 by Hadamard's inequality. So |n · p + d| ≤ 16M³ < 2^124, which the
    /// frame's corners reach (tested).
    ///
    /// Every step is checked all the same, and a step that does not fit is reported, not answered.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit `i128`, which no plane and point can
    /// cause: their bounds are checked when they are made, and the arithmetic above is asserted
    /// when the crate compiles (module docs, "No answer is silently wrong").
    #[verus_spec(r =>
        requires
            self.wf(),
            on_grid3(p),
        ensures
            r == Ok::<Sign, NumError>(sign_of(dot3(self.normal(), p) + self.offset())),
    )]
    pub const fn side(&self, p: P3) -> Result<Sign, NumError> {
        proof! {
            lemma_side_bounds(self.normal(), self.offset(), p);
        }
        match dot_point(&self.n, p) {
            Some(np) => match np.checked_add(self.d) {
                Some(v) => Ok(Sign::of_i128(v)),
                None => Err(NumError::BudgetExceeded),
            },
            None => Err(NumError::BudgetExceeded),
        }
    }
}

/// a × b for two plane normals (|`a_i`|, |`b_i`| < 2^83): each component below 2^167, in [`I256`].
#[verus_spec(res =>
    requires
        -0x800000000000000000000 < a@[0] < 0x800000000000000000000,
        -0x800000000000000000000 < a@[1] < 0x800000000000000000000,
        -0x800000000000000000000 < a@[2] < 0x800000000000000000000,
        -0x800000000000000000000 < b@[0] < 0x800000000000000000000,
        -0x800000000000000000000 < b@[1] < 0x800000000000000000000,
        -0x800000000000000000000 < b@[2] < 0x800000000000000000000,
    ensures
        res matches Some(c) && (c@[0].value(), c@[1].value(), c@[2].value())
            == cross3((a@[0] as int, a@[1] as int, a@[2] as int), (b@[0] as int, b@[1] as int, b@[2] as int)),
)]
fn cross(a: &[i128; 3], b: &[i128; 3]) -> Option<[I256; 3]> {
    let [ax, ay, az] = *a;
    let [bx, by, bz] = *b;
    Some([
        product_difference(ay, bz, az, by)?,
        product_difference(az, bx, ax, bz)?,
        product_difference(ax, by, ay, bx)?,
    ])
}

/// p · q − r · s for normal components below 2^83: each product below 2^166, the difference below
/// 2^167, in [`I256`].
#[verus_spec(res =>
    requires
        -0x800000000000000000000 < p < 0x800000000000000000000,
        -0x800000000000000000000 < q < 0x800000000000000000000,
        -0x800000000000000000000 < r < 0x800000000000000000000,
        -0x800000000000000000000 < s < 0x800000000000000000000,
    ensures
        res matches Some(v) && v.value() == p as int * q as int - r as int * s as int,
)]
fn product_difference(p: i128, q: i128, r: i128, s: i128) -> Option<I256> {
    proof! {
        lemma_half_base_4();
        lemma_product_bound(p as int, q as int, 9671406556917033397649407, 9671406556917033397649407);
        lemma_product_bound(r as int, s as int, 9671406556917033397649407, 9671406556917033397649407);
    }
    let pq = I256::from_i128(p).checked_mul(&I256::from_i128(q))?;
    let rs = I256::from_i128(r).checked_mul(&I256::from_i128(s))?;
    pq.checked_sub(&rs)
}

/// a · c for a normal a (|`a_i`| < 2^83) and a cross product of normals c (|`c_i`| < 2^167): each
/// product below 2^250, every partial sum below 3 · 2^250 < 2^252, in [`I256`].
#[verus_spec(res =>
    requires
        -0x800000000000000000000 < a@[0] < 0x800000000000000000000,
        -0x800000000000000000000 < a@[1] < 0x800000000000000000000,
        -0x800000000000000000000 < a@[2] < 0x800000000000000000000,
        -bound_167() < c@[0].value() < bound_167(),
        -bound_167() < c@[1].value() < bound_167(),
        -bound_167() < c@[2].value() < bound_167(),
    ensures
        res matches Some(v) && v.value()
            == dot_t((a@[0] as int, a@[1] as int, a@[2] as int), (c@[0].value(), c@[1].value(), c@[2].value())),
)]
fn dot(a: &[i128; 3], c: &[I256; 3]) -> Option<I256> {
    proof! {
        lemma_half_base_4();
        lemma_product_bound(a@[0] as int, c@[0].value(), 9671406556917033397649407, bound_167());
        lemma_product_bound(a@[1] as int, c@[1].value(), 9671406556917033397649407, bound_167());
        lemma_product_bound(a@[2] as int, c@[2].value(), 9671406556917033397649407, bound_167());
    }
    let ([ax, ay, az], [cx, cy, cz]) = (*a, c);
    I256::from_i128(ax)
        .checked_mul(cx)?
        .checked_add(&I256::from_i128(ay).checked_mul(cy)?)?
        .checked_add(&I256::from_i128(az).checked_mul(cz)?)
}

/// One numerator of [`Tpi::homogeneous`], −(`d_1` a + `d_2` b + `d_3` c) for the offsets `d_k` and one
/// component a, b, c of each cross product of normals (|d| < 2^125, |a|, |b|, |c| < 2^167): each
/// product below 2^292, every partial sum below 3 · 2^292 < 2^294, in [`I320`].
#[verus_spec(res =>
    requires
        -bound_125() < dp.value() < bound_125(),
        -bound_125() < dq.value() < bound_125(),
        -bound_125() < dr.value() < bound_125(),
        -bound_167() < a.value() < bound_167(),
        -bound_167() < b.value() < bound_167(),
        -bound_167() < c.value() < bound_167(),
    ensures
        res matches Some(v) && v.value() == -(dp.value() * a.value() + dq.value() * b.value() + dr.value() * c.value()),
)]
fn numerator(dp: &I320, dq: &I320, dr: &I320, a: &I256, b: &I256, c: &I256) -> Option<I320> {
    proof! {
        lemma_half_base_4();
        lemma_half_base_5();
        lemma_product_bound(dp.value(), a.value(), bound_125(), bound_167());
        lemma_product_bound(dq.value(), b.value(), bound_125(), bound_167());
        lemma_product_bound(dr.value(), c.value(), bound_125(), bound_167());
    }
    dp.checked_mul(&a.resize::<5>()?)?
        .checked_add(&dq.checked_mul(&b.resize::<5>()?)?)?
        .checked_add(&dr.checked_mul(&c.resize::<5>()?)?)?
        .checked_neg()
}

impl Tpi {
    /// The point's homogeneous coordinates: numerators X = (`X_x`, `X_y`, `X_z`) and denominator w,
    /// with the point at (`X_x` / w, `X_y` / w, `X_z` / w). w is never zero, and may be negative;
    /// swapping two planes negates both X and w.
    ///
    /// For planes (`n_1`, `d_1`), (`n_2`, `d_2`), (`n_3`, `d_3`), by Cramer's rule:
    /// w = `n_1` · (`n_2` × `n_3`) = det(`n_1`, `n_2`, `n_3`) and
    /// X = −(`d_1` (`n_2` × `n_3`) + `d_2` (`n_3` × `n_1`) + `d_3` (`n_1` × `n_2`)).
    /// (Check: `n_1` · X = −`d_1` w, because `n_1` · (`n_3` × `n_1`) = `n_1` · (`n_1` × `n_2`) = 0;
    /// likewise for the other two planes.)
    ///
    /// Bit budget (|`n_i`| < 2^83, |d| < 2^125, both checked when the planes were made):
    /// - each product of two normal components is below 2^166, so each component of a cross
    ///   product is below 2^167, in [`I256`];
    /// - each product `n_1i` · (`n_2` × `n_3`)_i is below 2^83 · 2^167 = 2^250, so every partial
    ///   sum of w stays below 3 · 2^250 < 2^252, in [`I256`] (limit 2^255);
    /// - each product `d_k` · (cross product)_i is below 2^125 · 2^167 = 2^292, so every partial
    ///   sum of `X_i` stays below 3 · 2^292 < 2^294, in [`I320`] (limit 2^319).
    ///
    /// Reach, for planes through grid points: w is the determinant of the normals, and `X_i`, up to
    /// sign, the same determinant with column i replaced by the offsets (Cramer's rule). A
    /// determinant is affine in each entry, so moving one entry to an end of its interval never
    /// lowers the magnitude for some choice of end; over a box of entries the largest magnitude
    /// therefore lies at a corner, where each entry is plus or minus its bound. Taking the bounds
    /// out of the columns leaves a determinant of entries ±1. A 3 × 3 one is at most 4 in
    /// magnitude: Hadamard's inequality bounds it by 3^(3/2) < 6, and it is a multiple of 4
    /// (subtracting the first row from the others leaves two rows of 0 and ±2). With |`n_i`| ≤ 4M²
    /// and |d| ≤ 4M³ ([`Plane::through`]), |w| ≤ 4 (4M²)³ = 256M^6 < 2^248 and
    /// |`X_i`| ≤ 4 (4M²)² (4M³) = 256M^7 < 2^288. Three planes through corners of the frame's cube
    /// reach both (tested).
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step did not fit its type. [`Tpi::new`] completed this
    /// same computation for the same planes, so it cannot happen; were the checked arithmetic
    /// itself wrong, this reports it rather than returning coordinates.
    #[verus_spec(res =>
        requires
            self.plane(0).wf(),
            self.plane(1).wf(),
            self.plane(2).wf(),
        ensures
            res matches Ok((x, w)) && tpi_coordinates(*self, x, w),
    )]
    pub fn homogeneous(&self) -> Result<([I320; 3], I256), NumError> {
        match self.homogeneous_checked() {
            Some(h) => Ok(h),
            None => Err(NumError::BudgetExceeded),
        }
    }

    /// [`Tpi::homogeneous`] with every wide operation checked: `None` only if a budget were wrong.
    #[allow(
        clippy::many_single_char_names,
        reason = "p, q and r are the planes, and w and x the point, as in the formulas"
    )]
    #[verus_spec(res =>
        requires
            self.plane(0).wf(),
            self.plane(1).wf(),
            self.plane(2).wf(),
        ensures
            res matches Some((x, w)) && tpi_coordinates(*self, x, w),
    )]
    fn homogeneous_checked(&self) -> Option<([I320; 3], I256)> {
        let [p, q, r] = &self.planes;
        proof! {
            lemma_tpi_bounds(*p, *q, *r);
            lemma_half_base_4();
            lemma_half_base_5();
        }
        let c_qr = cross(&q.n, &r.n)?;
        let c_rp = cross(&r.n, &p.n)?;
        let c_pq = cross(&p.n, &q.n)?;
        let w = dot(&p.n, &c_qr)?;
        let (dp, dq, dr) = (
            I320::from_i128(p.d),
            I320::from_i128(q.d),
            I320::from_i128(r.d),
        );
        let ([qr_x, qr_y, qr_z], [rp_x, rp_y, rp_z], [pq_x, pq_y, pq_z]) = (c_qr, c_rp, c_pq);
        let x = [
            numerator(&dp, &dq, &dr, &qr_x, &rp_x, &pq_x)?,
            numerator(&dp, &dq, &dr, &qr_y, &rp_y, &pq_y)?,
            numerator(&dp, &dq, &dr, &qr_z, &rp_z, &pq_z)?,
        ];
        Some((x, w))
    }
}

/// The sign of n · X + d · w times the sign of w, in `Wide<L>`: [`I384`] on the budgeted path,
/// [`I512`] on the total path. `None` if a step does not fit. Budgets for both on
/// [`Plane::side_of_tpi`].
#[verus_spec(res =>
    requires
        6 <= L <= 4096,
    ensures
        res.is_some() ==> res == Some(sign_mul(sign_of(d as int * w.value() + dot_t((n@[0] as int, n@[1] as int, n@[2] as int),
            (x@[0].value(), x@[1].value(), x@[2].value()))), sign_of(w.value()))),
        (L >= 8 || (-0x800000000000000000000 < n@[0] < 0x800000000000000000000
            && -0x800000000000000000000 < n@[1] < 0x800000000000000000000
            && -0x800000000000000000000 < n@[2] < 0x800000000000000000000
            && -0x20000000000000000000000000000000 < d < 0x20000000000000000000000000000000
            && -bound_294() < x@[0].value() < bound_294() && -bound_294() < x@[1].value() < bound_294()
            && -bound_294() < x@[2].value() < bound_294() && -bound_252() < w.value() < bound_252()))
            ==> res.is_some(),
)]
fn classify<const L: usize>(n: &[i128; 3], d: i128, x: &[I320; 3], w: &I256) -> Option<Sign> {
    proof! {
        lemma_classify_bounds::<L>(n, d, x, w);
    }
    let ([nx, ny, nz], [xx, xy, xz]) = (*n, x);
    let side = Wide::<L>::from_i128(d)
        .checked_mul(&w.resize::<L>()?)?
        .checked_add(&Wide::<L>::from_i128(nx).checked_mul(&xx.resize::<L>()?)?)?
        .checked_add(&Wide::<L>::from_i128(ny).checked_mul(&xy.resize::<L>()?)?)?
        .checked_add(&Wide::<L>::from_i128(nz).checked_mul(&xz.resize::<L>()?)?)?;
    Some(side.signum().mul(w.signum()))
}

impl Plane {
    /// [`Plane::side_of_tpi`] after the generated paths: the hand-written budgeted path in
    /// [`I384`], then the total path in [`I512`].
    #[inline(never)]
    #[verus_spec(res =>
        requires
            self.wf(),
            t.wf(),
        ensures
            res == Ok::<Sign, NumError>(tpi_side_sign(*self, *t)),
    )]
    fn side_of_tpi_hand(&self, t: &Tpi) -> Result<Sign, NumError> {
        let (x, w) = t.homogeneous()?;
        proof! {
            lemma_tpi_bounds(t.plane(0), t.plane(1), t.plane(2));
        }
        match classify::<BUDGET_LIMBS>(&self.n, self.d, &x, &w) {
            Some(s) => Ok(s),
            None => match classify::<TOTAL_LIMBS>(&self.n, self.d, &x, &w) {
                Some(s) => Ok(s),
                None => Err(NumError::BudgetExceeded),
            },
        }
    }
}

impl Plane {
    /// w = n · (q − p) and sₚ = n · p + d for the line from p to q, in `i128`: `None` if either
    /// does not fit.
    #[verus_spec(r =>
        requires
            self.wf(),
            on_grid3(p),
            on_grid3(q),
        ensures
            r matches Some((w, sp)) && w as int == lpi_w(*self, p, q) && sp as int == plane_at(*self, p),
    )]
    fn weight_and_offset(&self, p: P3, q: P3) -> Option<(i128, i128)> {
        proof! {
            lemma_side_bounds(self.normal(), self.offset(), p);
            lemma_side_bounds(self.normal(), self.offset(), q);
            lemma_lpi_bounds(*self, p, q);
        }
        let sp = dot_point(&self.n, p)?.checked_add(self.d)?;
        let sq = dot_point(&self.n, q)?.checked_add(self.d)?;
        Some((sq.checked_sub(sp)?, sp))
    }

    /// [`Plane::side_of_lpi`] after the generated paths: w₁ s₂ₚ − w₂ s₁ₚ in [`I256`], then in
    /// [`I320`], times the sign of w₁.
    #[inline(never)]
    #[verus_spec(res =>
        requires
            self.wf(),
            l.wf(),
        ensures
            res == Ok::<Sign, NumError>(lpi_side_sign(*self, *l)),
    )]
    fn side_of_lpi_hand(&self, l: &Lpi) -> Result<Sign, NumError> {
        proof! {
            lemma_lpi_bounds(l.plane, l.p, l.q);
            lemma_lpi_bounds(*self, l.p, l.q);
        }
        let (Some((w1, s1p)), Some((w2, s2p))) = (
            l.plane.weight_and_offset(l.p, l.q),
            self.weight_and_offset(l.p, l.q),
        ) else {
            return Err(NumError::BudgetExceeded);
        };
        let side = match lpi_classify::<4>(w1, s2p, w2, s1p) {
            Some(s) => Some(s),
            None => lpi_classify::<5>(w1, s2p, w2, s1p),
        };
        match side {
            Some(s) => Ok(s.mul(Sign::of_i128(w1))),
            None => Err(NumError::BudgetExceeded),
        }
    }
}

impl Lpi {
    /// The point's homogeneous coordinates: numerators X and denominator w, with the point at
    /// X / w. With `s_p` = n · p + d and `s_q` = n · q + d, the point p + t (q − p) lies on the plane
    /// for t = −`s_p` / (`s_q` − `s_p`), so X = `s_q` p − `s_p` q and w = `s_q` − `s_p` = n · (q − p), never zero.
    /// Swapping p and q negates both.
    ///
    /// Bit budget: |s| < 2^126 ([`Plane::side_of_lpi`]) and |`p_i`|, |`q_i`| ≤ M < 2^40, so each `X_i`
    /// is below 2^167, in [`I256`]; w is below 2^126, an `i128`.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step does not fit, which [`Lpi::new`] already ruled out
    /// for the same line and plane, and the products cannot exceed [`I256`] for any `i128`
    /// offsets and grid coordinates (at most 2^127 · 2^40 each).
    #[verus_spec(res =>
        requires
            self.wf(),
        ensures
            res matches Ok((x, w)) && w as int == lpi_w(self.plane(), self.p(), self.q())
                && x@[0].value() == plane_at(self.plane(), self.q()) * self.p().x@ - plane_at(self.plane(), self.p()) * self.q().x@
                && x@[1].value() == plane_at(self.plane(), self.q()) * self.p().y@ - plane_at(self.plane(), self.p()) * self.q().y@
                && x@[2].value() == plane_at(self.plane(), self.q()) * self.p().z@ - plane_at(self.plane(), self.p()) * self.q().z@,
    )]
    pub fn homogeneous(&self) -> Result<([I256; 3], i128), NumError> {
        proof! {
            lemma_lpi_bounds(self.plane, self.p, self.q);
        }
        let Some((w, sp)) = self.plane.weight_and_offset(self.p, self.q) else {
            return Err(NumError::BudgetExceeded);
        };
        let Some(sq) = sp.checked_add(w) else {
            return Err(NumError::BudgetExceeded);
        };
        let (Some(x), Some(y), Some(z)) = (
            lpi_axis(sq, sp, self.p.x, self.q.x),
            lpi_axis(sq, sp, self.p.y, self.q.y),
            lpi_axis(sq, sp, self.p.z, self.q.z),
        ) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(([x, y, z], w))
    }
}

/// One numerator of [`Lpi::homogeneous`], `s_q` · `p_i` − `s_p` · `q_i`, in [`I256`]: `None` if a
/// step does not fit, which no `i128` offsets and grid coordinates cause.
#[verus_spec(r =>
    requires
        pc.on_grid(),
        qc.on_grid(),
    ensures
        r matches Some(v) && v.value() == sq as int * pc@ - sp as int * qc@,
)]
fn lpi_axis(sq: i128, sp: i128, pc: Coord, qc: Coord) -> Option<I256> {
    proof! {
        lemma_half_base_4();
        lemma_on_grid(pc);
        lemma_on_grid(qc);
        lemma_product_bound(sq as int, pc@, 0x80000000000000000000000000000000, 1099511627775);
        lemma_product_bound(sp as int, qc@, 0x80000000000000000000000000000000, 1099511627775);
    }
    I256::product_i128(sq, i128::from(pc.get()))?
        .checked_sub(&I256::product_i128(sp, i128::from(qc.get()))?)
}

/// The sign of w₁ s₂ₚ − w₂ s₁ₚ in `Wide<L>`: `None` if a step does not fit.
#[verus_spec(r =>
    requires
        4 <= L <= 4096,
    ensures
        r.is_some() ==> r == Some(sign_of(w1 as int * s2p as int - w2 as int * s1p as int)),
        (L >= 5 || (-bound_126() < w1 < bound_126() && -bound_126() < s2p < bound_126()
            && -bound_126() < w2 < bound_126() && -bound_126() < s1p < bound_126())) ==> r.is_some(),
)]
fn lpi_classify<const L: usize>(w1: i128, s2p: i128, w2: i128, s1p: i128) -> Option<Sign> {
    proof! {
        lemma_lpi_classify_bounds::<L>(w1, s2p, w2, s1p);
    }
    let a = Wide::<L>::product_i128(w1, s2p)?;
    let b = Wide::<L>::product_i128(w2, s1p)?;
    Some(a.checked_sub(&b)?.signum())
}

