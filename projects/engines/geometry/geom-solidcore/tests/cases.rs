//! The 120 stored cases, run through geom-solidcore (decision 32.70): one test per case, each
//! holding solidcore's answer to the expected answer in `tests/cases.json`, with no case left out
//! and no check loosened.
//!
//! A case solidcore answers wrongly, or one the kernel's checker or this crate's interface cannot
//! yet judge (an entry marked ours), is a named fault in `tests/named-faults.json`: its test still
//! expects the kernel's answer and so fails, by design, and `kernel.yml`'s job `solidcore` passes
//! only when the tests that fail are exactly the named faults. A named fault that starts to pass,
//! or a case that fails without being named, turns the job red.

use std::str::FromStr;
use std::sync::OnceLock;

use bigdecimal::{BigDecimal, Signed, ToPrimitive};
use geom_check::{Level, check};
use geom_make::{RoundInput, cylinder};
use geom_num::{GridStep, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_solidcore::convert::metres;
use geom_solidcore::{Error, Hole, Modeller, Piece, Profile, Solid};
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

/// The grid step the table was written for, and of the tests that are not cases: 6.25 nm, on
/// which every length of the table but those of seven cases is a whole number of steps.
const STEP: GridStep = GridStep::Q1;

/// The grids, coarsest first, for a case whose lengths are not all whole numbers of [`STEP`].
const GRIDS: [GridStep; 3] = [GridStep::Q0, GridStep::Q1, GridStep::Q2];

/// The length written `x` metres, read as the exact decimal the table writes (serde_json keeps
/// each number's text, and bigdecimal reads it), as a whole number of steps of `step` if it is
/// exactly one on the grid's range: never a length any part of a step away, and never through an
/// f64.
fn whole(x: &str, step: GridStep) -> Option<i64> {
    let metres = BigDecimal::from_str(x).unwrap_or_else(|e| panic!("{x} is not a decimal: {e}"));
    // A step is `quarter_nm` quarter-nanometres, so a metre is 4e9 / quarter_nm steps: 6.4e6,
    // 1.6e8 or 4e9, each a whole number.
    let per_metre = 4_000_000_000_u64
        .checked_div(u64::from(step.quarter_nm()))
        .expect("a step of at least a quarter-nanometre");
    let n = metres * BigDecimal::from(per_metre);
    let k = n.is_integer().then(|| n.to_i64()).flatten()?;
    P3::new(k, 0, 0).is_ok().then_some(k)
}

/// The length written `x` metres as a whole number of steps of `step`; a length that is not one is
/// refused by name, never rounded.
fn steps(x: &str, step: GridStep) -> i64 {
    whole(x, step).unwrap_or_else(|| panic!("{x} m is not a whole number of steps of {step:?}"))
}

/// The decimal text of the table's number `v`.
fn text(v: &Value) -> String {
    match v {
        Value::Number(x) => x.to_string(),
        _ => panic!("not a number: {v}"),
    }
}

/// Every length of `v`: each number under it, but a plane's normal, which is a direction.
fn lengths(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Number(_) => out.push(text(v)),
        Value::Array(xs) => xs.iter().for_each(|x| lengths(x, out)),
        Value::Object(o) => o
            .iter()
            .filter(|(k, _)| k.as_str() != "normal")
            .for_each(|(_, x)| lengths(x, out)),
        _ => {}
    }
}

/// The grid of the case `c` (the grid is the modeller's parameter): [`STEP`], if every length of
/// the case, its operands' and its plane's point, is a whole number of its steps on its range;
/// otherwise the coarsest grid on which every one is. bool-19, a metre from 1e5 m, is past the
/// range of [`STEP`] and runs on Q0. A case with no such grid is run on the finest, where `steps`
/// refuses by name the length that is not whole: nothing is rounded. Those are isect-15 to
/// isect-20, whose plane holds no point of any grid (`diagonal_planes_hold_no_point_of_any_grid`),
/// and bldg-10, whose wall the table puts at 0.20000010000000001 m.
fn grid_of(c: &Value) -> GridStep {
    let mut all = Vec::new();
    lengths(&c["operands"], &mut all);
    if let Some(at) = c.get("plane").map(|p| &p["at"]) {
        lengths(at, &mut all);
    }
    let fits = |g: GridStep| all.iter().all(|x| whole(x, g).is_some());
    if fits(STEP) {
        return STEP;
    }
    GRIDS.into_iter().find(|&g| fits(g)).unwrap_or(GridStep::Q2)
}

