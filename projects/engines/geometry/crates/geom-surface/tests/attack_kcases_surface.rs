//! Attack tests from solidcore's kernel-case corpus: surface kinds, `geom-surface`.
//!
//! The corpus is `crates/solidcore/tests/kernel_cases/` in solidcore (tangency-labs/solidcore,
//! Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70): one entry per operation-level
//! case, with an expected answer derived in closed form (an area, a radius, a volume). The entries
//! here are the ones whose answer is a surface kind's own geometry: the cylinder, cone, sphere and
//! torus primitives, the surfaces that an extrude, a revolve, a ruled strip or a sweep with a
//! scale make, and an offset of an analytic surface. Each test builds the surface in our API from
//! the entry's description, integrates its area from the kernel's own partial derivatives (the
//! surface area is the integral of |S_u x S_v|, by a composite five-point Gauss-Legendre rule, in
//! plain doubles), and holds it to the entry's closed-form area; where the entry names the kind
//! the surface must be (a cylinder, a sphere, a cone), the test also holds every sampled point to
//! that kind's defining equation. Each test cites its entry as `<area>/<case>.json`.
//!
//! The entries' areas count flat caps beside the curved face; a test takes the curved face's part
//! (the entry's number less the caps' closed-form areas, written in the test).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::suboptimal_flops,
    reason = "tests: native arithmetic, casts and indexing, platform maths on an independent oracle (the formula as it is written), the corpus's numbers as it writes them, prose that names corpus files, and a failure stops the test"
)]

use std::f64::consts::PI;

use geom_curve::{Basis as Leaf, Circle, CurveError, Frame, Line, Trimmed};
use geom_nurbs::{Basis, Curve, Limit};
use geom_surface::{
    Cone, Cylinder, Extrusion, Offset, ParamBox, ParamSurface, Plane, Revolution, Ruled, Sphere,
    SurfaceKind, Torus,
};

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

/// A five-point Gauss-Legendre rule on [-1, 1].
const NODES: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683_1,
    0.0,
    0.538_469_310_105_683_1,
    0.906_179_845_938_664,
];
const WEIGHTS: [f64; 5] = [
    0.236_926_885_056_189_1,
    0.478_628_670_499_366_5,
    0.568_888_888_888_888_9,
    0.478_628_670_499_366_5,
    0.236_926_885_056_189_1,
];

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The area of `s` over `domain`: the integral of |S_u x S_v|, 24 panels of five points each way.
fn area(s: &impl ParamSurface, domain: ParamBox) -> f64 {
    const PANELS: usize = 24;
    let ((u0, u1), (v0, v1)) = domain;
    let (du, dv) = ((u1 - u0) / PANELS as f64, (v1 - v0) / PANELS as f64);
    let mut total = 0.0;
    for i in 0..PANELS {
        for j in 0..PANELS {
            for (a, wa) in NODES.iter().zip(WEIGHTS) {
                for (b, wb) in NODES.iter().zip(WEIGHTS) {
                    let u = u0 + du * (i as f64 + 0.5 + 0.5 * a);
                    let v = v0 + dv * (j as f64 + 0.5 + 0.5 * b);
                    let [_, su, sv] = s.partials(u, v, SIDES).unwrap();
                    total += wa * wb * 0.25 * du * dv * length(cross(su, sv));
                }
            }
        }
    }
    total
}

/// `got` is `want` to within `rel` of it, and within 1e-12 of it whatever `want` is (so zero works).
fn close(got: f64, want: f64, rel: f64, what: &str) {
    assert!(
        (got - want).abs() <= rel * want.abs() + 1e-12,
        "{what}: got {got}, want {want}"
    );
}

/// Every point of an 11 x 11 grid over `domain`, passed to `check`.
fn on_grid(s: &impl ParamSurface, domain: ParamBox, mut check: impl FnMut([f64; 3])) {
    let ((u0, u1), (v0, v1)) = domain;
    for i in 0..=10 {
        for j in 0..=10 {
            let u = u0 + (u1 - u0) * f64::from(i) / 10.0;
            let v = v0 + (v1 - v0) * f64::from(j) / 10.0;
            check(s.point(u, v, SIDES).unwrap());
        }
    }
}

