//! Face splitting for flat carriers: each face cut along its In and section edges into the faces it
//! splits into, its loops traced in the plane's picture by exact orientation (11.4 step 5, G244).
//!
//! What it does: [`split`] takes the interferences of an arrangement ([`crate::interfere`]) and,
//! for each face whose carrier is a plane of the table, gathers its pieces: its boundary's split
//! edges (On), other operands' split edges lying inside it (In) and its section edges, a common
//! block once. In the plane's picture (the coordinate the normal is largest in dropped, as
//! [`crate::inside`] does), the pieces leaving each vertex are put in turning order by their
//! tangents, and where two leave along one tangent, by their curvature; the loops are then traced
//! with each piece's left side as the face it bounds. A loop running counter-clockwise, by the sign
//! of its certified area, bounds a split face; one running clockwise is the outside of a connected
//! group of pieces, a hole of the smallest split face that holds it (by the certified ray test),
//! or, held by none, the outside of the face itself. Vertices of other operands inside the face
//! that no piece reaches are given to the split face that holds them.
//!
//! Guarantees:
//!
//! - **The turning order is proven or refused.** Two tangents are ordered by the half-plane each
//!   lies in and the sign of their cross product, exactly in wide integers where both are whole
//!   (a line between grid points, a section's n₁ × n₂), from enclosures otherwise; two along one
//!   tangent by their curvature enclosures. A pair neither orders fails with
//!   [`BuildError::UndecidedOrder`]. No angle is computed, and no tolerance compares two.
//! - **Each loop's sense is proven.** A loop's area is enclosed piece by piece: a straight piece's
//!   exactly from its ends, a circle's or ellipse's arc in closed form with certified sines and
//!   cosines of half-turns; an area that holds zero fails with [`BuildError::UndecidedArea`].
//! - **Holes go to the face that holds them, by proof.** A clockwise loop's point is placed in each
//!   counter-clockwise loop by the certified ray test, and the smallest by area that holds it takes
//!   it; no direction qualifying fails with [`BuildError::Unclassified`].
//! - **What this piece does not split, it says.** A face on a curved carrier, or bounded by a curve
//!   other than a line, circle or ellipse, is listed as waiting, for G244's analytic carriers and
//!   G245's NURBS and procedural ones.
//! - **Deterministic.** Faces in order, pieces in index order, vertices' pieces in turning order,
//!   loops in the order their first pieces come.
//!
//! Method: chapter 11.4, step 5 (`projects/engines/geometry/architecture/11-booleans.md`): each
//! face cut along its In and section edges in its own parameter space, the loops traced by ordering
//! the edges around each vertex, flat faces by exact orientation predicates. The loop walk is the
//! face traversal of a planar graph's rotation system (the next piece round each vertex from the
//! one arriving), as in M. de Berg, O. Cheong, M. van Kreveld and M. Overmars, *Computational
//! Geometry*, 3rd edition, Springer (2008), section 2.2, <https://doi.org/10.1007/978-3-540-77974-2>.
//! The order round a vertex, by tangents and then by curvature where tangents agree, is `SMLib`'s
//! `sm_NextRoundTheVertex` (`SmTrimmingTools::OrderCurvesIntoLoops`), read in solidcore's Rust
//! port; its tangents are compared in doubles within a tolerance, and its outer loops are found by
//! the areas of tessellations, neither of which is taken.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/c440e5f892452a9654aee8f3030de6095fdb1415/crates/solidcore/src/topology/trimming_tools.rs>,
//!   for the order round a vertex and the grouping of outer loops and holes; no code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: per flat face, its pieces sorted round each vertex (O(k log k) comparisons at a vertex of
//! k pieces, each a few wide multiplications or interval products), one walk over the pieces, an
//! area per loop, and a ray test per hole and loop pair; all charged to the meter. No recursion.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_intersect::IntersectError;
use geom_num::interval::Interval;
use geom_num::wide::{I256, I512};
use geom_num::{NumError, P3, Sign};
use geom_run::WorkMeter;
use geom_topo::{Axes, CurveKind, DerivativeBound, Line, ParamCurve};

use crate::carrier::{Bound, CarrierIx, CurveGeom};
use crate::crossing::{Enclosed, Hull};
use crate::inside::{Crossable, DIRECTIONS, FlatFace, ray_parity};
use crate::interfere::{Arrangement, BuildError, Entity, Interfered, committed, nearest_point};

/// A piece of edge that bounds a split face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Piece {
    /// A split edge, by its index in [`Interfered::splits`].
    Split(usize),
    /// A section edge, by its index in [`Interfered::sections`].
    Section(usize),
}

/// A piece in a loop, and which way the loop runs it: forward from its first pave to its last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    /// The piece.
    pub piece: Piece,
    /// Whether the loop runs it from its first pave to its last.
    pub forward: bool,
}

/// One of the faces a face splits into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitFace {
    /// The face it is part of.
    pub face: usize,
    /// Its outer loop, counter-clockwise in the face's picture.
    pub outer: Vec<Use>,
    /// Its holes, each clockwise in the picture.
    pub holes: Vec<Vec<Use>>,
    /// Pieces inside it that bound nothing on their own, such as a section that ends inside the
    /// face: each a group's walk, every piece run both ways.
    pub cuts: Vec<Vec<Use>>,
    /// Vertices of other operands inside it that no piece reaches.
    pub inside: Vec<usize>,
}

/// The faces of an arrangement split.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Splitting {
    /// The split faces, face by face.
    pub faces: Vec<SplitFace>,
    /// The faces this splitting leaves for the splitting of their kinds: on a curved carrier, or
    /// bounded by a curve other than a line, circle or ellipse.
    pub waiting: Vec<usize>,
}

/// A piece's geometry.
enum Shape<'a> {
    /// A stretch of an edge's curve, from one pave's parameter to the next, round the curve's seam
    /// when `wraps`; the whole curve when it has no pave.
    Curve {
        curve: &'a CurveKind<3>,
        span: Option<(Interval, Interval)>,
        wraps: bool,
    },
    /// A section's straight line between its two vertices, along `direction`, n₁ × n₂.
    Straight {
        direction: [I256; 3],
        approx: [Interval; 3],
    },
}

