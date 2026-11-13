//! Attacks on G246's claims (chapter 11.4 step 6): ray classification where its tests decide.
//! Flat bodies against their exact classification at every point of a lattice, with points whose
//! first rays run through edges and corners; points on a face and beside it; a body whose faces
//! meet only within a bound; a sphere against its closed form, with a ray that grazes it; a
//! cylinder whose side is a curved face with loops; and whole arrangements: one answer for each
//! group of split faces, the same on every run, and a typed refusal where a group lies in a face
//! of the other operand.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeMap;

use geom_bool::carrier::{Bound, CarrierIx, PointGeom};
use geom_bool::classify::{Boundary, Group, RAYS, classify, groups};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, FaceIn, Interfered, VertexIn, interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::split::{Splitting, split};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_num::plane::Plane;
use geom_num::{P3, Steps};
use geom_run::{Budget, Exec, WorkMeter};
use geom_surface::{Cylinder, Sphere, SurfaceKind};

/// The tests' unit, in steps: a millimetre on a grid of a micrometre.
const S: i64 = 1000;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 34))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x * S, y * S, z * S).unwrap()
}

fn coords(q: P3) -> [f64; 3] {
    [q.x.get() as f64, q.y.get() as f64, q.z.get() as f64]
}

#[derive(Default)]
struct Build {
    arr: Arrangement,
    /// For each face, whether its outside is the side its carrier's normal points to.
    outward: Vec<bool>,
}

impl Build {
    fn vertex(&mut self, at: P3, operand: u32) -> usize {
        self.arr.vertices.push(VertexIn {
            point: PointGeom::Grid(at),
            operand,
        });
        self.arr.vertices.len() - 1
    }

    fn edge(&mut self, curve: CurveKind<3>, ends: Option<[usize; 2]>, operand: u32) -> usize {
        let curve = self.arr.carriers.add_curve(curve).unwrap();
        self.arr.edges.push(EdgeIn {
            curve,
            ends,
            operand,
        });
        self.arr.edges.len() - 1
    }

    fn at(&self, v: usize) -> P3 {
        match self.arr.vertices[v].point {
            PointGeom::Grid(q) => q,
            _ => panic!("a grid vertex"),
        }
    }

    fn line(&mut self, a: usize, b: usize, operand: u32) -> usize {
        let (o, e) = (coords(self.at(a)), coords(self.at(b)));
        let d = [e[0] - o[0], e[1] - o[1], e[2] - o[2]];
        let line = Line::new(o, d, (0.0, 1.0)).unwrap();
        self.edge(CurveKind::Line(line), Some([a, b]), operand)
    }

    fn plane(&mut self, n: [i128; 3], d: i128) -> CarrierIx {
        CarrierIx::Plane(
            self.arr
                .carriers
                .add_plane(&Plane::new(n, d).unwrap())
                .unwrap(),
        )
    }

    fn face(
        &mut self,
        surface: CarrierIx,
        loops: Vec<Vec<usize>>,
        operand: u32,
        outward: bool,
    ) -> usize {
        self.arr.faces.push(FaceIn {
            surface,
            loops,
            operand,
        });
        self.outward.push(outward);
        self.arr.faces.len() - 1
    }

