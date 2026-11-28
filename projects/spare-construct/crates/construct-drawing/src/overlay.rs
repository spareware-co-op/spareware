//! The exact 2D work a drawing needs that the kernel does not answer yet: the union of closed
//! straight-edged loops keeping each edge's label, line work drawn once, and the pieces of a
//! straight line inside an area.
//!
//! Interim. The kernel's own 2D arrangement and booleans (`geom-2d`, chapter 14.4 and 14.5 of the
//! geometry architecture: pieces G196, G199, G200 and G202) and its hatches (14.6: G210) replace
//! this module through engine-geometry once they are built; today `geom-2d` is a scaffold and the
//! kernel's interface has no 2D operation, and no other engine answers one through it. Everything
//! here sits behind [`Overlay`], crate-private, so replacing it changes this one file; the tests
//! (`tests/merge.rs`, `tests/hatch.rs`) are written against the crate's public types and judge the
//! replacement unchanged. No library is used for the method: `i_overlay` rounds the points where
//! edges cross and works in 32-bit integers, so it cannot decide stored regions (chapter 14.5 and
//! 29 keep it for screen fills only), and exactness is the point here.
//!
//! What it promises: every answer is exact. Points are fractions of any size (num-rational's
//! `BigRational`); a point is handed back as a [`ViewPoint`] in lowest terms, and a value past 128
//! bits is [`DrawingError::TooBig`], never rounded. Same input, same output, in one order.
//!
//! The method, all of it decided by exact signs:
//!
//! - **Split.** Every edge is cut at every point where another edge crosses it, touches it, or
//!   starts or ends on it, so two pieces either are the same segment or meet only at ends.
//! - **Winding.** A loop's material lies on its left (outlines counter-clockwise, holes
//!   clockwise). The winding number on each side of a piece is counted with a ray from the piece's
//!   midpoint moved an infinitely small step off the piece (a symbolic perturbation: each sign
//!   that would be zero is decided by the step's direction instead), so no point is ever on an
//!   edge it is tested against. For a union, a side is inside when its winding number is above
//!   zero; a piece with inside on one side only is on the result's boundary, kept with inside on
//!   its left and the least label of the pieces running that way.
//! - **Loops.** From the end of each kept piece, the next is the first one turning clockwise from
//!   the way back: the tightest left turn. So areas that touch only at a point stay apart, and a
//!   hole touching its outline at a point stays a hole. Neighbouring pieces in one line with one
//!   label are joined back into one.
//! - **Areas.** A loop turning counter-clockwise is an outline; each clockwise loop is a hole of
//!   the smallest outline holding it.
//! - **Window.** A crop window (`Window`, for `crate::crop`) clips a segment to a simple closed outline: the
//!   segment is cut at every crossing and each piece is kept when the winding number just to either side
//!   of its middle is above zero (a piece along the outline stays); an arc is kept or dropped whole when
//!   its circle is clear of the outline, by exact distances squared, and not answered otherwise.
//! - **Clip.** A straight line's pieces inside an area are where the winding number is above zero
//!   just to both sides of the line, so a line along an edge, or touching a corner, gives nothing.
//! - **Cover.** What hides a section's line work (`Cover`, for `crate::section`): a segment is cut
//!   at every crossing with the covering areas' edges and each piece is kept when the winding
//!   number of all the areas together is zero just to both sides of its middle, so a piece inside
//!   an area or along its outline goes.
//!
//! Cost: the split and the winding counts compare every edge with every other, so time grows with
//! the square of the edges of one group; enough for a plan's walls of one material, and the
//! kernel's sweep (14.4) replaces it.

#![allow(
    clippy::many_single_char_names,
    reason = "geometry's own names: points a, b, m, directions d, n, r, v, parameters s, t"
)]

use std::cmp::Ordering;
use std::collections::BTreeMap;

use num_bigint::BigInt;
use num_integer::Integer as _;
use num_rational::BigRational;
use num_traits::{Signed as _, Zero as _};

use crate::DrawingError;
use crate::drawing::ViewPoint;

/// An exact fraction of any size. Its arithmetic cannot overflow; division checks its divisor.
/// Crate-visible so that the section's sweep (`crate::sweep`) works in the same exact numbers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Q(BigRational);

#[allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision fractions cannot overflow; the one division checks its divisor \
              beside it"
)]
impl Q {
    pub(crate) fn whole(v: i128) -> Self {
        Self(BigRational::from_integer(BigInt::from(v)))
    }