/// A piece with its ends, the vertices it runs between (none for a closed curve with no pave), and
/// for a stretch of the face's own boundary, whether the face lies on its left run forward.
struct Bounding<'a> {
    piece: Piece,
    ends: Option<[usize; 2]>,
    shape: Shape<'a>,
    left: Option<bool>,
}

/// The pieces leaving each vertex, in turning order, with their tangents: half-edge 2i leaves
/// piece i's first end, and 2i + 1 its last.
type Rotations = BTreeMap<usize, Vec<(usize, Dir)>>;

/// A face's loops, each a list of uses, with their areas; `None` for a walk that runs each of its
/// pieces both ways and so bounds nothing.
type Loops = (Vec<Vec<Use>>, Vec<Option<Interval>>);

/// The counter-clockwise loops (as indices into all the loops), ready for the ray test, with every
/// loop's area and group.
type Outers<'r> = (
    &'r [usize],
    &'r [Ring],
    &'r [Option<Interval>],
    &'r [Option<usize>],
);

/// A tangent in the picture, leaving a vertex: whole when it can be, its enclosure, and the
/// curvature's enclosure as it leaves (zero for a straight piece).
#[derive(Clone, Copy, Debug)]
struct Dir {
    exact: Option<[I256; 2]>,
    approx: [Interval; 2],
    curl: Interval,
}

/// The picture's two coordinates of a vector of three.
fn pick<T: Copy>(v: [T; 3], [first, second]: [usize; 2]) -> Option<[T; 2]> {
    Some([*v.get(first)?, *v.get(second)?])
}

/// A double as a whole number in a wide integer, if it is one and small enough.
fn whole(x: f64) -> Option<I256> {
    let rounded = geom_num::math::round_ties_even(x).ok()?.get();
    if rounded != x || x.abs() >= 9.0e15 {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number below 2^53 in size, which an integer holds exactly"
    )]
    Some(I256::from_i128(i128::from(x as i64)))
}

/// Three doubles, each enclosed as itself.
fn exact3(v: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [x, y, z] = v;
    Ok([
        Interval::point(x)?,
        Interval::point(y)?,
        Interval::point(z)?,
    ])
}

/// The enclosure of a box given by its corners.
fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [lx, ly, lz] = lo;
    let [hx, hy, hz] = hi;
    Ok([
        Interval::new(lx, hx)?,
        Interval::new(ly, hy)?,
        Interval::new(lz, hz)?,
    ])
}

/// The half-plane a tangent lies in, turning from the picture's first axis: 0 for the upper half
/// with the first axis's own direction, 1 for the lower, 2 for the first axis's direction when the
/// piece bends below it (it comes last round the vertex); `None` when it cannot be told.
fn half(d: &Dir) -> Option<u8> {
    if let Some([x, y]) = d.exact {
        let (sx, sy) = (x.signum(), y.signum());
        return Some(match (sy, sx) {
            (Sign::Zero, Sign::Positive) if d.curl.hi() < 0.0 => 2,
            (Sign::Positive, _) | (Sign::Zero, Sign::Positive) => 0,
            _ => 1,
        });
    }
    let [x, y] = d.approx;
    let on_axis = y.lo() == 0.0 && y.hi() == 0.0;
    if y.lo() > 0.0 {
        Some(0)
    } else if y.hi() < 0.0 || (on_axis && x.hi() < 0.0) {
        Some(1)
    } else if on_axis && x.lo() > 0.0 {
        Some(if d.curl.hi() < 0.0 { 2 } else { 0 })
    } else {
        None
    }
}

/// The sign of the cross product a × b of two tangents, exactly where both are whole.
fn cross(a: &Dir, b: &Dir) -> Option<Sign> {
    if let (Some([ax, ay]), Some([bx, by])) = (a.exact, b.exact) {
        let wide = |v: &I256| v.resize::<8>();
        let (ax, ay, bx, by) = (wide(&ax)?, wide(&ay)?, wide(&bx)?, wide(&by)?);
        let left: I512 = ax.checked_mul(&by)?;
        let right: I512 = ay.checked_mul(&bx)?;
        return Some(left.checked_sub(&right)?.signum());
    }
    let ([ax, ay], [bx, by]) = (a.approx, b.approx);
    let value = ax.mul(by).ok()?.sub(ay.mul(bx).ok()?).ok()?;
    if value.lo() > 0.0 {
        Some(Sign::Positive)
    } else if value.hi() < 0.0 {
        Some(Sign::Negative)
    } else {
        None
    }
}

/// The turning order of two tangents leaving one vertex, counter-clockwise from the picture's
/// first axis; along one tangent, the one bending less to the left first. `None` when not proven.
fn turning(a: &Dir, b: &Dir) -> Option<Ordering> {
    let (ha, hb) = (half(a)?, half(b)?);
    if ha != hb {
        return Some(ha.cmp(&hb));
    }
    match cross(a, b)? {
        Sign::Positive => Some(Ordering::Less),
        Sign::Negative => Some(Ordering::Greater),
        Sign::Zero => {
            if a.curl.hi() < b.curl.lo() {
                Some(Ordering::Less)
            } else if a.curl.lo() > b.curl.hi() {
                Some(Ordering::Greater)
            } else {
                None
            }
        }
    }
}

