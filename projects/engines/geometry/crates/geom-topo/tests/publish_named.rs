//! Publishing in name order through the public interface (8.2, 32.17): whatever order a body's
//! rows were built in, the body published is the same; its indices follow names and the module's
//! fixed rule; every link still joins the same entities; and each fault is refused in order.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod common;

use common::{ix, meter, wall_outside, wall_rows};
use geom_curve::{CurveKind, Line};
use geom_num::grid::P3;
use geom_run::{Budget, BudgetKind, Stop, WorkMeter};
use geom_surface::SurfaceKind;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Arena, Arenas, Body, BodyKind, Builder, Edge, Face, Fin, Ix, Link, Loop, Region, RegionKind,
    Sense, Shell, TopoError, Vertex,
};

/// Every row of a body, as plain vectors a test can reorder.
#[derive(Clone)]
struct Rows {
    regions: Vec<Region>,
    shells: Vec<Shell>,
    faces: Vec<Face>,
    loops: Vec<Loop>,
    fins: Vec<Fin>,
    edges: Vec<Edge>,
    vertices: Vec<Vertex>,
    surfaces: Vec<SurfaceKind>,
    curves: Vec<CurveKind<3>>,
    pcurves: Vec<CurveKind<2>>,
    points: Vec<P3>,
}

/// A 2D segment, as a pcurve.
fn pcurve(n: u32) -> CurveKind<2> {
    CurveKind::Line(Line::new([f64::from(n), 0.0], [1.0, 1.0], (0.0, 1.0)).unwrap())
}

/// The wall of `common`, with a pcurve on each fin of its first face (fin `n` on pcurve `3 − n`).
fn wall() -> Rows {
    let w = wall_rows();
    let mut fins = w.fins;
    for n in 0..4u32 {
        fins[n as usize].pcurve = Some(ix(3 - n));
    }
    Rows {
        regions: w.regions,
        shells: w.shells,
        faces: w.faces,
        loops: w.loops,
        fins,
        edges: w.edges,
        vertices: w.vertices,
        surfaces: w.surfaces,
        curves: w.curves,
        pcurves: (0..4).map(pcurve).collect(),
        points: w.points,
    }
}

/// A builder holding `r`.
fn builder(r: &Rows) -> Builder {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions.extend(&r.regions).unwrap();
    a.shells.extend(&r.shells).unwrap();
    a.faces.extend(&r.faces).unwrap();
    a.loops.extend(&r.loops).unwrap();
    a.fins.extend(&r.fins).unwrap();
    a.edges.extend(&r.edges).unwrap();
    a.vertices.extend(&r.vertices).unwrap();
    a.surfaces.extend(r.surfaces.clone()).unwrap();
    a.curves.extend(r.curves.clone()).unwrap();
    a.pcurves.extend(r.pcurves.clone()).unwrap();
    a.points.extend(r.points.clone()).unwrap();
    b
}

/// A 64-bit mix (`SplitMix64`'s finaliser), so names fall in no order related to rows.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Name `i` of the arena tagged `tag`, under `salt`.
fn name(tag: u64, i: usize, salt: u64) -> NameKey {
    let a = mix(salt ^ (tag << 40) ^ i as u64);
    let b = mix(a);
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&a.to_be_bytes());
    k[8..].copy_from_slice(&b.to_be_bytes());
    NameKey(k)
}

/// One name per region, face, edge and vertex of `r`.
fn names(r: &Rows, salt: u64) -> Names {
    Names {
        regions: (0..r.regions.len()).map(|i| name(1, i, salt)).collect(),
        faces: (0..r.faces.len()).map(|i| name(2, i, salt)).collect(),
        edges: (0..r.edges.len()).map(|i| name(3, i, salt)).collect(),
        vertices: (0..r.vertices.len()).map(|i| name(4, i, salt)).collect(),
    }
}

/// A random order of `n` rows: where each row moves.
fn order(n: usize, s: &mut u64) -> Vec<u32> {
    let mut p: Vec<u32> = (0..n as u32).collect();
    for i in (1..n).rev() {
        *s = mix(*s);
        p.swap(i, (*s % (i as u64 + 1)) as usize);
    }
    p
}

