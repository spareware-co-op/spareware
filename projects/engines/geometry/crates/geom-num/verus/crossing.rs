//! A Verus-annotated twin of `geom-num`'s 2D segment crossings (`src/crossing.rs`), for `verus` to
//! verify (chapter 28.7 of the kernel architecture; piece G39).
//!
//! It proves, for grid points (|v| ≤ 2^40 − 1):
//!
//! - `Ssi::new` makes the crossing of the lines a b and c d, `Degenerate` exactly when they are
//!   parallel (w = (b − a) × (d − c) = 0), and never `BudgetExceeded`.
//! - `Ssi::homogeneous` gives X = oa b − ob a and w = oa − ob, exactly (oa, ob the orientations of
//!   a and b against c d), and X / w lies on both lines.
//! - The hand-written classification gives the sign of oa · ob₂ − ob · oa₂ times the sign of w, in
//!   `I256`, which equals the orientation of p, q and the crossing point.
//!
//! The functions below the `verus!` block, with their `#[verus_spec]` attributes and their `proof!`
//! blocks removed, are token for token those of `src/crossing.rs`: `geom-gen`'s
//! `tests/verus_twin.rs` compares them. `Ssi::new`, an associated function without `self`, is
//! written inside the `verus!` block instead, and the test compares its doc comment, attributes,
//! visibility, parameters and body. Also inside the block are a model of `Ssi` with the same field
//! and the definitions and lemmas the proofs use.
//!
//! It is a module of `geom-num`'s Verus crate, `verus/lib.rs`. Verify: `verus --crate-type=lib
//! projects/engines/geometry/crates/geom-num/verus/lib.rs`, with the Verus release `kernel-dev.yml`
//! pins; its `verus` job does this on every pull request.

use vstd::prelude::*;

use crate::model::*;
use crate::plane::difference;
use crate::sign::*;
use crate::wide::*;

