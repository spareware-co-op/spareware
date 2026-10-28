//! The signs of 2 × 2 and 3 × 3 integer determinants in single-precision arithmetic, by Avnaim,
//! Boissonnat, Devillers, Preparata and Yvinec's method: chapter 23.3's reference for the exact
//! paths of orientation (piece G42).
//!
//! What it does: [`sign_of_det2`] finds the sign of x1·y2 − y1·x2 from the four entries without
//! ever forming a product: it compares, subtracts and divides, keeping every value within the
//! entries' own magnitude, and [`orient2d`] applies it to the three points' translations, as the
//! generated `orient2d` does. [`sign_of_det3`] finds the sign of a 3 × 3 determinant the same way,
//! by a walk in the lattice its first two rows span whose every test is a [`sign_of_det2`], and
//! [`orient3d`] applies it to the four points' translations, as the generated `orient3d` does. A
//! product is never computed, so no value needs more bits than a few entries: the method answers
//! in 64-bit arithmetic where the exact paths need 128 bits.
//!
//! Guarantees:
//!
//! - **The exact sign.** For every four entries of magnitude at most [`ENTRY_MAX`], the result is
//!   the sign of x1·y2 − y1·x2 (tested against 128-bit products and against the generated
//!   `orient2d` in `tests/attack_g42.rs`, and against big integers by geom-fuzz's `variants`
//!   target). For every nine entries of magnitude at most [`DET3_ENTRY_MAX`], the result of
//!   [`sign_of_det3`] is the determinant's sign (tested against big integers here, and against
//!   the generated `orient3d` in `tests/attack_g42.rs` and geom-fuzz's `variants` target). An entry
//!   past its bound is refused with `None`, never answered.
//! - **Checked and bounded.** Every step is a checked method. [`sign_of_det2`]'s loop runs at most
//!   [`ITERATIONS`] times: each pass either answers or replaces the second row by one whose entries
//!   are below half the first row's, so the entries halve at least every two passes.
//!   [`sign_of_det3`] runs at most [`DET3_PASSES`] passes, each with a lattice search of at most
//!   119 doublings and 118 halvings, kept in a fixed array.
//!
//! Method: Avnaim, Boissonnat, Devillers, Preparata and Yvinec, "Evaluating signs of determinants
//! using single-precision arithmetic", Algorithmica 17 (1997), sections 2 and 3,
//! doi:10.1007/BF02522822 (the authors' report: <https://inria.hal.science/inria-00090613>). Signs
//! first: an entry of zero leaves one product, and entries of mixed signs settle the sign or are
//! made positive by negating rows and columns. Then, with the rows ordered so that x2 ≥ x1 and
//! y2 ≥ y1, the Euclidean step x2 = k·x1 + xr, yr = y2 − k·y1 keeps the determinant
//! (|x1 y1; xr yr|), and the halves of the first row settle it or give a smaller second row:
//! (xr, yr) when both lie in the lower halves, (x1 − xr, y1 − yr) when both lie in the upper,
//! which negates the determinant. The paper writes that last determinant without the change of
//! sign; it is −|x1 y1; xr yr|, as expanding both shows, and this module follows the expansion.
//!
//! The 3 × 3 case follows section 3. With rows Ui = (ui, zi), D = z1·|u2 u3| + z2·|u3 u1| +
//! z3·|u1 u2|. Each pass makes every zi ≥ 0 and z3 the largest by negating and swapping rows
//! (section 3.3), answers when the three minors share a sign, and writes U3 = κ1·U1 + κ2·U2 +
//! κ3·Ez, so D = κ3·|u1 u2|. The first step (section 3.4) finds the cell of the lattice of u1
//! and u2 that holds u3, by doubling then halving along the lattice lines O u3 crosses, without
//! ever forming the cell's coordinates (up to 2^119); every probe is a 2 × 2 sign, and a probe
//! whose lattice heights lie both below zero or both above z3 settles κ3's sign. R = U3 minus the
//! cell's corner settles it when zR < 0 or zR > z1 + z2. The second step (section 3.6) places R
//! in one of the four triangles the cell's diagonals cut, settles κ3 against the triangle's
//! heights, or replaces U3 by R less the corner nearest in height, whose z is at most half the
//! largest. Collinear u1 and u2 make z3's cofactor zero, so z3 is set to zero (section 3.7).
//! Where this departs from the paper, the algebra decides:
//!
//! - Substep 1.1 reads the cell at O from the preliminary step's minors: κ1 = |u3 u2| / |u1 u2|
//!   and κ2 = |u1 u3| / |u1 u2|, the same signs the paper's three-test search finds.
//! - Equation 5 is printed cu = cv + ε1·u1 + ε2·u2. A cell of coordinates [a, a + 1] along
//!   vi = −ui spans [−a − 1, −a] along ui, so the reference point is cv − ε1·u1 − ε2·u2;
//!   the printed form is two cells off along each negated axis, and this module follows the
//!   definitions.
//! - A probe compares heights with z3 rather than 2^b, which z3 does not exceed: the paper's
//!   argument holds unchanged and settles sooner.
//! - The second step treats all four triangles, settling κ3 < 0 as well as κ3 > 0 (the paper
//!   shows one).
//! - Entries are at most 2^59 − 1 and words are 64 bits, so section 3.5's devices for keeping
//!   every value within b + 1 bits are not needed: the widest value formed, u3 − c(λ) − c(2^h),
//!   is at most five entries in magnitude, within [`ENTRY_MAX`].
//! - A point on a lattice line may be taken in either cell beside it: κ3 = zR − ρ1·z1 − ρ2·z2
//!   settles for ρ1, ρ2 anywhere in [0, 1].
//!
//! Cost: [`sign_of_det2`], at most [`ITERATIONS`] passes of a few comparisons, subtractions and one
//! Euclidean division each. [`sign_of_det3`], at most [`DET3_PASSES`] passes, each at most about
//! 480 calls of [`sign_of_det2`]; its search keeps up to 119 lattice points in a fixed array of
//! 2,856 bytes. No allocation, no recursion.

