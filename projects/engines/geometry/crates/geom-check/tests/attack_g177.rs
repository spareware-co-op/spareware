//! Attacks on piece G177's claims for group 2 on every kind: a face on a NURBS surface with
//! pcurves on its fins passes; without pcurves its edges are reported unchecked, never passed; a
//! vertex moved off such a face past the bound is caught, and one within it is not; a pcurve
//! moved off its edge is caught; a pcurve on another span than its edge is unchecked; and an edge
//! on a NURBS curve is held to it at both ends. For group 3: a cube of NURBS faces passes, each
//! pair meeting along its edge proven to meet only there, and a curved floor that stays clear of
//! the rest passes too, while one bulging through the top is reported, never passed.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

mod common;

use common::{Rows, cube, ix, report};
use geom_check::{Entity, Fault, FaultKind, Group};
use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_nurbs::{Basis, Curve, Surface};
use geom_surface::SurfaceKind;
use geom_topo::BodyKind;

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

/// Group 2's faults in a full check of the solid `rows`, recorded certified to `bound`.
fn at(rows: &Rows, bound: u64) -> Vec<Fault> {
    report(rows, BodyKind::Solid, certified(bound))
        .faults()
        .iter()
        .filter(|f| f.kind.group() == Group::Geometric)
        .copied()
        .collect()
}

/// S(u, v) = (u, v, 0) over [0, 10]², a bilinear NURBS patch on the cube's first face's plane.
fn floor_patch() -> SurfaceKind {
    let basis = || Basis::new(1, vec![0.0, 0.0, 10.0, 10.0]).unwrap();
    let net = [
        [0.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
    ];
    SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap())
}

/// Where vertex `v` of `r` lies.
fn place(r: &Rows, v: u32) -> [i64; 3] {
    let p = r.points[r.vertices[v as usize].point.row() as usize];
    [p.x.get(), p.y.get(), p.z.get()]
}

/// The fins of the cube's first face, the floor z = 0.
fn floor_fins(r: &Rows) -> Vec<usize> {
    (0..r.fins.len())
        .filter(|&k| r.loops[r.fins[k].loop_.row() as usize].face.row() == 0)
        .collect()
}

/// The cube with its floor on [`floor_patch`], and, if `pcurves`, each floor fin's pcurve the
/// line its edge runs along, read in the patch's (u, v) = (x, y), on the edge's own span.
fn nurbs_floor(pcurves: bool) -> Rows {
    let mut r = cube();
    r.surfaces[0] = floor_patch();
    if pcurves {
        for k in floor_fins(&r) {
            let e = r.fins[k].edge.row() as usize;
            let (a, b) = (
                place(&r, r.edges[e].start.unwrap().row()),
                place(&r, r.edges[e].end.unwrap().row()),
            );
            let d = [(b[0] - a[0]) as f64, (b[1] - a[1]) as f64];
            let pcurve =
                CurveKind::Line(Line::new([a[0] as f64, a[1] as f64], d, (0.0, 1.0)).unwrap());
            r.fins[k].pcurve = Some(ix(r.pcurves.len() as u32));
            r.pcurves.push(pcurve);
        }
    }
    r
}

/// The floor's edges, in fin order.
fn floor_edges(r: &Rows) -> Vec<u32> {
    floor_fins(r)
        .iter()
        .map(|&k| r.fins[k].edge.row())
        .collect()
}

#[test]
fn a_nurbs_face_with_pcurves_passes() {
    let r = nurbs_floor(true);
    assert_eq!(at(&r, 1), Vec::new());
}

#[test]
fn without_pcurves_its_edges_are_unchecked_never_passed() {
    let r = nurbs_floor(false);
    let faults = at(&r, 1);
    let want: Vec<Fault> = floor_edges(&r)
        .into_iter()
        .map(|e| Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Edge(ix(e))))
        .collect();
    assert_eq!(faults, want);
}

/// `r` with vertex `v` moved to `to`, its faces' stored boxes following.
fn moved(mut r: Rows, v: u32, to: [i64; 3]) -> Rows {
    let p = P3::new(to[0], to[1], to[2]).unwrap();
    let row = r.vertices[v as usize].point.row() as usize;
    r.points[row] = p;
    for face in &mut r.faces {
        face.bound = face.bound.union(&Box3::new(p, p).unwrap());
    }
    r
}

