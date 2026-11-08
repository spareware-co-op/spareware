//! What the test files share: a meter, the wall body of chapter 23.9's memory measure, and a
//! body of many faces. None of it calls anything but the public interface.

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
    reason = "test builders, named as in the geometry; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, FaceIx, Fin, FinIx, Ix, Loop, Region, RegionIx,
    RegionKind, Sense, Shell, Vertex,
};

/// A meter with no cap and no cancellation.
pub fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The index of row `row`.
pub fn ix<K>(row: u32) -> Ix<K> {
    Ix::new(row).unwrap()
}

/// The wall's size in grid steps: 4 m long, 0.2 m thick and 2.7 m high at a millimetre grid.
pub const WALL: [i64; 3] = [4000, 200, 2700];

/// Corner `v` of the wall: bit 0 is x, bit 1 is y, bit 2 is z.
fn corner(v: usize) -> [i64; 3] {
    [0, 1, 2].map(|k| if v >> k & 1 == 1 { WALL[k] } else { 0 })
}

/// A plane through `origin` spanned by unit axes `i` and `j` of the wall's size.
pub fn plane(origin: [f64; 3], i: usize, j: usize) -> SurfaceKind {
    let mut a = [0.0; 3];
    let mut b = [0.0; 3];
    a[i] = 1.0;
    b[j] = 1.0;
    let domain = ((0.0, WALL[i] as f64), (0.0, WALL[j] as f64));
    SurfaceKind::Plane(Plane::new(origin, a, b, domain).unwrap())
}

/// The segment from `a` to `b`.
pub fn line(a: [i64; 3], b: [i64; 3]) -> CurveKind<3> {
    let d = [0, 1, 2].map(|k| (b[k] - a[k]) as f64);
    CurveKind::Line(Line::new(a.map(|x| x as f64), d, (0.0, 1.0)).unwrap())
}

/// The rows of a plain wall, a box: regions (the solid, then the outside), one shell, six planar
/// faces each with one loop of four fins, twelve line edges each used by two fins in a radial
/// ring, eight vertices; with its six planes, twelve lines and eight points.
pub struct WallRows {
    pub regions: Vec<Region>,
    pub shells: Vec<Shell>,
    pub faces: Vec<Face>,
    pub loops: Vec<Loop>,
    pub fins: Vec<Fin>,
    pub edges: Vec<Edge>,
    pub vertices: Vec<Vertex>,
    pub surfaces: Vec<SurfaceKind>,
    pub curves: Vec<CurveKind<3>>,
    pub points: Vec<P3>,
}

/// The wall's rows. Each face is the side of the box across axis k at side 0 or 1; its corners
/// run counter-clockwise seen from outside, and its plane's normal is +k, so the side-0 faces are
/// `Reversed`. Edges run from the lower corner number to the higher.
pub fn wall_rows() -> WallRows {
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
            origin[k] = (side as i64 * WALL[k]) as f64;
            surfaces.push(plane(origin, i, j));
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
                let e = edge_pairs
                    .iter()
                    .position(|&p| p == (a.min(b), a.max(b)))
                    .unwrap();
                let fin = 4 * f + n;
                fin_of_edge[e].push(fin);
                fins.push(Fin {
                    loop_: ix(f),
                    next: ix(4 * f + (n + 1) % 4),
                    prev: ix(4 * f + (n + 3) % 4),
                    edge: ix(e as u32),
                    sense: if a < b {
                        Sense::Forward
                    } else {
                        Sense::Reversed
                    },
                    radial: ix(fin),
                    pcurve: None,
                });
            }
        }
    }
    for pair in &fin_of_edge {
        assert_eq!(pair.len(), 2, "every edge of a box has two fins");
        fins[pair[0] as usize].radial = ix(pair[1]);
        fins[pair[1] as usize].radial = ix(pair[0]);
    }
    let edges = edge_pairs
        .iter()
        .enumerate()
        .map(|(e, &(a, b))| Edge {
            first_fin: Some(ix(fin_of_edge[e][0])),
            start: Some(ix(a as u32)),
            end: Some(ix(b as u32)),
            curve: ix(e as u32),
        })
        .collect();
    let curves = edge_pairs
        .iter()
        .map(|&(a, b)| line(corner(a), corner(b)))
        .collect();
    WallRows {
        regions: vec![
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(ix(0)),
            },
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
        ],
        shells: vec![Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        }],
        faces,
        loops,
        fins,
        edges,
        vertices: (0..8).map(|v| Vertex { point: ix(v) }).collect(),
        surfaces,
        curves,
        points: (0..8)
            .map(|v| {
                let c = corner(v);
                P3::new(c[0], c[1], c[2]).unwrap()
            })
            .collect(),
    }
}

