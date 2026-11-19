//! Fuzz target: `geom-2d`'s transform and length edits keep their stated laws on any input.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a polyline's vertices, a
//! magnitude that scales them toward the coordinate range, a bulge for each segment, a closing flag
//! and a placement (quarter turns, a mirror flag, a whole scale and a shift). [`check`] tests each
//! rule against an independent reading of it: a placement succeeds exactly when every image is on
//! the grid and then gives the image the definition gives, in 128-bit whole numbers, with each
//! bulge kept (or negated by a mirror); a placement of unit scale is undone by its inverse; and the
//! certified length of a polyline is an ordered interval that meets the length of the same
//! polyline placed by a map that keeps lengths. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a disagreement is a [`Finding`]** naming the law and the case.
//!
//! Method: differential testing against the definition of a similarity of the plane in whole
//! numbers (`projects/engines/geometry/architecture/14-2d-drafting.md`, the edit table of 14.7),
//! written here from the rules and not from the code.
//!
//! Cost: constant per case, at most [`MAX_VERTICES`] vertices; at most [`MAX_INPUT_BYTES`] bytes
//! read.

use geom_2d::edit::{Similarity, place_polyline, polyline_length};
use geom_2d::entity::{Frac, Polyline};
use geom_num::grid::P2;
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most vertices of a case's polyline.
pub const MAX_VERTICES: u8 = 8;

/// The most bytes [`decode`] reads from any input: 32.
///
/// A vertex count and a closing flag: 2. A magnitude: 1. Two bytes for each of at most
/// [`MAX_VERTICES`] vertices: 16. One bulge byte for each of at most [`MAX_VERTICES`] segments (a
/// closed polyline has as many segments as vertices): 8. Quarter turns, a mirror flag, a scale and
/// two shift bytes: 5. So 2 + 1 + 16 + 8 + 5 = 32. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds the input.
pub const MAX_INPUT_BYTES: usize = 2 + 1 + 2 * 8 + 8 + 5;

const _: () = assert!(MAX_INPUT_BYTES == 32);

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The vertices, before the magnitude.
    pub vertices: Vec<(i8, i8)>,
    /// Every coordinate is its byte times 2 to this power.
    pub magnitude: u8,
    /// A bulge numerator (over three) for each segment.
    pub bulges: Vec<i8>,
    /// Whether the polyline closes.
    pub closed: bool,
    /// Quarter turns, 0 to 3.
    pub turns: u8,
    /// Whether the map mirrors.
    pub mirror: bool,
    /// The whole scale.
    pub scale: i8,
    /// The shift, in steps.
    pub shift: (i8, i8),
}

/// A byte as a signed number.
fn signed(b: u8) -> i8 {
    i8::from_le_bytes([b])
}

/// Decodes `data` into a case, and returns it with the number of bytes read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let count = usize::from(src.byte() % (MAX_VERTICES - 1)).saturating_add(2);
    let closed = src.byte() % 2 == 1 && count >= 3;
    let magnitude = src.byte() % 56;
    let vertices = (0..count)
        .map(|_| {
            let x = signed(src.byte());
            (x, signed(src.byte()))
        })
        .collect();
    let segments = if closed {
        count
    } else {
        count.saturating_sub(1)
    };
    let bulges = (0..segments).map(|_| signed(src.byte())).collect();
    let turns = src.byte() % 4;
    let mirror = src.byte() % 2 == 1;
    let scale = signed(src.byte());
    let shift = (signed(src.byte()), signed(src.byte()));
    let read = data.len().saturating_sub(src.remaining());
    (
        Case {
            vertices,
            magnitude,
            bulges,
            closed,
            turns,
            mirror,
            scale,
            shift,
        },
        read,
    )
}

fn fail(case: &Case, law: &str) -> Finding {
    Finding::new(law, || format!("{case:?}"))
}

/// The grid point of a byte pair scaled by the magnitude, or `None` past the range.
fn point(case: &Case, v: (i8, i8)) -> Option<P2> {
    let k = 1_i64.checked_shl(u32::from(case.magnitude))?;
    P2::new(
        i64::from(v.0).checked_mul(k)?,
        i64::from(v.1).checked_mul(k)?,
    )
    .ok()
}

/// A bulge of `b / 3`, reduced.
fn bulge(b: i8) -> Option<Frac> {
    Frac::new(i64::from(b), 3).ok()
}

