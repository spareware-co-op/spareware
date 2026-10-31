//! The reference implementation of the 2D arrangement (chapter 28.9): lines and circular arcs, their
//! arrangement, booleans, offsets and the boundary identity of chapter 28.3, in one-root numbers, by
//! brute force, to check `geom-2d` against.
//!
//! What it does: a [`OneRoot`] is a real number a + b√k with a, b and k rational. A line with
//! rational coefficients and a circle with a rational centre and squared radius are closed under
//! crossing: every crossing is a [`Point`] whose two coordinates share one root. A [`Piece`] is an
//! oriented stretch of a line or an arc monotone in x, and a [`Region`] is a set of pieces forming
//! closed chains with the interior on their left.
//!
//! - [`Arrangement`] cuts pieces at every crossing and counts their vertices, edges, components and
//!   faces, and walks each face's boundary from the order of the edges around each vertex.
//! - [`boolean`] cuts two regions at every crossing, and keeps each piece by what a ray from one
//!   point of it says about the other region.
//! - [`offset`] is the region grown or shrunk by a disc: the union with, or difference from, the
//!   stroke of every piece, by [`boolean`].
//! - [`boundary_sum`] is the signed sum of oriented pieces after every piece is cut at every
//!   crossing, and [`boundary_identity`] is chapter 28.3's law on it: the pieces of A ∪ B and A ∩ B
//!   sum to those of A and B.
//!
//! Guarantees:
//!
//! - **Exact.** Every value is exact, and every comparison is the sign of a + b√p + c√q, decided by
//!   squaring in rationals. Nothing rounds and there is no tolerance: tangency is a discriminant
//!   that is exactly zero.
//! - **No nested roots.** A sample point on a piece has a rational x, or a rational y on an upright
//!   line, and its ray runs along that rational line, so every crossing with the ray is one-root
//!   again and is compared along the line.
//! - **Independent of the kernel.** Written from the definitions below, without reading
//!   `geom_num::arc`, `geom_num::one_root` or `geom-2d`, sharing no code with the kernel and calling
//!   none (chapter 28.9: a different author, a different agent session).
//! - **Refused, never guessed.** A negative root, a point off its circle, an open chain and an
//!   offset that would leave the one-root numbers are [`ReferenceError`]s.
//! - **Regularised.** A boolean's result is the closure of its interior: a shared stretch of
//!   boundary with interior on both sides leaves nothing behind.
//!
//! Method: arcs split at their left and right extremes and comparison of one-root numbers by
//! repeated squaring, from the published description of circle-segment arrangements by Fogel,
//! Halperin and Wein, *CGAL Arrangements and Their Applications*, 2012,
//! <https://doi.org/10.1007/978-3-642-17283-0>, whose source is not read (29.2); ray parity for
//! location; Euler's formula for the faces; a boolean by selecting the boundary pieces each operator
//! keeps, Tilove's boundary classification, "Set membership classification", IEEE Trans. Computers
//! C-29(10), 1980, <https://doi.org/10.1109/TC.1980.1675457>; an offset as a Minkowski sum with a disc.
//!
//! Cost: `n` pieces cost O(n²) crossing tests and O(n²) for the rays, on rationals that grow with
//! each cut; an offset repeats that for each piece. This reference accepts that, because it runs on
//! small cases only. No recursion.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision rationals, which cannot, and every division is by a value shown nonzero
// beside it.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision rationals, which cannot overflow"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "points and vectors, named as the geometry they come from names them"
)]

use core::cmp::Ordering;
use core::fmt;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

/// An exact rational.
pub type Q = RBig;

/// What the reference refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceError {
    /// The square root of a negative number.
    NegativeRoot,
    /// A point that is not on the circle it was given for.
    OffCircle,
    /// A piece of no length, or a polygon of fewer than three corners.
    Degenerate,
    /// Chains that do not close: some point has more pieces in than out.
    Open,
    /// An offset that needs a value these numbers do not hold: a corner that is not rational, a line
    /// whose unit normal is not, or an arc whose radius is not.
    NotRational,
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReferenceError::NegativeRoot => "the square root of a negative number",
            ReferenceError::OffCircle => "a point off its circle",
            ReferenceError::Degenerate => "a degenerate piece or polygon",
            ReferenceError::Open => "chains that do not close",
            ReferenceError::NotRational => "an offset outside the one-root numbers",
        })
    }
}

impl std::error::Error for ReferenceError {}

/// √x when that is rational; x in lowest terms is a square of rationals only if both its parts are
/// squares.
#[must_use]
pub fn root(x: &Q) -> Option<Q> {
    if *x < Q::ZERO {
        return None;
    }
    let top: UBig = x.numerator().clone().try_into().ok()?;
    let bottom = x.denominator();
    let (r, s) = (top.nth_root(2), bottom.nth_root(2));
    (&r * &r == top && &s * &s == *bottom).then(|| Q::from_parts(IBig::from(r), s))
}

fn sign(x: &Q) -> Ordering {
    x.cmp(&Q::ZERO)
}

/// The sign of a + b√k.
fn sign_one(a: &Q, b: &Q, k: &Q) -> Ordering {
    let (s, t) = (
        sign(a),
        if *k == Q::ZERO {
            Ordering::Equal
        } else {
            sign(b)
        },
    );
    if t == Ordering::Equal {
        return s;
    }
    if s == Ordering::Equal || s == t {
        return t;
    }
    // Opposite signs: the larger square wins.
    match (a * a).cmp(&(b * b * k)) {
        Ordering::Greater => s,
        Ordering::Equal => Ordering::Equal,
        Ordering::Less => t,
    }
}

