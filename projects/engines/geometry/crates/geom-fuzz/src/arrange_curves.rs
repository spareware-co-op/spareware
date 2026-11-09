//! Fuzz target: `geom-2d`'s arrangement of segments, circular arcs and ellipses keeps its stated
//! laws on any input.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a count of curves and, for each,
//! a kind byte and the small numbers of a segment (its two ends), an arc (its two ends and how far
//! its centre lies along their bisector, in quarter steps) or a whole ellipse (its centre, its
//! major-axis vector and its axis ratio). [`check`] arranges them with `arrange_curves` and tests
//! each rule against an independent reading of it. A refusal because an ellipse may meet a circle
//! or another ellipse ([`ArrangeError::Conic`]) is right only if some such pair meets at all, by
//! the real roots of their quartic. Otherwise the vertices are exactly a pairwise oracle's: every
//! end of a segment or an arc, both extremes of every ellipse and every extreme inside an arc, and
//! every crossing of two curves that lies on both, found pair by pair from the curves' own
//! carriers and circles (an ellipse of ratio one is its circle); Euler's formula holds,
//! V − E + F = 1 + C for C connected components; every half-edge lies on exactly one cycle and
//! every cycle bounds one face or is a hole of exactly one; the vertices ascend and every edge runs
//! up and says what it lies on; and arranging the curves in the opposite order gives the same
//! verdict, vertices, edges, faces and order round each vertex, with only the source labels
//! renumbered. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against the crossings of each pair of curves on their own carriers
//! and circles (the one-root points of chapter 14.2, as Fogel, Halperin and Wein's circle-segment
//! traits define them, 2012, <https://doi.org/10.1007/978-3-642-17283-0>, from their book), with
//! the real roots of the quartic of an ellipse and a conic (Collins and Akritas 1976,
//! <https://doi.org/10.1145/800205.806346>) as the test of whether two such curves meet, and
//! Euler's formula for planar graphs; written here from the rules and not from the sweep's code.
//!
//! Cost: at most [`MAX_CURVES`] curves, so at most fifteen pairs, each a few exact one-root or
//! quartic decisions; at most [`MAX_INPUT_BYTES`] bytes read.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b, c, d, e, f, k, l, m, p, q, r, s, v, x, y and z are the geometry's own letters"
)]

use std::collections::BTreeSet;

use geom_2d::arrange::{ArrangeError, Arrangement, Curve, arrange_curves};
use geom_num::arc::{Arc2, Contact, Disc, Round};
use geom_num::carrier::{Carrier, Rational2};
use geom_num::conic::{Conic, Ellipse2, Meeting};
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};
use geom_num::{NumError, Sign};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most curves of a case.
pub const MAX_CURVES: u8 = 6;

/// The most bytes [`decode`] reads from any input: 37.
///
/// A count byte, and for each of at most [`MAX_CURVES`] curves a kind byte and at most five bytes
/// (an arc's four coordinates and its bend, or an ellipse's two coordinates, two axis parts and
/// ratio; a segment reads four): 1 + 6 · (1 + 5) = 37. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds the input.
pub const MAX_INPUT_BYTES: usize = 1 + 6 * (1 + 5);

const _: () = assert!(MAX_INPUT_BYTES == 37);

/// One curve of a case, as small numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A segment (x0, y0, x1, y1).
    Segment([i8; 4]),
    /// An arc from (x0, y0) to (x1, y1), counter-clockwise, about the point of their bisector
    /// (2(P + Q) + bend · (Q − P) turned a quarter) / 4; ignored when its ends are one point.
    Arc {
        /// Its ends, (x0, y0, x1, y1).
        ends: [i8; 4],
        /// Its centre's place along the bisector, from −4 to 4 quarter steps of the chord.
        bend: i8,
    },
    /// A whole ellipse about `centre`, its major axis `major` (the zero vector read as (1, 0)) and
    /// its axis ratio p / q, reduced, from 1 / 5 to 1.
    Ellipse {
        /// Its centre.
        centre: [i8; 2],
        /// Its major-axis vector.
        major: [i8; 2],
        /// The ratio's numerator, from 1 to `q`.
        p: u8,
        /// The ratio's denominator, from 2 to 5.
        q: u8,
    },
}

/// One case: its curves in input order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The curves.
    pub curves: Vec<Shape>,
}