/// The section line's direction n₁ × n₂, exactly and enclosed, from its two planes.
fn section_direction(
    arr: &Arrangement,
    curve: CurveGeom,
) -> Result<([I256; 3], [Interval; 3]), BuildError> {
    let CurveGeom::Line([p1, p2]) = curve else {
        return Err(IntersectError::Internal.into());
    };
    let plane = |ix| arr.carriers.plane(ix).ok_or(NumError::Degenerate);
    let (pa, pb) = (plane(p1)?, plane(p2)?);
    let ([a0, a1, a2], [b0, b1, b2]) = (pa.n().map(I256::from_i128), pb.n().map(I256::from_i128));
    let term = |x: &I256, y: &I256, z: &I256, w: &I256| -> Result<I256, NumError> {
        x.checked_mul(y)
            .zip(z.checked_mul(w))
            .and_then(|(l, r)| l.checked_sub(&r))
            .ok_or(NumError::BudgetExceeded)
    };
    let exact = [
        term(&a1, &b2, &a2, &b1)?,
        term(&a2, &b0, &a0, &b2)?,
        term(&a0, &b1, &a1, &b0)?,
    ];
    let (ea, eb) = (Enclosed::of(&pa)?, Enclosed::of(&pb)?);
    let ([c0, c1, c2], [d0, d1, d2]) = (ea.n, eb.n);
    let approx = [
        c1.mul(d2)?.sub(c2.mul(d1)?)?,
        c2.mul(d0)?.sub(c0.mul(d2)?)?,
        c0.mul(d1)?.sub(c1.mul(d0)?)?,
    ];
    Ok((exact, approx))
}

/// The pieces of flat face `f`: its boundary's split edges, the In split edges and its sections,
/// one split edge for each common block.
fn pieces_of<'a>(
    arr: &'a Arrangement,
    out: &Interfered,
    f: usize,
    senses: &BTreeMap<usize, bool>,
) -> Result<Vec<Bounding<'a>>, BuildError> {
    let info = out
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    // A split edge in a common block stands for the block's member on the face's own boundary, if
    // one is, and otherwise for its first member.
    let mut first_of: BTreeMap<usize, usize> = BTreeMap::new();
    for block in &out.common {
        let own = block
            .members
            .iter()
            .find(|m| info.on_blocks.binary_search(m).is_ok());
        if let Some(&lead) = own.or_else(|| block.members.first()) {
            for &m in &block.members {
                first_of.insert(m, lead);
            }
        }
    }
    let mut splits: Vec<usize> = info
        .on_blocks
        .iter()
        .chain(&info.in_blocks)
        .map(|k| first_of.get(k).copied().unwrap_or(*k))
        .collect();
    splits.sort_unstable();
    splits.dedup();
    let mut out_pieces = Vec::with_capacity(splits.len().saturating_add(info.sections.len()));
    for k in splits {
        let piece = out
            .splits
            .get(k)
            .ok_or(BuildError::Missing(Entity::Edge(k)))?;
        let edge = arr
            .edges
            .get(piece.source)
            .ok_or(BuildError::Missing(Entity::Edge(piece.source)))?;
        let curve = arr
            .carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(piece.source)))?;
        let (ends, span) = match (piece.block.from, piece.block.to) {
            (Some(from), Some(to)) => (Some([from.vertex, to.vertex]), Some((from.t, to.t))),
            _ => (None, None),
        };
        out_pieces.push(Bounding {
            piece: Piece::Split(k),
            ends,
            shape: Shape::Curve {
                curve,
                span,
                wraps: piece.block.wraps,
            },
            left: senses.get(&piece.source).copied(),
        });
    }
    for &s in &info.sections {
        let section = out
            .sections
            .get(s)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        let (direction, approx) = section_direction(arr, section.curve)?;
        out_pieces.push(Bounding {
            piece: Piece::Section(s),
            ends: Some([section.from.vertex, section.to.vertex]),
            shape: Shape::Straight { direction, approx },
            left: None,
        });
    }
    Ok(out_pieces)
}

