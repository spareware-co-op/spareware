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
//! resolution, so the two crossings are one meeting at the vertex, at the grid's largest radii too.
//!
//! The side from V may run a hair outside the rim (D · (V − C) = 1) or a hair into the disc
//! (D · (V − C) = −1). Into the disc, its second crossing lies on the side itself, 2 / |D| from V,
//! inside the meeting the edge stage makes V, and its tangent at V points into the disc while past
//! that crossing, all the arrangement keeps, it runs outside (#900). The edge stage records the side
//! and the rim as touching at V, the face split orders them there by how they bend, and the
//! triangle splits at the rim as it does the other way; both splits are checked here piece by
//! piece and by area. With the record taken away the tangents' order contradicts the arrangement,
//! and the split refuses the triangle (`Uncovered`) rather than return no faces for it, as it did;
//! a piece said to lie inside a face that lies outside it is refused the same way. A seeded sweep
//! runs both ways over radii from 53 up and sides up to 2^26 or 64 R.

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

use core::f64::consts::PI;

use geom_bool::carrier::{CarrierIx, PointGeom};
use geom_bool::interfere::{
    Arrangement, BuildError, EdgeIn, EdgeUse, FaceIn, Interfered, Interference, Touch, VertexIn,
    interfere,
};
use geom_bool::merge::default_remaining;
use geom_bool::pave::Pave;
use geom_bool::split::{Piece, SplitFace, Splitting, Use, split};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_intersect::Unresolved;
use geom_num::P3;
use geom_num::plane::Plane;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

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

