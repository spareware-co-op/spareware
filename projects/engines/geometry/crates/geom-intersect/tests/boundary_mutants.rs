//! An edge that lies in the other surface, traced through the march, with the boxes the proof of
//! it gives held to their recorded bits (G173a, chapter 12.8 step 5). The proof solves two of the
//! edge system's three equations: which two, the start of each piece's Gauss-Newton, the box about
//! it and the bound on how far the edge bows from its chords all leave their mark on the vertices
//! a run puts in a branch and on the branch's deviation. Each case has its answer known exactly as
//! well, a straight line through whole numbers or an arc of a circle, and every vertex is held
//! against it.
//!
//! Run at the scale of the SSI corpus (1024 steps to the unit, floor 8, ceiling 10.24), in both
//! orders of the pair. The parameters along each edge stand ten or more from zero, and so do the
//! other plane's, so a point read at the wrong place leaves a domain.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly, and platform maths may judge the kernel's"
)]

use geom_curve::Frame;
use geom_intersect::general::subdivide;
use geom_intersect::march::{Branch, march};
use geom_intersect::result::CurveEnd;
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Plane, SurfaceKind};

const UNIT: f64 = 1024.0;
const FLOOR: f64 = 8.0;
const CEILING: f64 = 10.24;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 24))
}

/// The one branch of the pair: nothing flagged, every step proven, an edge of a domain at both
/// ends.
fn branch(a: &SurfaceKind, b: &SurfaceKind) -> Branch {
    let found = subdivide(a, b, FLOOR, &mut meter()).unwrap();
    assert!(found.flagged.is_empty(), "{:#?}", found.flagged);
    let traced = march(a, b, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.ends, [CurveEnd::Boundary, CurveEnd::Boundary]);
    branch.clone()
}

/// The middle of an interval.
fn mid(x: Interval) -> f64 {
    x.lo() / 2.0 + x.hi() / 2.0
}

/// Whether `x` holds `at`.
fn holds(x: Interval, at: f64) -> bool {
    x.lo() <= at && at <= x.hi()
}

/// The vertices in order along the edge, by the middle of parameter `t`, each place once.
fn in_order(branch: &Branch, t: usize) -> Vec<[Interval; 4]> {
    let mut out = branch.vertices.clone();
    out.sort_by(|a, b| mid(a[t]).total_cmp(&mid(b[t])));
    out.dedup_by(|a, b| (mid(a[t]) - mid(b[t])).abs() <= 1e-9);
    out
}

/// The bits of the other surface's `(s, r)` at each vertex strictly between the branch's two
/// ends: the boxes the proof's pieces met in at its cuts.
fn inner_bits(vertices: &[[Interval; 4]], [s, r]: [usize; 2]) -> Vec<[u64; 4]> {
    let inner = vertices.get(1..vertices.len() - 1).unwrap_or_default();
    inner
        .iter()
        .map(|v| {
            [
                v[s].lo().to_bits(),
                v[s].hi().to_bits(),
                v[r].lo().to_bits(),
                v[r].hi().to_bits(),
            ]
        })
        .collect()
}

/// The other plane's two vectors, a step and a little more long: a tilt of 2^-12 of a step in y
/// on each, so the plane leans off y = 0 and every product of the normal equations counts. Its
/// determinants of pairs of rows are −4·2^-12, −8 and 4·2^-12 (whatever sign the columns take): the
/// rows solved are x and z, the pair whose determinant is largest, and y is left over. Over a
/// piece, y's residual is a 2^-12 part of the box's travel, within the floor's eighth; x's or z's
/// would be the whole travel, and no piece would be proven.
const TILT: f64 = 1.0 / 4096.0;
const A: [f64; 3] = [1.0, TILT, 3.0];
const B: [f64; 3] = [3.0, -TILT, 1.0];

