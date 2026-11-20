//! Recorded answers of geom-intersect's small steps, held exactly (chapters 12.2, 12.4 and 12.7):
//! the quartic two quadrics share, as the work its walk is charged and the bits of its arcs and of
//! its points on both sheets at each eighth of a turn; a torus's nearest points where the boxes
//! left undecided reach one end of a closed parameter, or both, as the bits of their parameters and
//! the work of finding them; and a bound whose count of steps lies just under a rounding midpoint.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly, and the answers are recorded bits"
)]

use geom_curve::Frame;
use geom_intersect::bound::{Terms, settle};
use geom_intersect::quartic::{Arc, meet};
use geom_intersect::result::{Item, ParamBox, PointContact};
use geom_intersect::solve::Field;
use geom_intersect::{IntersectError, Projection, project_surface};
use geom_math::Box3;
use geom_num::Exactness;
use geom_num::grid::{GridStep, P3, Steps};
use geom_num::interval::Interval;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Sphere, SurfaceKind, Torus};
use geom_test::golden::{hex, sha256};

const ROUND: (f64, f64) = (0.0, 2.0);
const TALL: (f64, f64) = (-5000.0, 5000.0);
const Z: [f64; 3] = [0.0, 0.0, 1.0];
const X: [f64; 3] = [1.0, 0.0, 0.0];

fn frame_along(n: [f64; 3]) -> Frame<3> {
    let x = if n[0] == 0.0 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    Frame::new(x, n).unwrap()
}

fn sphere(c: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(c, r, frame_along([0.0, 0.0, 1.0]), (ROUND, (-0.5, 0.5))).unwrap(),
    )
}

fn cylinder(c: [f64; 3], n: [f64; 3], r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new(c, r, frame_along(n), (ROUND, TALL)).unwrap())
}

fn cone(apex: [f64; 3], n: [f64; 3], k: f64) -> SurfaceKind {
    SurfaceKind::Cone(Cone::new(apex, 0.0, k, frame_along(n), (ROUND, TALL)).unwrap())
}

/// The digest of `bits`, as hex.
fn digest(bits: &[u64]) -> String {
    let bytes: Vec<u8> = bits.iter().flat_map(|b| b.to_le_bytes()).collect();
    hex(sha256(&bytes))
}

/// What `meet` finds for `a` and `b`: the work charged, and the digest of the bits of each arc's
/// ends, then of the point of each sheet at each eighth of a turn (both ends of each coordinate,
/// or a marker where there is none).
fn quartic(a: &SurfaceKind, b: &SurfaceKind) -> (u128, String) {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let found = meet(a, b, &mut meter).unwrap().unwrap();
    let mut out = Vec::new();
    for arc in &found.arcs {
        match arc {
            Arc::Whole => out.push(1),
            Arc::Between { from, to } => {
                out.extend([from.lo(), from.hi(), to.lo(), to.hi()].map(f64::to_bits));
            }
        }
    }
    if let Some(curve) = &found.curve {
        for k in 0..16 {
            let theta = Interval::point(f64::from(k) / 8.0).unwrap();
            for upper in [true, false] {
                match curve.point(theta, upper).unwrap() {
                    Some(p) => {
                        for x in p {
                            out.extend([x.lo().to_bits(), x.hi().to_bits()]);
                        }
                    }
                    None => out.push(3),
                }
            }
        }
    } else {
        out.push(2);
    }
    (meter.used().work, digest(&out))
}