    pub(crate) fn fraction(num: i128, den: i128) -> Result<Self, DrawingError> {
        if den == 0 {
            return Err(DrawingError::BadPoint);
        }
        Ok(Self(BigRational::new(BigInt::from(num), BigInt::from(den))))
    }

    pub(crate) fn plus(&self, other: &Self) -> Self {
        Self(&self.0 + &other.0)
    }

    pub(crate) fn minus(&self, other: &Self) -> Self {
        Self(&self.0 - &other.0)
    }

    pub(crate) fn times(&self, other: &Self) -> Self {
        Self(&self.0 * &other.0)
    }

    pub(crate) fn over(&self, other: &Self) -> Result<Self, DrawingError> {
        if other.0.is_zero() {
            return Err(DrawingError::Overlay("a division by zero".to_owned()));
        }
        Ok(Self(&self.0 / &other.0))
    }

    pub(crate) fn sign(&self) -> Ordering {
        if self.0.is_positive() {
            Ordering::Greater
        } else if self.0.is_negative() {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// The least whole number at or above it, and the greatest at or below it.
    fn ceil(&self) -> Result<i128, DrawingError> {
        i128::try_from(self.0.ceil().to_integer()).map_err(|_| DrawingError::TooBig)
    }

    pub(crate) fn floor(&self) -> Result<i128, DrawingError> {
        i128::try_from(self.0.floor().to_integer()).map_err(|_| DrawingError::TooBig)
    }

    /// Its numerator and its denominator, in lowest terms, the denominator positive.
    pub(crate) fn parts(&self) -> Result<(i128, i128), DrawingError> {
        let whole = |v: &BigInt| i128::try_from(v.clone()).map_err(|_| DrawingError::TooBig);
        Ok((whole(self.0.numer())?, whole(self.0.denom())?))
    }

    /// `x` and `y` over their least common denominator, as a view point.
    pub(crate) fn point(x: &Self, y: &Self) -> Result<ViewPoint, DrawingError> {
        let den = x.0.denom().lcm(y.0.denom());
        let num = |v: &Self| v.0.numer() * (&den / v.0.denom());
        let whole = |v: BigInt| i128::try_from(v).map_err(|_| DrawingError::TooBig);
        ViewPoint::new(whole(num(x))?, whole(num(y))?, whole(den)?)
    }
}

/// An exact point.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Pt {
    x: Q,
    y: Q,
}

impl Pt {
    fn of(p: ViewPoint) -> Result<Self, DrawingError> {
        Ok(Self {
            x: Q::fraction(p.x, p.den)?,
            y: Q::fraction(p.y, p.den)?,
        })
    }

    fn view(&self) -> Result<ViewPoint, DrawingError> {
        Q::point(&self.x, &self.y)
    }

    fn minus(&self, other: &Self) -> Self {
        Self {
            x: self.x.minus(&other.x),
            y: self.y.minus(&other.y),
        }
    }

    fn plus(&self, other: &Self) -> Self {
        Self {
            x: self.x.plus(&other.x),
            y: self.y.plus(&other.y),
        }
    }

    fn scaled(&self, by: &Q) -> Self {
        Self {
            x: self.x.times(by),
            y: self.y.times(by),
        }
    }

    /// Turned a quarter counter-clockwise: the left normal of a direction.
    fn left(&self) -> Self {
        Self {
            x: self.y.times(&Q::whole(-1)),
            y: self.x.clone(),
        }
    }

    fn negated(&self) -> Self {
        self.scaled(&Q::whole(-1))
    }

    fn cross(&self, other: &Self) -> Q {
        self.x.times(&other.y).minus(&self.y.times(&other.x))
    }