#[test]
fn a_vertex_off_a_nurbs_face_is_caught_past_the_bound() {
    let off = |faults: &[Fault]| {
        faults.contains(
            &Fault::at(FaultKind::VertexOffSurface, Entity::Vertex(ix(0)))
                .with(Entity::Face(ix(0))),
        )
    };
    // Corner 0 lifted 3 steps off the floor: off it with a bound of 2, on it with 3.
    let r = moved(nurbs_floor(true), 0, [0, 0, 3]);
    assert!(off(&at(&r, 2)), "{:?}", at(&r, 2));
    assert!(!off(&at(&r, 3)), "{:?}", at(&r, 3));
}

#[test]
fn a_pcurve_off_its_edge_is_caught() {
    let mut r = nurbs_floor(true);
    let k = floor_fins(&r)[0];
    let j = r.fins[k].pcurve.unwrap().row() as usize;
    // The pcurve moved 2 across its edge, into the patch: its image runs 2 steps from the edge.
    let CurveKind::Line(line) = &r.pcurves[j] else {
        panic!("a line")
    };
    let ([x, y], [dx, _]) = (line.origin(), line.direction());
    let inward = |c: f64| if c == 0.0 { 2.0 } else { -2.0 };
    let shifted = if dx == 0.0 {
        [x + inward(x), y]
    } else {
        [x, y + inward(y)]
    };
    r.pcurves[j] = CurveKind::Line(Line::new(shifted, line.direction(), (0.0, 1.0)).unwrap());
    let faults = at(&r, 1);
    assert!(
        faults.contains(
            &Fault::at(FaultKind::PcurveOffCurve, Entity::Pcurve(ix(j as u32)))
                .with(Entity::Fin(ix(k as u32)))
        ),
        "{faults:?}"
    );
    // The edge it was to prove on the face is then unchecked there, never passed.
    let e = r.fins[k].edge.row();
    assert!(
        faults.contains(
            &Fault::at(FaultKind::Unchecked, Entity::Surface(ix(0))).with(Entity::Edge(ix(e)))
        ),
        "{faults:?}"
    );
    // Within its bound it passes.
    assert_eq!(at(&r, 3), Vec::new());
}

#[test]
fn a_pcurve_on_another_span_is_unchecked() {
    let mut r = nurbs_floor(true);
    let k = floor_fins(&r)[1];
    let j = r.fins[k].pcurve.unwrap().row() as usize;
    let CurveKind::Line(line) = &r.pcurves[j] else {
        panic!("a line")
    };
    r.pcurves[j] = CurveKind::Line(Line::new(line.origin(), line.direction(), (0.0, 2.0)).unwrap());
    let faults = at(&r, 1);
    assert!(
        faults.contains(
            &Fault::at(FaultKind::Unchecked, Entity::Pcurve(ix(j as u32)))
                .with(Entity::Fin(ix(k as u32)))
        ),
        "{faults:?}"
    );
}

#[test]
fn an_edge_on_a_nurbs_curve_is_held_at_both_ends() {
    let mut r = cube();
    let e = 0;
    let (a, b) = (
        place(&r, r.edges[e].start.unwrap().row()),
        place(&r, r.edges[e].end.unwrap().row()),
    );
    let straight = |a: [i64; 3], b: [i64; 3]| {
        let basis = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let pts = [a.map(|x| x as f64), b.map(|x| x as f64)];
        CurveKind::Nurbs(Curve::new(basis, &pts).unwrap())
    };
    // On its ends: its place on its faces is unchecked (no pcurves), once.
    r.curves[e] = straight(a, b);
    assert_eq!(
        at(&r, 1),
        vec![Fault::at(FaultKind::Unchecked, Entity::Curve(ix(0))).with(Entity::Edge(ix(0)))]
    );
    // Its end moved 3 steps off its vertex: caught past a bound of 2.
    r.curves[e] = straight(a, [b[0], b[1], b[2] + 3]);
    let faults = at(&r, 2);
    let end = r.edges[e].end.unwrap().row();
    assert!(
        faults.contains(
            &Fault::at(FaultKind::VertexOffCurve, Entity::Vertex(ix(end)))
                .with(Entity::Edge(ix(0)))
        ),
        "{faults:?}"
    );
}