fn standard() -> Frame<3> {
    Frame::standard().unwrap()
}

/// The circle of radius `r` about (0, 0, z) in the plane z = const.
fn circle_at(r: f64, z: f64) -> Trimmed<3> {
    Trimmed::whole(Leaf::Circle(
        Circle::new([0.0, 0.0, z], r, standard(), (0.0, 2.0)).unwrap(),
    ))
    .unwrap()
}

fn segment(a: [f64; 3], b: [f64; 3]) -> Trimmed<3> {
    Trimmed::whole(Leaf::Line(Line::segment(a, b).unwrap().value)).unwrap()
}

/// A quarter circle of radius 10 in the plane y = 0 about the origin, from the +x axis to the +z
/// axis (frame x = (1, 0, 0), normal (0, -1, 0), so the frame's y axis is +z).
fn quarter_arc() -> Trimmed<3> {
    let f = Frame::<3>::new([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]).unwrap();
    Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 10.0, f, (0.0, 0.5)).unwrap(),
    ))
    .unwrap()
}

// ---------------------------------------------------------------------------------------------
// ladder/01-primitives: the round primitives' curved faces

/// ladder/01-primitives/002-cylinder.json: a cylinder r10 h30, area 2513.274122872 with its two
/// disks (2 pi 10^2 together): the curved face is 2 pi 10 30.
#[test]
fn ladder_01_002_a_cylinder_of_r10_h30_has_the_entrys_area() {
    let s = Cylinder::new([0.0; 3], 10.0, standard(), ((0.0, 2.0), (0.0, 30.0))).unwrap();
    let caps = 2.0 * PI * 100.0;
    close(area(&s, s.domain()) + caps, 2513.274_122_872, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(p[0].hypot(p[1]), 10.0, 1e-13, "radius");
    });
}

/// ladder/01-primitives/003-cone.json: a cone r10 h30 (a revolved right triangle), area
/// 1307.618091939 with its disk (pi 10^2): the curved face is pi r sqrt(r^2 + h^2).
#[test]
fn ladder_01_003_a_cone_of_r10_h30_has_the_entrys_area() {
    // The radius is 10 at v = 0 and falls to 0 at v = 30: slope -1/3.
    let s = Cone::new(
        [0.0; 3],
        10.0,
        -1.0 / 3.0,
        standard(),
        ((0.0, 2.0), (0.0, 30.0)),
    )
    .unwrap();
    close(
        area(&s, s.domain()) + PI * 100.0,
        1307.618_091_939,
        1e-11,
        "area",
    );
    on_grid(&s, s.domain(), |p| {
        close(
            p[0].hypot(p[1]),
            10.0 * (1.0 - p[2] / 30.0),
            1e-12,
            "radius at height",
        );
    });
}

