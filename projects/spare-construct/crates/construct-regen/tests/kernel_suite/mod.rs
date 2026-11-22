//! The kernel builder's suite: what Spare Construct asks of a shape built through the kernel's
//! engine seam, written over `engine_geometry`'s `Engine` trait and Construct's own `Geometry`
//! alone, so that any engine is judged by the same cases. A test file runs an engine through it by
//! calling each case with a way to make a fresh engine: this crate's `tests/kernel.rs` with a test
//! engine of its own, and the server's `tests/kernel.rs` (through `#[path]`) with the engine the
//! server links. An exact engine of the kernel's own runs the same file on the day it arrives.
//!
//! The cases: a wall of 4000 by 200 by 2700 mm along x and along y, a door of 900 by 300 by 2100 mm
//! and a block of 600 by 400 by 900 mm, each a recipe as the page and the seed write them.

#![allow(
    dead_code,
    reason = "each test file that includes the suite runs the cases it needs"
)]

use construct_model::geometry::{Built, Geometry, Recipe};
use construct_regen::kernel::{BLOCK_ROLES, ENGINE, Kernel, WALL_ROLES};
use engine_geometry::value::engine::Engine;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The persistent names of a straight wall's faces, by role, under `NameScheme v1`. Recorded:
/// a door hosted on a wall keeps its face only while these stay, so a change here is a new name
/// scheme, never an edit.
pub const WALL_NAMES: [(&str, &str); 6] = [
    ("base", "3e6bee217707ae9f30c5c0dae6f4cc2d"),
    ("end0", "e89b61a65a8238c369f27cf366582da2"),
    ("end1", "9dae105c2b0bc31fa83ec5ad954282f1"),
    ("exterior", "ea1ef69aaedba037dfde6e34608c45c1"),
    ("interior", "e0333d84c96c08f1e8fcde5b33a12518"),
    ("top", "927713314fdadf1dfdd625a7a73e8d1b"),
];

pub fn recipe(op: &str, args: Value) -> Recipe {
    Recipe::construct(op, serde_json::from_value(args).unwrap())
}

pub fn wall(length: i64, axis: &str) -> Recipe {
    recipe(
        "wall.straight",
        json!({ "length": length, "thickness": 200, "height": 2700, "axis": axis }),
    )
}

pub fn door() -> Recipe {
    recipe(
        "door.single",
        json!({ "width": 900, "thickness": 300, "height": 2100 }),
    )
}

pub fn block() -> Recipe {
    recipe(
        "block",
        json!({ "width": 600, "depth": 400, "height": 900 }),
    )
}

fn kernel<E: Engine + Send + 'static>(engine: E) -> Kernel<E> {
    Kernel::new(engine).expect("a millimetre is whole steps of the engine's grid")
}

fn built<E: Engine + Send + 'static>(k: &Kernel<E>, recipe: &Recipe) -> Built {
    k.build(recipe)
        .unwrap_or_else(|e| panic!("{} builds: {e}", recipe.op))
}

/// The roles of a built shape's names, and its names by role.
fn by_role(b: &Built) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for n in &b.names {
        assert!(
            found.insert(n.role.clone(), n.name.clone()).is_none(),
            "two faces in the role {}",
            n.role
        );
    }
    found
}

/// A straight wall is its plan extruded by its height: its bounds are the box the stand-in gives,
/// along x and along y, and its outline is its plan.
pub fn a_straight_wall_is_its_plan_extruded<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let along_x = built(&k, &wall(4000, "x"));
    assert_eq!(along_x.bounds.min, [0, 0, 0]);
    assert_eq!(along_x.bounds.max, [4000, 200, 2700]);
    assert_eq!(
        along_x.displays[0].outline,
        vec![[0, 0], [4000, 0], [4000, 200], [0, 200]]
    );
    let along_y = built(&k, &wall(4000, "y"));
    assert_eq!(along_y.bounds.min, [0, 0, 0]);
    assert_eq!(along_y.bounds.max, [200, 4000, 2700]);
    let stand_in = construct_regen::geometry();
    for r in [wall(4000, "x"), wall(4000, "y"), door(), block()] {
        let boxed = stand_in.build(&r).unwrap();
        assert_eq!(built(&k, &r).bounds, boxed.bounds, "{}", r.op);
    }
}