/// `rows` with row `old` moved to `p[old]`.
fn place<T: Clone>(rows: &[T], p: &[u32]) -> Vec<T> {
    let mut out = rows.to_vec();
    for (old, &new) in p.iter().enumerate() {
        out[new as usize] = rows[old].clone();
    }
    out
}

/// `ix` moved by `p`.
fn m<K>(p: &[u32], ix: Ix<K>) -> Ix<K> {
    Ix::new(p[ix.row() as usize]).unwrap()
}

/// `ix`, if any, moved by `p`.
fn mo<K>(p: &[u32], ix: Option<Ix<K>>) -> Option<Ix<K>> {
    ix.map(|i| m(p, i))
}

/// `r` and `n` with every arena in a random order from `seed` and every link moved to match.
fn shuffle(r: &Rows, n: &Names, seed: u64) -> (Rows, Names) {
    let mut s = seed;
    let pr = order(r.regions.len(), &mut s);
    let ps = order(r.shells.len(), &mut s);
    let pf = order(r.faces.len(), &mut s);
    let pl = order(r.loops.len(), &mut s);
    let pi = order(r.fins.len(), &mut s);
    let pe = order(r.edges.len(), &mut s);
    let pv = order(r.vertices.len(), &mut s);
    let psu = order(r.surfaces.len(), &mut s);
    let pc = order(r.curves.len(), &mut s);
    let pp = order(r.pcurves.len(), &mut s);
    let pt = order(r.points.len(), &mut s);
    let regions: Vec<Region> = r
        .regions
        .iter()
        .map(|x| Region {
            first_shell: mo(&ps, x.first_shell),
            ..*x
        })
        .collect();
    let shells: Vec<Shell> = r
        .shells
        .iter()
        .map(|x| Shell {
            region: m(&pr, x.region),
            next: mo(&ps, x.next),
            first_face: mo(&pf, x.first_face),
        })
        .collect();
    let faces: Vec<Face> = r
        .faces
        .iter()
        .map(|x| Face {
            shell: m(&ps, x.shell),
            next: mo(&pf, x.next),
            surface: m(&psu, x.surface),
            first_loop: mo(&pl, x.first_loop),
            ..*x
        })
        .collect();
    let loops: Vec<Loop> = r
        .loops
        .iter()
        .map(|x| Loop {
            face: m(&pf, x.face),
            next: mo(&pl, x.next),
            first_fin: mo(&pi, x.first_fin),
        })
        .collect();
    let fins: Vec<Fin> = r
        .fins
        .iter()
        .map(|x| Fin {
            loop_: m(&pl, x.loop_),
            next: m(&pi, x.next),
            prev: m(&pi, x.prev),
            edge: m(&pe, x.edge),
            radial: m(&pi, x.radial),
            pcurve: mo(&pp, x.pcurve),
            ..*x
        })
        .collect();
    let edges: Vec<Edge> = r
        .edges
        .iter()
        .map(|x| Edge {
            first_fin: mo(&pi, x.first_fin),
            start: mo(&pv, x.start),
            end: mo(&pv, x.end),
            curve: m(&pc, x.curve),
        })
        .collect();
    let vertices: Vec<Vertex> = r
        .vertices
        .iter()
        .map(|x| Vertex {
            point: m(&pt, x.point),
        })
        .collect();
    let rows = Rows {
        regions: place(&regions, &pr),
        shells: place(&shells, &ps),
        faces: place(&faces, &pf),
        loops: place(&loops, &pl),
        fins: place(&fins, &pi),
        edges: place(&edges, &pe),
        vertices: place(&vertices, &pv),
        surfaces: place(&r.surfaces, &psu),
        curves: place(&r.curves, &pc),
        pcurves: place(&r.pcurves, &pp),
        points: place(&r.points, &pt),
    };
    let names = Names {
        regions: place(&n.regions, &pr),
        faces: place(&n.faces, &pf),
        edges: place(&n.edges, &pe),
        vertices: place(&n.vertices, &pv),
    };
    (rows, names)
}

