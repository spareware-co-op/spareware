//! Inputs for the curved builder's broad phase (piece G242, decision 32.44): a scene of points
//! padded by the contact radius, as boxes with distinct keys, for the pairs whose boxes meet. And
//! for ray classification (piece G246): grid points in and round a box, with the box as an
//! arrangement of one operand ([`box_scene`]); about one point in eight lies inside it, and a
//! point on it, which has no winding number, is kept as the refusal it is.
//!
//! Class `typical` only: each input is a scene of [`SCENE`] points of a cube 512 steps on a side, so
//! each box (a point padded by 64 steps) meets several others and none meets them all. The
//! refusals of the broad phase (a box past the tree's range) are tested in `geom-bool` itself.
//!
//! Guarantees: every batch is full and the same on every target, and every result of a typical
//! batch is `Ok` (`tests/inputs.rs`); every result is the operation's own, run directly
//! (`tests/attack_g06.rs`).
//!
//! Method: the broad phase of `geom-bool`'s attack tests (`tests/near_contact_booleans.rs`, checked
//! there against brute force), built and queried whole under an unlimited budget on the serial
//! executor. Each is measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: about a hundred draws of [`SplitMix64`] per input, once per batch, outside every measured
//! run.

use geom_bool::broad::point_entries;
use geom_bool::carrier::{CarrierIx, PointGeom};
use geom_bool::distance::Carriers;
use geom_bool::interfere::{Arrangement, EdgeIn, FaceIn, VertexIn};
use geom_bvh::Entry;
use geom_curve::{CurveKind, Line};
use geom_num::grid::P3;
use geom_num::plane::Plane;

use super::{Class, batch_of, p3, rng, within};

/// Points per scene.
pub const SCENE: usize = 32;
/// Half the side of the cube a scene lies in, in grid steps.
const REACH: i64 = 256;

/// Half the side of the box of [`box_scene`], in grid steps.
pub const BOX: i64 = 128;

/// The box from −[`BOX`] to [`BOX`] on every axis as an arrangement of operand 0, with, for each
/// face, whether its outside is the side its plane's normal points to. `None` only if a part
/// cannot be built, which these whole numbers never cause.
#[must_use]
pub fn box_scene() -> Option<(Arrangement, Vec<bool>)> {
    let mut arr = Arrangement::default();
    let mut outward = Vec::new();
    let corner = |k: usize| -> [i64; 3] {
        [0, 1, 2].map(|axis| if k >> axis & 1 == 1 { BOX } else { -BOX })
    };
    for k in 0..8 {
        let [x, y, z] = corner(k);
        arr.vertices.push(VertexIn {
            point: PointGeom::Grid(p3(x, y, z)),
            operand: 0,
        });
    }
    // The edge from corner k up each axis it is low on, by its index in the arrangement.
    let mut slots = [[usize::MAX; 3]; 8];
    for (k, slot) in slots.iter_mut().enumerate() {
        for (axis, entry) in slot.iter_mut().enumerate() {
            if k >> axis & 1 == 1 {
                continue;
            }
            #[allow(
                clippy::cast_precision_loss,
                reason = "the corners' coordinates are at most 128 in size"
            )]
            let origin = corner(k).map(|c| c as f64);
            let mut direction = [0.0; 3];
            *direction.get_mut(axis)? = 2.0 * origin.get(axis)?.abs();
            let line = Line::new(origin, direction, (0.0, 1.0)).ok()?;
            let curve = arr.carriers.add_curve(CurveKind::Line(line)).ok()?;
            *entry = arr.edges.len();
            arr.edges.push(EdgeIn {
                curve,
                ends: Some([k, k | 1 << axis]),
                operand: 0,
            });
        }
    }
    for axis in 0..3_usize {
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in 0..2_usize {
            let ring =
                [(0, 0), (1, 0), (1, 1), (0, 1)].map(|(sb, sc)| side << axis | sb << b | sc << c);
            let mut edges = Vec::with_capacity(4);
            for i in 0..4_usize {
                let (u, v) = (*ring.get(i)?, *ring.get((i + 1) % 4)?);
                let (low, high) = if u < v { (u, v) } else { (v, u) };
                let along = usize::try_from((low ^ high).trailing_zeros()).ok()?;
                edges.push(*slots.get(low)?.get(along)?);
            }
            let mut normal = [0_i128; 3];
            *normal.get_mut(axis)? = 1;
            let offset = if side == 1 { -BOX } else { BOX };
            let plane = Plane::new(normal, i128::from(offset)).ok()?;
            let surface = CarrierIx::Plane(arr.carriers.add_plane(&plane).ok()?);
            arr.faces.push(FaceIn {
                surface,
                loops: vec![edges],
                operand: 0,
            });
            // The table's normal points up the axis: outward on the high side.
            outward.push(side == 1);
        }
    }
    Some((arr, outward))
}

/// Grid points in and round the box of [`box_scene`], for `Boundary::winding`.
#[must_use]
pub fn classify_points(class: Class) -> Vec<P3> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("curved::classify_points", class);
    batch_of(|_| {
        Some(p3(
            within(&mut g, REACH),
            within(&mut g, REACH),
            within(&mut g, REACH),
        ))
    })
}

/// Scenes of padded point boxes for `candidate_pairs`.
#[must_use]
pub fn broad_scenes(class: Class) -> Vec<Vec<Entry<3>>> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("curved::broad_scenes", class);
    let carriers = Carriers::new();
    batch_of(|_| {
        let points: Vec<PointGeom> = (0..SCENE)
            .map(|_| {
                PointGeom::Grid(p3(
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                    within(&mut g, REACH),
                ))
            })
            .collect();
        point_entries(&carriers, &points).ok()
    })
}