fn grid(v: &Value, step: GridStep) -> P3 {
    let [x, y, z] = [&v[0], &v[1], &v[2]].map(|c| steps(&text(c), step));
    P3::new(x, y, z).expect("a point of the grid")
}

fn length(v: &Value, step: GridStep) -> Steps {
    Steps(u64::try_from(steps(&text(v), step)).expect("a length not below zero"))
}

/// A point in grid steps of `step`, in metres.
fn in_metres(q: [f64; 3], step: GridStep) -> [f64; 3] {
    q.map(|c| c * metres(step))
}

fn from_bits(v: &Value) -> Option<f64> {
    v.as_str()
        .map(|s| f64::from_bits(u64::from_str_radix(s, 16).expect("hex bits")))
}

/// Builds an operand: a primitive, an extrusion, or a boolean of two operands.
fn solid(m: &mut Modeller, v: &Value) -> Result<Solid, Error> {
    for op in ["union", "difference", "intersection"] {
        if let Some(pair) = v.get(op) {
            let a = solid(m, &pair[0])?;
            let b = solid(m, &pair[1])?;
            return boolean(m, op, &a, &b);
        }
    }
    let g = m.step();
    let grid = |v: &Value| grid(v, g);
    let length = |v: &Value| length(v, g);
    match v["shape"].as_str().expect("shape") {
        "cuboid" => {
            let size = &v["size"];
            m.cuboid(
                grid(&v["corner"]),
                [length(&size[0]), length(&size[1]), length(&size[2])],
            )
        }
        "sphere" => m.sphere(grid(&v["centre"]), length(&v["radius"])),
        "cylinder" => m.cylinder(grid(&v["base"]), length(&v["radius"]), length(&v["height"])),
        "cone" => m.cone(
            grid(&v["base"]),
            length(&v["base_radius"]),
            length(&v["top_radius"]),
            length(&v["height"]),
        ),
        "extrusion" => {
            let corners = |v: &Value| -> Vec<P3> {
                v.as_array().expect("corners").iter().map(grid).collect()
            };
            let holes = v["holes"]
                .as_array()
                .expect("holes")
                .iter()
                .map(|h| match h.get("circle") {
                    Some(c) => Hole::Circle {
                        centre: grid(&c["centre"]),
                        radius: length(&c["radius"]),
                    },
                    None => Hole::Polygon(corners(&h["polygon"])),
                })
                .collect();
            let by = &v["by"];
            let profile = Profile {
                outer: corners(&v["outer"]),
                holes,
            };
            m.extrude(&profile, [0, 1, 2].map(|i| steps(&text(&by[i]), g)))
        }
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
            let q = in_metres(
                piece.point_at(m, t).expect("a point on the piece"),
                m.step(),
            );
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
    let step = grid_of(c);
    let mut m = Modeller::new(step);
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
                grid(&c["plane"]["at"], step),
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

/// A solid's case: a boolean of two operands, or an operation that makes its one operand
/// (`extrude`), judged by the solid's volume and, where given, its face count.
fn run_boolean(c: &Value) {
    let step = grid_of(c);
    let mut m = Modeller::new(step);
    let r = match c["operation"].as_str().expect("operation") {
        "extrude" => solid(&mut m, &c["operands"][0]).expect("the extrusion"),
        op => {
            let a = solid(&mut m, &c["operands"][0]).expect("first operand");
            let b = solid(&mut m, &c["operands"][1]).expect("second operand");
            boolean(&mut m, op, &a, &b).expect("the boolean")
        }
    };
    let faces = m.face_count(&r);
    if let Some(want) = c.get("faces").and_then(Value::as_u64) {
        assert_eq!(faces as u64, want, "{}: faces", c["name"]);
    }
    let exact = from_bits(&c["volume_bits"]).expect("volume_bits");
    if exact == 0.0 && faces == 0 {
        return;
    }
    let v = m.volume(&r).expect("the volume") * metres(step).powi(3);
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
    let mut m = Modeller::new(STEP);
    let (zero, half, one) = (
        steps("0", STEP),
        steps("0.5", STEP),
        Steps(steps("1", STEP) as u64),
    );
    let corner = |c: i64| P3::new(c, c, c).expect("a grid point");
    let a = m.cuboid(corner(zero), [one; 3]).expect("a");
    let b = m.cuboid(corner(half), [one; 3]).expect("b");
    let (va, vb) = (
        m.volume(&a).expect("a's volume"),
        m.volume(&b).expect("b's"),
    );
    // One cubic metre, in cubic steps: solidcore's 1.0 over the step cubed.
    assert_eq!(va, 1.0 / metres(STEP) / metres(STEP) / metres(STEP));
    let first = m.union(&a, &b).expect("union");
    let second = m.union(&a, &b).expect("union again");
    assert_eq!(m.volume(&first), m.volume(&second));
    assert_eq!(m.volume(&a), Ok(va));
    assert_eq!(m.volume(&b), Ok(vb));
    assert_eq!(a.body().arenas().faces.len(), 6);
}

/// What solidcore returns is the kernel's body, passed by the kernel's checker: a sphere's is one
/// face, a cylinder's three.
#[test]
fn results_are_the_kernels_bodies() {
    let mut m = Modeller::new(STEP);
    let o = P3::new(0, 0, 0).expect("the origin");
    let r = Steps(steps("0.5", STEP) as u64);
    let ball = m.sphere(o, r).expect("a sphere");
    assert_eq!(ball.body().arenas().faces.len(), 1);
    let can = m.cylinder(o, r, r).expect("a cylinder");
    assert_eq!(can.body().arenas().faces.len(), 3);
}

/// The extrusion takes its loops either way round, the way decided exactly from their corners'
/// whole steps: the square with a square hole of bldg-49, its loops given the other way round, or
/// extruded downwards, is the same solid (volume 9 and 10 faces); and before solidcore is reached
/// it refuses an outer polygon of two corners, a vector in the profile's plane and a hole off it.
#[test]
fn extrude_takes_its_loops_either_way_round() {
    let mut m = Modeller::new(STEP);
    let s = |x: &str| steps(x, STEP);
    let p = |x: &str, y: &str, z: &str| P3::new(s(x), s(y), s(z)).expect("a grid point");
    let square = |a: &str, b: &str, z: &str| vec![p(a, a, z), p(b, a, z), p(b, b, z), p(a, b, z)];
    let up = [0, 0, s("3")];
    let given = Profile {
        outer: square("-1", "1", "0"),
        holes: vec![Hole::Polygon(square("-0.5", "0.5", "0"))],
    };
    let mut reversed = given.clone();
    reversed.outer.reverse();
    for hole in &mut reversed.holes {
        if let Hole::Polygon(q) = hole {
            q.reverse();
        }
    }
    let nine = 9.0 / metres(STEP).powi(3);
    for (profile, by) in [(&given, up), (&reversed, up), (&given, up.map(|c| -c))] {
        let r = m.extrude(profile, by).expect("the extrusion");
        assert_eq!(m.face_count(&r), 10);
        let v = m.volume(&r).expect("the volume");
        assert!(
            (v - nine).abs() / nine <= tolerance("volume_relerr_flat"),
            "{v} for {by:?}"
        );
    }
    let invalid = |r: Result<Solid, Error>| matches!(r, Err(Error::Invalid { .. }));
    let two = Profile {
        outer: square("-1", "1", "0")[..2].to_vec(),
        holes: Vec::new(),
    };
    assert!(invalid(m.extrude(&two, up)));
    assert!(invalid(m.extrude(&given, [s("1"), 0, 0])));
    let off = Profile {
        outer: square("-1", "1", "0"),
        holes: vec![Hole::Polygon(square("-0.5", "0.5", "1"))],
    };
    assert!(invalid(m.extrude(&off, up)));
}

/// The named list, `tests/named-faults.json`.
fn named() -> Value {
    serde_json::from_str(include_str!("named-faults.json")).expect("named-faults.json")
}

/// SC-5 stays the kernel's own (decision 32.76): the cylinder of radius 0.5 and height 1 on the
/// origin, made by the kernel's own primitive (geom-make's `cylinder`), passes the kernel's checker
/// at its full level, while the modeller's, made by solidcore, is refused at its top cap; and every
/// SC-5 entry of the named list is marked as staying ours, naming this test and the measured reason
/// (a cap written on the exactly square plane changes 89 of solidcore's own stored answers).
#[test]
fn the_kernel_makes_its_own_cylinder() {
    let o = P3::new(0, 0, 0).expect("the origin");
    let r = Steps(steps("0.5", STEP) as u64);
    let h = Steps(steps("1", STEP) as u64);
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([5; 16])), Budget::UNLIMITED);
    let input = RoundInput {
        base: o,
        radius: r,
        height: h,
        step: STEP,
    };
    let made = cylinder(&ctx, &input).expect("the kernel's cylinder");
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = check(&made.result.body, made.exactness, Level::Full, &mut meter)
        .expect("the checker's report");
    assert!(report.passed(), "{:?}", report.faults());
    assert_eq!(made.result.body.arenas().faces.len(), 3);
    let theirs = Modeller::new(STEP).cylinder(o, r, h);
    assert!(
        matches!(&theirs, Err(Error::CheckFailed { report, .. })
            if report.ends_with("no certified test of this pair exists yet (face 1)")),
        "{theirs:?}"
    );
    for f in named()["faults"].as_array().expect("faults") {
        let sc5 = f["fault"].as_str().is_some_and(|s| s.starts_with("SC-5,"));
        let stays = f.get("stays_ours").and_then(Value::as_str);
        assert_eq!(
            stays.is_some_and(|s| s.contains("the_kernel_makes_its_own_cylinder")
                && s.contains("89 of solidcore's own stored answers")),
            sc5,
            "{}: {stays:?}",
            f["test"]
        );
    }
}