/// A signed byte divided by `by`, so that the curves fall in a narrow band and meet.
fn small(src: &mut ByteSource<'_>, by: i8) -> i8 {
    i8::from_le_bytes([src.byte()]).checked_div(by).unwrap_or(0)
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let count = usize::from(src.byte() % (MAX_CURVES + 1));
    let curves = (0..count)
        .map(|_| match src.byte() % 3 {
            0 => Shape::Segment([(); 4].map(|()| small(&mut src, 8))),
            1 => {
                // Ends nearer the middle than a segment's, so that arcs meet the ellipses too.
                let ends = [(); 4].map(|()| small(&mut src, 16));
                let bend = i8::from_le_bytes([src.byte()]).checked_rem(5).unwrap_or(0);
                Shape::Arc { ends, bend }
            }
            _ => {
                let centre = [(); 2].map(|()| small(&mut src, 8));
                let major = [(); 2].map(|()| small(&mut src, 32));
                let r = src.byte();
                let q = (r % 4).saturating_add(2);
                let p = (r / 4).checked_rem(q).unwrap_or(0).saturating_add(1);
                Shape::Ellipse {
                    centre,
                    major,
                    p,
                    q,
                }
            }
        })
        .collect();
    let read = data.len().saturating_sub(src.remaining());
    (Case { curves }, read)
}

fn fail(case: &Case, law: &str) -> Finding {
    Finding::new(law, || format!("{case:?}"))
}

fn point(x: i8, y: i8) -> Result<P2, NumError> {
    P2::new(i64::from(x), i64::from(y))
}

fn gcd(a: u8, b: u8) -> u8 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    a.max(1)
}

/// The curves of a case, in input order; an arc whose ends are one point is left out.
fn curves_of(case: &Case) -> Result<Vec<Curve>, NumError> {
    let mut out = Vec::new();
    for shape in &case.curves {
        match *shape {
            Shape::Segment([x0, y0, x1, y1]) => {
                out.push(Curve::Segment(point(x0, y0)?, point(x1, y1)?));
            }
            Shape::Arc { ends, bend } => {
                let [px, py, qx, qy] = ends.map(i128::from);
                if (px, py) == (qx, qy) {
                    continue;
                }
                let s = i128::from(bend);
                let fit = |v: Option<i128>| v.ok_or(NumError::BudgetExceeded);
                let x = fit(fit(px.checked_add(qx))?
                    .checked_mul(2)
                    .and_then(|v| v.checked_sub(s.checked_mul(qy.checked_sub(py)?)?)))?;
                let y = fit(fit(py.checked_add(qy))?
                    .checked_mul(2)
                    .and_then(|v| v.checked_add(s.checked_mul(qx.checked_sub(px)?)?)))?;
                let centre = Rational2::new(x, y, 4)?;
                let [x0, y0, x1, y1] = ends;
                out.push(Curve::Arc(Arc2::new(
                    point(x0, y0)?,
                    point(x1, y1)?,
                    centre,
                )?));
            }
            Shape::Ellipse {
                centre,
                major,
                p,
                q,
            } => {
                let m = if major == [0, 0] { [1, 0] } else { major };
                let [cx, cy] = centre;
                let g = gcd(p, q);
                let (p, q) = (p.checked_div(g).unwrap_or(p), q.checked_div(g).unwrap_or(q));
                out.push(Curve::Ellipse(Ellipse2::new(
                    point(cx, cy)?,
                    m.map(i64::from),
                    i64::from(p),
                    i64::from(q),
                )?));
            }
        }
    }
    Ok(out)
}

/// Whether an ellipse is a circle: its ratio is one.
fn round_one(e: &Ellipse2) -> bool {
    let (p, q) = e.ratio();
    p == q
}

/// An ellipse of ratio one as its circle: about its centre, through its centre plus its axis.
fn circle_of(e: &Ellipse2) -> Result<Round, NumError> {
    let k = e.centre();
    let [mx, my] = e.major();
    let on = P2::new(
        k.x.get().checked_add(mx).ok_or(NumError::BudgetExceeded)?,
        k.y.get().checked_add(my).ok_or(NumError::BudgetExceeded)?,
    )?;
    Ok(Round::new(on, k.into()))
}

/// Whether the ellipse `e` meets the conic `c` at a point of the plane: their quartic has a real
/// root. One curve written twice is not a meeting.
fn meets(e: &Ellipse2, c: &Conic) -> Result<bool, NumError> {
    Ok(match e.meeting(c)? {
        Meeting::Same => false,
        Meeting::At { roots, .. } => roots.count() > 0,
    })
}