/// The sign of b√p + c√q.
fn sign_two(b: &Q, p: &Q, c: &Q, q: &Q) -> Ordering {
    let (s, t) = (
        if *p == Q::ZERO {
            Ordering::Equal
        } else {
            sign(b)
        },
        if *q == Q::ZERO {
            Ordering::Equal
        } else {
            sign(c)
        },
    );
    if t == Ordering::Equal {
        return s;
    }
    if s == Ordering::Equal || s == t {
        return t;
    }
    match (b * b * p).cmp(&(c * c * q)) {
        Ordering::Greater => s,
        Ordering::Equal => Ordering::Equal,
        Ordering::Less => t,
    }
}

/// The sign of a + b√p + c√q: with u = b√p + c√q, when a and u differ in sign the sign of a² − u²
/// says which is larger, and a² − u² = (a² − b²p − c²q) − 2bc√(pq) is one root again.
fn sign_three(a: &Q, b: &Q, p: &Q, c: &Q, q: &Q) -> Ordering {
    let (s, t) = (sign(a), sign_two(b, p, c, q));
    if t == Ordering::Equal {
        return s;
    }
    if s == Ordering::Equal || s == t {
        return t;
    }
    let rest = a * a - b * b * p - c * c * q;
    let two = Q::ONE + Q::ONE;
    match sign_one(&rest, &(-(two * b * c)), &(p * q)) {
        Ordering::Greater => s,
        Ordering::Equal => Ordering::Equal,
        Ordering::Less => t,
    }
}

/// A real number a + b√k, with a, b and k rational and k ≥ 0. Ordered by value.
#[derive(Clone, Debug)]
pub struct OneRoot {
    a: Q,
    b: Q,
    k: Q,
}

impl OneRoot {
    /// a + b√k, folded to a rational when √k is one.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::NegativeRoot`] if k is negative.
    pub fn new(a: Q, b: Q, k: Q) -> Result<OneRoot, ReferenceError> {
        if k < Q::ZERO {
            return Err(ReferenceError::NegativeRoot);
        }
        Ok(match root(&k) {
            Some(r) => OneRoot::rational(a + b * r),
            None if b == Q::ZERO => OneRoot::rational(a),
            None => OneRoot { a, b, k },
        })
    }

    /// The rational `a`.
    #[must_use]
    pub const fn rational(a: Q) -> OneRoot {
        OneRoot {
            a,
            b: Q::ZERO,
            k: Q::ZERO,
        }
    }

    /// The value, if it is rational.
    #[must_use]
    pub fn as_rational(&self) -> Option<&Q> {
        (self.b == Q::ZERO).then_some(&self.a)
    }

    /// The sign of the value.
    #[must_use]
    pub fn sign(&self) -> Ordering {
        sign_one(&self.a, &self.b, &self.k)
    }

    /// The value plus the rational `q`.
    fn plus(&self, q: &Q) -> OneRoot {
        OneRoot {
            a: &self.a + q,
            b: self.b.clone(),
            k: self.k.clone(),
        }
    }

    /// The value times the rational `q`.
    fn times(&self, q: &Q) -> OneRoot {
        OneRoot {
            a: &self.a * q,
            b: &self.b * q,
            k: self.k.clone(),
        }
    }

    /// The root the two values share: one of a point's coordinates, or a vertex's, may be rational.
    fn shared(&self, other: &OneRoot) -> Q {
        if self.b == Q::ZERO {
            other.k.clone()
        } else {
            self.k.clone()
        }
    }

    /// The sum of two values on one root.
    fn sum(&self, other: &OneRoot) -> OneRoot {
        OneRoot {
            k: self.shared(other),
            a: &self.a + &other.a,
            b: &self.b + &other.b,
        }
    }

    /// The product of two values on one root.
    fn product(&self, other: &OneRoot) -> OneRoot {
        let k = self.shared(other);
        OneRoot {
            a: &self.a * &other.a + &self.b * &other.b * &k,
            b: &self.a * &other.b + &self.b * &other.a,
            k,
        }
    }

    /// A rational at or below the value: √k ≤ 1 + k.
    fn floor(&self) -> Q {
        &self.a - abs(&self.b) * (Q::ONE + &self.k) - Q::ONE
    }

    /// A rational at or above the value.
    fn ceiling(&self) -> Q {
        &self.a + abs(&self.b) * (Q::ONE + &self.k) + Q::ONE
    }
}

fn abs(x: &Q) -> Q {
    if *x < Q::ZERO { -x.clone() } else { x.clone() }
}

impl PartialEq for OneRoot {
    fn eq(&self, other: &OneRoot) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for OneRoot {}

impl PartialOrd for OneRoot {
    fn partial_cmp(&self, other: &OneRoot) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OneRoot {
    fn cmp(&self, other: &OneRoot) -> Ordering {
        sign_three(
            &(&self.a - &other.a),
            &self.b,
            &self.k,
            &(-&other.b),
            &other.k,
        )
    }
}

/// A rational strictly between `lo` and `hi`, which are in order, by bisection.
fn between(lo: &OneRoot, hi: &OneRoot) -> Q {
    let (mut bottom, mut top) = (lo.floor(), hi.ceiling());
    let two = Q::ONE + Q::ONE;
    loop {
        let middle = (&bottom + &top) / &two;
        let at = OneRoot::rational(middle.clone());
        if at <= *lo {
            bottom = middle;
        } else if at >= *hi {
            top = middle;
        } else {
            return middle;
        }
    }
}

/// A point of the plane, its coordinates on one root. Ordered by x, then y.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    x: OneRoot,
    y: OneRoot,
}

impl Point {
    /// The rational point (x, y).
    #[must_use]
    pub const fn at(x: Q, y: Q) -> Point {
        Point {
            x: OneRoot::rational(x),
            y: OneRoot::rational(y),
        }
    }

