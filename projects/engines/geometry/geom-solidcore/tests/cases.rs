//! The 71 stored cases, run through geom-solidcore (decision 32.70): one test per case, each
//! holding solidcore's answer to the expected answer in `tests/cases.json`, with no case left out
//! and no check loosened.
//!
//! A case solidcore answers wrongly is a named fault in `tests/named-faults.json`: its test still
//! expects the kernel's answer and so fails, by design, and `kernel.yml`'s job `solidcore` passes
//! only when the tests that fail are exactly the named faults. A named fault that starts to pass,
//! or a case that fails without being named, turns the job red.

use std::sync::OnceLock;

use geom_solidcore::{Error, Modeller, Piece, Solid};
use serde_json::Value;

/// The case table.
fn table() -> &'static Value {
    static TABLE: OnceLock<Value> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("cases.json")).expect("tests/cases.json is JSON")
    })
}

fn tolerance(key: &str) -> f64 {
    table()["tolerances"][key]
        .as_f64()
        .unwrap_or_else(|| panic!("tolerance {key}"))
}

fn case(id: &str) -> &'static Value {
    table()["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no case {id} in tests/cases.json"))
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn p3(v: &Value) -> [f64; 3] {
    [num(&v[0]), num(&v[1]), num(&v[2])]
}

fn from_bits(v: &Value) -> Option<f64> {
    v.as_str()
        .map(|s| f64::from_bits(u64::from_str_radix(s, 16).expect("hex bits")))
}

/// Builds an operand: a primitive, or a boolean of two operands.
fn solid(m: &mut Modeller, v: &Value) -> Result<Solid, Error> {
    for op in ["union", "difference", "intersection"] {
        if let Some(pair) = v.get(op) {
            let a = solid(m, &pair[0])?;
            let b = solid(m, &pair[1])?;
            return boolean(m, op, &a, &b);
        }
    }
    match v["shape"].as_str().expect("shape") {
        "cuboid" => m.cuboid(p3(&v["corner"]), p3(&v["size"])),
        "sphere" => m.sphere(p3(&v["centre"]), num(&v["radius"])),
        "cylinder" => m.cylinder(p3(&v["base"]), num(&v["radius"]), num(&v["height"])),
        "cone" => m.cone(
            p3(&v["base"]),
            num(&v["base_radius"]),
            num(&v["top_radius"]),
            num(&v["height"]),
        ),
        s => panic!("unknown shape {s}"),
    }
}