/// The other plane through the origin on A and B, over s from 21 to 23 units and r a quarter of a
/// unit either side of 22, in steps; and a plane standing on it, along A × B and A, whose edge
/// u = 0 from v = 10 to 12 is the line s = (v + 11) units, r = 22 units of the other plane, from one
/// end of its s to the other.
fn planes() -> (SurfaceKind, SurfaceKind) {
    let other = Plane::new(
        [0.0; 3],
        A,
        B,
        ((21.0 * UNIT, 23.0 * UNIT), (21.75 * UNIT, 22.25 * UNIT)),
    )
    .unwrap();
    let origin = [0, 1, 2].map(|k| UNIT * (11.0 * A[k] + 22.0 * B[k]));
    let normal = [4.0 * TILT, 8.0, -4.0 * TILT].map(|c| c * UNIT / 8.0);
    let own = Plane::new(
        origin,
        normal,
        A.map(|c| c * UNIT),
        ((0.0, 1.0), (10.0, 12.0)),
    )
    .unwrap();
    (SurfaceKind::Plane(own), SurfaceKind::Plane(other))
}

/// The bits of the other plane's `(s, r)` at the seven cuts between the eight pieces, with the
/// standing plane first, and second.
const LINE_BITS_SWAPPED: [[u64; 4]; 7] = [
    [
        0x40d5_37ff_ffff_fff1,
        0x40d5_4800_0000_0010,
        0x40d5_e7ff_ffff_fff2,
        0x40d6_1800_0000_000e,
    ],
    [
        0x40d5_77ff_ffff_fff1,
        0x40d5_8800_0000_0011,
        0x40d5_e7ff_ffff_fff2,
        0x40d6_1800_0000_000e,
    ],
    [
        0x40d5_b7ff_ffff_fff7,
        0x40d5_c800_0000_0011,
        0x40d5_e7ff_ffff_fff7,
        0x40d6_1800_0000_0005,
    ],
    [
        0x40d5_f7ff_ffff_fffb,
        0x40d6_0800_0000_0005,
        0x40d5_e7ff_ffff_fffb,
        0x40d6_1800_0000_0005,
    ],
    [
        0x40d6_37ff_ffff_ffef,
        0x40d6_4800_0000_0009,
        0x40d5_e7ff_ffff_fffb,
        0x40d6_1800_0000_0009,
    ],
    [
        0x40d6_77ff_ffff_ffed,
        0x40d6_8800_0000_0011,
        0x40d5_e7ff_ffff_fff1,
        0x40d6_1800_0000_000e,
    ],
    [
        0x40d6_b7ff_ffff_ffed,
        0x40d6_c800_0000_0011,
        0x40d5_e7ff_ffff_fff1,
        0x40d6_1800_0000_000f,
    ],
];
const LINE_BITS: [[u64; 4]; 7] = [
    [
        0x40d5_37ff_ffff_fffd,
        0x40d5_4800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d5_77ff_ffff_fffd,
        0x40d5_8800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d5_b7ff_ffff_fffd,
        0x40d5_c800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d5_f7ff_ffff_fffd,
        0x40d6_0800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d6_37ff_ffff_fffd,
        0x40d6_4800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d6_77ff_ffff_fffd,
        0x40d6_8800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
    [
        0x40d6_b7ff_ffff_fffd,
        0x40d6_c800_0000_0003,
        0x40d5_e7ff_ffff_fffd,
        0x40d6_1800_0000_0003,
    ],
];

/// The branch along the standing plane's edge, in both orders: on the line the edge's own u is 0,
/// s is its v and 11 more, in units, and r is 22 units. The edge is straight, so the branch is its
/// chords exactly, deviation 0. Its box across, in r, has a quarter of a unit and the room past
/// each end to stand in, so the edge is proven in eight equal pieces, and the boxes where they meet
/// have the recorded bits. Each piece's Gauss-Newton starts from the middle of the other
/// plane's domain, which is 21 units and more from zero.
#[test]
fn an_edge_along_a_tilted_plane_meets_in_the_recorded_boxes() {
    let (own, other) = planes();
    let mut bits = Vec::new();
    for own_first in [true, false] {
        let (found, [u, t, s, r]) = if own_first {
            (branch(&own, &other), [0, 1, 2, 3])
        } else {
            (branch(&other, &own), [2, 3, 0, 1])
        };
        assert_eq!(found.deviation.to_bits(), 0, "{}", found.deviation);
        let vertices = in_order(&found, t);
        for v in &vertices {
            // The ends are the subdivision's starts, boxes about the line's end a few roundings
            // wide in t, so the line's s there is held to a millionth of a step.
            let at = mid(v[t]);
            let near = |x: Interval, at: f64| x.lo() - 1e-6 <= at && at <= x.hi() + 1e-6;
            assert!(holds(v[u], 0.0), "{v:?}");
            assert!(
                near(v[s], (at + 11.0) * UNIT) && near(v[r], 22.0 * UNIT),
                "{v:?}"
            );
        }
        // The run goes from the middle of one start's t to the other's: in this order they are
        // 10 and 12 exactly, and the cuts, halvings of whole numbers, are exact; in the other the
        // starts' boxes round to one side, and the cuts stand a few roundings off the eighths.
        let places: Vec<f64> = vertices.iter().map(|v| mid(v[t])).collect();
        for (k, place) in (0_u32..).zip(&places) {
            let eighth = 10.0 + f64::from(k) / 4.0;
            if own_first {
                assert_eq!(place.to_bits(), eighth.to_bits(), "{places:?}");
            }
            assert!((place - eighth).abs() <= 1e-12, "{places:?}");
        }
        assert_eq!(places.len(), 9, "{places:?}");
        bits.push(inner_bits(&vertices, [s, r]));
    }
    let recorded = [LINE_BITS.as_slice(), LINE_BITS_SWAPPED.as_slice()];
    assert_eq!(bits, recorded, "{bits:#x?}");
}

/// The places along the arc where the branch along it has a vertex, and its deviation, recorded.
const ARC_PIECES: usize = 16;
const ARC_DEVIATION: u64 = 0x4023_b51f_ba89_ff01;

/// The quarter of a turn of the circle of radius one unit about z, from an eighth of a turn to
/// three eighths, as the edge v = 0 of a cylinder whose turn is counted from 10 half-turns on, lies
/// in the plane z = 0. Each piece of the proof bows from its chord by the piece's length times
/// the width of the circle's velocity over it, so it is halved until that is within the ceiling:
/// the branch's deviation is within it, its vertices are the ends of equal pieces, and each holds
/// the circle's point there.
#[test]
fn an_arc_in_a_plane_is_proven_in_pieces_within_the_ceiling() {
    let frame = Frame::standard().unwrap();
    let tube = SurfaceKind::Cylinder(
        Cylinder::new([0.0; 3], UNIT, frame, ((10.25, 10.75), (0.0, UNIT))).unwrap(),
    );
    let floor = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [UNIT, 0.0, 0.0],
            [0.0, UNIT, 0.0],
            ((-2.0, 2.0), (-2.0, 2.0)),
        )
        .unwrap(),
    );
    for tube_first in [true, false] {
        let (found, [t, height, x, y]) = if tube_first {
            (branch(&tube, &floor), [0, 1, 2, 3])
        } else {
            (branch(&floor, &tube), [2, 3, 0, 1])
        };
        assert!(found.deviation <= CEILING, "{}", found.deviation);
        let vertices = in_order(&found, t);
        for v in &vertices {
            let angle = core::f64::consts::PI * mid(v[t]);
            let near = |x: Interval, at: f64| x.lo() - 1e-9 <= at && at <= x.hi() + 1e-9;
            assert!(holds(v[height], 0.0), "{v:?}");
            assert!(near(v[x], angle.cos()) && near(v[y], angle.sin()), "{v:?}");
        }
        let pieces = vertices.len() - 1;
        let count = |k: usize| f64::from(u32::try_from(k).unwrap());
        for (k, v) in vertices.iter().enumerate() {
            let even = 10.25 + 0.5 * count(k) / count(pieces);
            assert!((mid(v[t]) - even).abs() <= 1e-9, "{k} of {pieces}: {v:?}");
        }
        assert_eq!(
            (pieces, found.deviation.to_bits()),
            (ARC_PIECES, ARC_DEVIATION),
            "{pieces} {:#x}",
            found.deviation.to_bits()
        );
    }
}