use core::cmp::Ordering;

use crate::grid::{P2, P3};
use crate::sign::Sign;

/// The largest entry magnitude taken: 2^62 − 1. The method subtracts entries and doubles none,
/// so 2^62 − 1 keeps every difference of two entries, and the negation of each, within `i64`.
pub const ENTRY_MAX: i64 = (1 << 62) - 1;

/// The most passes the loop runs: each pass halves the second row or answers, and the rows swap
/// at most every other pass, so 2 · 63 passes reach a zero entry, which answers.
pub const ITERATIONS: usize = 128;

/// The sign of x · y for nonzero or zero `x` and `y`, without the product.
fn sign_of_product(x: i64, y: i64) -> Sign {
    match (x.cmp(&0), y.cmp(&0)) {
        (Ordering::Equal, _) | (_, Ordering::Equal) => Sign::Zero,
        (a, b) if a == b => Sign::Positive,
        _ => Sign::Negative,
    }
}

/// `s` if `flip` is false, its negation otherwise.
const fn flipped(s: Sign, flip: bool) -> Sign {
    if flip { s.negate() } else { s }
}

/// The sign of the determinant |x1 y1; x2 y2| = x1·y2 − y1·x2, or `None` for an entry of
/// magnitude above [`ENTRY_MAX`].
#[must_use]
pub fn sign_of_det2(x1: i64, y1: i64, x2: i64, y2: i64) -> Option<Sign> {
    if [x1, y1, x2, y2]
        .iter()
        .any(|v| v.unsigned_abs() > ENTRY_MAX.unsigned_abs())
    {
        return None;
    }
    // A zero entry leaves one of the two products.
    if x1 == 0 || y2 == 0 {
        return Some(sign_of_product(y1, x2).negate());
    }
    if y1 == 0 || x2 == 0 {
        return Some(sign_of_product(x1, y2));
    }
    // Make the first column positive by negating rows, each a change of sign.
    let mut flip = false;
    let (mut x1, mut y1, mut x2, mut y2) = (x1, y1, x2, y2);
    if x1 < 0 {
        (x1, y1) = (x1.checked_neg()?, y1.checked_neg()?);
        flip = !flip;
    }
    if x2 < 0 {
        (x2, y2) = (x2.checked_neg()?, y2.checked_neg()?);
        flip = !flip;
    }
    // Mixed signs in the second column settle it: with x1, x2 > 0, D = x1·y2 − y1·x2 has the sign
    // of y2 when y1 and y2 differ in sign.
    match (y1 < 0, y2 < 0) {
        (true, false) => return Some(flipped(Sign::Positive, flip)),
        (false, true) => return Some(flipped(Sign::Negative, flip)),
        (true, true) => {
            (y1, y2) = (y1.checked_neg()?, y2.checked_neg()?);
            flip = !flip;
        }
        (false, false) => {}
    }
    for _ in 0..ITERATIONS {
        // Every entry is positive here except y2, which a pass can bring to zero. x2 never
        // reaches zero: a pass that goes on leaves xr > 0 (xr = 0 answers) or x1 − xr > 0 there.
        if y2 == 0 {
            // D = −y1·x2 with y1, x2 > 0.
            return Some(flipped(Sign::Negative, flip));
        }
        // Order the rows so that x2 ≥ x1 and y2 ≥ y1, or settle the sign.
        if x2 < x1 && y2 < y1 {
            (x1, y1, x2, y2) = (x2, y2, x1, y1);
            flip = !flip;
        } else if x1 <= x2 && y2 < y1 {
            // x1·y2 < x2·y1.
            return Some(flipped(Sign::Negative, flip));
        } else if x1 > x2 {
            // y2 ≥ y1 here (the two tests before take every y2 < y1), so x1·y2 > x2·y1.
            return Some(flipped(Sign::Positive, flip));
        }
        // x2 = k·x1 + xr, 0 ≤ xr < x1, and yr = y2 − k·y1, without forming k·y1 when it would
        // pass the range: then k·y1 > y2 and yr < 0.
        let k = x2.checked_div(x1)?;
        let xr = x2.checked_rem(x1)?;
        let yr = match k.checked_mul(y1) {
            Some(ky) if ky <= y2 => y2.checked_sub(ky)?,
            // k·y1 > y2, so yr < 0 and D = x1·yr − y1·xr < 0.
            _ => return Some(flipped(Sign::Negative, flip)),
        };
        // D = |x1 y1; xr yr| with x1, y1 > 0 and 0 ≤ xr < x1.
        if yr > y1 {
            // x1·yr > x1·y1 > xr·y1.
            return Some(flipped(Sign::Positive, flip));
        }
        if xr == 0 {
            return Some(flipped(
                if yr == 0 { Sign::Zero } else { Sign::Positive },
                flip,
            ));
        }
        // Where xr and yr lie against the halves of x1 and y1: 2·xr against x1 as xr against
        // x1 − xr, which cannot overflow.
        let cx = xr.cmp(&x1.checked_sub(xr)?);
        let cy = yr.cmp(&y1.checked_sub(yr)?);
        match (cx, cy) {
            // D = x1·yr − y1·xr with xr < x1/2 < … : the halves settle it.
            (Ordering::Less, Ordering::Greater) => return Some(flipped(Sign::Positive, flip)),
            (Ordering::Greater, Ordering::Less) => return Some(flipped(Sign::Negative, flip)),
            // 2·xr = x1: D = x1·(yr − y1/2), the sign of 2·yr − y1.
            (Ordering::Equal, c) => return Some(flipped(sign_of_ordering(c), flip)),
            // 2·yr = y1: D = y1·(x1/2 − xr), the sign of x1 − 2·xr.
            (c, Ordering::Equal) => return Some(flipped(sign_of_ordering(c).negate(), flip)),
            // Both in the lower halves: iterate on (x1, y1), (xr, yr).
            (Ordering::Less, Ordering::Less) => (x2, y2) = (xr, yr),
            // Both in the upper halves: iterate on (x1, y1), (x1 − xr, y1 − yr), which negates
            // the determinant.
            (Ordering::Greater, Ordering::Greater) => {
                (x2, y2) = (x1.checked_sub(xr)?, y1.checked_sub(yr)?);
                flip = !flip;
            }
        }
    }
    // Unreachable within ITERATIONS passes (module header); refused rather than guessed.
    None
}