    /// The point (x, y), which must share a root.
    #[must_use]
    pub const fn new(x: OneRoot, y: OneRoot) -> Point {
        Point { x, y }
    }

    /// x.
    #[must_use]
    pub const fn x(&self) -> &OneRoot {
        &self.x
    }

    /// y.
    #[must_use]
    pub const fn y(&self) -> &OneRoot {
        &self.y
    }

    /// The coordinates, if both are rational.
    fn rational(&self) -> Option<(&Q, &Q)> {
        Some((self.x.as_rational()?, self.y.as_rational()?))
    }

    /// The square of the distance from the rational point (cx, cy).
    fn power(&self, cx: &Q, cy: &Q) -> OneRoot {
        let (dx, dy) = (self.x.plus(&-cx), self.y.plus(&-cy));
        dx.product(&dx).sum(&dy.product(&dy))
    }
}

/// What a piece runs along: a line, or a circle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Carrier {
    /// a·x + b·y = c, scaled so that the first of a and b that is not zero is 1: one line has one
    /// form.
    Line {
        /// a.
        a: Q,
        /// b.
        b: Q,
        /// c.
        c: Q,
    },
    /// The circle about (x, y) of squared radius `r2`.
    Circle {
        /// The centre's x.
        x: Q,
        /// The centre's y.
        y: Q,
        /// The squared radius.
        r2: Q,
    },
}

/// Where two carriers meet: the points, and whether they touch rather than cross.
struct Meeting {
    points: Vec<Point>,
    touching: bool,
}

impl Meeting {
    const NOWHERE: Meeting = Meeting {
        points: Vec::new(),
        touching: false,
    };
}

/// A line by its coefficients, in its one form.
fn line(a: &Q, b: &Q, c: &Q) -> Carrier {
    let scale = if *a == Q::ZERO { b } else { a };
    Carrier::Line {
        a: a / scale,
        b: b / scale,
        c: c / scale,
    }
}

/// a·x + b·y = c with the circle: from the line's foot, along it by the root of the rest.
fn line_circle(a: &Q, b: &Q, c: &Q, x: &Q, y: &Q, r2: &Q) -> Meeting {
    let size = a * a + b * b;
    let e = (c - a * x - b * y) / &size;
    let (fx, fy) = (x + a * &e, y + b * &e);
    let rest = r2 - &e * &e * &size;
    match sign(&rest) {
        Ordering::Less => Meeting::NOWHERE,
        Ordering::Equal => Meeting {
            points: vec![Point::at(fx, fy)],
            touching: true,
        },
        Ordering::Greater => {
            let k = rest / size;
            // √k is a value here or the coordinates fold: `OneRoot::new` cannot refuse a k above 0.
            let along = |s: Q| {
                let (bx, by) = (-(b * &s), a * &s);
                let x = OneRoot::new(fx.clone(), bx, k.clone());
                let y = OneRoot::new(fy.clone(), by, k.clone());
                x.and_then(|x| Ok(Point::new(x, y?)))
            };
            Meeting {
                points: [along(Q::ONE), along(-Q::ONE)]
                    .into_iter()
                    .flatten()
                    .collect(),
                touching: false,
            }
        }
    }
}

/// Where `p` and `q` meet; two carriers that are one do not meet here.
fn meeting(p: &Carrier, q: &Carrier) -> Meeting {
    match (p, q) {
        (Carrier::Line { a, b, c }, Carrier::Line { a: d, b: e, c: f }) => {
            let det = a * e - d * b;
            if det == Q::ZERO {
                return Meeting::NOWHERE;
            }
            Meeting {
                points: vec![Point::at((c * e - f * b) / &det, (a * f - d * c) / &det)],
                touching: false,
            }
        }
        (Carrier::Line { a, b, c }, Carrier::Circle { x, y, r2 })
        | (Carrier::Circle { x, y, r2 }, Carrier::Line { a, b, c }) => {
            line_circle(a, b, c, x, y, r2)
        }
        (Carrier::Circle { x, y, r2 }, Carrier::Circle { x: u, y: v, r2: s }) => {
            if x == u && y == v {
                return Meeting::NOWHERE;
            }
            // The difference of the two circles' equations is the line through their crossings.
            let two = Q::ONE + Q::ONE;
            let a = &two * (u - x);
            let b = &two * (v - y);
            let c = (u * u + v * v - s) - (x * x + y * y - r2);
            line_circle(&a, &b, &c, x, y, r2)
        }
    }
}

/// An oriented stretch of a line, or an arc monotone in x.
#[derive(Clone, Debug)]
pub struct Piece {
    carrier: Carrier,
    start: Point,
    end: Point,
    ccw: bool,
}

impl Piece {
    /// The segment from `start` to `end`, whose coordinates are rational.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Degenerate`] if they are one point.
    pub fn segment(start: (Q, Q), end: (Q, Q)) -> Result<Piece, ReferenceError> {
        if start == end {
            return Err(ReferenceError::Degenerate);
        }
        let (a, b) = (&start.1 - &end.1, &end.0 - &start.0);
        let c = &a * &start.0 + &b * &start.1;
        Ok(Piece {
            carrier: line(&a, &b, &c),
            start: Point::at(start.0, start.1),
            end: Point::at(end.0, end.1),
            ccw: false,
        })
    }