    fn dot(&self, other: &Self) -> Q {
        self.x.times(&other.x).plus(&self.y.times(&other.y))
    }
}

/// A directed edge with its label.
#[derive(Clone, Debug)]
struct Edge {
    a: Pt,
    b: Pt,
    label: usize,
}

fn zero_division() -> DrawingError {
    DrawingError::Overlay("a piece of no length".to_owned())
}

/// The index after `i` round a loop of `n`.
fn after(i: usize, n: usize) -> usize {
    i.checked_add(1).and_then(|j| j.checked_rem(n)).unwrap_or(0)
}

/// The index before `i` round a loop of `n`.
fn before(i: usize, n: usize) -> usize {
    i.checked_sub(1).unwrap_or_else(|| n.saturating_sub(1))
}

/// The parameters `t` (strictly between 0 and 1) along `e`, whose direction is `r` and squared
/// length `rr`, where one of `others` crosses it, touches it, or starts or ends on it.
fn cuts_of<'a>(
    e: &Edge,
    r: &Pt,
    rr: &Q,
    others: impl Iterator<Item = &'a Edge>,
) -> Result<Vec<Q>, DrawingError> {
    let (zero, one) = (Q::whole(0), Q::whole(1));
    let mut cuts: Vec<Q> = Vec::new();
    for f in others {
        let s = f.b.minus(&f.a);
        let ca = f.a.minus(&e.a);
        let den = r.cross(&s);
        if den.is_zero() {
            if ca.cross(r).is_zero() {
                // In one line: where the other's ends fall on this one.
                for q in [&f.a, &f.b] {
                    let t = q.minus(&e.a).dot(r).over(rr)?;
                    if t > zero && t < one {
                        cuts.push(t);
                    }
                }
            }
        } else {
            let t = ca.cross(&s).over(&den)?;
            let u = ca.cross(r).over(&den)?;
            if t > zero && t < one && u >= zero && u <= one {
                cuts.push(t);
            }
        }
    }
    Ok(cuts)
}

/// Every edge cut at every point another edge crosses or touches it, or starts or ends on it.
fn split(edges: &[Edge]) -> Result<Vec<Edge>, DrawingError> {
    let mut out = Vec::new();
    for (i, e) in edges.iter().enumerate() {
        let r = e.b.minus(&e.a);
        let rr = r.dot(&r);
        if rr.is_zero() {
            return Err(zero_division());
        }
        let others = edges
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| f);
        let mut cuts = cuts_of(e, &r, &rr, others)?;
        cuts.sort();
        cuts.dedup();
        let mut from = e.a.clone();
        for t in &cuts {
            let to = e.a.plus(&r.scaled(t));
            out.push(Edge {
                a: from,
                b: to.clone(),
                label: e.label,
            });
            from = to;
        }
        out.push(Edge {
            a: from,
            b: e.b.clone(),
            label: e.label,
        });
    }
    Ok(out)
}

/// The winding number of `edges` about `m` moved an infinitely small step along `v`; `m` lies on
/// no edge but those in one line with `v`'s normal, which the step moves it off.
fn winding(edges: &[Edge], m: &Pt, v: &Pt) -> Result<i64, DrawingError> {
    // Whether `q` is above the moved point: q.y > m.y + δ v.y.
    let above = |q: &Pt| match q.y.cmp(&m.y) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => v.y.sign() == Ordering::Less,
    };
    let mut w: i64 = 0;
    for e in edges {
        let (up_a, up_b) = (above(&e.a), above(&e.b));
        if up_a == up_b {
            continue;
        }
        let r = e.b.minus(&e.a);
        // Which side of the edge the moved point is: its own sign, or the step's where that is 0.
        let side = match r.cross(&m.minus(&e.a)).sign() {
            Ordering::Equal => r.cross(v).sign(),
            s => s,
        };
        let step = match (up_a, side) {
            (false, Ordering::Greater) => 1,
            (true, Ordering::Less) => -1,
            _ => 0,
        };
        w = w
            .checked_add(step)
            .ok_or_else(|| DrawingError::Overlay("a winding number past 64 bits".to_owned()))?;
    }
    Ok(w)
}

/// Where `v` falls turning clockwise from `from`: a class (before a half turn, a half turn, past
/// it, a whole turn), compared within a class by the turn between them: `a` comes first when `b`
/// is clockwise of it, where `a × b` is negative.
fn clockwise(from: &Pt, a: &Pt, b: &Pt) -> Ordering {
    let class = |v: &Pt| match (from.cross(v).sign(), from.dot(v).sign()) {
        (Ordering::Less, _) => 0,
        (Ordering::Equal, Ordering::Less) => 1,
        (Ordering::Greater, _) => 2,
        _ => 3,
    };
    class(a).cmp(&class(b)).then_with(|| a.cross(b).sign())
}

/// A loop: each vertex with the label of the edge that leaves it.
type Ring = Vec<(Pt, usize)>;

/// The vertices of `ring` that are not between two pieces of one line with one label.
fn joined(ring: Ring) -> Ring {
    let n = ring.len();
    let keep: Vec<bool> = (0..n)
        .map(|i| {
            let prev = ring.get(before(i, n));
            let here = ring.get(i);
            let next = ring.get(after(i, n));
            match (prev, here, next) {
                (Some((p, lp)), Some((h, lh)), Some((x, _))) => {
                    !(lp == lh && h.minus(p).cross(&x.minus(h)).is_zero())
                }
                _ => true,
            }
        })
        .collect();
    ring.into_iter()
        .zip(keep)
        .filter_map(|(v, k)| k.then_some(v))
        .collect()
}