/// The sign an ordering of a value against zero gives.
const fn sign_of_ordering(o: Ordering) -> Sign {
    match o {
        Ordering::Less => Sign::Negative,
        Ordering::Equal => Sign::Zero,
        Ordering::Greater => Sign::Positive,
    }
}

/// `orient2d(a, b, c)` by the method: the sign of |b − a; c − a|, positive when a, b, c turn
/// counterclockwise, as `crate::predicate::orient2d`. Grid translations are at most 2^41 − 2 in
/// magnitude, so every entry is in range and the result is always `Some`.
#[must_use]
pub fn orient2d(a: P2, b: P2, c: P2) -> Option<Sign> {
    let x1 = b.x.get().checked_sub(a.x.get())?;
    let y1 = b.y.get().checked_sub(a.y.get())?;
    let x2 = c.x.get().checked_sub(a.x.get())?;
    let y2 = c.y.get().checked_sub(a.y.get())?;
    sign_of_det2(x1, y1, x2, y2)
}

/// The largest entry magnitude [`sign_of_det3`] takes: 2^59 − 1. The widest value its lattice
/// walk forms, u3 − c(λ) − c(2^h), is at most five entries in magnitude (module header), and
/// 5 · (2^59 − 1) < 2^62 − 1 = [`ENTRY_MAX`], so every 2 × 2 determinant it evaluates is in range.
pub const DET3_ENTRY_MAX: i64 = (1 << 59) - 1;

/// The most passes [`sign_of_det3`] runs. Each pass answers, or replaces the third row by one
/// whose z entry is at most half the largest (section 3.6), or zeroes it (section 3.7), so the
/// largest z entry halves at least every three passes. From at most 2^59 − 1 it is zero after
/// 3 · 59 passes, and the next pass answers.
pub const DET3_PASSES: usize = 3 * 59 + 1;