/// Checks that `b`'s finest mesh is closed (with its points joined where they are equal, every
/// edge is used by two triangles, once each way, and no triangle has two corners at one point)
/// and returns six times the volume it holds, exactly, in cubic millimetres.
#[allow(
    clippy::many_single_char_names,
    reason = "the corners and edges of a triangle, as the formulas name them"
)]
pub fn closed_six_volume(b: &Built) -> i128 {
    let mesh = &b.displays[0].mesh;
    assert_eq!(mesh.positions.len() % 3, 0);
    assert_eq!(mesh.indices.len() % 3, 0);
    assert_ne!(mesh.indices, [] as [u32; 0]);
    let point = |i: u32| -> [i64; 3] {
        let i = usize::try_from(i).unwrap() * 3;
        [
            mesh.positions[i],
            mesh.positions[i + 1],
            mesh.positions[i + 2],
        ]
    };
    let mut uses: BTreeMap<([i64; 3], [i64; 3]), (u32, i32)> = BTreeMap::new();
    let mut six = 0_i128;
    for t in mesh.indices.as_chunks::<3>().0 {
        let [p, q, r] = [point(t[0]), point(t[1]), point(t[2])];
        for (u, v) in [(p, q), (q, r), (r, p)] {
            assert_ne!(u, v, "a triangle with two corners at one point");
            let entry = uses.entry((u.min(v), u.max(v))).or_default();
            entry.0 += 1;
            entry.1 += if u < v { 1 } else { -1 };
        }
        let w = |a: [i64; 3]| a.map(i128::from);
        let (p, q, r) = (w(p), w(q), w(r));
        let cross = [
            q[1] * r[2] - q[2] * r[1],
            q[2] * r[0] - q[0] * r[2],
            q[0] * r[1] - q[1] * r[0],
        ];
        six += p[0] * cross[0] + p[1] * cross[1] + p[2] * cross[2];
    }
    assert!(
        uses.values().all(|&(n, _)| n == 2),
        "an edge not used by two triangles"
    );
    assert!(
        uses.values().all(|&(_, w)| w == 0),
        "an edge used twice the same way"
    );
    six
}

/// The engine's largest length tolerance, in millimetres, from the edition the builder recorded.
fn tolerance_mm<E: Engine + Send + 'static>(k: &Kernel<E>) -> f64 {
    let report = k.edition().report.expect("the engine's report");
    report
        .tolerances
        .iter()
        .filter(|t| t.unit == "m")
        .map(|t| t.value.parse::<f64>().expect("a decimal") * 1000.0)
        .fold(0.0, f64::max)
}

/// A wall built through the kernel holds the volume of its box, within the engine's stated
/// tolerance over its surface, and its mesh is closed; so do the door and the block.
#[allow(
    clippy::cast_precision_loss,
    reason = "a volume's difference set against a tolerance, both far under 2^53"
)]
pub fn the_wall_has_its_boxs_volume_and_a_closed_mesh<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let k = kernel(make());
    let tolerance = tolerance_mm(&k);
    for (r, [a, b, c]) in [
        (wall(4000, "x"), [4000_i128, 200, 2700]),
        (wall(4000, "y"), [200, 4000, 2700]),
        (door(), [900, 300, 2100]),
        (block(), [600, 400, 900]),
    ] {
        let six = closed_six_volume(&built(&k, &r));
        let expected = 6 * a * b * c;
        let surface = 2 * (a * b + b * c + a * c);
        let allowed = 6.0 * surface as f64 * tolerance;
        assert!(
            ((six - expected).abs() as f64) <= allowed,
            "{}: six times the volume is {six}, not {expected} within {allowed}",
            r.op
        );
    }
}