/// ladder/01-primitives/004-frustum.json and solids/cov-cone-frustum-along-x.json: a frustum
/// R10 -> r5, h20 (the entry's area 1364.182957455 counts both disks, pi (R^2 + r^2)); the second
/// entry stands it on the x axis from x = 5.
#[test]
fn ladder_01_004_a_frustum_has_the_entrys_area_on_the_z_axis_and_on_the_x_axis() {
    let caps = PI * (100.0 + 25.0);
    let up = Cone::new([0.0; 3], 10.0, -0.25, standard(), ((0.0, 2.0), (0.0, 20.0))).unwrap();
    close(
        area(&up, up.domain()) + caps,
        1364.182_957_455,
        1e-11,
        "along z",
    );
    // Along x: the frame's normal is +x, its reference vector +y; centre (5, 0, 0).
    let along_x = Frame::<3>::new([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
    let s = Cone::new(
        [5.0, 0.0, 0.0],
        10.0,
        -0.25,
        along_x,
        ((0.0, 2.0), (0.0, 20.0)),
    )
    .unwrap();
    close(
        area(&s, s.domain()) + caps,
        1364.182_957_455,
        1e-11,
        "along x",
    );
    on_grid(&s, s.domain(), |p| {
        let along = p[0] - 5.0;
        close(p[1].hypot(p[2]), 10.0 - 0.25 * along, 1e-12, "radius at x");
        assert!((5.0 - 1e-12..=25.0 + 1e-12).contains(&p[0]));
    });
}

/// solids/cov-cone-full-apex-negative-height.json: a full cone, base radius 6 at z = 10 and its
/// apex at z = -2 (height -12). Area 365.99066584669873 with its base disk (pi 36). The surface
/// runs from the apex (v = -12) to the base (v = 0), through the point where the radius is zero.
#[test]
fn solids_a_full_cone_to_its_apex_has_the_entrys_area() {
    let s = Cone::new(
        [0.0, 0.0, 10.0],
        6.0,
        0.5,
        standard(),
        ((0.0, 2.0), (-12.0, 0.0)),
    )
    .unwrap();
    close(
        area(&s, s.domain()) + PI * 36.0,
        365.990_665_846_698_73,
        1e-11,
        "area",
    );
    // The cone's v runs along +z here, so the apex is at z = 10 - 12 = -2.
    let apex = s.point(0.0, -12.0, SIDES).unwrap();
    assert!(
        apex[0].hypot(apex[1]) < 1e-12 && (apex[2] + 2.0).abs() < 1e-12,
        "{apex:?}"
    );
}

/// ladder/01-primitives/005-sphere.json: a sphere r10, area 1256.637061436 (4 pi r^2).
#[test]
fn ladder_01_005_a_sphere_of_r10_has_the_entrys_area() {
    let s = Sphere::new([0.0; 3], 10.0, standard(), ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    close(area(&s, s.domain()), 1256.637_061_436, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(length(p), 10.0, 1e-13, "radius");
    });
}

/// ladder/01-primitives/006-torus.json: a torus R20 r5, area 3947.841760436 (4 pi^2 R r).
#[test]
fn ladder_01_006_a_torus_of_r20_r5_has_the_entrys_area() {
    let s = Torus::new([0.0; 3], 20.0, 5.0, standard(), ((0.0, 2.0), (0.0, 2.0))).unwrap();
    close(area(&s, s.domain()), 3947.841_760_436, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        let ring = p[0].hypot(p[1]) - 20.0;
        close(
            ring.hypot(p[2]),
            5.0,
            1e-12,
            "distance from the core circle",
        );
    });
}

/// ladder/03-analytic-profiles/005-revolve-touching-axis-hemisphere.json: a hemisphere r20 (a
/// quarter disk with an edge on the axis, revolved a turn): area 3769.911184308 with its disk
/// (pi 20^2); the curved face is 2 pi r^2.
#[test]
fn ladder_03_005_a_hemisphere_from_a_quarter_disk_has_the_entrys_area() {
    let s = Sphere::new([0.0; 3], 20.0, standard(), ((0.0, 2.0), (0.0, 0.5))).unwrap();
    close(
        area(&s, s.domain()) + PI * 400.0,
        3_769.911_184_308,
        1e-11,
        "area",
    );
}

// ---------------------------------------------------------------------------------------------
// surfaces: the surfaces of an extrude, a revolve, a ruled strip and a sweep

/// surfaces/cov-revolve-line-full-turn-cylinder.json: a line parallel to the axis at r 10 (z 0 to
/// 20) revolved a full turn is the cylinder wall, area 2 pi 10 20 = 1256.6370614359173.
#[test]
fn surfaces_cov_revolve_a_line_parallel_to_the_axis_is_the_cylinder_wall() {
    let s = Revolution::new(
        segment([10.0, 0.0, 0.0], [10.0, 0.0, 20.0]),
        [0.0; 3],
        standard(),
        (0.0, 2.0),
    )
    .unwrap();
    close(area(&s, s.domain()), 1_256.637_061_435_917_3, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(p[0].hypot(p[1]), 10.0, 1e-13, "radius");
        assert!((-1e-12..=20.0 + 1e-12).contains(&p[2]), "{p:?}");
    });
}

