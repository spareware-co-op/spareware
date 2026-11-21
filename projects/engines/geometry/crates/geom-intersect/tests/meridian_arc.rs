//! A revolution whose profile is an arc of a circle meets a line of the half-plane only on that arc
//! (issue 1142).
//!
//! The reduced cases' closed form for a line with a meridian circle solved the arc as its whole
//! circle and kept every root on the line's span: the upper half of the tube about ρ = 5 cut by
//! z = −1/2 gave two rings where there are none, and the cylinder ρ = 11/2 gave rings at
//! z = ±√3/2 where only +√3/2 is on the arc. A root is kept now only where its turn on the circle
//! is proven on the arc's domain, and one whose turn's enclosure reaches an end only where exact
//! signs prove it is that end; one they cannot place is left to the general paths (`None`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_curve::{Basis, Circle, CurveKind, Frame, ParamCurve, Trimmed};
use geom_intersect::quadrics::natural;
use geom_intersect::result::{Intersection, Item};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Revolution, SurfaceKind};

const ROUND: (f64, f64) = (0.0, 2.0);
const WIDE: (f64, f64) = (-1000.0, 1000.0);

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(50_000_000))
}

fn up() -> Frame<3> {
    Frame::standard().unwrap()
}

/// The frame whose plane is y = 0 with x̂ along x and ŷ along z: the arc over (0, 1) is the upper
/// half of its circle.
fn upper() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]).unwrap()
}

/// The arc over `span` of the tube about ρ = 5 of radius 1 in y = 0, on `frame`.
fn tube(frame: Frame<3>, span: (f64, f64)) -> CurveKind<3> {
    CurveKind::Circle(Circle::new([5.0, 0.0, 0.0], 1.0, frame, span).unwrap())
}

/// The surface `profile` sweeps turning once about the z axis.
fn revolution(profile: CurveKind<3>) -> SurfaceKind {
    let span = profile.domain();
    let profile = Trimmed::new(Basis::try_from(profile).unwrap(), span).unwrap();
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up(), ROUND).unwrap())
}

/// The plane z = `z`, square to the axis.
fn level(z: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (WIDE, WIDE),
        )
        .unwrap(),
    )
}

fn cylinder(r: f64) -> SurfaceKind {
    SurfaceKind::Cylinder(Cylinder::new([0.0; 3], r, up(), (ROUND, WIDE)).unwrap())
}