/// The most lattice points the doubling search keeps (substep 1.3): c(2^l) for every 2^l ≤ α, and
/// α = |u3 q| / |p q| ≤ |u3 q| ≤ 2 · (2^59 − 1)^2 < 2^119, so l ≤ 118.
const SEARCH_DEPTH: usize = 119;

/// A row (x, y, z) of a 3 × 3 determinant, or a point of the lattice its first two rows span.
#[derive(Clone, Copy)]
struct Row {
    x: i64,
    y: i64,
    z: i64,
}

impl Row {
    /// The origin.
    const ZERO: Row = Row { x: 0, y: 0, z: 0 };

    /// `self + o`, or `None` past `i64`.
    fn add(self, o: Row) -> Option<Row> {
        Some(Row {
            x: self.x.checked_add(o.x)?,
            y: self.y.checked_add(o.y)?,
            z: self.z.checked_add(o.z)?,
        })
    }

    /// `self − o`, or `None` past `i64`.
    fn sub(self, o: Row) -> Option<Row> {
        Some(Row {
            x: self.x.checked_sub(o.x)?,
            y: self.y.checked_sub(o.y)?,
            z: self.z.checked_sub(o.z)?,
        })
    }

    /// `−self`, or `None` past `i64`.
    fn neg(self) -> Option<Row> {
        Some(Row {
            x: self.x.checked_neg()?,
            y: self.y.checked_neg()?,
            z: self.z.checked_neg()?,
        })
    }
}

/// The sign of |u v| = ux·vy − uy·vx, the 2 × 2 determinant of the rows' projections onto the
/// horizontal plane.
fn det(u: Row, v: Row) -> Option<Sign> {
    sign_of_det2(u.x, u.y, v.x, v.y)
}

/// What a step of the 3 × 3 method finds.
enum Found {
    /// The sign of κ3: U3's height above the plane H through O, U1 and U2, along Ez.
    Kappa(Sign),
    /// A lattice cell's reference point (substeps 1.2 to 1.5), or the row that replaces U3, which
    /// differs from it by a point of the lattice of U1 and U2 and so keeps the determinant.
    Point(Row),
}