/// The definition of the map on a vector, in 128-bit whole numbers: turn, mirror, scale.
fn linear(case: &Case, v: (i128, i128)) -> Option<(i128, i128)> {
    let (mut x, mut y) = v;
    for _ in 0..case.turns {
        (x, y) = (y.checked_neg()?, x);
    }
    if case.mirror {
        y = y.checked_neg()?;
    }
    let k = i128::from(case.scale);
    Some((x.checked_mul(k)?, y.checked_mul(k)?))
}

/// The map of the case: quarter turns about the origin, a mirror, the scale, then the shift, built
/// from the crate's own constructors in the order the definition names.
fn map(case: &Case) -> Option<Similarity> {
    let origin = P2::new(0, 0).ok()?;
    let mut m = Similarity::quarter_turns(case.turns, origin).ok()?;
    if case.mirror {
        m = Similarity::mirror_in_row(0).ok()?.after(&m).ok()?;
    }
    m = Similarity::scaling(i64::from(case.scale), origin)
        .ok()?
        .after(&m)
        .ok()?;
    Similarity::translation(i64::from(case.shift.0), i64::from(case.shift.1))
        .after(&m)
        .ok()
}

/// Tests the edit laws on the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let points: Option<Vec<P2>> = case.vertices.iter().map(|&v| point(case, v)).collect();
    let bulges: Option<Vec<Frac>> = case.bulges.iter().map(|&b| bulge(b)).collect();
    let (Some(points), Some(bulges)) = (points, bulges) else {
        return Ok(());
    };
    let Ok(poly) = Polyline::new(points.clone(), bulges.clone(), None, case.closed) else {
        return Err(fail(case, "a polyline with the right counts is made"));
    };
    let Some(map) = map(case) else {
        // A zero scale, or a shift the crate refuses: no map to test.
        return Ok(());
    };
    // The image of every vertex, by the definition.
    let images: Option<Vec<(i128, i128)>> = points
        .iter()
        .map(|p| {
            let (x, y) = linear(case, (i128::from(p.x.get()), i128::from(p.y.get())))?;
            Some((
                x.checked_add(i128::from(case.shift.0))?,
                y.checked_add(i128::from(case.shift.1))?,
            ))
        })
        .collect();
    let placed = place_polyline(&poly, &map);
    let on_grid: Option<Vec<P2>> = images.as_ref().and_then(|v| {
        v.iter()
            .map(|&(x, y)| P2::new(i64::try_from(x).ok()?, i64::try_from(y).ok()?).ok())
            .collect()
    });
    match (placed, on_grid) {
        (Ok(placed), Some(expect)) => {
            if placed.vertices() != expect.as_slice() {
                return Err(fail(
                    case,
                    "a placed vertex is the image the definition gives",
                ));
            }
            let flipped: Option<Vec<Frac>> = bulges
                .iter()
                .map(|b| {
                    if case.mirror {
                        Frac::new(b.numerator().checked_neg()?, b.denominator()).ok()
                    } else {
                        Some(*b)
                    }
                })
                .collect();
            if flipped.as_deref() != Some(placed.bulges()) {
                return Err(fail(case, "a bulge is kept, and negated by a mirror"));
            }
            lengths(case, &poly, &placed)?;
            inverse(case, &poly, &map, &placed)
        }
        (Err(_), None) => Ok(()),
        _ => Err(fail(
            case,
            "a placement succeeds exactly when every image is on the grid",
        )),
    }
}

/// A length interval is ordered and not negative, and a map that keeps lengths gives a polyline
/// whose interval meets the original's.
fn lengths(case: &Case, poly: &Polyline, placed: &Polyline) -> Result<(), Finding> {
    let Ok(before) = polyline_length(poly) else {
        return Ok(());
    };
    if before.lo() > before.hi() || before.lo() < 0.0 {
        return Err(fail(case, "a length interval is ordered and not negative"));
    }
    if case.scale.unsigned_abs() == 1
        && let Ok(after) = polyline_length(placed)
        && (after.lo() > before.hi() || before.lo() > after.hi())
    {
        return Err(fail(
            case,
            "a map that keeps lengths keeps the length interval",
        ));
    }
    Ok(())
}

/// A map of unit scale is undone by its inverse.
fn inverse(
    case: &Case,
    poly: &Polyline,
    map: &Similarity,
    placed: &Polyline,
) -> Result<(), Finding> {
    if case.scale.unsigned_abs() != 1 {
        return Ok(());
    }
    let Ok(back) = map.inverse() else {
        return Err(fail(case, "a map of unit scale has an inverse"));
    };
    match place_polyline(placed, &back) {
        Ok(round) if round == *poly => Ok(()),
        _ => Err(fail(case, "a unit map is undone by its inverse")),
    }
}

/// Decodes `data` and checks the case.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
