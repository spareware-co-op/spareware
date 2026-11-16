//! The quick ceiling on a kind's first partials (`SurfaceKind::speed_ceiling`), held against the
//! closed-form extents and hulls (`ParamSurface::derivative_bound`) it stands in for. A caller
//! takes twice the ceiling where it would take the length of the extent's box, so the ceiling must
//! be at least that length, rounding apart, on every kind that has one, both directions, turned
//! and scaled images and domains a turn far from zero among them; and it must not be so wide that
//! an ordinary operand falls to the extents.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_curve::tight;
use geom_curve::{Basis, Circle, CurveError, CurveKind, DerivativeBound, Frame, Line, Trimmed};
use geom_math::{Axis, Transform, TypedTransform};
use geom_nurbs::Direction;
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, ParamBox, ParamSurface, Plane, Sphere, SurfaceKind, Torus,
};

const PI: f64 = std::f64::consts::PI;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The length of the box the extents give the first partial along `dir` over the domain.
fn extent_length(s: &SurfaceKind, dir: Direction) -> f64 {
    let DerivativeBound::Bounded(b) = s.derivative_bound(s.domain(), dir, &mut meter()).unwrap()
    else {
        panic!("{s:?}: unbounded");
    };
    let mut sum = 0.0;
    for (lo, hi) in b.lo.iter().zip(&b.hi) {
        let far = tight::larger(lo.abs(), hi.abs());
        sum += far * far;
    }
    sum.sqrt()
}

fn ceiling(s: &SurfaceKind, dir: Direction) -> f64 {
    s.speed_ceiling(s.domain(), dir).unwrap().unwrap()
}

/// `exact` < `c` ≤ `exact` and a little: a sum the ceiling forms of doubles, each step rounded up.
fn just_above(c: f64, exact: f64) -> bool {
    exact <= c && c <= exact * (1.0 + 1e-14)
}

/// A frame turned off the axes, so that every unit vector has three coordinates.
fn leaning() -> Frame<3> {
    Frame::new([2.0, 1.0, -2.0], [1.0, 2.0, 2.0]).unwrap()
}

/// The surface swept along `d` from the segment from `a` to `b`, or from the circle about z of
/// radius 3 over a half-turn from `start`.
fn wall(a: [f64; 3], b: [f64; 3], d: [f64; 3]) -> SurfaceKind {
    let line = Line::segment(a, b).unwrap().value;
    let profile = Trimmed::new(Basis::Line(line), (0.0, 1.0)).unwrap();
    SurfaceKind::Extrusion(Extrusion::new(profile, d, (-2.0, 5.0)).unwrap())
}

fn arc_wall(start: f64) -> SurfaceKind {
    let span = (start, start + 1.0);
    let circle = Circle::new([0.0; 3], 3.0, Frame::standard().unwrap(), span).unwrap();
    let profile = Trimmed::new(Basis::try_from(CurveKind::Circle(circle)).unwrap(), span).unwrap();
    SurfaceKind::Extrusion(Extrusion::new(profile, [0.0, 1.0, 2.0], (0.0, 1.0)).unwrap())
}

