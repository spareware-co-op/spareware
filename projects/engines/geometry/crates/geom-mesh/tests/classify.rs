//! Point location through the public interface (piece G147), against rational oracles written apart
//! from the code: a height field's exact height at a point, and boxes.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test oracles and generators; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroU64;

use geom_mesh::classify::{Classifier, Located};
use geom_mesh::exact::build::{Corner, Polygons, build};
use geom_mesh::exact::{ExactMesh, MeshError};
use geom_num::grid::P3;
use geom_num::plane::{Plane, Tpi};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

mod shapes;

use shapes::{cuboid, frustum, g, prism, soup};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

fn classifier(m: &ExactMesh) -> Classifier<'_> {
    Classifier::new(m, &Exec::serial(), &mut meter()).unwrap()
}

fn locate(c: &Classifier<'_>, q: P3) -> Located {
    c.locate(q, &mut meter()).unwrap()
}

#[test]
fn a_box_knows_its_inside_its_outside_and_its_surface() {
    let m = mesh(&cuboid(4, 4, 4));
    let c = classifier(&m);
    // The ray from these points runs up corners and edges in plan: the moved start settles them.
    for x in 0..=4 {
        for y in 0..=4 {
            for z in -2..=6 {
                let strictly = (1..4).contains(&x) && (1..4).contains(&y) && (1..4).contains(&z);
                let closed = (0..=4).contains(&x) && (0..=4).contains(&y) && (0..=4).contains(&z);
                let want = if strictly {
                    Located::Winding(1)
                } else if closed {
                    Located::OnSurface
                } else {
                    Located::Winding(0)
                };
                assert_eq!(locate(&c, p(x, y, z)), want, "({x}, {y}, {z})");
            }
        }
    }
    assert_eq!(locate(&c, p(-1, 2, 2)), Located::Winding(0));
    assert_eq!(locate(&c, p(5, 2, 2)), Located::Winding(0));
}

#[test]
fn an_inside_out_box_winds_the_other_way() {
    let mut input = cuboid(4, 4, 4);
    for f in input.corners.chunks_mut(4) {
        f.reverse();
    }
    let m = mesh(&input);
    let c = classifier(&m);
    assert_eq!(locate(&c, p(2, 2, 2)), Located::Winding(-1));
    assert_eq!(locate(&c, p(9, 2, 2)), Located::Winding(0));
}

/// Where the height field of `heights` stands at (x, y): the sign of z − h as a fraction over the
/// step, or `None` off the field's plan.
fn against_field(heights: &[Vec<i64>], step: i64, q: P3) -> Option<(i128, i128)> {
    let (x, y, z) = (q.x.get(), q.y.get(), q.z.get());
    let n = heights.len() as i64;
    if x < 0 || y < 0 || x > (n - 1) * step || y > (n - 1) * step {
        return None;
    }
    let (cx, cy) = ((x / step).min(n - 2), (y / step).min(n - 2));
    let (dx, dy) = (i128::from(x - cx * step), i128::from(y - cy * step));
    let s = i128::from(step);
    let h = |a: i64, b: i64| i128::from(heights[b as usize][a as usize]);
    let (h00, h10, h11, h01) = (h(cx, cy), h(cx + 1, cy), h(cx + 1, cy + 1), h(cx, cy + 1));
    // Cells split along the diagonal from (0, 0) to (1, 1): s·height, exactly.
    let scaled = if dx >= dy {
        h00 * s + (h10 - h00) * dx + (h11 - h10) * dy
    } else {
        h00 * s + (h01 - h00) * dy + (h11 - h01) * dx
    };
    Some((i128::from(z) * s - scaled, s))
}