verus! {

/// Whether both coordinates of p are on the grid.
pub open spec fn on_grid2(p: P2) -> bool {
    p.x.on_grid() && p.y.on_grid()
}

/// The orientation of a, b, c: (b − a) × (c − a), over the integers.
pub open spec fn orient(a: P2, b: P2, c: P2) -> int {
    (b.x@ - a.x@) * (c.y@ - a.y@) - (b.y@ - a.y@) * (c.x@ - a.x@)
}

/// 2^84, the bound of an orientation of grid points.
pub open spec fn bound_84() -> int {
    19342813113834066795298816int
}

/// An orientation of grid points is below 2^84.
pub proof fn lemma_orient_bound(a: P2, b: P2, c: P2)
    requires
        on_grid2(a),
        on_grid2(b),
        on_grid2(c),
    ensures
        -bound_84() < orient(a, b, c) < bound_84(),
        -(4 * 1099511627775 * 1099511627775) <= (b.x@ - a.x@) * (c.y@ - a.y@) <= 4 * 1099511627775 * 1099511627775,
        -(4 * 1099511627775 * 1099511627775) <= (b.y@ - a.y@) * (c.x@ - a.x@) <= 4 * 1099511627775 * 1099511627775,
{
    lemma_on_grid(a.x);
    lemma_on_grid(a.y);
    lemma_on_grid(b.x);
    lemma_on_grid(b.y);
    lemma_on_grid(c.x);
    lemma_on_grid(c.y);
    let m2 = 2 * 1099511627775int;
    lemma_product_bound(b.x@ - a.x@, c.y@ - a.y@, m2, m2);
    lemma_product_bound(b.y@ - a.y@, c.x@ - a.x@, m2, m2);
}

/// The crossing point X / w, X = oa b − ob a, w = oa − ob, lies on the line c d, and its
/// orientation against p q, times w, is oa · ob₂ − ob · oa₂ (orient2d is affine). With X − w r =
/// oa (b − r) − ob (a − r) for any point r, each cross product splits into two orientations.
pub proof fn lemma_crossing_point(a: P2, b: P2, c: P2, d: P2, p: P2, q: P2)
    ensures
        ({
            let (oa, ob) = (orient(c, d, a), orient(c, d, b));
            let (x0, x1, w) = (oa * b.x@ - ob * a.x@, oa * b.y@ - ob * a.y@, oa - ob);
            &&& (d.x@ - c.x@) * (x1 - w * c.y@) - (d.y@ - c.y@) * (x0 - w * c.x@) == 0
            &&& (q.x@ - p.x@) * (x1 - w * p.y@) - (q.y@ - p.y@) * (x0 - w * p.x@)
                == oa * orient(p, q, b) - ob * orient(p, q, a)
        }),
{
    let (oa, ob) = (orient(c, d, a), orient(c, d, b));
    let (x0, x1, w) = (oa * b.x@ - ob * a.x@, oa * b.y@ - ob * a.y@, oa - ob);
    lemma_cross_split(oa, ob, a, b, c, d);
    lemma_cross_split(oa, ob, a, b, p, q);
    assert(oa * ob - ob * oa == 0) by (nonlinear_arith);
}

/// For the line r s: (s − r) × (X − w r) = oa · orient(r, s, b) − ob · orient(r, s, a), with
/// X = oa b − ob a and w = oa − ob.
pub proof fn lemma_cross_split(oa: int, ob: int, a: P2, b: P2, r: P2, s: P2)
    ensures
        ({
            let (x0, x1, w) = (oa * b.x@ - ob * a.x@, oa * b.y@ - ob * a.y@, oa - ob);
            (s.x@ - r.x@) * (x1 - w * r.y@) - (s.y@ - r.y@) * (x0 - w * r.x@)
                == oa * orient(r, s, b) - ob * orient(r, s, a)
        }),
{
    let (u, v) = (s.x@ - r.x@, s.y@ - r.y@);
    let (b0, b1, a0, a1) = (b.x@ - r.x@, b.y@ - r.y@, a.x@ - r.x@, a.y@ - r.y@);
    let (x0, x1, w) = (oa * b.x@ - ob * a.x@, oa * b.y@ - ob * a.y@, oa - ob);
    assert(x1 - w * r.y@ == oa * b1 - ob * a1) by (nonlinear_arith)
        requires x1 == oa * b.y@ - ob * a.y@, w == oa - ob, b1 == b.y@ - r.y@, a1 == a.y@ - r.y@;
    assert(x0 - w * r.x@ == oa * b0 - ob * a0) by (nonlinear_arith)
        requires x0 == oa * b.x@ - ob * a.x@, w == oa - ob, b0 == b.x@ - r.x@, a0 == a.x@ - r.x@;
    assert(u * (oa * b1 - ob * a1) == oa * (u * b1) - ob * (u * a1)) by (nonlinear_arith);
    assert(v * (oa * b0 - ob * a0) == oa * (v * b0) - ob * (v * a0)) by (nonlinear_arith);
    assert(oa * (u * b1 - v * b0) == oa * (u * b1) - oa * (v * b0)) by (nonlinear_arith);
    assert(ob * (u * a1 - v * a0) == ob * (u * a1) - ob * (v * a0)) by (nonlinear_arith);
    assert(orient(r, s, b) == u * b1 - v * b0);
    assert(orient(r, s, a) == u * a1 - v * a0);
}

/// The steps of `side` fit `Wide<L>`: in the budget for L ≥ 4, for any `i128`s for L ≥ 5.
pub proof fn lemma_side_bounds2<const L: usize>(a: i128, b: i128, c: i128, d: i128)
    requires
        4 <= L <= 4096,
    ensures
        (L >= 5 || (-bound_84() < a < bound_84() && -bound_84() < b < bound_84()
            && -bound_84() < c < bound_84() && -bound_84() < d < bound_84())) ==> {
            &&& fits(a as int * b as int, L as nat)
            &&& fits(c as int * d as int, L as nat)
            &&& fits(a as int * b as int - c as int * d as int, L as nat)
        },
{
    lemma_half_base_4();
    lemma_half_base_5();
    lemma_base_monotone(4, L as nat);
    if L >= 5 {
        lemma_base_monotone(5, L as nat);
        let m = 0x80000000000000000000000000000000int;
        lemma_product_bound(a as int, b as int, m, m);
        lemma_product_bound(c as int, d as int, m, m);
    } else if -bound_84() < a < bound_84() && -bound_84() < b < bound_84()
        && -bound_84() < c < bound_84() && -bound_84() < d < bound_84() {
        lemma_product_bound(a as int, b as int, bound_84(), bound_84());
        lemma_product_bound(c as int, d as int, bound_84(), bound_84());
    }
}

/// A model of `geom_num::crossing::Ssi`: the point where the line from a to b crosses the line
/// from c to d.
#[derive(Clone, Copy)]
pub struct Ssi {
    points: [P2; 4],
}

impl Ssi {
    /// Point k of a, b, c, d.
    pub closed spec fn point(self, k: int) -> P2 {
        self.points@[k]
    }

    /// What `Ssi::new` makes: grid points, and lines that are not parallel (oa ≠ ob).
    pub closed spec fn wf(self) -> bool {
        &&& on_grid2(self.points@[0]) &&& on_grid2(self.points@[1])
        &&& on_grid2(self.points@[2]) &&& on_grid2(self.points@[3])
        &&& orient(self.points@[2], self.points@[3], self.points@[0])
            != orient(self.points@[2], self.points@[3], self.points@[1])
    }
}

/// The side of the line p q that s's crossing point lies on: the sign of oa · ob₂ − ob · oa₂ times
/// the sign of w = oa − ob.
pub open spec fn ssi_side_sign(s: Ssi, p: P2, q: P2) -> Sign {
    let (a, b, c, d) = (s.point(0), s.point(1), s.point(2), s.point(3));
    let (oa, ob) = (orient(c, d, a), orient(c, d, b));
    sign_mul(sign_of(oa * orient(p, q, b) - ob * orient(p, q, a)), sign_of(oa - ob))
}


impl Ssi {
    /// The point where the line from `a` to `b` crosses the line from `c` to `d`.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] when the lines meet in no point or in a whole line: they are
    /// parallel or the same line, or a = b, or c = d. Decided exactly: w = (b − a) × (d − c) = 0.
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    pub fn new(a: P2, b: P2, c: P2, d: P2) -> (res: Result<Ssi, NumError>)
        requires
            on_grid2(a),
            on_grid2(b),
            on_grid2(c),
            on_grid2(d),
        ensures
            match res {
                Ok(s) => s.wf() && s.point(0) == a && s.point(1) == b && s.point(2) == c && s.point(3) == d,
                Err(e) => e == NumError::Degenerate && orient(c, d, a) == orient(c, d, b),
            },
    {
        let s = Ssi {
            points: [a, b, c, d],
        };
        match s.offsets() {
            Some((oa, ob)) if oa == ob => Err(NumError::Degenerate),
            Some(_) => Ok(s),
            None => Err(NumError::BudgetExceeded),
        }
    }
}

} // verus!

