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
use geom_intersect::general::{Settled, Subdivided, subdivide};
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

/// The other plane's two vectors for a tie: whole numbers, none zero, whose three determinants of
/// pairs of rows are all −7. The rows solved are the first pair of a tie, x and y, and z is left
/// over; the normal equations of the middle's Gauss-Newton have every product counting
/// (`[[26, 23], [23, 26]]`).
const TIED_A: [f64; 3] = [-4.0, -1.0, 3.0];
const TIED_B: [f64; 3] = [-3.0, 1.0, 4.0];

/// The places of the branch's vertices along the edge, and the bits of the first and last cut's
/// `(s, r)`, with the standing plane first, and second.
const TIED_BITS: [(usize, [u64; 4], [u64; 4]); 2] = [
    (
        129,
        [
            0x40d5_0007_ffff_fffd,
            0x40d5_0008_0000_0003,
            0x40d5_fffb_6db6_db6b,
            0x40d6_0004_9249_2495,
        ],
        [
            0x40d5_03f7_ffff_fffd,
            0x40d5_03f8_0000_0003,
            0x40d5_fffb_6db6_db6b,
            0x40d6_0004_9249_2495,
        ],
    ),
    (
        129,
        [
            0x40d5_0007_ffff_fff9,
            0x40d5_0008_0000_0008,
            0x40d5_fffb_6db6_db67,
            0x40d6_0004_9249_2499,
        ],
        [
            0x40d5_03f7_ffff_fff8,
            0x40d5_03f8_0000_0007,
            0x40d5_fffb_6db6_db67,
            0x40d6_0004_9249_249a,
        ],
    ),
];

/// An edge a sixty-fourth of a unit long, from v = 10 to 10 + 1/64, of a plane standing on the
/// other plane on vectors `along` and `across` along their cross product, which for each pair here
/// lies along (−1, 1, −1): the line s = (v + 11) units, r = 22 units, from one end of the other
/// plane's s to the other. Short, so that the left-over row, whose coefficients are whole numbers
/// and not a tilt, is held within the floor's eighth. Gives, with the standing plane first and then
/// second, how many vertices the branch has and the bits of its first and last cut's `(s, r)`.
fn tied_rows(along: [f64; 3], across: [f64; 3]) -> Vec<(usize, [u64; 4], [u64; 4])> {
    let length = 1.0 / 64.0;
    let other = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            along,
            across,
            (
                (21.0 * UNIT, (21.0 + length) * UNIT),
                (21.75 * UNIT, 22.25 * UNIT),
            ),
        )
        .unwrap(),
    );
    let origin = [0, 1, 2].map(|k| UNIT * (11.0 * along[k] + 22.0 * across[k]));
    let own = SurfaceKind::Plane(
        Plane::new(
            origin,
            [-UNIT, UNIT, -UNIT],
            along.map(|c| c * UNIT),
            ((0.0, 1.0), (10.0, 10.0 + length)),
        )
        .unwrap(),
    );
    let mut got = Vec::new();
    for own_first in [true, false] {
        let (found, [u, t, s, r]) = if own_first {
            (branch(&own, &other), [0, 1, 2, 3])
        } else {
            (branch(&other, &own), [2, 3, 0, 1])
        };
        assert_eq!(found.deviation.to_bits(), 0, "{}", found.deviation);
        let vertices = in_order(&found, t);
        for v in &vertices {
            let at = mid(v[t]);
            let near = |x: Interval, at: f64| x.lo() - 1e-6 <= at && at <= x.hi() + 1e-6;
            assert!(holds(v[u], 0.0), "{v:?}");
            assert!(
                near(v[s], (at + 11.0) * UNIT) && near(v[r], 22.0 * UNIT),
                "{v:?}"
            );
        }
        let bits = inner_bits(&vertices, [s, r]);
        let (Some(first), Some(last)) = (bits.first(), bits.last()) else {
            panic!("{vertices:?}");
        };
        got.push((vertices.len(), *first, *last));
    }
    got
}

/// The run along an edge in the plane whose rows tie at −7 takes 128 pieces, an eighth of a step
/// each, and meets in the recorded boxes.
#[test]
fn an_edge_along_a_plane_with_tied_rows_meets_in_the_recorded_boxes() {
    let got = tied_rows(TIED_A, TIED_B);
    assert_eq!(got, TIED_BITS, "{got:#x?}");
}