    /// The arc about `centre` of squared radius `r2` from `start` to `end`, the whole circle if they
    /// are one point, as pieces monotone in x, cut at the circle's left and right extremes.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::OffCircle`] if an end is not on the circle, and
    /// [`ReferenceError::Degenerate`] unless `r2` is positive.
    pub fn arc(
        centre: (Q, Q),
        r2: Q,
        start: &Point,
        end: &Point,
        ccw: bool,
    ) -> Result<Vec<Piece>, ReferenceError> {
        if r2 <= Q::ZERO {
            return Err(ReferenceError::Degenerate);
        }
        let on = |p: &Point| p.power(&centre.0, &centre.1) == OneRoot::rational(r2.clone());
        if !on(start) || !on(end) {
            return Err(ReferenceError::OffCircle);
        }
        let (from, to) = if ccw { (start, end) } else { (end, start) };
        let (cx, cy) = centre;
        let extreme = |s: Q| {
            OneRoot::new(cx.clone(), s, r2.clone())
                .map(|x| Point::new(x, OneRoot::rational(cy.clone())))
        };
        let (right, left) = (extreme(Q::ONE)?, extreme(-Q::ONE)?);
        let carrier = Carrier::Circle {
            x: cx.clone(),
            y: cy.clone(),
            r2,
        };
        let upper = |p: &Point| {
            let high = p.y.plus(&-&cy).sign();
            high == Ordering::Greater
                || (high == Ordering::Equal && p.x.plus(&-&cx).sign() == Ordering::Greater)
        };
        let piece = |a: &Point, b: &Point| Piece {
            carrier: carrier.clone(),
            start: a.clone(),
            end: b.clone(),
            ccw: true,
        };
        // Counter-clockwise, the upper half runs right to left and the lower left to right.
        let mut pieces = Vec::new();
        let mut at = from.clone();
        // A circle has two extremes, so this ends within three steps.
        for _ in 0..3 {
            let high = upper(&at);
            let stop = if high { &left } else { &right };
            let ahead = if high { *to < at } else { *to > at };
            if to == stop || (upper(to) == high && ahead) {
                pieces.push(piece(&at, to));
                break;
            }
            pieces.push(piece(&at, stop));
            at = stop.clone();
        }
        if !ccw {
            pieces.reverse();
            pieces = pieces.iter().map(Piece::reversed).collect();
        }
        Ok(pieces)
    }

    /// Where it starts.
    #[must_use]
    pub const fn start(&self) -> &Point {
        &self.start
    }

    /// Where it ends.
    #[must_use]
    pub const fn end(&self) -> &Point {
        &self.end
    }

    /// What it runs along.
    #[must_use]
    pub const fn carrier(&self) -> &Carrier {
        &self.carrier
    }

    /// The same stretch the other way.
    #[must_use]
    pub fn reversed(&self) -> Piece {
        Piece {
            carrier: self.carrier.clone(),
            start: self.end.clone(),
            end: self.start.clone(),
            ccw: !self.ccw && matches!(self.carrier, Carrier::Circle { .. }),
        }
    }

    /// Its left and right ends.
    fn ends(&self) -> (&Point, &Point) {
        if self.start < self.end {
            (&self.start, &self.end)
        } else {
            (&self.end, &self.start)
        }
    }

    /// Whether an arc is on the circle's upper half: counter-clockwise, those run right to left.
    fn upper(&self) -> bool {
        self.ccw == (self.start > self.end)
    }

    /// Whether `p`, on this carrier, is on this piece.
    fn holds(&self, p: &Point) -> bool {
        let (left, right) = self.ends();
        if p < left || p > right {
            return false;
        }
        match &self.carrier {
            Carrier::Line { .. } => true,
            Carrier::Circle { y, .. } => {
                let high = p.y.plus(&-y).sign();
                if self.upper() {
                    high != Ordering::Less
                } else {
                    high != Ordering::Greater
                }
            }
        }
    }

    /// Whether `other` is this stretch: `Some(true)` the same way, `Some(false)` the other way.
    fn same_as(&self, other: &Piece) -> Option<bool> {
        if self.carrier != other.carrier {
            return None;
        }
        let round = matches!(self.carrier, Carrier::Circle { .. });
        if round && self.upper() != other.upper() {
            return None;
        }
        if self.start == other.start && self.end == other.end {
            Some(true)
        } else if self.start == other.end && self.end == other.start {
            Some(false)
        } else {
            None
        }
    }

    /// The pieces this one makes, cut at each of `cuts` inside it.
    fn cut(&self, cuts: &[Point]) -> Vec<Piece> {
        let mut inner: Vec<&Point> = cuts
            .iter()
            .filter(|p| **p != self.start && **p != self.end && self.holds(p))
            .collect();
        inner.sort();
        inner.dedup();
        if self.start > self.end {
            inner.reverse();
        }
        let mut pieces = Vec::with_capacity(inner.len() + 1);
        let mut at = &self.start;
        for p in inner.into_iter().chain(core::iter::once(&self.end)) {
            pieces.push(Piece {
                carrier: self.carrier.clone(),
                start: at.clone(),
                end: p.clone(),
                ccw: self.ccw,
            });
            at = p;
        }
        pieces
    }

