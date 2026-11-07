//! Fuzz target: `geom-2d`'s entity model keeps its stated laws on any input.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a fraction's numerator and
//! denominator, a byte that may name a tag, a polyline's vertices, bulge count, closing flag and
//! width flag, a shape to build, an attribute key and a transform's classification. [`check`]
//! tests each of the model's rules against an independent reading of it: a fraction is refused
//! exactly when its denominator is zero or its reduced size is past the bound, and is otherwise the
//! same number, reduced, with a positive denominator; a number names a tag exactly when it is one
//! of the eighteen and the tag gives the number back; a polyline is made exactly when its counts
//! are right; and a shape put under a classification becomes what chapter 14.10 says (a circle an
//! ellipse under a non-uniform transform, a polyline with a bulge a run, nothing else changes
//! kind, a degenerate transform is refused), with its attribute key untouched. [`harness`] is
//! `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against the rules of chapters 14.1 and 14.10
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`), written here from the rules and not
//! from the code: Euclid's algorithm on 128-bit numbers for the fractions, and a table of the
//! kind changes.
//!
//! Cost: constant per case, at most [`MAX_VERTICES`] vertices; at most [`MAX_INPUT_BYTES`] bytes
//! read.

use geom_2d::carrier::{Circle2, Line2};
use geom_2d::entity::{
    Attr, Becomes, Ends, Entity, EntityError, FRAC_LIMIT, Frac, Polyline, Shape, Tag,
};
use geom_2d::point::Pt2;
use geom_math::{Classification, TransformKind};
use geom_num::arc::Circle2 as GridCircle;
use geom_num::grid::{P2, Steps};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most vertices of a case's polyline.
pub const MAX_VERTICES: u8 = 8;

/// The most bytes [`decode`] reads from any input: 58.
///
/// A mode byte and the fraction's two words: 1 + 16. A tag byte: 1. A vertex count, a bulge count,
/// a closing flag and a width flag: 4. A bulge value: 1. Two bytes for each of at most
/// [`MAX_VERTICES`] vertices: 16. A shape pick, a kind and a mirror flag: 3. The attribute key's
/// word: 8. Eight spare bytes for the circle and the segment: 8. So
/// 1 + 16 + 1 + 4 + 1 + 16 + 3 + 8 + 8 = 58. The unit test `the_input_bound_is_reached_and_never_passed`
/// builds the input.
pub const MAX_INPUT_BYTES: usize = 1 + 16 + 1 + 4 + 1 + 2 * 8 + 3 + 8 + 8;

const _: () = assert!(MAX_INPUT_BYTES == 58);

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The numerator offered to `Frac::new`.
    pub n: i64,
    /// The denominator offered.
    pub d: i64,
    /// A number that may or may not be a tag.
    pub tag: u8,
    /// Vertices offered to the polyline.
    pub vertices: Vec<(i64, i64)>,
    /// How many bulges are offered, before the count is checked.
    pub bulges: u8,
    /// Whether the polyline closes.
    pub closed: bool,
    /// Whether widths are offered, one for each vertex.
    pub widths: bool,
    /// The bulge value, a numerator over three.
    pub bulge: i8,
    /// Which shape to build: 0 polyline, 1 circle, 2 segment, 3 point.
    pub pick: u8,
    /// The classification's kind, 0 to 5.
    pub kind: u8,
    /// Whether the classification mirrors.
    pub mirror: bool,
    /// The attribute key.
    pub attr: u64,
    /// A circle's centre and radius, and a segment's end points, as small numbers.
    pub spare: [i8; 8],
}

/// A byte as a signed number.
fn signed(b: u8) -> i8 {
    i8::from_le_bytes([b])
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let mode = src.byte() % 3;
    let (a, b) = (src.word(), src.word());
    // Mode 0: raw words; 1: a byte each; 2: near the bound, where the size check matters.
    let (n, d) = match mode {
        0 => (i64::from_le_bytes(a.to_le_bytes()), i64::from_le_bytes(b.to_le_bytes())),
        1 => (i64::from(signed(a.to_le_bytes()[0])), i64::from(signed(b.to_le_bytes()[0]))),
        _ => (
            near_bound(a),
            near_bound(b),
        ),
    };
    let tag = src.byte();
    let count = src.byte() % (MAX_VERTICES + 1);
    let bulges = src.byte() % (MAX_VERTICES + 1);
    let closed = src.byte() % 2 == 1;
    let widths = src.byte() % 2 == 1;
    let bulge = signed(src.byte());
    let vertices = (0..count)
        .map(|_| {
            let x = i64::from(signed(src.byte()));
            (x, i64::from(signed(src.byte())))
        })
        .collect();
    let pick = src.byte() % 4;
    let kind = src.byte() % 6;
    let mirror = src.byte() % 2 == 1;
    let attr = src.word();
    let mut spare = [0_i8; 8];
    for s in &mut spare {
        *s = signed(src.byte());
    }
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            n,
            d,
            tag,
            vertices,
            bulges,
            closed,
            widths,
            bulge,
            pick,
            kind,
            mirror,
            attr,
            spare,
        },
        read,
    )
}

/// A number within one of a multiple of half the bound: the edge of what a fraction may hold.
fn near_bound(word: u64) -> i64 {
    let k = i64::try_from(word % 4).unwrap_or(0);
    let step = i64::try_from((word >> 8) % 3).unwrap_or(0).saturating_sub(1);
    k.saturating_mul(FRAC_LIMIT / 2).saturating_add(step)
}

/// The greatest common divisor, by Euclid's algorithm, on 128-bit numbers.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    a
}

fn fail(case: &Case, law: &str) -> Finding {
    Finding::new(law, || format!("{case:?}"))
}

/// Tests the model's laws on the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    fraction(case)?;
    tags(case)?;
    let polyline = polyline(case)?;
    kinds(case, polyline)
}

/// A fraction is refused exactly when its denominator is zero or its reduced size is past the
/// bound, and is otherwise the same number, reduced, with a positive denominator.
fn fraction(case: &Case) -> Result<(), Finding> {
    let (n, d) = (i128::from(case.n), i128::from(case.d));
    let got = Frac::new(case.n, case.d);
    if d == 0 {
        return if got == Err(EntityError::Fraction) {
            Ok(())
        } else {
            Err(fail(case, "a zero denominator is refused"))
        };
    }
    let g = gcd(n.unsigned_abs(), d.unsigned_abs());
    let (rn, rd) = (
        n.unsigned_abs().checked_div(g).unwrap_or(0),
        d.unsigned_abs().checked_div(g).unwrap_or(0),
    );
    let limit = u128::from(FRAC_LIMIT.unsigned_abs());
    let too_big = rn >= limit || rd >= limit;
    match (got, too_big) {
        (Err(EntityError::Fraction), true) => Ok(()),
        (Ok(f), false) => {
            let (fnum, fden) = (i128::from(f.numerator()), i128::from(f.denominator()));
            if fden <= 0 {
                return Err(fail(case, "a fraction's denominator is positive"));
            }
            if fnum.checked_mul(d) != fden.checked_mul(n) {
                return Err(fail(case, "a fraction is the number it was made from"));
            }
            if gcd(fnum.unsigned_abs(), fden.unsigned_abs()) != 1 {
                return Err(fail(case, "a fraction is reduced"));
            }
            Ok(())
        }
        _ => Err(fail(
            case,
            "a fraction is refused exactly when it has a zero denominator or is too big",
        )),
    }
}

/// A number names a tag exactly when it is 1 to 18, and the tag gives the number back.
fn tags(case: &Case) -> Result<(), Finding> {
    let got = Tag::from_u8(case.tag);
    if got.is_some() != (1..=18).contains(&case.tag) {
        return Err(fail(case, "a number names a tag exactly when it is 1 to 18"));
    }
    if let Some(t) = got {
        let place = usize::from(case.tag).checked_sub(1).and_then(|i| Tag::ALL.get(i));
        if t.as_u8() != case.tag || place != Some(&t) {
            return Err(fail(case, "a tag gives its number back and sits at that place"));
        }
    }
    Ok(())
}

/// A polyline is made exactly when its counts are right.
fn polyline(case: &Case) -> Result<Option<Polyline>, Finding> {
    let points = case
        .vertices
        .iter()
        .map(|&(x, y)| P2::new(x, y).map_err(|_| fail(case, "small points are on the grid")))
        .collect::<Result<Vec<P2>, Finding>>()?;
    let bulge = Frac::new(i64::from(case.bulge), 3)
        .map_err(|_| fail(case, "a small fraction is made"))?;
    let bulges = vec![bulge; usize::from(case.bulges)];
    let widths = case
        .widths
        .then(|| vec![(Steps(1), Steps(2)); case.vertices.len()]);
    let n = case.vertices.len();
    let segments = if case.closed { Some(n) } else { n.checked_sub(1) };
    let least = if case.closed { 3 } else { 2 };
    // Widths are offered one for each vertex, which is right only for a closed polyline.
    let widths_right = widths.as_ref().is_none_or(|w| Some(w.len()) == segments);
    let expected = n >= least && segments == Some(bulges.len()) && widths_right;
    match (Polyline::new(points, bulges, widths, case.closed), expected) {
        (Ok(p), true) => Ok(Some(p)),
        (Err(EntityError::Polyline), false) => Ok(None),
        _ => Err(fail(
            case,
            "a polyline is made exactly when its counts are right",
        )),
    }
}

/// A shape put under a classification becomes what 14.10 says, and its key is untouched.
fn kinds(case: &Case, polyline: Option<Polyline>) -> Result<(), Finding> {
    let spare = |i: usize| i64::from(case.spare.get(i).copied().unwrap_or(0));
    let at = |x: i64, y: i64| {
        P2::new(x, y).map_err(|_| fail(case, "small points are on the grid"))
    };
    let shape = match case.pick {
        0 => match polyline {
            Some(pl) => Shape::Polyline(pl),
            None => return Ok(()),
        },
        1 => {
            let radius = spare(2).abs().saturating_add(1);
            match GridCircle::new(at(spare(0), spare(1))?, i128::from(radius)) {
                Ok(c) => Shape::Circle(Circle2::Grid(c)),
                Err(_) => return Ok(()),
            }
        }
        2 => Shape::Line {
            carrier: Line2::new(0, 1, 0).map_err(|_| fail(case, "a line carrier is made"))?,
            ends: Ends::Segment(
                Pt2::Grid(at(spare(3), spare(4))?),
                Pt2::Grid(at(spare(5), spare(6))?),
            ),
        },
        _ => Shape::Point(at(spare(0), spare(1))?),
    };
    let kind = match case.kind {
        0 => TransformKind::Identity,
        1 => TransformKind::Translation,
        2 => TransformKind::Rigid,
        3 => TransformKind::UniformScale,
        4 => TransformKind::NonUniform,
        _ => TransformKind::Degenerate,
    };
    let class = Classification {
        kind,
        mirror: case.mirror && kind != TransformKind::Degenerate,
    };
    let got = Entity::new(Attr(case.attr), shape.clone()).under(class);
    if kind == TransformKind::Degenerate {
        return if got.map(|u| u.change.mirror) == Err(EntityError::Degenerate) {
            Ok(())
        } else {
            Err(fail(case, "a degenerate transform has no image"))
        };
    }
    let under = got.map_err(|_| fail(case, "an invertible transform has an image"))?;
    if under.attr != Attr(case.attr) {
        return Err(fail(case, "the attribute key passes through"));
    }
    if under.change.mirror != class.mirror {
        return Err(fail(case, "a mirror is reported"));
    }
    let round = kind != TransformKind::NonUniform;
    let expected = match (&shape, round) {
        (Shape::Circle(_), false) => Becomes::One(Tag::Ellipse),
        (Shape::Polyline(pl), false) if !pl.is_straight() => Becomes::Run,
        _ => Becomes::One(shape.tag()),
    };
    if under.change.becomes != expected {
        return Err(fail(case, "a kind changes exactly as chapter 14.10 says"));
    }
    Ok(())
}

/// Decodes `data` and checks the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
