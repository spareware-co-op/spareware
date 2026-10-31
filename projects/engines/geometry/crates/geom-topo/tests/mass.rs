//! Certified mass properties with implicit corners (10.2, 19.2; piece G102): each interval holds
//! the exact rational value, computed here with big rationals from the same planes, and is tight;
//! grid corners agree with `query`'s exact sums.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_num::P3;
use geom_num::interval::Interval;
use geom_num::plane::{Lpi, Plane, Tpi};
use geom_run::{Budget, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};
use geom_topo::mass::{Corner, loop_area, shell_volume};
use geom_topo::query::QueryError;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn plane(a: [i64; 3], b: [i64; 3], c: [i64; 3]) -> Plane {
    Plane::through(
        p(a[0], a[1], a[2]),
        p(b[0], b[1], b[2]),
        p(c[0], c[1], c[2]),
    )
    .unwrap()
}

/// A corner's exact point.
fn exact(c: &Corner) -> [BigRational; 3] {
    match c {
        Corner::Grid(q) => {
            [q.x, q.y, q.z].map(|v| BigRational::from_integer(BigInt::from(v.get())))
        }
        Corner::Tpi(t) => {
            let (x, w) = t.homogeneous().unwrap();
            let w = big(&w.resize::<5>().unwrap());
            x.map(|n| BigRational::new(big(&n), w.clone()))
        }
        Corner::Lpi(l) => {
            let (x, w) = l.homogeneous().unwrap();
            x.map(|n| BigRational::new(big(&n.resize::<5>().unwrap()), BigInt::from(w)))
        }
    }
}

/// A 320-bit integer as a big integer.
fn big(v: &geom_num::I320) -> BigInt {
    let limbs = v.to_le_limbs();
    let mut out = BigInt::from(0);
    for limb in limbs.iter().rev() {
        out = (out << 64) + BigInt::from(*limb);
    }
    if v.is_negative() {
        out - (BigInt::from(1) << 320)
    } else {
        out
    }
}

fn exact_six_volume(loops: &[&[Corner]]) -> BigRational {
    let mut sum = BigRational::from_integer(BigInt::from(0));
    for corners in loops {
        let pts: Vec<_> = corners.iter().map(exact).collect();
        for k in 1..pts.len() - 1 {
            let (a, b, c) = (&pts[0], &pts[k], &pts[k + 1]);
            let cross = [
                &b[1] * &c[2] - &b[2] * &c[1],
                &b[2] * &c[0] - &b[0] * &c[2],
                &b[0] * &c[1] - &b[1] * &c[0],
            ];
            sum += &a[0] * &cross[0] + &a[1] * &cross[1] + &a[2] * &cross[2];
        }
    }
    sum
}

/// Whether `i` holds the exact `r`, compared exactly.
fn holds(i: Interval, r: &BigRational) -> bool {
    let lo = BigRational::from_float(i.lo()).unwrap();
    let hi = BigRational::from_float(i.hi()).unwrap();
    &lo <= r && r <= &hi
}

/// The width of `i` over the size of `r`, as a double.
fn relative(i: Interval, r: &BigRational) -> f64 {
    let width = BigRational::from_float(i.hi()).unwrap() - BigRational::from_float(i.lo()).unwrap();
    (width / r.abs()).to_f64().unwrap()
}

/// The tetrahedron cut from the corner of the positive octant by a plane through (a, 0, 0) ...: its
/// corner on the x axis implicit, the meeting of the plane 3x + y + z = 1 scaled by `scale` with
/// y = 0 and z = 0.
fn tetrahedron(scale: i64) -> [Vec<Corner>; 4] {
    let tilted = plane([0, scale, 0], [0, 0, scale], [scale, -scale, -scale]);
    let x_axis = Tpi::new(
        tilted,
        plane([0, 0, 0], [1, 0, 0], [0, 0, 1]),
        plane([0, 0, 0], [1, 0, 0], [0, 1, 0]),
    )
    .unwrap();
    let o = || Corner::Grid(p(0, 0, 0));
    let y = || Corner::Grid(p(0, scale, 0));
    let z = || Corner::Grid(p(0, 0, scale));
    let x = || Corner::Tpi(x_axis);
    [
        vec![o(), y(), x()],
        vec![o(), x(), z()],
        vec![o(), z(), y()],
        vec![x(), y(), z()],
    ]
}

#[test]
fn a_tetrahedron_with_an_implicit_corner_holds_its_exact_volume() {
    for scale in [1, 7, 1000, 1 << 38] {
        let faces = tetrahedron(scale);
        let loops: Vec<&[Corner]> = faces.iter().map(Vec::as_slice).collect();
        let six = shell_volume(&loops, &mut meter()).unwrap();
        let want = exact_six_volume(&loops);
        // 6V = x y z for corners on the axes, x = scale / 3.
        let s = BigRational::from_integer(BigInt::from(scale));
        assert_eq!(
            want,
            &s * &s * &s / BigRational::from_integer(BigInt::from(3))
        );
        assert!(holds(six, &want), "{scale}: {six:?}");
        assert!(relative(six, &want) < 1e-14, "{scale}: {six:?}");
    }
}