    /// y at the rational `x`, for a piece that is not upright and spans `x`.
    fn y_at(&self, x: &Q) -> Option<OneRoot> {
        match &self.carrier {
            Carrier::Line { a, b, c } => {
                (*b != Q::ZERO).then(|| OneRoot::rational((c - a * x) / b))
            }
            Carrier::Circle { x: cx, y: cy, r2 } => {
                let across = x - cx;
                let up = if self.upper() { Q::ONE } else { -Q::ONE };
                OneRoot::new(cy.clone(), up, r2 - &across * &across).ok()
            }
        }
    }

    /// One point inside the piece, on no vertex's x (or, upright, y), and the rational line along
    /// which its ray runs: every other piece's crossing with that line is one-root.
    fn sample(&self, vertices: &[Point]) -> Option<Sample> {
        let (left, right) = self.ends();
        if left.x == right.x {
            let x = left.x.as_rational()?.clone();
            let y = gap(&left.y, &right.y, vertices.iter().map(Point::y));
            let ray = Carrier::Line {
                a: Q::ZERO,
                b: Q::ONE,
                c: y.clone(),
            };
            return Some(Sample {
                along: OneRoot::rational(x),
                ray,
                coordinate: Point::x,
            });
        }
        let x = gap(&left.x, &right.x, vertices.iter().map(Point::x));
        let along = self.y_at(&x)?;
        let ray = Carrier::Line {
            a: Q::ONE,
            b: Q::ZERO,
            c: x,
        };
        Some(Sample {
            along,
            ray,
            coordinate: Point::y,
        })
    }
}

/// A rational above `lo`, below `hi` and below each of `avoid` above `lo`.
fn gap<'a>(lo: &OneRoot, hi: &OneRoot, avoid: impl Iterator<Item = &'a OneRoot>) -> Q {
    let mut top = hi;
    for value in avoid {
        if value > lo && value < top {
            top = value;
        }
    }
    between(lo, top)
}

/// A point to cast a ray from: `along` is its coordinate on the line `ray`, which `coordinate`
/// reads.
struct Sample {
    along: OneRoot,
    ray: Carrier,
    coordinate: fn(&Point) -> &OneRoot,
}

impl Sample {
    /// Whether the ray crosses `pieces` an odd number of times. A touch does not cross, and the ray
    /// meets no vertex.
    fn inside(&self, pieces: &[Piece]) -> bool {
        let mut crossings = 0_u64;
        for piece in pieces {
            let met = meeting(&self.ray, &piece.carrier);
            if met.touching {
                continue;
            }
            for p in &met.points {
                if piece.holds(p) && *(self.coordinate)(p) > self.along {
                    crossings = crossings.saturating_add(1);
                }
            }
        }
        crossings % 2 == 1
    }
}

/// Where a piece lies against a region's boundary, cut at every crossing with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    In,
    Out,
    /// On the boundary, the same way.
    Along,
    /// On the boundary, the other way.
    Against,
}

fn side(piece: &Piece, pieces: &[Piece], vertices: &[Point]) -> Side {
    if let Some(same) = pieces.iter().find_map(|p| piece.same_as(p)) {
        return if same { Side::Along } else { Side::Against };
    }
    match piece.sample(vertices) {
        Some(sample) if sample.inside(pieces) => Side::In,
        _ => Side::Out,
    }
}

/// Every point `p` and `q` share: their crossings and, along one carrier, their ends.
fn common(p: &Piece, q: &Piece) -> Vec<Point> {
    if p.carrier == q.carrier {
        let on_p = [&q.start, &q.end].into_iter().filter(|e| p.holds(e));
        let on_q = [&p.start, &p.end].into_iter().filter(|e| q.holds(e));
        return on_p.chain(on_q).cloned().collect();
    }
    meeting(&p.carrier, &q.carrier)
        .points
        .into_iter()
        .filter(|e| p.holds(e) && q.holds(e))
        .collect()
}

/// `pieces`, each cut at every crossing with `others`.
fn cut_all(pieces: &[Piece], others: &[Piece]) -> Vec<Piece> {
    pieces
        .iter()
        .flat_map(|p| {
            let cuts: Vec<Point> = others.iter().flat_map(|q| common(p, q)).collect();
            p.cut(&cuts)
        })
        .collect()
}

fn vertices_of(pieces: &[Piece]) -> Vec<Point> {
    let mut points: Vec<Point> = pieces
        .iter()
        .flat_map(|p| [p.start.clone(), p.end.clone()])
        .collect();
    points.sort();
    points.dedup();
    points
}

/// Where a point lies against a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// In its interior.
    In,
    /// On its boundary.
    On,
    /// Outside it.
    Out,
}

/// One stretch of steps of a closed path.
#[derive(Clone, Debug)]
pub enum Step {
    /// A segment to this rational point.
    Line(Q, Q),
    /// An arc to this rational point about this rational centre.
    Arc {
        /// Where it ends.
        to: (Q, Q),
        /// Its centre.
        centre: (Q, Q),
        /// Whether it runs counter-clockwise.
        ccw: bool,
    },
}

/// Closed chains of pieces with the interior on their left.
#[derive(Clone, Debug, Default)]
pub struct Region {
    pieces: Vec<Piece>,
}

impl Region {
    /// The region `pieces` bound.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Open`] unless as many pieces leave every point as reach it.
    pub fn new(pieces: Vec<Piece>) -> Result<Region, ReferenceError> {
        for v in vertices_of(&pieces) {
            let count = |at: fn(&Piece) -> &Point| pieces.iter().filter(|p| *at(p) == v).count();
            if count(Piece::start) != count(Piece::end) {
                return Err(ReferenceError::Open);
            }
        }
        Ok(Region { pieces })
    }

