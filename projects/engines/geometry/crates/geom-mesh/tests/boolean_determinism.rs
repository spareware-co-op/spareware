//! The replay hash of mesh booleans (piece G148): the union, intersection and difference, in one
//! arrangement, of each pair of the shapes bench's corpus at its smallest size class and radius
//! 2^13 steps, as the bench's `small` times them, and of three pairs of solids whose coordinates
//! reach ±(2^40 − 1). Every array of each
//! result mesh, its runs (the names it publishes) and its run-level history are written out and
//! hashed with SHA-256: the digest must be the same on every target, and must not move when the
//! merge is only made faster (chapters 19.5, 21 and 28.13; the golden store of piece G19).
//!
//! The round shapes take their points from `geom_num::math`'s `sinpi` and `cospi`, which are
//! correctly rounded, so every target rounds the same points to the grid. The cubes turn by the
//! rotations with cosines 3/5 and 5/13, whose doubles every target computes alike.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "the corpus generator, corners a, b, c and d as in the bench; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use geom_mesh::boolean::boolean;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, Facing, PropTable, Role, Vertex};
use geom_num::Exactness;
use geom_num::grid::P3;
use geom_num::math::{cospi, sinpi};
use geom_op::{NameId, OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-mesh/boolean";

/// The grid's last step each way: every coordinate lies within ±(2^40 − 1).
const TOP: i64 = (1 << 40) - 1;

/// A closed triangle mesh as polygons: `points` on the grid and `tris` of indices into them,
/// turned about if need be so that they face out of the solid they close.
fn triangles(points: &[[i64; 3]], tris: &[[u32; 3]], name: u8) -> Polygons {
    // Six times the volume by the divergence theorem: negative if the faces point in.
    let at = |i: u32| points[i as usize].map(i128::from);
    let mut volume = 0_i128;
    for t in tris {
        let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
        volume += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    let vertices = points
        .iter()
        .map(|p| Corner::Grid(P3::new(p[0], p[1], p[2]).unwrap()))
        .collect();
    let mut corners = Vec::new();
    let mut loop_start = vec![0_u32];
    for t in tris {
        if volume < 0 {
            corners.extend_from_slice(&[t[0], t[2], t[1]]);
        } else {
            corners.extend_from_slice(t);
        }
        loop_start.push(corners.len() as u32);
    }
    let faces = loop_start.len() as u32 - 1;
    Polygons {
        vertices,
        corners,
        loop_start,
        face_loops: (0..=faces).collect(),
        face_run: vec![0; faces as usize],
        runs: vec![Source {
            name: NameId::from_bytes([name; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    }
}

/// A surface of rows of `around` points, wrapped round in both directions (a torus) or closed at
/// two poles (a sphere): `at(row, column)` gives each point, and `rows` rows are made.
fn wrapped(
    rows: u32,
    around: u32,
    poles: Option<[[i64; 3]; 2]>,
    at: &dyn Fn(u32, u32) -> [i64; 3],
) -> (Vec<[i64; 3]>, Vec<[u32; 3]>) {
    let mut points = Vec::new();
    for r in 0..rows {
        for c in 0..around {
            points.push(at(r, c));
        }
    }
    let index = |r: u32, c: u32| (r % rows) * around + c % around;
    let mut tris = Vec::new();
    let bands = if poles.is_some() { rows - 1 } else { rows };
    if let Some(pole) = poles {
        let (north, south) = (points.len() as u32, points.len() as u32 + 1);
        points.extend(pole);
        for c in 0..around {
            tris.push([north, index(0, c), index(0, c + 1)]);
            tris.push([south, index(rows - 1, c + 1), index(rows - 1, c)]);
        }
    }
    for r in 0..bands {
        for c in 0..around {
            let (a, b, d, e) = (
                index(r, c),
                index(r, c + 1),
                index(r + 1, c),
                index(r + 1, c + 1),
            );
            tris.push([a, d, b]);
            tris.push([b, d, e]);
        }
    }
    (points, tris)
}

/// sin and cos of `turns` half-turns, correctly rounded.
fn sin_cos(turns: f64) -> (f64, f64) {
    (sinpi(turns).unwrap().get(), cospi(turns).unwrap().get())
}

/// A sphere of radius `radius` about `center`: `rings - 1` rows of `around` points.
fn sphere(rings: u32, around: u32, radius: f64, center: [i64; 3], name: u8) -> Polygons {
    let at = |r: u32, c: u32| {
        let (sin_theta, cos_theta) = sin_cos(f64::from(r + 1) / f64::from(rings));
        let (sin_phi, cos_phi) = sin_cos(2.0 * f64::from(c) / f64::from(around));
        [
            center[0] + (radius * sin_theta * cos_phi).round() as i64,
            center[1] + (radius * sin_theta * sin_phi).round() as i64,
            center[2] + (radius * cos_theta).round() as i64,
        ]
    };
    let top = radius.round() as i64;
    let pole = [
        [center[0], center[1], center[2] + top],
        [center[0], center[1], center[2] - top],
    ];
    let (points, tris) = wrapped(rings - 1, around, Some(pole), &at);
    triangles(&points, &tris, name)
}

/// A torus of major radius `major` and minor radius `minor` about `center`.
fn torus(
    rings: u32,
    around: u32,
    (major, minor): (f64, f64),
    center: [i64; 3],
    name: u8,
) -> Polygons {
    let at = |r: u32, c: u32| {
        let (sin_v, cos_v) = sin_cos(2.0 * f64::from(r) / f64::from(rings));
        let (sin_u, cos_u) = sin_cos(2.0 * f64::from(c) / f64::from(around));
        let ring = major + minor * cos_v;
        [
            center[0] + (ring * cos_u).round() as i64,
            center[1] + (ring * sin_u).round() as i64,
            center[2] + (minor * sin_v).round() as i64,
        ]
    };
    let (points, tris) = wrapped(rings, around, None, &at);
    triangles(&points, &tris, name)
}

/// A cube of half-width `half` about `center`, each face `k` by `k` squares of two triangles; when
/// `turned`, rotated about z by the angle with cosine 3/5 and then about x by the one with cosine
/// 5/13, each point rounded to the grid after.
fn cube(k: u32, half: i64, center: [i64; 3], turned: bool, name: u8) -> Polygons {
    let coordinate = |i: u32| -half + (2 * half * i64::from(i) + i64::from(k) / 2) / i64::from(k);
    let mut ids = BTreeMap::new();
    let mut points: Vec<[i64; 3]> = Vec::new();
    let mut tris = Vec::new();
    let mut point = |p: [u32; 3]| -> u32 {
        *ids.entry(p).or_insert_with(|| {
            let mut v = p.map(coordinate).map(|x| x as f64);
            if turned {
                v = [
                    (3.0 * v[0] - 4.0 * v[1]) / 5.0,
                    (4.0 * v[0] + 3.0 * v[1]) / 5.0,
                    v[2],
                ];
                v = [
                    v[0],
                    (5.0 * v[1] - 12.0 * v[2]) / 13.0,
                    (12.0 * v[1] + 5.0 * v[2]) / 13.0,
                ];
            }
            points.push([
                center[0] + v[0].round() as i64,
                center[1] + v[1].round() as i64,
                center[2] + v[2].round() as i64,
            ]);
            points.len() as u32 - 1
        })
    };
    for axis in 0..3 {
        for end in [0, k] {
            for i in 0..k {
                for j in 0..k {
                    let corner = |a: u32, b: u32| {
                        let mut p = [0; 3];
                        p[axis] = end;
                        p[(axis + 1) % 3] = a;
                        p[(axis + 2) % 3] = b;
                        p
                    };
                    let (a, b, c, d) = (
                        point(corner(i, j)),
                        point(corner(i + 1, j)),
                        point(corner(i + 1, j + 1)),
                        point(corner(i, j + 1)),
                    );
                    // Facing out: along the axis at its far end, against it at its near end.
                    if end == k {
                        tris.push([a, b, c]);
                        tris.push([a, c, d]);
                    } else {
                        tris.push([a, c, b]);
                        tris.push([a, d, c]);
                    }
                }
            }
        }
    }
    triangles(&points, &tris, name)
}

/// The shapes bench's four pairs at size class 0, about 1,000 faces a side, radius 2^13 steps.
fn shape_pairs() -> Vec<(Polygons, Polygons)> {
    let s = 1_i64 << 13;
    let sf = s as f64;
    vec![
        (
            sphere(17, 32, sf, [0; 3], 1),
            sphere(17, 32, sf, [s / 2, 0, 0], 2),
        ),
        (
            sphere(17, 32, sf, [0; 3], 1),
            torus(16, 32, (sf, sf * 0.35), [s / 5, 0, 0], 2),
        ),
        (
            torus(16, 32, (sf, sf * 0.35), [0; 3], 1),
            torus(16, 32, (sf, sf * 0.35), [s, 0, 0], 2),
        ),
        (cube(9, s, [0; 3], false, 1), cube(9, s, [0; 3], true, 2)),
    ]
}

/// Solids that reach the grid's ends: two boxes that cross, one from −(2^40 − 1) and the other to
/// 2^40 − 1 on x; a turned cube across one of them, its corners off the axes at about 0.87 · 2^40;
/// and a box that fills the grid around a turned cube, a cavity in the difference.
fn extreme_pairs() -> Vec<(Polygons, Polygons)> {
    let half = 3 * (1 << 38) - 1;
    vec![
        (
            cube(2, half, [-(1 << 38), 0, 0], false, 1),
            cube(2, half, [1 << 38, 1 << 37, 1 << 36], false, 2),
        ),
        (
            cube(2, half, [1 << 38, 1 << 37, 1 << 36], false, 1),
            cube(2, (1 << 39) - 1, [0; 3], true, 2),
        ),
        (
            cube(1, TOP, [0; 3], false, 1),
            cube(3, (1 << 39) - 1, [0; 3], true, 2),
        ),
    ]
}

fn words(out: &mut Vec<u8>, items: &[u32]) {
    out.extend((items.len() as u64).to_le_bytes());
    for i in items {
        out.extend(i.to_le_bytes());
    }
}

fn facings(out: &mut Vec<u8>, items: &[Facing]) {
    out.extend((items.len() as u64).to_le_bytes());
    for f in items {
        out.extend(f.row.to_le_bytes());
        out.push(u8::from(f.flipped));
    }
}

/// Every array of `m`, its runs and its volume, in canonical order.
fn write(out: &mut Vec<u8>, m: &ExactMesh) {
    let p = m.parts();
    out.extend((p.planes.len() as u64).to_le_bytes());
    for plane in &p.planes {
        for c in plane.n() {
            out.extend(c.to_le_bytes());
        }
        out.extend(plane.d().to_le_bytes());
    }
    out.extend((p.verts.len() as u64).to_le_bytes());
    for v in &p.verts {
        match v {
            Vertex::Grid(c) => {
                out.push(0);
                for x in [c.x.get(), c.y.get(), c.z.get()] {
                    out.extend(x.to_le_bytes());
                }
            }
            Vertex::Planes(rows) => {
                out.push(1);
                for r in rows {
                    out.extend(r.to_le_bytes());
                }
            }
        }
    }
    facings(out, &p.face_plane);
    words(out, &p.face_loops);
    words(out, &p.loop_start);
    words(out, &p.he_vert);
    facings(out, &p.he_plane);
    words(out, &p.he_pair);
    words(out, &p.he_prop);
    // The runs, the names the mesh publishes, as their debug text: names, roles, flags and the
    // faces each covers.
    let runs = format!("{:?}", p.runs);
    out.extend((runs.len() as u64).to_le_bytes());
    out.extend(runs.as_bytes());
    match m
        .volume(&mut WorkMeter::new(Budget::of_work(1 << 24)))
        .unwrap()
    {
        Volume::Exact(v) => {
            for limb in v.to_le_limbs() {
                out.extend(limb.to_le_bytes());
            }
        }
        Volume::Certified(v) => {
            out.extend(v.lo().to_bits().to_le_bytes());
            out.extend(v.hi().to_bits().to_le_bytes());
        }
    }
}

/// The corpus's canonical bytes and how many results went in.
fn corpus() -> (Vec<u8>, u32) {
    let step = OpPath::new(OpId::from_bytes([7; 16]));
    let tests = [
        WindingTest::unite(),
        WindingTest::intersect(2),
        WindingTest::subtract(0),
    ];
    let mut out = Vec::new();
    let mut results = 0;
    let pairs = shape_pairs().into_iter().chain(extreme_pairs());
    for (a, b) in pairs {
        let mesh = |input: &Polygons| {
            build(
                input,
                &Exec::serial(),
                &mut WorkMeter::new(Budget::UNLIMITED),
            )
            .unwrap()
        };
        let (a, b) = (mesh(&a), mesh(&b));
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let done = boolean(&[&a, &b], &tests, &step, &Exec::serial(), &mut meter).unwrap();
        for outcome in &done {
            write(&mut out, &outcome.mesh);
            let history = format!("{:?}", outcome.history);
            out.extend((history.len() as u64).to_le_bytes());
            out.extend(history.as_bytes());
            results += 1;
        }
    }
    (out, results)
}

#[test]
fn boolean_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.0), rec)
    });
    let (bytes, results) = &checked.value;
    println!(
        "geom-mesh boolean corpus: {results} results, {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert_eq!(*results, 21, "every result ran");
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target orders a plane, a vertex, a face, a name \
             or a history record differently (compare this log's digest with another target's), \
             or a change meant only to make the merge faster changed what it publishes"
        )
    });
}
