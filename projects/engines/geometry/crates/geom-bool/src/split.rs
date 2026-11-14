//! Face splitting for flat and analytic carriers: each face cut along its In and section edges into
//! the faces it splits into, its loops traced in the face's picture by exact orientation (11.4
//! step 5, G244).
//!
//! What it does: [`split`] takes the interferences of an arrangement ([`crate::interfere`]) and,
//! for each face whose carrier is a plane of the table, a cylinder, one nappe of a cone, a sphere
//! or a torus whose axis lies along a coordinate, gathers its pieces: its boundary's split edges
//! (On), other operands' split edges lying inside it (In) and its section edges, a common block
//! once. In the face's picture (a plane's with the coordinate its normal is largest in dropped, as
//! [`crate::inside`] does; a cylinder's, cone's or sphere's seen from a point inside the carrier,
//! and a torus's seen along its axis, `crate::picture`), the pieces leaving each vertex are put
//! in turning order by their tangents, counted from a whole reference direction that no tangent
//! there can lie along, and two along one tangent by how they bend; the loops are then traced with
//! each piece's left side as the face it bounds. A loop running counter-clockwise, by the sign of
//! its certified area, bounds a split face; one running clockwise is the outside of a connected
//! group of pieces, a hole of the innermost split face that holds it (by the certified ray test),
//! or, held by none, the outside of the face itself. Vertices of other operands inside the face
//! that no piece reaches are given to the split face that holds them.
//!
//! **Curved faces.** A cylinder's or cone's picture has no seam and no pole: a cylinder's whole
//! side is a ring between its two end circles, and a cone's apex is a point like any other, where
//! the rulings that meet are ordered exactly. A sphere's face is seen from a point between it and
//! a pole, and a torus's along its axis, the half of the torus it lies in being the graph of a
//! height over the plane square to the axis. A split face's loops run counter-clockwise (its
//! outer loop) and clockwise (its holes) seen from outside the carrier. A face
//! is split only when the face stages have met everything with it: one that an edge or a face of
//! another operand may still cross (`FaceInfo::pending`) waits, a curved face or a flat one
//! whose box meets a curved face's.
//!
//! **Which side is the face.** A face's loops say it themselves: each runs with the face on its
//! left seen from outside the carrier ([`crate::interfere::FaceIn`]), as a body's fins do, so a
//! stretch of the face's own boundary bounds the split faces on that side only, on any carrier.
//! Nothing is derived from which loop comes first or which is outermost: on a sphere or a torus
//! no loop is outer. A flat face's picture keeps two coordinates as they come, so it is the view
//! from outside or its mirror, by the sign of the normal's dropped coordinate; the loops' sense
//! is turned over where it is the mirror.
//!
//! Guarantees:
//!
//! - **The turning order is proven or refused.** Two tangents are ordered by the side of a whole
//!   reference direction each leaves on and the sign of their cross product, exactly in wide
//!   integers where both are whole (a line between grid points, a section's n₁ × n₂), from
//!   enclosures otherwise. The reference at a vertex is the first ray direction of
//!   [`crate::inside::DIRECTIONS`] whose line no tangent's enclosure there meets, so a tangent
//!   along an axis, as a circle's at its top, is ordered as any other. Two pieces along one
//!   tangent are ordered by their curvature where they are proven to touch there exactly: on a
//!   flat face, two circles or a line and a circle given by whole numbers in its plane, the
//!   distance of their centres (or of the centre from the line) compared exactly with their radii.
//!   Any other pair whose cross product is not proven, and a vertex no direction serves, fail
//!   with [`BuildError::UndecidedOrder`]. No angle is computed, and no tolerance compares two.
//! - **Each loop's sense is proven.** A loop's area is enclosed piece by piece: a straight piece's
//!   exactly from its ends, a conic's arc on a flat face in closed form (a circle's or ellipse's
//!   with certified sines and cosines of half-turns, a hyperbola's with certified exponentials, a
//!   parabola's as a polynomial); an area that holds zero fails with
//!   [`BuildError::UndecidedArea`]. On a curved face a piece that is not straight is summed over
//!   parts of its parameter, each part's share enclosed by the mean value theorem, over more parts
//!   until the sign is proven or 4,096 parts leave it open.
//! - **Holes go to the face that holds them, by proof.** A clockwise loop's point is placed in each
//!   counter-clockwise loop of another group by the certified ray test. Two that hold it are
//!   nested, and the inner is the one whose own point the other holds, again by the ray test: no
//!   two areas are compared. No direction qualifying fails with [`BuildError::Unclassified`].
//! - **What this piece does not split, it says.** A face with no picture (a whole sphere; a
//!   torus's face across its middle plane, or on a torus whose axis is not along a coordinate; a
//!   face on any other curved carrier), a curved face with something pending, and a face bounded
//!   in a flat picture by a curve other than a line or a conic are listed as waiting: for G245's
//!   NURBS and procedural carriers.
//! - **Deterministic.** Faces in order, pieces in index order, vertices' pieces in turning order,
//!   loops in the order their first pieces come.
//!
//! Method: chapter 11.4, step 5 (`projects/engines/geometry/architecture/11-booleans.md`): each
//! face cut along its In and section edges in its own parameter space, the loops traced by ordering
//! the edges around each vertex, flat faces by exact orientation predicates. The loop walk is the
//! face traversal of a planar graph's rotation system (the next piece round each vertex from the
//! one arriving), as in M. de Berg, O. Cheong, M. van Kreveld and M. Overmars, *Computational
//! Geometry*, 3rd edition, Springer (2008), section 2.2, <https://doi.org/10.1007/978-3-540-77974-2>.
//! The order round a vertex by tangents is `SMLib`'s `sm_NextRoundTheVertex`
//! (`SmTrimmingTools::OrderCurvesIntoLoops`), read in solidcore's Rust port; it compares tangents
//! in doubles within a tolerance, and where they agree goes on to curvature, and its outer loops
//! are found by the areas of tessellations. None of that is taken: tangents are compared by proven
//! signs, and two along one tangent go on to curvature only where their touching is proven
//! exactly, never because two tangents agree within a tolerance. The order is counted from a
//! reference direction chosen to avoid every tangent at the vertex, so no tangent sits where the
//! count starts and ends: counted from a fixed axis, a curve leaving along the axis could belong
//! at either end.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/c440e5f892452a9654aee8f3030de6095fdb1415/crates/solidcore/src/topology/trimming_tools.rs>,
//!   for the order round a vertex and the grouping of outer loops and holes; no code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: per face, its pieces sorted round each vertex (at most 16 side tests per piece there to
//! find the reference, then O(k²) comparisons at a vertex of k pieces, each a few wide
//! multiplications or interval products), one walk over the pieces, an area per loop (on a curved
//! face 16 to 4,096 bounds of each piece that is not straight), and a ray test per hole and loop
//! pair; all charged to the meter. No recursion.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_intersect::IntersectError;
use geom_num::interval::Interval;
use geom_num::wide::{I256, I512};
use geom_num::{NumError, P3, Sign};
use geom_run::WorkMeter;
use geom_topo::{Axes, CurveKind, DerivativeBound, Line, ParamCurve};