    /// A box from `lo` to `hi`, in steps: its faces x low, x high, y low, y high, z low, z high.
    fn cuboid(&mut self, lo: [i64; 3], hi: [i64; 3], operand: u32) -> [usize; 6] {
        let mut corners = [0; 8];
        for (k, slot) in corners.iter_mut().enumerate() {
            let pick = |axis: usize| {
                if k >> axis & 1 == 1 {
                    hi[axis]
                } else {
                    lo[axis]
                }
            };
            let at = P3::new(pick(0), pick(1), pick(2)).unwrap();
            *slot = self.vertex(at, operand);
        }
        let mut edge_of = BTreeMap::new();
        for k in 0..8_usize {
            for axis in 0..3 {
                if k >> axis & 1 == 0 {
                    let j = k | 1 << axis;
                    let e = self.line(corners[k], corners[j], operand);
                    edge_of.insert((k, j), e);
                }
            }
        }
        let mut faces = [0; 6];
        for axis in 0..3 {
            let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
            for side in 0..2_usize {
                let ring: Vec<usize> = [(0, 0), (1, 0), (1, 1), (0, 1)]
                    .iter()
                    .map(|&(sb, sc)| side << axis | sb << b | sc << c)
                    .collect();
                let edges = (0..4)
                    .map(|k| {
                        let (u, v) = (ring[k], ring[(k + 1) % 4]);
                        edge_of[&(u.min(v), u.max(v))]
                    })
                    .collect();
                let mut n = [0_i128; 3];
                n[axis] = 1;
                let at = if side == 1 { hi[axis] } else { lo[axis] };
                let plane = self.plane(n, -i128::from(at));
                // The table's normal points up the axis: outward on the high side.
                faces[2 * axis + side] = self.face(plane, vec![edges], operand, side == 1);
            }
        }
        faces
    }

    fn boundary(&self, operand: u32, bound: u64) -> Boundary {
        Boundary::new(
            &self.arr,
            operand,
            &self.outward,
            Bound(Steps(bound)),
            &mut meter(),
        )
        .unwrap()
    }

    fn run(&self) -> (Interfered, Splitting) {
        let out = interfere(
            &self.arr,
            default_remaining(),
            &Exec::serial(),
            &mut meter(),
        )
        .unwrap();
        let splitting = split(&self.arr, &out, &mut meter()).unwrap();
        (out, splitting)
    }

    fn classify(&self, operands: &[u32]) -> Result<Vec<Group>, BuildError> {
        let (out, splitting) = self.run();
        let bounds: BTreeMap<u32, Bound> = operands.iter().map(|&o| (o, Bound::ZERO)).collect();
        classify(
            &self.arr,
            &out,
            &splitting,
            &self.outward,
            &bounds,
            &mut meter(),
        )
    }
}

/// Whether a grid point is strictly inside the box, strictly outside it, or on it, exactly.
fn in_box(q: P3, lo: [i64; 3], hi: [i64; 3]) -> Option<bool> {
    let at = [q.x.get(), q.y.get(), q.z.get()];
    if (0..3).all(|k| lo[k] < at[k] && at[k] < hi[k]) {
        Some(true)
    } else if (0..3).any(|k| at[k] < lo[k] || at[k] > hi[k]) {
        Some(false)
    } else {
        None
    }
}

#[test]
fn a_box_agrees_with_its_exact_classification_on_a_lattice() {
    let mut b = Build::default();
    let (lo, hi) = ([0, 0, 0], [10 * S, 10 * S, 10 * S]);
    b.cuboid(lo, hi, 0);
    let boundary = b.boundary(0, 0);
    let (mut inside, mut outside, mut later_ray) = (0, 0, 0);
    // The lattice holds the box's own planes, so many first rays run along faces, through edges
    // and through corners.
    let stops = [-5 * S, 0, 3 * S, 5 * S, 10 * S, 14 * S];
    for &x in &stops {
        for &y in &stops {
            for &z in &stops {
                let q = P3::new(x, y, z).unwrap();
                let found = boundary.winding(&b.arr.carriers, q, &mut meter());
                match in_box(q, lo, hi) {
                    Some(true) => {
                        let w = found.unwrap();
                        assert_eq!((w.number, w.crossings), (1, 1), "{q:?}");
                        inside += 1;
                    }
                    Some(false) => {
                        let w = found.unwrap();
                        assert_eq!(w.number, 0, "{q:?} by ray {}", w.ray);
                        assert!(w.crossings == 0 || w.crossings == 2, "{q:?}");
                        later_ray += usize::from(w.ray > 0);
                        outside += 1;
                    }
                    // On the box: no winding number, and no ray may claim one.
                    None => assert!(
                        matches!(found, Err(BuildError::NoRay { operand: 0, at, .. }) if at == q),
                        "{q:?}: {found:?}"
                    ),
                }
            }
        }
    }
    assert_eq!(inside, 8);
    assert!(outside > 100 && later_ray >= 10, "{outside} {later_ray}");
}