/// surfaces/cov-revolve-quarter-arc-half-turn-sphere.json: a quarter arc r 10 about the axis,
/// revolved half a turn: half a hemisphere on the sphere, area pi r^2 = 314.1592653589793.
#[test]
fn surfaces_cov_revolve_a_quarter_arc_half_a_turn_is_on_the_sphere() {
    let s = Revolution::new(quarter_arc(), [0.0; 3], standard(), (0.0, 1.0)).unwrap();
    close(area(&s, s.domain()), 314.159_265_358_979_3, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(length(p), 10.0, 1e-13, "distance from the centre");
    });
}

/// surfaces/cov-extrude-quarter-arc-is-a-cylinder.json: a quarter arc r 10 extruded 20 is a quarter
/// cylinder, area (pi/2) 10 20 = 314.1592653589793.
#[test]
fn surfaces_cov_extrude_a_quarter_arc_is_a_quarter_cylinder() {
    // The arc in the plane z = 0 about the origin, from +x to +y, extruded along +z.
    let arc = Trimmed::whole(Leaf::Circle(
        Circle::new([0.0; 3], 10.0, standard(), (0.0, 0.5)).unwrap(),
    ))
    .unwrap();
    let s = Extrusion::new(arc, [0.0, 0.0, 1.0], (0.0, 20.0)).unwrap();
    close(area(&s, s.domain()), 314.159_265_358_979_3, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(p[0].hypot(p[1]), 10.0, 1e-13, "radius");
        assert!(p[0] >= -1e-12 && p[1] >= -1e-12, "{p:?}");
    });
}

/// surfaces/cov-extrude-line-symmetric.json and ...-legacy-per-side.json, and
/// surfaces/cov-sweep-line-along-a-line-plane.json: a line extruded or swept straight is a plane
/// strip: 10 x 10 (5 each side), 10 x 20 (10 each side) and 10 x 20 along z.
#[test]
fn surfaces_cov_a_line_extruded_or_swept_straight_is_a_plane_strip() {
    let line = segment([0.0, 0.0, -5.0], [10.0, 0.0, -5.0]);
    let s = Extrusion::new(line, [0.0, 0.0, 1.0], (0.0, 10.0)).unwrap();
    close(area(&s, s.domain()), 100.0, 1e-12, "5 each side");
    let line = segment([0.0, 0.0, -10.0], [10.0, 0.0, -10.0]);
    let s = Extrusion::new(line, [0.0, 0.0, 1.0], (0.0, 20.0)).unwrap();
    close(area(&s, s.domain()), 200.0, 1e-12, "10 each side");
    on_grid(&s, s.domain(), |p| {
        assert!(p[1].abs() < 1e-13, "in the plane y = 0: {p:?}");
    });
    let line = segment([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    let s = Extrusion::new(line, [0.0, 0.0, 1.0], (0.0, 20.0)).unwrap();
    close(area(&s, s.domain()), 200.0, 1e-12, "swept along z");
}

/// surfaces/cov-ruled-cylinder-rim-at-45-degrees-is-a-cone.json: a ruled strip off a cylinder's
/// top rim (r 10 at z = 10), 5 long, leaning out at 45 degrees: the cone frustum from r 10 at
/// z = 10 to r 10 + 5/sqrt 2 at z = 10 + 5/sqrt 2; area pi (r1 + r2) 5 = 369.69530208595893.
#[test]
fn surfaces_cov_a_ruled_strip_at_45_degrees_off_a_rim_is_a_cone() {
    let d = 5.0 / 2.0_f64.sqrt();
    let s = Ruled::new(
        circle_at(10.0, 10.0),
        circle_at(10.0 + d, 10.0 + d),
        (0.0, 1.0),
    )
    .unwrap();
    close(area(&s, s.domain()), 369.695_302_085_958_93, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(
            p[0].hypot(p[1]) - 10.0,
            p[2] - 10.0,
            1e-12,
            "45 degrees: dr = dz",
        );
    });
}

/// surfaces/cov-ruled-cylinder-rim-tangent-to-the-top-is-a-flat-ring.json: a strip off a rim
/// continuing the top face is the flat ring r 10 to 15 at z = 10, area pi (225 - 100) =
/// 392.6990816987241.
#[test]
fn surfaces_cov_a_ruled_strip_continuing_the_top_is_a_flat_ring() {
    let s = Ruled::new(circle_at(10.0, 10.0), circle_at(15.0, 10.0), (0.0, 1.0)).unwrap();
    close(area(&s, s.domain()), 392.699_081_698_724_1, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        assert!((p[2] - 10.0).abs() < 1e-13, "{p:?}");
    });
}