/// `r` published by name with `n`.
fn publish(r: &Rows, n: &Names) -> Result<Body, TopoError> {
    builder(r).publish_named(n, BodyKind::Solid, outside(r), &mut meter())
}

/// The outside region of `r`: its void region.
fn outside(r: &Rows) -> Ix<Region> {
    let row = r
        .regions
        .iter()
        .position(|x| x.kind == RegionKind::Void)
        .unwrap();
    ix(row as u32)
}

/// The least of `items` (by row) whose owner is `owner`.
fn least_of(owners: impl Iterator<Item = (usize, u32)>, n: usize) -> Vec<Option<u32>> {
    let mut best = vec![None; n];
    for (owner, place) in owners {
        let b: &mut Option<u32> = &mut best[owner];
        *b = Some(b.map_or(place, |x| x.min(place)));
    }
    best
}

/// Checks that `body`'s rows are in the order of the module's keys, each strictly increasing.
fn assert_in_name_order(body: &Body) {
    let n = body.names().unwrap();
    for list in [&n.regions, &n.faces, &n.edges, &n.vertices] {
        assert!(list.windows(2).all(|w| w[0] < w[1]));
    }
    let a = body.arenas();
    let rows = |len: u32| len as usize;
    let shell_faces = least_of(
        a.faces
            .shell
            .iter()
            .zip(0..)
            .map(|(s, f)| (s.to_usize(), f)),
        rows(a.shells.len()),
    );
    let shells: Vec<_> = a
        .shells
        .region
        .iter()
        .map(|r| r.row())
        .zip(shell_faces)
        .collect();
    assert!(shells.windows(2).all(|w| w[0] < w[1]), "{shells:?}");
    let loop_edges = least_of(
        a.fins
            .loop_
            .iter()
            .zip(a.fins.edge.iter())
            .map(|(l, e)| (l.to_usize(), e.row())),
        rows(a.loops.len()),
    );
    let loops: Vec<_> = a
        .loops
        .face
        .iter()
        .map(|f| f.row())
        .zip(loop_edges)
        .collect();
    assert!(loops.windows(2).all(|w| w[0] < w[1]), "{loops:?}");
    let fins: Vec<_> = a
        .fins
        .edge
        .iter()
        .zip(a.fins.loop_.iter())
        .zip(a.fins.sense.iter())
        .map(|((e, l), s)| (e.row(), l.row(), *s == Sense::Reversed))
        .collect();
    assert!(fins.windows(2).all(|w| w[0] < w[1]), "{fins:?}");
    let pools = [
        least_of(
            a.faces
                .surface
                .iter()
                .zip(0..)
                .map(|(s, f)| (s.to_usize(), f)),
            rows(a.surfaces.len()),
        ),
        least_of(
            a.edges
                .curve
                .iter()
                .zip(0..)
                .map(|(c, e)| (c.to_usize(), e)),
            rows(a.curves.len()),
        ),
        least_of(
            a.fins
                .pcurve
                .iter()
                .zip(0..)
                .filter_map(|(p, f)| p.map(|p| (p.to_usize(), f))),
            rows(a.pcurves.len()),
        ),
        least_of(
            a.vertices
                .point
                .iter()
                .zip(0..)
                .map(|(p, v)| (p.to_usize(), v)),
            rows(a.points.len()),
        ),
    ];
    for keys in pools {
        assert!(keys.iter().all(Option::is_some));
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{keys:?}");
    }
}