#[test]
fn a_ray_through_an_edge_or_a_corner_is_dropped_for_the_next() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    let boundary = b.boundary(0, 0);
    // Along x at y = 0: the first ray runs along two faces and through four edges.
    let w = boundary
        .winding(&b.arr.carriers, p(-5, 0, 5), &mut meter())
        .unwrap();
    assert_eq!(w.number, 0);
    assert!(w.ray > 0, "{w:?}");
    // On the box's diagonal: (1, 1, 1) would run corner to corner, and the axes miss the box.
    let w = boundary
        .winding(&b.arr.carriers, p(-5, -5, -5), &mut meter())
        .unwrap();
    assert_eq!((w.number, w.ray), (0, 0), "the first ray misses the box");
    // Inside, with the first three rays through the middles of faces: each is good.
    let w = boundary
        .winding(&b.arr.carriers, p(5, 5, 5), &mut meter())
        .unwrap();
    assert_eq!((w.number, w.crossings, w.ray), (1, 1, 0));
    // Inside, level with nothing: the first ray leaves through a face a step from its edge.
    let q = P3::new(5 * S, 1, 5 * S).unwrap();
    let w = boundary.winding(&b.arr.carriers, q, &mut meter()).unwrap();
    assert_eq!(w.number, 1, "{w:?}");
}

#[test]
fn a_point_beside_a_face_winds_but_is_not_clear() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    let boundary = b.boundary(0, 0);
    let carriers = &b.arr.carriers;
    // A step inside the face x = 0, and a step outside it.
    let (just_in, just_out) = (
        P3::new(1, 5 * S, 5 * S).unwrap(),
        P3::new(-1, 5 * S, 5 * S).unwrap(),
    );
    assert_eq!(
        boundary
            .winding(carriers, just_in, &mut meter())
            .unwrap()
            .number,
        1
    );
    assert_eq!(
        boundary
            .winding(carriers, just_out, &mut meter())
            .unwrap()
            .number,
        0
    );
    // Neither has a clear box of a step: the face is that near.
    for q in [just_in, just_out] {
        assert!(
            !boundary
                .clear(carriers, q, Bound(Steps(1)), &mut meter())
                .unwrap()
        );
    }
    // A hundred steps off, the box of a step is clear; of a hundred, not.
    let off = P3::new(-100, 5 * S, 5 * S).unwrap();
    assert!(
        boundary
            .clear(carriers, off, Bound(Steps(1)), &mut meter())
            .unwrap()
    );
    assert!(
        !boundary
            .clear(carriers, off, Bound(Steps(100)), &mut meter())
            .unwrap()
    );
    // Beside the face's plane but past its edge: clear, by the place in the face.
    let past = P3::new(0, 5 * S, 12 * S).unwrap();
    assert!(
        boundary
            .clear(carriers, past, Bound(Steps(1)), &mut meter())
            .unwrap()
    );
    // On the face: never clear.
    assert!(
        !boundary
            .clear(carriers, p(0, 5, 5), Bound::ZERO, &mut meter())
            .unwrap()
    );
}