/// Twice the signed area of `ring`: positive when it turns counter-clockwise.
fn twice_area(ring: &Ring) -> Q {
    let mut sum = Q::whole(0);
    for (i, (p, _)) in ring.iter().enumerate() {
        if let Some((q, _)) = ring.get(after(i, ring.len())) {
            sum = sum.plus(&p.cross(q));
        }
    }
    sum
}

fn edges_of(ring: &Ring) -> Vec<Edge> {
    (0..ring.len())
        .filter_map(|i| {
            let (a, label) = ring.get(i)?;
            let (b, _) = ring.get(after(i, ring.len()))?;
            Some(Edge {
                a: a.clone(),
                b: b.clone(),
                label: *label,
            })
        })
        .collect()
}

/// `ring` as view points from its least point.
fn finished(ring: &Ring) -> Result<Vec<(ViewPoint, usize)>, DrawingError> {
    let mut out = Vec::with_capacity(ring.len());
    for (p, label) in ring {
        out.push((p.view()?, *label));
    }
    let mut least = 0;
    for (i, (p, _)) in ring.iter().enumerate() {
        if ring.get(least).is_some_and(|(l, _)| p < l) {
            least = i;
        }
    }
    out.rotate_left(least);
    Ok(out)
}

/// One area of a union: its outline first (counter-clockwise), then its holes (clockwise), each
/// from its least point, every vertex with the label of the edge leaving it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Area {
    /// The loops: the outline, then the holes in order.
    pub(crate) loops: Vec<Vec<(ViewPoint, usize)>>,
}

/// The interim 2D overlay. See the module's documentation.
pub(crate) struct Overlay;