/// The sign of the 3 × 3 determinant with rows `u1`, `u2` and `u3`, each `[x, y, z]`, or `None`
/// for an entry of magnitude above [`DET3_ENTRY_MAX`].
#[must_use]
pub fn sign_of_det3(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> Option<Sign> {
    if u1
        .iter()
        .chain(&u2)
        .chain(&u3)
        .any(|v| v.unsigned_abs() > DET3_ENTRY_MAX.unsigned_abs())
    {
        return None;
    }
    let row = |[x, y, z]: [i64; 3]| Row { x, y, z };
    let (mut a, mut b, mut c) = (row(u1), row(u2), row(u3));
    let mut flip = false;
    for _ in 0..DET3_PASSES {
        // Section 3.3: every z entry made nonnegative by negating rows, and the largest put in
        // the third row by swapping two, each a change of sign.
        for r in [&mut a, &mut b, &mut c] {
            if r.z < 0 {
                *r = r.neg()?;
                flip = !flip;
            }
        }
        if a.z > c.z && a.z >= b.z {
            (a, c) = (c, a);
            flip = !flip;
        } else if b.z > c.z {
            (b, c) = (c, b);
            flip = !flip;
        }
        // Every z entry zero: the last column is zero.
        if c.z == 0 {
            return Some(Sign::Zero);
        }
        // D = z1·|u2 u3| + z2·|u3 u1| + z3·|u1 u2|, with z1, z2 ≥ 0 and z3 > 0.
        let m12 = det(a, b)?;
        if m12.is_zero() {
            // Section 3.7: z3's cofactor |u1 u2| is zero, so z3 may be.
            c.z = 0;
            continue;
        }
        let (m23, m31) = (det(b, c)?, det(c, a)?);
        if m23 == m12 && m31 == m12 {
            // u1, u2 and u3 span the plane positively (or negatively): every term has m12's sign.
            return Some(flipped(m12, flip));
        }
        let found = match first_step(a, b, c, m12, m23, m31)? {
            Found::Point(r) => second_step(a, b, r, m12)?,
            kappa @ Found::Kappa(_) => kappa,
        };
        match found {
            // D = κ3·|u1 u2| (equation 2).
            Found::Kappa(k) => return Some(flipped(k.mul(m12), flip)),
            Found::Point(r) => c = r,
        }
    }
    // Unreachable within DET3_PASSES passes (module header); refused rather than guessed.
    None
}

/// Section 3.4, the first step, for rows with 0 ≤ z1, z2 ≤ z3, z3 > 0 and |u1 u2| ≠ 0, whose
/// minors' signs are `m12` = sign |u1 u2|, `m23` = sign |u2 u3| and `m31` = sign |u3 u1|: the sign
/// of κ3, or R = U3 − k1·U1 − k2·U2 for a cell of the lattice of u1 and u2 that holds u3.
fn first_step(u1: Row, u2: Row, u3: Row, m12: Sign, m23: Sign, m31: Sign) -> Option<Found> {
    // Substep 1.1: u3 = κ1·u1 + κ2·u2 with κ1 = |u3 u2| / |u1 u2| and κ2 = |u1 u3| / |u1 u2|, so
    // κ1 < 0 exactly when |u2 u3| has |u1 u2|'s sign, and κ2 < 0 when |u3 u1| has. With vi = −ui
    // where κi < 0 and ui otherwise, u3 = α·v1 + β·v2 with α, β ≥ 0.
    let (neg1, neg2) = (m23 == m12, m31 == m12);
    let v1 = if neg1 { u1.neg()? } else { u1 };
    let v2 = if neg2 { u2.neg()? } else { u2 };
    let s12 = if neg1 == neg2 { m12 } else { m12.negate() };
    // The basis ordered so that α ≥ β (the paper's K(1) = D2 + v1): |u3, v1 + v2| is
    // (α − β)·|v1 v2|.
    let walk = if det(u3, v1.add(v2)?)?.mul(s12) == Sign::Negative {
        Walk {
            u3,
            p: v2,
            q: v1,
            s: s12.negate(),
        }
    } else {
        Walk {
            u3,
            p: v1,
            q: v2,
            s: s12,
        }
    };
    let cell = match walk.cell()? {
        Found::Point(c) => c,
        kappa @ Found::Kappa(_) => return Some(kappa),
    };
    // The cell's reference point in the basis (u1, u2): it gains vi for each vi = −ui
    // (module header, on equation 5).
    let mut corner = cell;
    if neg1 {
        corner = corner.add(v1)?;
    }
    if neg2 {
        corner = corner.add(v2)?;
    }
    let r = u3.sub(corner)?;
    // Substep 1.6: κ3 = zR − ρ1·z1 − ρ2·z2 with ρ1, ρ2 in [0, 1] and z1, z2 ≥ 0.
    if r.z < 0 {
        return Some(Found::Kappa(Sign::Negative));
    }
    if r.z > u1.z.checked_add(u2.z)? {
        return Some(Found::Kappa(Sign::Positive));
    }
    Some(Found::Point(r))
}

/// Substeps 1.2 to 1.5, in a basis (p, q) of the lattice with u3 = α·p + β·q, α ≥ β ≥ 0. K(λ) is
/// the lattice line {λ·p + t·q}, and c(λ) = λ·p + ⌊λ·β/α⌋·q, so the segment O u3 crosses K(λ),
/// when λ ≤ α, between c(λ) and c(λ) + q. Only the points c(λ) are formed, never λ.
struct Walk {
    /// The third row.
    u3: Row,
    /// The basis vector along which the search doubles.
    p: Row,
    /// The other basis vector.
    q: Row,
    /// The sign of |p q|.
    s: Sign,
}

impl Walk {
    /// Whether O u3 reaches K(λ), for `w` on it: α ≥ λ, as |u3 − w, q| = (α − λ)·|p q|.
    fn reaches(&self, w: Row) -> Option<bool> {
        Some(det(self.u3.sub(w)?, self.q)?.mul(self.s) != Sign::Negative)
    }

    /// c(λ) from `w` = λ·p + μ·q with μ ≤ λ·β/α < μ + 2: w + q when μ + 1 ≤ λ·β/α, which is
    /// |u3, w + q| = (α·(μ + 1) − β·λ)·|p q| ≤ 0, and w otherwise.
    fn crossing(&self, w: Row) -> Option<Row> {
        let up = w.add(self.q)?;
        Some(if det(self.u3, up)?.mul(self.s) == Sign::Positive {
            w
        } else {
            up
        })
    }

    /// Substeps 1.3.1 and 1.4.1, for c = c(λ) with λ ≤ α and `up` = c + q: O u3 crosses K(λ) at
    /// t·u3 for some t in (0, 1], where the plane H's height lies between those of c and c + q,
    /// and H's height at u3 is that divided by t. Both below zero put H below U3 there (κ3 > 0);
    /// both above z3 put it above (κ3 < 0).
    fn settles(&self, c: Row, up: Row) -> Option<Sign> {
        let (low, high) = (c.z, up.z);
        if low < 0 && high < 0 {
            Some(Sign::Positive)
        } else if low > self.u3.z && high > self.u3.z {
            Some(Sign::Negative)
        } else {
            None
        }
    }

    /// The reference point, in the basis (p, q), of a lattice cell holding u3, or the sign of κ3
    /// when a probe settles it.
    fn cell(&self) -> Option<Found> {
        // Substep 1.2: α < 1, and β ≤ α: u3 is in the cell at O.
        if !self.reaches(self.p)? {
            return Some(Found::Point(Row::ZERO));
        }
        // Substep 1.3: c(1), c(2), c(4), … while O u3 reaches K(2^l), each kept; 2·c(λ) is on
        // K(2λ).
        let mut kept = [Row::ZERO; SEARCH_DEPTH];
        let mut depth = 0_usize;
        let mut c = self.crossing(self.p)?;
        loop {
            if let Some(k) = self.settles(c, c.add(self.q)?) {
                return Some(Found::Kappa(k));
            }
            *kept.get_mut(depth)? = c;
            depth = depth.checked_add(1)?;
            let w = c.add(c)?;
            if !self.reaches(w)? {
                break;
            }
            c = self.crossing(w)?;
        }
        // Substep 1.4: c = c(λ) with λ = 2^k. For h from k − 1 down to 0, c(λ) + c(2^h) is on
        // K(λ + 2^h), and λ grows by 2^h when O u3 reaches that line; λ ends at ⌊α⌋.
        let mut h = depth.checked_sub(1)?;
        while h > 0 {
            h = h.checked_sub(1)?;
            let w = c.add(*kept.get(h)?)?;
            if self.reaches(w)? {
                c = self.crossing(w)?;
                if let Some(k) = self.settles(c, c.add(self.q)?) {
                    return Some(Found::Kappa(k));
                }
            }
        }
        // Substep 1.5: u3 is in the cell at c or the one at c + q, as |p, u3 − c − q| is
        // (β − ⌊λ·β/α⌋ − 1)·|p q|.
        let up = c.add(self.q)?;
        Some(Found::Point(
            if det(self.p, self.u3.sub(up)?)?.mul(self.s) == Sign::Negative {
                c
            } else {
                up
            },
        ))
    }
}

/// Section 3.6, the second step, for R with (xR, yR) = s·u1 + t·u2, s and t in [0, 1], and
/// 0 ≤ zR ≤ z1 + z2: the sign of κ3, or R − W for a corner W of the cell, whose z entry is at most
/// max(z1, z2)/2 in magnitude.
fn second_step(u1: Row, u2: Row, r: Row, m12: Sign) -> Option<Found> {
    // The diagonals cut the cell into four triangles of two corners and the centre:
    // |r, u1 + u2| = (s − t)·|u1 u2| and |r − u1, u2 − u1| = (s + t − 1)·|u1 u2| say which.
    let far = u1.add(u2)?;
    let s_at_least_t = det(r, far)?.mul(m12) != Sign::Negative;
    let sum_at_least_one = det(r.sub(u1)?, u2.sub(u1)?)?.mul(m12) != Sign::Negative;
    let (w1, w2) = match (s_at_least_t, sum_at_least_one) {
        (true, false) => (Row::ZERO, u1),
        (true, true) => (u1, far),
        (false, true) => (far, u2),
        (false, false) => (u2, Row::ZERO),
    };
    // κ3 = zR − (s·z1 + t·z2), and over the triangle s·z1 + t·z2 lies between the least and the
    // greatest of its corners' heights and the centre's, (z1 + z2)/2; all compared doubled.
    let z = r.z.checked_mul(2)?;
    let heights = [
        w1.z.checked_mul(2)?,
        w2.z.checked_mul(2)?,
        u1.z.checked_add(u2.z)?,
    ];
    if heights.iter().all(|&h| z > h) {
        return Some(Found::Kappa(Sign::Positive));
    }
    if heights.iter().all(|&h| z < h) {
        return Some(Found::Kappa(Sign::Negative));
    }
    // zR lies within the triangle's heights, so the nearer corner's height is within
    // max(z1, z2)/2 of it: the corners' heights differ by z1 or z2, and the centre's is within
    // max(z1, z2)/2 of the nearer corner's.
    let (r1, r2) = (r.sub(w1)?, r.sub(w2)?);
    Some(Found::Point(
        if r1.z.unsigned_abs() <= r2.z.unsigned_abs() {
            r1
        } else {
            r2
        },
    ))
}

/// `orient3d(a, b, c, d)` by the method: the sign of |b − a; c − a; d − a|, the triple product
/// (b − a) · ((c − a) × (d − a)) the generated `orient3d` evaluates. Grid translations are at most
/// 2^41 − 2 in magnitude, within [`DET3_ENTRY_MAX`], so the result is always `Some`.
#[must_use]
pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> Option<Sign> {
    let from_a = |p: P3| -> Option<[i64; 3]> {
        Some([
            p.x.get().checked_sub(a.x.get())?,
            p.y.get().checked_sub(a.y.get())?,
            p.z.get().checked_sub(a.z.get())?,
        ])
    };
    sign_of_det3(from_a(b)?, from_a(c)?, from_a(d)?)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests: oracles and generators"
)]
mod tests {
    use super::*;