/// SC-7: the tests and stored-answer units of solidcore's own suite that need upstream test files
/// the public repository does not hold are named one by one and counted, and stay named until
/// they run: the list's counts are its lengths, every name is listed once, in order.
#[test]
fn what_needs_upstream_files_is_named_and_counted() {
    let n = named();
    let sc7 = &n["upstream"];
    assert!(
        sc7["fault"]
            .as_str()
            .is_some_and(|s| s.starts_with("SC-7,")),
        "{sc7}"
    );
    for key in ["tests", "units"] {
        let names: Vec<&str> = sc7[key]
            .as_array()
            .unwrap_or_else(|| panic!("SC-7 {key}"))
            .iter()
            .map(|x| x.as_str().expect("a name"))
            .collect();
        assert_eq!(
            names.len() as u64,
            sc7["count"][key].as_u64().expect("a count"),
            "{key}"
        );
        assert!(
            names.windows(2).all(|w| w[0] < w[1]),
            "{key} in order, once"
        );
    }
    assert_eq!(sc7["count"]["units"].as_u64(), Some(132));
    for key in ["blocks", "until"] {
        assert!(sc7[key].as_str().is_some_and(|s| !s.is_empty()), "{key}");
    }
}

/// Why isect-15 to isect-20 run on no grid, by exact arithmetic on the table's decimals: each
/// plane is `x + y + z = 3a`, with `a` the decimal the table writes for every coordinate of its
/// point (the plane's distance `d` along the diagonal, `d / sqrt(3)`, cut at 16 or 17 digits).
/// The plane passes through `(a, a, a)`, but a point of any grid is a point of Q2's (each step a
/// whole number of quarter-nanometres), whose coordinates' sum is `k / 4e9` m for a whole `k`, so
/// the plane holds a point of a grid only if `3a * 4e9` is whole, and it is not. Neither the point
/// the table gives nor any other point of its plane is on a grid, and these cases are WR-1 of
/// tests/named-faults.json.
#[test]
fn diagonal_planes_hold_no_point_of_any_grid() {
    for id in [
        "isect-15", "isect-16", "isect-17", "isect-18", "isect-19", "isect-20",
    ] {
        let c = case(id);
        let at = &c["plane"]["at"];
        assert_eq!(p3(&c["plane"]["normal"]), [1.0; 3], "{id}");
        let a = text(&at[0]);
        assert!(text(&at[1]) == a && text(&at[2]) == a, "{id}");
        let sum =
            BigDecimal::from_str(&a).expect("a decimal") * BigDecimal::from(12_000_000_000_u64);
        assert!(
            sum.is_positive() && !sum.is_integer(),
            "{id}: 3a * 4e9 = {sum}"
        );
        assert!(GRIDS.iter().all(|&g| whole(&a, g).is_none()), "{id}");
    }
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
    assert_eq!(ids.len(), 120);
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

/// Every named fault names a case of the table and its test, or, with no case, a test of this
/// file that is not a case's, with a fault name and what it blocks. A fault of solidcore is
/// named SC; one marked ours, a test the kernel's checker lacks (CK) or a limit of this crate's
/// interface (WR), says so in its name, and no test is named twice.
#[test]
fn named_faults_name_their_cases() {
    let faults: Value =
        serde_json::from_str(include_str!("named-faults.json")).expect("named-faults.json");
    let names = include_str!("case_names.txt");
    let source = include_str!("cases.rs");
    let mut tests = Vec::new();
    for f in faults["faults"].as_array().expect("faults") {
        let test = f["test"].as_str().expect("test");
        let id = f["case"].as_str().unwrap_or("none");
        if f["case"].is_null() {
            assert!(source.contains(&format!("\nfn {test}() {{")), "{test}");
            assert!(
                !names.lines().any(|l| l.ends_with(&format!(" {test}"))),
                "{test}"
            );
        } else {
            assert!(
                names.lines().any(|l| l == format!("{id} {test}")),
                "{id} {test}"
            );
        }
        let fault = f["fault"].as_str().unwrap_or("");
        if f["ours"].as_bool() == Some(true) {
            assert!(
                ["CK-", "WR-"].iter().any(|p| fault.starts_with(p)) && fault.contains("(ours: "),
                "{id}: {fault}"
            );
        } else {
            assert!(f.get("ours").is_none() && fault.starts_with("SC-"), "{id}");
        }
        assert!(!tests.contains(&test), "{test} is named twice");
        tests.push(test);
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
    bldg_00_l_corner_butt_and_overlap => "bldg-00",
    bldg_01_t_overlapping => "bldg-01",
    bldg_02_t_wall_ending_flush_on_the_other_s_face => "bldg-02",
    bldg_03_cross => "bldg-03",
    bldg_04_mitred_corner_at_30_degrees => "bldg-04",
    bldg_05_mitred_corner_at_45_degrees => "bldg-05",
    bldg_06_mitred_corner_at_60_degrees => "bldg-06",
    bldg_07_mitred_corner_at_90_degrees => "bldg-07",
    bldg_08_walls_of_different_thickness_and_height => "bldg-08",
    bldg_09_two_walls_in_line_end_to_end => "bldg-09",
    bldg_10_wall_1e_7_away_from_another_two_solids => "bldg-10",
    bldg_11_wall_1e_4_away_from_another_two_solids => "bldg-11",
    bldg_12_curved_wall_alone_cylindrical_box => "bldg-12",
    bldg_14_curved_wall_meeting_a_straight_wall_flush_at_its_end => "bldg-14",
    bldg_15_curved_wall_overlapping_a_straight_wall => "bldg-15",
    bldg_16_window_through_a_straight_wall => "bldg-16",
    bldg_17_door_void_running_below_the_floor => "bldg-17",
    bldg_18_door_void_flush_with_the_wall_bottom => "bldg-18",
    bldg_19_window_and_door_in_one_wall => "bldg-19",
    bldg_20_niche_void_flush_with_one_face_only => "bldg-20",
    bldg_21_window_leaving_a_1e_3_sliver_of_head => "bldg-21",
    bldg_22_window_leaving_a_1e_6_sliver_of_head => "bldg-22",
    bldg_24_rectangular_opening_through_a_curved_wall => "bldg-24",
    bldg_25_round_opening_through_a_wall => "bldg-25",
    bldg_26_round_opening_flush_with_both_wall_faces => "bldg-26",
    bldg_27_slab_with_a_rectangular_opening => "bldg-27",
    bldg_28_slab_with_a_round_opening => "bldg-28",
    bldg_29_slab_with_a_rectangular_and_a_round_opening => "bldg-29",
    bldg_30_slab_cut_right_through_by_a_wall_two_pieces => "bldg-30",
    bldg_31_slab_notched_by_a_wall_along_its_edge => "bldg-31",
    bldg_32_sloped_slab => "bldg-32",
    bldg_33_sloped_slab_with_a_vertical_round_opening => "bldg-33",
    bldg_34_slab_edge_shared_with_a_wall_face => "bldg-34",
    bldg_35_slab_sitting_on_a_wall_top => "bldg-35",
    bldg_36_two_roof_planes_meeting_at_a_ridge_solid_halves => "bldg-36",
    bldg_37_two_thick_roof_planes_meeting_at_a_ridge => "bldg-37",
    bldg_38_hip_roof => "bldg-38",
    bldg_39_valley_two_gables_meeting_in_a_t => "bldg-39",
    bldg_40_gable_end_wall_cut_by_two_roof_planes => "bldg-40",
    bldg_41_eave_wall_cut_by_a_shed_roof_plane => "bldg-41",
    bldg_42_dormer_through_a_roof_plane => "bldg-42",
    bldg_43_extrusion_of_a_rectangle => "bldg-43",
    bldg_44_extrusion_of_a_rectangle_as_a_void_cutting_a_box => "bldg-44",
    bldg_45_extrusion_of_an_l => "bldg-45",
    bldg_46_extrusion_of_an_l_as_a_void_cutting_a_box => "bldg-46",
    bldg_47_extrusion_of_a_square_with_a_round_hole => "bldg-47",
    bldg_48_extrusion_of_a_square_with_a_round_hole_as_a_void_cutting_a_box => "bldg-48",
    bldg_49_extrusion_of_a_square_with_a_square_hole => "bldg-49",
    bldg_50_extrusion_of_a_square_with_a_square_hole_as_a_void_cutting_a_box => "bldg-50",
}
