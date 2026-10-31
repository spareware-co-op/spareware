//! What the test files share: bodies built row by row through geom-topo's public builder, valid
//! ones of every kind and the shapes Euler's formula must accept, and helpers that run the checker
//! and read its faults. Nothing here calls anything but public interfaces.

#![allow(
    dead_code,
    reason = "each test file that shares this module uses some of it"
)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc,
    reason = "test builders; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_check::{Entity, Fault, FaultKind, Level, Report, check};
use geom_curve::{Circle, CurveKind, Frame, Line};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense, Shell, Vertex,
};

/// A meter with no cap and no cancellation.
pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The index of row `row`.
pub fn ix<K>(row: u32) -> Ix<K> {
    Ix::new(row).unwrap()
}

/// A plane along the axes; its exact placement does not matter to the checks here.
pub fn plane() -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// A sphere, a curved surface.
pub fn sphere() -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            1.0,
            Frame::standard().unwrap(),
            ((-1.0, 1.0), (-0.5, 0.5)),
        )
        .unwrap(),
    )
}

/// The segment from `a` to `b` (along x if they coincide).
pub fn line(a: [i64; 3], b: [i64; 3]) -> CurveKind<3> {
    let d = [0, 1, 2].map(|k| (b[k] - a[k]) as f64);
    let d = if d == [0.0; 3] { [1.0, 0.0, 0.0] } else { d };
    CurveKind::Line(Line::new(a.map(|x| x as f64), d, (0.0, 1.0)).unwrap())
}