fn kinds() -> Vec<SurfaceKind> {
    let standard = Frame::standard().unwrap();
    let turn = (0.0, 2.0);
    let far = 17_592_186_044_416.75;
    let mut out = vec![
        SurfaceKind::Plane(
            Plane::new(
                [1.0, 2.0, 3.0],
                [1.0, -2.0, 0.5],
                [0.0, 1.0, 1.0],
                ((-3.0, 5.0), (0.0, 4.0)),
            )
            .unwrap(),
        ),
        SurfaceKind::Plane(
            Plane::new(
                [-(far * 3.0), 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                ((far, far + 1.0), (0.0, 1.0)),
            )
            .unwrap(),
        ),
        wall([1.0, 2.0, 0.0], [4.0, -2.0, 1.0], [0.0, 0.0, 1.0]),
        wall([0.0; 3], [0.0, 3.0, 0.0], [1.0, -1.0, 0.5]),
    ];
    for frame in [standard, leaning()] {
        for u in [turn, (0.25, 0.5), (far, far + 2.0), (-far - 2.0, -far)] {
            out.push(SurfaceKind::Cylinder(
                Cylinder::new([1.0, -2.0, 3.0], 2.0, frame, (u, (-4.0, 8.0))).unwrap(),
            ));
            out.push(SurfaceKind::Cone(
                Cone::new([0.0, 1.0, 0.0], 1.0, 0.5, frame, (u, (-4.0, 8.0))).unwrap(),
            ));
            out.push(SurfaceKind::Cone(
                Cone::new([0.0; 3], 0.0, 2.0, frame, (u, (0.0, 3.0))).unwrap(),
            ));
            for v in [(-0.5, 0.5), (-0.1, 0.1), (0.25, 0.5)] {
                out.push(SurfaceKind::Sphere(
                    Sphere::new([4.0, 0.0, -1.0], 3.0, frame, (u, v)).unwrap(),
                ));
            }
            for v in [turn, (0.5, 1.0)] {
                out.push(SurfaceKind::Torus(
                    Torus::new([0.0; 3], 5.0, 1.5, frame, (u, v)).unwrap(),
                ));
            }
        }
    }
    // Each one turned and scaled too: a similarity keeps an analytic kind's form, and any
    // transform keeps an extrusion one, its vectors placed.
    let transforms = [
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::uniform_scale(1.5).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
    ];
    let mut images = Vec::new();
    for s in &out {
        for t in &transforms {
            images.push(s.transformed(t, &mut meter()).unwrap().value);
        }
    }
    out.extend(images);
    out
}

#[test]
fn the_ceiling_holds_the_extents_box_and_twice_it_holds_it_with_room() {
    let mut held = 0;
    for s in kinds() {
        for dir in [Direction::U, Direction::V] {
            let Some(c) = s.speed_ceiling(s.domain(), dir).unwrap() else {
                continue;
            };
            let n = extent_length(&s, dir);
            // At least the box's length, to the rounding of its square root here.
            assert!(
                n <= c * (1.0 + 1e-12),
                "{s:?} {dir:?}: ceiling {c}, box {n}"
            );
            assert!(n < 2.0 * c, "{s:?} {dir:?}: ceiling {c}, box {n}");
            // And within a few times it: a turned frame's unit vectors count up to √3 each, and a
            // sphere's v sums three terms where its speed is one.
            assert!(c <= 6.0 * n, "{s:?} {dir:?}: ceiling {c}, box {n}");
            held += 1;
        }
    }
    // Every kind built has both, the 64 round ones, the two planes and the two straight walls;
    // their images add more (an affine image of a round kind has none).
    assert!(held > 2 * 68, "{held}");
}

#[test]
fn the_ceiling_is_the_sum_of_the_terms_bounds() {
    let standard = Frame::standard().unwrap();
    // A plane's partials are its vectors: their coordinates' magnitudes, summed.
    let plane = SurfaceKind::Plane(
        Plane::new(
            [1.0, 2.0, 3.0],
            [1.0, -2.0, 0.5],
            [0.0, 1.0, 1.0],
            ((-3.0, 5.0), (0.0, 4.0)),
        )
        .unwrap(),
    );
    assert!(just_above(ceiling(&plane, Direction::U), 3.5));
    assert!(just_above(ceiling(&plane, Direction::V), 2.0));
    // A cylinder's u runs r·π·(cos, sin), each bounded by r·π; its v is the axis, 1.
    let tube = SurfaceKind::Cylinder(
        Cylinder::new([1.0; 3], 2.0, standard, ((0.0, 2.0), (-4.0, 8.0))).unwrap(),
    );
    assert!(just_above(ceiling(&tube, Direction::U), 4.0 * PI));
    assert!(just_above(ceiling(&tube, Direction::V), 1.0));
    // A cone's u runs (r₀ + k·v)·π·(cos, sin): r₀ and k times the end of v further from zero, 8
    // and not 4, twice over, times π; its v is k·(cos, sin) and the axis, 2k + 1.
    let cone = SurfaceKind::Cone(
        Cone::new([0.0; 3], 1.0, 0.5, standard, ((0.0, 2.0), (-8.0, 4.0))).unwrap(),
    );
    assert!(just_above(ceiling(&cone, Direction::U), 10.0 * PI));
    assert!(just_above(ceiling(&cone, Direction::V), 2.0));
    // A sphere's u sums π·r for each of its two turning terms, its v for all three.
    let ball = SurfaceKind::Sphere(
        Sphere::new([0.0; 3], 3.0, standard, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
    );
    assert!(just_above(ceiling(&ball, Direction::U), 6.0 * PI));
    assert!(just_above(ceiling(&ball, Direction::V), 9.0 * PI));
    // A torus's u turns the major and the minor radius twice over, its v the minor three times.
    let ring = SurfaceKind::Torus(
        Torus::new([0.0; 3], 5.0, 1.5, standard, ((0.0, 2.0), (0.0, 2.0))).unwrap(),
    );
    assert!(just_above(ceiling(&ring, Direction::U), 13.0 * PI));
    assert!(just_above(ceiling(&ring, Direction::V), 4.5 * PI));
    // A straight wall's u is the segment's direction, its v the sweep's.
    let straight = wall([1.0, 2.0, 0.0], [4.0, -2.0, 1.0], [0.0, -2.0, 1.0]);
    assert!(just_above(ceiling(&straight, Direction::U), 8.0));
    assert!(just_above(ceiling(&straight, Direction::V), 3.0));
}

#[test]
fn a_turn_far_from_zero_has_the_ceiling_of_one_near_it() {
    // The ceiling reads only the forms and the far end of a t: a turn's start does not move it.
    for frame in [Frame::standard().unwrap(), leaning()] {
        let tube = |u: (f64, f64)| {
            SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 1024.0, frame, (u, (-1.0, 1.0))).unwrap())
        };
        let near = tube((0.0, 2.0));
        let far = tube((17_592_186_044_416.75, 17_592_186_044_418.75));
        for dir in [Direction::U, Direction::V] {
            assert_eq!(
                ceiling(&near, dir).to_bits(),
                ceiling(&far, dir).to_bits(),
                "{dir:?}"
            );
        }
    }
}