/// The wall's outside region.
pub fn wall_outside() -> RegionIx {
    ix(1)
}

/// A builder holding the wall, filled row by row with `push` (`exact` false) or each arena at
/// once with `extend`, which reserves exactly (`exact` true).
pub fn wall_builder(exact: bool) -> Builder {
    let w = wall_rows();
    let mut b = Builder::new();
    let a = &mut b.arenas;
    if exact {
        a.regions.extend(&w.regions).unwrap();
        a.shells.extend(&w.shells).unwrap();
        a.faces.extend(&w.faces).unwrap();
        a.loops.extend(&w.loops).unwrap();
        a.fins.extend(&w.fins).unwrap();
        a.edges.extend(&w.edges).unwrap();
        a.vertices.extend(&w.vertices).unwrap();
        a.surfaces.extend(w.surfaces).unwrap();
        a.curves.extend(w.curves).unwrap();
        a.points
            .extend(w.points.into_iter().map(geom_topo::PointKind::Grid))
            .unwrap();
    } else {
        for r in w.regions {
            a.regions.push(r).unwrap();
        }
        for r in w.shells {
            a.shells.push(r).unwrap();
        }
        for r in w.faces {
            a.faces.push(r).unwrap();
        }
        for r in w.loops {
            a.loops.push(r).unwrap();
        }
        for r in w.fins {
            a.fins.push(r).unwrap();
        }
        for r in w.edges {
            a.edges.push(r).unwrap();
        }
        for r in w.vertices {
            a.vertices.push(r).unwrap();
        }
        for r in w.surfaces {
            a.surfaces.push(r).unwrap();
        }
        for r in w.curves {
            a.curves.push(r).unwrap();
        }
        for r in w.points {
            a.points.push(r.into()).unwrap();
        }
    }
    b
}

/// The wall, published.
pub fn wall() -> Body {
    wall_builder(true)
        .publish(BodyKind::Solid, wall_outside(), &mut meter())
        .unwrap()
}

/// A body of `n` faces on one shell of one solid region, each face on surface 0 with a box of its
/// own, a `next` chain through them all and no loops: arena-valid, as publishing checks, though
/// not a closed solid. Two surfaces, so an edit can move a face to the other.
pub fn many_faces(n: u32) -> Body {
    many_faces_builder(n)
        .publish(BodyKind::Sheet, ix(1), &mut meter())
        .unwrap()
}

/// [`many_faces`]' arenas, not yet published.
pub fn many_faces_builder(n: u32) -> Builder {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions
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
    a.shells
        .push(Shell {
            region: ix(0),
            next: None,
            first_face: Some(ix(0)),
        })
        .unwrap();
    a.surfaces
        .extend([plane([0.0; 3], 0, 1), plane([0.0, 0.0, 1.0], 0, 1)])
        .unwrap();
    let rows: Vec<Face> = (0..n)
        .map(|f| {
            let x = i64::from(f);
            Face {
                shell: ix(0),
                next: (f + 1 < n).then(|| ix(f + 1)),
                surface: ix(0),
                sense: Sense::Forward,
                first_loop: None,
                bound: Box3::new(P3::new(x, 0, 0).unwrap(), P3::new(x + 1, 1, 0).unwrap()).unwrap(),
            }
        })
        .collect();
    a.faces.extend(&rows).unwrap();
    b
}

/// The face at `row`.
pub fn face(row: u32) -> FaceIx {
    ix(row)
}

/// The fin at `row`.
pub fn fin(row: u32) -> FinIx {
    ix(row)
}