impl Overlay {
    /// The union of `loops` (each vertex with the label of the edge leaving it; outlines turning
    /// counter-clockwise, holes clockwise) as separate areas in order, each boundary edge keeping
    /// the least label of the edges it came from.
    pub(crate) fn union(loops: &[Vec<(ViewPoint, usize)>]) -> Result<Vec<Area>, DrawingError> {
        let mut edges = Vec::new();
        for ring in loops {
            let n = ring.len();
            for i in 0..n {
                let (Some((a, label)), Some((b, _))) = (ring.get(i), ring.get(after(i, n))) else {
                    continue;
                };
                if a != b {
                    edges.push(Edge {
                        a: Pt::of(*a)?,
                        b: Pt::of(*b)?,
                        label: *label,
                    });
                }
            }
        }
        let pieces = split(&edges)?;
        // Each segment once, from its lesser end: the labels running each way.
        let mut segments: BTreeMap<(Pt, Pt), (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        for p in pieces {
            if p.a < p.b {
                segments.entry((p.a, p.b)).or_default().0.push(p.label);
            } else {
                segments.entry((p.b, p.a)).or_default().1.push(p.label);
            }
        }
        let half = Q::fraction(1, 2)?;
        let mut kept: Vec<Edge> = Vec::new();
        for ((lo, hi), (forward, backward)) in segments {
            let v = hi.minus(&lo).left();
            let m = lo.plus(&hi).scaled(&half);
            let inside_left = winding(&edges, &m, &v)? > 0;
            let inside_right = winding(&edges, &m, &v.negated())? > 0;
            let least = |own: &[usize], other: &[usize]| {
                own.iter()
                    .min()
                    .or_else(|| other.iter().min())
                    .copied()
                    .ok_or_else(|| DrawingError::Overlay("a segment with no edge".to_owned()))
            };
            match (inside_left, inside_right) {
                (true, false) => kept.push(Edge {
                    label: least(&forward, &backward)?,
                    a: lo,
                    b: hi,
                }),
                (false, true) => kept.push(Edge {
                    label: least(&backward, &forward)?,
                    a: hi,
                    b: lo,
                }),
                _ => {}
            }
        }
        let rings = trace(&kept)?;
        areas(rings)
    }

    /// Whether `area` holds the material just to the left of the middle of the edge from `a` to
    /// `b` (an edge of a region inside the union `area` came from, so the point moved there lies on
    /// none of `area`'s edges).
    pub(crate) fn holds(area: &Area, a: ViewPoint, b: ViewPoint) -> Result<bool, DrawingError> {
        let (a, b) = (Pt::of(a)?, Pt::of(b)?);
        let mut edges = Vec::new();
        for ring in &area.loops {
            let ring: Ring = ring
                .iter()
                .map(|&(p, label)| Pt::of(p).map(|p| (p, label)))
                .collect::<Result<_, _>>()?;
            edges.extend(edges_of(&ring));
        }
        let m = a.plus(&b).scaled(&Q::fraction(1, 2)?);
        Ok(winding(&edges, &m, &b.minus(&a).left())? > 0)
    }

    /// `pieces` (each a segment and its label) drawn once: where pieces lie on one another, the
    /// one with the least label is kept there; neighbouring pieces of one label are joined. In
    /// order of label, then along the piece.
    pub(crate) fn once(
        pieces: &[(ViewPoint, ViewPoint, usize)],
    ) -> Result<Vec<(ViewPoint, ViewPoint, usize)>, DrawingError> {
        let mut edges = Vec::new();
        for (a, b, label) in pieces {
            if a != b {
                edges.push(Edge {
                    a: Pt::of(*a)?,
                    b: Pt::of(*b)?,
                    label: *label,
                });
            }
        }
        let mut best: BTreeMap<(Pt, Pt), Edge> = BTreeMap::new();
        for p in split(&edges)? {
            let key = if p.a < p.b {
                (p.a.clone(), p.b.clone())
            } else {
                (p.b.clone(), p.a.clone())
            };
            match best.get(&key) {
                Some(have) if have.label <= p.label => {}
                _ => {
                    best.insert(key, p);
                }
            }
        }
        // By label, then along the label's own direction (its pieces all run one way).
        let mut by_label: BTreeMap<usize, Vec<Edge>> = BTreeMap::new();
        for e in best.into_values() {
            by_label.entry(e.label).or_default().push(e);
        }
        let mut out = Vec::new();
        for (label, mut list) in by_label {
            let Some(first) = list.first() else {
                continue;
            };
            let r = first.b.minus(&first.a);
            list.sort_by_key(|x| r.dot(&x.a));
            let mut run: Option<(Pt, Pt)> = None;
            for e in list {
                run = match run {
                    Some((a, b)) if b == e.a => Some((a, e.b)),
                    Some((a, b)) => {
                        out.push((a.view()?, b.view()?, label));
                        Some((e.a, e.b))
                    }
                    None => Some((e.a, e.b)),
                };
            }
            if let Some((a, b)) = run {
                out.push((a.view()?, b.view()?, label));
            }
        }
        Ok(out)
    }

    /// The pieces inside the area bounded by `loops` (outlines counter-clockwise, holes
    /// clockwise) of each line `n · p = k · step` for whole `k`, where `n` is `direction` turned a
    /// quarter counter-clockwise: in order of `k`, then along `direction`. The lines are counted
    /// first, from the area's extent across them, and more than `most` is refused.
    pub(crate) fn family(
        loops: &[Vec<ViewPoint>],
        direction: [i128; 2],
        step: (i128, i128),
        most: u64,
    ) -> Result<Vec<[ViewPoint; 2]>, DrawingError> {
        let d = Pt {
            x: Q::whole(direction[0]),
            y: Q::whole(direction[1]),
        };
        let n = d.left();
        let dd = d.dot(&d);
        let step = Q::fraction(step.0, step.1)?;
        if dd.is_zero() || step.is_zero() {
            return Err(DrawingError::BadShape(
                "a hatch family with no direction or no spacing".to_owned(),
            ));
        }
        let mut edges = Vec::new();
        let mut range: Option<(Q, Q)> = None;
        for ring in loops {
            let pts: Vec<Pt> = ring.iter().map(|p| Pt::of(*p)).collect::<Result<_, _>>()?;
            for (i, a) in pts.iter().enumerate() {
                let c = n.dot(a);
                range = Some(match range {
                    None => (c.clone(), c),
                    Some((lo, hi)) => (lo.min(c.clone()), hi.max(c)),
                });
                if let Some(b) = pts.get(after(i, pts.len())) {
                    edges.push(Edge {
                        a: a.clone(),
                        b: b.clone(),
                        label: 0,
                    });
                }
            }
        }
        let Some((lo, hi)) = range else {
            return Ok(Vec::new());
        };
        let (lo, hi) = (lo.over(&step)?, hi.over(&step)?);
        let (first, last) = (lo.clone().min(hi.clone()).ceil()?, lo.max(hi).floor()?);
        let count = if last < first {
            0
        } else {
            last.checked_sub(first)
                .and_then(|c| c.checked_add(1))
                .and_then(|c| u64::try_from(c).ok())
                .ok_or(DrawingError::TooBig)?
        };
        if count > most {
            return Err(DrawingError::Budget { lines: count, most });
        }
        let per = Q::whole(1).over(&dd)?;
        let mut out = Vec::new();
        let mut k = first;
        while k <= last {
            let c = Q::whole(k).times(&step);
            for [s0, s1] in inside(&edges, &n, &d, &c)? {
                // The point of the line at s along it: (c n + s d) / |d|².
                let at = |s: &Q| n.scaled(&c).plus(&d.scaled(s)).scaled(&per).view();
                out.push([at(&s0)?, at(&s1)?]);
            }
            k = k.checked_add(1).ok_or(DrawingError::TooBig)?;
        }
        Ok(out)
    }
}

/// A crop window: a simple closed outline a picture is clipped to, exactly. The closed region
/// counts: a line along the outline stays.
pub(crate) struct Window {
    edges: Vec<Edge>,
}

impl Window {
    /// The window bounded by `outline` (its corners in order, either way round).
    ///
    /// # Errors
    ///
    /// [`DrawingError::BadShape`] for fewer than three corners, a corner twice in a row or an
    /// outline of no area.
    pub(crate) fn new(outline: &[ViewPoint]) -> Result<Self, DrawingError> {
        let mut ring: Ring = outline
            .iter()
            .map(|&p| Pt::of(p).map(|p| (p, 0)))
            .collect::<Result<_, _>>()?;
        if ring.len() < 3 || ring.windows(2).any(|w| w.first() == w.get(1)) {
            return Err(DrawingError::BadShape(
                "a crop of fewer than three corners, or a corner twice in a row".to_owned(),
            ));
        }
        match twice_area(&ring).sign() {
            Ordering::Equal => {
                return Err(DrawingError::BadShape("a crop of no area".to_owned()));
            }
            Ordering::Less => ring.reverse(),
            Ordering::Greater => {}
        }
        Ok(Self {
            edges: edges_of(&ring),
        })
    }