/// Every row of `a` described by names alone: what it is and what it joins, each link written as
/// the name-made identity of its target. Sorted, so two descriptions are equal exactly when the
/// arenas join the same named entities in the same way, whatever their row order.
fn describe(a: &Arenas, n: &Names) -> Vec<String> {
    let none = NameKey([0; 16]);
    let face_name = |f: Ix<Face>| n.faces[f.to_usize()];
    let edge_name = |e: Ix<Edge>| n.edges[e.to_usize()];
    let shell_id = |s: Ix<Shell>| {
        let least = (0..a.faces.len())
            .filter(|&f| a.faces.get(ix(f)).unwrap().shell == s)
            .map(|f| n.faces[f as usize])
            .min()
            .unwrap_or(none);
        let region = a.shells.get(s).unwrap().region;
        (n.regions[region.to_usize()], least)
    };
    let loop_id = |l: Ix<Loop>| {
        let least = (0..a.fins.len())
            .map(|i| a.fins.get(ix(i)).unwrap())
            .filter(|fin| fin.loop_ == l)
            .map(|fin| edge_name(fin.edge))
            .min()
            .unwrap_or(none);
        (face_name(a.loops.get(l).unwrap().face), least)
    };
    let fin_id = |i: Ix<Fin>| {
        let fin = a.fins.get(i).unwrap();
        (loop_id(fin.loop_), edge_name(fin.edge), fin.sense)
    };
    let mut out = Vec::new();
    for r in 0..a.regions.len() {
        let x = a.regions.get(ix(r)).unwrap();
        out.push(format!(
            "region {:?} {:?} {:?}",
            n.regions[r as usize],
            x.kind,
            x.first_shell.map(shell_id)
        ));
    }
    for s in 0..a.shells.len() {
        let x = a.shells.get(ix(s)).unwrap();
        out.push(format!(
            "shell {:?} {:?} {:?}",
            shell_id(ix(s)),
            x.next.map(shell_id),
            x.first_face.map(face_name)
        ));
    }
    for f in 0..a.faces.len() {
        let x = a.faces.get(ix(f)).unwrap();
        out.push(format!(
            "face {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            n.faces[f as usize],
            shell_id(x.shell),
            x.next.map(face_name),
            a.surfaces.get(x.surface).unwrap(),
            x.sense,
            x.first_loop.map(loop_id),
            x.bound
        ));
    }
    for l in 0..a.loops.len() {
        let x = a.loops.get(ix(l)).unwrap();
        out.push(format!(
            "loop {:?} {:?} {:?}",
            loop_id(ix(l)),
            x.next.map(loop_id),
            x.first_fin.map(fin_id)
        ));
    }
    for i in 0..a.fins.len() {
        let x = a.fins.get(ix(i)).unwrap();
        out.push(format!(
            "fin {:?} {:?} {:?} {:?} {:?}",
            fin_id(ix(i)),
            fin_id(x.next),
            fin_id(x.prev),
            fin_id(x.radial),
            x.pcurve.map(|p| a.pcurves.get(p).unwrap())
        ));
    }
    for e in 0..a.edges.len() {
        let x = a.edges.get(ix(e)).unwrap();
        let vertex = |v: Ix<Vertex>| {
            let point = a.vertices.get(v).unwrap().point;
            (n.vertices[v.to_usize()], *a.points.get(point).unwrap())
        };
        out.push(format!(
            "edge {:?} {:?} {:?} {:?} {:?}",
            n.edges[e as usize],
            x.first_fin.map(fin_id),
            x.start.map(vertex),
            x.end.map(vertex),
            a.curves.get(x.curve).unwrap()
        ));
    }
    out.push(format!(
        "pools {} {} {} {}",
        a.surfaces.len(),
        a.curves.len(),
        a.pcurves.len(),
        a.points.len()
    ));
    out.sort();
    out
}

#[test]
fn any_order_of_the_rows_publishes_the_same_body() {
    let w = wall();
    for salt in 0..8 {
        let n = names(&w, salt);
        let body = publish(&w, &n).unwrap();
        assert_in_name_order(&body);
        assert_eq!(
            describe(body.arenas(), body.names().unwrap()),
            describe(&builder(&w).arenas, &n)
        );
        for seed in 0..16 {
            let (r, sn) = shuffle(&w, &n, mix(salt << 8 | seed));
            assert_eq!(publish(&r, &sn).unwrap(), body, "salt {salt}, seed {seed}");
        }
    }
}