/// The tangent of `bounding` leaving its first end (`at_start`) or its last, in the picture.
/// `None` for a curve kind this piece does not order.
fn leaving(
    bounding: &Bounding<'_>,
    at_start: bool,
    keep: [usize; 2],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Dir>, BuildError> {
    let zero = Interval::point(0.0)?;
    let flip = |v: Interval| if at_start { v } else { v.neg() };
    match &bounding.shape {
        Shape::Straight { direction, approx } => {
            let (Some([x, y]), Some([ax, ay])) = (pick(*direction, keep), pick(*approx, keep))
            else {
                return Err(IntersectError::Internal.into());
            };
            let exact = if at_start {
                [x, y]
            } else {
                [
                    x.checked_neg().ok_or(NumError::BudgetExceeded)?,
                    y.checked_neg().ok_or(NumError::BudgetExceeded)?,
                ]
            };
            Ok(Some(Dir {
                exact: Some(exact),
                approx: [flip(ax), flip(ay)],
                curl: zero,
            }))
        }
        Shape::Curve { curve, span, .. } => {
            let Some((from, to)) = span else {
                return Err(IntersectError::Internal.into());
            };
            let t = if at_start { *from } else { *to };
            if let CurveKind::Line(line) = curve {
                let Some([dx, dy]) = pick(line.direction(), keep) else {
                    return Err(IntersectError::Internal.into());
                };
                let exact = match (whole(dx), whole(dy)) {
                    (Some(x), Some(y)) if at_start => Some([x, y]),
                    (Some(x), Some(y)) => Some([
                        x.checked_neg().ok_or(NumError::BudgetExceeded)?,
                        y.checked_neg().ok_or(NumError::BudgetExceeded)?,
                    ]),
                    _ => None,
                };
                return Ok(Some(Dir {
                    exact,
                    approx: [flip(Interval::point(dx)?), flip(Interval::point(dy)?)],
                    curl: zero,
                }));
            }
            let span = (t.lo(), t.hi());
            let (DerivativeBound::Bounded(d1), DerivativeBound::Bounded(d2)) = (
                curve
                    .derivative_bound(span, 1, meter)
                    .map_err(IntersectError::from)?,
                curve
                    .derivative_bound(span, 2, meter)
                    .map_err(IntersectError::from)?,
            ) else {
                return Ok(None);
            };
            let (Some([x1, y1]), Some([x2, y2])) = (
                pick(boxed(d1.lo, d1.hi)?, keep),
                pick(boxed(d2.lo, d2.hi)?, keep),
            ) else {
                return Err(IntersectError::Internal.into());
            };
            // The signed curvature, (x′y″ − y′x″) / |v|³; running the curve backwards turns its
            // tangent round and its curvature's sign with it.
            let speed = Interval::norm(&[x1, y1])?;
            let curl = x1
                .mul(y2)?
                .sub(y1.mul(x2)?)?
                .div(speed.mul(speed)?.mul(speed)?)?;
            Ok(Some(Dir {
                exact: None,
                approx: [flip(x1), flip(y1)],
                curl: flip(curl),
            }))
        }
    }
}

/// π, enclosed.
fn pi() -> Result<Interval, NumError> {
    Interval::new(core::f64::consts::PI, core::f64::consts::PI.next_up())
}

/// ∫ x dy along a conic arc c + cos πt·a + sin πt·b in the picture, from `t0` to `t1`.
fn conic_area(
    [c, a, b]: [[Interval; 2]; 3],
    (t0, t1): (Interval, Interval),
) -> Result<Interval, NumError> {
    let ([cx, cy], [ax, ay], [bx, by]) = (c, a, b);
    let quarter = Interval::point(0.25)?;
    let half = Interval::point(0.5)?;
    // F(t) = cx·y(t) + (bx·by − ax·ay)·(−cos 2πt / 4) + ax·by·(πt/2 + sin 2πt / 4)
    //      − bx·ay·(πt/2 − sin 2πt / 4), whose difference is the integral.
    let at = |t: Interval| -> Result<Interval, NumError> {
        let y = cy.add(t.cospi()?.mul(ay)?)?.add(t.sinpi()?.mul(by)?)?;
        let double = t.mul(Interval::point(2.0)?)?;
        let (cos2, sin2) = (double.cospi()?, double.sinpi()?);
        let sweep = pi()?.mul(t)?.mul(half)?;
        let cross = bx
            .mul(by)?
            .sub(ax.mul(ay)?)?
            .mul(cos2.mul(quarter)?.neg())?;
        let up = ax.mul(by)?.mul(sweep.add(sin2.mul(quarter)?)?)?;
        let down = bx.mul(ay)?.mul(sweep.sub(sin2.mul(quarter)?)?)?;
        cx.mul(y)?.add(cross)?.add(up)?.sub(down)
    };
    at(t1)?.sub(at(t0)?)
}

/// The picture's centre and two axis vectors of a circle or ellipse, enclosed; `None` for another
/// kind.
fn conic_of(
    curve: &CurveKind<3>,
    keep: [usize; 2],
) -> Result<Option<[[Interval; 2]; 3]>, BuildError> {
    let scaled = |v: [Interval; 3], k: f64| -> Result<[Interval; 3], NumError> {
        let k = Interval::point(k)?;
        let [x, y, z] = v;
        Ok([x.mul(k)?, y.mul(k)?, z.mul(k)?])
    };
    let (centre, a, b) = match curve {
        CurveKind::Circle(circle) => {
            let (_, [x, y, _]) = circle
                .frame()
                .unit_vectors()
                .map_err(IntersectError::from)?;
            let r = circle.radius();
            (exact3(circle.center())?, scaled(x, r)?, scaled(y, r)?)
        }
        CurveKind::Ellipse(ellipse) => {
            let (a, b) = match ellipse.axes() {
                Axes::Frame { frame, a, b } => {
                    let (_, [x, y, _]) = frame.unit_vectors().map_err(IntersectError::from)?;
                    (scaled(x, a)?, scaled(y, b)?)
                }
                Axes::Vectors { u, v } => (exact3(u)?, exact3(v)?),
            };
            (exact3(ellipse.center())?, a, b)
        }
        _ => return Ok(None),
    };
    let (Some(c), Some(a), Some(b)) = (pick(centre, keep), pick(a, keep), pick(b, keep)) else {
        return Err(IntersectError::Internal.into());
    };
    Ok(Some([c, a, b]))
}

/// (x₀ + x₁)/2 · (y₁ − y₀): ∫ x dy along a straight piece from `p0` to `p1` in the picture.
fn chord_area(p0: [Interval; 2], p1: [Interval; 2]) -> Result<Interval, NumError> {
    let ([x0, y0], [x1, y1]) = (p0, p1);
    x0.add(x1)?.mul(Interval::point(0.5)?)?.mul(y1.sub(y0)?)
}

/// A grid point's coordinates in the picture, enclosed.
fn picture_point(p: P3, keep: [usize; 2]) -> Result<[Interval; 2], BuildError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is at most 2^40 in size, which a double holds exactly"
    )]
    let v = [p.x.get() as f64, p.y.get() as f64, p.z.get() as f64];
    pick(exact3(v)?, keep).ok_or_else(|| IntersectError::Internal.into())
}

/// ∫ x dy along `bounding` run `forward` or back, in the picture; `None` for a curve kind this
/// piece does not measure.
fn area_of(
    arr: &Arrangement,
    out: &Interfered,
    bounding: &Bounding<'_>,
    forward: bool,
    keep: [usize; 2],
) -> Result<Option<Interval>, BuildError> {
    let along = match &bounding.shape {
        Shape::Straight { .. } => {
            let Some([u, v]) = bounding.ends else {
                return Err(IntersectError::Internal.into());
            };
            let (pu, _) = committed(&arr.carriers, &out.vertices, u)?;
            let (pv, _) = committed(&arr.carriers, &out.vertices, v)?;
            chord_area(picture_point(pu, keep)?, picture_point(pv, keep)?)?
        }
        Shape::Curve { .. } => return curve_area(bounding, forward, keep),
    };
    Ok(Some(if forward { along } else { along.neg() }))
}