    /// Nothing.
    #[must_use]
    pub const fn empty() -> Region {
        Region { pieces: Vec::new() }
    }

    /// The polygon of `corners`, counter-clockwise.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Degenerate`] for fewer than three corners or a repeated neighbour.
    pub fn polygon(corners: &[(Q, Q)]) -> Result<Region, ReferenceError> {
        let Some((first, rest)) = corners.split_first() else {
            return Err(ReferenceError::Degenerate);
        };
        if rest.len() < 2 {
            return Err(ReferenceError::Degenerate);
        }
        let steps: Vec<Step> = rest
            .iter()
            .chain(core::iter::once(first))
            .map(|(x, y)| Step::Line(x.clone(), y.clone()))
            .collect();
        Region::path(first, &steps)
    }

    /// The disc about `centre`, of squared radius `r2`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Degenerate`] unless `r2` is positive.
    pub fn disc(centre: (Q, Q), r2: Q) -> Result<Region, ReferenceError> {
        if r2 <= Q::ZERO {
            return Err(ReferenceError::Degenerate);
        }
        let right = Point::new(
            OneRoot::new(centre.0.clone(), Q::ONE, r2.clone())?,
            OneRoot::rational(centre.1.clone()),
        );
        Ok(Region {
            pieces: Piece::arc(centre, r2, &right, &right, true)?,
        })
    }

    /// The closed path from `start` through `steps`, which must end back at `start`.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Open`] if it does not close, and an arc's or a segment's errors.
    pub fn path(start: &(Q, Q), steps: &[Step]) -> Result<Region, ReferenceError> {
        let mut pieces = Vec::new();
        let mut at = start.clone();
        for step in steps {
            match step {
                Step::Line(x, y) => {
                    let to = (x.clone(), y.clone());
                    pieces.push(Piece::segment(at, to.clone())?);
                    at = to;
                }
                Step::Arc { to, centre, ccw } => {
                    let (dx, dy) = (&at.0 - &centre.0, &at.1 - &centre.1);
                    let from = Point::at(at.0.clone(), at.1.clone());
                    let end = Point::at(to.0.clone(), to.1.clone());
                    let r2 = &dx * &dx + &dy * &dy;
                    pieces.extend(Piece::arc(centre.clone(), r2, &from, &end, *ccw)?);
                    at = to.clone();
                }
            }
        }
        if at != *start {
            return Err(ReferenceError::Open);
        }
        Region::new(pieces)
    }

    /// Its pieces.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Whether there is nothing to it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// Where the rational point (x, y) lies. Its ray runs along a line through it of a whole-number
    /// slope that meets no vertex, and each vertex rules out at most one slope.
    #[must_use]
    pub fn locate(&self, x: &Q, y: &Q) -> Location {
        let at = Point::at(x.clone(), y.clone());
        for piece in &self.pieces {
            let on_carrier = match &piece.carrier {
                Carrier::Line { a, b, c } => a * x + b * y == *c,
                Carrier::Circle { x: u, y: v, r2 } => {
                    at.power(u, v) == OneRoot::rational(r2.clone())
                }
            };
            if on_carrier && piece.holds(&at) {
                return Location::On;
            }
        }
        let vertices = vertices_of(&self.pieces);
        let mut slope = Q::ZERO;
        let ray = loop {
            let ray = line(&-&slope, &Q::ONE, &(y - &slope * x));
            let clear = !vertices.iter().any(|v| on_line(v, &ray));
            if clear {
                break ray;
            }
            slope += Q::ONE;
        };
        let sample = Sample {
            along: at.x.clone(),
            ray,
            coordinate: Point::x,
        };
        if sample.inside(&self.pieces) {
            Location::In
        } else {
            Location::Out
        }
    }
}

fn on_line(v: &Point, carrier: &Carrier) -> bool {
    let Carrier::Line { a, b, c } = carrier else {
        return false;
    };
    v.x.times(a).sum(&v.y.times(b)).plus(&-c).sign() == Ordering::Equal
}

/// What a boolean does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// A ∪ B.
    Union,
    /// A ∩ B.
    Intersection,
    /// A − B.
    Difference,
}

/// `a` `op` `b`, regularised.
#[must_use]
pub fn boolean(a: &Region, b: &Region, op: Op) -> Region {
    let everything: Vec<Piece> = a.pieces.iter().chain(&b.pieces).cloned().collect();
    let (first, second) = (
        cut_all(&a.pieces, &everything),
        cut_all(&b.pieces, &everything),
    );
    let vertices = vertices_of(&first.iter().chain(&second).cloned().collect::<Vec<Piece>>());
    let mut pieces = Vec::new();
    for piece in &first {
        if matches!(
            (op, side(piece, &second, &vertices)),
            (Op::Union, Side::Out | Side::Along)
                | (Op::Intersection, Side::In | Side::Along)
                | (Op::Difference, Side::Out | Side::Against)
        ) {
            pieces.push(piece.clone());
        }
    }
    for piece in &second {
        match (op, side(piece, &first, &vertices)) {
            (Op::Union, Side::Out) | (Op::Intersection, Side::In) => pieces.push(piece.clone()),
            (Op::Difference, Side::In) => pieces.push(piece.reversed()),
            _ => {}
        }
    }
    Region { pieces }
}