/// Whether two curves are a pair the sweep may refuse: an ellipse of ratio other than one and a
/// circle or another ellipse, that meet.
fn conic_pair(c: &Curve, d: &Curve) -> Result<bool, NumError> {
    match (c, d) {
        (Curve::Ellipse(e), Curve::Ellipse(f)) => {
            if round_one(e) && round_one(f) {
                Ok(false)
            } else {
                meets(e, &Conic::of_ellipse(f)?)
            }
        }
        (Curve::Ellipse(e), Curve::Arc(a)) | (Curve::Arc(a), Curve::Ellipse(e)) => {
            if round_one(e) {
                Ok(false)
            } else {
                meets(e, &Conic::of_round(&a.round())?)
            }
        }
        _ => Ok(false),
    }
}

/// Order along a line: by x, then by y.
fn along(u: &Crossing2, v: &Crossing2) -> Result<Sign, NumError> {
    Ok(match u.cmp_x(v)? {
        Sign::Zero => u.cmp_y(v)?,
        s => s,
    })
}

/// Whether the crossing `z` of a segment's line lies between the segment's ends.
fn within(z: &Crossing2, a: P2, b: P2) -> Result<bool, NumError> {
    let za = Crossing2::of_point(&a.into())?;
    let zb = Crossing2::of_point(&b.into())?;
    let (sa, sb) = (along(z, &za)?, along(z, &zb)?);
    Ok(sa != sb || sa == Sign::Zero)
}

/// The roots where a line meets a curve, by contact.
fn roots_of(contact: Contact) -> &'static [Root] {
    match contact {
        Contact::Apart => &[],
        Contact::Tangent => &[Root::First],
        Contact::Crossing => &[Root::First, Root::Second],
    }
}