use crate::carrier::{CarrierIx, CurveGeom};
use crate::crossing::{Enclosed, Hull};
use crate::inside::{Crossable, DIRECTIONS, FlatFace, hull as hull_of, ray_parity};
use crate::interfere::{Arrangement, BuildError, Entity, Interfered, committed, nearest_point};
use crate::picture::{Central, Graph, Picture, whole};
use crate::rule::{greater, lesser};

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
    /// bounded by a curve other than a line or a conic.
    pub waiting: Vec<usize>,
}

/// A piece's geometry.
enum Shape<'a> {
    /// A stretch of an edge's or a section's curve, from one pave's parameter to the next, round the curve's seam
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

/// The counter-clockwise loops (as indices into all the loops), each ready for the ray test and
/// with a grid point on it, and every loop's group.
type Outers<'r> = (&'r [usize], &'r [Ring], &'r [P3], &'r [Option<usize>]);

/// A tangent in the picture, leaving a vertex: whole when it can be, and its enclosure.
#[derive(Clone, Copy, Debug)]
struct Dir {
    exact: Option<[I256; 2]>,
    approx: [Interval; 2],
}

/// The picture's two coordinates of a vector of three.
fn pick<T: Copy>(v: [T; 3], [first, second]: [usize; 2]) -> Option<[T; 2]> {
    Some([*v.get(first)?, *v.get(second)?])
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

/// Where a tangent leaves from the whole reference direction r, turning counter-clockwise: 0 along
/// r itself, 1 left of r's line, 2 along −r, 3 right of r's line. Only a whole tangent is proven to
/// lie along the line; `None` for an enclosure that meets it.
fn side(d: &Dir, (run, rise): (i64, i64)) -> Option<u8> {
    if let Some([x, y]) = d.exact {
        let (run, rise) = (I256::from_i64(run), I256::from_i64(rise));
        // r × d and r · d, exactly.
        let across = run.checked_mul(&y)?.checked_sub(&rise.checked_mul(&x)?)?;
        let along = run.checked_mul(&x)?.checked_add(&rise.checked_mul(&y)?)?;
        return Some(match (across.signum(), along.signum()) {
            (Sign::Positive, _) => 1,
            (Sign::Negative, _) => 3,
            (Sign::Zero, Sign::Positive) => 0,
            (Sign::Zero, Sign::Negative | Sign::Zero) => 2,
        });
    }
    let [x, y] = d.approx;
    #[allow(
        clippy::cast_precision_loss,
        reason = "a reference direction's parts are at most 3"
    )]
    let (run, rise) = (
        Interval::point(run as f64).ok()?,
        Interval::point(rise as f64).ok()?,
    );
    match run
        .mul(y)
        .ok()?
        .sub(rise.mul(x).ok()?)
        .ok()?
        .certain_sign()?
    {
        Sign::Positive => Some(1),
        Sign::Negative => Some(3),
        Sign::Zero => None,
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
    ax.mul(by).ok()?.sub(ay.mul(bx).ok()?).ok()?.certain_sign()
}

/// The turning order of two tangents leaving one vertex, counter-clockwise from the reference
/// direction `r`, which neither's enclosure meets the line of: by the side each leaves on, then,
/// on one side, where every turn between them is less than a half-turn, by their cross product.
/// `None` when not proven, as for two along one tangent.
fn turning(a: &Dir, b: &Dir, r: (i64, i64)) -> Option<Ordering> {
    let (sa, sb) = (side(a, r)?, side(b, r)?);
    if sa != sb {
        return Some(sa.cmp(&sb));
    }
    match cross(a, b)? {
        Sign::Positive => Some(Ordering::Less),
        Sign::Negative => Some(Ordering::Greater),
        Sign::Zero => None,
    }
}

/// A piece of a flat face as the curvature step reads it: a line through a grid point along a
/// whole direction, or a circle about a grid point of a whole radius, given exactly in the face's
/// plane, and how it bends leaving its vertex: its signed curvature s / r in the picture, s the
/// side of its tangent its centre lies on (1 on the left, −1 on the right, 0 for a line) and r its
/// radius (1 for a line).
#[derive(Clone, Copy, Debug)]
enum Planar {
    /// A line: its origin and direction.
    Straight([I256; 3], [I256; 3]),
    /// A circle: its centre and radius, and the side of its tangent its centre lies on.
    Round([I256; 3], I256, Sign),
}

/// Three doubles as whole numbers, if each is one.
fn whole3(v: [f64; 3]) -> Option<[I256; 3]> {
    let [x, y, z] = v;
    Some([whole(x)?, whole(y)?, whole(z)?])
}

/// a · b, exactly; `None` past the width.
fn dot_whole(a: &[I256; 3], b: &[I256; 3]) -> Option<I256> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    a0.checked_mul(b0)?
        .checked_add(&a1.checked_mul(b1)?)?
        .checked_add(&a2.checked_mul(b2)?)
}

/// a − b, exactly.
fn minus_whole(a: &[I256; 3], b: &[I256; 3]) -> Option<[I256; 3]> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Some([
        a0.checked_sub(b0)?,
        a1.checked_sub(b1)?,
        a2.checked_sub(b2)?,
    ])
}

/// a × b, exactly.
fn cross_whole(a: &[I256; 3], b: &[I256; 3]) -> Option<[I256; 3]> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    let term =
        |x: &I256, y: &I256, z: &I256, w: &I256| x.checked_mul(y)?.checked_sub(&z.checked_mul(w)?);
    Some([
        term(a1, b2, a2, b1)?,
        term(a2, b0, a0, b2)?,
        term(a0, b1, a1, b0)?,
    ])
}