impl Ssi {
    /// oa = orient2d(c, d, a) and ob = orient2d(c, d, b), in `i128`: how far a and b lie on
    /// either side of the second line.
    #[verus_spec(r =>
        requires
            on_grid2(self.point(0)),
            on_grid2(self.point(1)),
            on_grid2(self.point(2)),
            on_grid2(self.point(3)),
        ensures
            r matches Some((oa, ob)) && oa as int == orient(self.point(2), self.point(3), self.point(0))
                && ob as int == orient(self.point(2), self.point(3), self.point(1)),
    )]
    fn offsets(&self) -> Option<(i128, i128)> {
        let [a, b, c, d] = self.points;
        Some((cross(c, d, a)?, cross(c, d, b)?))
    }

    /// The point's homogeneous coordinates: numerators X and denominator w, with the point at
    /// X / w. With oa = orient2d(c, d, a) and ob = orient2d(c, d, b), the point a + t (b − a) lies
    /// on the line from c to d for t = oa / (oa − ob), so X = oa b − ob a and w = oa − ob =
    /// (b − a) × (d − c), never zero.
    ///
    /// Bit budget: with coordinates within ±M (M = 2^40 − 1), each of oa, ob is a 2 × 2
    /// determinant of differences within ±2M, whose largest magnitude lies at a corner of the box,
    /// at most 4M² (82 bits); so |w| ≤ 8M² and |`X_i`| ≤ 8M³ (123 bits), all in `i128`.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] if a step does not fit, which no grid points can cause (the
    /// budget above).
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    #[verus_spec(res =>
        requires
            self.wf(),
        ensures
            ({
                let (a, b, c, d) = (self.point(0), self.point(1), self.point(2), self.point(3));
                let (oa, ob) = (orient(c, d, a), orient(c, d, b));
                res matches Ok((x, w)) && x@[0] as int == oa * b.x@ - ob * a.x@
                    && x@[1] as int == oa * b.y@ - ob * a.y@ && w as int == oa - ob
            }),
    )]
    pub fn homogeneous(&self) -> Result<([i128; 2], i128), NumError> {
        let [a, b, _, _] = self.points;
        proof! {
            lemma_orient_bound(self.point(2), self.point(3), self.point(0));
            lemma_orient_bound(self.point(2), self.point(3), self.point(1));
        }
        let Some((oa, ob)) = self.offsets() else {
            return Err(NumError::BudgetExceeded);
        };
        let (Some(x), Some(y), Some(w)) = (
            ssi_axis(oa, ob, b.x, a.x),
            ssi_axis(oa, ob, b.y, a.y),
            oa.checked_sub(ob),
        ) else {
            return Err(NumError::BudgetExceeded);
        };
        Ok(([x, y], w))
    }

    /// [`Ssi::orient2d`] by hand: oa · ob₂ − ob · oa₂ in [`I256`], then in [`I320`], times the
    /// sign of w.
    #[inline(never)]
    #[allow(
        clippy::many_single_char_names,
        reason = "a, b, c and d are the two lines' points, p and q the third line's, and w the weight, as in the formulas"
    )]
    #[verus_spec(res =>
        requires
            self.wf(),
            on_grid2(p),
            on_grid2(q),
        ensures
            res == Ok::<Sign, NumError>(ssi_side_sign(*self, p, q)),
    )]
    fn orient2d_hand(&self, p: P2, q: P2) -> Result<Sign, NumError> {
        let [a, b, _, _] = self.points;
        proof! {
            lemma_orient_bound(self.point(2), self.point(3), self.point(0));
            lemma_orient_bound(self.point(2), self.point(3), self.point(1));
            lemma_orient_bound(p, q, self.point(0));
            lemma_orient_bound(p, q, self.point(1));
        }
        let (Some((oa, ob)), Some(oa2), Some(ob2)) =
            (self.offsets(), cross(p, q, a), cross(p, q, b))
        else {
            return Err(NumError::BudgetExceeded);
        };
        let Some(w) = oa.checked_sub(ob) else {
            return Err(NumError::BudgetExceeded);
        };
        let s = match side::<4>(oa, ob2, ob, oa2) {
            Some(s) => Some(s),
            None => side::<5>(oa, ob2, ob, oa2),
        };
        match s {
            Some(s) => Ok(s.mul(Sign::of_i128(w))),
            None => Err(NumError::BudgetExceeded),
        }
    }
}