/// Where two circles cross, as two `Round`s: none for one circle twice.
fn rounds_cross(x: &Round, y: &Round) -> Result<Vec<Crossing2>, NumError> {
    let mut found = Vec::new();
    for r in [Root::First, Root::Second] {
        match Crossing2::of_rounds(x, y, r) {
            Ok(z) => found.push(z),
            Err(NumError::Degenerate) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(found)
}

/// Where two curves cross, on both. An ellipse of ratio one is its circle; an ellipse of another
/// ratio meets a circle or an ellipse at no point of both, or the sweep would have refused.
fn crossings(c: &Curve, d: &Curve) -> Result<Vec<Crossing2>, NumError> {
    let mut found = Vec::new();
    match (c, d) {
        (Curve::Segment(a, b), Curve::Segment(e, f)) => {
            if a == b || e == f {
                return Ok(found);
            }
            let (l, m) = (Carrier::through(*a, *b)?, Carrier::through(*e, *f)?);
            if l.parallel(&m)? {
                return Ok(found);
            }
            let z = Crossing2::of_point(&l.meet(&m)?)?;
            if within(&z, *a, *b)? && within(&z, *e, *f)? {
                found.push(z);
            }
        }
        (Curve::Segment(a, b), Curve::Ellipse(e)) | (Curve::Ellipse(e), Curve::Segment(a, b)) => {
            if a == b {
                return Ok(found);
            }
            let l = Carrier::through(*a, *b)?;
            for &r in roots_of(e.meets(&l)?) {
                let z = Crossing2::of_ellipse(&l, e, r)?;
                if within(&z, *a, *b)? {
                    found.push(z);
                }
            }
        }
        (Curve::Segment(a, b), Curve::Arc(arc)) | (Curve::Arc(arc), Curve::Segment(a, b)) => {
            if a == b {
                return Ok(found);
            }
            let l = Carrier::through(*a, *b)?;
            for &r in roots_of(arc.round().meets(&l)?) {
                let z = Crossing2::of_round(&l, &arc.round(), r)?;
                if within(&z, *a, *b)? && arc.holds_crossing(&z)? {
                    found.push(z);
                }
            }
        }
        (Curve::Arc(x), Curve::Arc(y)) => {
            for z in rounds_cross(&x.round(), &y.round())? {
                if x.holds_crossing(&z)? && y.holds_crossing(&z)? {
                    found.push(z);
                }
            }
        }
        (Curve::Arc(arc), Curve::Ellipse(e)) | (Curve::Ellipse(e), Curve::Arc(arc)) => {
            if round_one(e) {
                for z in rounds_cross(&circle_of(e)?, &arc.round())? {
                    if arc.holds_crossing(&z)? {
                        found.push(z);
                    }
                }
            }
        }
        (Curve::Ellipse(e), Curve::Ellipse(f)) => {
            if round_one(e) && round_one(f) {
                found = rounds_cross(&circle_of(e)?, &circle_of(f)?)?;
            }
        }
        (Curve::EllipticArc { .. }, _) | (_, Curve::EllipticArc { .. }) => {
            return Err(NumError::Degenerate);
        }
    }
    Ok(found)
}

/// The oracle's vertices: every end, both extremes of every ellipse and every extreme strictly
/// inside an arc, and every crossing of two curves that lies on both, each once.
fn oracle(curves: &[Curve]) -> Result<Vec<Crossing2>, NumError> {
    let mut out: Vec<Crossing2> = Vec::new();
    let mut add = |z: Crossing2| -> Result<(), NumError> {
        for o in &out {
            if o.same(&z)? {
                return Ok(());
            }
        }
        out.push(z);
        Ok(())
    };
    let grid = |q: P2| Crossing2::of_point(&q.into());
    for c in curves {
        match c {
            Curve::Segment(a, b) => {
                if a != b {
                    add(grid(*a)?)?;
                    add(grid(*b)?)?;
                }
            }
            Curve::Arc(arc) => {
                add(grid(arc.start())?)?;
                add(grid(arc.end())?)?;
                let disc = Disc::of_round(&arc.round())?;
                for r in [Root::First, Root::Second] {
                    let z = Crossing2::of_extreme(&disc, r)?;
                    if arc.holds_crossing(&z)? {
                        add(z)?;
                    }
                }
            }
            Curve::Ellipse(e) => {
                for r in [Root::First, Root::Second] {
                    add(Crossing2::of_ellipse_extreme(e, r)?)?;
                }
            }
            Curve::EllipticArc { .. } => return Err(NumError::Degenerate),
        }
    }
    for (i, c) in curves.iter().enumerate() {
        for d in curves.get(i.saturating_add(1)..).unwrap_or_default() {
            for z in crossings(c, d)? {
                add(z)?;
            }
        }
    }
    Ok(out)
}

/// The number of connected components of the edge graph.
fn components(a: &Arrangement) -> usize {
    let n = a.vertices().len();
    let mut parent: Vec<usize> = (0..n).collect();
    let find = |parent: &mut Vec<usize>, mut x: usize| {
        while let Some(&p) = parent.get(x) {
            if p == x {
                break;
            }
            let grand = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(x) {
                *slot = grand;
            }
            x = grand;
        }
        x
    };
    for e in a.edges() {
        let u = find(&mut parent, usize::try_from(e.from).unwrap_or(0));
        let v = find(&mut parent, usize::try_from(e.to).unwrap_or(0));
        if let Some(slot) = parent.get_mut(u) {
            *slot = v;
        }
    }
    let mut roots = BTreeSet::new();
    for v in 0..n {
        roots.insert(find(&mut parent, v));
    }
    roots.len()
}

/// Euler's formula, every half-edge on one cycle, every cycle in one face, the vertices strictly
/// ascending, and every edge running up with what it lies on.
fn laws(case: &Case, a: &Arrangement) -> Result<(), Finding> {
    let (v, e, f, c) = (
        a.vertices().len(),
        a.edges().len(),
        a.faces().len(),
        components(a),
    );
    if v.checked_add(f) != e.checked_add(c).and_then(|n| n.checked_add(1)) {
        return Err(fail(case, "Euler's formula: V - E + F = 1 + C"));
    }
    let half_edges: usize = a
        .cycles()
        .iter()
        .map(Vec::len)
        .fold(0, usize::saturating_add);
    if Some(half_edges) != e.checked_mul(2) {
        return Err(fail(case, "every half-edge is on one cycle"));
    }
    let mut owners = vec![0_u32; a.cycles().len()];
    for face in a.faces() {
        for c in face.outer.iter().chain(face.holes.iter()) {
            if let Some(n) = usize::try_from(*c).ok().and_then(|i| owners.get_mut(i)) {
                *n = n.saturating_add(1);
            }
        }
    }
    if owners.iter().any(|&n| n != 1) {
        return Err(fail(
            case,
            "every cycle bounds one face or is a hole of exactly one",
        ));
    }
    for w in a.vertices().windows(2) {
        let [u, v] = w else { continue };
        let up = match u.cmp_x(v) {
            Ok(Sign::Negative) => true,
            Ok(Sign::Zero) => u.cmp_y(v) == Ok(Sign::Negative),
            _ => false,
        };
        if !up {
            return Err(fail(case, "the vertices ascend strictly"));
        }
    }
    for (i, edge) in a.edges().iter().enumerate() {
        let known = u32::try_from(i).ok().and_then(|i| a.along(i)).is_some();
        if edge.from >= edge.to || !known {
            return Err(fail(case, "every edge runs up and says what it lies on"));
        }
    }
    Ok(())
}

/// The same arrangement for the curves in the opposite order, but for the source labels.
fn same_reversed(a: &Arrangement, b: &Arrangement, n: usize) -> bool {
    let vertices = a.vertices().len() == b.vertices().len()
        && a.vertices()
            .iter()
            .zip(b.vertices())
            .all(|(u, v)| u.same(v) == Ok(true));
    let ends =
        |x: &Arrangement| -> Vec<(u32, u32)> { x.edges().iter().map(|e| (e.from, e.to)).collect() };
    let lies = |x: &Arrangement| -> Vec<_> {
        (0..u32::try_from(x.edges().len()).unwrap_or(0))
            .map(|e| x.along(e))
            .collect()
    };
    let last = u32::try_from(n).unwrap_or(0).saturating_sub(1);
    let sources = a.edges().iter().zip(b.edges()).all(|(ea, eb)| {
        let mut mapped: Vec<u32> = eb.sources.iter().map(|&s| last.saturating_sub(s)).collect();
        mapped.sort_unstable();
        ea.sources == mapped
    });
    let rounds =
        (0..u32::try_from(a.vertices().len()).unwrap_or(0)).all(|v| a.around(v) == b.around(v));
    vertices
        && ends(a) == ends(b)
        && lies(a) == lies(b)
        && sources
        && a.faces() == b.faces()
        && a.circles().len() == b.circles().len()
        && a.ellipses() == b.ellipses()
        && rounds
}

fn arrange(curves: &[Curve]) -> Result<Arrangement, ArrangeError> {
    arrange_curves(curves, &mut WorkMeter::new(Budget::UNLIMITED))
}

/// Tests the arrangement's laws on the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let curves = curves_of(case).map_err(|_| fail(case, "small curves are accepted"))?;
    let mut reversed = curves.clone();
    reversed.reverse();
    let (a, b) = (arrange(&curves), arrange(&reversed));
    let a = match (a, b) {
        (Ok(a), Ok(b)) => {
            if !same_reversed(&a, &b, curves.len()) {
                return Err(fail(case, "the input order does not matter"));
            }
            a
        }
        (Err(ArrangeError::Conic), Err(ArrangeError::Conic)) => {
            let mut any = false;
            for (i, c) in curves.iter().enumerate() {
                for d in curves.get(i.saturating_add(1)..).unwrap_or_default() {
                    any |= conic_pair(c, d).map_err(|_| fail(case, "the oracle fits"))?;
                }
            }
            return if any {
                Ok(())
            } else {
                Err(fail(
                    case,
                    "refused only where an ellipse meets a circle or another ellipse",
                ))
            };
        }
        (Err(ArrangeError::Conic), Ok(_)) | (Ok(_), Err(ArrangeError::Conic)) => {
            return Err(fail(case, "the input order does not matter"));
        }
        _ => return Err(fail(case, "small curves are within the exact budget")),
    };
    laws(case, &a)?;
    let want = oracle(&curves).map_err(|_| fail(case, "the oracle fits"))?;
    let mut matched = a.vertices().len() == want.len();
    for v in a.vertices() {
        let z = v
            .crossing()
            .map_err(|_| fail(case, "a vertex is a one-root point"))?;
        let mut found = false;
        for w in &want {
            found |= w.same(&z).map_err(|_| fail(case, "the oracle fits"))?;
        }
        matched &= found;
    }
    if !matched {
        return Err(fail(
            case,
            "the vertices are exactly the ends, the extremes and the crossings on both curves",
        ));
    }
    Ok(())
}

/// Whether the case is refused because an ellipse may meet a circle or another ellipse.
#[must_use]
pub fn refused(case: &Case) -> bool {
    curves_of(case).is_ok_and(|c| matches!(arrange(&c), Err(ArrangeError::Conic)))
}

/// Decodes `data` and checks the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