/// A circle, a closed curve that is not a line.
pub fn circle() -> CurveKind<3> {
    CurveKind::Circle(Circle::new([0.0; 3], 1.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap())
}

/// A 2D line, for a pcurve.
pub fn pline() -> CurveKind<2> {
    CurveKind::Line(Line::new([0.0; 2], [1.0, 0.0], (0.0, 1.0)).unwrap())
}

/// A 2D circle, for a pcurve.
pub fn pcircle() -> CurveKind<2> {
    CurveKind::Circle(Circle::new([0.0; 2], 1.0, Frame::standard().unwrap(), (0.0, 2.0)).unwrap())
}

/// A body's rows, as the builder takes them, and its outside region.
#[derive(Clone)]
pub struct Rows {
    pub regions: Vec<Region>,
    pub shells: Vec<Shell>,
    pub faces: Vec<Face>,
    pub loops: Vec<Loop>,
    pub fins: Vec<Fin>,
    pub edges: Vec<Edge>,
    pub vertices: Vec<Vertex>,
    pub surfaces: Vec<SurfaceKind>,
    pub curves: Vec<CurveKind<3>>,
    pub pcurves: Vec<CurveKind<2>>,
    pub points: Vec<P3>,
    pub outside: u32,
}

impl Rows {
    /// Rows holding nothing but one void region, the outside.
    pub fn empty() -> Rows {
        Rows {
            regions: vec![Region {
                kind: RegionKind::Void,
                first_shell: None,
            }],
            shells: Vec::new(),
            faces: Vec::new(),
            loops: Vec::new(),
            fins: Vec::new(),
            edges: Vec::new(),
            vertices: Vec::new(),
            surfaces: Vec::new(),
            curves: Vec::new(),
            pcurves: Vec::new(),
            points: Vec::new(),
            outside: 0,
        }
    }

    /// The body these rows make, of `kind`.
    pub fn body(&self, kind: BodyKind) -> Body {
        let mut b = Builder::new();
        let a = &mut b.arenas;
        a.regions.extend(&self.regions).unwrap();
        a.shells.extend(&self.shells).unwrap();
        a.faces.extend(&self.faces).unwrap();
        a.loops.extend(&self.loops).unwrap();
        a.fins.extend(&self.fins).unwrap();
        a.edges.extend(&self.edges).unwrap();
        a.vertices.extend(&self.vertices).unwrap();
        a.surfaces.extend(self.surfaces.clone()).unwrap();
        a.curves.extend(self.curves.clone()).unwrap();
        a.pcurves.extend(self.pcurves.clone()).unwrap();
        a.points.extend(self.points.clone()).unwrap();
        b.publish(kind, ix(self.outside), &mut meter()).unwrap()
    }

    /// Adds a vertex at `p`; its row.
    pub fn vertex(&mut self, p: [i64; 3]) -> u32 {
        self.points.push(P3::new(p[0], p[1], p[2]).unwrap());
        let v = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            point: ix(self.points.len() as u32 - 1),
        });
        v
    }

    /// Adds a region of `kind` with no shells; its row.
    pub fn region(&mut self, kind: RegionKind) -> u32 {
        self.regions.push(Region {
            kind,
            first_shell: None,
        });
        self.regions.len() as u32 - 1
    }

    /// Adds an edge, without fins yet, from vertex `a` to `b` on a line, or a closed edge
    /// without vertices on a circle (`None`); its row.
    pub fn edge(&mut self, ends: Option<(u32, u32)>) -> u32 {
        let curve = match ends {
            Some((a, b)) => {
                let at = |v: u32| {
                    let p = self.points[self.vertices[v as usize].point.row() as usize];
                    [p.x.get(), p.y.get(), p.z.get()]
                };
                line(at(a), at(b))
            }
            None => circle(),
        };
        self.curves.push(curve);
        let e = self.edges.len() as u32;
        self.edges.push(Edge {
            first_fin: None,
            start: ends.map(|(a, _)| ix(a)),
            end: ends.map(|(_, b)| ix(b)),
            curve: ix(self.curves.len() as u32 - 1),
        });
        e
    }

    /// The plane a face with `loops` lies on, and the box of its vertices: the plane through the
    /// first three of its vertices that are not on one line, spanned by their differences, so
    /// every vertex of a flat polygon lies on it exactly; `plane()`, z = 0, for loops of closed
    /// edges without vertices (the circles all lie in z = 0).
    fn carrier(&self, loops: &[&[(u32, Sense)]]) -> (SurfaceKind, Box3) {
        let mut points: Vec<P3> = Vec::new();
        for ring in loops {
            for &(e, _) in *ring {
                let edge = self.edges[e as usize];
                for v in [edge.start, edge.end].into_iter().flatten() {
                    let p = self.points[self.vertices[v.row() as usize].point.row() as usize];
                    if !points.contains(&p) {
                        points.push(p);
                    }
                }
            }
        }
        let xyz = |p: P3| [p.x.get(), p.y.get(), p.z.get()];
        let corner = |pick: fn(i64, i64) -> i64| {
            let first = points.first().map_or([0; 3], |&p| xyz(p));
            let c = points
                .iter()
                .fold(first, |acc, &p| [0, 1, 2].map(|k| pick(acc[k], xyz(p)[k])));
            P3::new(c[0], c[1], c[2]).unwrap()
        };
        let bound = Box3::new(corner(i64::min), corner(i64::max)).unwrap();
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                for k in j + 1..points.len() {
                    let (o, a, b) = (xyz(points[i]), xyz(points[j]), xyz(points[k]));
                    let u = [a[0] - o[0], a[1] - o[1], a[2] - o[2]];
                    let v = [b[0] - o[0], b[1] - o[1], b[2] - o[2]];
                    let cross = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    if cross != [0; 3] {
                        let f = |w: [i64; 3]| w.map(|x| x as f64);
                        let surface = SurfaceKind::Plane(
                            Plane::new(f(o), f(u), f(v), ((0.0, 1.0), (0.0, 1.0))).unwrap(),
                        );
                        return (surface, bound);
                    }
                }
            }
        }
        (plane(), bound)
    }

    /// Adds a shell to region `r`, at the end of the region's list, with no faces yet; its row.
    pub fn shell(&mut self, r: u32) -> u32 {
        let s = self.shells.len() as u32;
        self.shells.push(Shell {
            region: ix(r),
            next: None,
            first_face: None,
        });
        match self.regions[r as usize].first_shell {
            None => self.regions[r as usize].first_shell = Some(ix(s)),
            Some(first) => {
                let mut at = first.row();
                while let Some(n) = self.shells[at as usize].next {
                    at = n.row();
                }
                self.shells[at as usize].next = Some(ix(s));
            }
        }
        s
    }

    /// Adds a face on a plane to shell `s`, at the end of its list, with one loop for each list of
    /// `(edge, sense)` in `loops`; its row. Fins join their edges' radial rings in the order
    /// added.
    pub fn face(&mut self, s: u32, loops: &[&[(u32, Sense)]]) -> u32 {
        let f = self.faces.len() as u32;
        let (surface, bound) = self.carrier(loops);
        self.surfaces.push(surface);
        self.faces.push(Face {
            shell: ix(s),
            next: None,
            surface: ix(self.surfaces.len() as u32 - 1),
            sense: Sense::Forward,
            first_loop: None,
            bound,
        });
        match self.shells[s as usize].first_face {
            None => self.shells[s as usize].first_face = Some(ix(f)),
            Some(first) => {
                let mut at = first.row();
                while let Some(n) = self.faces[at as usize].next {
                    at = n.row();
                }
                self.faces[at as usize].next = Some(ix(f));
            }
        }
        let mut previous: Option<u32> = None;
        for ring in loops {
            let l = self.loops.len() as u32;
            let first = self.fins.len() as u32;
            let n = ring.len() as u32;
            self.loops.push(Loop {
                face: ix(f),
                next: None,
                first_fin: Some(ix(first)),
            });
            match previous {
                None => self.faces[f as usize].first_loop = Some(ix(l)),
                Some(p) => self.loops[p as usize].next = Some(ix(l)),
            }
            previous = Some(l);
            for (k, &(e, sense)) in ring.iter().enumerate() {
                let fin = first + k as u32;
                self.fins.push(Fin {
                    loop_: ix(l),
                    next: ix(first + (k as u32 + 1) % n),
                    prev: ix(first + (k as u32 + n - 1) % n),
                    edge: ix(e),
                    sense,
                    radial: ix(fin),
                    pcurve: None,
                });
                // Join the edge's radial ring, after its last fin.
                match self.edges[e as usize].first_fin {
                    None => self.edges[e as usize].first_fin = Some(ix(fin)),
                    Some(head) => {
                        let mut at = head.row();
                        while self.fins[at as usize].radial.row() != head.row() {
                            at = self.fins[at as usize].radial.row();
                        }
                        self.fins[at as usize].radial = ix(fin);
                        self.fins[fin as usize].radial = head;
                    }
                }
            }
        }
        f
    }
}

