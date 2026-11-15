//! Whole lines and circles meeting at a shared vertex at the smallest angles whole numbers allow
//! (G244, chapter 11.4 step 5): the bound behind the curvature step taking no touch test.
//!
//! A line through a grid point V of a whole circle (centre C, radius R) along a whole direction D
//! meets it at an angle with sin θ = |D · (V − C)| / (|D| R), at least 1 / (|D| R), and again a
//! chord of 2R sin θ = 2 |D · (V − C)| / |D| away. Two whole circles through one grid point meet
//! at an angle with sin θ at least about √2 / R: the product of their crossing's two whole factors
//! ((R₁ + R₂)² − d²)(d² − (R₁ − R₂)²) is at least 4R₁R₂ − 1, and a grid point on both makes the
//! smaller factor even. So two whole circles crossing at a grid vertex have tangents a double tells
//! apart there, at any radius the grid holds; a line can meet a circle at an angle no enclosure
//! tells, and then crosses it again within a fraction of a step, below the edge stage's
//! resolution, so the two crossings are one meeting at the vertex. At the grid's largest radii the
//! edge stage refuses such a meeting instead.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_bool::carrier::{CarrierIx, PointGeom};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, EdgeUse, Entity, FaceIn, Interfered, Interference, VertexIn,
    interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::split::{Piece, Splitting, split};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::Unresolved;
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 30))
}

#[derive(Default)]
struct Build {
    arr: Arrangement,
}