/// The signed sum of `terms`' oriented pieces, each term's count times its regions', every piece cut
/// at every crossing with every other: what is left after opposite pieces cancel.
#[must_use]
pub fn boundary_sum(terms: &[(i64, &Region)]) -> Vec<(Piece, i64)> {
    let everything: Vec<Piece> = terms
        .iter()
        .flat_map(|(_, r)| r.pieces.iter().cloned())
        .collect();
    let mut sum: Vec<(Piece, i64)> = Vec::new();
    for (count, region) in terms {
        for piece in cut_all(&region.pieces, &everything) {
            let found = sum
                .iter_mut()
                .find_map(|(p, c)| piece.same_as(p).map(|same| (c, same)));
            match found {
                Some((held, true)) => *held = held.saturating_add(*count),
                Some((held, false)) => *held = held.saturating_sub(*count),
                None => sum.push((piece, *count)),
            }
        }
    }
    sum.retain(|(_, c)| *c != 0);
    sum
}

/// Chapter 28.3's law: the oriented pieces of `union` and `intersection` sum to those of `a` and `b`.
#[must_use]
pub fn boundary_identity(a: &Region, b: &Region, union: &Region, intersection: &Region) -> bool {
    boundary_sum(&[(1, union), (1, intersection), (-1, a), (-1, b)]).is_empty()
}

/// Whether `a` and `b` have one boundary, however their pieces are cut.
#[must_use]
pub fn same_boundary(a: &Region, b: &Region) -> bool {
    boundary_sum(&[(1, a), (-1, b)]).is_empty()
}

/// The direction a half-edge leaves `at` in. A line's is rational; an arc's is at right angles to
/// its radius, so it is on `at`'s root.
fn leaving(piece: &Piece, at: &Point) -> (OneRoot, OneRoot) {
    match &piece.carrier {
        Carrier::Line { a, b, .. } => {
            let forward = piece.start < piece.end;
            let rightward = *b > Q::ZERO || (*b == Q::ZERO && *a < Q::ZERO);
            let s = if forward == rightward {
                Q::ONE
            } else {
                -Q::ONE
            };
            (OneRoot::rational(b * &s), OneRoot::rational(-(a * &s)))
        }
        Carrier::Circle { x, y, .. } => {
            let (rx, ry) = (at.x.plus(&-x), at.y.plus(&-y));
            let s = if piece.ccw { Q::ONE } else { -Q::ONE };
            (ry.times(&-&s), rx.times(&s))
        }
    }
}

/// 0 for directions in the half-turn from +x, 1 for the rest.
fn half(d: &(OneRoot, OneRoot)) -> u8 {
    let up = d.1.sign();
    u8::from(
        !(up == Ordering::Greater || (up == Ordering::Equal && d.0.sign() == Ordering::Greater)),
    )
}

/// Curvature's order: a line's is 0, and an arc's is 1/r, turning left positive.
fn bend(piece: &Piece, other: &Piece) -> Ordering {
    let of = |p: &Piece| match &p.carrier {
        Carrier::Line { .. } => (0_i8, Q::ZERO),
        Carrier::Circle { r2, .. } => (if p.ccw { 1 } else { -1 }, r2.clone()),
    };
    let ((s, r), (t, u)) = (of(piece), of(other));
    // Among left turns the smaller circle bends more; among right turns, less.
    s.cmp(&t).then(if s > 0 { u.cmp(&r) } else { r.cmp(&u) })
}

/// The counter-clockwise order of two half-edges leaving one point: by direction, then, when they
/// leave together, the one bending further left comes later.
fn around(p: &Piece, q: &Piece, at: &Point) -> Ordering {
    let (d, e) = (leaving(p, at), leaving(q, at));
    half(&d)
        .cmp(&half(&e))
        .then_with(|| {
            let negative = e.1.product(&d.0).sum(&e.0.product(&d.1).times(&-Q::ONE));
            Ordering::Equal.cmp(&negative.sign())
        })
        .then_with(|| bend(p, q))
}

/// The arrangement of a set of pieces: each cut at every crossing, repeats dropped.
#[derive(Clone, Debug)]
pub struct Arrangement {
    vertices: Vec<Point>,
    edges: Vec<Piece>,
}

impl Arrangement {
    /// The arrangement of `pieces`.
    #[must_use]
    pub fn of(pieces: &[Piece]) -> Arrangement {
        let mut edges: Vec<Piece> = Vec::new();
        for piece in cut_all(pieces, pieces) {
            if !edges.iter().any(|e| e.same_as(&piece).is_some()) {
                edges.push(piece);
            }
        }
        Arrangement {
            vertices: vertices_of(&edges),
            edges,
        }
    }

    /// How many vertices.
    #[must_use]
    pub fn vertices(&self) -> usize {
        self.vertices.len()
    }

    /// How many edges.
    #[must_use]
    pub fn edges(&self) -> usize {
        self.edges.len()
    }

    /// How many connected components.
    #[must_use]
    pub fn components(&self) -> usize {
        let mut seen: Vec<&Point> = Vec::new();
        let mut count = 0;
        for start in &self.vertices {
            if seen.contains(&start) {
                continue;
            }
            count += 1;
            let mut todo = vec![start];
            while let Some(v) = todo.pop() {
                if seen.contains(&v) {
                    continue;
                }
                seen.push(v);
                for e in &self.edges {
                    if e.start == *v {
                        todo.push(&e.end);
                    } else if e.end == *v {
                        todo.push(&e.start);
                    }
                }
            }
        }
        count
    }

    /// How many faces, the outer one included, by Euler's formula.
    #[must_use]
    pub fn faces(&self) -> usize {
        (self.edges() + 1 + self.components()).saturating_sub(self.vertices())
    }