#[test]
fn a_body_with_gaps_keeps_rays_its_bound_away_from_every_edge() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    let carriers = &b.arr.carriers;
    // The first ray passes thirty steps from the edge along z at x = 0, y = 0.
    let q = P3::new(-5 * S, 30, 5 * S).unwrap();
    let exact = b.boundary(0, 0).winding(carriers, q, &mut meter()).unwrap();
    assert_eq!((exact.number, exact.crossings, exact.ray), (0, 2, 0));
    // With faces that meet only within forty steps, that ray could slip through the seam.
    let loose = b
        .boundary(0, 40)
        .winding(carriers, q, &mut meter())
        .unwrap();
    assert_eq!(loose.number, 0);
    assert!(loose.ray > 0, "{loose:?}");
    // A bound of two steps leaves the ray room to spare, in space and in the face's picture.
    let tight = b.boundary(0, 2).winding(carriers, q, &mut meter()).unwrap();
    assert_eq!((tight.number, tight.ray), (0, 0));
    // Inside too: the same answers, whatever ray gives them.
    let inside = P3::new(5 * S, 30, 5 * S).unwrap();
    for bound in [0, 2, 5, 40] {
        let w = b
            .boundary(0, bound)
            .winding(carriers, inside, &mut meter())
            .unwrap();
        assert_eq!(w.number, 1, "bound {bound}: {w:?}");
    }
}

/// A whole sphere about `center`, as an operand of one face without loops.
fn ball(b: &mut Build, center: [i64; 3], radius: i64, operand: u32, outward: bool) -> usize {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let sphere = Sphere::new(
        center.map(|c| c as f64),
        radius as f64,
        frame,
        ((0.0, 2.0), (-0.5, 0.5)),
    )
    .unwrap();
    let surface = b
        .arr
        .carriers
        .add_surface(SurfaceKind::Sphere(sphere))
        .unwrap();
    b.face(CarrierIx::Surface(surface), Vec::new(), operand, outward)
}

#[test]
fn a_sphere_agrees_with_its_closed_form() {
    let mut b = Build::default();
    let r = 10 * S;
    ball(&mut b, [0, 0, 0], r, 0, true);
    let boundary = b.boundary(0, 0);
    let carriers = &b.arr.carriers;
    let (mut inside, mut outside) = (0, 0);
    for x in [-12 * S, -7 * S, -1, 3 * S, 9 * S, 13 * S] {
        for y in [-11 * S, -4 * S, 0, 6 * S] {
            for z in [-13 * S, -2 * S, 5 * S] {
                let q = P3::new(x, y, z).unwrap();
                let square: i128 = [x, y, z]
                    .iter()
                    .map(|&c| i128::from(c) * i128::from(c))
                    .sum();
                let w = boundary.winding(carriers, q, &mut meter()).unwrap();
                if square < i128::from(r) * i128::from(r) {
                    assert_eq!((w.number.abs(), w.crossings), (1, 1), "{q:?}");
                    inside += 1;
                } else {
                    assert_eq!(w.number, 0, "{q:?}: {w:?}");
                    assert!(w.crossings == 0 || w.crossings == 2);
                    outside += 1;
                }
            }
        }
    }
    assert!(inside > 10 && outside > 20, "{inside} {outside}");
    // The sign follows the side called outside.
    let mut flipped = Build::default();
    ball(&mut flipped, [0, 0, 0], r, 0, false);
    let centre = p(0, 0, 0);
    let one = boundary.winding(carriers, centre, &mut meter()).unwrap();
    let other = flipped
        .boundary(0, 0)
        .winding(&flipped.arr.carriers, centre, &mut meter())
        .unwrap();
    assert_eq!(one.number, -other.number);
    assert_eq!(one.number.abs(), 1);
}