impl Planar {
    /// Half-edge `h`'s piece of `pieces` (leaving its first end for an even `h`), with its
    /// tangent there `dir`, when it is a line or a circle given exactly in the plane `normal` ·
    /// p + `offset` = 0 of a flat face's `picture`; `None` for any other.
    fn of(
        pieces: &[Bounding<'_>],
        (h, dir): (usize, &Dir),
        (normal, offset): (&[I256; 3], &I256),
        picture: &Picture,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Planar>, BuildError> {
        let Picture::Flat { keep } = picture else {
            return Ok(None);
        };
        let bounding = pieces.get(h / 2).ok_or(IntersectError::Internal)?;
        let Shape::Curve {
            curve,
            span: Some((from, to)),
            ..
        } = &bounding.shape
        else {
            return Ok(None);
        };
        let on_plane = |p: &[I256; 3]| -> Option<bool> {
            Some(dot_whole(normal, p)?.checked_add(offset)?.signum() == Sign::Zero)
        };
        let square_to =
            |v: &[I256; 3]| -> Option<bool> { Some(dot_whole(normal, v)?.signum() == Sign::Zero) };
        let parallel = |v: &[I256; 3]| -> Option<bool> {
            Some(
                cross_whole(normal, v)?
                    .iter()
                    .all(|part| part.signum() == Sign::Zero),
            )
        };
        Ok(match curve {
            CurveKind::Line(line) => {
                let (Some(origin), Some(direction)) =
                    (whole3(line.origin()), whole3(line.direction()))
                else {
                    return Ok(None);
                };
                // Its origin in the plane, and its direction along it.
                let held = [on_plane(&origin), square_to(&direction)];
                held.iter()
                    .all(|one| *one == Some(true))
                    .then_some(Planar::Straight(origin, direction))
            }
            CurveKind::Circle(circle) => {
                let (Some(centre), Some(radius), Some(axis)) = (
                    whole3(circle.center()),
                    whole(circle.radius()),
                    whole3(circle.frame().normal()),
                ) else {
                    return Ok(None);
                };
                // Its centre in the plane, and its axis square to it.
                let held = [on_plane(&centre), parallel(&axis)];
                if !held.iter().all(|one| *one == Some(true)) {
                    return Ok(None);
                }
                // The side of the tangent the centre lies on, in the picture.
                let t = if h % 2 == 0 { *from } else { *to };
                let at = point_at(curve, t, meter)?;
                let (Some([cx, cy]), Some([px, py])) =
                    (pick(exact3(circle.center())?, *keep), pick(at, *keep))
                else {
                    return Err(IntersectError::Internal.into());
                };
                let [dx, dy] = dir.approx;
                let side = dx
                    .mul(cy.sub(py)?)?
                    .sub(dy.mul(cx.sub(px)?)?)?
                    .certain_sign();
                match side {
                    Some(sign @ (Sign::Positive | Sign::Negative)) => {
                        Some(Planar::Round(centre, radius, sign))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
    }

    /// The signed curvature as a fraction: its side and its radius.
    fn bend(&self) -> (Sign, I256) {
        match self {
            Planar::Straight(..) => (Sign::Zero, I256::from_i64(1)),
            Planar::Round(_, radius, side) => (*side, *radius),
        }
    }
}

/// Whether two pieces of a flat face are proven, exactly, to touch where they meet: two circles
/// in its plane whose centres lie the sum or the difference of their radii apart, or a line and a
/// circle whose centre lies its radius from the line. Two such curves meet in one point, so at a
/// vertex both leave they leave along one tangent.
fn touch(first: &Planar, second: &Planar) -> bool {
    let tangent = match (first, second) {
        (Planar::Round(c1, r1, _), Planar::Round(c2, r2, _)) => (|| {
            let gap = minus_whole(c1, c2)?;
            let apart = dot_whole(&gap, &gap)?;
            if apart.signum() == Sign::Zero {
                return Some(false);
            }
            // ((r₁ + r₂)² − d²)((r₁ − r₂)² − d²) is nothing just when one factor is: one sign.
            let off =
                |radius: I256| -> Option<I256> { radius.checked_mul(&radius)?.checked_sub(&apart) };
            let both = off(r1.checked_add(r2)?)?.checked_mul(&off(r1.checked_sub(r2)?)?)?;
            Some(both.signum() == Sign::Zero)
        })(),
        (Planar::Straight(origin, along), Planar::Round(centre, radius, _))
        | (Planar::Round(centre, radius, _), Planar::Straight(origin, along)) => (|| {
            let across = cross_whole(along, &minus_whole(centre, origin)?)?;
            let far = dot_whole(&across, &across)?;
            let length = dot_whole(along, along)?;
            Some(
                radius
                    .checked_mul(radius)?
                    .checked_mul(&length)?
                    .checked_sub(&far)?
                    .signum()
                    == Sign::Zero,
            )
        })(),
        (Planar::Straight(..), Planar::Straight(..)) => None,
    };
    tangent == Some(true)
}

/// Two half-edges leaving one vertex, each with its tangent there.
type Leaving<'d> = (&'d (usize, Dir), &'d (usize, Dir));

/// The curvature step (11.4 step 5): the turning order of half-edges `a` and `b`, leaving one
/// vertex of a flat face along one tangent, when both pieces are proven to touch there
/// ([`touch`]). Leaving along one direction, the one that bends further to the left in the
/// picture, with the larger signed curvature, lies further counter-clockwise. A flat picture is
/// an affine map of the plane, which scales both curves' bend from their common tangent by one
/// factor, so the order of the curvatures in the plane, taken with the sides read in the picture,
/// is the order in the picture. `None` for any other pair, and for two that bend alike.
fn bend_order(
    arr: &Arrangement,
    pieces: &[Bounding<'_>],
    ((f, picture), (a, b)): ((usize, &Picture), Leaving<'_>),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Ordering>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let CarrierIx::Plane(ix) = face.surface else {
        return Ok(None);
    };
    let plane = arr
        .carriers
        .plane(ix)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let normal = plane.n().map(I256::from_i128);
    let offset = I256::from_i128(plane.d());
    let flat = (&normal, &offset);
    let (Some(first), Some(second)) = (
        Planar::of(pieces, (a.0, &a.1), flat, picture, meter)?,
        Planar::of(pieces, (b.0, &b.1), flat, picture, meter)?,
    ) else {
        return Ok(None);
    };
    if !touch(&first, &second) {
        return Ok(None);
    }
    let ((s1, r1), (s2, r2)) = (first.bend(), second.bend());
    let signed = |s: Sign, r: &I256| -> Option<I256> {
        match s {
            Sign::Positive => Some(*r),
            Sign::Negative => r.checked_neg(),
            Sign::Zero => Some(I256::from_i64(0)),
        }
    };
    // s₁ / r₁ against s₂ / r₂, both radii positive: s₁·r₂ against s₂·r₁.
    let (Some(left), Some(right)) = (signed(s1, &r2), signed(s2, &r1)) else {
        return Ok(None);
    };
    let Some(difference) = left.checked_sub(&right) else {
        return Ok(None);
    };
    Ok(match difference.signum() {
        Sign::Positive => Some(Ordering::Greater),
        Sign::Negative => Some(Ordering::Less),
        Sign::Zero => None,
    })
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

/// The pieces of face `f`: its boundary's split edges, the In split edges and its sections (a
/// section on a curve, as a split edge on a curve is), one split edge for each common block.
fn pieces_of<'a>(
    arr: &'a Arrangement,
    out: &'a Interfered,
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
        let (ends, span) = match (section.from, section.to) {
            (Some(from), Some(to)) => (Some([from.vertex, to.vertex]), Some((from.t, to.t))),
            _ => (None, None),
        };
        let shape = if let CurveGeom::Section(ix) = section.curve {
            Shape::Curve {
                curve: out.curves.get(ix).ok_or(IntersectError::Internal)?,
                span,
                wraps: section.wraps,
            }
        } else {
            let (direction, approx) = section_direction(arr, section.curve)?;
            Shape::Straight { direction, approx }
        };
        out_pieces.push(Bounding {
            piece: Piece::Section(s),
            ends,
            shape,
            left: None,
        });
    }
    Ok(out_pieces)
}

/// The tangent of `bounding` leaving its first end (`at_start`) or its last, in the picture.
/// `None` for a curve kind this piece does not order, and for a section's chord on a curved face.
fn leaving(
    bounding: &Bounding<'_>,
    at_start: bool,
    picture: &Picture,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Dir>, BuildError> {
    let flip = |v: Interval| if at_start { v } else { v.neg() };
    // Running a piece backwards turns its tangent round.
    let turn = |[x, y]: [I256; 2]| -> Result<[I256; 2], NumError> {
        if at_start {
            return Ok([x, y]);
        }
        Ok([
            x.checked_neg().ok_or(NumError::BudgetExceeded)?,
            y.checked_neg().ok_or(NumError::BudgetExceeded)?,
        ])
    };
    match &bounding.shape {
        Shape::Straight { direction, approx } => {
            let Picture::Flat { keep } = picture else {
                return Ok(None);
            };
            let (Some(exact), Some([ax, ay])) = (pick(*direction, *keep), pick(*approx, *keep))
            else {
                return Err(IntersectError::Internal.into());
            };
            Ok(Some(Dir {
                exact: Some(turn(exact)?),
                approx: [flip(ax), flip(ay)],
            }))
        }
        Shape::Curve { curve, span, .. } => {
            let Some((from, to)) = span else {
                return Err(IntersectError::Internal.into());
            };
            let t = if at_start { *from } else { *to };
            if let CurveKind::Line(line) = curve {
                let (exact, [ax, ay]) = match picture {
                    Picture::Flat { keep } => {
                        let Some([dx, dy]) = pick(line.direction(), *keep) else {
                            return Err(IntersectError::Internal.into());
                        };
                        (
                            whole(dx).zip(whole(dy)).map(|(x, y)| [x, y]),
                            [Interval::point(dx)?, Interval::point(dy)?],
                        )
                    }
                    Picture::Central(central) => (
                        central.line_tangent(line.origin(), line.direction()),
                        central.tangent(exact3(line.origin())?, exact3(line.direction())?)?,
                    ),
                };
                return Ok(Some(Dir {
                    exact: exact.map(turn).transpose()?,
                    approx: [flip(ax), flip(ay)],
                }));
            }
            let DerivativeBound::Bounded(d1) = curve
                .derivative_bound((t.lo(), t.hi()), 1, meter)
                .map_err(IntersectError::from)?
            else {
                return Ok(None);
            };
            let along = boxed(d1.lo, d1.hi)?;
            let [x1, y1] = match picture {
                Picture::Flat { keep } => {
                    let Some(kept) = pick(along, *keep) else {
                        return Err(IntersectError::Internal.into());
                    };
                    kept
                }
                Picture::Central(central) => central.tangent(point_at(curve, t, meter)?, along)?,
            };
            Ok(Some(Dir {
                exact: None,
                approx: [flip(x1), flip(y1)],
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

/// How a conic runs with its parameter: c + cos πt·a + sin πt·b (a circle or an ellipse),
/// c + cosh t·a + sinh t·b (a hyperbola's branch), or c + t²·a + t·b (a parabola).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Conic {
    /// A circle or an ellipse.
    Round,
    /// A hyperbola's branch.
    Branch,
    /// A parabola.
    Parabola,
}

/// A conic's centre and two axis vectors in the picture, enclosed.
type Terms = [[Interval; 2]; 3];

/// `v` times `by`, enclosed.
fn scaled(v: [Interval; 3], by: f64) -> Result<[Interval; 3], NumError> {
    let by = Interval::point(by)?;
    let [v0, v1, v2] = v;
    Ok([v0.mul(by)?, v1.mul(by)?, v2.mul(by)?])
}

/// A conic's two axis vectors, enclosed: on a frame, its unit vectors scaled; otherwise as stored.
fn axis_vectors(axes: Axes<3>) -> Result<([Interval; 3], [Interval; 3]), BuildError> {
    Ok(match axes {
        Axes::Frame { frame, a, b } => {
            let (_, [x, y, _]) = frame.unit_vectors().map_err(IntersectError::from)?;
            (scaled(x, a)?, scaled(y, b)?)
        }
        Axes::Vectors { u, v } => (exact3(u)?, exact3(v)?),
    })
}

/// The picture's centre (a parabola's vertex) and two axis vectors of a conic, enclosed, with how
/// it runs; `None` for another kind.
fn conic_of(curve: &CurveKind<3>, keep: [usize; 2]) -> Result<Option<(Conic, Terms)>, BuildError> {
    let (conic, centre, (a, b)) = match curve {
        CurveKind::Circle(circle) => {
            let (_, [x, y, _]) = circle
                .frame()
                .unit_vectors()
                .map_err(IntersectError::from)?;
            let r = circle.radius();
            (
                Conic::Round,
                circle.center(),
                (scaled(x, r)?, scaled(y, r)?),
            )
        }
        CurveKind::Ellipse(e) => (Conic::Round, e.center(), axis_vectors(e.axes())?),
        CurveKind::Hyperbola(h) => (Conic::Branch, h.center(), axis_vectors(h.axes())?),
        CurveKind::Parabola(p) => (Conic::Parabola, p.vertex(), axis_vectors(p.axes())?),
        _ => return Ok(None),
    };
    let (Some(c), Some(a), Some(b)) = (pick(exact3(centre)?, keep), pick(a, keep), pick(b, keep))
    else {
        return Err(IntersectError::Internal.into());
    };
    Ok(Some((conic, [c, a, b])))
}

/// ∫ x dy along an open conic in the picture, from `t0` to `t1`: a hyperbola's branch
/// c + cosh t·a + sinh t·b, or a parabola c + t²·a + t·b. With x = cx + C·ax + S·bx and
/// y′ = S·ay + C·by on the branch (C = cosh t, S = sinh t), the antiderivative is
/// cx·y + (ax·ay + bx·by)·S²/2 + ax·by·(t + S·C)/2 + bx·ay·(S·C − t)/2; on the parabola,
/// cx·y + ax·ay·t⁴/2 + (ax·by + 2·bx·ay)·t³/3 + bx·by·t²/2.
fn open_conic_area(
    conic: Conic,
    [c, a, b]: [[Interval; 2]; 3],
    (t0, t1): (Interval, Interval),
) -> Result<Interval, NumError> {
    let ([cx, cy], [ax, ay], [bx, by]) = (c, a, b);
    let half = Interval::point(0.5)?;
    let at = |t: Interval| -> Result<Interval, NumError> {
        if conic == Conic::Parabola {
            let t2 = t.mul(t)?;
            let t3 = t2.mul(t)?;
            let y = cy.add(t2.mul(ay)?)?.add(t.mul(by)?)?;
            let third = Interval::point(1.0)?.div(Interval::point(3.0)?)?;
            let cubic = ax
                .mul(by)?
                .add(Interval::point(2.0)?.mul(bx)?.mul(ay)?)?
                .mul(t3)?
                .mul(third)?;
            return cx
                .mul(y)?
                .add(ax.mul(ay)?.mul(t3.mul(t)?)?.mul(half)?)?
                .add(cubic)?
                .add(bx.mul(by)?.mul(t2)?.mul(half)?);
        }
        let (up, down) = (t.exp()?, t.neg().exp()?);
        let (ch, sh) = (up.add(down)?.mul(half)?, up.sub(down)?.mul(half)?);
        let y = cy.add(ch.mul(ay)?)?.add(sh.mul(by)?)?;
        let sc = sh.mul(ch)?;
        cx.mul(y)?
            .add(ax.mul(ay)?.add(bx.mul(by)?)?.mul(sh.mul(sh)?)?.mul(half)?)?
            .add(ax.mul(by)?.mul(t.add(sc)?)?.mul(half)?)?
            .add(bx.mul(ay)?.mul(sc.sub(t)?)?.mul(half)?)
    };
    at(t1)?.sub(at(t0)?)
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

/// ∫ x dy along `bounding` run `forward` or back, in the picture, a curved piece on a curved
/// face summed over `pieces` parts of its parameter; `None` for a curve kind this piece does not
/// measure.
fn area_of(
    arr: &Arrangement,
    out: &Interfered,
    bounding: &Bounding<'_>,
    forward: bool,
    (picture, pieces): (&Picture, u32),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, BuildError> {
    let along = match &bounding.shape {
        Shape::Straight { .. } => {
            let Picture::Flat { keep } = picture else {
                return Ok(None);
            };
            let Some([u, v]) = bounding.ends else {
                return Err(IntersectError::Internal.into());
            };
            let (pu, _) = committed(&arr.carriers, &out.vertices, u)?;
            let (pv, _) = committed(&arr.carriers, &out.vertices, v)?;
            chord_area(picture_point(pu, *keep)?, picture_point(pv, *keep)?)?
        }
        Shape::Curve { .. } => return curve_area(bounding, forward, (picture, pieces), meter),
    };
    Ok(Some(if forward { along } else { along.neg() }))
}

/// ∫ x dy along a curve piece run `forward` or back, in the picture; `None` for a curve kind this
/// piece does not measure.
fn curve_area(
    bounding: &Bounding<'_>,
    forward: bool,
    (picture, pieces): (&Picture, u32),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, BuildError> {
    let Shape::Curve { curve, span, wraps } = &bounding.shape else {
        return Err(IntersectError::Internal.into());
    };
    let (a, b) = curve.domain();
    let (lo, hi) = match span {
        Some((from, to)) => (*from, *to),
        None => (Interval::point(a)?, Interval::point(b)?),
    };
    let along = match picture {
        Picture::Flat { keep } => {
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
                let (Some(p0), Some(p1)) = (pick(at(lo)?, *keep), pick(at(hi)?, *keep)) else {
                    return Err(IntersectError::Internal.into());
                };
                chord_area(p0, p1)?
            } else {
                let Some((kind, conic)) = conic_of(curve, *keep)? else {
                    return Ok(None);
                };
                if kind != Conic::Round {
                    // An open conic runs from one pave to the next, never round a seam.
                    if *wraps {
                        return Err(IntersectError::Internal.into());
                    }
                    open_conic_area(kind, conic, (lo, hi))?
                } else if *wraps {
                    conic_area(conic, (lo, Interval::point(b)?))?
                        .add(conic_area(conic, (Interval::point(a)?, hi))?)?
                } else {
                    conic_area(conic, (lo, hi))?
                }
            }
        }
        Picture::Central(central) => {
            let spans = if *wraps {
                [
                    Some((lo, Interval::point(b)?)),
                    Some((Interval::point(a)?, hi)),
                ]
            } else {
                [Some((lo, hi)), None]
            };
            let mut total = Interval::point(0.0)?;
            for span in spans.into_iter().flatten() {
                let Some(part) = swept(central, curve, span, pieces, meter)? else {
                    return Ok(None);
                };
                total = total.add(part)?;
            }
            total
        }
    };
    Ok(Some(if forward { along } else { along.neg() }))
}

/// ∫ x dy along `curve` over the parameter span `ta..tb`, in a curved face's picture: the
/// picture's first coordinate over the span times the rate of its second there, times the span's
/// length, which holds the integral by the mean value theorem. `None` where the curve's tangent
/// is not bounded.
fn strip(
    central: &Central,
    curve: &CurveKind<3>,
    (ta, tb): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, BuildError> {
    let DerivativeBound::Bounded(d1) = curve
        .derivative_bound((ta, tb), 1, meter)
        .map_err(IntersectError::from)?
    else {
        return Ok(None);
    };
    let at = point_at(curve, Interval::new(ta, tb)?, meter)?;
    let [x, _] = central.point(at)?;
    let [_, rate] = central.rate(at, boxed(d1.lo, d1.hi)?)?;
    let length = Interval::point(tb)?.sub(Interval::point(ta)?)?;
    Ok(Some(x.mul(rate)?.mul(length)?))
}

/// ∫ x dy along `curve` from a parameter in `t0` to one in `t1`, in a curved face's picture. A
/// straight line's picture is straight, so its integral is its chord's. Any other curve is summed
/// over `pieces` equal parts between the two enclosures ([`strip`]); the stretch within each
/// enclosure, of which the piece runs an unknown part, adds anything from nothing to its whole
/// strip. `None` where the curve's tangent is not bounded.
fn swept(
    central: &Central,
    curve: &CurveKind<3>,
    (t0, t1): (Interval, Interval),
    pieces: u32,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, BuildError> {
    if let CurveKind::Line(_) = curve {
        let p0 = central.point(point_at(curve, t0, meter)?)?;
        let p1 = central.point(point_at(curve, t1, meter)?)?;
        return Ok(Some(chord_area(p0, p1)?));
    }
    let mut total = Interval::point(0.0)?;
    // Between the two enclosures; where they overlap, a span of no length, which adds nothing.
    let start = t0.hi();
    let end = greater(t1.lo(), start);
    let mut from = start;
    for k in 1..=pieces {
        meter.charge(1).map_err(IntersectError::from)?;
        // The last part ends at the end itself, so the parts cover the span and nothing more.
        let to = if k == pieces {
            end
        } else {
            start + (end - start) * (f64::from(k) / f64::from(pieces))
        };
        let Some(part) = strip(central, curve, (from, to), meter)? else {
            return Ok(None);
        };
        total = total.add(part)?;
        from = to;
    }
    for t in [t0, t1] {
        let Some(part) = strip(central, curve, (t.lo(), t.hi()), meter)? else {
            return Ok(None);
        };
        total = total.add(Interval::new(
            lesser(part.lo(), 0.0),
            greater(part.hi(), 0.0),
        )?)?;
    }
    Ok(Some(total))
}

/// A loop's area, summed by `sum` over more and more parts of its curved pieces until its sign is
/// proven; `None` when `sum` gives none.
fn proven(
    (f, picture): (usize, &Picture),
    mut sum: impl FnMut(u32) -> Result<Option<Interval>, BuildError>,
) -> Result<Option<Interval>, BuildError> {
    for &pieces in picture.levels() {
        let Some(total) = sum(pieces)? else {
            return Ok(None);
        };
        if let Some(Sign::Positive | Sign::Negative) = total.certain_sign() {
            return Ok(Some(total));
        }
    }
    Err(BuildError::UndecidedArea { face: f })
}

/// The grid point nearest the middle of `curve`.
fn middle_point(curve: &CurveKind<3>, meter: &mut WorkMeter<'_>) -> Result<P3, BuildError> {
    let (start, end) = curve.domain();
    let middle = point_at(curve, Interval::point(start / 2.0 + end / 2.0)?, meter)?;
    Ok(nearest_point(middle.map(|c| c.lo() / 2.0 + c.hi() / 2.0))?)
}

/// Whether counter-clockwise in `picture` is counter-clockwise seen from outside face `f`'s
/// carrier ([`FaceIn`]): always for a curved face's picture, which is seen from outside; for a
/// flat face's, when the coordinates kept run as the plane's normal turns them. Seen from the
/// third coordinate, the kept two run counter-clockwise when the three are in cyclic order, and
/// the normal points to the side it is seen from when its own part there is positive. A torus's
/// face seen along its axis is a flat picture too, its outward normal pointing away from the
/// torus's middle plane on the face's side of it ([`Graph`]).
fn seen_from_outside(
    arr: &Arrangement,
    f: usize,
    picture: &Picture,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, BuildError> {
    let Picture::Flat { keep } = picture else {
        return Ok(true);
    };
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let (cyclic, dropped): (bool, usize) = match keep {
        [1, 2] => (true, 0),
        [0, 2] => (false, 1),
        _ => (true, 2),
    };
    let up = if let CarrierIx::Plane(ix) = face.surface {
        let plane = arr
            .carriers
            .plane(ix)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        // Named types, so the recursion check can read them.
        let normal: [i128; 3] = plane.n();
        normal.get(dropped).is_some_and(|n: &i128| *n > 0)
    } else {
        let Some((_, up, _)) = Graph::side(arr, f, meter)? else {
            return Err(BuildError::Missing(Entity::Face(f)));
        };
        up
    };
    Ok(up == cyclic)
}

/// The area face `f`'s loops bound in `picture`, each loop run as its uses say and the whole
/// turned over where the picture is the mirror of the view from outside, so that it is the area
/// the face's loops have it on their left: positive when the face is the part of the picture its
/// loops close round, negative when it is the rest of the picture. Summed level by level until
/// its sign is proven; `None` for an edge kind this piece does not measure.
///
/// # Errors
///
/// [`BuildError::UndecidedArea`] for an area whose sign no level proves; an enclosure's.
pub(crate) fn face_area(
    arr: &Arrangement,
    f: usize,
    picture: &Picture,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Interval>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let same = seen_from_outside(arr, f, picture, meter)?;
    proven((f, picture), |pieces| {
        let mut total = Interval::point(0.0)?;
        for one in face.loops.iter().flatten() {
            let edge = arr
                .edges
                .get(one.edge)
                .ok_or(BuildError::Missing(Entity::Edge(one.edge)))?;
            let curve = arr
                .carriers
                .curve(edge.curve)
                .ok_or(BuildError::Missing(Entity::Edge(one.edge)))?;
            let (a, b) = curve.domain();
            let whole = Bounding {
                piece: Piece::Split(one.edge),
                ends: None,
                shape: Shape::Curve {
                    curve,
                    span: Some((Interval::point(a)?, Interval::point(b)?)),
                    wraps: false,
                },
                left: None,
            };
            let Some(part) = curve_area(&whole, one.forward == same, (picture, pieces), meter)?
            else {
                return Ok(None);
            };
            total = total.add(part)?;
        }
        Ok(Some(total))
    })
}

/// For each edge of face `f`'s loops, whether the face lies on its left in the picture when it
/// is run forward: as its loop runs it, the face lying on each loop's left seen from outside the
/// carrier ([`FaceIn`]), turned over where the picture is that view's mirror. Each loop is first
/// checked to run end to end: every edge starting where the one before it ends, and a closed edge
/// alone in its loop.
///
/// # Errors
///
/// [`BuildError::BrokenLoop`] for a loop that does not run end to end.
fn senses(
    arr: &Arrangement,
    (f, picture): (usize, &Picture),
    meter: &mut WorkMeter<'_>,
) -> Result<BTreeMap<usize, bool>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let same = seen_from_outside(arr, f, picture, meter)?;
    let mut left = BTreeMap::new();
    for (k, ring) in face.loops.iter().enumerate() {
        let broken = BuildError::BrokenLoop { face: f, ring: k };
        // Each use's first and last vertex, in the order the loop runs it.
        let mut runs: Vec<Option<[usize; 2]>> = Vec::with_capacity(ring.len());
        for one in ring {
            let edge = arr
                .edges
                .get(one.edge)
                .ok_or(BuildError::Missing(Entity::Edge(one.edge)))?;
            runs.push(
                edge.ends
                    .map(|[s, t]| if one.forward { [s, t] } else { [t, s] }),
            );
        }
        for (i, run) in runs.iter().enumerate() {
            let next = runs
                .get(i.saturating_add(1))
                .or(runs.first())
                .ok_or(broken)?;
            match (run, next) {
                (Some([_, end]), Some([start, _])) if end == start => {}
                (None, None) if runs.len() == 1 => {}
                _ => return Err(broken),
            }
        }
        for one in ring {
            left.insert(one.edge, one.forward == same);
        }
    }
    Ok(left)
}

/// The pieces leaving each vertex, in turning order: half-edge 2i leaves piece i's first end, and
/// 2i + 1 its last. `None` when a tangent cannot be had.
fn rotations(
    arr: &Arrangement,
    pieces: &[Bounding<'_>],
    (f, picture): (usize, &Picture),
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
            let Some(dir) = leaving(bounding, at_start, picture, meter)? else {
                return Ok(None);
            };
            at.entry(vertex).or_default().push((half_edge, dir));
        }
    }
    for (vertex, list) in &mut at {
        // The order is counted from the first ray direction whose line no tangent's enclosure at
        // the vertex meets; any such direction gives the same order round the vertex.
        let Some(reference) = DIRECTIONS
            .into_iter()
            .find(|&r| list.iter().all(|(_, d)| side(d, r).is_some()))
        else {
            return Err(BuildError::UndecidedOrder {
                face: f,
                vertex: *vertex,
            });
        };
        // Insertion sort by a proven order: a vertex has few pieces.
        let mut sorted: Vec<(usize, Dir)> = Vec::with_capacity(list.len());
        for item in list.drain(..) {
            let mut place = sorted.len();
            for (k, other) in sorted.iter().enumerate() {
                // Two along one tangent go by how they bend, where that is proven.
                let order = match turning(&item.1, &other.1, reference) {
                    Some(order) => Some(order),
                    None => bend_order(arr, pieces, ((f, picture), (&item, other)), meter)?,
                };
                match order {
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
        // Each join is the box round the ends of the sides that meet at its vertex, where the ring
        // could pass from one side to the next: a chord's ends are its vertices' grid points, a
        // curve's its points at the paves' parameters.
        match &bounding.shape {
            Shape::Straight { .. } => {
                let [u, v] = bounding.ends.ok_or(IntersectError::Internal)?;
                let kind = chord(arr, out, [u, v])?;
                join(u, point_at(&kind, Interval::point(0.0)?, meter)?)?;
                join(v, point_at(&kind, Interval::point(1.0)?, meter)?)?;
                let hull = Hull::of(&kind, meter)?;
                made.sides
                    .push((Source::Chord(made.chords.len()), (0.0, 1.0), hull));
                made.chords.push(kind);
            }
            Shape::Curve { curve, span, wraps } => {
                let (a, b) = curve.domain();
                let hull = Hull::of(curve, meter)?;
                if let (Some((from, to)), Some([u, v])) = (span, bounding.ends) {
                    // The stretches within the paves' parameters lie in the joins at their
                    // vertices, so the ray test searches only between them; round the seam, the
                    // seam is a join of its own, where the search's two spans meet.
                    join(u, point_at(curve, *from, meter)?)?;
                    join(v, point_at(curve, *to, meter)?)?;
                    if *wraps {
                        made.joins.push(hull_of(
                            point_at(curve, Interval::point(a)?, meter)?,
                            point_at(curve, Interval::point(b)?, meter)?,
                        )?);
                        made.sides
                            .push((Source::Piece(i), (from.hi(), b), hull.clone()));
                        made.sides.push((Source::Piece(i), (a, to.lo()), hull));
                    } else {
                        made.sides
                            .push((Source::Piece(i), (from.hi(), to.lo()), hull));
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
    (f, picture): (usize, &Picture),
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
        if let Some(inside) = ray_parity(picture, (&sides, &ring.joins), point, direction, meter)? {
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
    middle_point(curve, meter)
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

/// The loops of face `f`'s pieces with their areas, or `None` when this piece leaves the face
/// for another.
fn loops_of(
    arr: &Arrangement,
    out: &Interfered,
    (f, picture): (usize, &Picture),
    (pieces, index): (&[Bounding<'_>], &BTreeMap<Piece, usize>),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Loops>, BuildError> {
    let Some(at) = rotations(arr, pieces, (f, picture), meter)? else {
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
        let Some(total) = proven((f, picture), |parts| {
            let mut total = Interval::point(0.0)?;
            for one in uses {
                let i = *index.get(&one.piece).ok_or(IntersectError::Internal)?;
                let bounding = pieces.get(i).ok_or(IntersectError::Internal)?;
                let Some(part) = area_of(arr, out, bounding, one.forward, (picture, parts), meter)?
                else {
                    return Ok(None);
                };
                total = total.add(part)?;
            }
            Ok(Some(total))
        })?
        else {
            return Ok(None);
        };
        areas.push(Some(total));
    }
    Ok(Some((loops, areas)))
}

/// The faces face `f` splits into, or `None` when this piece leaves it for another: a face with
/// no picture, and a face that an edge or a face of another operand may cross, which the face
/// stages have not yet met with it (a curved face, or a flat one whose box meets a curved one's).
fn split_face(
    arr: &Arrangement,
    out: &Interfered,
    f: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<SplitFace>>, BuildError> {
    let waits = out
        .faces
        .get(f)
        .is_some_and(|info| !info.pending.edges.is_empty() || !info.pending.faces.is_empty());
    if waits {
        return Ok(None);
    }
    let picture = if let Some(flat) = FlatFace::new(arr, f, meter)? {
        Picture::Flat { keep: flat.keep() }
    } else if let Some(central) = Central::of_face(arr, f, meter)? {
        Picture::Central(central)
    } else if let Some(graph) = Graph::of_face(arr, f, meter)? {
        Picture::Flat { keep: graph.keep() }
    } else {
        return Ok(None);
    };
    let picture = &picture;
    let senses = senses(arr, (f, picture), meter)?;
    // The face its loops name lies in the part of the picture they close round: on a plane, a
    // cylinder or a cone the face is bounded, and a sphere's or a torus's picture is chosen so.
    let Some(area) = face_area(arr, f, picture, meter)? else {
        return Ok(None);
    };
    if area.certain_sign() != Some(Sign::Positive) {
        return Err(BuildError::Unbounded { face: f });
    }
    let pieces = pieces_of(arr, out, f, &senses)?;
    let index: BTreeMap<Piece, usize> = pieces
        .iter()
        .enumerate()
        .map(|(i, b)| (b.piece, i))
        .collect();
    let Some((loops, areas)) = loops_of(arr, out, (f, picture), (&pieces, &index), meter)? else {
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
        (f, picture),
        (&pieces, &index),
        (loops, areas),
        meter,
    )
    .map(Some)
}

/// The innermost of `outer` (loops ready as `rings`, with a grid point on each) holding `point`,
/// of a group other than `own`: its index in `outer`. Two loops of different groups that hold one
/// point are nested, one inside the other, and the inner is the one whose own point the other
/// holds: proven by the ray test, never read from two areas.
fn smallest(
    pieces: &[Bounding<'_>],
    (f, picture): (usize, &Picture),
    (outer, rings, points, groups_of): Outers<'_>,
    (point, own): (P3, Option<usize>),
    meter: &mut WorkMeter<'_>,
) -> Result<Option<usize>, BuildError> {
    let mut best: Option<(usize, &Ring)> = None;
    for (slot, (&k, ring)) in outer.iter().zip(rings).enumerate() {
        if own.is_some() && groups_of.get(k).copied().flatten() == own {
            continue;
        }
        if !holds(ring, pieces, (f, picture), point, meter)? {
            continue;
        }
        let inner = match best {
            None => true,
            Some((_, so_far)) => {
                let on_ring = *points.get(slot).ok_or(IntersectError::Internal)?;
                holds(so_far, pieces, (f, picture), on_ring, meter)?
            }
        };
        if inner {
            best = Some((slot, ring));
        }
    }
    Ok(best.map(|(slot, _)| slot))
}

/// The split faces: each counter-clockwise loop with the clockwise loops of other groups and the
/// lone vertices it is the smallest to hold.
fn assemble(
    arr: &Arrangement,
    out: &Interfered,
    (f, picture): (usize, &Picture),
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
    let mut points = Vec::with_capacity(outer.len());
    let mut faces: Vec<SplitFace> = Vec::with_capacity(outer.len());
    for &k in &outer {
        let uses = loops.get(k).ok_or(IntersectError::Internal)?;
        rings.push(ring(arr, out, (pieces, index), uses, meter)?);
        let first = uses.first().ok_or(IntersectError::Internal)?;
        let bounding = pieces
            .get(*index.get(&first.piece).ok_or(IntersectError::Internal)?)
            .ok_or(IntersectError::Internal)?;
        points.push(point_on(arr, out, bounding, meter)?);
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
        points.as_slice(),
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
        if let Some(slot) = smallest(pieces, (f, picture), found, (point, own), meter)? {
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
        if let Some(slot) = smallest(pieces, (f, picture), found, (point, None), meter)? {
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
        match split_face(arr, out, f, meter)? {
            Some(faces) => made.faces.extend(faces),
            None => made.waiting.push(f),
        }
    }
    Ok(made)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]
mod tests {
    use super::*;
    use crate::carrier::CarrierIx;
    use crate::interfere::{EdgeIn, EdgeUse, FaceIn};
    use geom_curve::{Circle, Frame};
    use geom_run::Budget;
    use geom_surface::Cylinder;
    use geom_topo::SurfaceKind;

    fn at(t: f64) -> Interval {
        Interval::point(t).unwrap()
    }

    /// Whether an enclosure holds `value`.
    fn holds_value(x: Interval, value: f64) -> bool {
        x.lo() <= value && value <= x.hi()
    }

    /// A cylinder of radius 50 up the third coordinate from 0 to 100, its two end circles, and
    /// its picture: seen from (0, 0, 258), mirrored.
    fn tube() -> (Central, Vec<CurveKind<3>>) {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let mut arr = Arrangement::default();
        let mut circles = Vec::new();
        for z in [0.0, 100.0] {
            let circle =
                CurveKind::Circle(Circle::new([0.0, 0.0, z], 50.0, frame, (0.0, 2.0)).unwrap());
            circles.push(circle.clone());
            let curve = arr.carriers.add_curve(circle).unwrap();
            arr.edges.push(EdgeIn {
                curve,
                ends: None,
                operand: 0,
            });
        }
        let cylinder = Cylinder::new([0.0; 3], 50.0, frame, ((0.0, 2.0), (0.0, 100.0))).unwrap();
        let ix = arr
            .carriers
            .add_surface(SurfaceKind::Cylinder(cylinder))
            .unwrap();
        arr.faces.push(FaceIn {
            surface: CarrierIx::Surface(ix),
            // The face lies above its bottom circle and below its top one.
            loops: vec![
                vec![EdgeUse {
                    edge: 0,
                    forward: true,
                }],
                vec![EdgeUse {
                    edge: 1,
                    forward: false,
                }],
            ],
            operand: 0,
        });
        let m = &mut WorkMeter::new(Budget::UNLIMITED);
        (Central::of_face(&arr, 0, m).unwrap().unwrap(), circles)
    }

    #[test]
    fn a_circle_s_area_in_a_cylinder_s_picture_is_enclosed_ever_closer() {
        let (central, circles) = tube();
        let m = &mut WorkMeter::new(Budget::UNLIMITED);
        // A circle at depth d is a circle of radius 50 / d in the picture, run clockwise there
        // (the picture is mirrored): its area is −π (50 / d)².
        for (circle, depth) in circles.iter().zip([258.0_f64, 158.0]) {
            let area = -core::f64::consts::PI * (50.0 / depth) * (50.0 / depth);
            let mut last = f64::INFINITY;
            for pieces in [16, 256, 4096] {
                let got = swept(&central, circle, (at(0.0), at(2.0)), pieces, m)
                    .unwrap()
                    .unwrap();
                assert!(holds_value(got, area), "{pieces}: {got:?} {area}");
                let width = got.hi() - got.lo();
                assert!(width < last / 8.0, "{pieces}: {width} after {last}");
                last = width;
            }
            assert!(last < area.abs() / 50.0, "{last} of {area}");
            // Half the circle sweeps half of it.
            let half = swept(&central, circle, (at(0.0), at(1.0)), 4096, m)
                .unwrap()
                .unwrap();
            assert!(holds_value(half, area / 2.0), "{half:?}");
            assert!(half.hi() - half.lo() < area.abs() / 50.0);
        }
    }

    #[test]
    fn a_span_between_two_enclosures_holds_every_piece_it_may_be() {
        let (central, circles) = tube();
        let m = &mut WorkMeter::new(Budget::UNLIMITED);
        let r = 50.0 / 158.0;
        let quarter = |a: f64, b: f64| Interval::new(a, b).unwrap();
        // From somewhere in the first quarter of a half turn to somewhere in the last: at most
        // the half circle, −π r² / 2, at least its middle half, −r² (π / 4 − 1 / 2).
        let got = swept(
            &central,
            &circles[1],
            (quarter(0.0, 0.25), quarter(0.75, 1.0)),
            4096,
            m,
        )
        .unwrap()
        .unwrap();
        let (most, least) = (
            -core::f64::consts::PI * r * r / 2.0,
            -r * r * (core::f64::consts::FRAC_PI_4 - 0.5),
        );
        assert!(holds_value(got, most) && holds_value(got, least), "{got:?}");
        assert!(got.lo() > most * 1.25 && got.hi() < 0.0, "{got:?}");
        // Enclosures that overlap leave a span of no length: only what the two may add.
        let got = swept(
            &central,
            &circles[1],
            (quarter(0.25, 0.75), quarter(0.5, 1.0)),
            16,
            m,
        )
        .unwrap()
        .unwrap();
        assert!(
            holds_value(got, 0.0) && holds_value(got, most * 0.9),
            "{got:?}"
        );
    }

    #[test]
    fn a_ruling_s_area_is_its_chord_s() {
        let (central, _) = tube();
        let m = &mut WorkMeter::new(Budget::UNLIMITED);
        // The ruling through (30, 40) from the bottom to the top: in the picture the straight
        // piece from (−30, 40) / 258 to (−30, 40) / 158.
        let ruling =
            CurveKind::Line(Line::new([30.0, 40.0, 0.0], [0.0, 0.0, 100.0], (0.0, 1.0)).unwrap());
        let got = swept(&central, &ruling, (at(0.0), at(1.0)), 16, m)
            .unwrap()
            .unwrap();
        let (near, far) = (1.0 / 158.0_f64, 1.0 / 258.0_f64);
        let area = -15.0 * (near + far) * 40.0 * (near - far);
        assert!(holds_value(got, area), "{got:?} {area}");
        assert!(got.hi() - got.lo() < 1e-12, "{got:?}");
    }

    #[test]
    fn touching_is_proven_exactly_from_whole_data() {
        let w = |v: [i64; 3]| v.map(I256::from_i64);
        let round = |c: [i64; 3], r: i64| Planar::Round(w(c), I256::from_i64(r), Sign::Positive);
        let straight = |o: [i64; 3], d: [i64; 3]| Planar::Straight(w(o), w(d));
        // Circles touching from outside and from inside, and two that cross.
        assert!(touch(&round([0, 0, 0], 3), &round([5, 0, 0], 2)));
        assert!(touch(&round([0, 0, 0], 5), &round([3, 4, 0], 10)));
        assert!(touch(&round([0, 0, 0], 5), &round([0, 3, 0], 2)));
        assert!(!touch(&round([0, 0, 0], 5), &round([0, 3, 0], 3)));
        // Circles about one centre never touch.
        assert!(!touch(&round([1, 1, 0], 5), &round([1, 1, 0], 3)));
        // A line along y = 5 touches the circle of radius 5 about the origin, either way round;
        // along y = 4 it crosses it, and two lines are no touch of this kind.
        assert!(touch(&straight([7, 5, 0], [2, 0, 0]), &round([0, 0, 0], 5)));
        assert!(touch(&round([0, 0, 0], 5), &straight([7, 5, 0], [2, 0, 0])));
        assert!(!touch(
            &straight([7, 4, 0], [2, 0, 0]),
            &round([0, 0, 0], 5)
        ));
        assert!(!touch(
            &straight([0, 0, 0], [1, 0, 0]),
            &straight([0, 1, 0], [1, 0, 0])
        ));
        // A line's curvature is nothing, a circle's its side over its radius.
        let (s, r) = straight([0, 0, 0], [1, 0, 0]).bend();
        assert_eq!((s, r.signum()), (Sign::Zero, Sign::Positive));
        let (s, r) = round([0, 0, 0], 5).bend();
        assert_eq!(s, Sign::Positive);
        assert_eq!(
            r.checked_sub(&I256::from_i64(5)).unwrap().signum(),
            Sign::Zero
        );
    }

    #[test]
    fn an_area_is_proven_at_the_first_level_that_decides_it() {
        let picture = Picture::Central(tube().0);
        let iv = |lo: f64, hi: f64| Interval::new(lo, hi).unwrap();
        // Decided at the third level, negative; the levels asked for are the picture's.
        let mut asked = Vec::new();
        let got = proven((7, &picture), |pieces| {
            asked.push(pieces);
            Ok(Some(match asked.len() {
                1 => iv(-1.0, 1.0),
                2 => iv(0.0, 0.0),
                _ => iv(-0.75, -0.25),
            }))
        })
        .unwrap()
        .unwrap();
        assert_eq!((got.lo(), got.hi()), (-0.75, -0.25));
        assert_eq!(asked, vec![16, 64, 256]);
        // Never decided: refused, with the face.
        let never = proven((7, &picture), |_| Ok(Some(iv(-1.0, 1.0))));
        assert!(matches!(never, Err(BuildError::UndecidedArea { face: 7 })));
        // A kind that is not measured gives none at once.
        let mut asked = 0;
        let none = proven((7, &picture), |_| {
            asked += 1;
            Ok(None)
        })
        .unwrap();
        assert!(none.is_none());
        assert_eq!(asked, 1);
        // A flat face's area is a closed form: one level.
        let flat = Picture::Flat { keep: [0, 1] };
        let mut asked = 0;
        let refused = proven((3, &flat), |_| {
            asked += 1;
            Ok(Some(iv(-1.0, 1.0)))
        });
        assert!(matches!(
            refused,
            Err(BuildError::UndecidedArea { face: 3 })
        ));
        assert_eq!(asked, 1);
        let positive = proven((3, &flat), |_| Ok(Some(iv(2.0, 3.0))))
            .unwrap()
            .unwrap();
        assert_eq!(positive.lo(), 2.0);
    }
}