#[test]
fn a_body_in_name_order_publishes_unchanged() {
    let w = wall();
    let n = names(&w, 3);
    let body = publish(&w, &n).unwrap();
    let again = Builder::edit(&body)
        .publish_named(
            body.names().unwrap(),
            BodyKind::Solid,
            body.outside(),
            &mut meter(),
        )
        .unwrap();
    assert_eq!(again, body);
    let plain = Builder::edit(&body)
        .publish(BodyKind::Solid, body.outside(), &mut meter())
        .unwrap();
    assert_eq!(plain.arenas(), body.arenas());
    assert_eq!(plain.names(), None);
    assert_eq!(body.kind(), BodyKind::Solid);
}

#[test]
fn fins_of_one_edge_in_one_loop_go_forward_first() {
    // A face whose loop runs along a seam edge both ways, as on a cylinder: edge 0 forward, the
    // seam (edge 1) forward, edge 2 reversed, the seam reversed.
    let mut w = wall();
    let face = 0u32;
    let first = 4 * face;
    let seam = w.fins[first as usize + 1].edge;
    let back = first + 3;
    w.fins[back as usize].edge = seam;
    w.fins[back as usize].sense = Sense::Reversed;
    w.fins[first as usize + 1].sense = Sense::Forward;
    let n = names(&w, 5);
    let body = publish(&w, &n).unwrap();
    assert_in_name_order(&body);
    let a = body.arenas();
    let on_seam: Vec<(u32, Sense)> = (0..a.fins.len())
        .map(|i| (i, a.fins.get(ix(i)).unwrap()))
        .filter(|(_, f)| {
            a.fins.get(ix(0)).is_some()
                && n.edges[seam.to_usize()] == body.names().unwrap().edges[f.edge.to_usize()]
        })
        .map(|(i, f)| (i, f.sense))
        .collect();
    let same_loop: Vec<_> = on_seam
        .iter()
        .filter(|(i, _)| {
            let l = a.fins.get(ix(*i)).unwrap().loop_;
            on_seam
                .iter()
                .filter(|(j, _)| a.fins.get(ix(*j)).unwrap().loop_ == l)
                .count()
                == 2
        })
        .collect();
    assert_eq!(same_loop.len(), 2);
    assert!(same_loop[0].0 < same_loop[1].0);
    assert_eq!(
        (same_loop[0].1, same_loop[1].1),
        (Sense::Forward, Sense::Reversed)
    );
    for seed in 0..8 {
        let (r, sn) = shuffle(&w, &n, seed);
        assert_eq!(publish(&r, &sn).unwrap(), body);
    }
}

#[test]
fn many_chunks_move_and_are_charged_by_the_chunk() {
    // One solid region with one shell of three chunks of faces and one face more, each on a
    // surface of its own, so the faces and surfaces span four chunks each.
    let rows = 3 * geom_topo::CHUNK_ROWS + 1;
    let w = wall();
    let mut faces = Vec::new();
    let mut surfaces = Vec::new();
    for f in 0..rows {
        faces.push(Face {
            shell: ix(0),
            next: (f + 1 < rows).then(|| ix(f + 1)),
            surface: ix(f),
            first_loop: None,
            ..w.faces[0]
        });
        surfaces.push(w.surfaces[(f % 6) as usize].clone());
    }
    let r = Rows {
        regions: w.regions.clone(),
        shells: w.shells.clone(),
        faces,
        loops: Vec::new(),
        fins: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
        surfaces,
        curves: Vec::new(),
        pcurves: Vec::new(),
        points: Vec::new(),
    };
    let n = names(&r, 9);
    let mut m = meter();
    let body = builder(&r)
        .publish_named(&n, BodyKind::Sheet, outside(&r), &mut m)
        .unwrap();
    assert_in_name_order(&body);
    // Publishing's link checks, twice: the regions' first shells 1; the shells' region, next and
    // first face 1 each; the faces' shell, next, surface and first loop 4 each. Then the moves:
    // regions 1, shells 1, faces 4, surfaces 4.
    let checks = 1 + 3 + 4 * 4;
    assert_eq!(m.used().work, 2 * checks + 1 + 1 + 4 + 4);
    let (s, sn) = shuffle(&r, &n, 1);
    assert_eq!(
        builder(&s)
            .publish_named(&sn, BodyKind::Sheet, outside(&s), &mut meter())
            .unwrap(),
        body
    );
}