    /// The exact sign by 128-bit products.
    fn exact(x1: i64, y1: i64, x2: i64, y2: i64) -> Sign {
        Sign::of_i128(i128::from(x1) * i128::from(y2) - i128::from(y1) * i128::from(x2))
    }

    /// `SplitMix64` (Steele, Lea and Flood, 2014).
    struct Mix(u64);

    impl Mix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// An entry of a random bit length and sign, or an extreme, zero or a small value.
        fn entry(&mut self) -> i64 {
            let v = match self.next() % 8 {
                0 => ENTRY_MAX,
                1 => 0,
                2 => (self.next() % 5) as i64,
                _ => (self.next() >> (2 + self.next() % 62)) as i64,
            };
            if self.next() % 2 == 0 { v } else { -v }
        }
    }

    #[test]
    fn agrees_with_128_bit_products() {
        let mut mix = Mix(0x0642_a7a1_0000_0001);
        for _ in 0..1_000_000 {
            let [x1, y1, x2, y2] = core::array::from_fn(|_| mix.entry());
            assert_eq!(
                sign_of_det2(x1, y1, x2, y2),
                Some(exact(x1, y1, x2, y2)),
                "{x1} {y1} {x2} {y2}"
            );
        }
    }

    #[test]
    #[allow(
        clippy::many_single_char_names,
        reason = "the rows' entries, as the paper names them"
    )]
    fn agrees_on_near_degenerate_entries() {
        // Rows nearly proportional: (x, y) and (k·x + e, k·y + f) for small e and f.
        let mut mix = Mix(0x0642_a7a1_0000_0002);
        for _ in 0..200_000 {
            let x = (mix.next() >> 34) as i64 + 1;
            let y = (mix.next() >> 34) as i64 + 1;
            let k = (mix.next() % 1_000) as i64 + 1;
            let (e, f) = ((mix.next() % 3) as i64 - 1, (mix.next() % 3) as i64 - 1);
            let (x2, y2) = (k * x + e, k * y + f);
            assert_eq!(sign_of_det2(x, y, x2, y2), Some(exact(x, y, x2, y2)));
            assert_eq!(sign_of_det2(-x, y, x2, -y2), Some(exact(-x, y, x2, -y2)));
        }
    }

    #[test]
    fn every_sign_pattern_and_the_limits() {
        let m = ENTRY_MAX;
        for x1 in [-m, -1, 0, 1, m] {
            for y1 in [-m, -1, 0, 1, m] {
                for x2 in [-m, -1, 0, 1, m] {
                    for y2 in [-m, -1, 0, 1, m] {
                        assert_eq!(sign_of_det2(x1, y1, x2, y2), Some(exact(x1, y1, x2, y2)));
                    }
                }
            }
        }
        assert_eq!(sign_of_det2(m + 1, 1, 1, 1), None);
        assert_eq!(sign_of_det2(1, i64::MIN, 1, 1), None);
    }

    /// The exact sign of the 3 × 3 determinant with rows `u1`, `u2` and `u3`, by big integers.
    fn exact3(u1: [i64; 3], u2: [i64; 3], u3: [i64; 3]) -> Sign {
        use num_bigint::BigInt;
        let n = BigInt::from;
        let minor = |a: i64, b: i64, c: i64, d: i64| n(a) * n(b) - n(c) * n(d);
        let ([x1, y1, z1], [x2, y2, z2], [x3, y3, z3]) = (u1, u2, u3);
        let d = n(x1) * minor(y2, z3, z2, y3) - n(y1) * minor(x2, z3, z2, x3)
            + n(z1) * minor(x2, y3, y2, x3);
        match d.sign() {
            num_bigint::Sign::Minus => Sign::Negative,
            num_bigint::Sign::NoSign => Sign::Zero,
            num_bigint::Sign::Plus => Sign::Positive,
        }
    }

    impl Mix {
        /// An entry for `sign_of_det3`: a random bit length and sign, or an extreme, zero or a
        /// small value.
        fn entry3(&mut self) -> i64 {
            let v = match self.next() % 8 {
                0 => DET3_ENTRY_MAX,
                1 => 0,
                2 => (self.next() % 5) as i64,
                _ => (self.next() >> (5 + self.next() % 59)) as i64,
            };
            if self.next() % 2 == 0 { v } else { -v }
        }
    }

    #[test]
    fn det3_agrees_with_big_integers() {
        let mut mix = Mix(0x0642_a7a1_0000_0003);
        for _ in 0..200_000 {
            let r: [[i64; 3]; 3] = core::array::from_fn(|_| core::array::from_fn(|_| mix.entry3()));
            assert_eq!(
                sign_of_det3(r[0], r[1], r[2]),
                Some(exact3(r[0], r[1], r[2])),
                "{r:?}"
            );
        }
    }

    #[test]
    fn det3_agrees_near_the_plane_of_the_first_two_rows() {
        // U3 = k1·U1 + k2·U2 + e: on or one step from the plane, so κ3 is zero or tiny, and the
        // passes run long.
        let mut mix = Mix(0x0642_a7a1_0000_0004);
        for _ in 0..50_000 {
            let bits = 1 + mix.next() % 28;
            let mut small = || {
                let v = (mix.next() >> (64 - bits)) as i64;
                if mix.next() % 2 == 0 { v } else { -v }
            };
            let u1 = [small(), small(), small()];
            let u2 = [small(), small(), small()];
            let (k1, k2) = (small() >> 1, small() >> 1);
            let e = (mix.next() % 3) as i64 - 1;
            let axis = (mix.next() % 3) as usize;
            let mut u3: [i64; 3] = core::array::from_fn(|i| k1 * u1[i] + k2 * u2[i]);
            u3[axis] += e;
            assert_eq!(
                sign_of_det3(u1, u2, u3),
                Some(exact3(u1, u2, u3)),
                "{u1:?} {u2:?} {u3:?}"
            );
            assert_eq!(sign_of_det3(u3, u1, u2), Some(exact3(u3, u1, u2)));
        }
    }

    #[test]
    fn det3_agrees_when_the_lattice_walk_runs_deep() {
        // u1 = (a, a + 1) and u2 = (a − 1, a) have |u1 u2| = 1, so a large u3 has lattice
        // coordinates near 2^118 and the doubling search runs its full depth.
        let mut mix = Mix(0x0642_a7a1_0000_0005);
        for _ in 0..20_000 {
            let a = (mix.next() >> 6) as i64 - (1 << 57);
            let z = |mix: &mut Mix| (mix.next() >> 5) as i64 - (1 << 58);
            let u1 = [a, a + 1, z(&mut mix)];
            let u2 = [a - 1, a, z(&mut mix)];
            let u3 = [mix.entry3(), mix.entry3(), mix.entry3()];
            assert_eq!(
                sign_of_det3(u1, u2, u3),
                Some(exact3(u1, u2, u3)),
                "{u1:?} {u2:?} {u3:?}"
            );
            // And on the plane: u3 an integer combination of the first two rows, whose
            // coordinates are then small.
            let a = (mix.next() >> 36) as i64;
            let (k1, k2) = (
                (mix.next() >> 35) as i64 - (1 << 28),
                (mix.next() >> 35) as i64,
            );
            let u1 = [a, a + 1, (mix.next() >> 36) as i64];
            let u2 = [a - 1, a, (mix.next() >> 36) as i64];
            let e = (mix.next() % 3) as i64 - 1;
            let u3 = [
                k1 * u1[0] + k2 * u2[0],
                k1 * u1[1] + k2 * u2[1],
                k1 * u1[2] + k2 * u2[2] + e,
            ];
            assert_eq!(
                sign_of_det3(u1, u2, u3),
                Some(exact3(u1, u2, u3)),
                "{u1:?} {u2:?} {u3:?}"
            );
        }
    }

    #[test]
    fn det3_every_sign_pattern_and_the_limits() {
        let m = DET3_ENTRY_MAX;
        let values = [-m, 0, 1, m];
        for i in 0..4_usize.pow(9) {
            let pick = |j: u32| values[i / 4_usize.pow(j) % 4];
            let u1 = [pick(0), pick(1), pick(2)];
            let u2 = [pick(3), pick(4), pick(5)];
            let u3 = [pick(6), pick(7), pick(8)];
            assert_eq!(
                sign_of_det3(u1, u2, u3),
                Some(exact3(u1, u2, u3)),
                "{u1:?} {u2:?} {u3:?}"
            );
        }
        assert_eq!(sign_of_det3([m + 1, 0, 0], [0, 1, 0], [0, 0, 1]), None);
        assert_eq!(sign_of_det3([1, 0, 0], [0, 1, 0], [0, 0, -m - 1]), None);
        assert_eq!(
            sign_of_det3([1, 0, 0], [0, 1, 0], [0, 0, 1]),
            Some(Sign::Positive)
        );
    }
}
