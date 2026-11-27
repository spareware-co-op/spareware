//! A railing's pieces: a post at every vertex, a piece of rail along every piece of line, every
//! baluster, each a flat profile swept along a vector, on the grid; a rail's section on a slope
//! and at an angle stretched so its true section is its profile.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, at, laid, laid_stair, mm, on_stair, sketched, spaced, stair};
use construct_railings::{Part, Profile, RailingType, Side, StairHost, shape};

/// A 3 m railing at 100 mm: two posts 50 mm square and 1000 mm high, one piece of rail 50 mm
/// square with its top at 1000 mm, and 29 balusters 20 mm square from 50 mm to 950 mm.
#[test]
fn a_straight_railing_is_its_posts_rail_and_balusters() {
    let railing = sketched(spaced(100), vec![at(0, 0), at(3000, 0)]);
    let pieces = shape(railing.kind(), &laid(&railing, None)).unwrap();
    let parts = |p: Part| pieces.iter().filter(|x| x.part == p).count();
    assert_eq!(
        (
            parts(Part::Post),
            parts(Part::TopRail),
            parts(Part::Baluster)
        ),
        (2, 1, 29)
    );
    let post = &pieces[0];
    assert_eq!(post.id, "post.1.1");
    assert_eq!(
        post.profile,
        vec![
            [-25 * MM, -25 * MM, 0],
            [25 * MM, -25 * MM, 0],
            [25 * MM, 25 * MM, 0],
            [-25 * MM, 25 * MM, 0]
        ]
    );
    assert_eq!(post.by, [0, 0, 1000 * MM]);
    let rail = pieces.iter().find(|p| p.id == "rail.1.1").unwrap();
    assert_eq!(
        rail.profile,
        vec![
            [0, -25 * MM, 1000 * MM],
            [0, 25 * MM, 1000 * MM],
            [0, 25 * MM, 950 * MM],
            [0, -25 * MM, 950 * MM]
        ]
    );
    assert_eq!(rail.by, [3000 * MM, 0, 0]);
    let baluster = pieces.iter().find(|p| p.id == "baluster.1.1.1").unwrap();
    assert_eq!(baluster.profile[0], [90 * MM, -10 * MM, 50 * MM]);
    assert_eq!(baluster.by, [0, 0, 900 * MM]);
}

/// On the stair of 17 risers a rail's section is set upright in the plane x = 0, its depth
/// stretched by sqrt(277) / 14 (50 mm to 59.44 mm upright) so its true section is 50 mm, and
/// swept along the pitch to the last nosing; the first post stands on the floor and reaches the
/// rail, 1180 mm.
#[test]
fn a_rail_on_a_pitch_keeps_its_true_section() {
    let s = stair(&[None], &[]);
    let laid_out = laid_stair(&s, 3060);
    let host = StairHost {
        stair: &s,
        layout: &laid_out,
        shift: mm(0),
    };
    let railing = on_stair(RailingType::standard(), Side::Right, 50);
    let pieces = shape(railing.kind(), &laid(&railing, Some(host))).unwrap();
    let rail = pieces.iter().find(|p| p.id == "rail.1.1").unwrap();
    assert!(rail.profile.iter().all(|p| p[0] == 0), "flat in x = 0");
    let top = 1180 * MM;
    // 50 x sqrt(277) / 14 mm = 59.4404 mm: 380,418.67 steps, rounded once.
    assert_eq!(rail.profile[2][2], top - 380_419);
    assert_eq!(rail.profile[0], [0, 25 * MM, top]);
    assert_eq!(rail.by, [4480 * MM, 0, 2880 * MM]);
    let post = pieces.iter().find(|p| p.id == "post.1.1").unwrap();
    assert_eq!(post.profile[0][2], 0);
    assert_eq!(post.by, [0, 0, 1180 * MM]);
}

/// At any angle a rail's section is set upright in the axis plane nearer square to it, stretched
/// across by len / dx: at 45 degrees a 50 mm rail spans 50 sqrt(2) = 70.71 mm along y in the plane
/// x = 0, its corners at +-35.355 mm (226,274.2 steps, rounded once); a round post's twelve corners
/// lie within half a step of its circle.
#[test]
fn a_rail_at_any_angle_is_flat_and_true() {
    let mut kind = RailingType::standard();
    kind.posts = Profile::Round { diameter: mm(60) };
    let railing = sketched(kind, vec![at(0, 0), at(1000, 1000)]);
    let pieces = shape(railing.kind(), &laid(&railing, None)).unwrap();
    let rail = pieces.iter().find(|p| p.id == "rail.1.1").unwrap();
    let ys: Vec<i64> = rail.profile.iter().map(|p| p[1]).collect();
    assert_eq!(ys, vec![-226_274, 226_274, 226_274, -226_274]);
    assert!(rail.profile.iter().all(|p| p[0] == 0));
    let post = pieces.iter().find(|p| p.id == "post.1.2").unwrap();
    assert_eq!(post.profile.len(), 12);
    for corner in &post.profile {
        let (dx, dy) = (
            (corner[0] - 1000 * MM) as f64,
            (corner[1] - 1000 * MM) as f64,
        );
        let off = (dx * dx + dy * dy).sqrt() - 30.0 * MM as f64;
        assert!(
            off.abs() <= 0.71,
            "{corner:?} is {off} steps off its circle"
        );
    }
}

/// A round rail's twelve corners, from its top: the top one at the rail's height, and the
/// section's width at its middle its diameter.
#[test]
fn a_round_rail_is_twelve_corners() {
    let mut kind = RailingType::standard();
    kind.top_rail.profile = Profile::Round { diameter: mm(48) };
    let railing = sketched(kind, vec![at(0, 0), at(2000, 0)]);
    let pieces = shape(railing.kind(), &laid(&railing, None)).unwrap();
    let rail = pieces.iter().find(|p| p.id == "rail.1.1").unwrap();
    assert_eq!(rail.profile.len(), 12);
    assert_eq!(rail.profile[0], [0, 0, 1000 * MM]);
    assert_eq!(rail.profile[3], [0, -24 * MM, 976 * MM]);
    assert_eq!(rail.profile[6], [0, 0, 952 * MM]);
}