/// Two more ties of the three determinants, with the solved pair x and y and z left over, whose
/// last determinant `A1·B2 − B1·A2` has larger products than the first tie's: here 30 less 16,
/// at a tie of 14, and in the third 2 less 9 (in the vectors' signs), at a tie of 7. A slip in
/// that determinant that leaves it larger than the tie moves the rows solved to y and z, and
/// with them every box of the run; in the first tie no such slip leaves it so large. Between
/// them the two ties are moved by each of the four slips there, a product or the difference read
/// as a sum, or the first product as a quotient.
const TIED_TOO_A: [f64; 3] = [-2.0, -6.0, -4.0];
const TIED_TOO_B: [f64; 3] = [1.0, -4.0, -5.0];
const TIED_AGAIN_A: [f64; 3] = [2.0, -1.0, -3.0];
const TIED_AGAIN_B: [f64; 3] = [1.0, 3.0, 2.0];

/// The places of the branch's vertices along the edge, and the bits of the first and last cut's
/// `(s, r)`, with the standing plane first, and second, for the third tie.
const TIED_AGAIN_BITS: [(usize, [u64; 4], [u64; 4]); 2] = [
    (
        65,
        [
            0x40d5_000f_ffff_fffd,
            0x40d5_0010_0000_0003,
            0x40d5_fffb_6db6_db6b,
            0x40d6_0004_9249_2495,
        ],
        [
            0x40d5_03ef_ffff_fffd,
            0x40d5_03f0_0000_0003,
            0x40d5_fffb_6db6_db6b,
            0x40d6_0004_9249_2495,
        ],
    ),
    (
        65,
        [
            0x40d5_000f_ffff_fff4,
            0x40d5_0010_0000_000e,
            0x40d5_fffb_6db6_db66,
            0x40d6_0004_9249_249a,
        ],
        [
            0x40d5_03ef_ffff_fff2,
            0x40d5_03f0_0000_000c,
            0x40d5_fffb_6db6_db66,
            0x40d6_0004_9249_249a,
        ],
    ),
];

/// The run along an edge in the plane of the third tie takes 64 pieces, a fourth of a step each,
/// and meets in the recorded boxes.
#[test]
fn an_edge_along_a_plane_whose_rows_tie_again_meets_in_the_recorded_boxes() {
    let got = tied_rows(TIED_AGAIN_A, TIED_AGAIN_B);
    assert_eq!(got, TIED_AGAIN_BITS, "{got:#x?}");
}

/// The places of the branch's vertices along the edge, and the bits of the first and last cut's
/// `(s, r)`, with the standing plane first, and second, for the second tie.
const TIED_TOO_BITS: [(usize, [u64; 4], [u64; 4]); 2] = [
    (
        257,
        [
            0x40d5_0003_ffff_fffd,
            0x40d5_0004_0000_0003,
            0x40d5_fffc_9249_248f,
            0x40d6_0003_6db6_db71,
        ],
        [
            0x40d5_03fb_ffff_fffd,
            0x40d5_03fc_0000_0003,
            0x40d5_fffc_9249_248f,
            0x40d6_0003_6db6_db71,
        ],
    ),
    (
        257,
        [
            0x40d5_0003_ffff_fffc,
            0x40d5_0004_0000_0006,
            0x40d5_fffc_9249_248b,
            0x40d6_0003_6db6_db75,
        ],
        [
            0x40d5_03fb_ffff_fffa,
            0x40d5_03fc_0000_0004,
            0x40d5_fffc_9249_248b,
            0x40d6_0003_6db6_db75,
        ],
    ),
];

/// The run along an edge in the plane of the second tie takes 256 pieces, a sixteenth of a step
/// each, and meets in the recorded boxes.
#[test]
fn an_edge_along_a_plane_whose_last_rows_tie_meets_in_the_recorded_boxes() {
    let got = tied_rows(TIED_TOO_A, TIED_TOO_B);
    assert_eq!(got, TIED_TOO_BITS, "{got:#x?}");
}

/// The tilted pair's point of the standing plane's edge u = 0 at `v`, as a start: the line s =
/// (v + 11) units, r = 22 units of the other plane, every number exact.
fn start_on_the_edge(v: f64) -> [Interval; 4] {
    let at = |x: f64| Interval::new(x, x).unwrap();
    [at(0.0), at(v), at((v + 11.0) * UNIT), at(22.0 * UNIT)]
}