#[test]
fn grid_corners_agree_with_the_exact_sums() {
    let c = |x, y, z| Corner::Grid(p(x, y, z));
    let square = [c(0, 0, 0), c(4, 0, 0), c(4, 3, 0), c(0, 3, 0)];
    let area = loop_area(&square, &mut meter()).unwrap();
    assert!(area[0].contains(0.0) && area[1].contains(0.0));
    assert!(area[0].hi() - area[0].lo() < 1e-300);
    assert!(area[2].contains(24.0));
    assert!(area[2].hi() - area[2].lo() < 1e-12);
    let cube: [[Corner; 4]; 6] = [
        [c(0, 0, 0), c(0, 2, 0), c(2, 2, 0), c(2, 0, 0)],
        [c(0, 0, 2), c(2, 0, 2), c(2, 2, 2), c(0, 2, 2)],
        [c(0, 0, 0), c(2, 0, 0), c(2, 0, 2), c(0, 0, 2)],
        [c(0, 2, 0), c(0, 2, 2), c(2, 2, 2), c(2, 2, 0)],
        [c(0, 0, 0), c(0, 0, 2), c(0, 2, 2), c(0, 2, 0)],
        [c(2, 0, 0), c(2, 2, 0), c(2, 2, 2), c(2, 0, 2)],
    ];
    let loops: Vec<&[Corner]> = cube.iter().map(<[Corner; 4]>::as_slice).collect();
    let six = shell_volume(&loops, &mut meter()).unwrap();
    assert!(six.contains(48.0), "{six:?}");
    assert!(six.hi() - six.lo() < 1e-12);
}

#[test]
fn every_kind_of_corner_is_held_on_random_shapes() {
    let mut r = SplitMix64::new(resolve("g102-mass", Seed(0x6102)).unwrap());
    for _ in 0..200 {
        let mut coord = || r.range_i64(-1000, 1000).unwrap();
        let (a, b, d) = (
            [coord(), coord(), coord()],
            [coord(), coord(), coord()],
            [coord(), coord(), coord()],
        );
        let (e, f, g) = (
            [coord(), coord(), coord()],
            [coord(), coord(), coord()],
            [coord(), coord(), coord()],
        );
        let (h, i) = ([coord(), coord(), coord()], [coord(), coord(), coord()]);
        let (Ok(u), Ok(v), Ok(w)) = (
            Plane::through(
                p(a[0], a[1], a[2]),
                p(b[0], b[1], b[2]),
                p(d[0], d[1], d[2]),
            ),
            Plane::through(
                p(e[0], e[1], e[2]),
                p(f[0], f[1], f[2]),
                p(g[0], g[1], g[2]),
            ),
            Plane::through(
                p(h[0], h[1], h[2]),
                p(i[0], i[1], i[2]),
                p(a[0], a[1], a[2]),
            ),
        ) else {
            continue;
        };
        let Ok(t) = Tpi::new(u, v, w) else { continue };
        let Ok(l) = Lpi::new(p(a[0], a[1], a[2]), p(e[0], e[1], e[2]), v) else {
            continue;
        };
        let corners = [
            Corner::Tpi(t),
            Corner::Lpi(l),
            Corner::Grid(p(b[0], b[1], b[2])),
            Corner::Grid(p(f[0], f[1], f[2])),
        ];
        let loops: [&[Corner]; 1] = [&corners];
        let six = shell_volume(&loops, &mut meter()).unwrap();
        assert!(holds(six, &exact_six_volume(&loops)), "{six:?}");
        let area = loop_area(&corners, &mut meter()).unwrap();
        for (k, component) in area.iter().enumerate() {
            let pts: Vec<_> = corners.iter().map(exact).collect();
            let mut sum = BigRational::from_integer(BigInt::from(0));
            for n in 0..4 {
                let (s, t) = (&pts[n], &pts[(n + 1) % 4]);
                let (j, m) = ((k + 1) % 3, (k + 2) % 3);
                sum += &s[j] * &t[m] - &s[m] * &t[j];
            }
            assert!(holds(*component, &sum), "{component:?}");
        }
    }
}

#[test]
fn an_empty_shell_and_a_small_loop_add_nothing() {
    assert!(shell_volume(&[], &mut meter()).unwrap().contains(0.0));
    let none: [Corner; 0] = [];
    let loops: [&[Corner]; 1] = [&none];
    assert_eq!(shell_volume(&loops, &mut meter()).unwrap().lo(), 0.0);
    assert_eq!(shell_volume(&loops, &mut meter()).unwrap().hi(), 0.0);
    let two = [Corner::Grid(p(1, 2, 3)), Corner::Grid(p(4, 5, 6))];
    let loops: [&[Corner]; 1] = [&two];
    assert_eq!(shell_volume(&loops, &mut meter()).unwrap().hi(), 0.0);
    let area = loop_area(&two, &mut meter()).unwrap();
    assert!(area.iter().all(|c| c.contains(0.0)));
}

#[test]
fn each_corner_visited_is_charged_and_stops_the_meter() {
    let faces = tetrahedron(5);
    let loops: Vec<&[Corner]> = faces.iter().map(Vec::as_slice).collect();
    let mut m = meter();
    shell_volume(&loops, &mut m).unwrap();
    assert_eq!(m.used().work, 4);
    let mut m = meter();
    loop_area(&faces[0], &mut m).unwrap();
    assert_eq!(m.used().work, 3);
    for work in 0..4 {
        assert!(matches!(
            shell_volume(&loops, &mut WorkMeter::new(Budget::of_work(work))),
            Err(QueryError::Stopped(_))
        ));
    }
    assert!(shell_volume(&loops, &mut WorkMeter::new(Budget::of_work(4))).is_ok());
    assert!(matches!(
        loop_area(&faces[0], &mut WorkMeter::new(Budget::of_work(2))),
        Err(QueryError::Stopped(_))
    ));
}