fn boolean(m: &mut Modeller, op: &str, a: &Solid, b: &Solid) -> Result<Solid, Error> {
    match op {
        "union" => m.union(a, b),
        "difference" => m.difference(a, b),
        "intersection" => m.intersection(a, b),
        _ => panic!("unknown boolean {op}"),
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// The distance of `q` from an exactly known surface (for a solid, from its nearest boundary
/// piece), the expected answer's own surfaces.
fn residual(s: &Value, q: [f64; 3]) -> f64 {
    if let Some(p) = s.get("plane") {
        let n = p3(&p["normal"]);
        return (dot(sub(q, p3(&p["at"])), n) / norm(n)).abs();
    }
    if let Some(p) = s.get("sphere") {
        return (norm(sub(q, p3(&p["centre"]))) - num(&p["radius"])).abs();
    }
    if let Some(p) = s.get("solid_cylinder") {
        let b = p3(&p["base"]);
        let (r, h) = (num(&p["radius"]), num(&p["height"]));
        let rad = ((q[0] - b[0]).powi(2) + (q[1] - b[1]).powi(2)).sqrt();
        let (z0, z1) = (b[2], b[2] + h);
        let side = (rad - r).abs().max((z0 - q[2]).max(q[2] - z1).max(0.0));
        let cap = |z: f64| (q[2] - z).abs().max((rad - r).max(0.0));
        return side.min(cap(z0)).min(cap(z1));
    }
    if let Some(p) = s.get("solid_cone") {
        let b = p3(&p["base"]);
        let (rb, rt, h) = (
            num(&p["base_radius"]),
            num(&p["top_radius"]),
            num(&p["height"]),
        );
        let rad = ((q[0] - b[0]).powi(2) + (q[1] - b[1]).powi(2)).sqrt();
        let t = (q[2] - b[2]) / h;
        let rz = rb + (rt - rb) * t;
        let slope = (rt - rb) / h;
        let side = ((rad - rz) / (1.0 + slope * slope).sqrt())
            .abs()
            .max((-t).max(t - 1.0).max(0.0) * h);
        let cap = |z: f64, rr: f64| (q[2] - z).abs().max((rad - rr).max(0.0));
        return side.min(cap(b[2], rb)).min(cap(b[2] + h, rt));
    }
    panic!("unknown surface {s}")
}

/// The worst residual against both surfaces, and the polyline length, of every piece.
fn sample(m: &Modeller, pieces: &[Piece], surfaces: &Value) -> (f64, f64) {
    let n = tolerance("samples_per_piece") as usize;
    let (mut worst, mut len) = (0.0f64, 0.0);
    for piece in pieces {
        let (t0, t1) = piece.span();
        let mut prev: Option<[f64; 3]> = None;
        for i in 0..=n {
            let t = t0 + (t1 - t0) * (i as f64) / (n as f64);
            let q = piece.point_at(m, t).expect("a point on the piece");
            worst = worst
                .max(residual(&surfaces[0], q))
                .max(residual(&surfaces[1], q));
            if let Some(p) = prev {
                len += norm(sub(q, p));
            }
            prev = Some(q);
        }
    }
    (worst, len)
}

fn run_intersection(c: &Value) {
    let mut m = Modeller::new();
    let operands: Vec<Solid> = c["operands"]
        .as_array()
        .expect("operands")
        .iter()
        .map(|o| solid(&mut m, o).expect("operand"))
        .collect();
    let pieces = match c["operation"].as_str() {
        Some("plane_section") => m
            .plane_section(
                &operands[0],
                p3(&c["plane"]["at"]),
                p3(&c["plane"]["normal"]),
            )
            .expect("plane_section"),
        Some("meet") => {
            m.meet(
                &operands[0],
                &operands[1],
                c["with_points"].as_bool().expect("with_points"),
            )
            .expect("meet")
            .pieces
        }
        o => panic!("unknown operation {o:?}"),
    };
    let (worst, len) = sample(&m, &pieces, &c["surfaces"]);
    assert!(
        worst <= tolerance("max_residual"),
        "{}: a point {worst:e} off a surface",
        c["name"]
    );
    if let Some(exact) = from_bits(&c["length_bits"]) {
        let err = if exact == 0.0 {
            len
        } else {
            (len - exact).abs() / exact
        };
        assert!(
            err <= tolerance("length_relerr"),
            "{}: length {len:?}, expected {exact:?} (relative error {err:e})",
            c["name"]
        );
    }
}

fn run_boolean(c: &Value) {
    let mut m = Modeller::new();
    let a = solid(&mut m, &c["operands"][0]).expect("first operand");
    let b = solid(&mut m, &c["operands"][1]).expect("second operand");
    let r =
        boolean(&mut m, c["operation"].as_str().expect("operation"), &a, &b).expect("the boolean");
    let faces = m.face_count(&r);
    if let Some(want) = c.get("faces").and_then(Value::as_u64) {
        assert_eq!(faces as u64, want, "{}: faces", c["name"]);
    }
    let exact = from_bits(&c["volume_bits"]).expect("volume_bits");
    if exact == 0.0 && faces == 0 {
        return;
    }
    let v = m.volume(&r).expect("the volume");
    let rel = if exact == 0.0 {
        v.abs()
    } else {
        (v - exact).abs() / exact
    };
    let bound = if c["curved"].as_bool().expect("curved") {
        tolerance("volume_relerr_curved")
    } else {
        tolerance("volume_relerr_flat")
    };
    assert!(
        rel <= bound,
        "{}: volume {v:?}, expected {exact:?} (relative error {rel:e})",
        c["name"]
    );
}

fn run(id: &str) {
    let c = case(id);
    match c["operation"].as_str() {
        Some("plane_section" | "meet") => run_intersection(c),
        _ => run_boolean(c),
    }
}

/// The operands are not changed by an operation: a boolean of the same two solids twice gives
/// the same volume, and each operand's own volume is the same after it.
#[test]
fn operands_are_not_consumed() {
    let mut m = Modeller::new();
    let a = m.cuboid([0.0; 3], [1.0; 3]).expect("a");
    let b = m.cuboid([0.5; 3], [1.0; 3]).expect("b");
    let first = m.union(&a, &b).expect("union");
    let second = m.union(&a, &b).expect("union again");
    assert_eq!(m.volume(&first), m.volume(&second));
    assert_eq!(m.volume(&a), Ok(1.0));
    assert_eq!(m.volume(&b), Ok(1.0));
}

/// Every case of the table has its test below, under its own name, and no test names a case the
/// table does not have.
#[test]
fn every_case_has_its_test() {
    let names = include_str!("case_names.txt");
    let ids: Vec<&str> = table()["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|c| c["id"].as_str().expect("id"))
        .collect();
    let listed: Vec<&str> = names
        .lines()
        .map(|l| l.split(' ').next().expect("id"))
        .collect();
    assert_eq!(ids, listed);
    assert_eq!(ids.len(), 71);
    let source = include_str!("cases.rs");
    for line in names.lines() {
        let mut parts = line.split(' ');
        let (id, name) = (parts.next().expect("id"), parts.next().expect("name"));
        assert!(
            source.contains(&format!("    {name} => \"{id}\",")),
            "case {id} has no test {name}"
        );
    }
}

/// Every named fault names a case of the table and its test, with a fault name and what it
/// blocks.
#[test]
fn named_faults_name_their_cases() {
    let faults: Value =
        serde_json::from_str(include_str!("named-faults.json")).expect("named-faults.json");
    let names = include_str!("case_names.txt");
    for f in faults["faults"].as_array().expect("faults") {
        let id = f["case"].as_str().expect("case");
        let test = f["test"].as_str().expect("test");
        assert!(
            names.lines().any(|l| l == format!("{id} {test}")),
            "{id} {test}"
        );
        for key in ["fault", "solidcore_gives", "expected", "blocks"] {
            assert!(
                f[key].as_str().is_some_and(|s| !s.is_empty()),
                "{id}: {key}"
            );
        }
    }
}

macro_rules! cases {
    ($($name:ident => $id:literal,)*) => {
        $(
            #[test]
            fn $name() {
                run($id);
            }
        )*
    };
}

cases! {
    isect_00_plane_sphere_n_z_d_0 => "isect-00",
    isect_01_plane_sphere_n_z_d_0_3 => "isect-01",
    isect_02_plane_sphere_n_z_d_0_6 => "isect-02",
    isect_03_plane_sphere_n_z_d_0_9 => "isect-03",
    isect_04_plane_sphere_n_z_d_0_999 => "isect-04",
    isect_05_plane_sphere_n_z_d_1 => "isect-05",
    isect_06_plane_sphere_n_z_d_1_2 => "isect-06",
    isect_07_plane_sphere_n_x_d_0 => "isect-07",
    isect_08_plane_sphere_n_x_d_0_3 => "isect-08",
    isect_09_plane_sphere_n_x_d_0_6 => "isect-09",
    isect_10_plane_sphere_n_x_d_0_9 => "isect-10",
    isect_11_plane_sphere_n_x_d_0_999 => "isect-11",
    isect_12_plane_sphere_n_x_d_1 => "isect-12",
    isect_13_plane_sphere_n_x_d_1_2 => "isect-13",
    isect_14_plane_sphere_n_diag_d_0 => "isect-14",
    isect_15_plane_sphere_n_diag_d_0_3 => "isect-15",
    isect_16_plane_sphere_n_diag_d_0_6 => "isect-16",
    isect_17_plane_sphere_n_diag_d_0_9 => "isect-17",
    isect_18_plane_sphere_n_diag_d_0_999 => "isect-18",
    isect_19_plane_sphere_n_diag_d_1 => "isect-19",
    isect_20_plane_sphere_n_diag_d_1_2 => "isect-20",
    isect_21_plane_cylinder_perp_z_2 => "isect-21",
    isect_22_plane_cylinder_tilt_30 => "isect-22",
    isect_23_plane_cylinder_tilt_60 => "isect-23",
    isect_24_plane_cylinder_axial_x_0 => "isect-24",
    isect_25_plane_cylinder_axial_x_0_5 => "isect-25",
    isect_26_plane_cylinder_tangent_x_1 => "isect-26",
    isect_27_plane_cylinder_miss_x_1_5 => "isect-27",
    isect_28_plane_cone_perp_z_1 => "isect-28",
    isect_29_plane_cone_perp_z_0_4 => "isect-29",
    isect_30_plane_cone_tilt_10_ellipse => "isect-30",
    isect_31_plane_cone_axial_x_0_lines_and_chords => "isect-31",
    isect_32_sphere_sphere_r2_1_d_1 => "isect-32",
    isect_33_sphere_sphere_r2_1_d_1_5 => "isect-33",
    isect_34_sphere_sphere_r2_1_d_0_5 => "isect-34",
    isect_35_sphere_sphere_r2_1_d_2 => "isect-35",
    isect_36_sphere_sphere_r2_1_d_2_5 => "isect-36",
    isect_37_sphere_sphere_r2_0_5_d_0_5 => "isect-37",
    isect_38_sphere_sphere_r2_0_5_d_0_25 => "isect-38",
    isect_39_sphere_sphere_r2_0_5_d_0 => "isect-39",
    isect_40_cylinder_sphere_coaxial_r_1_5 => "isect-40",
    isect_41_cylinder_sphere_coaxial_r_1 => "isect-41",
    bool_00_union_offset_cubes => "bool-00",
    bool_01_intersection_offset_cubes => "bool-01",
    bool_02_difference_offset_cubes => "bool-02",
    bool_03_union_sharing_a_whole_face => "bool-03",
    bool_04_union_sharing_part_of_a_face => "bool-04",
    bool_05_union_sharing_an_edge => "bool-05",
    bool_06_union_sharing_a_vertex => "bool-06",
    bool_07_union_of_identical_cubes => "bool-07",
    bool_08_difference_of_identical_cubes_empty => "bool-08",
    bool_09_intersection_of_disjoint_cubes_empty => "bool-09",
    bool_10_union_of_disjoint_cubes_two_lumps => "bool-10",
    bool_11_l_by_union_coplanar_top_and_bottom => "bool-11",
    bool_12_wall_with_window_cutter_thicker => "bool-12",
    bool_13_wall_with_window_cutter_flush => "bool-13",
    bool_14_cavity => "bool-14",
    bool_15_lump_floating_in_a_cavity => "bool-15",
    bool_16_pocket_flush_with_the_top => "bool-16",
    bool_17_intersection_with_coplanar_faces => "bool-17",
    bool_18_tiny_boxes_ub_175 => "bool-18",
    bool_19_far_from_the_origin_1e5 => "bool-19",
    bool_20_gap_of_1e_7_two_lumps => "bool-20",
    bool_21_overlap_of_1e_7 => "bool-21",
    bool_22_box_minus_through_cylinder => "bool-22",
    bool_23_box_minus_cylinder_whose_axis_lies_in_a_face => "bool-23",
    bool_24_box_minus_cylinder_tangent_to_a_face => "bool-24",
    bool_25_sphere_intersect_box_octant => "bool-25",
    bool_26_sphere_minus_concentric_sphere => "bool-26",
    bool_27_column_minus_internally_tangent_cylinder_fail_3665 => "bool-27",
    bool_28_box_union_cylinder_with_coplanar_caps => "bool-28",
}