/// Every face of a wall carries a persistent role name: the two sides, the top, the base and the
/// two ends, each once. The names are the recorded ones, and they stay when the wall is rebuilt,
/// lengthened or turned, so a door hosted on its exterior keeps its face. A door has a wall's
/// roles and a block an extrusion's, under names of their own operations.
pub fn every_face_carries_a_persistent_role_name<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let short = by_role(&built(&k, &wall(3000, "x")));
    let roles: BTreeSet<&str> = short.keys().map(String::as_str).collect();
    assert_eq!(roles, WALL_ROLES.into_iter().collect());
    let names: BTreeSet<&String> = short.values().collect();
    assert_eq!(names.len(), 6, "two faces share a name");
    let pinned: BTreeMap<String, String> = WALL_NAMES
        .iter()
        .map(|&(r, n)| (r.to_owned(), n.to_owned()))
        .collect();
    assert_eq!(short, pinned);
    for again in [wall(3000, "x"), wall(5000, "x"), wall(5000, "y")] {
        assert_eq!(by_role(&built(&k, &again)), short);
    }
    let again = kernel(make());
    assert_eq!(by_role(&built(&again, &wall(3000, "x"))), short);

    let door = by_role(&built(&k, &door()));
    let roles: BTreeSet<&str> = door.keys().map(String::as_str).collect();
    assert_eq!(roles, WALL_ROLES.into_iter().collect());
    assert_ne!(door["exterior"], short["exterior"]);
    let block = by_role(&built(&k, &block()));
    let roles: BTreeSet<&str> = block.keys().map(String::as_str).collect();
    assert_eq!(roles, BLOCK_ROLES.into_iter().collect());
}

/// The same model built twice, by two fresh builders, gives the same bytes: bodies, bounds,
/// names and every display record.
pub fn the_same_model_built_twice_gives_the_same_bytes<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let model = [wall(4000, "x"), wall(2500, "y"), door(), block()];
    let bytes = || {
        let k = kernel(make());
        let mut out = Vec::new();
        for r in &model {
            let b = built(&k, r);
            out.extend(b.body_id().into_bytes());
            out.extend(serde_json::to_vec(&b.bounds).unwrap());
            out.extend(serde_json::to_vec(&b.names).unwrap());
            for d in &b.displays {
                out.extend(d.bytes());
            }
        }
        out
    };
    let first = bytes();
    assert_ne!(first, [] as [u8; 0]);
    assert_eq!(first, bytes());
    let k = kernel(make());
    let b = built(&k, &model[0]);
    assert_ne!(
        b.displays[0].id(),
        b.displays[1].id(),
        "each level of detail is a record of its own"
    );
}

/// The edition is this builder's, with the engine's own report in it: its name, its version, its
/// grid step, its tolerances and which part answers each operation, word for word.
pub fn the_edition_carries_the_engines_report<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let said = make().edition();
    let k = kernel(make());
    let edition = k.edition();
    assert_eq!(edition.engine, ENGINE);
    assert_eq!(edition.name_scheme, 1);
    let report = edition.report.clone().expect("the engine's report");
    assert_eq!(report.engine, said.engine);
    assert_eq!(report.version, said.version);
    assert_eq!(report.steps_per_mm, k.units().steps_per_mm());
    assert_eq!(report.tolerances.len(), said.tolerances.len());
    for (kept, t) in report.tolerances.iter().zip(&said.tolerances) {
        assert_eq!(kept.name, t.name);
        assert_eq!(kept.unit, t.unit);
        assert_eq!(
            kept.value.parse::<f64>().unwrap().to_bits(),
            t.value.to_bits()
        );
    }
    let answers: Vec<(&str, &str, &str)> = report
        .answers
        .iter()
        .map(|a| (a.operation.as_str(), a.case.as_str(), a.by.as_str()))
        .collect();
    let given: Vec<(&str, &str, &str)> = said
        .answers
        .iter()
        .map(|a| (a.operation.as_str(), a.case.as_str(), a.by.as_str()))
        .collect();
    assert_eq!(answers, given);
    assert_ne!(edition, construct_regen::geometry().edition());
    assert_eq!(edition, kernel(make()).edition());
}

/// A recipe that can't build says why, with the codes the stand-in gives for the same recipe.
pub fn a_recipe_that_cant_build_says_why<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let stand_in = construct_regen::geometry();
    for bad in [
        recipe("stair.spiral", json!({})),
        recipe("block", json!({ "width": 1 })),
        recipe("block", json!({ "width": 0, "depth": 1, "height": 1 })),
        recipe(
            "wall.straight",
            json!({ "length": 1, "thickness": "2", "height": 1 }),
        ),
        recipe(
            "door.single",
            json!({ "width": 1, "thickness": 1, "height": -1 }),
        ),
    ] {
        let code = k.build(&bad).unwrap_err().code;
        assert_eq!(code, stand_in.build(&bad).unwrap_err().code, "{}", bad.op);
    }
    let huge = recipe(
        "block",
        json!({ "width": 200_000_000, "depth": 1, "height": 1 }),
    );
    assert_eq!(k.build(&huge).unwrap_err().code, "TooBig");
}