/// Adds `faces` (lists of vertex numbers, counter-clockwise seen from outside) to shell `s`,
/// making each edge the first time a pair of vertices meets, from the lower vertex number to the
/// higher.
pub fn faces_into(r: &mut Rows, s: u32, faces: &[&[u32]]) {
    for ring in faces {
        let n = ring.len();
        let fins: Vec<(u32, Sense)> = (0..n)
            .map(|k| {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                let key = (a.min(b), a.max(b));
                let found = (0..r.edges.len()).find(|&e| {
                    let edge = r.edges[e];
                    edge.start.map(Ix::row) == Some(key.0) && edge.end.map(Ix::row) == Some(key.1)
                });
                let e = match found {
                    Some(e) => e as u32,
                    None => r.edge(Some(key)),
                };
                let sense = if a < b {
                    Sense::Forward
                } else {
                    Sense::Reversed
                };
                (e, sense)
            })
            .collect();
        r.face(s, &[&fins]);
    }
}

/// The rows of a solid polyhedron: its vertices at `points`, and its faces, each a list of vertex
/// numbers counter-clockwise seen from outside, in one shell of one solid region beside the
/// outside void.
pub fn polyhedron(points: &[[i64; 3]], faces: &[&[u32]]) -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    for &p in points {
        r.vertex(p);
    }
    let s = r.shell(solid);
    faces_into(&mut r, s, faces);
    r
}

