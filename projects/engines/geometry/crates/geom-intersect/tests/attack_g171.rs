//! Attacks on piece G171's claims: a bowl on a bowl is one isolated point, a saddle on its tangent
//! plane a crossing, a contact of higher order is never given a verdict, a cylinder on a plane a
//! band of the width 12.9 gives, and overlaps are read as overlaps. Lengths are in steps, so what a
//! reading rounds up to whole steps stays a measure.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::march::march;
use geom_intersect::result::{Coincidence, CurveContact, CurveEnd, PointContact, Test, Unresolved};
use geom_num::grid::Steps;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cylinder, Plane, SurfaceKind, Torus};

/// One unit of the shapes below.
const UNIT: f64 = 1024.0;
const CEILING: f64 = 1024.0;

/// A meter above what any case here takes, so a search that loses its way fails, not hangs.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 23))
}

/// z = h + (a·x² + b·y²)/`UNIT` over [−r, r]², exact in Bézier form.
fn graph(r: f64, h: f64, a: f64, b: f64) -> SurfaceKind {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    let square = r * r / UNIT;
    for (i, x) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, y) in [-r, 0.0, r].into_iter().enumerate() {
            let w = |k: usize| if k == 1 { -square } else { square };
            net.push([x, y, h + a * w(i) + b * w(j)]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// z = c·(x⁴ + y⁴)/r⁴ + d·y³/r³ over [−r, r]², exact in Bézier form.
fn quartic(r: f64, c: f64, d: f64) -> SurfaceKind {
    let basis = || Basis::new(4, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    // The Bernstein coefficients of (2t − 1)⁴ and (2t − 1)³ at degree 4.
    let four = [1.0, -1.0, 1.0, -1.0, 1.0];
    let three = [-1.0, 0.5, 0.0, -0.5, 1.0];
    let mut net = Vec::new();
    for i in 0..5 {
        for j in 0..5 {
            let x = r * (i as f64 / 2.0 - 1.0);
            let y = r * (j as f64 / 2.0 - 1.0);
            net.push([x, y, c * (four[i] + four[j]) + d * three[j]]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// The plane z = `z` over `domain`, off-centre so that no cut falls on a contact.
fn flat(domain: ((f64, f64), (f64, f64)), z: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new([64.0, 32.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap(),
    )
}

/// A plane past both ends of a shape of half-width `r`.
fn wide(r: f64, z: f64) -> SurfaceKind {
    flat(((-2.0 * r, 2.0 * r), (-2.0 * r, 2.0 * r)), z)
}

/// A cylinder of radius 5 units along y, resting on z = 0.
fn resting() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0, 0.0, 5.0 * UNIT],
            5.0 * UNIT,
            Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
            ((0.1, 0.9), (-UNIT, UNIT)),
        )
        .unwrap(),
    )
}

fn read(a: &SurfaceKind, b: &SurfaceKind, floor: f64, ceiling: f64) -> Vec<Reading> {
    let found = subdivide(a, b, floor, &mut meter()).unwrap();
    assert_ne!(found.flagged.len(), 0, "{found:?}");
    analyse(a, b, &found, floor, ceiling, &mut meter()).unwrap()
}

fn nothing() -> Subdivided {
    Subdivided {
        settled: Vec::new(),
        flagged: Vec::new(),
    }
}

#[test]
fn a_bowl_on_a_bowl_is_one_isolated_point() {
    let low = graph(2.0 * UNIT, 0.0, 1.0, 1.0);
    let high = graph(2.0 * UNIT, 0.0, -1.0, -1.0);
    let readings = read(&low, &high, 8.0, CEILING);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Point { at, gap, contact } = readings[0] else {
        panic!("{readings:?}");
    };
    assert_eq!(contact, PointContact::IsolatedTangent);
    assert!(gap.contains(0.0), "{gap:?}");
    for x in at {
        assert!(x.contains(0.5), "{at:?}");
    }
}

#[test]
fn a_saddle_on_its_tangent_plane_is_a_crossing_of_four_half_branches() {
    let saddle = graph(2.0 * UNIT, 0.0, 1.0, -2.0);
    let plane = wide(2.0 * UNIT, 0.0);
    let floor = 8.0;
    let found = subdivide(&saddle, &plane, floor, &mut meter()).unwrap();
    let readings = analyse(&saddle, &plane, &found, floor, CEILING, &mut meter()).unwrap();
    assert_eq!(readings.len(), 1, "{readings:?}");
    assert!(
        matches!(
            readings[0],
            Reading::Point {
                contact: PointContact::Singular { branches: 4 },
                ..
            }
        ),
        "{readings:?}"
    );
    // All four half-branches are traced, each to the flagged pairs around the crossing.
    let traced = march(&saddle, &plane, &found, floor, CEILING, &mut meter()).unwrap();
    let reaching = traced
        .branches
        .iter()
        .filter(|b| b.ends.contains(&CurveEnd::Boundary) && b.ends.contains(&CurveEnd::Flagged))
        .count();
    assert_eq!(reaching, 4, "{:?}", traced.branches.len());
    assert_eq!(traced.unproven, Vec::new());
}

/// The same contact with the surfaces the other way about, so each surface's form takes each sign.
#[test]
fn the_reading_does_not_depend_on_the_order() {
    let low = graph(2.0 * UNIT, 0.0, 1.0, 1.0);
    let high = graph(2.0 * UNIT, 0.0, -1.0, -3.0);
    let readings = read(&high, &low, 8.0, CEILING);
    assert!(matches!(
        readings.as_slice(),
        [Reading::Point {
            contact: PointContact::IsolatedTangent,
            ..
        }]
    ));
}

/// Parameters that shear across each other: a form taken in the wrong frame changes its sign here.
#[test]
fn a_form_is_read_in_the_frame_of_the_tangent_plane_not_of_the_parameters() {
    let basis = || Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let r = 2.0 * UNIT;
    let shear = 2.0;
    // z = (x² − 2·y²)/UNIT, in parameters that run at a slant: the net of a sheared graph.
    let mut net = Vec::new();
    for (i, s) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, t) in [-r, 0.0, r].into_iter().enumerate() {
            let w = |k: usize| if k == 1 { -r * r / UNIT } else { r * r / UNIT };
            let (x, y) = (s + shear * t, t);
            let along = w(i) + 2.0 * shear * s * t / UNIT + shear * shear * w(j);
            net.push([x, y, along - 2.0 * w(j)]);
        }
    }
    let saddle = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let plane = wide(4.0 * UNIT, 0.0);
    let readings = read(&saddle, &plane, 8.0, CEILING);
    assert!(
        readings.iter().any(|r| matches!(
            r,
            Reading::Point {
                contact: PointContact::Singular { branches: 4 },
                ..
            }
        )),
        "{readings:?}"
    );
    // And a bowl in those parameters is one point.
    let mut net = Vec::new();
    for (i, s) in [-r, 0.0, r].into_iter().enumerate() {
        for (j, t) in [-r, 0.0, r].into_iter().enumerate() {
            let w = |k: usize| if k == 1 { -r * r / UNIT } else { r * r / UNIT };
            let (x, y) = (s + shear * t, t);
            let along = w(i) + 2.0 * shear * s * t / UNIT + shear * shear * w(j);
            net.push([x, y, along + w(j)]);
        }
    }
    let bowl = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let readings = read(&bowl, &plane, 8.0, CEILING);
    assert!(
        matches!(
            readings.as_slice(),
            [Reading::Point {
                contact: PointContact::IsolatedTangent,
                ..
            }]
        ),
        "{readings:?}"
    );
}

/// A gap clear of zero is a near miss, whatever the forms say.
#[test]
fn two_bowls_a_hair_apart_are_a_near_miss() {
    let low = graph(2.0 * UNIT, -1.0, 1.0, 1.0);
    let high = graph(2.0 * UNIT, 1.0, -1.0, -1.0);
    let readings = read(&low, &high, 128.0, CEILING);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Undecided {
        why: Unresolved::NearMiss { gap },
        ..
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    assert_eq!((gap.lo(), gap.hi()), (Steps(0), Steps(3)));
}

/// x⁴ + y⁴ on a plane: no form says anything, and it is not an overlap, because it parts all round.
#[test]
fn a_contact_of_fourth_order_in_the_open_stays_undecided() {
    let r = 2.0 * UNIT;
    let bump = quartic(r, UNIT, 0.0);
    let plane = wide(r, 0.0);
    let readings = read(&bump, &plane, 8.0, CEILING);
    assert_eq!(readings.len(), 1, "{readings:?}");
    assert!(
        matches!(
            readings[0],
            Reading::Undecided {
                why: Unresolved::HigherOrderContact,
                ..
            }
        ),
        "{readings:?}"
    );
}

/// The same contact cut off at each side of either surface's domain does reach an edge.
#[test]
fn a_flat_contact_at_an_edge_is_an_overlap_on_any_side_of_either_surface() {
    let r = 2.0 * UNIT;
    let bump = quartic(r, UNIT, 0.0);
    let (near, far) = (-512.0, 2.0 * r);
    let plates = [
        ((-96.0 - 64.0, far), (-far, far)),
        ((-far, 64.0 + 96.0 - 128.0), (-far, far)),
        ((-far, far), (near, far)),
        ((-far, far), (-far, 512.0)),
    ];
    for domain in plates {
        let plate = flat(domain, 0.0);
        for (a, b) in [(&bump, &plate), (&plate, &bump)] {
            let readings = read(a, b, 8.0, CEILING);
            assert!(
                readings.iter().all(|r| matches!(
                    r,
                    Reading::Overlap { .. }
                        | Reading::Undecided {
                            why: Unresolved::HigherOrderContact,
                            ..
                        }
                )),
                "{readings:?}"
            );
        }
    }
}

/// z = x² + y³ on a plane is a cusp, and a NURBS form can't be proven degenerate, so it stays flagged.
#[test]
fn a_cusp_on_nurbs_stays_flagged() {
    let r = 2.0 * UNIT;
    let basis = || Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let mut net = Vec::new();
    // (2t − 1)² and (2t − 1)³ at degree 3.
    let two = [1.0, -1.0 / 3.0, -1.0 / 3.0, 1.0];
    let three = [-1.0, 1.0, -1.0, 1.0];
    for i in 0..4 {
        for j in 0..4 {
            let x = r * (i as f64 * 2.0 / 3.0 - 1.0);
            let y = r * (j as f64 * 2.0 / 3.0 - 1.0);
            net.push([x, y, UNIT * two[i] + 2.0 * UNIT * three[j]]);
        }
    }
    let cusp = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let plane = wide(r, 0.0);
    let readings = read(&cusp, &plane, 8.0, CEILING);
    assert!(
        matches!(
            readings.as_slice(),
            [Reading::Undecided {
                why: Unresolved::HigherOrderContact,
                ..
            }]
        ),
        "{readings:?}"
    );
}

/// A torus and a cylinder whose curvatures agree in one direction, off the torus's plane of
/// symmetry: the contact can't be isolated, and two half-branches leave it, on one side only.
#[test]
fn a_cusp_on_analytic_surfaces_has_the_two_half_branches_that_leave_it() {
    let (major, minor) = (3.0 * UNIT, UNIT);
    let (cos, sin) = (0.5f64, 3.0f64.sqrt() / 2.0);
    let torus = SurfaceKind::Torus(
        Torus::new(
            [0.0, 0.0, 0.0],
            major,
            minor,
            Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            ((-0.25, 0.25), (1.0 / 3.0 - 0.25, 1.0 / 3.0 + 0.25)),
        )
        .unwrap(),
    );
    // The contact point, its normal, and the torus's principal directions there.
    let at = [major + minor * cos, 0.0, minor * sin];
    let n = [cos, 0.0, sin];
    let (p, t) = ([0.0, 1.0, 0.0], [-sin, 0.0, cos]);
    // Curved along c, at 45° between them, and radius 4 units makes the forms' difference singular.
    let half = 0.5f64.sqrt();
    let c: [f64; 3] = core::array::from_fn(|i| half * (p[i] + t[i]));
    let w: [f64; 3] = core::array::from_fn(|i| half * (t[i] - p[i]));
    let radius = 4.0 * UNIT;
    let centre: [f64; 3] = core::array::from_fn(|i| at[i] - radius * n[i]);
    let cylinder = SurfaceKind::Cylinder(
        Cylinder::new(
            centre,
            radius,
            Frame::new(n, w).unwrap(),
            ((-0.2, 0.2), (-1.5 * UNIT, 1.5 * UNIT)),
        )
        .unwrap(),
    );
    assert_eq!(c.len(), 3);
    let readings = read(&torus, &cylinder, 8.0, CEILING);
    assert!(
        readings.iter().any(|r| matches!(
            r,
            Reading::Point {
                contact: PointContact::Singular { branches: 2 },
                ..
            }
        )),
        "{readings:?}"
    );
}

/// A cylinder resting on a plane touches along a line, in a band of √(2δ/Δκ).
#[test]
fn a_cylinder_on_a_plane_is_a_band_of_the_width_the_chapter_gives() {
    let cylinder = resting();
    let plane = wide(6.0 * UNIT, 0.0);
    let floor = 8.0;
    let readings = read(&cylinder, &plane, floor, CEILING);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Curve {
        contact: CurveContact::Tangent { band, gap },
        ..
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    // Δκ is at least 1/(5 units), less what the boxes spread it by; δ at most twice the floor.
    let least = (2.0f64 * floor * 5.0 * UNIT).sqrt();
    let most = 2.0 * (2.0f64 * 2.0 * floor * 5.0 * UNIT).sqrt();
    assert!(
        (band.0 as f64) >= least && (band.0 as f64) <= most,
        "{band:?} from {least} to {most}"
    );
    assert_eq!(gap.lo(), Steps(0));
    assert!(gap.hi().0 <= 2 * floor as u64, "{gap:?}");
}

/// A band past the ceiling is flagged with its width, never called a tangent curve.
#[test]
fn a_band_past_the_ceiling_is_flagged_with_its_width() {
    let cylinder = resting();
    let plane = wide(6.0 * UNIT, 0.0);
    let readings = read(&cylinder, &plane, 8.0, 16.0);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Undecided {
        why: Unresolved::TangentBand { width },
        ..
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    assert!(width.0 > 16, "{width:?}");
}

#[test]
fn identical_surfaces_overlap_by_their_data_alone() {
    let a = graph(2.0 * UNIT, 0.0, 1.0, 1.0);
    let b = a.clone();
    let readings = analyse(&a, &b, &nothing(), 8.0, CEILING, &mut meter()).unwrap();
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Overlap {
        first,
        second,
        coincidence,
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    assert_eq!(
        (first, second),
        (((0.0, 1.0), (0.0, 1.0)), ((0.0, 1.0), (0.0, 1.0)))
    );
    assert_eq!(coincidence, Coincidence::Identical);
}

/// Two surfaces too close to part at the floor overlap, within what the boxes reach.
#[test]
fn surfaces_closer_than_the_floor_overlap_within_their_reach() {
    let a = graph(UNIT, 0.0, 1.0, 1.0);
    let b = graph(UNIT, 1.0, 1.0, 1.0);
    let floor = 256.0;
    let readings = read(&a, &b, floor, CEILING);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Overlap {
        coincidence: Coincidence::Within(within),
        ..
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    assert!(within.0 > 1 && within.0 <= 2 * floor as u64, "{within:?}");
}

/// The same pair under a ceiling the boxes can't meet is near-coincident, not coincident.
#[test]
fn surfaces_past_the_ceiling_are_near_coincident() {
    let a = graph(UNIT, 0.0, 1.0, 1.0);
    let b = graph(UNIT, 1.0, 1.0, 1.0);
    let readings = read(&a, &b, 256.0, 2.0);
    assert_eq!(readings.len(), 1, "{readings:?}");
    let Reading::Undecided {
        why: Unresolved::NearCoincident { gap },
        ..
    } = readings[0]
    else {
        panic!("{readings:?}");
    };
    assert!(gap.hi().0 > 2, "{gap:?}");
}

/// A pair flagged for what its starts showed is left as it was flagged.
#[test]
fn a_pair_flagged_for_its_starts_keeps_its_reason() {
    // The line x = y runs from corner to corner of the slope.
    let r = 2.0 * UNIT;
    let basis = || Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let net: Vec<[f64; 3]> = [(-r, -r), (-r, r), (r, -r), (r, r)]
        .into_iter()
        .map(|(x, y)| [x, y, x - y])
        .collect();
    let slope = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
    let plane = wide(r, 0.0);
    let readings = read(&slope, &plane, 8.0, CEILING);
    assert!(
        readings.iter().all(|r| matches!(
            r,
            Reading::Undecided {
                why: Unresolved::UndecidedAtFloor {
                    test: Test::Exclusion
                },
                ..
            }
        )),
        "{readings:?}"
    );
}

#[test]
fn a_floor_or_ceiling_that_is_not_positive_is_refused() {
    let a = graph(2.0 * UNIT, 0.0, 1.0, 1.0);
    let b = graph(2.0 * UNIT, 0.0, -1.0, -1.0);
    for (floor, ceiling) in [(0.0, 1.0), (f64::NAN, 1.0), (1.0, 0.0), (1.0, f64::NAN)] {
        assert_eq!(
            analyse(&a, &b, &nothing(), floor, ceiling, &mut meter()),
            Err(IntersectError::InvalidInput)
        );
    }
}

/// An analysis's work and the size of its answer, to the unit: it is a pure function, so a change
/// here is a change to what it reads or how it joins, which the readings alone cannot show. A pin
/// holds at its own work and fails one unit under it.
#[test]
fn each_analysis_takes_the_work_it_always_took() {
    let r = 2.0 * UNIT;
    let cases: [(SurfaceKind, SurfaceKind, f64, u64, usize); 5] = [
        (
            graph(r, 0.0, 1.0, 1.0),
            graph(r, 0.0, -1.0, -1.0),
            8.0,
            1_520,
            1,
        ),
        (graph(r, 0.0, 1.0, -2.0), wide(r, 0.0), 8.0, 2_806, 1),
        (resting(), wide(6.0 * UNIT, 0.0), 8.0, 1_138_913, 1),
        (
            graph(UNIT, 0.0, 1.0, 1.0),
            graph(UNIT, 1.0, 1.0, 1.0),
            256.0,
            204_071,
            1,
        ),
        (quartic(r, UNIT, 0.0), wide(r, 0.0), 8.0, 46_557, 1),
    ];
    for (a, b, floor, work, count) in &cases {
        let found = subdivide(a, b, *floor, &mut meter()).unwrap();
        let run = |budget: u64| {
            let mut m = WorkMeter::new(Budget::of_work(budget));
            analyse(a, b, &found, *floor, CEILING, &mut m)
        };
        assert_eq!(run(work - 1), Err(IntersectError::BudgetExceeded));
        assert_eq!(run(*work).unwrap().len(), *count);
    }
}