    /// The pieces of the segment from `a` to `b` inside the window or along its outline, in order
    /// along the segment, neighbouring pieces joined.
    pub(crate) fn pieces(
        &self,
        a: ViewPoint,
        b: ViewPoint,
    ) -> Result<Vec<[ViewPoint; 2]>, DrawingError> {
        let e = Edge {
            a: Pt::of(a)?,
            b: Pt::of(b)?,
            label: 0,
        };
        let r = e.b.minus(&e.a);
        let rr = r.dot(&r);
        if rr.is_zero() {
            return Err(zero_division());
        }
        let mut cuts = cuts_of(&e, &r, &rr, self.edges.iter())?;
        cuts.sort();
        cuts.dedup();
        let mut stops = vec![Q::whole(0)];
        stops.extend(cuts);
        stops.push(Q::whole(1));
        let half = Q::fraction(1, 2)?;
        let v = r.left();
        let mut out: Vec<[Pt; 2]> = Vec::new();
        for pair in stops.windows(2) {
            let (Some(t0), Some(t1)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let p0 = e.a.plus(&r.scaled(t0));
            let p1 = e.a.plus(&r.scaled(t1));
            let m = p0.plus(&p1).scaled(&half);
            // Inside to either side: a piece along the outline has the inside on one side only.
            if winding(&self.edges, &m, &v)? > 0 || winding(&self.edges, &m, &v.negated())? > 0 {
                match out.last_mut() {
                    Some(last) if last[1] == p0 => last[1] = p1,
                    _ => out.push([p0, p1]),
                }
            }
        }
        out.into_iter()
            .map(|[p0, p1]| Ok([p0.view()?, p1.view()?]))
            .collect()
    }

    /// Whether the arc about `centre` starting at `start` lies inside the window: `Some(true)`
    /// wholly inside, `Some(false)` wholly outside, `None` when its circle meets the outline, where
    /// an exact clip of the arc would be a point that is not a fraction of the grid.
    pub(crate) fn arc(
        &self,
        centre: ViewPoint,
        start: ViewPoint,
    ) -> Result<Option<bool>, DrawingError> {
        let (c, s) = (Pt::of(centre)?, Pt::of(start)?);
        let squared = |p: &Pt| {
            let d = p.minus(&c);
            d.dot(&d)
        };
        let radius = squared(&s);
        let (zero, one) = (Q::whole(0), Q::whole(1));
        for e in &self.edges {
            let r = e.b.minus(&e.a);
            let rr = r.dot(&r);
            let t = c.minus(&e.a).dot(&r).over(&rr)?;
            let t = t.max(zero.clone()).min(one.clone());
            let near = squared(&e.a.plus(&r.scaled(&t)));
            let far = squared(&e.a).max(squared(&e.b));
            if near <= radius && radius <= far {
                return Ok(None);
            }
        }
        let along_x = Pt { x: one, y: zero };
        Ok(Some(winding(&self.edges, &s, &along_x)? > 0))
    }
}

/// What hides line work: the areas bounded by loops (outlines counter-clockwise, holes clockwise),
/// any number of them, overlapping or not. A section's cut regions, and the outlines of what lies
/// wholly nearer the viewer, hide what lies beyond them (`crate::section`). Each area's winding
/// number is one inside it and none outside, so the sum of them all is above zero exactly where
/// some area is.
pub(crate) struct Cover {
    edges: Vec<Edge>,
}

impl Cover {
    /// The cover of `loops`.
    pub(crate) fn new<'a>(
        loops: impl IntoIterator<Item = &'a [ViewPoint]>,
    ) -> Result<Self, DrawingError> {
        let mut edges = Vec::new();
        for ring in loops {
            let pts: Vec<Pt> = ring.iter().map(|p| Pt::of(*p)).collect::<Result<_, _>>()?;
            for (i, a) in pts.iter().enumerate() {
                if let Some(b) = pts.get(after(i, pts.len()))
                    && a != b
                {
                    edges.push(Edge {
                        a: a.clone(),
                        b: b.clone(),
                        label: 0,
                    });
                }
            }
        }
        Ok(Self { edges })
    }