    /// Every boundary cycle: from each half-edge, the next is the first one clockwise from its twin
    /// at its end, which keeps one face on the left all the way round. A face with holes has one
    /// cycle for each, so there are E − V + 2C of them.
    #[must_use]
    pub fn cycles(&self) -> Vec<Vec<Piece>> {
        let halves: Vec<Piece> = self
            .edges
            .iter()
            .flat_map(|e| [e.clone(), e.reversed()])
            .collect();
        let twin = |h: usize| h ^ 1;
        let rotate = |h: usize| -> Option<usize> {
            // The half-edges leaving this one's end, and the one just clockwise of its twin.
            let edge = halves.get(h)?;
            let mut out: Vec<usize> = (0..halves.len())
                .filter(|i| halves.get(*i).is_some_and(|p| p.start == edge.end))
                .collect();
            out.sort_by(|i, j| match (halves.get(*i), halves.get(*j)) {
                (Some(p), Some(q)) => around(p, q, &edge.end),
                _ => Ordering::Equal,
            });
            let place = out.iter().position(|i| *i == twin(h))?;
            let before = place.checked_sub(1).unwrap_or(out.len().saturating_sub(1));
            out.get(before).copied()
        };
        let mut done = vec![false; halves.len()];
        let mut cycles = Vec::new();
        for first in 0..halves.len() {
            let mut cycle = Vec::new();
            let mut at = first;
            while let Some(flag) = done.get_mut(at) {
                if *flag {
                    break;
                }
                *flag = true;
                cycle.extend(halves.get(at).cloned());
                match rotate(at) {
                    Some(next) => at = next,
                    None => break,
                }
            }
            if !cycle.is_empty() {
                cycles.push(cycle);
            }
        }
        cycles
    }
}

/// The unit vector along (x, y), if it is rational.
fn unit(x: &Q, y: &Q) -> Option<(Q, Q)> {
    let size = root(&(x * x + y * y))?;
    (size != Q::ZERO).then(|| (x / &size, y / &size))
}

/// Everything within `d` of a segment: a rectangle and, by [`offset`], its end discs.
fn segment_stroke(s: (&Q, &Q), e: (&Q, &Q), d: &Q) -> Result<Region, ReferenceError> {
    let (ux, uy) = unit(&(e.0 - s.0), &(e.1 - s.1)).ok_or(ReferenceError::NotRational)?;
    let (nx, ny) = (-(&uy * d), &ux * d);
    Region::polygon(&[
        (s.0 - &nx, s.1 - &ny),
        (e.0 - &nx, e.1 - &ny),
        (e.0 + &nx, e.1 + &ny),
        (s.0 + &nx, s.1 + &ny),
    ])
}

/// Everything within `d` of a counter-clockwise arc, but for its end discs: an annular sector, or a
/// sector when `d` reaches the centre.
fn arc_stroke(
    c: (&Q, &Q),
    r: &Q,
    s: (&Q, &Q),
    e: (&Q, &Q),
    d: &Q,
) -> Result<Region, ReferenceError> {
    let at = |p: (&Q, &Q), radius: &Q| {
        let t = radius / r;
        (c.0 + (p.0 - c.0) * &t, c.1 + (p.1 - c.1) * &t)
    };
    let (outer, inner) = (r + d, r - d);
    let centre = (c.0.clone(), c.1.clone());
    let (a, b) = (at(s, &outer), at(e, &outer));
    let mut steps = vec![Step::Arc {
        to: b,
        centre: centre.clone(),
        ccw: true,
    }];
    if inner > Q::ZERO {
        let (p, q) = (at(e, &inner), at(s, &inner));
        steps.push(Step::Line(p.0, p.1));
        steps.push(Step::Arc {
            to: q,
            centre,
            ccw: false,
        });
    } else {
        steps.push(Step::Line(c.0.clone(), c.1.clone()));
    }
    steps.push(Step::Line(a.0.clone(), a.1.clone()));
    Region::path(&a, &steps)
}

/// Everything within `d` of `piece`, which must have rational ends and, as a line, a rational unit
/// normal or, as an arc, a rational radius.
fn stroke(piece: &Piece, d: &Q) -> Result<Region, ReferenceError> {
    let (s, e) = (
        piece.start.rational().ok_or(ReferenceError::NotRational)?,
        piece.end.rational().ok_or(ReferenceError::NotRational)?,
    );
    let body = match &piece.carrier {
        Carrier::Line { .. } => segment_stroke(s, e, d)?,
        Carrier::Circle { x, y, r2 } => {
            let r = root(r2).ok_or(ReferenceError::NotRational)?;
            let (s, e) = if piece.ccw { (s, e) } else { (e, s) };
            arc_stroke((x, y), &r, s, e, d)?
        }
    };
    let cap = |p: (&Q, &Q)| Region::disc((p.0.clone(), p.1.clone()), d * d);
    let capped = boolean(&body, &cap(s)?, Op::Union);
    Ok(boolean(&capped, &cap(e)?, Op::Union))
}

/// `region` grown by `d`, or shrunk if `d` is negative: its union with, or difference from, the
/// stroke of radius |d| of every piece.
///
/// # Errors
///
/// [`ReferenceError::NotRational`] for a piece whose stroke these numbers cannot hold.
pub fn offset(region: &Region, d: &Q) -> Result<Region, ReferenceError> {
    let width = abs(d);
    if width == Q::ZERO {
        return Ok(region.clone());
    }
    let op = if *d > Q::ZERO {
        Op::Union
    } else {
        Op::Difference
    };
    region
        .pieces
        .iter()
        .try_fold(region.clone(), |grown, piece| {
            Ok(boolean(&grown, &stroke(piece, &width)?, op))
        })
}