/// The least-angle case's m and |D|: R = 2047² + 4 and |D| about 6.8e7, or under Miri, where that
/// radius takes minutes, R = 255² + 4 and the same |D|. The side then leaves V at sin θ about
/// 2.3e-13 and crosses the rim again 3e-8 steps away, as before within the meeting the edge stage
/// makes V: the same touch is recorded and the faces split by how the side and the rim bend there,
/// so the same assertions hold.
fn least_angle() -> (i64, i64) {
    if cfg!(miri) {
        (255, 1 << 26)
    } else {
        (2047, 1 << 26)
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
/// corner V = (m² − 4, 4m) on the rim, a corner at the centre and a corner W = V ± D, where
/// D · V = 1 and |D| is about `reach`, or below R the least such: outward the side from V leaves
/// along D, a hair
/// outside the rim's tangent, at sin θ = 1 / (|D| R), and the line through it crosses the rim
/// again 2 / |D| behind V, off the side; inward it runs along −D, a hair into the disc, and
/// crosses the rim again 2 / |D| along the side. The triangle's third side, between W and the
/// centre, crosses the rim at Q.
struct RimAndTriangle {
    build: Build,
    disc: usize,
    rim: usize,
    triangle: usize,
    /// The side between V and W.
    side: usize,
    /// The side between the centre and V, a radius.
    radius: usize,
    v: usize,
    /// The radius and the corners V and W.
    r: i64,
    corners: [[i64; 2]; 2],
}

fn rim_and_triangle(m: i64, reach: i64, inward: bool) -> RimAndTriangle {
    let r = m * m + 4;
    let (a, b) = (m * m - 4, 4 * m);
    let (g, x, y) = euclid(i128::from(a), i128::from(b));
    assert_eq!(g, 1);
    // D = (x, y) + t (−b, a): still D · V = 1, and as long as the grid allows.
    let t = i128::from(reach / r);
    let d = [x - t * i128::from(b), y + t * i128::from(a)];
    assert_eq!(d[0] * i128::from(a) + d[1] * i128::from(b), 1);
    let d = d.map(|c| i64::try_from(c).unwrap());
    let e = if inward { d.map(|c| -c) } else { d };
    let mut build = Build::default();
    let (disc, rim) = build.disc([0, 0], r, 0);
    let v = build.vertex([a, b], 1);
    let w = build.vertex([a + e[0], b + e[1]], 1);
    let c = build.vertex([0, 0], 1);
    // D turns clockwise from V's radius here, so V, W, C run counter-clockwise outward, and V, C,
    // W inward.
    assert!(i128::from(d[0]) * i128::from(b) - i128::from(d[1]) * i128::from(a) < 0);
    let (triangle, side, radius) = if inward {
        let (triangle, [radius, _, side]) = build.triangle([v, c, w], 1);
        (triangle, side, radius)
    } else {
        let (triangle, [side, _, radius]) = build.triangle([v, w, c], 1);
        (triangle, side, radius)
    };
    RimAndTriangle {
        build,
        disc,
        rim,
        triangle,
        side,
        radius,
        v,
        r,
        corners: [[a, b], [a + e[0], b + e[1]]],
    }
}

/// The vertices a use runs between, in the order it runs them.
fn ends(out: &Interfered, one: &Use) -> [usize; 2] {
    let Piece::Split(k) = one.piece else {
        panic!("a section: {one:?}")
    };
    let block = out.splits[k].block;
    let (from, to) = (block.from.unwrap().vertex, block.to.unwrap().vertex);
    if one.forward { [from, to] } else { [to, from] }
}

/// ∫ x dy along a use, in doubles, with what it may be off by: a side between its points at the
/// middles of its paves' parameter enclosures, an arc of the rim about the origin in closed form.
/// Each end may lie anywhere its enclosure allows, which moves the integral by up to the curve's
/// speed times the enclosure's width times the coordinates' size, and doubles at coordinates of
/// that size lose about 2^-52 of its square a product.
fn swept(case: &RimAndTriangle, out: &Interfered, one: &Use) -> (f64, f64) {
    let Piece::Split(k) = one.piece else {
        panic!("a section: {one:?}")
    };
    let piece = &out.splits[k];
    let ends = [piece.block.from, piece.block.to].map(|pave: Option<Pave>| pave.unwrap().t);
    let [t0, t1] = ends.map(|t| t.lo() / 2.0 + t.hi() / 2.0);
    let width = ends[0].hi() - ends[0].lo() + ends[1].hi() - ends[1].lo();
    let arr = &case.build.arr;
    let (along, speed, reach) = match arr.carriers.curve(arr.edges[piece.source].curve).unwrap() {
        CurveKind::Line(line) => {
            let (o, d) = (line.origin(), line.direction());
            let at = |t: f64| [o[0] + t * d[0], o[1] + t * d[1]];
            let (p, q) = (at(t0), at(t1));
            let reach = [p[0], p[1], q[0], q[1]]
                .iter()
                .fold(0.0_f64, |most, c| most.max(c.abs()));
            let speed = d[0].abs() + d[1].abs();
            (f64::midpoint(p[0], q[0]) * (q[1] - p[1]), speed, reach)
        }
        CurveKind::Circle(circle) => {
            // x = R cos θ and y = R sin θ, θ = πt from (1, 0): ∫ x dy = R² (θ + sin θ cos θ) / 2.
            let r = circle.radius();
            let f = |t: f64| {
                let theta = PI * t;
                r * r / 2.0 * (theta + theta.sin() * theta.cos())
            };
            let t1 = if piece.block.wraps { t1 + 2.0 } else { t1 };
            (f(t1) - f(t0), PI * r, r)
        }
        other => panic!("{other:?}"),
    };
    let off = 2.0 * reach * speed * width + 1e-13 * reach * reach;
    (if one.forward { along } else { -along }, off)
}

/// Each use of `parts` counted by its piece and the way it runs it.
fn counted(parts: &[&SplitFace]) -> Vec<(Piece, bool, usize)> {
    let mut counts: Vec<(Piece, bool, usize)> = Vec::new();
    for one in parts.iter().flat_map(|s| &s.outer) {
        match counts
            .iter_mut()
            .find(|(p, f, _)| *p == one.piece && *f == one.forward)
        {
            Some((_, _, n)) => *n += 1,
            None => counts.push((one.piece, one.forward, 1)),
        }
    }
    counts.sort_unstable_by_key(|(p, f, _)| (*p, *f));
    counts
}

/// The splitting of `case` checked as a face split must be, for the disc and the triangle: each
/// in two faces with no holes, each face's loop running end to end, every piece of the face's own
/// edges once forward (each loop runs its edges forward, counter-clockwise) and every piece of the
/// other's once each way; and by area. The part of the triangle inside the disc is the sector
/// between the radius to V and the one to Q, of area R² θ / 2 with θ the angle from V to W seen
/// from the centre: the triangle's other part is the triangle less it, and the disc's the disc
/// less it.
fn checked(case: &RimAndTriangle, out: &Interfered, splitting: &Splitting) {
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
    let r = case.r as f64;
    let [[vx, vy], [wx, wy]] = case.corners.map(|p| p.map(|c| c as f64));
    let theta = (vx * wy - vy * wx).abs().atan2(vx * wx + vy * wy);
    let sector = r * r * theta / 2.0;
    let [[ax, ay], [bx, by]] = case.corners.map(|p| p.map(i128::from));
    let triangle = (ax * by - ay * bx).abs() as f64 / 2.0;
    let source = |piece: Piece| match piece {
        Piece::Split(k) => out.splits[k].source,
        Piece::Section(_) => panic!("a section"),
    };
    for (f, whole) in [(case.disc, PI * r * r), (case.triangle, triangle)] {
        let parts: Vec<_> = splitting.faces.iter().filter(|s| s.face == f).collect();
        assert_eq!(parts.len(), 2, "{parts:?}");
        for part in &parts {
            assert!(part.holes.is_empty() && part.cuts.is_empty() && part.inside.is_empty());
            let n = part.outer.len();
            for (k, one) in part.outer.iter().enumerate() {
                assert_eq!(ends(out, one)[1], ends(out, &part.outer[(k + 1) % n])[0]);
            }
        }
        let own = |e: usize| case.build.arr.edges[e].operand == case.build.arr.faces[f].operand;
        let mut inner = Vec::new();
        for (piece, forward, n) in counted(&parts) {
            assert_eq!(n, 1, "{piece:?} {forward}");
            if own(source(piece)) {
                assert!(forward, "{piece:?}");
            } else {
                inner.push((piece, forward));
            }
        }
        // Every piece of the face's own edges bounds it.
        for (k, piece) in out.splits.iter().enumerate() {
            if own(piece.source) {
                assert!(
                    parts
                        .iter()
                        .any(|s| s.outer.iter().any(|u| u.piece == Piece::Split(k))),
                    "{k}"
                );
            }
        }
        // The other's pieces inside: the rim's arc in the triangle, the radius and the third
        // side's piece in the disc, each run both ways.
        let wanted = if f == case.disc { 2 } else { 1 };
        assert_eq!(inner.len(), 2 * wanted, "{inner:?}");
        assert!(inner.chunks(2).all(|two| two[0].0 == two[1].0));
        let mut found = Vec::new();
        for part in &parts {
            let (mut area, mut room) = (0.0, 0.0);
            for one in &part.outer {
                let (along, off) = swept(case, out, one);
                area += along;
                room += off;
            }
            // The sector is the part the radius to V bounds in the triangle, and in the disc the
            // part that runs the triangle's arc forward.
            let holds_sector = part.outer.iter().any(|u| {
                if f == case.triangle {
                    source(u.piece) == case.radius
                } else {
                    u.forward
                        && source(u.piece) == case.rim
                        && in_triangle(splitting, case, u.piece)
                }
            });
            let want = if holds_sector { sector } else { whole - sector };
            // What the pieces' sum may be off by, and the closed form's own rounding.
            let room = room + 1e-12 * want.abs();
            assert!(
                (area - want).abs() <= room,
                "face {f}: {area} against {want} ({room})"
            );
            found.push(holds_sector);
        }
        found.sort_unstable();
        assert_eq!(found, [false, true]);
    }
}

/// Whether the triangle's split faces run `piece`.
fn in_triangle(splitting: &Splitting, case: &RimAndTriangle, piece: Piece) -> bool {
    splitting
        .faces
        .iter()
        .filter(|s| s.face == case.triangle)
        .any(|s| s.outer.iter().any(|u| u.piece == piece))
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
    let (m, reach) = least_angle();
    let case = rim_and_triangle(m, reach, false);
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
    assert!(splitting.waiting.is_empty(), "{:?}", splitting.waiting);
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
    // The meeting the edge stage made V is recorded, and the two are ordered there by how they
    // bend, which this way agrees with their tangents. The split holds as a split must.
    assert_eq!(
        out.touches,
        [Touch {
            vertex: v,
            edges: [case.rim, case.side]
        }]
    );
    checked(&case, &out, &splitting);
}

/// The same side run a hair into the disc, D · (V − C) = −1 (#900): its second crossing lies on
/// the side itself, 2.9e-8 steps from V, inside the meeting the edge stage makes V, which it
/// records as the side and the rim meeting there. Ordered by how they bend at V, the side outside
/// the rim, the triangle splits at the rim into the sector inside the disc and the rest, and the
/// disc into the sector and the rest, as the other way. Before the touch was handed on, the
/// tangents put the side inside the rim and the triangle split into no faces at all.
#[test]
fn a_side_run_a_hair_into_the_disc_splits_the_triangle_at_the_rim() {
    let case = rim_and_triangle(2047, 1 << 26, true);
    let out = case.build.run().unwrap();
    // V, W, the centre and Q, and the side's ends its only paves: nothing made near V.
    assert_eq!(out.vertices.len(), 4, "{:?}", out.vertices);
    let v = out.merged[case.v];
    assert_eq!(
        out.paves[case.side]
            .iter()
            .map(|p| p.vertex)
            .collect::<Vec<_>>(),
        [out.merged[case.v + 1], v]
    );
    assert_eq!(
        out.touches,
        [Touch {
            vertex: v,
            edges: [case.rim, case.side]
        }]
    );
    let splitting = split(&case.build.arr, &out, &mut meter()).unwrap();
    checked(&case, &out, &splitting);
}

/// The side a hair into the disc at R = 53 and |D| about 150, where the closed form holds its two
/// crossings of the rim apart, both proven, in one grid box at V, the second about 0.013 steps
/// from V (#1150). The contact rule makes both V. The crossing is V's pave on each edge, not a
/// second one beside it, which would cut each into a piece from V to V, and the two are recorded as
/// touching at V, as for a held region there: with the second pave the split refused the disc, and
/// with one pave and no touch it orders the two at V by their tangents and refuses the triangle.
#[test]
fn two_proven_crossings_made_one_vertex_are_one_pave_and_a_touch() {
    one_pave_and_a_touch(7, 159);
}

/// The same at R = 533 and |D| = 1,076,143, the seeded sweep's case at m = 23: the second crossing
/// about 1.9e-6 steps from V, held apart once the closed form takes the exact discriminant.
#[test]
fn the_sweep_s_side_into_a_disc_of_radius_533_is_one_pave_and_a_touch() {
    one_pave_and_a_touch(23, 1_076_143);
}

/// The inward case at `m` and `reach`: V, W, the centre and Q its only vertices, V one pave on the
/// side and on the rim, the touch at V, and the split as [`checked`] holds a split to.
fn one_pave_and_a_touch(m: i64, reach: i64) {
    let case = rim_and_triangle(m, reach, true);
    let out = case.build.run().unwrap();
    assert_eq!(out.vertices.len(), 4, "{:?}", out.vertices);
    let v = out.merged[case.v];
    assert_eq!(
        out.paves[case.side]
            .iter()
            .map(|p| p.vertex)
            .collect::<Vec<_>>(),
        [out.merged[case.v + 1], v]
    );
    let on_rim = out.paves[case.rim].iter().filter(|p| p.vertex == v).count();
    assert_eq!(on_rim, 1, "{:?}", out.paves[case.rim]);
    assert_eq!(
        out.touches,
        [Touch {
            vertex: v,
            edges: [case.rim, case.side]
        }]
    );
    let splitting = split(&case.build.arr, &out, &mut meter()).unwrap();
    checked(&case, &out, &splitting);
}

/// A rim that leaves its vertex V and comes back to it, a whole circle of radius 100 from V round
/// to V, and a chord of another operand along its diameter, through V and the far side U: V is on
/// the rim at both ends of its domain and keeps both paves there; the chord meets the rim at V,
/// where both pass, as one pave on each and no touch, the two crossing there; and its meeting at
/// U, proven apart from V, is a vertex of its own with a pave on each edge.
#[test]
fn a_rim_back_at_its_vertex_keeps_both_paves_and_a_meeting_apart_its_own() {
    let mut build = Build::default();
    let corner = build.vertex([100, 0], 0);
    let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let circle = Circle::new([0.0, 0.0, 0.0], 100.0, frame, (0.0, 2.0)).unwrap();
    let rim = build.edge(CurveKind::Circle(circle), Some([corner, corner]), 0);
    let ends = [build.vertex([300, 0], 1), build.vertex([-300, 0], 1)];
    let line = Line::new([300.0, 0.0, 0.0], [-600.0, 0.0, 0.0], (0.0, 1.0)).unwrap();
    let chord = build.edge(CurveKind::Line(line), Some(ends), 1);
    let out = build.run().unwrap();
    let v = out.merged[corner];
    // V, the chord's ends, and U, made last.
    assert_eq!(out.vertices.len(), 4, "{:?}", out.vertices);
    let u = 3;
    let at = |e: usize| out.paves[e].iter().map(|p| p.vertex).collect::<Vec<_>>();
    assert_eq!(at(rim), [v, u, v], "{:?}", out.paves[rim]);
    assert_eq!(
        at(chord),
        [out.merged[ends[0]], v, u, out.merged[ends[1]]],
        "{:?}",
        out.paves[chord]
    );
    assert!(out.touches.is_empty(), "{:?}", out.touches);
    let met: Vec<(usize, bool)> = out
        .interferences
        .iter()
        .filter_map(|i| match i {
            Interference::EdgeEdge { vertex, reused, .. } => Some((*vertex, *reused)),
            _ => None,
        })
        .collect();
    assert_eq!(met, [(v, true), (u, false)]);
}

/// A thin triangle of another operand with its corner V on a rim of radius 10^6 and its far side
/// AB crossing the rim half a step from V, in a grid box that holds V: the contact rule makes that
/// crossing V, the rim's pave of V stands for it, and AB, which did not pass through V, gets its
/// first pave of V. The two cross at V rather than touch, so no touch is recorded.
#[test]
fn a_side_across_the_rim_by_a_corner_it_does_not_pass_is_its_pave_there_and_no_touch() {
    let r = 1_000_000;
    let mut build = Build::default();
    let (_, rim) = build.disc([0, 0], r, 0);
    let corner = build.vertex([0, r], 1);
    let ends = [
        build.vertex([1, r - 1000], 1),
        build.vertex([0, r + 1000], 1),
    ];
    let (_, [_, far, _]) = build.triangle([corner, ends[0], ends[1]], 1);
    let out = build.run().unwrap();
    let v = out.merged[corner];
    assert_eq!(out.vertices.len(), 3, "{:?}", out.vertices);
    let at = |e: usize| out.paves[e].iter().map(|p| p.vertex).collect::<Vec<_>>();
    assert_eq!(at(rim), [v], "{:?}", out.paves[rim]);
    assert_eq!(
        at(far),
        [out.merged[ends[0]], v, out.merged[ends[1]]],
        "{:?}",
        out.paves[far]
    );
    assert!(out.touches.is_empty(), "{:?}", out.touches);
    assert!(out.interferences.iter().any(|i| matches!(
        i,
        Interference::EdgeEdge { first, second, vertex, reused: true }
            if (*first, *second, *vertex) == (rim, far, v)
    )));
}

/// The reproducer of #900 with the edge stage's record of the touch taken away: the side and the
/// rim are then ordered at V by their tangents, which put the side inside the rim while the
/// arrangement has it outside, and every loop of the triangle runs a stretch of its boundary with
/// the triangle on its right. The split refuses the triangle rather than return no faces for it.
#[test]
fn a_split_that_would_lose_the_triangle_is_refused() {
    let case = rim_and_triangle(2047, 1 << 26, true);
    let mut out = case.build.run().unwrap();
    out.touches.clear();
    let got = split(&case.build.arr, &out, &mut meter());
    assert!(
        matches!(got, Err(BuildError::Uncovered { face }) if face == case.triangle),
        "{got:?}"
    );
}

/// A piece said to lie inside a face that lies outside it: the third side's piece from Q to W,
/// outside the disc, planted among the disc's In pieces. The walk puts it in the loop round the
/// disc's outside, which it drops, and still finds the two parts the disc splits into; the piece
/// bounds neither, so the disc is refused rather than split as if it were not there.
#[test]
fn a_piece_inside_no_part_of_its_face_is_refused() {
    let (m, reach) = least_angle();
    let case = rim_and_triangle(m, reach, false);
    let mut out = case.build.run().unwrap();
    let third = case.build.arr.faces[case.triangle].loops[0]
        .iter()
        .map(|u| u.edge)
        .find(|e| ![case.side, case.radius].contains(e))
        .unwrap();
    let outside = (0..out.splits.len())
        .find(|k| out.splits[*k].source == third && !out.faces[case.disc].in_blocks.contains(k))
        .unwrap();
    out.faces[case.disc].in_blocks.push(outside);
    let got = split(&case.build.arr, &out, &mut meter());
    assert!(
        matches!(got, Err(BuildError::Uncovered { face }) if face == case.disc),
        "{got:?}"
    );
}

/// At R = 32767² + 4, about 1.07e9, and |D| about 3.4e8 (sin θ about 2.8e-18) the edge stage still
/// settles the meeting of the side and the rim at its floor of a step, both ways, and the faces
/// split as a split must. Run outside the disc it once refused it (`Unresolved`), while the
/// intersection split both parameters as shares of their domains, the rim's moving about ten times
/// as far a unit as the side's; it now splits the side that reaches farthest in space.
#[test]
fn at_the_largest_radii_the_edge_stage_settles_such_a_meeting() {
    for inward in [false, true] {
        let case = rim_and_triangle(32767, 1 << 26, inward);
        let out = case
            .build
            .run()
            .unwrap_or_else(|e| panic!("inward {inward}: {e:?}"));
        let splitting = split(&case.build.arr, &out, &mut meter()).unwrap();
        checked(&case, &out, &splitting);
    }
}

/// #904's first case: R = 53 and a side about as long as the radius, D = (−23, 37) and |D| about
/// 43.6, both ways. Into the disc the edge stage once refused it with a non-finite number in the
/// contact rule (`Contact(Num(NotFinite))`): the common-block test measured a piece of one block
/// from the other's whole curve, and where that curve's nearest point lay outside the block it
/// set the distance's upper end to infinity, which no enclosure holds. Each block is now measured
/// on its own stretch of its curve. Both ways the triangle, within the contact radius at this
/// size, is then refused where a point is placed in it (`Unclassified`), as it was outward.
#[test]
fn a_side_as_long_as_the_radius_is_refused_without_a_non_finite_number() {
    for inward in [false, true] {
        let case = rim_and_triangle(7, 53, inward);
        let got = case.build.run();
        assert!(
            matches!(got, Err(BuildError::Unclassified { face, .. }) if face == case.triangle),
            "inward {inward}: {got:?}"
        );
    }
}

/// A whole number from `lo` to `hi`, each power of two between them as likely as the next and
/// each value within one as likely as the next, so small and large values both come.
fn spread(rng: &mut SplitMix64, lo: i64, hi: i64) -> i64 {
    let power = rng
        .range_i64(i64::from(lo.ilog2()), i64::from(hi.ilog2()))
        .unwrap();
    rng.range_i64(lo.max(1 << power), hi.min((2 << power) - 1))
        .unwrap()
}

/// #904's second case: R = 53 and |D| = 2^32, a side four thousand million steps long run a hair
/// outside the small disc's rim and a hair into it. Into the disc the side and the rim cross at V
/// and again 2^-31 steps along the side, and their intersection flags the tangent band at V. Its
/// search once split both parameters as shares of their domains, while the side moves 2^32 steps
/// a unit of its parameter and the rim 166: the rim's was cut 2^24 times finer than the floor
/// needs, a million boxes and 1.6e7 units of work (100 seconds in a debug build). It now splits
/// the side that reaches farthest in space, and interfere fits in 2^16 units of work either way;
/// the faces split as a split must.
#[test]
fn a_long_side_by_a_small_disc_meets_its_rim_in_little_work() {
    for inward in [false, true] {
        let case = rim_and_triangle(7, 1 << 32, inward);
        let out = interfere(
            &case.build.arr,
            default_remaining(),
            &Exec::serial(),
            &mut WorkMeter::new(Budget::of_work(1 << 16)),
        )
        .unwrap_or_else(|e| panic!("inward {inward}: {e:?}"));
        let splitting = split(&case.build.arr, &out, &mut meter()).unwrap();
        checked(&case, &out, &splitting);
    }
}

/// The side from V a hair outside the rim and a hair into the disc, at radii R = m² + 4 from 53
/// (m = 7) to about 6.7e7 (m = 8191) and lengths |D| from 2^20, or R, to 2^26 or 64 R, both
/// seeded: each case splits as [`checked`] holds a split to. Up to these radii the edge stage
/// settles the meeting of the side and the rim, and at the grid's largest too
/// ([`at_the_largest_radii_the_edge_stage_settles_such_a_meeting`]). A longer side into a small
/// disc is split right too ([`a_long_side_by_a_small_disc_meets_its_rim_in_little_work`]), but at
/// m = 541 and |D| about 2^39 the edge stage refuses the side outside the rim
/// (`CoincidenceChain`), so the sweep stops short of such lengths.
#[test]
fn near_tangent_sides_split_at_the_rim_both_ways_at_seeded_sizes() {
    const CASES: usize = 32;
    // The largest m: R about 6.7e7.
    const MOST_M: i64 = 8191;
    let seed = resolve("near-tangent sides at a rim vertex", Seed::DEFAULT).unwrap();
    let mut rng = SplitMix64::new(seed);
    for _ in 0..CASES {
        let m = spread(&mut rng, 7, MOST_M) | 1;
        let r = m * m + 4;
        // W = V ± D stays inside the grid.
        let longest = (r << 6).max(1 << 26).min((1 << 40) - 1 - 2 * r);
        let reach = spread(&mut rng, r.max(1 << 20), longest);
        for inward in [false, true] {
            let case = rim_and_triangle(m, reach, inward);
            let out = case
                .build
                .run()
                .unwrap_or_else(|e| panic!("m {m}, |D| {reach}, inward {inward}: {e:?}"));
            let splitting = split(&case.build.arr, &out, &mut meter())
                .unwrap_or_else(|e| panic!("m {m}, |D| {reach}, inward {inward}: {e:?}"));
            checked(&case, &out, &splitting);
        }
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