/// The rings of `cut`, as (radius, height), each a circle about the z axis.
fn rings(cut: &Intersection) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = cut
        .items()
        .iter()
        .map(|item| match item {
            Item::Curve { curve, .. } => match curve.as_ref() {
                CurveKind::Circle(ring) => {
                    let [x, y, z] = ring.center();
                    assert!(x.abs() < 1e-9 && y.abs() < 1e-9, "{ring:?} off the axis");
                    (ring.radius(), z)
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    out
}

/// That `a` meets `b` in the rings `want`, each to within 1e-9.
fn assert_rings(a: &SurfaceKind, b: &SurfaceKind, want: &[(f64, f64)]) {
    let r = natural(a, b, &mut meter()).unwrap().unwrap();
    let got = rings(&r);
    assert_eq!(got.len(), want.len(), "{got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9,
            "{got:?}, want {want:?}"
        );
    }
}

#[test]
fn a_half_tube_cut_below_its_arc_meets_no_ring() {
    // The issue's first case: z = −1/2 crosses the whole circle at ρ = 5 ± √3/2, both below the
    // arc z ≥ 0.
    let half = revolution(tube(upper(), (0.0, 1.0)));
    assert_rings(&level(-0.5), &half, &[]);
}

#[test]
fn a_half_tube_keeps_only_the_ring_on_its_arc() {
    // The issue's second case: ρ = 11/2 crosses the whole circle at z = ±√3/2, and only +√3/2 is
    // on the arc. The same arc two and four turns out, over (4, 5) and (8, 9), is the same surface.
    let z = 0.75_f64.sqrt();
    for span in [(0.0, 1.0), (4.0, 5.0), (8.0, 9.0)] {
        let half = revolution(tube(upper(), span));
        assert_rings(&cylinder(5.5), &half, &[(5.5, z)]);
        assert_rings(&level(0.5), &half, &[(5.0 - z, 0.5), (5.0 + z, 0.5)]);
    }
}

#[test]
fn a_cone_meets_a_half_tube_only_on_its_arc() {
    // The cones ρ = 5 + z and ρ = 5 − z cross the whole circle at z = ±1/√2, and only +1/√2 is on
    // the arc: each keeps one ring, its turn read on the cone's line from the cone's own data.
    let half = revolution(tube(upper(), (0.0, 1.0)));
    let z = 0.5_f64.sqrt();
    for (slope, rho) in [(1.0, 5.0 + z), (-1.0, 5.0 - z)] {
        let cone = Cone::new([0.0; 3], 5.0, slope, up(), (ROUND, (-4.0, 4.0))).unwrap();
        assert_rings(&SurfaceKind::Cone(cone), &half, &[(rho, z)]);
    }
}

#[test]
fn a_cut_through_an_arcs_ends_keeps_its_rings_by_their_proof() {
    // z = 0 meets the upper half at its two ends, ρ = 6 at turn 0 and ρ = 4 at turn 1, and the
    // cylinders ρ = 6 and ρ = 4 touch it there; each end is proven on the line by exact signs.
    let half = revolution(tube(upper(), (0.0, 1.0)));
    assert_rings(&level(0.0), &half, &[(4.0, 0.0), (6.0, 0.0)]);
    assert_rings(&cylinder(6.0), &half, &[(6.0, 0.0)]);
    assert_rings(&cylinder(4.0), &half, &[(4.0, 0.0)]);
    // The quarter from the top (5, 1) at turn 0 to the outside (6, 0) at turn 1/2, on the frame
    // whose x̂ is along z: z = 1 touches it at its start, ρ = 6 at its end, and z = 0 meets it at
    // its end (ρ = 4 is off it).
    let top = Frame::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]).unwrap();
    let quarter = revolution(tube(top, (0.0, 0.5)));
    assert_rings(&level(1.0), &quarter, &[(5.0, 1.0)]);
    assert_rings(&cylinder(6.0), &quarter, &[(6.0, 0.0)]);
    assert_rings(&level(0.0), &quarter, &[(6.0, 0.0)]);
    // The lower half, over (1, 2), from ρ = 4 at turn 1 to ρ = 6 at turn 2: its start's turn is
    // read across atan2's cut, a hair either side of 1.
    let lower = revolution(tube(upper(), (1.0, 2.0)));
    assert_rings(&level(0.0), &lower, &[(4.0, 0.0), (6.0, 0.0)]);
    assert_rings(&cylinder(5.5), &lower, &[(5.5, -(0.75_f64.sqrt()))]);
    // The quarter over (0, 1/2) of the first frame rises from (6, 0) to the top (5, 1), which
    // z = 1 touches at its end: the end a quarter turn on from x̂, on the side the normal names.
    let rising = revolution(tube(upper(), (0.0, 0.5)));
    assert_rings(&level(1.0), &rising, &[(5.0, 1.0)]);
}

#[test]
fn an_arc_on_a_slanted_frame_keeps_its_ends_by_their_proof() {
    // The quarter of the circle about ρ = 10 of radius 5 from (13, 4), along x = (3, 0, 4), to
    // (6, 3) at turn 1/2: z = 4 meets it at its start and at ρ = 7, and z = 3 at its end alone
    // (ρ = 14 is before its start). Each end is proven on its line by exact signs from the
    // frame's stored x, which is not of unit length.
    let slant = Frame::new([3.0, 0.0, 4.0], [0.0, -1.0, 0.0]).unwrap();
    let arc = Circle::new([10.0, 0.0, 0.0], 5.0, slant, (0.0, 0.5)).unwrap();
    let quarter = revolution(CurveKind::Circle(arc));
    assert_rings(&level(4.0), &quarter, &[(7.0, 4.0), (13.0, 4.0)]);
    assert_rings(&level(3.0), &quarter, &[(6.0, 3.0)]);
    // The double under 4 meets the circle 4.7e−17 half-turns before the start, inside the
    // enclosure of its turn: the line passes beside the start, not through it, so nothing proves
    // where the meeting stands and the pair is the general paths'.
    let below = level(f64::from_bits(4.0_f64.to_bits() - 1));
    assert!(natural(&below, &quarter, &mut meter()).unwrap().is_none());
}