/// surfaces/cov-loft-two-circles-ruled-frustum.json and surfaces/cov-sweep-circle-scaled-half-is-a-frustum.json:
/// the ruled surface between circles r 10 (z 0) and r 5 (z 10) has area pi (10 + 5) sqrt(5^2 + 10^2)
/// = 526.8611048280544; between r 10 (z 0) and r 5 (z 20), pi (10 + 5) sqrt(5^2 + 20^2) =
/// 971.4838757561392 (the sweep that scales a circle to half over 20).
#[test]
fn surfaces_cov_a_ruled_surface_between_coaxial_circles_is_a_frustum() {
    let s = Ruled::new(circle_at(10.0, 0.0), circle_at(5.0, 10.0), (0.0, 1.0)).unwrap();
    close(
        area(&s, s.domain()),
        526.861_104_828_054_4,
        1e-11,
        "loft, height 10",
    );
    let s = Ruled::new(circle_at(10.0, 0.0), circle_at(5.0, 20.0), (0.0, 1.0)).unwrap();
    close(
        area(&s, s.domain()),
        971.483_875_756_139_2,
        1e-11,
        "scaled sweep, height 20",
    );
    on_grid(&s, s.domain(), |p| {
        close(
            p[0].hypot(p[1]),
            10.0 - 0.25 * p[2],
            1e-12,
            "radius at height",
        );
    });
}

/// surfaces/cov-extend-quarter-cylinder-along-its-axis.json: a quarter cylinder r 10, 20 high,
/// extended naturally by 5: the same cylinder 25 high, area (pi/2) 10 25 = 392.6990816987241. A
/// natural extension keeps the surface's own equation: the cylinder over a longer v range.
#[test]
fn surfaces_cov_a_quarter_cylinder_extended_along_its_axis_stays_the_cylinder() {
    let s = Cylinder::new([0.0; 3], 10.0, standard(), ((0.0, 0.5), (0.0, 25.0))).unwrap();
    close(area(&s, s.domain()), 392.699_081_698_724_1, 1e-11, "area");
    on_grid(&s, s.domain(), |p| {
        close(p[0].hypot(p[1]), 10.0, 1e-13, "radius");
    });
}

/// surfaces/cov-fair-single-span-face-has-nothing-free-to-move.json: the cubic Bezier (0,0),
/// (13,8), (27,-8), (40,0) in the plane y = 0 extruded 20: one span each way, area
/// 827.9663653036151 (20 times the curve's length).
#[test]
fn surfaces_cov_fair_the_extruded_cubic_bezier_has_the_entrys_area() {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [
        [0.0, 0.0, 0.0],
        [13.0, 0.0, 8.0],
        [27.0, 0.0, -8.0],
        [40.0, 0.0, 0.0],
    ];
    let curve = Curve::<3>::new(basis, &points).unwrap();
    let profile = Trimmed::whole(Leaf::Nurbs(curve)).unwrap();
    let s = Extrusion::new(profile, [0.0, -1.0, 0.0], (0.0, 20.0)).unwrap();
    close(area(&s, s.domain()), 827.966_365_303_615_1, 1e-9, "area");
}

// ---------------------------------------------------------------------------------------------
// surfaces: offsets

/// The unit normal S_u x S_v of `s` at (u, v).
fn unit_normal(s: &impl ParamSurface, u: f64, v: f64) -> [f64; 3] {
    let [_, su, sv] = s.partials(u, v, SIDES).unwrap();
    let n = cross(su, sv);
    let l = length(n);
    [n[0] / l, n[1] / l, n[2] / l]
}

