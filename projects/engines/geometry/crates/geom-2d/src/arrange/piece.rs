//! The pieces an arrangement sweeps: segments, and arcs cut where their circles turn back in x, with
//! every exact question the sweep and the face walk ask of them.
//!
//! What it does: a segment is one [`Piece`]. An arc is cut at the leftmost and the rightmost point
//! of its circle wherever one lies inside it ([`cut`]), so that each piece runs one way in x and lies
//! on one half of its circle, the upper (from the leftmost point over the top to the rightmost) or
//! the lower. Its ends are [`At`]s: an arc's own ends are grid points, and a cut is the one-root
//! point `Crossing2::of_extreme` gives. The sweep asks which side of a piece a point lies on
//! ([`side`]), where two pieces meet ([`meetings`]), and in what order pieces leave a point
//! ([`order_at`]): by the half-plane their tangent points into, counter-clockwise from the negative x
//! direction, then by the tangent's turn, then by curvature. The circles are listed once each
//! ([`circles_of`]), so two arcs of one circle name the same circle.
//!
//! Guarantees:
//!
//! - **Exact.** Every answer is a sign of `geom-num`'s exact questions: `Carrier::side`,
//!   `Round::side`, `Crossing2::side`, `Crossing2::against_round`, `cmp_x` and `cmp_y`,
//!   `Rational2::orient`, `Round::meets` and the crossings of `Crossing2`, or `sign_of_root` on the
//!   whole numbers written out below. An interval filter on a crossing's certified enclosure answers
//!   first, and only when its sign is certain.
//! - **Ordered as the curves leave a point.** Two pieces leaving one point are ordered by their
//!   tangents, and pieces with one tangent by signed curvature (a line's is zero, a counter-clockwise
//!   arc's 1 / r and a clockwise one's −1 / r), so a tangency is a touching vertex and the curve that
//!   bends left more lies above (chapter 14.4). Equal tangent and equal curvature at one point means
//!   one curve: an overlap.
//! - **Nothing overflows silently.** Every step is a checked operation, at the width its derivation
//!   below needs; a step that did not fit would be an error, never an answer. With a, b, k and w a
//!   crossing's parts (below 2^628, 2^209, 2^837 and 2^585: `geom_num::one_root`'s widest column),
//!   a rational point's X and Y below 2^124 and W below 2^83, and a direction's parts below 2^42:
//!
//!   | Question | As whole numbers | Width |
//!   | --- | --- | --- |
//!   | A direction against the way from a centre to a rational point | d · (X·W' − X'·W, Y·W' − Y'·W) | 251 bits, in 512 |
//!   | The same, to a crossing | α + β√k, α = d · (A·W' − X'·w), β = W'(d · B) | α 755, β 335 bits, in 1,024 |
//!   | The turn of a crossing and two centres | α + β√k, Uᵢ = Cᵢ·w − A·Wᵢ, Vᵢ = B·Wᵢ, α = U₁ × U₂, β = −(U₁ × V₂ + V₁ × U₂) | α 1,425, β 1,006 bits, in 1,536 |
//!   | Two radii | rr · W'² against rr' · W² | 418 bits, in 512 |
//!
//!   `sign_of_root` then squares α and β²k within its 80 limbs: below 2^2850 and 2^2851.
//! - **Deterministic.** Integers, and certified intervals whose answers are exact signs; no platform
//!   maths. The circles' order and each circle's stored form are chosen by exact comparison and then
//!   by the integers of the input, never by the input's order.
//!
//! Method: Bentley and Ottmann's sweep generalised to x-monotone curves, as de Berg, Cheong, van
//! Kreveld and Overmars, *Computational Geometry*, 3rd ed., section 2.1 (doi:10.1007/978-3-540-77974-2)
//! and Fogel, Halperin and Wein, *CGAL Arrangements and Their Applications*, 2012, chapter 5
//! (<https://doi.org/10.1007/978-3-642-17283-0>), describe it; curves cut into x-monotone pieces and
//! curves at a point ordered by tangent and then curvature, as chapter 14.4 sets out
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`). The book only, as 14.2 says: CGAL's
//! source not read (29.2). A tangent's turn is a cross product: for a counter-clockwise arc about C
//! the tangent at p is p − C turned a quarter, so its cross with a direction d is d · (p − C) and
//! with another arc's is the turn of p and the two centres.
//!
//! Cost: constant time per question: a filter of a few interval operations, then at worst a few
//! dozen products of at most 24 limbs and one `sign_of_root`. [`cut`] makes at most three pieces;
//! [`circles_of`] sorts the arcs' circles, O(n log n) comparisons. No recursion.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::arc::{Arc2, Contact, Disc, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::interval::Interval;
use geom_num::one_root::{Crossing2, Root, enclose, sign_of_root};
use geom_num::wide::{I512, Wide};
use geom_num::{NumError, Sign};

use super::{ArrangeError, At, failed, set};

/// Limbs for the way from a centre to a crossing: α, the widest, is 755 bits.
const MID_LIMBS: usize = 16;

const _: () = assert!(755 < MID_LIMBS * 64);

/// Integers for the way from a centre to a crossing.
type Mid = Wide<MID_LIMBS>;

/// Limbs for the turn of a crossing and two centres: α, the widest, is 1,425 bits.
const LONG_LIMBS: usize = 24;

const _: () = assert!(1_425 < LONG_LIMBS * 64);

/// Integers for the turn of a crossing and two centres.
type Long = Wide<LONG_LIMBS>;

/// The order a sign means.
pub(super) const fn ordering(s: Sign) -> Ordering {
    match s {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    }
}

/// Lexicographic order of (x, y).
pub(super) fn cmp_pt(a: &At, b: &At) -> Result<Ordering, ArrangeError> {
    match ordering(a.cmp_x(b)?) {
        Ordering::Equal => Ok(ordering(a.cmp_y(b)?)),
        other => Ok(other),
    }
}

/// The cross product of two directions, as a sign; each part below 2^42, so each product is below
/// 2^84 and fits.
pub(super) fn cross_sign(a: [i128; 2], b: [i128; 2]) -> Result<Ordering, ArrangeError> {
    let [ax, ay] = a;
    let [bx, by] = b;
    let l = ax.checked_mul(by).ok_or(ArrangeError::Range)?;
    let r = ay.checked_mul(bx).ok_or(ArrangeError::Range)?;
    Ok(l.cmp(&r))
}

/// A circle the arcs lie on, with what its questions need.
#[derive(Clone, Debug)]
pub(super) struct Circle {
    /// The circle, in the form its canonical arc gives it.
    pub(super) round: Round,
    /// Its centre.
    centre: Rational2,
    /// Its centre as a one-root point, for comparison with one.
    middle: Crossing2,
    /// The same circle about its centre, with its squared radius over the centre's weight squared.
    disc: Disc,
}

impl Circle {
    /// The circle `round`, with its centre and squared radius read out once.
    fn of(round: Round) -> Result<Circle, ArrangeError> {
        let centre = round.centre();
        Ok(Circle {
            round,
            centre,
            middle: Crossing2::of_point(&centre)?,
            disc: Disc::of_round(&round)?,
        })
    }
}

/// What a piece lies on.
#[derive(Clone, Copy, Debug)]
pub(super) enum On {
    /// A line: its smallest carrier, running from the piece's lower end to its higher, and that
    /// direction, hi − lo divided by the parts' common factor.
    Line {
        /// The carrier.
        line: Carrier,
        /// The way it runs.
        dir: [i128; 2],
    },
    /// One half of a listed circle.
    Arc {
        /// The circle's place in the list.
        circle: u32,
        /// Whether the piece is on the upper half.
        upper: bool,
    },
}

/// A piece of the sweep, from its lexicographically lower end to its higher.
#[derive(Clone, Debug)]
pub(super) struct Piece {
    /// The lower end.
    pub(super) lo: At,
    /// The higher end.
    pub(super) hi: At,
    /// What it lies on.
    pub(super) on: On,
}

/// Which way a curve leaves a point: along a line, or along a circle, counter-clockwise or not. Its
/// tangent at p is the line's direction, or p − C turned a quarter counter-clockwise (or the other
/// way), and its curvature 0, 1 / r or −1 / r.
#[derive(Clone, Copy, Debug)]
pub(super) enum Way<'a> {
    /// Along a line, this way.
    Line([i128; 2]),
    /// Along a circle.
    Arc {
        /// The circle.
        circle: &'a Circle,
        /// Whether it runs counter-clockwise.
        ccw: bool,
    },
}

/// The circle at `at` in the list.
pub(super) fn circle_of(circles: &[Circle], at: u32) -> Result<&Circle, ArrangeError> {
    usize::try_from(at)
        .ok()
        .and_then(|i| circles.get(i))
        .ok_or(ArrangeError::TooMany)
}

/// The way `piece` runs from its lower end, or from its higher end back when `back`.
pub(super) fn way<'a>(
    piece: &Piece,
    circles: &'a [Circle],
    back: bool,
) -> Result<Way<'a>, ArrangeError> {
    Ok(match piece.on {
        On::Line { dir: [dx, dy], .. } => {
            if back {
                let turned = |v: i128| v.checked_neg().ok_or(ArrangeError::Range);
                Way::Line([turned(dx)?, turned(dy)?])
            } else {
                Way::Line([dx, dy])
            }
        }
        // Along the lower half from its left end the way runs counter-clockwise; along the upper,
        // clockwise; back, the other way.
        On::Arc { circle, upper } => Way::Arc {
            circle: circle_of(circles, circle)?,
            ccw: upper == back,
        },
    })
}

/// How `p`'s x (`across`) or y compares with the centre of `c`.
fn from_centre(p: &At, c: &Circle, across: bool) -> Result<Sign, NumError> {
    match (p, across) {
        (At::Rational(r), true) => r.cmp_x(&c.centre),
        (At::Rational(r), false) => r.cmp_y(&c.centre),
        (At::Root(z), true) => z.cmp_x(&c.middle),
        (At::Root(z), false) => z.cmp_y(&c.middle),
    }
}

/// Where `p` lies against the circle: inside, on it or outside.
fn against(p: &At, c: &Circle) -> Result<Sign, NumError> {
    match p {
        At::Rational(r) => c.round.side(r),
        At::Root(z) => z.against_round(&c.round),
    }
}

/// Where `p` lies against `piece`: [`Sign::Positive`] above it, [`Sign::Zero`] on it,
/// [`Sign::Negative`] below, for a point whose x the piece spans. Above a line is its left side, as
/// it runs from its lower end to its higher. Above the upper half is outside the circle and not below
/// the centre; above the lower half is inside the circle, or above the centre.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn side(piece: &Piece, circles: &[Circle], p: &At) -> Result<Sign, ArrangeError> {
    match piece.on {
        On::Line { line, .. } => Ok(match p {
            At::Rational(r) => line.side(r)?,
            At::Root(z) => z.side(&line)?,
        }),
        On::Arc { circle, upper } => {
            let c = circle_of(circles, circle)?;
            let level = from_centre(p, c, false)?;
            if upper {
                if level == Sign::Negative {
                    return Ok(Sign::Negative);
                }
                Ok(against(p, c)?)
            } else {
                if level == Sign::Positive {
                    return Ok(Sign::Positive);
                }
                Ok(against(p, c)?.negate())
            }
        }
    }
}

/// Whether `q`, a point of `piece`'s line or circle past a point the piece has begun at, lies on the
/// piece: not past its higher end, and on its half of a circle. [`meetings`] asks only of points past
/// the sweep's point, which every piece in the status has begun at, so `q` is past the lower end.
fn holds(piece: &Piece, circles: &[Circle], q: &At) -> Result<bool, ArrangeError> {
    if cmp_pt(q, &piece.hi)? == Ordering::Greater {
        return Ok(false);
    }
    match piece.on {
        On::Line { .. } => Ok(true),
        On::Arc { circle, upper } => {
            let level = from_centre(q, circle_of(circles, circle)?, false)?;
            Ok(if upper {
                level != Sign::Negative
            } else {
                level != Sign::Positive
            })
        }
    }
}

/// A crossing, boxed as a vertex.
fn root(z: &Crossing2) -> At {
    At::Root(Box::new(*z))
}

/// Where the line `l` meets the circle `c`: none, one point at a tangency, or two.
fn line_and_circle(l: &Carrier, c: &Circle) -> Result<[Option<At>; 2], ArrangeError> {
    let at = |r: Root| Crossing2::of_round(l, &c.round, r).map(|z| root(&z));
    Ok(match c.round.meets(l)? {
        Contact::Apart => [None, None],
        Contact::Tangent => [Some(at(Root::First)?), None],
        Contact::Crossing => [Some(at(Root::First)?), Some(at(Root::Second)?)],
    })
}

/// Where two different circles meet: none (apart, nested or about one centre), or the two roots,
/// one point twice at a tangency.
fn two_circles(c: &Circle, d: &Circle) -> Result<[Option<At>; 2], ArrangeError> {
    let at = |r: Root| -> Result<Option<At>, ArrangeError> {
        match Crossing2::of_rounds(&c.round, &d.round, r) {
            Err(NumError::Degenerate) => Ok(None),
            z => Ok(Some(root(&z?))),
        }
    };
    Ok([at(Root::First)?, at(Root::Second)?])
}

/// The points after `after` where `s` and `t` meet, on both pieces: none, one or two. Two pieces of
/// one circle meet only at their ends, which are events already, and two pieces of one line likewise.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn meetings(
    s: &Piece,
    t: &Piece,
    circles: &[Circle],
    after: &At,
) -> Result<[Option<At>; 2], ArrangeError> {
    let found = match (s.on, t.on) {
        (On::Line { line: l, dir: a }, On::Line { line: m, dir: b }) => {
            if cross_sign(a, b)? == Ordering::Equal {
                [None, None]
            } else {
                [Some(At::Rational(l.meet(&m)?)), None]
            }
        }
        (On::Line { line, .. }, On::Arc { circle, .. })
        | (On::Arc { circle, .. }, On::Line { line, .. }) => {
            line_and_circle(&line, circle_of(circles, circle)?)?
        }
        (On::Arc { circle: i, .. }, On::Arc { circle: j, .. }) => {
            if i == j {
                [None, None]
            } else {
                two_circles(circle_of(circles, i)?, circle_of(circles, j)?)?
            }
        }
    };
    let mut out = [None, None];
    for (slot, q) in out.iter_mut().zip(found) {
        if let Some(q) = q
            && cmp_pt(&q, after)? == Ordering::Greater
            && holds(s, circles, &q)?
            && holds(t, circles, &q)?
        {
            *slot = Some(q);
        }
    }
    Ok(out)
}

/// The signs of the x and y parts of the tangent of `w` at `p`: for a circle, p − C turned a quarter
/// counter-clockwise is (−(y − cy), x − cx), (cx, cy) the centre.
fn tangent(p: &At, w: &Way<'_>) -> Result<(Sign, Sign), ArrangeError> {
    Ok(match w {
        Way::Line([dx, dy]) => (Sign::of_i128(*dx), Sign::of_i128(*dy)),
        Way::Arc { circle, ccw } => {
            let tx = from_centre(p, circle, false)?.negate();
            let ty = from_centre(p, circle, true)?;
            if *ccw {
                (tx, ty)
            } else {
                (tx.negate(), ty.negate())
            }
        }
    })
}

/// The half-plane of a tangent, counter-clockwise from the negative x direction: 0 for the lower half
/// with the negative x direction itself, 1 for the upper half with the positive x direction.
fn half(t: (Sign, Sign)) -> u8 {
    let (tx, ty) = t;
    u8::from(!(ty == Sign::Negative || (ty == Sign::Zero && tx == Sign::Negative)))
}

/// Whether `w` leaves `p` exactly in the negative x direction and does not bend left: before the
/// wedge that points west and a hair south, which is where a component's outside lies at its
/// lowest-leftmost vertex when the sweep line is tilted.
pub(super) fn west_not_left(p: &At, w: &Way<'_>) -> Result<bool, ArrangeError> {
    let bends_left = matches!(w, Way::Arc { ccw: true, .. });
    Ok(!bends_left && tangent(p, w)? == (Sign::Negative, Sign::Zero))
}

/// An integer as a certified interval.
fn interval(v: i128) -> Result<Interval, NumError> {
    enclose(&I512::from_i128(v))
}

/// A rational number X / W as a certified interval.
fn fraction(x: i128, w: i128) -> Result<Interval, NumError> {
    interval(x)?.div(interval(w)?)
}

/// A checked product of wide integers.
fn mul<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    a.checked_mul(b).ok_or(NumError::BudgetExceeded)
}

/// A checked sum.
fn add<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    a.checked_add(b).ok_or(NumError::BudgetExceeded)
}

/// A checked difference.
fn sub<const L: usize>(a: &Wide<L>, b: &Wide<L>) -> Result<Wide<L>, NumError> {
    a.checked_sub(b).ok_or(NumError::BudgetExceeded)
}

/// A wide integer at width `M`.
fn wide<const L: usize, const M: usize>(v: &Wide<L>) -> Result<Wide<M>, NumError> {
    v.resize().ok_or(NumError::BudgetExceeded)
}

/// The sign of d · (p − C), the direction `d` against the way from the centre of `c` to `p`.
fn dot_from(p: &At, d: [i128; 2], c: &Circle) -> Result<Sign, NumError> {
    let ([cx, cy], cw) = c.centre.coordinates();
    let [dx, dy] = d;
    match p {
        At::Rational(r) => {
            // Over the weights' product, positive: d · (X · W' − X' · W, Y · W' − Y' · W).
            let ([x, y], w) = r.coordinates();
            let n = I512::from_i128;
            let part = |v: i128, cv: i128| sub(&mul(&n(v), &n(cw))?, &mul(&n(cv), &n(w))?);
            let value = add(&mul(&n(dx), &part(x, cx)?)?, &mul(&n(dy), &part(y, cy)?)?)?;
            Ok(value.signum())
        }
        At::Root(z) => {
            let [ix, iy] = z.enclosure();
            let value = interval(dx)?
                .mul(ix.sub(fraction(cx, cw)?)?)?
                .add(interval(dy)?.mul(iy.sub(fraction(cy, cw)?)?)?)?;
            if let Some(sign @ (Sign::Positive | Sign::Negative)) = value.certain_sign() {
                return Ok(sign);
            }
            // p = (A + B√k) / w, so p − C over w · W' is (A · W' − C · w) + B · W' √k.
            let ([ax, ay], [bx, by], k, w) = z.parts();
            let n = Mid::from_i128;
            let (w, cw) = (wide::<14, MID_LIMBS>(&w)?, n(cw));
            let part = |a: &Mid, cv: i128| sub(&mul(a, &cw)?, &mul(&n(cv), &w)?);
            let x_term = mul(&n(dx), &part(&wide(&ax)?, cx)?)?;
            let y_term = mul(&n(dy), &part(&wide(&ay)?, cy)?)?;
            let alpha = add(&x_term, &y_term)?;
            let along = add(&mul(&n(dx), &wide(&bx)?)?, &mul(&n(dy), &wide(&by)?)?)?;
            let beta = mul(&cw, &along)?;
            sign_of_root(&alpha, &beta, &wide::<14, MID_LIMBS>(&k)?)
        }
    }
}

/// The turn of `p` and the centres of `c` and `e`: the sign of (C − p) × (E − p).
fn turn(p: &At, c: &Circle, e: &Circle) -> Result<Sign, NumError> {
    match p {
        At::Rational(r) => Rational2::orient(r, &c.centre, &e.centre),
        At::Root(z) => {
            let [ix, iy] = z.enclosure();
            let towards = |q: &Circle| -> Result<[Interval; 2], NumError> {
                let ([x, y], w) = q.centre.coordinates();
                Ok([fraction(x, w)?.sub(ix)?, fraction(y, w)?.sub(iy)?])
            };
            let ([ux, uy], [vx, vy]) = (towards(c)?, towards(e)?);
            let value = ux.mul(vy)?.sub(uy.mul(vx)?)?;
            if let Some(sign @ (Sign::Positive | Sign::Negative)) = value.certain_sign() {
                return Ok(sign);
            }
            // C − p over W · w is U − V√k, U = C · w − A · W and V = B · W; the cross of two such
            // is U₁ × U₂ − (U₁ × V₂ + V₁ × U₂)√k, the k term B × B being zero.
            let ([ax, ay], [bx, by], k, w) = z.parts();
            let (ax, ay, bx, by, w) = (
                wide::<14, LONG_LIMBS>(&ax)?,
                wide::<14, LONG_LIMBS>(&ay)?,
                wide::<14, LONG_LIMBS>(&bx)?,
                wide::<14, LONG_LIMBS>(&by)?,
                wide::<14, LONG_LIMBS>(&w)?,
            );
            let parts = |q: &Circle| -> Result<([Long; 2], [Long; 2]), NumError> {
                let ([x, y], qw) = q.centre.coordinates();
                let n = Long::from_i128;
                let u = [
                    sub(&mul(&n(x), &w)?, &mul(&ax, &n(qw))?)?,
                    sub(&mul(&n(y), &w)?, &mul(&ay, &n(qw))?)?,
                ];
                Ok((u, [mul(&bx, &n(qw))?, mul(&by, &n(qw))?]))
            };
            let cross = |a: &[Long; 2], b: &[Long; 2]| -> Result<Long, NumError> {
                let ([a0, a1], [b0, b1]) = (a, b);
                sub(&mul(a0, b1)?, &mul(a1, b0)?)
            };
            let ((u1, v1), (u2, v2)) = (parts(c)?, parts(e)?);
            let alpha = cross(&u1, &u2)?;
            let beta = add(&cross(&u1, &v2)?, &cross(&v1, &u2)?)?
                .checked_neg()
                .ok_or(NumError::BudgetExceeded)?;
            sign_of_root(&alpha, &beta, &wide::<14, LONG_LIMBS>(&k)?)
        }
    }
}

/// The sign of the cross product of the tangents of `a` and `b` at `p`, positive when `b` turns
/// counter-clockwise from `a`. For a circle the tangent is σ(p − C) turned a quarter, σ = 1
/// counter-clockwise; a quarter turn keeps a cross product, and the cross of d with p − C turned is
/// d · (p − C).
fn cross(p: &At, a: &Way<'_>, b: &Way<'_>) -> Result<Sign, ArrangeError> {
    let by = |ccw: bool, s: Sign| if ccw { s } else { s.negate() };
    Ok(match (a, b) {
        (Way::Line(d), Way::Line(e)) => {
            let [dx, dy] = *d;
            let [ex, ey] = *e;
            let l = dx.checked_mul(ey).ok_or(ArrangeError::Range)?;
            let r = dy.checked_mul(ex).ok_or(ArrangeError::Range)?;
            Sign::of_i128(l.checked_sub(r).ok_or(ArrangeError::Range)?)
        }
        (Way::Line(d), Way::Arc { circle, ccw }) => by(*ccw, dot_from(p, *d, circle)?),
        (Way::Arc { circle, ccw }, Way::Line(d)) => by(*ccw, dot_from(p, *d, circle)?).negate(),
        (
            Way::Arc {
                circle: c,
                ccw: first,
            },
            Way::Arc {
                circle: e,
                ccw: second,
            },
        ) => by(first == second, turn(p, c, e)?),
    })
}

/// How the radius of `c` compares with that of `e`: rr / W² against rr' / W'², cross-multiplied.
fn radius(c: &Circle, e: &Circle) -> Result<Ordering, ArrangeError> {
    let scaled = |a: &Circle, b: &Circle| -> Result<I512, NumError> {
        let rr: I512 = wide(&a.disc.squared_radius())?;
        let (_, w) = b.centre.coordinates();
        let w = I512::from_i128(w);
        mul(&rr, &mul(&w, &w)?)
    };
    Ok(scaled(c, e)?.cmp(&scaled(e, c)?))
}

/// The order of the signed curvatures of `a` and `b`, ascending: a clockwise arc's −1 / r, a line's
/// zero, a counter-clockwise arc's 1 / r.
fn bend(a: &Way<'_>, b: &Way<'_>) -> Result<Ordering, ArrangeError> {
    let turning = |w: &Way<'_>| match w {
        Way::Line(_) => 1_u8,
        Way::Arc { ccw: false, .. } => 0,
        Way::Arc { ccw: true, .. } => 2,
    };
    match turning(a).cmp(&turning(b)) {
        Ordering::Equal => {}
        other => return Ok(other),
    }
    Ok(match (a, b) {
        // Counter-clockwise, the smaller circle bends more; clockwise, the larger bends less.
        (Way::Arc { circle: c, ccw }, Way::Arc { circle: e, .. }) => {
            let wider = radius(c, e)?;
            if *ccw { wider.reverse() } else { wider }
        }
        _ => Ordering::Equal,
    })
}

/// The order of two ways out of `p`, counter-clockwise from the negative x direction: by the
/// half-plane of the tangent, then by its turn, then by curvature, ascending. For pieces leaving a
/// sweep event to the right this is their order just after it, lowest first.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn order_at(p: &At, a: &Way<'_>, b: &Way<'_>) -> Result<Ordering, ArrangeError> {
    let (ha, hb) = (half(tangent(p, a)?), half(tangent(p, b)?));
    if ha != hb {
        return Ok(ha.cmp(&hb));
    }
    // Within a half-plane no two tangents point opposite ways, so a cross of zero is one tangent.
    match cross(p, a, b)? {
        Sign::Positive => Ok(Ordering::Less),
        Sign::Negative => Ok(Ordering::Greater),
        Sign::Zero => bend(a, b),
    }
}

/// The circles of `arcs`, each once, in order of centre x, then y, then radius, and for each arc its
/// circle's place. Of the arcs on one circle, the one whose start, then centre, has the least
/// integers gives the circle its stored form, so the list does not depend on the arcs' order.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit, and [`ArrangeError::TooMany`] past 2^32 circles.
pub(super) fn circles_of(arcs: &[Arc2]) -> Result<(Vec<Circle>, Vec<u32>), ArrangeError> {
    let all: Result<Vec<Circle>, _> = arcs.iter().map(|a| Circle::of(a.round())).collect();
    let all = all?;
    let get = |i: usize| all.get(i).ok_or(ArrangeError::TooMany);
    let same_circle = |c: &Circle, e: &Circle| -> Result<Ordering, ArrangeError> {
        match ordering(c.centre.cmp_x(&e.centre)?) {
            Ordering::Equal => {}
            other => return Ok(other),
        }
        match ordering(c.centre.cmp_y(&e.centre)?) {
            Ordering::Equal => {}
            other => return Ok(other),
        }
        radius(c, e)
    };
    let form = |c: &Circle| {
        let on = c.round.on();
        let ([x, y], w) = c.centre.coordinates();
        (on.x.get(), on.y.get(), w, x, y)
    };
    let mut order: Vec<usize> = (0..all.len()).collect();
    let mut fail = Ok(());
    order.sort_by(|&i, &j| {
        let order = get(i).and_then(|c| {
            let e = get(j)?;
            let by_form = || form(c).cmp(&form(e)).then(i.cmp(&j));
            Ok(same_circle(c, e)?.then_with(by_form))
        });
        order.unwrap_or_else(|err| failed(&mut fail, err))
    });
    fail?;
    let mut circles: Vec<Circle> = Vec::new();
    let mut place: Vec<u32> = alloc::vec![0; all.len()];
    for &i in &order {
        let c = get(i)?;
        let fresh = match circles.last() {
            Some(last) => same_circle(last, c)? != Ordering::Equal,
            None => true,
        };
        if fresh {
            circles.push(c.clone());
        }
        let at = u32::try_from(circles.len().saturating_sub(1));
        set(&mut place, i, at.map_err(|_| ArrangeError::TooMany)?)?;
    }
    Ok((circles, place))
}

/// The pieces of `arc`, on the listed circle `circle`, appended to `out`: cut at the circle's
/// leftmost and rightmost points that lie strictly inside the arc, in the order the arc runs. Going
/// counter-clockwise, x falls along the upper half and rises along the lower, so the way x goes
/// between a piece's ends says its half.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn cut(
    arc: &Arc2,
    circle: u32,
    circles: &[Circle],
    out: &mut Vec<Piece>,
) -> Result<(), ArrangeError> {
    let c = circle_of(circles, circle)?;
    let start = At::Rational(arc.start().into());
    let end = At::Rational(arc.end().into());
    let mut cuts: Vec<Crossing2> = Vec::new();
    for r in [Root::First, Root::Second] {
        let z = Crossing2::of_extreme(&c.disc, r)?;
        let at = root(&z);
        if arc.holds_crossing(&z)? && !at.same(&start)? && !at.same(&end)? {
            cuts.push(z);
        }
    }
    if let [a, b] = cuts.as_slice()
        && arc.order_crossings(a, b)? != Sign::Positive
    {
        cuts.reverse();
    }
    let mut along: Vec<At> = alloc::vec![start];
    along.extend(cuts.iter().map(root));
    along.push(end);
    for (p, q) in along.iter().zip(along.iter().skip(1)) {
        // Two points of one x on a circle are a vertical chord's ends, and the arc between them
        // passes the leftmost or the rightmost point, which was cut: so x never stays.
        let upper = falls(q.cmp_x(p)?).ok_or(ArrangeError::Range)?;
        let (lo, hi) = if upper { (q, p) } else { (p, q) };
        out.push(Piece {
            lo: lo.clone(),
            hi: hi.clone(),
            on: On::Arc { circle, upper },
        });
    }
    Ok(())
}

/// Whether a way x goes falls: `None` if x stays.
const fn falls(x: Sign) -> Option<bool> {
    match x {
        Sign::Negative => Some(true),
        Sign::Positive => Some(false),
        Sign::Zero => None,
    }
}

#[cfg(test)]
mod tests {
    use super::falls;
    use geom_num::Sign;

    #[test]
    fn x_falls_rises_or_stays() {
        assert_eq!(falls(Sign::Negative), Some(true));
        assert_eq!(falls(Sign::Positive), Some(false));
        assert_eq!(falls(Sign::Zero), None);
    }
}