#[test]
fn an_arc_in_a_half_plane_off_the_frames_axes_is_touched_by_its_proof() {
    // The upper half of the tube of radius 1 about (3, 4, 0), in the half-plane through the axis
    // and that centre: the centre's ρ is 5 exactly, though the half-plane's direction (0.6, 0.8)
    // rounds. z = 1 touches it at its top, z = −1 at its bottom, off it, and ρ = 6 and ρ = 4 at
    // its two ends; each touch is told on the centre's ρ, not on its part along that direction.
    let frame = Frame::new([3.0, 4.0, 0.0], [4.0, -3.0, 0.0]).unwrap();
    let arc = Circle::new([3.0, 4.0, 0.0], 1.0, frame, (0.0, 1.0)).unwrap();
    let half = revolution(CurveKind::Circle(arc));
    assert_rings(&level(1.0), &half, &[(5.0, 1.0)]);
    assert_rings(&level(-1.0), &half, &[]);
    assert_rings(&cylinder(6.0), &half, &[(6.0, 0.0)]);
    assert_rings(&cylinder(4.0), &half, &[(4.0, 0.0)]);
}

#[test]
fn a_ring_no_enclosure_tells_from_the_arcs_end_is_left_to_the_general_paths() {
    // The arc up to the turn 1/3 (as a double, a hair under it): z = 0.8660254037844386, the double
    // under √3/2, meets it 1.3e−17 half-turns inside its end, which no enclosure of the turn tells
    // and no exact sign reaches (the end is no multiple of a half-turn), so the pair is the
    // general paths'. Away from the end the arc decides: z = 1/2 keeps ρ = 5 + √3/2 alone, and
    // z = 0.9 meets it nowhere.
    let third = revolution(tube(upper(), (0.0, 1.0 / 3.0)));
    let at_end = level(0.866_025_403_784_438_6);
    assert!(natural(&at_end, &third, &mut meter()).unwrap().is_none());
    let z = 0.75_f64.sqrt();
    assert_rings(&level(0.5), &third, &[(5.0 + z, 0.5)]);
    assert_rings(&level(0.9), &third, &[]);
}

#[test]
fn a_ring_a_hair_from_an_arcs_end_is_placed_by_the_proven_sign_of_its_offset() {
    // The arc from the turn 8 + 1/6 to 8 + 1/3 (as doubles), four turns out: its ends are no
    // multiple of a half-turn, and a turn's enclosure moved there is an ulp of 8 wide, 1.8e−15
    // half-turns, so it reaches an end that close. Each cut meets the arc about 1e−15 half-turns
    // from an end, and the sign of sin π(t − s), on geom-num's proven cospi and sinpi of the end,
    // tells the side (60-digit check of each offset). sin π(lo − 8) = 0.4999999999999983890…:
    // z = 0.49999999999999617 meets the circle 8.2e−16 half-turns before the start, z =
    // 0.5000000000000012 1.0e−15 after it.
    let arc = revolution(tube(upper(), (8.0 + 1.0 / 6.0, 8.0 + 1.0 / 3.0)));
    let rho = |z: f64| 5.0 + (1.0 - z * z).sqrt();
    assert_rings(&level(0.499_999_999_999_996_17), &arc, &[]);
    let z = 0.500_000_000_000_001_2;
    assert_rings(&level(z), &arc, &[(rho(z), z)]);
    // sin π(hi − 8) = 0.8660254037844395768…: z = 0.866025403784438 meets it 9.8e−16 half-turns
    // before the end, z = 0.8660254037844412 1.0e−15 past it.
    let z = 0.866_025_403_784_438;
    assert_rings(&level(z), &arc, &[(rho(z), z)]);
    assert_rings(&level(0.866_025_403_784_441_2), &arc, &[]);
}
