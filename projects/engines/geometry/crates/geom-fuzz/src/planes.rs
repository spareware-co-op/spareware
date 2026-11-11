//! Fuzz target: `geom-poly`'s canonical planes and arrangement predicates, on any bytes, give one
//! entry to one plane and one order to any points, rays or flaps.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into [`PLANES`] planes through grid points on the
//! hard cases of chapter 28.5: small, on coarse sub-grids, one step off them, at the grid's edge.
//! [`check`] holds the planes to the laws below, which hold whatever the planes are.
//! [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, and [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a
//!   canonical form that moves, a plane given two entries or two planes one, a flip that does not
//!   say which way a plane faced; an order that is not antisymmetric or not transitive; a turn that
//!   changes when its sides swap; a point that is not the point it was made as. No law may panic
//!   or return `BudgetExceeded`: every plane here is within budget.
//!
//! Method: chapter 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`),
//! with the laws of an order: antisymmetry and transitivity.
//!
//! Cost: at most [`PLANES`]³ predicates on each case; at most [`MAX_INPUT_BYTES`] bytes read.

use core::cmp::Ordering;

use geom_num::grid::COORD_MAX;
use geom_num::plane::{Plane, Tpi};
use geom_num::{NumError, P3, Sign};
use geom_poly::order::{Line, Ray, along, around, normals_det, same_point, turn};
use geom_poly::plane::{PlaneTable, canonical};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The planes a case holds.
pub const PLANES: usize = 5;

/// The bytes one coordinate reads: a rule and a word.
const VALUE_BYTES: usize = 1 + 8;

/// The most bytes [`decode`] reads: three points of three coordinates for each plane.
pub const MAX_INPUT_BYTES: usize = PLANES * 3 * 3 * VALUE_BYTES;

/// One case: planes in the order made, some of them collinear points and so left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The planes.
    pub planes: Vec<Plane>,
}

/// A coordinate through its rule byte: small, on a sub-grid of 1 mm, one step off it, at the grid's
/// edge, or anything.
fn coordinate(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let any = i64::from_le_bytes(src.word().to_le_bytes()) % (1 << 40);
    let fine = (any % 1024).saturating_mul(6_400);
    match rule % 6 {
        0 => any % 8,
        1 => fine,
        2 => fine.saturating_add(1),
        3 => COORD_MAX.saturating_sub((any % 8).saturating_abs()),
        4 => (any % 8).saturating_abs().saturating_sub(COORD_MAX),
        _ => any,
    }
}

/// A grid point; `None` is out of the range only by a defect in [`coordinate`].
fn point(src: &mut ByteSource<'_>) -> Option<P3> {
    let (x, y, z) = (coordinate(src), coordinate(src), coordinate(src));
    P3::new(x, y, z).ok()
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let mut planes = Vec::with_capacity(PLANES);
    for _ in 0..PLANES {
        let (a, b, c) = (point(&mut src), point(&mut src), point(&mut src));
        if let (Some(a), Some(b), Some(c)) = (a, b, c)
            && let Ok(plane) = Plane::through(a, b, c)
        {
            planes.push(plane);
        }
    }
    let read = data.len().saturating_sub(src.remaining());
    (Case { planes }, read)
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

fn budget(case: &Case) -> impl Fn(NumError) -> Finding + '_ {
    move |e| {
        Finding::new("planes within budget are never refused", || {
            format!("{e:?}: {case:?}")
        })
    }
}