/// Each case's recorded answer, the failures listed together.
fn holds(found: &[(u128, String)], want: &[(u128, &str)]) {
    let mut wrong = Vec::new();
    for (i, (got, want)) in found.iter().zip(want).enumerate() {
        if got.0 != want.0 || got.1 != want.1 {
            wrong.push(format!("case {i}: ({}, \"{}\")", got.0, got.1));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(found.len(), want.len());
}

#[test]
fn quartics_of_two_quadrics_are_their_recorded_walks_and_points() {
    // Two loops and one, exact and off the grid; a node; skew pipes one sample wide; a cone with a
    // cylinder round its axis and with one along a generator (a cubic and its line); two cones.
    let cases = vec![
        (sphere([0.0; 3], 5.0), cylinder([1.0, 0.0, 0.0], Z, 1.0)),
        (sphere([0.0; 3], 2.0), cylinder([1.0, 0.0, 0.0], Z, 1.0)),
        (
            cylinder([0.0; 3], Z, 1.0),
            cylinder([0.0, 1.0, 0.0], X, 1.0),
        ),
        (
            cylinder([0.0; 3], Z, 1.0),
            cylinder([0.0, 0.302, 0.0], X, 1.3),
        ),
        (cone([0.0; 3], Z, 1.0), cylinder([1.0, 0.0, 0.0], Z, 2.0)),
        (
            cone([0.0; 3], Z, 1.0),
            cylinder([0.0, 1.0, 0.0], [1.0, 0.0, 1.0], 1.0),
        ),
        (
            sphere([0.25, 0.5, 0.0], 5.3),
            cylinder([1.1, 0.0, 0.0], Z, 1.7),
        ),
        (
            cone([0.0; 3], Z, 1.0),
            cone([5.0, 0.0, 0.0], [1.0, 1.0, 0.0], 1.0),
        ),
        // On the grid and symmetric, where coefficients and pivots come out exactly equal or zero.
        (sphere([0.0; 3], 5.0), cylinder([5.0, 0.0, 0.0], Z, 1.0)),
        (
            sphere([7.0, 3.0, 2.0], 5.0),
            cylinder([12.0, 3.0, 2.0], Z, 1.0),
        ),
        (
            cylinder([0.0; 3], Z, 1.0),
            cylinder([0.0, 0.25, 0.0], X, 2.0),
        ),
        (
            cylinder([0.0; 3], Z, 1.0),
            cylinder([3.0, 0.0, 0.0], [3.0, 4.0, 0.0], 3.0),
        ),
        (
            sphere([0.0; 3], 5.0),
            cylinder([2.0, 0.0, 0.0], [0.0, 1.0, 1.0], 1.5),
        ),
        (cone([0.0; 3], Z, 1.0), sphere([1.0, 0.0, 0.0], 3.0)),
        (cone([0.0; 3], Z, 1.0), cylinder([1.0, 0.0, 0.0], Z, 1.0)),
        (cone([0.0; 3], Z, 1.0), cylinder([0.0, 3.1, 0.0], X, 3.7)),
        (cylinder([1.0, 0.0, 0.0], Z, 1.0), sphere([0.0; 3], 5.0)),
        (cylinder([0.0; 3], X, 2.0), sphere([0.0, 1.0, 0.0], 2.0)),
    ];
    let found: Vec<_> = cases.iter().map(|(a, b)| quartic(a, b)).collect();
    holds(&found, &QUARTICS);
}

const QUARTICS: [(u128, &str); 18] = [
    (
        64,
        "4aa9109c1b31ce5e4af7a1b8819e7ca8d1fa08e417affd918100f938f8150ea0",
    ),
    (
        150,
        "325200cb1b5b724d6e8068d8a608ccf595832d72d80f4787741d608bba1ac0fb",
    ),
    (
        142,
        "41124698bb471f8481b140fc57ba5edffcbe4ada8f083465806a476c526ec9c6",
    ),
    (
        130,
        "9de620f7ad86d69487a980a23f941aedc893ce3bda9d8574a34c6dcd0629c1ec",
    ),
    (
        64,
        "2806492a5048d0eda0f6128479de053397013dc53d96fc143def3d02d1b74299",
    ),
    (
        64,
        "3da17c751f5a582e224d2a6154b26759f3d00b0394798e25047e8e0b58e737b7",
    ),
    (
        64,
        "118f8203ca627402c4103a6fc86eeee2b613e1b5ff4ef0447ffcaa1ce970b08c",
    ),
    (
        64,
        "5de313398cd137d0a112f68e7e297f8bc800f53720d8cee21e50526df10afe74",
    ),
    (
        132,
        "16ffa8d1a39a12bdf86842d816507e21cb65fd2655fa12b9c715cbdef5778e8d",
    ),
    (
        132,
        "ffb46728b12be8b4768a14bb1879d357887da9fca518608fdbe54babbf078b7b",
    ),
    (
        64,
        "ecb52c0aa360d3089230b7bfc6807deb902454509da72f35a2c583e2cc33e41b",
    ),
    (
        141,
        "273b2d461cf5e405df73c2d2aeea65d91e4b85e667a1a9fa3100c66a0f5023eb",
    ),
    (
        216,
        "601116c3f91e45030918f20e29339766548f53f384827f92af86c265dd5bf261",
    ),
    (
        64,
        "471b330f4101d337fe3c15f2ce1302fe83974ee09c8f3860d9fae06e5e6b2bdb",
    ),
    (
        64,
        "ab303a87257cca9d127b47c91b304cbba14d37d6e4dd8d700962355c0ebfdc7b",
    ),
    (
        64,
        "4cb3900a1b880b3350ce93783255bab9c8266cc3e559f0e9e865432de89ee1ac",
    ),
    (
        64,
        "913afef352faaca6e7fa4a0f7355ba385bca5fd2f9bcf5aab55cf52a19c8e2e1",
    ),
    (
        136,
        "3caeb7294bf0708ef47f31d32f1b3e49fb9340a06f3b2f22f91d9621a104d91e",
    ),
];

/// A torus of radii 6 and 2 about the z axis on `domain`.
fn ring(domain: ((f64, f64), (f64, f64))) -> SurfaceKind {
    SurfaceKind::Torus(Torus::new([0.0; 3], 6.0, 2.0, Frame::standard().unwrap(), domain).unwrap())
}

/// The nearest points of `p` on `s`: the work, and the digest of how many and each one's
/// parameters, or of the enclosure where it is not a point.
fn nearest(s: &SurfaceKind, p: [f64; 3]) -> (u128, String) {
    let mut meter = WorkMeter::new(Budget::of_work(1 << 22));
    let found = project_surface(s, &p, 1e-6, &mut meter).unwrap();
    let mut out = Vec::new();
    match &found {
        Projection::Points(points) => {
            out.push(u64::try_from(points.len()).unwrap());
            for n in points {
                for x in n.at {
                    out.extend([x.lo().to_bits(), x.hi().to_bits()]);
                }
            }
        }
        Projection::NonIsolated { enclosure } => {
            out.push(u64::MAX);
            for x in enclosure {
                out.extend([x.lo().to_bits(), x.hi().to_bits()]);
            }
        }
        other @ Projection::Set { .. } => panic!("{other:?}"),
    }
    (meter.used().work, digest(&out))
}

#[test]
fn a_torus_splits_its_undecided_boxes_only_across_a_seam_they_reach_both_sides_of() {
    // A strip of the tube a few floors wide, v in [-1e-5, 1e-5], seen from the tube's centre: every
    // point of it is 2 away, so the boxes left undecided run its whole width, an open side, and
    // their hull is an enclosure.
    let strip = (-1e-5, 1e-5);
    let cases = [
        // At u = 1/2, off the u seam.
        (ring((ROUND, strip)), [0.0, 6.0, 0.0]),
        // On the u seam of a torus on u in [-1, 1], at u = 1 and u = -1.
        (ring(((-1.0, 1.0), strip)), [-6.0, 0.0, 0.0]),
        // Just past u = 0 and just short of u = 2: boxes at one end of the seam only.
        (ring((ROUND, strip)), [6.0, 2e-6, 0.0]),
        (ring((ROUND, strip)), [6.0, -2e-6, 0.0]),
    ];
    let found: Vec<_> = cases.iter().map(|(s, p)| nearest(s, *p)).collect();
    holds(&found, &NEAREST);
}

const NEAREST: [(u128, &str); 4] = [
    (
        2_583_747,
        "e9e2f3d39476335d6875bf63b440572d5da54371ccae8f0c690676183da306bf",
    ),
    (
        3_026_565,
        "c4d98e714133f4f94cbd8a3e20d2d520c5a1ed65195cfe98026fbcc157ce2a07",
    ),
    (
        2_812_021,
        "9a6fc0280afc726f406e6303aa68aa2b7b2e41a164bd8a3dd57b946cdf0f34fd",
    ),
    (
        2_812_021,
        "9a6fc0280afc726f406e6303aa68aa2b7b2e41a164bd8a3dd57b946cdf0f34fd",
    ),
];

fn p3(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

#[test]
fn a_bound_on_a_count_just_under_a_rounding_midpoint_is_read() {
    // 3·2^51 − 1 steps: a double, and one ε more of it lies just under the midpoint above the next
    // double, so `count`'s interval is [n(1 − ε), n(1 + ε)] rounded, its ends in order.
    let n = 3 * (1_u64 << 51) - 1;
    let terms = Terms {
        operands: [Steps(n - 1), Steps(1)],
        sine: 1.0,
        curvature: 0.0,
        step: GridStep::Q0,
        xyz: Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap(),
    };
    let item = Item::Point {
        at: ParamBox::new(&[Interval::point(0.5).unwrap(); 2]).unwrap(),
        xyz: Box3::new(p3(0, 0, 0), p3(1, 1, 1)).unwrap(),
        contact: PointContact::Crossing,
        class: Exactness::Certified {
            bound: Steps(n),
            step: GridStep::Q0,
        },
    };
    let settled = settle(item, &terms, Steps(u64::MAX)).unwrap();
    let Item::Point { class, .. } = settled else {
        panic!("{settled:?}");
    };
    assert_eq!(
        class,
        Exactness::Certified {
            bound: Steps(13_510_798_882_111_492),
            step: GridStep::Q0
        }
    );
}

/// A field that keeps every default: x ↦ x.
struct Plain;

impl Field<2> for Plain {
    fn at(&self, x: [f64; 2], _meter: &mut WorkMeter<'_>) -> Result<[Interval; 2], IntersectError> {
        Ok([Interval::point(x[0])?, Interval::point(x[1])?])
    }

    fn over(
        &self,
        x: &[Interval; 2],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 2]>, IntersectError> {
        Ok(Some(*x))
    }

    fn slope(
        &self,
        _x: &[Interval; 2],
        _meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 2]; 2]>, IntersectError> {
        let (one, zero) = (Interval::point(1.0)?, Interval::point(0.0)?);
        Ok(Some([[one, zero], [zero, one]]))
    }

    fn reach(&self, _x: &[Interval; 2], _meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        Ok(1.0)
    }
}

#[test]
fn a_fields_default_weights_are_ones() {
    // Any positive weights prove a contraction; the default is the plain norm's, one each.
    assert_eq!(Plain.weights(), [1.0; 2]);
}