impl Build {
    fn vertex(&mut self, [x, y]: [i64; 2], operand: u32) -> usize {
        self.arr.vertices.push(VertexIn {
            point: PointGeom::Grid(P3::new(x, y, 0).unwrap()),
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

    fn at(&self, v: usize) -> [f64; 3] {
        match self.arr.vertices[v].point {
            PointGeom::Grid(q) => [q.x.get() as f64, q.y.get() as f64, 0.0],
            _ => panic!("a grid vertex"),
        }
    }

    fn flat(&mut self, loops: Vec<Vec<EdgeUse>>, operand: u32) -> usize {
        let plane = self
            .arr
            .carriers
            .add_plane(&Plane::new([0, 0, 1], 0).unwrap())
            .unwrap();
        self.arr.faces.push(FaceIn {
            surface: CarrierIx::Plane(plane),
            loops,
            operand,
        });
        self.arr.faces.len() - 1
    }

    /// A triangle on z = 0 with its corners counter-clockwise; its edges in order.
    fn triangle(&mut self, corners: [usize; 3], operand: u32) -> (usize, [usize; 3]) {
        let edges = [0, 1, 2].map(|k| {
            let (a, b) = (corners[k], corners[(k + 1) % 3]);
            let (o, e) = (self.at(a), self.at(b));
            let line = Line::new(o, [e[0] - o[0], e[1] - o[1], 0.0], (0.0, 1.0)).unwrap();
            self.edge(CurveKind::Line(line), Some([a, b]), operand)
        });
        let uses = edges
            .iter()
            .map(|&edge| EdgeUse {
                edge,
                forward: true,
            })
            .collect();
        (self.flat(vec![uses], operand), edges)
    }

    /// A disc on z = 0 about `c` of radius `r`: its face and its rim, closed with no vertex.
    fn disc(&mut self, c: [i64; 2], r: i64, operand: u32) -> (usize, usize) {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let circle =
            Circle::new([c[0] as f64, c[1] as f64, 0.0], r as f64, frame, (0.0, 2.0)).unwrap();
        let rim = self.edge(CurveKind::Circle(circle), None, operand);
        let face = self.flat(
            vec![vec![EdgeUse {
                edge: rim,
                forward: true,
            }]],
            operand,
        );
        (face, rim)
    }

    fn run(&self) -> Result<Interfered, BuildError> {
        interfere(
            &self.arr,
            default_remaining(),
            &Exec::serial(),
            &mut meter(),
        )
    }
}

/// a and b with a·x + b·y = gcd(a, b), and the gcd.
fn euclid(a: i128, b: i128) -> (i128, i128, i128) {
    let (mut r0, mut r1, mut x0, mut x1, mut y0, mut y1) = (a, b, 1, 0, 0, 1);
    while r1 != 0 {
        let q = r0 / r1;
        (r0, r1) = (r1, r0 - q * r1);
        (x0, x1) = (x1, x0 - q * x1);
        (y0, y1) = (y1, y0 - q * y1);
    }
    (r0, x0, y0)
}

/// The disc of radius R = m² + 4 about the origin (operand 0), and a triangle (operand 1) with a
/// corner V = (m² − 4, 4m) on the rim, a corner at the centre and a corner W = V + D, where
/// D · V = 1 and |D| is about `reach`: the side from V leaves along D, a hair outside the rim's
/// tangent, at sin θ = 1 / (|D| R), and the line through it crosses the rim again 2 / |D| behind
/// V, off the side. The triangle's third side, from W to the centre, crosses the rim at Q.
struct RimAndTriangle {
    build: Build,
    disc: usize,
    rim: usize,
    triangle: usize,
    /// The side from V along D.
    side: usize,
    v: usize,
}

fn rim_and_triangle(m: i64, reach: i64) -> RimAndTriangle {
    let r = m * m + 4;
    let (a, b) = (m * m - 4, 4 * m);
    let (g, x, y) = euclid(i128::from(a), i128::from(b));
    assert_eq!(g, 1);
    // D = (x, y) + t (−b, a): still D · V = 1, and as long as the grid allows.
    let t = i128::from(reach / r);
    let d = [x - t * i128::from(b), y + t * i128::from(a)];
    assert_eq!(d[0] * i128::from(a) + d[1] * i128::from(b), 1);
    let d = d.map(|c| i64::try_from(c).unwrap());
    let mut build = Build::default();
    let (disc, rim) = build.disc([0, 0], r, 0);
    let v = build.vertex([a, b], 1);
    let w = build.vertex([a + d[0], b + d[1]], 1);
    let c = build.vertex([0, 0], 1);
    // D turns clockwise from V's radius here, so V, W, C run counter-clockwise.
    assert!(i128::from(d[0]) * i128::from(b) - i128::from(d[1]) * i128::from(a) < 0);
    let (triangle, [side, ..]) = build.triangle([v, w, c], 1);
    RimAndTriangle {
        build,
        disc,
        rim,
        triangle,
        side,
        v,
    }
}

/// The split edges of `edge`, by index.
fn pieces_of(out: &Interfered, edge: usize) -> Vec<Piece> {
    (0..out.splits.len())
        .filter(|k| out.splits[*k].source == edge)
        .map(Piece::Split)
        .collect()
}

/// How often each split face of `f` runs `piece` forward and backward, face by face.
fn runs(splitting: &Splitting, f: usize, piece: Piece) -> Vec<(usize, usize)> {
    splitting
        .faces
        .iter()
        .filter(|s| s.face == f)
        .map(|s| {
            let count = |forward: bool| {
                s.outer
                    .iter()
                    .filter(|u| u.piece == piece && u.forward == forward)
                    .count()
            };
            (count(true), count(false))
        })
        .collect()
}

/// R = 2047² + 4, about 4.2e6, and |D| about 6.8e7: sin θ about 3.5e-15, below what the rim's
/// tangent enclosure tells at V, and the second crossing 2.9e-8 steps behind V. The two crossings
/// are one meeting at V: the side gets no vertex but its ends, and no vertex is made near V. Both
/// faces split in two, the disc along the triangle's sides inside it and the triangle along the
/// rim's arc inside it, each arc and side run once each way by the two halves.
#[test]
fn a_side_leaving_a_rim_vertex_at_the_least_angle_meets_the_rim_once_there() {
    let case = rim_and_triangle(2047, 1 << 26);
    let out = case.build.run().unwrap();
    // V, W, the centre, and Q where the third side crosses the rim: nothing else.
    assert_eq!(out.vertices.len(), 4, "{:?}", out.vertices);
    let v = out.merged[case.v];
    assert_eq!(
        out.paves[case.side]
            .iter()
            .map(|p| p.vertex)
            .collect::<Vec<_>>(),
        [v, out.merged[case.v + 1]]
    );
    assert!(out.paves[case.rim].iter().any(|p| p.vertex == v));
    assert!(
        out.interferences.iter().all(|i| match i {
            Interference::EdgeEdge {
                first,
                second,
                vertex,
                reused,
            } if [*first, *second] == [case.rim, case.side] => *vertex == v && *reused,
            _ => true,
        }),
        "{:?}",
        out.interferences
    );
    let splitting = split(&case.build.arr, &out, &mut meter()).unwrap();
    assert_eq!(splitting.waiting, []);
    for f in [case.disc, case.triangle] {
        let parts: Vec<_> = splitting.faces.iter().filter(|s| s.face == f).collect();
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert!(
            parts
                .iter()
                .all(|s| s.outer.len() == 3 && s.holes.is_empty())
        );
    }
    // The triangle's side from V bounds only the half outside the disc; the rim's arc inside the
    // triangle bounds both halves, once each way.
    let side = pieces_of(&out, case.side);
    assert_eq!(side.len(), 1);
    let mut on_side = runs(&splitting, case.triangle, side[0]);
    on_side.sort_unstable();
    assert_eq!(on_side, [(0, 0), (1, 0)]);
    let arcs = pieces_of(&out, case.rim);
    assert_eq!(arcs.len(), 2);
    let inside: Vec<Vec<(usize, usize)>> = arcs
        .iter()
        .map(|arc| runs(&splitting, case.triangle, *arc))
        .collect();
    assert!(
        inside
            .iter()
            .any(|r| r.contains(&(1, 0)) && r.contains(&(0, 1))),
        "{inside:?}"
    );
}

/// At R = 32767² + 4, about 1.07e9, and |D| about 3.4e8 (sin θ about 2.8e-18) the edge stage can no
/// longer settle the meeting of the side and the rim at its floor of a step, and refuses it: no
/// order is ever asked of the two pieces there.
#[test]
fn at_the_largest_radii_the_edge_stage_refuses_such_a_meeting() {
    let case = rim_and_triangle(32767, 1 << 26);
    match case.build.run() {
        Err(BuildError::Unresolved { first, second, .. }) => {
            assert_eq!(
                [first, second],
                [Entity::Edge(case.rim), Entity::Edge(case.side)]
            );
        }
        other => panic!("{other:?}"),
    }
}

/// Two circles through one grid point V: V − C₁ and its radius, V − C₂ and its radius, and V.
type Pair = ([i64; 2], i64, [i64; 2], i64, [i64; 2]);

/// Two whole circles through one grid vertex at the smallest angle whole numbers allow, sin θ
/// about √2 / R, with radii about 2^40 (one inside the other) or 2^39 (side by side, their second
/// crossing √2 steps away): the circles' closed form cannot hold the two crossings apart in
/// doubles and makes them one tangent band, which reaches past the contact radius of the vertex
/// they share, so the edge stage refuses it. The tangents at the vertex differ by far more than
/// their enclosures, so the curvature step is not asked either way.
#[test]
fn two_circles_through_one_vertex_at_the_least_angle_are_refused() {
    let max = (1_i64 << 40) - 1;
    // V − C₁ and V − C₂, each a whole vector of whole length, and where V may stand.
    let cases: [Pair; 2] = [
        (
            [1_099_507_252_851, 891_287_900],
            1_099_507_614_101,
            [1_099_358_119_024, 891_167_007],
            1_099_358_480_225,
            [max - 361_250 - 10, max - 1_098_616_326_201 - 10],
        ),
        (
            [549_746_868_179, 1_647_501_900],
            549_749_336_821,
            [-549_613_031_220, -1_647_100_811],
            549_615_499_261,
            [max - 1_099_228_530_481 - 10, 0],
        ),
    ];
    for (u, r1, w, r2, v) in cases {
        for (vector, r) in [(u, r1), (w, r2)] {
            let length = i128::from(vector[0]).pow(2) + i128::from(vector[1]).pow(2);
            assert_eq!(length, i128::from(r) * i128::from(r));
        }
        let cross = i128::from(u[0]) * i128::from(w[1]) - i128::from(u[1]) * i128::from(w[0]);
        let sin = cross.unsigned_abs() as f64 / (r1 as f64 * r2 as f64);
        assert!(sin < 3.0 / r2 as f64, "{sin}");
        let mut build = Build::default();
        build.disc([v[0] - u[0], v[1] - u[1]], r1, 0);
        build.disc([v[0] - w[0], v[1] - w[1]], r2, 1);
        build.vertex(v, 2);
        match build.run() {
            Err(BuildError::Unresolved {
                why: Unresolved::TangentBand { .. },
                ..
            }) => {}
            other => panic!("{other:?}"),
        }
    }
}