    /// The pieces of the segment from `a` to `b` that no area covers, in order along it,
    /// neighbouring pieces joined. A piece inside an area, or along an area's outline (where the
    /// area's own line is drawn), is hidden.
    pub(crate) fn outside(
        &self,
        a: ViewPoint,
        b: ViewPoint,
    ) -> Result<Vec<[ViewPoint; 2]>, DrawingError> {
        let e = Edge {
            a: Pt::of(a)?,
            b: Pt::of(b)?,
            label: 0,
        };
        let r = e.b.minus(&e.a);
        let rr = r.dot(&r);
        if rr.is_zero() {
            return Err(zero_division());
        }
        let mut cuts = cuts_of(&e, &r, &rr, self.edges.iter())?;
        cuts.sort();
        cuts.dedup();
        let mut stops = vec![Q::whole(0)];
        stops.extend(cuts);
        stops.push(Q::whole(1));
        let half = Q::fraction(1, 2)?;
        let v = r.left();
        let mut out: Vec<[Pt; 2]> = Vec::new();
        for pair in stops.windows(2) {
            let (Some(t0), Some(t1)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let p0 = e.a.plus(&r.scaled(t0));
            let p1 = e.a.plus(&r.scaled(t1));
            let m = p0.plus(&p1).scaled(&half);
            // Clear to both sides: a piece along an outline has the area on one side.
            if winding(&self.edges, &m, &v)? == 0 && winding(&self.edges, &m, &v.negated())? == 0 {
                match out.last_mut() {
                    Some(last) if last[1] == p0 => last[1] = p1,
                    _ => out.push([p0, p1]),
                }
            }
        }
        out.into_iter()
            .map(|[p0, p1]| Ok([p0.view()?, p1.view()?]))
            .collect()
    }
}

/// The intervals of `s = d · p` along the line `n · p = c` that are inside `edges`' area just to
/// both sides of the line.
fn inside(edges: &[Edge], n: &Pt, d: &Pt, c: &Q) -> Result<Vec<[Q; 2]>, DrawingError> {
    let mut sides = Vec::with_capacity(2);
    for strict in [true, false] {
        // A point is past the line moved a small step one way (strict) or the other.
        let past = |f: &Q| match f.sign() {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => !strict,
        };
        let mut crossings: Vec<(Q, i64)> = Vec::new();
        for e in edges {
            let (fa, fb) = (n.dot(&e.a).minus(c), n.dot(&e.b).minus(c));
            let (pa, pb) = (past(&fa), past(&fb));
            if pa == pb {
                continue;
            }
            let t = fa.over(&fa.minus(&fb))?;
            let p = e.a.plus(&e.b.minus(&e.a).scaled(&t));
            crossings.push((d.dot(&p), if pa { -1 } else { 1 }));
        }
        crossings.sort();
        let mut intervals = Vec::new();
        let mut w: i64 = 0;
        let mut open: Option<Q> = None;
        let mut i = 0;
        while let Some((s, _)) = crossings.get(i) {
            let s = s.clone();
            while let Some((t, step)) = crossings.get(i) {
                if *t != s {
                    break;
                }
                w = w.checked_add(*step).ok_or_else(|| {
                    DrawingError::Overlay("a winding number past 64 bits".to_owned())
                })?;
                i = i.saturating_add(1);
            }
            match (open.take(), w != 0) {
                (None, true) => open = Some(s),
                (Some(start), false) => {
                    if start < s {
                        intervals.push([start, s]);
                    }
                }
                (Some(start), true) => open = Some(start),
                (None, false) => {}
            }
        }
        sides.push(intervals);
    }
    // Inside to both sides: the overlap of the two lists.
    let (Some(a), Some(b)) = (sides.first(), sides.get(1)) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while let (Some([a0, a1]), Some([b0, b1])) = (a.get(i), b.get(j)) {
        let lo = a0.clone().max(b0.clone());
        let hi = a1.clone().min(b1.clone());
        if lo < hi {
            out.push([lo, hi]);
        }
        if a1 < b1 {
            i = i.saturating_add(1);
        } else {
            j = j.saturating_add(1);
        }
    }
    Ok(out)
}

/// The kept edges joined into loops, each turning the tightest left at every vertex.
fn trace(kept: &[Edge]) -> Result<Vec<Ring>, DrawingError> {
    let mut leaving: BTreeMap<&Pt, Vec<usize>> = BTreeMap::new();
    for (i, e) in kept.iter().enumerate() {
        leaving.entry(&e.a).or_default().push(i);
    }
    let broken = || DrawingError::Overlay("the boundary does not close into loops".to_owned());
    let mut used = vec![false; kept.len()];
    let mut rings = Vec::new();
    for start in 0..kept.len() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut ring = Vec::new();
        let mut at = start;
        loop {
            let e = kept.get(at).ok_or_else(broken)?;
            if let Some(u) = used.get_mut(at) {
                *u = true;
            }
            ring.push((e.a.clone(), e.label));
            let back = e.a.minus(&e.b);
            let next = leaving
                .get(&e.b)
                .ok_or_else(broken)?
                .iter()
                .copied()
                .min_by(|&x, &y| match (kept.get(x), kept.get(y)) {
                    (Some(ex), Some(ey)) => {
                        clockwise(&back, &ex.b.minus(&ex.a), &ey.b.minus(&ey.a))
                    }
                    _ => Ordering::Equal,
                })
                .ok_or_else(broken)?;
            if next == start {
                break;
            }
            if used.get(next).copied().unwrap_or(true) {
                return Err(broken());
            }
            at = next;
        }
        rings.push(joined(ring));
    }
    Ok(rings)
}

/// The loops sorted into areas: each hole with the smallest outline holding it.
fn areas(rings: Vec<Ring>) -> Result<Vec<Area>, DrawingError> {
    let mut outlines: Vec<(Q, Ring, Vec<Ring>)> = Vec::new();
    let mut holes = Vec::new();
    for ring in rings {
        let area = twice_area(&ring);
        match area.sign() {
            Ordering::Greater => outlines.push((area, ring, Vec::new())),
            Ordering::Less => holes.push(ring),
            Ordering::Equal => {
                return Err(DrawingError::Overlay("a loop of no area".to_owned()));
            }
        }
    }
    let orphan = || DrawingError::Overlay("a hole inside no outline".to_owned());
    for hole in holes {
        // A point just on the material side (the left) of the hole's first edge.
        let (Some((a, _)), Some((b, _))) = (hole.first(), hole.get(1)) else {
            return Err(orphan());
        };
        let m = a.plus(b).scaled(&Q::fraction(1, 2)?);
        let v = b.minus(a).left();
        let mut best: Option<usize> = None;
        for (i, (area, outline, _)) in outlines.iter().enumerate() {
            if winding(&edges_of(outline), &m, &v)? > 0
                && best
                    .and_then(|b| outlines.get(b))
                    .is_none_or(|(have, _, _)| area < have)
            {
                best = Some(i);
            }
        }
        let i = best.ok_or_else(orphan)?;
        outlines.get_mut(i).ok_or_else(orphan)?.2.push(hole);
    }
    let mut out = Vec::with_capacity(outlines.len());
    for (_, outline, holes) in outlines {
        let mut hole_loops = Vec::with_capacity(holes.len());
        for hole in holes {
            hole_loops.push(finished(&hole)?);
        }
        hole_loops.sort();
        let mut loops = vec![finished(&outline)?];
        loops.extend(hole_loops);
        out.push(Area { loops });
    }
    out.sort();
    Ok(out)
}