/// ∫ x dy along a curve piece run `forward` or back, in the picture; `None` for a curve kind this
/// piece does not measure.
fn curve_area(
    bounding: &Bounding<'_>,
    forward: bool,
    keep: [usize; 2],
) -> Result<Option<Interval>, BuildError> {
    let along = match &bounding.shape {
        Shape::Straight { .. } => return Err(IntersectError::Internal.into()),
        Shape::Curve { curve, span, wraps } => {
            let (a, b) = curve.domain();
            let (lo, hi) = match span {
                Some((from, to)) => (*from, *to),
                None => (Interval::point(a)?, Interval::point(b)?),
            };
            if let CurveKind::Line(line) = curve {
                let (o, d) = (exact3(line.origin())?, exact3(line.direction())?);
                let at = |t: Interval| -> Result<[Interval; 3], NumError> {
                    let [o0, o1, o2] = o;
                    let [d0, d1, d2] = d;
                    Ok([
                        o0.add(t.mul(d0)?)?,
                        o1.add(t.mul(d1)?)?,
                        o2.add(t.mul(d2)?)?,
                    ])
                };
                let (Some(p0), Some(p1)) = (pick(at(lo)?, keep), pick(at(hi)?, keep)) else {
                    return Err(IntersectError::Internal.into());
                };
                chord_area(p0, p1)?
            } else {
                let Some(conic) = conic_of(curve, keep)? else {
                    return Ok(None);
                };
                if *wraps {
                    conic_area(conic, (lo, Interval::point(b)?))?
                        .add(conic_area(conic, (Interval::point(a)?, hi))?)?
                } else {
                    conic_area(conic, (lo, hi))?
                }
            }
        }
    };
    Ok(Some(if forward { along } else { along.neg() }))
}

/// For each edge of flat face `f`'s loops, whether the face lies on its left in the picture when it
/// is run forward: each loop is run edge after edge, its sense read from its certified area, the
/// outer loop (the first) counter-clockwise round the face and the others clockwise round its
/// holes. `None` for an edge kind this piece does not measure.
fn senses(
    arr: &Arrangement,
    (f, keep): (usize, [usize; 2]),
) -> Result<Option<BTreeMap<usize, bool>>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let mut out = BTreeMap::new();
    for (k, ring) in face.loops.iter().enumerate() {
        // Which way the loop runs each edge: on from the end it shares with the edge before.
        let mut runs: Vec<(usize, bool)> = Vec::with_capacity(ring.len());
        let mut at: Option<usize> = None;
        for (i, &e) in ring.iter().enumerate() {
            let edge = arr
                .edges
                .get(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let forward = match (edge.ends, at) {
                (None, _) => true,
                (Some([s, _]), Some(v)) => s == v,
                (Some([_, t]), None) => {
                    // The first edge: forward when its end is shared with the next edge.
                    let next = ring
                        .get(i.saturating_add(1))
                        .or(ring.first())
                        .and_then(|n| arr.edges.get(*n))
                        .and_then(|n| n.ends);
                    next.is_none_or(|[a, b]| a == t || b == t)
                }
            };
            at = edge.ends.map(|[s, t]| if forward { t } else { s });
            runs.push((e, forward));
        }
        let mut total = Interval::point(0.0)?;
        for &(e, forward) in &runs {
            let edge = arr
                .edges
                .get(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let curve = arr
                .carriers
                .curve(edge.curve)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let (a, b) = curve.domain();
            let whole = Bounding {
                piece: Piece::Split(0),
                ends: None,
                shape: Shape::Curve {
                    curve,
                    span: Some((Interval::point(a)?, Interval::point(b)?)),
                    wraps: false,
                },
                left: None,
            };
            let Some(part) = curve_area(&whole, forward, keep)? else {
                return Ok(None);
            };
            total = total.add(part)?;
        }
        if total.lo() <= 0.0 && total.hi() >= 0.0 {
            return Err(BuildError::UndecidedArea { face: f });
        }
        // The outer loop runs counter-clockwise with the face on its left; a hole clockwise.
        let left_turning = total.lo() > 0.0;
        let wanted = k == 0;
        for (e, forward) in runs {
            out.insert(e, forward == (left_turning == wanted));
        }
    }
    Ok(Some(out))
}

/// The pieces leaving each vertex, in turning order: half-edge 2i leaves piece i's first end, and
/// 2i + 1 its last. `None` when a tangent cannot be had.
fn rotations(
    pieces: &[Bounding<'_>],
    (f, keep): (usize, [usize; 2]),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Rotations>, BuildError> {
    let mut at: Rotations = BTreeMap::new();
    for (i, bounding) in pieces.iter().enumerate() {
        let Some([u, v]) = bounding.ends else {
            continue;
        };
        let twice = i.checked_mul(2).ok_or(NumError::BudgetExceeded)?;
        for (vertex, at_start, half_edge) in [(u, true, twice), (v, false, twice.saturating_add(1))]
        {
            meter.charge(1).map_err(IntersectError::from)?;
            let Some(dir) = leaving(bounding, at_start, keep, meter)? else {
                return Ok(None);
            };
            at.entry(vertex).or_default().push((half_edge, dir));
        }
    }
    // Insertion sort by a proven order: a vertex has few pieces.
    for (vertex, list) in &mut at {
        let mut sorted: Vec<(usize, Dir)> = Vec::with_capacity(list.len());
        for item in list.drain(..) {
            let mut place = sorted.len();
            for (k, other) in sorted.iter().enumerate() {
                match turning(&item.1, &other.1) {
                    Some(Ordering::Less) => {
                        place = k;
                        break;
                    }
                    Some(_) => {}
                    None => {
                        return Err(BuildError::UndecidedOrder {
                            face: f,
                            vertex: *vertex,
                        });
                    }
                }
            }
            sorted.insert(place, item);
        }
        *list = sorted;
    }
    Ok(Some(at))
}

/// The loops of the pieces, each a list of uses, traced with each piece's left side as the face
/// it bounds: from half-edge h into vertex v, the next is the one before h's twin round v.
fn trace(pieces: &[Bounding<'_>], at: &Rotations) -> Result<Vec<Vec<Use>>, BuildError> {
    let count = pieces
        .len()
        .checked_mul(2)
        .ok_or(NumError::BudgetExceeded)?;
    // Where each half-edge stands round the vertex it leaves.
    let mut origin: Vec<Option<(usize, usize)>> = alloc::vec![None; count];
    for (&vertex, list) in at {
        for (k, (h, _)) in list.iter().enumerate() {
            if let Some(slot) = origin.get_mut(*h) {
                *slot = Some((vertex, k));
            }
        }
    }
    let mut seen = alloc::vec![false; count];
    let mut loops = Vec::new();
    for (i, bounding) in pieces.iter().enumerate() {
        if bounding.ends.is_none() {
            loops.push(alloc::vec![Use {
                piece: bounding.piece,
                forward: true,
            }]);
            continue;
        }
        for start in [i.saturating_mul(2), i.saturating_mul(2).saturating_add(1)] {
            if seen.get(start).copied().unwrap_or(true) {
                continue;
            }
            let mut ring = Vec::new();
            let mut h = start;
            // A loop uses each half-edge once, so the walk ends within `count` steps.
            for _ in 0..count {
                let Some(slot) = seen.get_mut(h) else {
                    return Err(IntersectError::Internal.into());
                };
                if *slot {
                    break;
                }
                *slot = true;
                let p = pieces.get(h / 2).ok_or(IntersectError::Internal)?;
                ring.push(Use {
                    piece: p.piece,
                    forward: h % 2 == 0,
                });
                let twin = h ^ 1;
                let (vertex, k) = origin
                    .get(twin)
                    .copied()
                    .flatten()
                    .ok_or(IntersectError::Internal)?;
                let list = at.get(&vertex).ok_or(IntersectError::Internal)?;
                let before = k.checked_sub(1).unwrap_or(list.len().saturating_sub(1));
                h = list.get(before).ok_or(IntersectError::Internal)?.0;
            }
            loops.push(ring);
        }
    }
    Ok(loops)
}

/// Where a side of a loop gets its curve.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The curve of the piece at this index.
    Piece(usize),
    /// The chord at this index of the ring's own.
    Chord(usize),
}

/// A loop made ready for the ray test: its sides, each a curve over a span of its parameter,
/// and the boxes round its joins.
struct Ring {
    chords: Vec<CurveKind<3>>,
    sides: Vec<(Source, (f64, f64), Hull)>,
    joins: Vec<[Interval; 3]>,
}

/// The box round two enclosed points.
fn hull_of(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let span = |x: Interval, y: Interval| {
        Interval::new(
            if x.lo() < y.lo() { x.lo() } else { y.lo() },
            if x.hi() > y.hi() { x.hi() } else { y.hi() },
        )
    };
    Ok([span(a0, b0)?, span(a1, b1)?, span(a2, b2)?])
}

/// The enclosure of a curve's point over the parameter enclosure `t`.
fn point_at(
    curve: &CurveKind<3>,
    t: Interval,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], BuildError> {
    let at = curve
        .bound((t.lo(), t.hi()), meter)
        .map_err(IntersectError::from)?;
    Ok(boxed(at.lo, at.hi)?)
}

/// A grid point within `bound`, as a box.
fn ball(point: P3, bound: Bound) -> Result<[Interval; 3], NumError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a coordinate is at most 2^40 and a bound a small count, which doubles hold exactly"
    )]
    let (reach, at) = (
        bound.steps() as f64,
        [
            point.x.get() as f64,
            point.y.get() as f64,
            point.z.get() as f64,
        ],
    );
    let [cx, cy, cz] = at;
    Ok([
        Interval::new(cx - reach, cx + reach)?,
        Interval::new(cy - reach, cy + reach)?,
        Interval::new(cz - reach, cz + reach)?,
    ])
}