/// A cube's corners: bit 0 is x, bit 1 y, bit 2 z, of side 10.
pub const CUBE: [[i64; 3]; 8] = [
    [0, 0, 0],
    [10, 0, 0],
    [0, 10, 0],
    [10, 10, 0],
    [0, 0, 10],
    [10, 0, 10],
    [0, 10, 10],
    [10, 10, 10],
];

/// The cube's faces, counter-clockwise from outside.
pub const CUBE_FACES: [&[u32]; 6] = [
    &[0, 2, 3, 1],
    &[4, 5, 7, 6],
    &[0, 1, 5, 4],
    &[2, 6, 7, 3],
    &[0, 4, 6, 2],
    &[1, 3, 7, 5],
];

/// A cube: 8 vertices, 12 edges, 6 faces, one closed shell.
pub fn cube() -> Rows {
    polyhedron(&CUBE, &CUBE_FACES)
}

/// A square sheet: one face with four boundary edges, its shell in the outside region.
pub fn square_sheet() -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let s = r.shell(0);
    faces_into(&mut r, s, &[&[0, 1, 2, 3]]);
    r
}

/// A disc sheet: one face bounded by one closed edge without vertices.
pub fn disc_sheet() -> Rows {
    let mut r = Rows::empty();
    let e = r.edge(None);
    let s = r.shell(0);
    r.face(s, &[&[(e, Sense::Forward)]]);
    r
}

/// An annulus sheet: a square face with a hole bounded by one closed edge without vertices.
pub fn annulus_sheet() -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 10, 0]] {
        r.vertex(p);
    }
    let sides: Vec<(u32, Sense)> = (0..4)
        .map(|k| (r.edge(Some((k, (k + 1) % 4))), Sense::Forward))
        .collect();
    // The last side runs from vertex 3 to 0, from the higher number: it is made that way here.
    let hole = r.edge(None);
    let s = r.shell(0);
    r.face(s, &[&sides, &[(hole, Sense::Reversed)]]);
    r
}

/// A solid cylinder: top, bottom and side faces, two closed edges without vertices; the side has
/// one loop on each.
pub fn cylinder() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (top, bottom) = (r.edge(None), r.edge(None));
    let s = r.shell(solid);
    r.face(s, &[&[(top, Sense::Forward)]]);
    r.face(s, &[&[(bottom, Sense::Forward)]]);
    r.face(
        s,
        &[&[(top, Sense::Reversed)], &[(bottom, Sense::Reversed)]],
    );
    r
}

/// A solid torus of genus one, as one face whose one loop runs round its two seams: two closed
/// edges at one vertex, each used both ways in the loop.
pub fn torus() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let v = r.vertex([0, 0, 0]);
    let (u, w) = (r.edge(Some((v, v))), r.edge(Some((v, v))));
    let s = r.shell(solid);
    r.face(
        s,
        &[&[
            (u, Sense::Forward),
            (w, Sense::Forward),
            (u, Sense::Reversed),
            (w, Sense::Reversed),
        ]],
    );
    r
}

/// A wire: three vertices and two edges with no fins.
pub fn wire() -> Rows {
    let mut r = Rows::empty();
    for p in [[0, 0, 0], [10, 0, 0], [10, 10, 0]] {
        r.vertex(p);
    }
    r.edge(Some((0, 1)));
    r.edge(Some((1, 2)));
    r
}

/// An acorn: one vertex.
pub fn acorn() -> Rows {
    let mut r = Rows::empty();
    r.vertex([0, 0, 0]);
    r
}