#[test]
fn a_ray_that_grazes_a_sphere_is_dropped() {
    let mut b = Build::default();
    let r = 10 * S;
    ball(&mut b, [0, 0, 0], r, 0, true);
    let boundary = b.boundary(0, 0);
    // Along x at y = r: the first ray touches the sphere at (0, r, 0) and crosses nothing.
    let q = P3::new(-20 * S, r, 0).unwrap();
    let w = boundary.winding(&b.arr.carriers, q, &mut meter()).unwrap();
    assert_eq!(w.number, 0);
    assert!(w.ray > 0, "a tangent ray decided: {w:?}");
    // A step lower it crosses twice, a step higher it misses: both proven.
    let low = P3::new(-20 * S, r - 1, 0).unwrap();
    let w = boundary
        .winding(&b.arr.carriers, low, &mut meter())
        .unwrap();
    assert_eq!(w.number, 0);
    let high = P3::new(-20 * S, r + 1, 0).unwrap();
    let w = boundary
        .winding(&b.arr.carriers, high, &mut meter())
        .unwrap();
    assert_eq!((w.number, w.crossings, w.ray), (0, 0, 0));
    // The sphere's clear box: a point outside by more than the reach.
    assert!(
        boundary
            .clear(&b.arr.carriers, high, Bound::ZERO, &mut meter())
            .unwrap()
            || !boundary
                .clear(&b.arr.carriers, high, Bound(Steps(2)), &mut meter())
                .unwrap()
    );
    assert!(
        boundary
            .clear(
                &b.arr.carriers,
                P3::new(-20 * S, 0, 0).unwrap(),
                Bound(Steps(50)),
                &mut meter()
            )
            .unwrap()
    );
    assert!(
        !boundary
            .clear(
                &b.arr.carriers,
                P3::new(-r, 0, 0).unwrap(),
                Bound(Steps(1)),
                &mut meter()
            )
            .unwrap()
    );
}

/// A cylinder about the z axis from z = 0 to `height`: two flat caps bounded by circles, and a
/// side on a curved carrier with those circles as its loops.
fn cylinder(b: &mut Build, radius: i64, height: i64, operand: u32) {
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let mut rims = Vec::new();
    for z in [0, height] {
        let circle = Circle::new([0.0, 0.0, z as f64], radius as f64, frame, (0.0, 2.0)).unwrap();
        rims.push(b.edge(CurveKind::Circle(circle), None, operand));
        let plane = b.plane([0, 0, 1], -i128::from(z));
        b.face(plane, vec![vec![*rims.last().unwrap()]], operand, z > 0);
    }
    let side = Cylinder::new(
        [0.0, 0.0, 0.0],
        radius as f64,
        frame,
        ((0.0, 2.0), (0.0, height as f64)),
    )
    .unwrap();
    let surface = b
        .arr
        .carriers
        .add_surface(SurfaceKind::Cylinder(side))
        .unwrap();
    b.face(
        CarrierIx::Surface(surface),
        vec![vec![rims[0]], vec![rims[1]]],
        operand,
        true,
    );
}

#[test]
fn a_curved_face_with_loops_is_passed_only_by_rays_that_miss_its_carrier() {
    let mut b = Build::default();
    cylinder(&mut b, 5 * S, 20 * S, 0);
    let boundary = b.boundary(0, 0);
    let carriers = &b.arr.carriers;
    // Inside: every ray across the side is refused, and the one up the axis leaves by the cap.
    let w = boundary
        .winding(carriers, p(1, 1, 10), &mut meter())
        .unwrap();
    assert_eq!((w.number, w.crossings), (1, 1), "{w:?}");
    assert_eq!(RAYS[w.ray], [0, 0, 1]);
    // Outside, beside it: the first ray that misses the carrier decides.
    let w = boundary
        .winding(carriers, p(-20, 9, 10), &mut meter())
        .unwrap();
    assert_eq!((w.number, w.crossings, w.ray), (0, 0, 0));
    // Outside, facing it: the ray across the side is refused, and one that misses decides.
    let w = boundary
        .winding(carriers, p(-20, 0, 10), &mut meter())
        .unwrap();
    assert_eq!(w.number, 0);
    assert!(w.ray > 0, "{w:?}");
    // Below a cap: up the axis the ray crosses both caps.
    let w = boundary
        .winding(carriers, p(1, 1, -10), &mut meter())
        .unwrap();
    assert_eq!(w.number, 0, "{w:?}");
}