/// (b − a) × (c − a) = orient2d(a, b, c) for grid points, in `i128`: `None` if a step does not
/// fit, which coordinates within the frame cannot cause.
#[verus_spec(r =>
    requires
        on_grid2(a),
        on_grid2(b),
        on_grid2(c),
    ensures
        r matches Some(v) && v as int == orient(a, b, c),
)]
fn cross(a: P2, b: P2, c: P2) -> Option<i128> {
    proof! {
        lemma_orient_bound(a, b, c);
    }
    let (bx, by, cx, cy) = (
        difference(b.x, a.x)?,
        difference(b.y, a.y)?,
        difference(c.x, a.x)?,
        difference(c.y, a.y)?,
    );
    bx.checked_mul(cy)?.checked_sub(by.checked_mul(cx)?)
}

/// One numerator of [`Ssi::homogeneous`], oa · `b_i` − ob · `a_i`, in `i128`: `None` if a step does
/// not fit, which grid points cannot cause.
#[verus_spec(r =>
    requires
        -bound_84() < oa < bound_84(),
        -bound_84() < ob < bound_84(),
        bc.on_grid(),
        ac.on_grid(),
    ensures
        r matches Some(v) && v as int == oa as int * bc@ - ob as int * ac@,
)]
fn ssi_axis(oa: i128, ob: i128, bc: Coord, ac: Coord) -> Option<i128> {
    proof! {
        lemma_on_grid(bc);
        lemma_on_grid(ac);
        lemma_product_bound(oa as int, bc@, bound_84(), 1099511627775int);
        lemma_product_bound(ob as int, ac@, bound_84(), 1099511627775int);
    }
    oa.checked_mul(i128::from(bc.get()))?
        .checked_sub(ob.checked_mul(i128::from(ac.get()))?)
}

/// The sign of oa · ob₂ − ob · oa₂ in `Wide<L>`: `None` if a step does not fit.
#[verus_spec(r =>
    requires
        4 <= L <= 4096,
    ensures
        r.is_some() ==> r == Some(sign_of(oa as int * ob2 as int - ob as int * oa2 as int)),
        (L >= 5 || (-bound_84() < oa < bound_84() && -bound_84() < ob2 < bound_84()
            && -bound_84() < ob < bound_84() && -bound_84() < oa2 < bound_84())) ==> r.is_some(),
)]
fn side<const L: usize>(oa: i128, ob2: i128, ob: i128, oa2: i128) -> Option<Sign> {
    proof! {
        lemma_side_bounds2::<L>(oa, ob2, ob, oa2);
    }
    let left = Wide::<L>::product_i128(oa, ob2)?;
    let right = Wide::<L>::product_i128(ob, oa2)?;
    Some(left.checked_sub(&right)?.signum())
}