/// The plane z = `z`.
pub fn level(z: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((0.0, 1.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// The full circle about `center` of `radius` with `normal`, starting along `x`.
pub fn round(center: [f64; 3], radius: f64, x: [f64; 3], normal: [f64; 3]) -> CurveKind<3> {
    CurveKind::Circle(Circle::full(center, radius, Frame::new(x, normal).unwrap()).unwrap())
}

/// The full circle in the plane z = `z` about the z axis, of `radius`, starting on the x axis.
pub fn parallel(z: f64, radius: f64) -> CurveKind<3> {
    round([0.0, 0.0, z], radius, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
}

/// The cylinder of radius 5 about the z axis, for heights 0 to 10.
pub fn cylinder_surface() -> SurfaceKind {
    SurfaceKind::Cylinder(
        Cylinder::new(
            [0.0; 3],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 10.0)),
        )
        .unwrap(),
    )
}

/// A solid cylinder of radius 5 and height 10 about the z axis, as G84's `cylinder` joins it: caps
/// on the planes z = 10 and z = 0, a cylindrical side, two closed circles without vertices.
pub fn round_cylinder() -> Rows {
    let mut r = cylinder();
    r.curves[0] = parallel(10.0, 5.0);
    r.curves[1] = parallel(0.0, 5.0);
    r.surfaces[0] = level(10.0);
    r.surfaces[1] = level(0.0);
    r.surfaces[2] = cylinder_surface();
    r
}

/// The same cylinder with a seam: its circles start and end at vertices 0, (5, 0, 0), and 1,
/// (5, 0, 10), joined by a line on the side.
pub fn seamed_cylinder() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (low, high) = (r.vertex([5, 0, 0]), r.vertex([5, 0, 10]));
    let bottom = r.edge(Some((low, low)));
    let top = r.edge(Some((high, high)));
    let seam = r.edge(Some((low, high)));
    let s = r.shell(solid);
    r.face(s, &[&[(top, Sense::Forward)]]);
    r.face(s, &[&[(bottom, Sense::Forward)]]);
    r.face(
        s,
        &[&[
            (bottom, Sense::Reversed),
            (seam, Sense::Forward),
            (top, Sense::Reversed),
            (seam, Sense::Reversed),
        ]],
    );
    r.curves[bottom as usize] = parallel(0.0, 5.0);
    r.curves[top as usize] = parallel(10.0, 5.0);
    r.surfaces[0] = level(10.0);
    r.surfaces[1] = level(0.0);
    r.surfaces[2] = cylinder_surface();
    r
}

/// A solid cone about the z axis, radius 6 at z = 0 narrowing to its apex at z = 12: a base disc,
/// its base circle starting and ending at vertex 0, (6, 0, 0), and a seam line from the apex,
/// vertex 1, down to it.
pub fn cone() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let (base, apex) = (r.vertex([6, 0, 0]), r.vertex([0, 0, 12]));
    let rim = r.edge(Some((base, base)));
    let seam = r.edge(Some((apex, base)));
    let s = r.shell(solid);
    r.face(s, &[&[(rim, Sense::Forward)]]);
    r.face(
        s,
        &[&[
            (rim, Sense::Reversed),
            (seam, Sense::Reversed),
            (seam, Sense::Forward),
        ]],
    );
    r.curves[rim as usize] = parallel(0.0, 6.0);
    r.surfaces[0] = level(0.0);
    r.surfaces[1] = SurfaceKind::Cone(
        Cone::new(
            [0.0; 3],
            6.0,
            -0.5,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 12.0)),
        )
        .unwrap(),
    );
    r
}

/// The same cone without its apex vertex and seam: a base disc and a side joined by the rim, a
/// full circle without vertices.
pub fn bare_cone() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let rim = r.edge(None);
    let s = r.shell(solid);
    r.face(s, &[&[(rim, Sense::Forward)]]);
    r.face(s, &[&[(rim, Sense::Reversed)]]);
    r.curves[rim as usize] = parallel(0.0, 6.0);
    let side = cone().surfaces[1].clone();
    r.surfaces[0] = level(0.0);
    r.surfaces[1] = side;
    r
}

/// The sphere of radius 5 about the origin, its northern half.
pub fn dome_surface() -> SurfaceKind {
    SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 0.5)),
        )
        .unwrap(),
    )
}