#[test]
fn the_meter_is_charged_exactly_and_stops_publishing() {
    let mut w = wall();
    w.fins.iter_mut().for_each(|f| f.pcurve = None);
    w.pcurves.clear();
    let n = names(&w, 2);
    // 22 link columns of one chunk each, checked twice, and ten arenas of one chunk moved (the
    // pcurve pool is empty).
    let mut m = meter();
    publish_with(&w, &n, &mut m).unwrap();
    assert_eq!(m.used().work, 22 + 10 + 22);
    for (budget, ok) in [
        (21, false),
        (22, false),
        (31, false),
        (32, false),
        (53, false),
        (54, true),
    ] {
        let mut m = WorkMeter::new(Budget::new(budget, u64::MAX));
        let got = publish_with(&w, &n, &mut m);
        if ok {
            assert!(got.is_ok(), "budget {budget}");
        } else {
            assert_eq!(
                got,
                Err(TopoError::Stopped(Stop::OverBudget(BudgetKind::Work))),
                "budget {budget}"
            );
        }
    }
}

fn publish_with(r: &Rows, n: &Names, m: &mut WorkMeter<'_>) -> Result<Body, TopoError> {
    builder(r).publish_named(n, BodyKind::Solid, outside(r), m)
}

#[test]
fn publishing_checks_come_first() {
    let w = wall();
    let n = names(&w, 1);
    let mut b = builder(&w);
    b.arenas.faces.sense.push(Sense::Forward).unwrap();
    assert_eq!(
        b.publish_named(&n, BodyKind::Solid, outside(&w), &mut meter()),
        Err(TopoError::Ragged(Arena::Face))
    );
    let mut r = w.clone();
    r.faces[2].surface = ix(6);
    assert_eq!(
        publish(&r, &Names::default()),
        Err(TopoError::Dangling {
            link: Link::FaceSurface,
            row: 2
        })
    );
    assert_eq!(
        builder(&w).publish_named(&n, BodyKind::Solid, ix(0), &mut meter()),
        Err(TopoError::Outside)
    );
    assert_eq!(
        builder(&w).publish_named(&n, BodyKind::Solid, ix(2), &mut meter()),
        Err(TopoError::Outside)
    );
}

/// One named arena's names.
type Pick = fn(&mut Names) -> &mut Vec<NameKey>;

#[test]
fn names_must_be_one_per_row_and_unique() {
    let w = wall();
    let good = names(&w, 4);
    let cases: [(Pick, Arena); 4] = [
        (|n| &mut n.regions, Arena::Region),
        (|n| &mut n.faces, Arena::Face),
        (|n| &mut n.edges, Arena::Edge),
        (|n| &mut n.vertices, Arena::Vertex),
    ];
    for (list, arena) in cases {
        let mut n = good.clone();
        list(&mut n).pop();
        assert_eq!(publish(&w, &n), Err(TopoError::Names(arena)));
        let mut n = good.clone();
        let extra = list(&mut n)[0];
        list(&mut n).push(extra);
        assert_eq!(publish(&w, &n), Err(TopoError::Names(arena)));
        let mut n = good.clone();
        let l = list(&mut n);
        l[1] = l[0];
        assert_eq!(
            publish(&w, &n),
            Err(TopoError::DuplicateName {
                arena,
                first: 0,
                second: 1
            })
        );
    }
    // Arena by arena: a duplicate region name comes before a face count that is wrong.
    let mut n = good.clone();
    n.regions[1] = n.regions[0];
    n.faces.pop();
    assert_eq!(
        publish(&w, &n),
        Err(TopoError::DuplicateName {
            arena: Arena::Region,
            first: 0,
            second: 1
        })
    );
}