/// Two starts of the tilted pair on the standing plane's edge u = 0, both past the end v = 12 of
/// its domain, in the room the subdivision searches an edge's ends in: the zeros it reads on an
/// end of the other surface's domain are not cut back to the patch. Between them the edge stands
/// outside its own domain, so no run along it is tried: the march goes on, walks neither to the
/// other, and leaves the pair unproven.
#[test]
fn two_starts_past_the_end_of_an_edge_have_no_run_between_them() {
    let (own, other) = planes();
    let found = Subdivided {
        settled: vec![Settled {
            first: ((0.0, 1.0), (10.0, 12.0)),
            second: ((21.0 * UNIT, 23.0 * UNIT), (21.75 * UNIT, 22.25 * UNIT)),
            starts: vec![
                start_on_the_edge(12.0 + 1.0 / 64.0),
                start_on_the_edge(12.0 + 1.0 / 32.0),
            ],
        }],
        flagged: Vec::new(),
    };
    let traced = march(&own, &other, &found, FLOOR, CEILING, &mut meter()).unwrap();
    assert!(traced.branches.is_empty(), "{:#?}", traced.branches);
    assert_eq!(traced.unproven, found.settled, "{:#?}", traced.unproven);
}

/// Where the other plane's parameters stand on the edge of the next case: ten million, where a
/// double's spacing is 2^-29.
const FAR: f64 = 10_000_000.0;

/// How far apart, along the edge, the next case's two starts stand: 2^-33, a sixteenth of a
/// double's spacing at `FAR`.
const HAIR: f64 = 1.0 / 8_589_934_592.0;

/// The work the march along the short edge of the next case takes, recorded.
const HAIR_WORK: u128 = 181;

/// Two starts a hair apart on the edge u = 0 of a plane standing on another, whose parameters on
/// the edge are (`FAR` + v, `FAR` − v): the other plane on (−1, 3, 3) and (3, −2, 1) through the
/// origin, the own plane on their cross product and their difference from the other's point
/// (`FAR`, `FAR`). The run between the starts is one piece, whose travel in the other plane's
/// parameters is below their rounding: its first box, a little more than the first correction,
/// rounds to no wider than the residual's own rounding and cannot hold Krawczyk's image, and is
/// doubled until it does. The branch is the two starts, with the recorded work.
#[test]
fn two_starts_a_hair_apart_far_from_zero_are_joined_along_their_edge() {
    let (along, across) = ([-1.0, 3.0, 3.0], [3.0, -2.0, 1.0]);
    let on = |s: f64, r: f64| [0, 1, 2].map(|k| s * along[k] + r * across[k]);
    let wide = ((FAR - 1.0, FAR + 1.0), (FAR - 1.0, FAR + 1.0));
    let other = SurfaceKind::Plane(Plane::new([0.0; 3], along, across, wide).unwrap());
    let own = SurfaceKind::Plane(
        Plane::new(
            on(FAR, FAR),
            [9.0, 10.0, -7.0],
            on(1.0, -1.0),
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    let at = |x: f64| Interval::new(x, x).unwrap();
    let (here, there) = (0.5, 0.5 + HAIR);
    // The second start's (s, r) are not doubles: each is held between the two doubles about it.
    let starts = vec![
        [at(0.0), at(here), at(FAR + here), at(FAR - here)],
        [
            at(0.0),
            at(there),
            Interval::new(FAR + here, (FAR + here).next_up()).unwrap(),
            Interval::new((FAR - here).next_down(), FAR - here).unwrap(),
        ],
    ];
    let found = Subdivided {
        settled: vec![Settled {
            first: ((0.0, 1.0), (0.0, 1.0)),
            second: wide,
            starts: starts.clone(),
        }],
        flagged: Vec::new(),
    };
    let mut work = meter();
    let traced = march(&own, &other, &found, FLOOR, CEILING, &mut work).unwrap();
    assert!(traced.unproven.is_empty(), "{:#?}", traced.unproven);
    let [branch] = traced.branches.as_slice() else {
        panic!("{:#?}", traced.branches);
    };
    assert_eq!(branch.vertices, starts, "{:#?}", branch.vertices);
    assert_eq!(branch.deviation.to_bits(), 0, "{}", branch.deviation);
    assert_eq!(work.used().work, HAIR_WORK, "{:?}", work.used());
}