#[test]
fn height_fields_agree_with_their_heights() {
    let seed = resolve("geom-mesh classify fields", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    let below = |rng: &mut SplitMix64, n: u64| rng.below(NonZeroU64::new(n).unwrap());
    let (mut inside, mut outside, mut on) = (0, 0, 0);
    for _ in 0..24 {
        let n = 3 + below(&mut rng, 5) as usize;
        let step = 4 * (1 + below(&mut rng, 3) as i64);
        let heights: Vec<Vec<i64>> = (0..n)
            .map(|_| (0..n).map(|_| 1 + below(&mut rng, 16) as i64).collect())
            .collect();
        let m = mesh(&prism(&heights, step).0);
        let c = classifier(&m);
        for _ in 0..4000 {
            let span = (n as u64 + 1) * step as u64;
            let q = p(
                below(&mut rng, span) as i64 - step / 2,
                below(&mut rng, span) as i64 - step / 2,
                below(&mut rng, 24) as i64 - 2,
            );
            let want = match against_field(&heights, step, q) {
                None => Located::Winding(0),
                Some((top, _)) if q.z.get() < 0 || top > 0 => Located::Winding(0),
                Some((0, _)) => Located::OnSurface,
                Some(_) if q.z.get() == 0 => Located::OnSurface,
                Some(_) => {
                    let on_wall = q.x.get() == 0
                        || q.y.get() == 0
                        || q.x.get() == (n as i64 - 1) * step
                        || q.y.get() == (n as i64 - 1) * step;
                    if on_wall {
                        Located::OnSurface
                    } else {
                        Located::Winding(1)
                    }
                }
            };
            assert_eq!(locate(&c, q), want, "{q:?}");
            match want {
                Located::Winding(1) => inside += 1,
                Located::Winding(_) => outside += 1,
                Located::OnSurface => on += 1,
            }
        }
    }
    assert!(
        inside > 1000 && outside > 1000 && on > 100,
        "{inside} {outside} {on}"
    );
}

#[test]
fn faces_off_the_grid_are_crossed_exactly() {
    let m = mesh(&frustum());
    let c = classifier(&m);
    // The side x = −2 + (2/3)·z ... the corners at z = 1 are at ±4/3, so (1, 1) under z = 1 is in.
    assert_eq!(locate(&c, p(0, 0, 0)), Located::OnSurface);
    assert_eq!(locate(&c, p(1, 0, 2)), Located::Winding(0));
    assert_eq!(locate(&c, p(0, 0, 1)), Located::OnSurface);
    assert_eq!(locate(&c, p(1, 1, 1)), Located::OnSurface);
    assert_eq!(locate(&c, p(2, 2, 1)), Located::Winding(0));
    assert_eq!(locate(&c, p(0, 0, -1)), Located::Winding(0));
    assert_eq!(locate(&c, p(3, 0, 0)), Located::Winding(0));
    assert_eq!(locate(&c, p(1, 1, -1)), Located::Winding(0));
}

#[test]
fn a_taller_frustum_holds_grid_points_under_faces_off_the_grid() {
    // The same pyramid scaled three times, cut at z = 4 of 9: the top corners are at ±10/3.
    let mut input = frustum();
    let scale = |plane: Plane, k: i128| Plane::new(plane.n(), plane.d() * k).unwrap();
    let wide = |c: Corner| match c {
        Corner::Grid(q) => g(q.x.get() * 3, q.y.get() * 3, q.z.get() * 3),
        Corner::Planes(ps) => Corner::Planes(ps.map(|q| scale(q, 3))),
    };
    input.vertices = input.vertices.iter().copied().map(wide).collect();
    input.face_plane = input
        .face_plane
        .iter()
        .map(|q| q.map(|q| scale(q, 3)))
        .collect();
    input.edge_plane = input
        .edge_plane
        .iter()
        .map(|q| q.map(|q| scale(q, 3)))
        .collect();
    let m = mesh(&input);
    let c = classifier(&m);
    assert_eq!(locate(&c, p(0, 0, 1)), Located::Winding(1));
    assert_eq!(locate(&c, p(3, 3, 1)), Located::Winding(1));
    assert_eq!(locate(&c, p(3, -3, 2)), Located::Winding(1));
    assert_eq!(locate(&c, p(5, 0, 2)), Located::Winding(0));
    assert_eq!(locate(&c, p(0, 0, 4)), Located::Winding(0));
    assert_eq!(locate(&c, p(0, 0, 3)), Located::OnSurface);
}

#[test]
fn three_plane_points_are_traced() {
    let m = mesh(&cuboid(9, 9, 9));
    let c = classifier(&m);
    let plane = |n: [i128; 3], d: i128| Plane::new(n, d).unwrap();
    let thirds = |x: i128, y: i128, z: i128| {
        Tpi::new(
            plane([3, 0, 0], -x),
            plane([0, 3, 0], -y),
            plane([0, 0, 3], -z),
        )
        .unwrap()
    };
    assert_eq!(c.beside(&thirds(10, 10, 10), &mut meter()), Ok(1));
    assert_eq!(c.beside(&thirds(-1, 10, 10), &mut meter()), Ok(0));
    assert_eq!(c.beside(&thirds(10, 10, 28), &mut meter()), Ok(0));
    assert_eq!(c.beside(&thirds(1, 1, 1), &mut meter()), Ok(1));
    assert_eq!(c.beside(&thirds(-1, -1, -1), &mut meter()), Ok(0));
}

#[test]
fn an_open_mesh_is_refused() {
    let m = mesh(&soup(
        vec![g(0, 0, 0), g(4, 0, 0), g(0, 4, 0)],
        &[&[0, 1, 2]],
    ));
    assert_eq!(
        Classifier::new(&m, &Exec::serial(), &mut meter()).err(),
        Some(MeshError::NotClosed)
    );
}

#[test]
fn the_points_come_back_in_order_on_any_pool() {
    use geom_pool::native::Rayon;

    let m = mesh(&cuboid(40, 40, 40));
    let c = classifier(&m);
    let points: Vec<P3> = (0..3000)
        .map(|i| p(i % 50, (i * 7) % 50, (i * 13) % 50))
        .collect();
    let serial = c
        .locate_all(&points, &Exec::serial(), &mut meter())
        .unwrap();
    let rayon = Rayon::new(std::num::NonZeroUsize::new(3).unwrap()).unwrap();
    let pooled = c
        .locate_all(&points, &Exec::with_pool(&rayon), &mut meter())
        .unwrap();
    assert_eq!(serial, pooled);
    for (q, got) in points.iter().zip(&serial) {
        assert_eq!(*got, locate(&c, *q));
    }
}