#[test]
fn rows_the_rule_cannot_place_are_refused() {
    let w = wall();
    let n = names(&w, 6);
    // Two shells with no faces in the outside region.
    let mut r = w.clone();
    let empty = Shell {
        region: ix(1),
        next: None,
        first_face: None,
    };
    r.shells.extend([empty, empty]);
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unordered {
            arena: Arena::Shell,
            first: 1,
            second: 2
        })
    );
    // One shell with no face is placed: it has its region.
    let mut r = w.clone();
    r.shells.push(empty);
    let body = publish(&r, &n).unwrap();
    assert_in_name_order(&body);
    // Two loops with no fins on one face; one is placed.
    let mut r = w.clone();
    let empty = Loop {
        face: ix(2),
        next: None,
        first_fin: None,
    };
    r.loops.push(empty);
    assert_in_name_order(&publish(&r, &n).unwrap());
    r.loops.push(empty);
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unordered {
            arena: Arena::Loop,
            first: 6,
            second: 7
        })
    );
    // Two fins on one edge, in one loop, in one sense.
    let mut r = w.clone();
    let copy = r.fins[5];
    r.fins.push(copy);
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unordered {
            arena: Arena::Fin,
            first: 5,
            second: 24
        })
    );
    // Shells before loops before fins.
    let mut r = w.clone();
    let shell = r.shells[0];
    r.shells.extend(
        [Shell {
            first_face: None,
            ..shell
        }; 2],
    );
    r.loops.extend([empty, empty]);
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unordered {
            arena: Arena::Shell,
            first: 1,
            second: 2
        })
    );
    r.shells.truncate(1);
    let copy = r.fins[5];
    r.fins.push(copy);
    assert_eq!(
        publish(&r, &n).map(|_| ()),
        Err(TopoError::Unordered {
            arena: Arena::Loop,
            first: 6,
            second: 7
        })
    );
}

#[test]
fn every_pool_row_must_be_used() {
    let w = wall();
    let n = names(&w, 7);
    let mut r = w.clone();
    r.surfaces.push(r.surfaces[0].clone());
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unused {
            arena: Arena::Surface,
            row: 6
        })
    );
    let mut r = w.clone();
    r.curves.push(r.curves[0].clone());
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unused {
            arena: Arena::Curve,
            row: 12
        })
    );
    let mut r = w.clone();
    r.fins[2].pcurve = None;
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unused {
            arena: Arena::Pcurve,
            row: 1
        })
    );
    let mut r = w.clone();
    r.points.push(r.points[0]);
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unused {
            arena: Arena::Point,
            row: 8
        })
    );
    // In turn: an unused surface before an unused point.
    r.surfaces.push(r.surfaces[0].clone());
    assert_eq!(
        publish(&r, &n),
        Err(TopoError::Unused {
            arena: Arena::Surface,
            row: 6
        })
    );
    // A pool row many entities share is placed by the least of them.
    let mut r = w.clone();
    r.faces[5].surface = ix(0);
    r.surfaces.truncate(5);
    let body = publish(&r, &n).unwrap();
    assert_in_name_order(&body);
    assert_eq!(body.arenas().surfaces.len(), 5);
    for seed in 0..4 {
        let (s, sn) = shuffle(&r, &n, seed);
        assert_eq!(publish(&s, &sn).unwrap(), body);
    }
}

#[test]
fn the_new_faults_read_plainly() {
    let cases = [
        (
            TopoError::Names(Arena::Face),
            "the names of the face arena are not one per row",
        ),
        (
            TopoError::DuplicateName {
                arena: Arena::Edge,
                first: 2,
                second: 9,
            },
            "edge rows 2 and 9 have the same name",
        ),
        (
            TopoError::Unordered {
                arena: Arena::Fin,
                first: 1,
                second: 4,
            },
            "fin rows 1 and 4 join the same names, so name order cannot place them",
        ),
        (
            TopoError::Unused {
                arena: Arena::Point,
                row: 3,
            },
            "no entity uses row 3 of the point pool",
        ),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text);
    }
}

#[test]
fn an_empty_body_publishes_by_name() {
    let r = Rows {
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
    };
    let n = names(&r, 0);
    let body = builder(&r)
        .publish_named(&n, BodyKind::Acorn, ix(0), &mut meter())
        .unwrap();
    assert_eq!(body.names(), Some(&n));
    assert_eq!(body.outside(), ix(0));
    let _ = wall_outside();
}
