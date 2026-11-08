//! The pieces an arrangement sweeps: segments, and arcs and ellipses cut where they turn back in x,
//! with every exact question the sweep and the face walk ask of them.
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
//! A whole ellipse is cut at its leftmost and rightmost points, the one-root points
//! `Crossing2::of_ellipse_extreme` gives ([`cut_oval`]), into a lower and an upper half split by the
//! diameter through them: with S its form about its centre K and u = p − K, that diameter is where
//! s(p) = B u + 2C v is zero, the line through the middles of its vertical chords, and the upper
//! half is where s(p) ≥ 0. Its tangent is the form's gradient turned a quarter, (−s(p), r(p)) with
//! r(p) = 2A u + B v, so every side, tangent and turn against a line is the sign of a linear form in
//! p. A line meets it at `Crossing2::of_ellipse`'s one-root points. An ellipse and a circle or
//! another ellipse meet at roots of a quartic, conic-root points this sweep does not place, so
//! [`meetings`] refuses two such pieces as `ArrangeError::Conic` unless their curves do not meet, or
//! each root, narrowed [`NARROW`] levels and enclosed, certainly lies off one of the pieces. The
//! ellipses are listed once each ([`ovals_of`]), one curve however its axis and ratio were written.
//! An arc of an ellipse from one rational point of it counter-clockwise to another is cut at the
//! extremes that lie strictly inside it, on the right of its chord, as every point of a
//! counter-clockwise arc of a convex curve is ([`cut_oval_arc`]); the side of the chord is the turn
//! of the point and the two ends, as for two circles' centres.
//!
//! Guarantees:
//!
//! - **Exact.** Every answer is a sign of `geom-num`'s exact questions: `Carrier::side`,
//!   `Round::side`, `Crossing2::side`, `Crossing2::against_round`, `cmp_x` and `cmp_y`,
//!   `Rational2::orient`, `Round::meets` and the crossings of `Crossing2`, or `sign_of_root` on the
//!   whole numbers written out below; for ellipses `Ellipse2::side`, `Crossing2::against_ellipse`,
//!   `Ellipse2::meets`, `Crossing2::of_ellipse` and `sign_of_root` on the linear forms below. An
//!   interval filter on a crossing's certified enclosure answers first, and only when its sign is
//!   certain.
//! - **Refused, never guessed.** Where an ellipse may meet a circle or another ellipse on the pieces
//!   the sweep holds, the answer is `ArrangeError::Conic`. A pair is let pass only on certified
//!   enclosures of the quartic's roots, so a pair let pass never meets on its pieces.
//! - **Ordered as the curves leave a point.** Two pieces leaving one point are ordered by their
//!   tangents, and pieces with one tangent by signed curvature (a line's is zero, a counter-clockwise
//!   arc's 1 / r and a clockwise one's −1 / r; an ellipse's form is positive definite, so it bends
//!   left exactly when it runs counter-clockwise), so a tangency is a touching vertex and the curve
//!   that bends left more lies above (chapter 14.4). Equal tangent and equal curvature at one point
//!   means one curve: an overlap. A line touches an ellipse only at a rational point, where the
//!   discriminant of where they meet is zero.
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
//!   | A linear form e · (p − K) of an ellipse's (s, r, or d · ∇S, e below 2^170), at a rational point | e · (X − W·K) | 296 bits, in 1,024 |
//!   | The same, at a crossing | α + β√k, α = e · (A − w·K), β = e · B | α 800, β 380 bits, in 1,024 |
//!   | Two ellipses as curves, by their forms' ratios to their levels | A · ℓ' against A' · ℓ | 334 bits, in 512 |
//!
//!   `sign_of_root` then squares α and β²k within its 80 limbs: below 2^2850 and 2^2851. An ellipse's
//!   form is below 2^126 in each coefficient and its level below 2^208 (`Ellipse2::form`).
//! - **Deterministic.** Integers, and certified intervals whose answers are exact signs; no platform
//!   maths. The circles' and ellipses' order and each one's stored form are chosen by exact
//!   comparison and then by the integers of the input, never by the input's order.
//!
//! Method: Bentley and Ottmann's sweep generalised to x-monotone curves, as de Berg, Cheong, van
//! Kreveld and Overmars, *Computational Geometry*, 3rd ed., section 2.1 (doi:10.1007/978-3-540-77974-2)
//! and Fogel, Halperin and Wein, *CGAL Arrangements and Their Applications*, 2012, chapter 5
//! (<https://doi.org/10.1007/978-3-642-17283-0>), describe it; curves cut into x-monotone pieces and
//! curves at a point ordered by tangent and then curvature, as chapter 14.4 sets out
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`). The book only, as 14.2 says: CGAL's
//! source not read (29.2). A tangent's turn is a cross product: for a counter-clockwise arc about C
//! the tangent at p is p − C turned a quarter, so its cross with a direction d is d · (p − C) and
//! with another arc's is the turn of p and the two centres. An ellipse's halves are split by the
//! diameter conjugate to the vertical, which halves every vertical chord, and its tangent is its
//! form's gradient turned a quarter.
//!
//! Cost: constant time per question: a filter of a few interval operations, then at worst a few
//! dozen products of at most 24 limbs and one `sign_of_root`. [`cut`] makes at most three pieces and
//! [`cut_oval`] two; [`circles_of`] and [`ovals_of`] sort, O(n log n) comparisons. Telling whether an
//! ellipse and a circle or another ellipse meet on two pieces costs one `Ellipse2::meeting` and
//! [`NARROW`] levels of narrowing for each of at most four roots. No recursion.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::arc::{Arc2, Contact, Disc, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::P2;
use geom_num::interval::Interval;
use geom_num::one_root::{Crossing2, Root, Stored, enclose, sign_of_root};
use geom_num::quartic::{Quartic, RealRoot, Roots};
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

/// Levels to which a root of an ellipse's quartic is narrowed before its place is read, when the
/// sweep asks whether two curves meet on two pieces: 2^−64 of the parameter, a place within
/// 2^−22 of a step for an ellipse within the budget, as `geom_2d::tangent` reads its joins.
const NARROW: u32 = 64;

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

/// An ellipse the pieces lie on, with what its questions need.
#[derive(Clone, Debug)]
pub(super) struct Oval {
    /// The ellipse, in the form its least-integer copy gives it.
    pub(super) ellipse: Ellipse2,
    /// A, B and C of its form, about its centre: below 2^126 each.
    form: [I512; 3],
    /// Its level, the form's value on it: below 2^208.
    level: I512,
}

impl Oval {
    /// The ellipse `ellipse`, with its form and level read out once.
    fn of(ellipse: Ellipse2) -> Result<Oval, ArrangeError> {
        let [a, b, c] = ellipse.form()?;
        let narrow = |v: &Wide<64>| v.resize::<8>().ok_or(ArrangeError::Range);
        Ok(Oval {
            ellipse,
            form: [narrow(&a)?, narrow(&b)?, narrow(&c)?],
            level: narrow(&ellipse.level()?)?,
        })
    }

    /// Its form's A, B and C at the width of the linear questions.
    fn parts(&self) -> Result<[Mid; 3], NumError> {
        let [a, b, c] = &self.form;
        Ok([wide(a)?, wide(b)?, wide(c)?])
    }

    /// The sign of s(p) = B u + 2C v, with u = p − K, K the centre: zero on the diameter through
    /// the leftmost and rightmost points, which halves every vertical chord (where the form, as a
    /// quadratic in v, has its middle), and positive above it, as C is positive.
    fn level_at(&self, p: &At) -> Result<Sign, NumError> {
        let [_, b, c] = self.parts()?;
        linear(p, &[b, add(&c, &c)?], self.ellipse.centre())
    }

    /// The sign of r(p) = 2A u + B v: the x part of the form's gradient at p, as
    /// [`Oval::level_at`]'s s(p) is its y part.
    fn across_at(&self, p: &At) -> Result<Sign, NumError> {
        let [a, b, _] = self.parts()?;
        linear(p, &[add(&a, &a)?, b], self.ellipse.centre())
    }

    /// The sign of d · ∇S(p), the direction `d` against the form's gradient at p:
    /// (2A dx + B dy) u + (B dx + 2C dy) v, each coefficient below 2^170, d's parts below 2^42.
    fn along_at(&self, p: &At, d: [i128; 2]) -> Result<Sign, NumError> {
        let [a, b, c] = self.parts()?;
        let [dx, dy] = d.map(Mid::from_i128);
        let twice = |v: &Mid| add(v, v);
        let e = [
            add(&mul(&twice(&a)?, &dx)?, &mul(&b, &dy)?)?,
            add(&mul(&b, &dx)?, &mul(&twice(&c)?, &dy)?)?,
        ];
        linear(p, &e, self.ellipse.centre())
    }

    /// Where `p` lies against it: [`Sign::Negative`] inside, [`Sign::Zero`] on it,
    /// [`Sign::Positive`] outside.
    fn against(&self, p: &At) -> Result<Sign, NumError> {
        match p {
            At::Rational(r) => self.ellipse.side(r),
            At::Root(z) => z.against_ellipse(&self.ellipse),
        }
    }

    /// Where the root `t` of a quartic along it lies, certainly: K + M cos θ + (p / q) M⊥ sin θ,
    /// M⊥ = (−My, Mx), from the root's enclosure of cos θ and sin θ, t = tan(θ / 2).
    fn place(&self, t: &RealRoot) -> Result<[Interval; 2], NumError> {
        let [cos, sin] = t.angle()?;
        let k = self.ellipse.centre();
        let [mx, my] = self.ellipse.major();
        let (mx, my) = (interval(i128::from(mx))?, interval(i128::from(my))?);
        let (p, q) = self.ellipse.ratio();
        let ratio = interval(i128::from(p))?.div(interval(i128::from(q))?)?;
        let x = interval(i128::from(k.x.get()))?
            .add(mx.mul(cos)?)?
            .sub(ratio.mul(my)?.mul(sin)?)?;
        let y = interval(i128::from(k.y.get()))?
            .add(my.mul(cos)?)?
            .add(ratio.mul(mx)?.mul(sin)?)?;
        Ok([x, y])
    }

    /// The interval of s(p), [`Oval::level_at`]'s value, over the box `at`.
    fn level_over(&self, at: &[Interval; 2]) -> Result<Interval, NumError> {
        let [_, b, c] = &self.form;
        let k = self.ellipse.centre();
        let [x, y] = at;
        let u = x.sub(interval(i128::from(k.x.get()))?)?;
        let v = y.sub(interval(i128::from(k.y.get()))?)?;
        let twice_c = enclose(c)?.mul(Interval::point(2.0)?)?;
        enclose(b)?.mul(u)?.add(twice_c.mul(v)?)
    }
}

/// The circles and ellipses the pieces lie on, each listed once.
#[derive(Clone, Debug, Default)]
pub(super) struct Shapes {
    /// The circles, as [`circles_of`] lists them.
    pub(super) circles: Vec<Circle>,
    /// The ellipses, as [`ovals_of`] lists them.
    pub(super) ovals: Vec<Oval>,
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
    /// One half of a listed ellipse: the upper half, from its leftmost point over the top to its
    /// rightmost, or the lower, split by the diameter through those two points.
    Ellipse {
        /// The ellipse's place in the list.
        ellipse: u32,
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

/// Which way a curve leaves a point: along a line, or along a circle or an ellipse,
/// counter-clockwise or not. Its tangent at p is the line's direction, p − C turned a quarter
/// counter-clockwise (or the other way), or the ellipse's form's gradient at p turned so; and its
/// curvature 0, 1 / r or −1 / r, or of an ellipse's sign.
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
    /// Along an ellipse.
    Ellipse {
        /// The ellipse.
        oval: &'a Oval,
        /// Its place in the list, which names it.
        index: u32,
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

/// The ellipse at `at` in the list.
pub(super) fn oval_of(ovals: &[Oval], at: u32) -> Result<&Oval, ArrangeError> {
    usize::try_from(at)
        .ok()
        .and_then(|i| ovals.get(i))
        .ok_or(ArrangeError::TooMany)
}

/// The way `piece` runs from its lower end, or from its higher end back when `back`.
pub(super) fn way<'a>(
    piece: &Piece,
    shapes: &'a Shapes,
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
            circle: circle_of(&shapes.circles, circle)?,
            ccw: upper == back,
        },
        // The same holds of an ellipse's halves.
        On::Ellipse { ellipse, upper } => Way::Ellipse {
            oval: oval_of(&shapes.ovals, ellipse)?,
            index: ellipse,
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
/// the centre; above the lower half is inside the circle, or above the centre. An ellipse's halves
/// are the same with the diameter that halves its vertical chords in place of the centre's level.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn side(piece: &Piece, shapes: &Shapes, p: &At) -> Result<Sign, ArrangeError> {
    match piece.on {
        On::Line { line, .. } => Ok(match p {
            At::Rational(r) => line.side(r)?,
            At::Root(z) => z.side(&line)?,
        }),
        On::Arc { circle, upper } => {
            let c = circle_of(&shapes.circles, circle)?;
            let level = from_centre(p, c, false)?;
            Ok(halves(upper, level, || against(p, c))?)
        }
        On::Ellipse { ellipse, upper } => {
            let o = oval_of(&shapes.ovals, ellipse)?;
            Ok(halves(upper, o.level_at(p)?, || o.against(p))?)
        }
    }
}

/// Where a point lies against one half of a closed curve, from its level against the half's
/// dividing line and, when that does not settle it, where it lies against the curve (`against`,
/// negative inside).
fn halves(
    upper: bool,
    level: Sign,
    against: impl FnOnce() -> Result<Sign, NumError>,
) -> Result<Sign, NumError> {
    if upper {
        if level == Sign::Negative {
            return Ok(Sign::Negative);
        }
        against()
    } else {
        if level == Sign::Positive {
            return Ok(Sign::Positive);
        }
        Ok(against()?.negate())
    }
}

/// Whether `q`, a point of `piece`'s line, circle or ellipse past a point the piece has begun at,
/// lies on the piece: not past its higher end, and on its half of a circle or an ellipse.
/// [`meetings`] asks only of points past the sweep's point, which every piece in the status has
/// begun at, so `q` is past the lower end.
fn holds(piece: &Piece, shapes: &Shapes, q: &At) -> Result<bool, ArrangeError> {
    if cmp_pt(q, &piece.hi)? == Ordering::Greater {
        return Ok(false);
    }
    let (level, upper) = match piece.on {
        On::Line { .. } => return Ok(true),
        On::Arc { circle, upper } => (
            from_centre(q, circle_of(&shapes.circles, circle)?, false)?,
            upper,
        ),
        On::Ellipse { ellipse, upper } => (oval_of(&shapes.ovals, ellipse)?.level_at(q)?, upper),
    };
    Ok(if upper {
        level != Sign::Negative
    } else {
        level != Sign::Positive
    })
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

/// Where the line `l` meets the ellipse `o`: none, one point at a tangency, or two.
fn line_and_oval(l: &Carrier, o: &Oval) -> Result<[Option<At>; 2], ArrangeError> {
    let at = |r: Root| Crossing2::of_ellipse(l, &o.ellipse, r).map(|z| root(&z));
    Ok(match o.ellipse.meets(l)? {
        Contact::Apart => [None, None],
        Contact::Tangent => [Some(at(Root::First)?), None],
        Contact::Crossing => [Some(at(Root::First)?), Some(at(Root::Second)?)],
    })
}

/// Where `p` lies, certainly.
fn enclosure_of(p: &At) -> Result<[Interval; 2], NumError> {
    Ok(match p {
        At::Rational(r) => Crossing2::of_point(r)?.enclosure(),
        At::Root(z) => z.enclosure(),
    })
}

/// Whether no point of the box `at` lies on `piece`, on its curve's upper half or lower: the box is
/// wholly left or right of the piece's ends, or `level`, the level over the box of the line dividing
/// the curve's halves, is wholly on the far side of it.
fn certainly_off(
    piece: &Piece,
    upper: bool,
    level: Interval,
    at: &[Interval; 2],
) -> Result<bool, ArrangeError> {
    let [x, _] = at;
    let ([lo, _], [hi, _]) = (enclosure_of(&piece.lo)?, enclosure_of(&piece.hi)?);
    let wrong = if upper {
        Sign::Negative
    } else {
        Sign::Positive
    };
    Ok(x.hi() < lo.lo() || x.lo() > hi.hi() || level.certain_sign() == Some(wrong))
}

/// The curve of a piece [`apart`] weighs: a circle or an ellipse.
#[derive(Clone, Copy)]
enum Rounded<'a> {
    /// A circle.
    Circle(&'a Circle),
    /// An ellipse.
    Oval(&'a Oval),
}

impl Rounded<'_> {
    /// Its equation.
    fn conic(self) -> Result<Conic, NumError> {
        match self {
            Rounded::Circle(c) => Conic::of_round(&c.round),
            Rounded::Oval(o) => Conic::of_ellipse(&o.ellipse),
        }
    }

    /// The level over the box `at` of the line dividing its halves, positive on the upper half's
    /// side: for a circle the height above its centre, for an ellipse [`Oval::level_over`].
    fn level_over(self, at: &[Interval; 2]) -> Result<Interval, NumError> {
        match self {
            Rounded::Circle(c) => {
                let ([_, cy], cw) = c.centre.coordinates();
                let [_, y] = at;
                y.sub(fraction(cy, cw)?)
            }
            Rounded::Oval(o) => o.level_over(at),
        }
    }
}

/// The quartic and roots of a meeting, `None` when the two are one curve.
fn quartic_of(meeting: &Meeting) -> Option<(&Quartic, &Roots)> {
    match meeting {
        Meeting::At { quartic, roots } => Some((quartic, roots)),
        Meeting::Same => None,
    }
}

/// Refuses a piece of an ellipse and a piece of a circle or of another ellipse that may meet, as
/// [`ArrangeError::Conic`]: they meet at roots of a quartic along the ellipse, which this sweep
/// does not place. They are let pass when the curves do not meet at all, or when each root,
/// narrowed [`NARROW`] levels and its place enclosed, certainly lies off one of the two pieces.
///
/// `first` is the piece of the ellipse `oval`, on its upper half or lower, and `other` the other
/// piece, its half and its curve; [`meetings`] gives them so. Two curves that are one are deduplicated
/// before the sweep, and an ellipse of ratio one is its circle, so a meeting is never one curve:
/// were it, it would be refused.
fn apart(
    first: (&Piece, bool, &Oval),
    other: (&Piece, bool, Rounded<'_>),
) -> Result<(), ArrangeError> {
    let (piece, upper, o) = first;
    let (them, their_upper, curve) = other;
    let meeting = o.ellipse.meeting(&curve.conic()?)?;
    let (quartic, roots) = quartic_of(&meeting).ok_or(ArrangeError::Conic)?;
    if roots.count() == 0 {
        return Ok(());
    }
    for r in roots.narrowed(quartic, NARROW)?.iter() {
        let at = o.place(r)?;
        let off_first = certainly_off(piece, upper, o.level_over(&at)?, &at)?;
        if !(off_first || certainly_off(them, their_upper, curve.level_over(&at)?, &at)?) {
            return Err(ArrangeError::Conic);
        }
    }
    Ok(())
}

/// The points after `after` where `s` and `t` meet, on both pieces: none, one or two. Two pieces of
/// one circle or one ellipse meet only at their ends, which are events already, and two pieces of
/// one line likewise.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit, and [`ArrangeError::Conic`] for a piece of an
/// ellipse and a piece of a circle or of another ellipse that may meet ([`apart`]).
pub(super) fn meetings(
    s: &Piece,
    t: &Piece,
    shapes: &Shapes,
    after: &At,
) -> Result<[Option<At>; 2], ArrangeError> {
    let circles = &shapes.circles;
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
        (On::Line { line, .. }, On::Ellipse { ellipse, .. })
        | (On::Ellipse { ellipse, .. }, On::Line { line, .. }) => {
            line_and_oval(&line, oval_of(&shapes.ovals, ellipse)?)?
        }
        (
            On::Ellipse { ellipse: i, upper },
            On::Ellipse {
                ellipse: j,
                upper: also,
            },
        ) => {
            if i != j {
                let (o, e) = (oval_of(&shapes.ovals, i)?, oval_of(&shapes.ovals, j)?);
                apart((s, upper, o), (t, also, Rounded::Oval(e)))?;
            }
            [None, None]
        }
        (
            On::Ellipse { ellipse, upper },
            On::Arc {
                circle,
                upper: also,
            },
        ) => {
            let (o, c) = (
                oval_of(&shapes.ovals, ellipse)?,
                circle_of(circles, circle)?,
            );
            apart((s, upper, o), (t, also, Rounded::Circle(c)))?;
            [None, None]
        }
        (
            On::Arc {
                circle,
                upper: also,
            },
            On::Ellipse { ellipse, upper },
        ) => {
            let (o, c) = (
                oval_of(&shapes.ovals, ellipse)?,
                circle_of(circles, circle)?,
            );
            apart((t, upper, o), (s, also, Rounded::Circle(c)))?;
            [None, None]
        }
    };
    let mut out = [None, None];
    for (slot, q) in out.iter_mut().zip(found) {
        if let Some(q) = q
            && cmp_pt(&q, after)? == Ordering::Greater
            && holds(s, shapes, &q)?
            && holds(t, shapes, &q)?
        {
            *slot = Some(q);
        }
    }
    Ok(out)
}

/// The signs of the x and y parts of the tangent of `w` at `p`: for a circle, p − C turned a quarter
/// counter-clockwise is (−(y − cy), x − cx), (cx, cy) the centre; for an ellipse, its form's
/// gradient turned so, (−s(p), r(p)) as [`Oval::level_at`] and [`Oval::across_at`] say.
fn tangent(p: &At, w: &Way<'_>) -> Result<(Sign, Sign), ArrangeError> {
    let (tx, ty, ccw) = match w {
        Way::Line([dx, dy]) => return Ok((Sign::of_i128(*dx), Sign::of_i128(*dy))),
        Way::Arc { circle, ccw } => (
            from_centre(p, circle, false)?.negate(),
            from_centre(p, circle, true)?,
            *ccw,
        ),
        Way::Ellipse { oval, ccw, .. } => (oval.level_at(p)?.negate(), oval.across_at(p)?, *ccw),
    };
    Ok(if ccw {
        (tx, ty)
    } else {
        (tx.negate(), ty.negate())
    })
}

/// The half-plane of a tangent, counter-clockwise from the negative x direction: 0 for the lower half
/// with the negative x direction itself, 1 for the upper half with the positive x direction.
fn half(t: (Sign, Sign)) -> u8 {
    let (tx, ty) = t;
    u8::from(!(ty == Sign::Negative || (ty == Sign::Zero && tx == Sign::Negative)))
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

/// The sign of e · (p − K), with K a grid point and e's parts below 2^170: a linear form in p. Over
/// p's weight it is e · (X − W · K) at a rational point, below 2^296, and α + β√k at a one-root
/// point, α = e · (A − w · K) below 2^800 and β = e · B below 2^380, as `geom_num::one_root`'s
/// widest column bounds A at 2^628, w at 2^585 and B at 2^209.
fn linear(p: &At, e: &[Mid; 2], k: P2) -> Result<Sign, NumError> {
    let [ex, ey] = e;
    let (kx, ky) = (k.x.get(), k.y.get());
    match p {
        At::Rational(r) => {
            let ([x, y], w) = r.coordinates();
            let n = Mid::from_i128;
            let part = |v: i128, c: i64| sub(&n(v), &mul(&Mid::from_i64(c), &n(w))?);
            Ok(add(&mul(ex, &part(x, kx)?)?, &mul(ey, &part(y, ky)?)?)?.signum())
        }
        At::Root(z) => {
            let [ix, iy] = z.enclosure();
            let from = |i: Interval, c: i64| -> Result<Interval, NumError> {
                i.sub(interval(i128::from(c))?)
            };
            let value = enclose(ex)?
                .mul(from(ix, kx)?)?
                .add(enclose(ey)?.mul(from(iy, ky)?)?)?;
            if let Some(sign @ (Sign::Positive | Sign::Negative)) = value.certain_sign() {
                return Ok(sign);
            }
            let ([ax, ay], [bx, by], root, w) = z.parts();
            let w: Mid = wide(&w)?;
            let part = |a: &Stored, c: i64| sub(&wide(a)?, &mul(&Mid::from_i64(c), &w)?);
            let alpha = add(&mul(ex, &part(&ax, kx)?)?, &mul(ey, &part(&ay, ky)?)?)?;
            let beta = add(&mul(ex, &wide(&bx)?)?, &mul(ey, &wide(&by)?)?)?;
            sign_of_root(&alpha, &beta, &wide::<14, MID_LIMBS>(&root)?)
        }
    }
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
    orient(p, &c.centre, &e.centre)
}

/// The turn of `p` and the rational points `c` and `e`, each within a circle's centre's budget: the
/// sign of (C − p) × (E − p), positive when p, C and E run counter-clockwise.
fn orient(p: &At, c: &Rational2, e: &Rational2) -> Result<Sign, NumError> {
    match p {
        At::Rational(r) => Rational2::orient(r, c, e),
        At::Root(z) => {
            let [ix, iy] = z.enclosure();
            let towards = |q: &Rational2| -> Result<[Interval; 2], NumError> {
                let ([x, y], w) = q.coordinates();
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
            let parts = |q: &Rational2| -> Result<([Long; 2], [Long; 2]), NumError> {
                let ([x, y], qw) = q.coordinates();
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
/// d · (p − C). For an ellipse it is σ∇S(p) turned, so the cross of d with it is d · ∇S(p). An
/// ellipse and a circle or another ellipse leave one point only where they meet, which
/// [`meetings`] refuses: [`ArrangeError::Conic`].
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
        (Way::Line(d), Way::Ellipse { oval, ccw, .. }) => by(*ccw, oval.along_at(p, *d)?),
        (Way::Ellipse { oval, ccw, .. }, Way::Line(d)) => by(*ccw, oval.along_at(p, *d)?).negate(),
        (Way::Ellipse { index: i, .. }, Way::Ellipse { index: j, .. }) if i == j => Sign::Zero,
        (Way::Arc { .. } | Way::Ellipse { .. }, Way::Ellipse { .. })
        | (Way::Ellipse { .. }, Way::Arc { .. }) => return Err(ArrangeError::Conic),
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
/// zero, a counter-clockwise arc's 1 / r. An ellipse's form is positive definite, so it bends left
/// exactly when it runs counter-clockwise, as a circle does; two pieces of one ellipse leaving a point
/// one way are one curve, and an ellipse beside a circle or another ellipse is [`cross`]'s refusal.
fn bend(a: &Way<'_>, b: &Way<'_>) -> Result<Ordering, ArrangeError> {
    let turning = |w: &Way<'_>| match w {
        Way::Line(_) => 1_u8,
        Way::Arc { ccw: false, .. } | Way::Ellipse { ccw: false, .. } => 0,
        Way::Arc { ccw: true, .. } | Way::Ellipse { ccw: true, .. } => 2,
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
        // Two lines, or two pieces of one ellipse leaving one way, which are one curve: [`cross`]
        // refuses an ellipse beside a circle or another ellipse before their curvatures are asked.
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
    listed(&all?, same_circle, form)
}

/// The curves of `all`, each once, in `same`'s order, and for each item of `all` its curve's place.
/// Of the items `same` finds one curve, the one whose `form`, then place in `all`, is least gives
/// the curve its stored form, so the list does not depend on the input's order.
///
/// # Errors
///
/// The first refusal of `same`, and [`ArrangeError::TooMany`] past 2^32 curves.
fn listed<T: Clone, K: Ord>(
    all: &[T],
    same: impl Fn(&T, &T) -> Result<Ordering, ArrangeError>,
    form: impl Fn(&T) -> K,
) -> Result<(Vec<T>, Vec<u32>), ArrangeError> {
    let get = |i: usize| all.get(i).ok_or(ArrangeError::TooMany);
    let mut order: Vec<usize> = (0..all.len()).collect();
    let mut fail = Ok(());
    order.sort_by(|&i, &j| {
        let order = get(i).and_then(|c| {
            let e = get(j)?;
            let by_form = || form(c).cmp(&form(e)).then(i.cmp(&j));
            Ok(same(c, e)?.then_with(by_form))
        });
        order.unwrap_or_else(|err| failed(&mut fail, err))
    });
    fail?;
    let mut once: Vec<T> = Vec::new();
    let mut place: Vec<u32> = alloc::vec![0; all.len()];
    for &i in &order {
        let c = get(i)?;
        let fresh = match once.last() {
            Some(last) => same(last, c)? != Ordering::Equal,
            None => true,
        };
        if fresh {
            once.push(c.clone());
        }
        let at = u32::try_from(once.len().saturating_sub(1));
        set(&mut place, i, at.map_err(|_| ArrangeError::TooMany)?)?;
    }
    Ok((once, place))
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

/// How two ellipses compare as curves: by centre x, then y, then the ratios A / ℓ, B / ℓ and C / ℓ
/// of their forms to their levels, cross-multiplied, each product below 2^334; equal exactly when
/// they are one curve, whatever major-axis vector and ratio wrote them.
fn same_oval(o: &Oval, e: &Oval) -> Result<Ordering, ArrangeError> {
    let (k, m) = (o.ellipse.centre(), e.ellipse.centre());
    let by_centre = (k.x.get(), k.y.get()).cmp(&(m.x.get(), m.y.get()));
    if by_centre != Ordering::Equal {
        return Ok(by_centre);
    }
    for (a, b) in o.form.iter().zip(&e.form) {
        let here = mul(a, &e.level)?;
        let there = mul(b, &o.level)?;
        match here.cmp(&there) {
            Ordering::Equal => {}
            other => return Ok(other),
        }
    }
    Ok(Ordering::Equal)
}

/// The ellipses of `ellipses`, each once, in [`same_oval`]'s order, and for each its place. Of the
/// copies of one curve, the one whose major-axis vector, then ratio, has the least integers gives it
/// its stored form, so the list does not depend on the input's order.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit, and [`ArrangeError::TooMany`] past 2^32 ellipses.
pub(super) fn ovals_of(ellipses: &[Ellipse2]) -> Result<(Vec<Oval>, Vec<u32>), ArrangeError> {
    let all: Result<Vec<Oval>, _> = ellipses.iter().map(|e| Oval::of(*e)).collect();
    let form = |o: &Oval| {
        let [mx, my] = o.ellipse.major();
        let (p, q) = o.ellipse.ratio();
        (mx, my, p, q)
    };
    listed(&all?, same_oval, form)
}

/// The pieces of the arc of the listed ellipse at `ellipse` that runs counter-clockwise from the
/// rational point `start` of it to `end`, appended to `out`, cut at its leftmost and rightmost points
/// where they lie strictly inside the arc: on the right of the chord from `start` to `end`, as every
/// point of a counter-clockwise arc of a convex curve is. Going counter-clockwise the rightmost
/// point comes before the upper half and the leftmost after it, so from a start on the upper half
/// the leftmost cut comes first. Each piece is the half its ends' order in x says. `start` equal to
/// `end` is the whole ellipse.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn cut_oval_arc(
    ellipse: u32,
    ovals: &[Oval],
    start: &Rational2,
    end: &Rational2,
    out: &mut Vec<Piece>,
) -> Result<(), ArrangeError> {
    if start.same(end)? {
        return cut_oval(ellipse, ovals, out);
    }
    let o = oval_of(ovals, ellipse)?;
    let (from, to) = (At::Rational(*start), At::Rational(*end));
    let first = if o.level_at(&from)? == Sign::Negative {
        [Root::Second, Root::First]
    } else {
        [Root::First, Root::Second]
    };
    let mut along: Vec<At> = alloc::vec![from];
    for r in first {
        let z = root(&Crossing2::of_ellipse_extreme(&o.ellipse, r)?);
        if orient(&z, start, end)? == Sign::Negative {
            along.push(z);
        }
    }
    along.push(to);
    for (p, q) in along.iter().zip(along.iter().skip(1)) {
        // A cut lies between any two points of one x, so two neighbours never share one.
        let upper = falls(q.cmp_x(p)?).ok_or(ArrangeError::Range)?;
        let (lo, hi) = if upper { (q, p) } else { (p, q) };
        out.push(Piece {
            lo: lo.clone(),
            hi: hi.clone(),
            on: On::Ellipse { ellipse, upper },
        });
    }
    Ok(())
}

/// The two pieces of the whole listed ellipse at `ellipse`, appended to `out`: its lower half and
/// then its upper, each from its leftmost point to its rightmost, the one-root points
/// `Crossing2::of_ellipse_extreme` gives.
///
/// # Errors
///
/// [`ArrangeError::Range`] if a step did not fit.
pub(super) fn cut_oval(
    ellipse: u32,
    ovals: &[Oval],
    out: &mut Vec<Piece>,
) -> Result<(), ArrangeError> {
    let o = oval_of(ovals, ellipse)?;
    let left = root(&Crossing2::of_ellipse_extreme(&o.ellipse, Root::First)?);
    let right = root(&Crossing2::of_ellipse_extreme(&o.ellipse, Root::Second)?);
    for upper in [false, true] {
        out.push(Piece {
            lo: left.clone(),
            hi: right.clone(),
            on: On::Ellipse { ellipse, upper },
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
    use super::{falls, quartic_of};
    use geom_num::Sign;
    use geom_num::conic::Meeting;

    #[test]
    fn one_curve_has_no_quartic() {
        assert!(quartic_of(&Meeting::Same).is_none());
    }

    #[test]
    fn x_falls_rises_or_stays() {
        assert_eq!(falls(Sign::Negative), Some(true));
        assert_eq!(falls(Sign::Positive), Some(false));
        assert_eq!(falls(Sign::Zero), None);
    }
}
