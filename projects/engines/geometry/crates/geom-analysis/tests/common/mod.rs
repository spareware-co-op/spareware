//! What the test files share: a meter, the flat strip and exact Bezier graphs the constructed joins
//! are made of, the pcurve along a parameter line, and a unit box whose fins carry pcurves.

#![allow(
    dead_code,
    reason = "each test file that shares this module uses some of it"
)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    clippy::cast_lossless,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::disallowed_methods,
    reason = "test builders, named as in the geometry; the rules are for kernel code"
)]

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_nurbs::{Basis, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
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

/// The flat strip z = `lift` over x in [−1, 0], y in [0, 1], its edge x = 0 at u = 0.
pub fn strip(lift: f64) -> SurfaceKind {
    SurfaceKind::Plane(
        Plane::new(
            [0.0, 0.0, lift],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            ((-1.0, 0.0), (0.0, 1.0)),
        )
        .unwrap(),
    )
}

/// C(n, k) as a double.
fn choose(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

/// The Bernstein coefficients, in degree at least 1, of the polynomial with power coefficients
/// `c`.
fn bernstein(c: &[f64]) -> Vec<f64> {
    let mut c = c.to_vec();
    if c.len() < 2 {
        c.push(0.0);
    }
    let p = c.len() - 1;
    (0..=p)
        .map(|i| {
            let mut s = 0.0;
            for k in 0..=i {
                s += c[k] * choose(i, k) / choose(p, k);
            }
            s
        })
        .collect()
}

/// A clamped Bezier basis of the degree that `n` coefficients make, on [0, 1].
fn bezier(n: usize) -> Basis {
    let p = n - 1;
    let mut knots = vec![0.0; p + 1];
    knots.extend(vec![1.0; p + 1]);
    Basis::new(p, knots).unwrap()
}

/// The graph z = f(x)·g(y) over [0, 1]², an exact Bezier patch with u = x and v = y: f with power
/// coefficients `f`, g with Bernstein coefficients `g` (at least two).
pub fn graph(f: &[f64], g: &[f64]) -> SurfaceKind {
    let fb = bernstein(f);
    let (p, q) = (fb.len() - 1, g.len() - 1);
    let mut pts = Vec::new();
    for (i, fi) in fb.iter().enumerate() {
        for (j, gj) in g.iter().enumerate() {
            pts.push([i as f64 / p as f64, j as f64 / q as f64, fi * gj]);
        }
    }
    SurfaceKind::Nurbs(Surface::new(bezier(p + 1), bezier(q + 1), &pts).unwrap())
}

/// The pcurve u = `u`, v = t over t in [0, 1].
pub fn along(u: f64) -> CurveKind<2> {
    CurveKind::Line(Line::new([u, 0.0], [0.0, 1.0], (0.0, 1.0)).unwrap())
}

/// Corner `v` of the unit box: bit 0 is x, bit 1 is y, bit 2 is z.
fn corner(v: usize) -> [i64; 3] {
    [0, 1, 2].map(|k| i64::from(v >> k & 1 == 1))
}

/// The rows of a unit box: regions (the solid, then the outside), one shell, six planar faces
/// each with one loop of four fins (corners counter-clockwise from outside, the side-0 faces
/// `Reversed` since each plane's normal is +k), twelve line edges from the lower corner number
/// to the higher, each used by two fins, eight vertices; with `pcurves`, each fin carries the
/// line of its edge in its face's parameters, on the edge's parameter.
pub fn box_builder(pcurves: bool) -> Builder {
    let mut edge_pairs: Vec<(usize, usize)> = Vec::new();
    for v in 0..8usize {
        for k in 0..3 {
            if v >> k & 1 == 0 {
                edge_pairs.push((v, v | 1 << k));
            }
        }
    }
    edge_pairs.sort_unstable();
    let mut faces = Vec::new();
    let mut loops = Vec::new();
    let mut fins: Vec<Fin> = Vec::new();
    let mut surfaces = Vec::new();
    let mut pcurve_rows: Vec<CurveKind<2>> = Vec::new();
    let mut fin_of_edge: Vec<Vec<u32>> = vec![Vec::new(); edge_pairs.len()];
    for k in 0..3usize {
        let (i, j) = ((k + 1) % 3, (k + 2) % 3);
        for side in 0..2usize {
            let f = faces.len() as u32;
            let at = |a: usize, b: usize| (side << k) | (a << i) | (b << j);
            let mut ring = [at(0, 0), at(1, 0), at(1, 1), at(0, 1)];
            if side == 0 {
                ring.reverse();
            }
            let mut origin = [0.0; 3];
            origin[k] = side as f64;
            let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
            a[i] = 1.0;
            b[j] = 1.0;
            surfaces.push(SurfaceKind::Plane(
                Plane::new(origin, a, b, ((0.0, 1.0), (0.0, 1.0))).unwrap(),
            ));
            let lo = corner(at(0, 0));
            let hi = corner(at(1, 1));
            faces.push(Face {
                shell: ix(0),
                next: (f < 5).then(|| ix(f + 1)),
                surface: ix(f),
                sense: if side == 1 {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
                first_loop: Some(ix(f)),
                bound: Box3::new(
                    P3::new(lo[0], lo[1], lo[2]).unwrap(),
                    P3::new(hi[0], hi[1], hi[2]).unwrap(),
                )
                .unwrap(),
            });
            loops.push(Loop {
                face: ix(f),
                next: None,
                first_fin: Some(ix(4 * f)),
            });
            for n in 0..4u32 {
                let (a, b) = (ring[n as usize], ring[(n as usize + 1) % 4]);
                let (s, e) = (a.min(b), a.max(b));
                let edge = edge_pairs.iter().position(|&p| p == (s, e)).unwrap();
                let fin = 4 * f + n;
                fin_of_edge[edge].push(fin);
                let (cs, ce) = (corner(s), corner(e));
                let start = [cs[i] as f64, cs[j] as f64];
                let dir = [(ce[i] - cs[i]) as f64, (ce[j] - cs[j]) as f64];
                pcurve_rows.push(CurveKind::Line(Line::new(start, dir, (0.0, 1.0)).unwrap()));
                fins.push(Fin {
                    loop_: ix(f),
                    next: ix(4 * f + (n + 1) % 4),
                    prev: ix(4 * f + (n + 3) % 4),
                    edge: ix(edge as u32),
                    sense: if a < b {
                        Sense::Forward
                    } else {
                        Sense::Reversed
                    },
                    radial: ix(fin),
                    pcurve: pcurves.then(|| ix(fin)),
                });
            }
        }
    }
    for pair in &fin_of_edge {
        fins[pair[0] as usize].radial = ix(pair[1]);
        fins[pair[1] as usize].radial = ix(pair[0]);
    }
    let edges: Vec<Edge> = edge_pairs
        .iter()
        .enumerate()
        .map(|(e, &(a, b))| Edge {
            first_fin: Some(ix(fin_of_edge[e][0])),
            start: Some(ix(a as u32)),
            end: Some(ix(b as u32)),
            curve: ix(e as u32),
        })
        .collect();
    let curves: Vec<CurveKind<3>> = edge_pairs
        .iter()
        .map(|&(a, b)| {
            let (ca, cb) = (corner(a), corner(b));
            let d = [0, 1, 2].map(|k| (cb[k] - ca[k]) as f64);
            CurveKind::Line(Line::new(ca.map(|x| x as f64), d, (0.0, 1.0)).unwrap())
        })
        .collect();
    let mut builder = Builder::new();
    let ar = &mut builder.arenas;
    ar.regions
        .extend(&[
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ])
        .unwrap();
    ar.shells
        .extend(&[Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        }])
        .unwrap();
    ar.faces.extend(&faces).unwrap();
    ar.loops.extend(&loops).unwrap();
    ar.fins.extend(&fins).unwrap();
    ar.edges.extend(&edges).unwrap();
    let vertices: Vec<Vertex> = (0..8).map(|v| Vertex { point: ix(v) }).collect();
    ar.vertices.extend(&vertices).unwrap();
    ar.surfaces.extend(surfaces).unwrap();
    ar.curves.extend(curves).unwrap();
    if pcurves {
        ar.pcurves.extend(pcurve_rows).unwrap();
    }
    ar.points
        .extend(
            (0..8)
                .map(|v| {
                    let c = corner(v);
                    P3::new(c[0], c[1], c[2]).unwrap()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    builder
}

/// The unit box, published, with or without pcurves on its fins.
pub fn unit_box(pcurves: bool) -> Body {
    box_builder(pcurves)
        .publish(BodyKind::Solid, ix(1), &mut meter())
        .unwrap()
}

/// The unit box with pcurves, but edge 0's two fins each alone in its radial ring.
pub fn lone_fin() -> Body {
    let mut b = box_builder(true);
    let first = b.arenas.edges.get(ix(0)).unwrap().first_fin.unwrap();
    let second = b.arenas.fins.get(first).unwrap().radial;
    for fin in [first, second] {
        b.arenas.fins.radial.set(fin, fin).unwrap();
    }
    b.publish(BodyKind::Solid, ix(1), &mut meter()).unwrap()
}
