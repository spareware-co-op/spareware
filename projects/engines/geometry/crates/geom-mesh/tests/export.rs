//! Exports through the public interface (piece G149), against what 19.5 and 24.10 say of them:
//! exact decimals that read back to the grid, `f32`s within the stated bound, and rounding that
//! breaks a mesh refused with its place.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "test checks; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::num::NonZeroUsize;

use geom_mesh::exact::ExactMesh;
use geom_mesh::exact::build::{Polygons, build};
use geom_mesh::export::{Export, ExportError, Flaw, Format, decimal, export, triangles};
use geom_num::Exactness;
use geom_num::grid::{GridStep, Steps};
use geom_pool::native::Rayon;
use geom_run::{Budget, Exec, WorkMeter};

mod shapes;

use shapes::{cuboid, frustum, g, join, soup};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(1 << 22))
}

fn mesh(input: &Polygons) -> ExactMesh {
    build(input, &Exec::serial(), &mut meter()).unwrap()
}

fn written(m: &ExactMesh, format: Format) -> Export {
    export(
        m,
        format,
        GridStep::Q0,
        false,
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap()
}

fn text(e: &Export) -> String {
    String::from_utf8(e.bytes.clone()).unwrap()
}

/// A decimal of millimetres in steps of Q0, which must come out whole.
fn steps(text: &str) -> i64 {
    let negative = text.starts_with('-');
    let digits = text.trim_start_matches('-');
    let (whole, part) = digits.split_once('.').unwrap_or((digits, ""));
    let mut value: i128 = whole.parse::<i128>().unwrap() * 100_000_000;
    let mut scale = 10_000_000;
    for c in part.chars() {
        value += i128::from(c.to_digit(10).unwrap()) * scale;
        scale /= 10;
    }
    // One step is 15,625 hundred-millionths of a millimetre.
    assert_eq!(value % 15_625, 0, "{text}");
    let out = (value / 15_625) as i64;
    if negative { -out } else { out }
}

/// Six times the volume of triangles, from their corners.
fn six_volume(points: &[[i64; 3]], corners: &[[u32; 3]]) -> i128 {
    let mut sum = 0_i128;
    for c in corners {
        let [p, q, r] = c.map(|v| points[v as usize].map(i128::from));
        sum += p[0] * (q[1] * r[2] - q[2] * r[1]) - p[1] * (q[0] * r[2] - q[2] * r[0])
            + p[2] * (q[0] * r[1] - q[1] * r[0]);
    }
    sum
}

#[test]
fn decimals_are_exact() {
    assert_eq!(decimal(0, GridStep::Q0), "0");
    assert_eq!(decimal(1, GridStep::Q0), "0.00015625");
    assert_eq!(decimal(1, GridStep::Q1), "0.00000625");
    assert_eq!(decimal(1, GridStep::Q2), "0.00000025");
    assert_eq!(decimal(6400, GridStep::Q0), "1");
    assert_eq!(decimal(-6400, GridStep::Q0), "-1");
    assert_eq!(decimal(6401, GridStep::Q0), "1.00015625");
    assert_eq!(decimal(3200, GridStep::Q0), "0.5");
    assert_eq!(decimal(-1, GridStep::Q0), "-0.00015625");
}

#[test]
fn obj_carries_grid_points_exactly() {
    let m = mesh(&cuboid(6401, 3, 12800));
    let e = written(&m, Format::Obj);
    assert_eq!(e.class, Exactness::Exact);
    assert_eq!(e.bound, 0);
    assert_eq!(e.flaws, Vec::new());
    let (mut points, mut corners) = (Vec::new(), Vec::new());
    for line in text(&e).lines() {
        let mut parts = line.split(' ');
        match parts.next() {
            Some("v") => {
                let c: Vec<i64> = parts.map(steps).collect();
                points.push([c[0], c[1], c[2]]);
            }
            Some("f") => {
                let c: Vec<u32> = parts.map(|v| v.parse::<u32>().unwrap() - 1).collect();
                corners.push([c[0], c[1], c[2]]);
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!(points.len(), 8);
    assert_eq!(corners.len(), 12);
    assert_eq!(six_volume(&points, &corners), 6 * 6401 * 3 * 12800);
}

#[test]
fn ascii_stl_and_3mf_carry_the_same_points() {
    let m = mesh(&cuboid(6401, 3, 12800));
    let stl = text(&written(&m, Format::AsciiStl));
    assert!(stl.starts_with("solid "));
    assert!(stl.ends_with("endsolid mesh\n"));
    assert_eq!(
        stl.matches("facet normal").count() - stl.matches("endfacet").count(),
        0
    );
    assert_eq!(stl.matches("endfacet").count(), 12);
    let mut corners = Vec::new();
    for line in stl.lines().filter(|l| l.trim_start().starts_with("vertex")) {
        let c: Vec<i64> = line.split_whitespace().skip(1).map(steps).collect();
        corners.push([c[0], c[1], c[2]]);
    }
    let triangles: Vec<[u32; 3]> = (0..12).map(|t| [3 * t, 3 * t + 1, 3 * t + 2]).collect();
    assert_eq!(six_volume(&corners, &triangles), 6 * 6401 * 3 * 12800);
    let model = text(&written(&m, Format::ThreeMf));
    assert!(model.contains("unit=\"millimeter\""));
    assert_eq!(model.matches("<vertex ").count(), 8);
    assert_eq!(model.matches("<triangle ").count(), 12);
    assert!(model.contains("x=\"1.00015625\""));
    assert!(model.contains("<build><item objectid=\"1\"/></build>"));
}

#[test]
fn a_face_with_a_hole_splits_into_triangles_round_it() {
    // A square plate with a square hole through it, as faces with holes and four inner walls.
    let vertices = vec![
        g(0, 0, 0),
        g(9, 0, 0),
        g(9, 9, 0),
        g(0, 9, 0),
        g(3, 3, 0),
        g(6, 3, 0),
        g(6, 6, 0),
        g(3, 6, 0),
        g(0, 0, 2),
        g(9, 0, 2),
        g(9, 9, 2),
        g(0, 9, 2),
        g(3, 3, 2),
        g(6, 3, 2),
        g(6, 6, 2),
        g(3, 6, 2),
    ];
    let walls: Vec<Vec<u32>> = (0..4)
        .flat_map(|i| {
            let j = (i + 1) % 4;
            [vec![i, j, j + 8, i + 8], vec![j + 4, i + 4, i + 12, j + 12]]
        })
        .collect();
    let refs: Vec<&[u32]> = walls.iter().map(Vec::as_slice).collect();
    let mut input = soup(vertices, &refs);
    // Two faces of two loops each.
    let rings: [&[u32]; 4] = [
        &[3, 2, 1, 0],
        &[4, 5, 6, 7],
        &[8, 9, 10, 11],
        &[15, 14, 13, 12],
    ];
    let faces = input.face_loops.len() as u32 - 1;
    for ring in rings {
        input.corners.extend_from_slice(ring);
        input.loop_start.push(input.corners.len() as u32);
    }
    input.face_loops.push(faces + 2);
    input.face_loops.push(faces + 4);
    input.face_run.extend([0, 0]);
    let m = mesh(&input);
    let t = triangles(&m, &mut meter()).unwrap();
    let points: Vec<[i64; 3]> = t
        .vertices
        .iter()
        .map(|p| [p.x.get(), p.y.get(), p.z.get()])
        .collect();
    assert_eq!(six_volume(&points, &t.corners), 6 * 2 * (81 - 9));
    assert_eq!(t.corners.len(), t.faces.len());
    // Each ring of 8 corners and a hole is 8 triangles; each wall, 2.
    assert_eq!(t.corners.len(), 8 * 2 + 8 * 2);
}

#[test]
fn f32s_are_within_the_bound_stated() {
    let m = mesh(&cuboid(1_000_003, 777_777, 123_457));
    let e = written(&m, Format::BinaryStl);
    assert_eq!(e.bytes.len(), 84 + 12 * 50);
    assert_eq!(u32::from_le_bytes(e.bytes[80..84].try_into().unwrap()), 12);
    let Exactness::Certified { bound, step } = e.class else {
        panic!("f32s are rounded");
    };
    assert_eq!(step, GridStep::Q0);
    assert_eq!(bound, Steps(e.bound.div_ceil(625)));
    assert!(e.bound > 0);
    // The chunk's origin is the middle of the box, so each written coordinate is half a side from
    // zero, one way or the other.
    let half = [1_000_003 / 2, 777_777 / 2, 123_457 / 2];
    let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
    for t in 0..12 {
        for v in 0..3 {
            for k in 0..3 {
                let at = 84 + t * 50 + 12 + v * 12 + k * 4;
                let value = f32::from_le_bytes(e.bytes[at..at + 4].try_into().unwrap());
                low[k] = low[k].min(f64::from(value));
                high[k] = high[k].max(f64::from(value));
            }
        }
    }
    for k in 0..3 {
        // In quarter-nanometres, exact in f64: 24 bits by 22.
        let span = (high[k] - low[k]) * 4_000_000.0;
        let side = [1_000_003_f64, 777_777.0, 123_457.0][k] * 625.0;
        assert!((span - side).abs() <= 2.0 * e.bound as f64, "{span} {side}");
        assert!(low[k] <= 0.0 && high[k] >= 0.0);
        let _ = half;
    }
}

#[test]
fn glb_is_a_well_formed_file() {
    let m = mesh(&cuboid(64_000, 64_000, 32_000));
    let e = written(&m, Format::Glb);
    let b = &e.bytes;
    assert_eq!(&b[0..4], b"glTF");
    assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize,
        b.len()
    );
    let json = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    assert_eq!(json % 4, 0);
    assert_eq!(&b[16..20], b"JSON");
    let text = std::str::from_utf8(&b[20..20 + json]).unwrap();
    assert!(text.contains("\"version\":\"2.0\""));
    assert!(text.contains("\"count\":8"));
    assert!(text.contains("\"count\":36"));
    // Half of 10 mm, in metres, for the origin; half a side either way for the box.
    assert!(
        text.contains("\"translation\":[0.005,0.005,0.0025]"),
        "{text}"
    );
    assert!(text.contains("\"min\":[-5,-5,-2.5]"), "{text}");
    assert!(text.contains("\"max\":[5,5,2.5]"), "{text}");
    let binary = u32::from_le_bytes(b[20 + json..24 + json].try_into().unwrap()) as usize;
    assert_eq!(binary, 8 * 12 + 36 * 4);
    assert_eq!(&b[24 + json..28 + json], b"BIN\0");
    assert_eq!(b.len(), 28 + json + binary);
    // A value on the lattice is exact: every corner is 5 mm or 2.5 mm from the origin.
    let first = f32::from_le_bytes(b[28 + json..32 + json].try_into().unwrap());
    assert_eq!(first.abs(), 5.0);
}

#[test]
fn points_off_the_grid_round_once_and_say_so() {
    let m = mesh(&frustum());
    let e = written(&m, Format::Obj);
    assert_eq!(e.bound, 313);
    assert_eq!(
        e.class,
        Exactness::Certified {
            bound: Steps(1),
            step: GridStep::Q0
        }
    );
    assert!(text(&e).starts_with("v "));
}

/// A plate from the origin to (`long`, `long`, `thin`).
fn thin(long: i64, thin: i64) -> Polygons {
    cuboid(long, long, thin)
}

#[test]
fn a_plate_too_thin_for_the_lattice_is_refused() {
    let m = mesh(&thin(1 << 36, 1));
    let refused = export(
        &m,
        Format::BinaryStl,
        GridStep::Q0,
        false,
        &Exec::serial(),
        &mut meter(),
    );
    let Err(ExportError::RoundingFlip { flaws }) = refused else {
        panic!("a step's thickness is less than the lattice's unit");
    };
    assert!(flaws.iter().any(|f| matches!(f, Flaw::Flipped { .. })));
    let accepted = export(
        &m,
        Format::BinaryStl,
        GridStep::Q0,
        true,
        &Exec::serial(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(accepted.flaws, flaws);
    assert!(matches!(accepted.class, Exactness::Certified { .. }));
    assert_ne!(refused_text(&flaws), "");
}

fn refused_text(flaws: &[Flaw]) -> String {
    ExportError::RoundingFlip {
        flaws: flaws.to_vec(),
    }
    .to_string()
}

#[test]
fn plates_that_come_to_touch_are_named() {
    // Two plates a little apart at the far end of a long chunk, each thick enough to stay whole.
    let far = 1_i64 << 36;
    let mut upper = cuboid(far, far, 1 << 16);
    for v in &mut upper.vertices {
        let geom_mesh::exact::build::Corner::Grid(p) = *v else {
            panic!("a box is on the grid");
        };
        *v = g(p.x.get(), p.y.get(), p.z.get() + (1 << 16) + 1);
    }
    let m = mesh(&join(cuboid(far, far, 1 << 16), &upper));
    let refused = export(
        &m,
        Format::BinaryStl,
        GridStep::Q0,
        false,
        &Exec::serial(),
        &mut meter(),
    );
    let Err(ExportError::RoundingFlip { flaws }) = refused else {
        panic!("one step apart is less than the lattice's unit");
    };
    assert!(
        flaws.iter().any(|f| matches!(f, Flaw::Touching { .. })),
        "{flaws:?}"
    );
}

#[test]
fn the_same_mesh_gives_the_same_bytes_on_any_pool() {
    let m = mesh(&frustum());
    let pool = Rayon::new(NonZeroUsize::new(4).unwrap()).unwrap();
    for format in [
        Format::Obj,
        Format::AsciiStl,
        Format::ThreeMf,
        Format::BinaryStl,
        Format::Glb,
    ] {
        let one = export(
            &m,
            format,
            GridStep::Q0,
            false,
            &Exec::serial(),
            &mut meter(),
        )
        .unwrap();
        let two = export(
            &m,
            format,
            GridStep::Q0,
            false,
            &Exec::with_pool(&pool),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(one, two);
    }
}

#[test]
fn the_meter_stops_an_export() {
    let m = mesh(&cuboid(2, 2, 2));
    let mut short = WorkMeter::new(Budget::of_work(4));
    assert!(matches!(
        export(
            &m,
            Format::Obj,
            GridStep::Q0,
            false,
            &Exec::serial(),
            &mut short
        ),
        Err(ExportError::Stopped(_) | ExportError::Cdt(_))
    ));
}