/// surfaces/cov-offset-cylinder-side-out.json: the offset of a cylinder r 10 by 2 along its
/// normal is the coaxial cylinder r 12 (area 2 pi 12 20 = 1507.9644737231006). The offset runs
/// along the normal S_u x S_v, which the entry calls outward: the test reads the base's normal,
/// so the answer holds for either orientation.
#[test]
fn surfaces_cov_an_offset_of_a_cylinder_is_the_coaxial_cylinder() {
    let base = Cylinder::new([0.0; 3], 10.0, standard(), ((0.0, 2.0), (0.0, 20.0))).unwrap();
    let outward = {
        let p = base.point(0.0, 0.0, SIDES).unwrap();
        let n = unit_normal(&base, 0.0, 0.0);
        n[0] * p[0] + n[1] * p[1] > 0.0
    };
    let d = if outward { 2.0 } else { -2.0 };
    let s = Offset::new(SurfaceKind::Cylinder(base), d).unwrap();
    on_grid(&s, s.domain(), |p| {
        close(p[0].hypot(p[1]), 12.0, 1e-9, "offset cylinder radius");
    });
    close(
        area(&s, s.domain()),
        1_507.964_473_723_100_6,
        1e-9,
        "cylinder area",
    );
}

/// surfaces/cov-offset-sphere-inward-closes-a-solid.json: the offset of a sphere r 10 by -4 along
/// its outward normal is the sphere r 6 (area 4 pi 36 = 452.3893421169302).
#[test]
fn surfaces_cov_an_offset_of_a_sphere_inward_is_the_concentric_sphere() {
    let base = Sphere::new([0.0; 3], 10.0, standard(), ((0.0, 2.0), (-0.5, 0.5))).unwrap();
    let p = base.point(0.3, 0.1, SIDES).unwrap();
    let n = unit_normal(&base, 0.3, 0.1);
    let outward = n[0] * p[0] + n[1] * p[1] + n[2] * p[2] > 0.0;
    let d = if outward { -4.0 } else { 4.0 };
    let s = Offset::new(SurfaceKind::Sphere(base), d).unwrap();
    // Away from the poles, where the base's normal does not vanish, every point is on the sphere
    // r 6; at a pole the offset kind refuses (`CurveError::Unbounded`, its documented guarantee:
    // an enclosure is refused where the base's normal may vanish), and never gives a wrong point.
    for iu in 0..=10 {
        for iv in 0..=18 {
            let u = 2.0 * f64::from(iu) / 10.0;
            let v = -0.45 + 0.9 * f64::from(iv) / 18.0;
            close(
                length(s.point(u, v, SIDES).unwrap()),
                6.0,
                1e-9,
                "offset sphere radius",
            );
        }
    }
    for pole in [-0.5, 0.5] {
        match s.point(0.0, pole, SIDES) {
            Ok(p) => close(length(p), 6.0, 1e-9, "the pole, if it is given"),
            Err(e) => assert_eq!(e, CurveError::Unbounded, "the pole's refusal"),
        }
    }
    close(
        area(&s, s.domain()),
        452.389_342_116_930_2,
        1e-9,
        "sphere area",
    );
}

/// surfaces/cov-offset-box-top-face-up.json: the top face of a 20 x 10 x 10 box offset 5 up: a
/// plane moves exactly, to the square 20 x 10 at z = 15, area 200.
#[test]
fn surfaces_cov_an_offset_of_a_plane_moves_it_exactly() {
    let base = Plane::new(
        [0.0, 0.0, 10.0],
        [20.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        ((0.0, 1.0), (0.0, 1.0)),
    )
    .unwrap();
    let n = unit_normal(&base, 0.5, 0.5);
    assert!((n[2] - 1.0).abs() < 1e-15, "the normal is +z: {n:?}");
    let s = Offset::new(SurfaceKind::Plane(base), 5.0).unwrap();
    close(area(&s, s.domain()), 200.0, 1e-12, "area");
    on_grid(&s, s.domain(), |p| {
        assert!((p[2] - 15.0).abs() < 1e-12, "{p:?}");
    });
}