#[test]
fn two_boxes_that_overlap_give_each_group_one_answer() {
    let mut b = Build::default();
    let (alo, ahi) = ([0, 0, 0], [10 * S, 10 * S, 10 * S]);
    let (blo, bhi) = ([5 * S, 5 * S, 5 * S], [15 * S, 15 * S, 15 * S]);
    b.cuboid(alo, ahi, 0);
    b.cuboid(blo, bhi, 1);
    let (out, splitting) = b.run();
    let parts = groups(&b.arr, &out, &splitting);
    // Each box: the part outside the other (three whole faces and three L shapes) and the part
    // inside it (three squares).
    assert_eq!(parts.len(), 4, "{parts:?}");
    let mut sizes: Vec<usize> = parts.iter().map(Vec::len).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, [3, 3, 6, 6]);
    let found = b.classify(&[0, 1]).unwrap();
    assert_eq!(found.len(), 4);
    for group in &found {
        assert_eq!(group.against.len(), 1);
        let one = group.against[0];
        assert_eq!(one.operand, 1 - group.operand);
        // The answer is the exact classification of the point it was counted from.
        let (lo, hi) = if one.operand == 0 {
            (alo, ahi)
        } else {
            (blo, bhi)
        };
        let exact = in_box(one.at, lo, hi).expect("a point off the other box");
        assert_eq!(one.winding.number, i32::from(exact), "{group:?}");
        // Three faces inside the other box, six outside it.
        assert_eq!(group.faces.len(), if exact { 3 } else { 6 }, "{group:?}");
        assert!(group.faces.contains(&one.face));
    }
    // The same on every run.
    assert_eq!(b.classify(&[0, 1]).unwrap(), found);
}

#[test]
fn boxes_apart_are_each_one_group_outside_the_other() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    b.cuboid([20 * S, 0, 0], [30 * S, 10 * S, 10 * S], 1);
    let found = b.classify(&[0, 1]).unwrap();
    assert_eq!(found.len(), 2);
    for group in &found {
        assert_eq!(group.faces.len(), 6);
        assert_eq!(group.against[0].winding.number, 0);
    }
}

#[test]
fn a_box_inside_a_box_is_inside_and_its_holder_outside() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [30 * S, 30 * S, 30 * S], 0);
    b.cuboid([10 * S, 10 * S, 10 * S], [20 * S, 20 * S, 20 * S], 1);
    let found = b.classify(&[0, 1]).unwrap();
    assert_eq!(found.len(), 2);
    for group in &found {
        let expected = i32::from(group.operand == 1);
        assert_eq!(group.against[0].winding.number, expected, "{group:?}");
    }
}

#[test]
fn a_group_lying_in_the_other_operand_s_face_has_no_clear_box() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    // A box standing on the first one's top, larger than it: the top lies inside its base.
    b.cuboid([-5 * S, -5 * S, 10 * S], [15 * S, 15 * S, 20 * S], 1);
    match b.classify(&[0, 1]) {
        Err(BuildError::NoClearBox { operand, near, .. }) => {
            assert!(operand <= 1);
            assert!(near.enclosure.contains(0.0));
            assert_eq!(near.remedies().len(), 3);
        }
        other => panic!("a group in a face classified: {other:?}"),
    }
}

#[test]
fn an_operand_without_its_bound_is_missing() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    b.cuboid([20 * S, 0, 0], [30 * S, 10 * S, 10 * S], 1);
    assert!(matches!(b.classify(&[1]), Err(BuildError::Missing(_))));
    // And a face without its outward entry.
    let short = Boundary::new(&b.arr, 1, &b.outward[..7], Bound::ZERO, &mut meter());
    assert!(matches!(short, Err(BuildError::Missing(_))));
}

#[test]
fn a_stopped_meter_stops_the_count() {
    let mut b = Build::default();
    b.cuboid([0, 0, 0], [10 * S, 10 * S, 10 * S], 0);
    let boundary = b.boundary(0, 0);
    let mut spent = WorkMeter::new(Budget::of_work(0));
    let stopped = boundary.winding(&b.arr.carriers, p(5, 5, 5), &mut spent);
    assert!(
        matches!(stopped, Err(BuildError::Contact(_))),
        "{stopped:?}"
    );
}