#[test]
fn only_some_kinds_and_directions_have_a_ceiling_and_only_inside_their_domains() {
    // A curved wall has one along its sweep only.
    for start in [0.0, 17_592_186_044_416.75] {
        let curved = arc_wall(start);
        assert_eq!(
            curved.speed_ceiling(curved.domain(), Direction::U),
            Ok(None)
        );
        let along = ceiling(&curved, Direction::V);
        assert!(just_above(along, 3.0), "{along}");
        assert!(extent_length(&curved, Direction::V) <= along);
    }
    // An affine image of a cylinder has none.
    let squashed = SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
    .transformed(
        &TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
        &mut meter(),
    )
    .unwrap()
    .value;
    assert!(matches!(squashed, SurfaceKind::Affine(_)));
    assert_eq!(
        squashed.speed_ceiling(squashed.domain(), Direction::V),
        Ok(None)
    );
    // A plane without end has no ceiling over its whole domain, though its partials are finite.
    let open = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((f64::NEG_INFINITY, f64::INFINITY), (0.0, 1.0)),
        )
        .unwrap(),
    );
    assert_eq!(
        open.speed_ceiling(open.domain(), Direction::U),
        Err(CurveError::SpanOutside)
    );
    let outside: ParamBox = ((0.0, 3.0), (-0.5, 0.5));
    let ball = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    assert_eq!(
        ball.speed_ceiling(outside, Direction::V),
        Err(CurveError::SpanOutside)
    );
    let w = wall([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(
        w.speed_ceiling(((0.0, 1.0), (-3.0, 5.0)), Direction::V),
        Err(CurveError::SpanOutside)
    );
}

#[test]
fn a_ceiling_past_the_doubles_is_refused() {
    let big = 1.0e308;
    let p = SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [big, big, 0.0],
            [0.0, 0.0, 1.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    );
    assert_eq!(
        p.speed_ceiling(p.domain(), Direction::U),
        Err(CurveError::NotFinite)
    );
    assert!(just_above(ceiling(&p, Direction::V), 1.0));
    let w = wall([0.0; 3], [1.0, 0.0, 0.0], [big, big, 0.0]);
    assert_eq!(
        w.speed_ceiling(w.domain(), Direction::V),
        Err(CurveError::NotFinite)
    );
    assert!(just_above(ceiling(&w, Direction::U), 1.0));
}