/// Group 3's faults in a full check of the solid `rows`, recorded certified to `bound`.
fn crossings(rows: &Rows, bound: u64) -> Vec<Fault> {
    report(rows, BodyKind::Solid, certified(bound))
        .faults()
        .iter()
        .filter(|f| f.kind.group() == Group::SelfIntersection)
        .copied()
        .collect()
}

/// The cube with every face on a NURBS patch over [0, 10]² of the two coordinates its plane
/// spans, the third held, of degree `degree` in each (control points spread evenly), and every
/// fin's pcurve its edge's line read in those coordinates; `lift` raises the floor's middle
/// control points by that much, so a curved floor keeps its edges.
fn nurbs_cube(degree: usize, lift: f64) -> Rows {
    let mut r = cube();
    for f in 0..r.faces.len() {
        let fins: Vec<usize> = (0..r.fins.len())
            .filter(|&k| r.loops[r.fins[k].loop_.row() as usize].face.row() == f as u32)
            .collect();
        let corners: Vec<[i64; 3]> = fins
            .iter()
            .map(|&k| {
                place(
                    &r,
                    r.edges[r.fins[k].edge.row() as usize].start.unwrap().row(),
                )
            })
            .collect();
        // The held coordinate: the one every corner shares.
        let held = (0..3)
            .find(|&c| corners.iter().all(|p| p[c] == corners[0][c]))
            .unwrap();
        let (i, j) = match held {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let value = corners[0][held] as f64;
        let mut knots = vec![0.0; degree + 1];
        knots.extend(vec![10.0; degree + 1]);
        let basis = || Basis::new(degree, knots.clone()).unwrap();
        let mut net = Vec::new();
        for a in 0..=degree {
            for b in 0..=degree {
                let (u, v) = (
                    10.0 * a as f64 / degree as f64,
                    10.0 * b as f64 / degree as f64,
                );
                let mut p = [0.0; 3];
                p[i] = u;
                p[j] = v;
                p[held] = value;
                let inner = a > 0 && a < degree && b > 0 && b < degree;
                if f == 0 && inner {
                    p[held] += lift;
                }
                net.push(p);
            }
        }
        r.surfaces[f] = SurfaceKind::Nurbs(Surface::new(basis(), basis(), &net).unwrap());
        for k in fins {
            let e = r.fins[k].edge.row() as usize;
            let (a, b) = (
                place(&r, r.edges[e].start.unwrap().row()),
                place(&r, r.edges[e].end.unwrap().row()),
            );
            let d = [(b[i] - a[i]) as f64, (b[j] - a[j]) as f64];
            let pcurve =
                CurveKind::Line(Line::new([a[i] as f64, a[j] as f64], d, (0.0, 1.0)).unwrap());
            r.fins[k].pcurve = Some(ix(r.pcurves.len() as u32));
            r.pcurves.push(pcurve);
        }
    }
    r
}

#[test]
fn a_nurbs_cube_passes_groups_2_and_3() {
    // Every face on a patch, every fin with its pcurve: each pair of faces meeting along an edge
    // is proven to meet only there, by the tube about the edge, and each face clean alone.
    let r = nurbs_cube(1, 0.0);
    assert_eq!(at(&r, 1), Vec::new());
    assert_eq!(crossings(&r, 1), Vec::new());
}

#[test]
fn a_floor_curved_below_the_top_passes() {
    // The floor's middle raised: a dome peaking 2 steps up, clear of every other face.
    let r = nurbs_cube(2, 8.0);
    assert_eq!(at(&r, 1), Vec::new());
    assert_eq!(crossings(&r, 1), Vec::new());
}

#[test]
fn a_floor_bulging_through_the_top_is_never_passed() {
    // Peaking 12 steps up, through the top at 10: the pair is reported, never passed. Its edges
    // stay on the walls (group 2 passes); what is reported is all the floor's.
    let r = nurbs_cube(2, 48.0);
    assert_eq!(at(&r, 1), Vec::new());
    let found = crossings(&r, 1);
    assert!(
        found.contains(
            &Fault::at(FaultKind::PairUnchecked, Entity::Face(ix(0))).with(Entity::Face(ix(1)))
        ),
        "{found:?}"
    );
    for f in &found {
        assert!(
            f.at == Entity::Face(ix(0)) || f.with == Some(Entity::Face(ix(0))),
            "{f:?}"
        );
    }
}