/// A solid hemisphere of radius 5 on the plane z = 0: a base disc and a dome, joined by the
/// equator, which starts and ends at vertex 0, (5, 0, 0).
pub fn hemisphere() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let v = r.vertex([5, 0, 0]);
    let equator = r.edge(Some((v, v)));
    let s = r.shell(solid);
    r.face(s, &[&[(equator, Sense::Forward)]]);
    r.face(s, &[&[(equator, Sense::Reversed)]]);
    r.curves[equator as usize] = parallel(0.0, 5.0);
    r.surfaces[0] = level(0.0);
    r.surfaces[1] = dome_surface();
    r
}

/// A solid ball of radius 5: one spherical face with no loops.
pub fn ball() -> Rows {
    let mut r = Rows::empty();
    let solid = r.region(RegionKind::Solid);
    let s = r.shell(solid);
    r.face(s, &[]);
    r.surfaces[0] = SurfaceKind::Sphere(
        Sphere::new(
            [0.0; 3],
            5.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (-0.5, 0.5)),
        )
        .unwrap(),
    );
    r
}

/// The torus of radii 10 and 3 about the z axis.
pub fn torus_surface() -> SurfaceKind {
    SurfaceKind::Torus(
        Torus::new(
            [0.0; 3],
            10.0,
            3.0,
            Frame::standard().unwrap(),
            ((0.0, 2.0), (0.0, 2.0)),
        )
        .unwrap(),
    )
}

/// A solid ring torus of radii 10 and 3 about the z axis, as G84's `torus` joins it: its one
/// vertex at (13, 0, 0), a parallel seam (the outer equator, edge 0) and a meridian seam (in the
/// plane y = 0, edge 1) through it.
pub fn ring_torus() -> Rows {
    let mut r = torus();
    r.points[0] = P3::new(13, 0, 0).unwrap();
    r.curves[0] = parallel(0.0, 13.0);
    r.curves[1] = round([10.0, 0.0, 0.0], 3.0, [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]);
    r.surfaces[0] = torus_surface();
    r
}

/// A square sheet of side 10 with a round hole of radius 2 at its centre: G84's annulus with its
/// hole moved inside.
pub fn holed_sheet() -> Rows {
    let mut r = annulus_sheet();
    r.curves[4] = round([5.0, 5.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    r
}

/// Adds to shell `s` a face with one loop for each ring of vertex numbers in `rings`, an edge for
/// each step round a ring, from the vertex before to the vertex after; its row.
pub fn face_of(r: &mut Rows, s: u32, rings: &[&[u32]]) -> u32 {
    let mut loops: Vec<Vec<(u32, Sense)>> = Vec::new();
    for ring in rings {
        let mut fins = Vec::new();
        for k in 0..ring.len() {
            let e = r.edge(Some((ring[k], ring[(k + 1) % ring.len()])));
            fins.push((e, Sense::Forward));
        }
        loops.push(fins);
    }
    let refs: Vec<&[(u32, Sense)]> = loops.iter().map(Vec::as_slice).collect();
    r.face(s, &refs)
}

/// The report of a full check of `rows` as a body of `kind`, recorded `exactness`.
pub fn report(rows: &Rows, kind: BodyKind, exactness: Exactness) -> Report {
    check(&rows.body(kind), exactness, Level::Full, &mut meter()).unwrap()
}

/// The faults of a full check of `rows` as a body of `kind`, recorded certified (so group 4
/// accepts any carrier).
pub fn faults(rows: &Rows, kind: BodyKind) -> Vec<Fault> {
    report(rows, kind, certified()).faults().to_vec()
}

/// A certified record of one step of the building grid.
pub fn certified() -> Exactness {
    Exactness::Certified {
        bound: Steps(1),
        step: GridStep::Q0,
    }
}

/// Whether `faults` holds a fault of `kind` at `at`.
pub fn has(faults: &[Fault], kind: FaultKind, at: Entity) -> bool {
    faults.iter().any(|f| f.kind == kind && f.at == at)
}

/// The kinds of `faults`, in order.
pub fn kinds(faults: &[Fault]) -> Vec<FaultKind> {
    faults.iter().map(|f| f.kind).collect()
}