/// The laws of one plane and of a table.
#[allow(
    clippy::many_single_char_names,
    reason = "x, y and z the normal, a and b the entries"
)]
fn table_laws(case: &Case) -> Result<(), Finding> {
    let mut table = PlaneTable::new();
    for plane in &case.planes {
        let made = canonical(plane).map_err(budget(case))?;
        let again = canonical(&made.plane).map_err(budget(case))?;
        if again.plane != made.plane || again.flipped {
            return Err(Finding::new("a canonical form stays put", || {
                format!("{case:?}")
            }));
        }
        let [x, y, z] = plane.n();
        let turned = [x, y, z].map(i128::saturating_neg);
        let opposite = Plane::new(turned, plane.d().saturating_neg()).map_err(budget(case))?;
        let (a, b) = (table.insert(plane), table.insert(&opposite));
        let bad = || Finding::new("a table takes any plane", || format!("{case:?}"));
        let (a, b) = (a.map_err(|_| bad())?, b.map_err(|_| bad())?);
        if a.plane != b.plane || a.flipped == b.flipped {
            return Err(Finding::new(
                "a plane and its opposite are one entry",
                || format!("{case:?}"),
            ));
        }
        if table.plane(a.plane) != Some(made.plane) {
            return Err(Finding::new("an entry gives back its plane", || {
                format!("{case:?}")
            }));
        }
    }
    Ok(())
}

/// The laws of order along the line of the first two planes that meet.
fn along_laws(case: &Case, line: &Line) -> Result<(), Finding> {
    let [p, q] = line.planes();
    let mut crossing = Vec::new();
    for plane in &case.planes {
        if normals_det(&p, &q, plane).map_err(budget(case))? != Sign::Zero {
            crossing.push(plane);
        }
    }
    let order = |a: &Plane, b: &Plane| along(line, a, b).map_err(budget(case));
    for a in &crossing {
        for b in &crossing {
            if order(a, b)? != order(b, a)?.reverse() {
                return Err(Finding::new("an order is antisymmetric", || {
                    format!("{case:?}")
                }));
            }
            let same = same_point(
                &Tpi::new(p, q, **a).map_err(budget(case))?,
                &Tpi::new(p, q, **b).map_err(budget(case))?,
            )
            .map_err(budget(case))?;
            if same != (order(a, b)? == Ordering::Equal) {
                return Err(Finding::new("equal in order is one point", || {
                    format!("{case:?}")
                }));
            }
            for c in &crossing {
                let up = order(a, b)? != Ordering::Greater && order(b, c)? != Ordering::Greater;
                if up && order(a, c)? == Ordering::Greater {
                    return Err(Finding::new("an order is transitive", || {
                        format!("{case:?}")
                    }));
                }
            }
        }
    }
    Ok(())
}

/// The laws of rays about the first plane.
fn ray_laws(case: &Case, face: &Plane) -> Result<(), Finding> {
    let mut rays = Vec::new();
    for plane in &case.planes {
        if Line::new(*face, *plane).is_ok() {
            for reversed in [false, true] {
                rays.push(Ray {
                    with: *plane,
                    reversed,
                });
            }
        }
    }
    for a in &rays {
        for b in &rays {
            let (x, y) = (
                turn(face, a, b).map_err(budget(case))?,
                turn(face, b, a).map_err(budget(case))?,
            );
            let flipped = match x {
                Sign::Positive => Sign::Negative,
                Sign::Negative => Sign::Positive,
                Sign::Zero => Sign::Zero,
            };
            if y != flipped {
                return Err(Finding::new(
                    "a turn changes sign when its rays swap",
                    || format!("{case:?}"),
                ));
            }
            for first in &rays {
                let order = |p: &Ray, q: &Ray| around(face, first, p, q).map_err(budget(case));
                if order(a, b)? != order(b, a)?.reverse() {
                    return Err(Finding::new("an order is antisymmetric", || {
                        format!("{case:?}")
                    }));
                }
                if order(first, a)? == Ordering::Greater {
                    return Err(Finding::new("nothing comes before the first ray", || {
                        format!("{case:?}")
                    }));
                }
            }
        }
    }
    Ok(())
}

/// Holds the case to the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    table_laws(case)?;
    let Some((first, rest)) = case.planes.split_first() else {
        return Ok(());
    };
    ray_laws(case, first)?;
    for other in rest {
        if let Ok(line) = Line::new(*first, *other) {
            return along_laws(case, &line);
        }
    }
    Ok(())
}