/// The straight chord of a section between its two vertices, as a line of the curve kinds.
fn chord(
    arr: &Arrangement,
    out: &Interfered,
    [u, v]: [usize; 2],
) -> Result<CurveKind<3>, BuildError> {
    let (pu, _) = committed(&arr.carriers, &out.vertices, u)?;
    let (pv, _) = committed(&arr.carriers, &out.vertices, v)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "coordinates are at most 2^40 in size, and their differences 2^41"
    )]
    let (o, e) = (
        [pu.x.get() as f64, pu.y.get() as f64, pu.z.get() as f64],
        [pv.x.get() as f64, pv.y.get() as f64, pv.z.get() as f64],
    );
    let [ox, oy, oz] = o;
    let [ex, ey, ez] = e;
    let line =
        Line::new(o, [ex - ox, ey - oy, ez - oz], (0.0, 1.0)).map_err(IntersectError::from)?;
    Ok(CurveKind::Line(line))
}

/// The loop `uses` of `pieces` made ready for the ray test.
fn ring(
    arr: &Arrangement,
    out: &Interfered,
    (pieces, index): (&[Bounding<'_>], &BTreeMap<Piece, usize>),
    uses: &[Use],
    meter: &mut WorkMeter<'_>,
) -> Result<Ring, BuildError> {
    let mut made = Ring {
        chords: Vec::new(),
        sides: Vec::new(),
        joins: Vec::new(),
    };
    let mut at_vertex: BTreeMap<usize, [Interval; 3]> = BTreeMap::new();
    let mut join = |v: usize, point: [Interval; 3]| -> Result<(), NumError> {
        let joined = match at_vertex.get(&v) {
            Some(&so_far) => hull_of(so_far, point)?,
            None => point,
        };
        at_vertex.insert(v, joined);
        Ok(())
    };
    for one in uses {
        let i = *index.get(&one.piece).ok_or(IntersectError::Internal)?;
        let bounding = pieces.get(i).ok_or(IntersectError::Internal)?;
        if let Some([u, v]) = bounding.ends {
            for w in [u, v] {
                let (p, bound) = committed(&arr.carriers, &out.vertices, w)?;
                join(w, ball(p, bound)?)?;
            }
        }
        match &bounding.shape {
            Shape::Straight { .. } => {
                let ends = bounding.ends.ok_or(IntersectError::Internal)?;
                let kind = chord(arr, out, ends)?;
                let hull = Hull::of(&kind, meter)?;
                made.sides
                    .push((Source::Chord(made.chords.len()), (0.0, 1.0), hull));
                made.chords.push(kind);
            }
            Shape::Curve { curve, span, wraps } => {
                let (a, b) = curve.domain();
                let hull = Hull::of(curve, meter)?;
                if let (Some((from, to)), Some([u, v])) = (span, bounding.ends) {
                    join(u, point_at(curve, *from, meter)?)?;
                    join(v, point_at(curve, *to, meter)?)?;
                    if *wraps {
                        made.sides
                            .push((Source::Piece(i), (from.lo(), b), hull.clone()));
                        made.sides.push((Source::Piece(i), (a, to.hi()), hull));
                    } else {
                        made.sides
                            .push((Source::Piece(i), (from.lo(), to.hi()), hull));
                    }
                } else {
                    // A closed curve with no pave: its seam is a join of its own.
                    let seam = hull_of(
                        point_at(curve, Interval::point(a)?, meter)?,
                        point_at(curve, Interval::point(b)?, meter)?,
                    )?;
                    made.joins.push(seam);
                    made.sides.push((Source::Piece(i), (a, b), hull));
                }
            }
        }
    }
    made.joins.extend(at_vertex.into_values());
    Ok(made)
}

/// Whether the grid point `point` lies inside `ring`, by the first ray that qualifies.
fn holds(
    ring: &Ring,
    pieces: &[Bounding<'_>],
    (f, keep): (usize, [usize; 2]),
    point: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, BuildError> {
    let mut sides = Vec::with_capacity(ring.sides.len());
    for (source, span, hull) in &ring.sides {
        let curve = match source {
            Source::Piece(i) => match pieces.get(*i).map(|b| &b.shape) {
                Some(Shape::Curve { curve, .. }) => *curve,
                _ => return Err(IntersectError::Internal.into()),
            },
            Source::Chord(k) => ring.chords.get(*k).ok_or(IntersectError::Internal)?,
        };
        sides.push(Crossable {
            curve,
            span: *span,
            hull,
        });
    }
    for direction in DIRECTIONS {
        if let Some(inside) = ray_parity(keep, (&sides, &ring.joins), point, direction, meter)? {
            return Ok(inside);
        }
    }
    Err(BuildError::Unclassified { face: f, at: point })
}

/// A grid point on a loop: its first piece's first end, or for a closed curve with no pave the
/// grid point nearest its middle.
fn point_on(
    arr: &Arrangement,
    out: &Interfered,
    bounding: &Bounding<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<P3, BuildError> {
    if let Some([u, _]) = bounding.ends {
        return Ok(committed(&arr.carriers, &out.vertices, u)?.0);
    }
    let Shape::Curve { curve, .. } = &bounding.shape else {
        return Err(IntersectError::Internal.into());
    };
    let (start, end) = curve.domain();
    let middle = point_at(curve, Interval::point(start / 2.0 + end / 2.0)?, meter)?;
    Ok(nearest_point(middle.map(|c| c.lo() / 2.0 + c.hi() / 2.0))?)
}

/// The groups of pieces joined at vertices: each piece's group, named by the lowest piece in it.
fn groups(pieces: &[Bounding<'_>]) -> Vec<usize> {
    let mut lead: Vec<usize> = (0..pieces.len()).collect();
    let find = |lead: &[usize], start: usize| -> usize {
        let mut i = start;
        // Each step moves to a lower index, so the walk ends within the pieces' count.
        for _ in 0..lead.len() {
            let up = lead.get(i).copied().unwrap_or(i);
            if up == i {
                break;
            }
            i = up;
        }
        i
    };
    let mut first_at: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, bounding) in pieces.iter().enumerate() {
        for v in bounding.ends.into_iter().flatten() {
            if let Some(&j) = first_at.get(&v) {
                let (a, b) = (find(&lead, i), find(&lead, j));
                let (low, high) = (a.min(b), a.max(b));
                if let Some(slot) = lead.get_mut(high) {
                    *slot = low;
                }
            } else {
                first_at.insert(v, i);
            }
        }
    }
    (0..pieces.len()).map(|i| find(&lead, i)).collect()
}

/// The loops of flat face `f`'s pieces with their areas, or `None` when this piece leaves the
/// face for another.
fn loops_of(
    arr: &Arrangement,
    out: &Interfered,
    (f, keep): (usize, [usize; 2]),
    (pieces, index): (&[Bounding<'_>], &BTreeMap<Piece, usize>),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Loops>, BuildError> {
    let Some(at) = rotations(pieces, (f, keep), meter)? else {
        return Ok(None);
    };
    let mut loops = trace(pieces, &at)?;
    // A closed curve with no pave bounds both ways: its inside and its outside.
    let lone: Vec<Use> = loops
        .iter()
        .filter(|l| l.len() == 1)
        .filter_map(|l| l.first().copied())
        .filter(|u| {
            index
                .get(&u.piece)
                .and_then(|i| pieces.get(*i))
                .is_some_and(|b| b.ends.is_none())
        })
        .collect();
    for one in lone {
        loops.push(alloc::vec![Use {
            piece: one.piece,
            forward: false,
        }]);
    }
    let mut areas = Vec::with_capacity(loops.len());
    for uses in &loops {
        // A walk that runs each of its pieces both ways bounds nothing: a cut inside a face.
        let both = uses.iter().all(|one| {
            uses.iter()
                .any(|other| other.piece == one.piece && other.forward != one.forward)
        });
        if both {
            areas.push(None);
            continue;
        }
        let mut total = Interval::point(0.0)?;
        for one in uses {
            let i = *index.get(&one.piece).ok_or(IntersectError::Internal)?;
            let bounding = pieces.get(i).ok_or(IntersectError::Internal)?;
            let Some(part) = area_of(arr, out, bounding, one.forward, keep)? else {
                return Ok(None);
            };
            total = total.add(part)?;
        }
        if total.lo() <= 0.0 && total.hi() >= 0.0 {
            return Err(BuildError::UndecidedArea { face: f });
        }
        areas.push(Some(total));
    }
    Ok(Some((loops, areas)))
}

/// The faces flat face `f` splits into, or `None` when this piece leaves it for another.
fn split_face(
    arr: &Arrangement,
    out: &Interfered,
    f: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<SplitFace>>, BuildError> {
    let Some(flat) = FlatFace::new(arr, f, meter)? else {
        return Ok(None);
    };
    let keep = flat.keep();
    let Some(senses) = senses(arr, (f, keep))? else {
        return Ok(None);
    };
    let pieces = pieces_of(arr, out, f, &senses)?;
    let index: BTreeMap<Piece, usize> = pieces
        .iter()
        .enumerate()
        .map(|(i, b)| (b.piece, i))
        .collect();
    let Some((loops, areas)) = loops_of(arr, out, (f, keep), (&pieces, &index), meter)? else {
        return Ok(None);
    };
    // A loop that runs a stretch of the face's own boundary with the face on its right bounds
    // what lies outside the face, or inside one of its holes.
    let mut kept: Loops = (Vec::new(), Vec::new());
    for (uses, area) in loops.into_iter().zip(areas) {
        let outside = uses.iter().any(|one| {
            index
                .get(&one.piece)
                .and_then(|i| pieces.get(*i))
                .and_then(|b| b.left)
                .is_some_and(|left| left != one.forward)
        });
        if !outside {
            kept.0.push(uses);
            kept.1.push(area);
        }
    }
    let (loops, areas) = kept;
    assemble(
        arr,
        out,
        (f, keep),
        (&pieces, &index),
        (loops, areas),
        meter,
    )
    .map(Some)
}

/// The smallest of `outer` (loops of `loops`, ready as `rings`) holding `point`, of a group other
/// than `own`: its index in `outer`.
fn smallest(
    pieces: &[Bounding<'_>],
    (f, keep): (usize, [usize; 2]),
    (outer, rings, areas, groups_of): Outers<'_>,
    (point, own): (P3, Option<usize>),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<usize>, BuildError> {
    let mut best: Option<(usize, f64)> = None;
    for (slot, (&k, ring)) in outer.iter().zip(rings).enumerate() {
        if own.is_some() && groups_of.get(k).copied().flatten() == own {
            continue;
        }
        let size = areas
            .get(k)
            .copied()
            .flatten()
            .ok_or(IntersectError::Internal)?
            .hi();
        if best.is_some_and(|(_, b)| b <= size) {
            continue;
        }
        if holds(ring, pieces, (f, keep), point, meter)? {
            best = Some((slot, size));
        }
    }
    Ok(best.map(|(slot, _)| slot))
}

/// The split faces: each counter-clockwise loop with the clockwise loops of other groups and the
/// lone vertices it is the smallest to hold.
fn assemble(
    arr: &Arrangement,
    out: &Interfered,
    (f, keep): (usize, [usize; 2]),
    (pieces, index): (&[Bounding<'_>], &BTreeMap<Piece, usize>),
    (loops, areas): Loops,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<SplitFace>, BuildError> {
    let group = groups(pieces);
    let groups_of: Vec<Option<usize>> = loops
        .iter()
        .map(|uses| {
            let i = *index.get(&uses.first()?.piece)?;
            group.get(i).copied()
        })
        .collect();
    let outer: Vec<usize> = (0..loops.len())
        .filter(|k| {
            areas
                .get(*k)
                .copied()
                .flatten()
                .is_some_and(|a| a.lo() > 0.0)
        })
        .collect();
    let mut rings = Vec::with_capacity(outer.len());
    let mut faces: Vec<SplitFace> = Vec::with_capacity(outer.len());
    for &k in &outer {
        let uses = loops.get(k).ok_or(IntersectError::Internal)?;
        rings.push(ring(arr, out, (pieces, index), uses, meter)?);
        faces.push(SplitFace {
            face: f,
            outer: uses.clone(),
            holes: Vec::new(),
            cuts: Vec::new(),
            inside: Vec::new(),
        });
    }
    let found = (
        outer.as_slice(),
        rings.as_slice(),
        areas.as_slice(),
        groups_of.as_slice(),
    );
    for (k, uses) in loops.iter().enumerate() {
        let area = areas.get(k).copied().ok_or(IntersectError::Internal)?;
        if area.is_some_and(|a| a.lo() > 0.0) {
            continue;
        }
        let first = uses.first().ok_or(IntersectError::Internal)?;
        let bounding = pieces
            .get(*index.get(&first.piece).ok_or(IntersectError::Internal)?)
            .ok_or(IntersectError::Internal)?;
        let point = point_on(arr, out, bounding, meter)?;
        let own = groups_of.get(k).copied().flatten();
        // Held by none: the outside of the face itself.
        if let Some(slot) = smallest(pieces, (f, keep), found, (point, own), meter)? {
            let face = faces.get_mut(slot).ok_or(IntersectError::Internal)?;
            if area.is_some() {
                face.holes.push(uses.clone());
            } else {
                face.cuts.push(uses.clone());
            }
        }
    }
    let ends: Vec<usize> = pieces
        .iter()
        .flat_map(|b| b.ends.into_iter().flatten())
        .collect();
    let info = out
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    for &v in &info.in_vertices {
        if ends.contains(&v) {
            continue;
        }
        let (point, _) = committed(&arr.carriers, &out.vertices, v)?;
        if let Some(slot) = smallest(pieces, (f, keep), found, (point, None), meter)? {
            faces
                .get_mut(slot)
                .ok_or(IntersectError::Internal)?
                .inside
                .push(v);
        }
    }
    Ok(faces)
}

/// The faces of `arr`'s flat faces split along what `out` found on them (module documentation).
///
/// # Errors
///
/// [`BuildError::UndecidedOrder`] for two pieces at a vertex whose order is not proven;
/// [`BuildError::UndecidedArea`] for a loop whose sense is not proven;
/// [`BuildError::Unclassified`] for a hole no ray could place; a stopped meter.
pub fn split(
    arr: &Arrangement,
    out: &Interfered,
    meter: &mut WorkMeter<'_>,
) -> Result<Splitting, BuildError> {
    let mut made = Splitting::default();
    for f in 0..arr.faces.len() {
        match arr.faces.get(f).map(|face| face.surface) {
            Some(CarrierIx::Plane(_)) => match split_face(arr, out, f, meter)? {
                Some(faces) => made.faces.extend(faces),
                None => made.waiting.push(f),
            },
            _ => made.waiting.push(f),
        }
    }
    Ok(made)
}
