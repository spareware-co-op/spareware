//! The kernel builder's suite: what Spare Construct asks of a shape built through the kernel's
//! engine seam, written over `engine_geometry`'s `Engine` trait and Construct's own `Geometry`
//! alone, so that any engine is judged by the same cases. A test file runs an engine through it by
//! calling each case with a way to make a fresh engine: this crate's `tests/kernel.rs` with a test
//! engine of its own, and the server's `tests/kernel.rs` (through `#[path]`) with the engine the
//! server links. An exact engine of the kernel's own runs the same file on the day it arrives.
//!
//! The cases: a wall of 4000 by 200 by 2700 mm along x and along y, a door of 900 by 300 by 2100 mm
//! and a block of 600 by 400 by 900 mm, each a recipe as the page and the seed write them; and the
//! wall with doors hosted in it, centred on its thickness as the seed places them (50 mm proud of
//! each side): one door to the floor, a raised opening, two doors, and the cases at the edge. The
//! cases that cut need an engine that subtracts; the refusals and the names do not reach the
//! engine's subtraction, so a test engine that only extrudes runs them too.
//!
//! Placed families (job 6.4): the library's single-swing door (`d900x2100`) and fixed window
//! (`w1200x1200`) in a three-layer wall of whole-millimetre faces, each layer losing exactly the
//! openings' volume, each family's solids exactly theirs and inside their opening; the opening
//! moving with its wall and resized with its type; and every refusal of a family that does not
//! fit, in the builder's words, the same from `fits`.

#![allow(
    dead_code,
    reason = "each test file that includes the suite runs the cases it needs"
)]

use construct_model::geometry::{Built, Geometry, Mesh, Recipe};
use construct_regen::kernel::{BLOCK_ROLES, ENGINE, Hosted, Kernel, WALL_ROLES};
use engine_geometry::value::engine::Engine;
use engine_geometry::value::grid::P3;
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
    closed_mesh_six_volume(&b.displays[0].mesh)
}

/// [`closed_six_volume`] of any one mesh.
#[allow(
    clippy::many_single_char_names,
    reason = "the corners and edges of a triangle, as the formulas name them"
)]
pub fn closed_mesh_six_volume(mesh: &Mesh) -> i128 {
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

/// A grid line and a reference plane are datums, with no solid: the kernel's builder hands them to
/// the one datum builder, so each is built the same as by the stand-in, bytes included, and an
/// engine is never asked for one.
pub fn a_datum_is_built_as_the_stand_in_builds_it<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let k = kernel(make());
    let stand_in = construct_regen::geometry();
    let text = |x: &str| json!(format!("length:q0:{x}"));
    for datum in [
        recipe(
            "grid.line",
            json!({ "x0": text("0"), "y0": text("0"), "x1": text("38400000"), "y1": text("0") }),
        ),
        recipe(
            "grid.arc",
            json!({ "cx": text("0"), "cy": text("0"), "radius": text("6400000"),
                    "from": "angle:0", "to": "angle:810000000" }),
        ),
        recipe("refplane.horizontal", json!({ "z": text("-1920000") })),
    ] {
        let here = k
            .build(&datum)
            .unwrap_or_else(|e| panic!("{}: {e}", datum.op));
        let there = stand_in.build(&datum).unwrap();
        assert_eq!(here, there, "{}", datum.op);
        assert_eq!(here.body_id(), there.body_id());
    }
    // One that is not as written is a failure, with the same code from both.
    let bad = recipe("grid.line", json!({ "x0": text("0") }));
    assert_eq!(k.build(&bad).unwrap_err().code, "BadDatum");
    assert_eq!(stand_in.build(&bad).unwrap_err().code, "BadDatum");
}

/// The persistent names of the faces door `D-1`'s opening makes in a straight wall, by role, under
/// `NameScheme v1`: `Created { op: the wall's, role: tool(D-1), from: [the opening's own face] }`.
/// Recorded: a reference to a reveal lands only while these stay, so a change here is a new name
/// scheme, never an edit.
pub const D1_NAMES: [(&str, &str); 4] = [
    ("tool(D-1).head", "d3d64ce857a2de69a72d29d028087078"),
    ("tool(D-1).jamb.end", "b6350eac010b7c426896ceb28b0e4eb0"),
    ("tool(D-1).jamb.start", "c604d634f5e0b6b6368855ff880da0ec"),
    ("tool(D-1).sill", "3bb75a32a59ed9793f97b8538b6183ff"),
];

/// A door of `width` by 300 by `height` mm, along `axis`.
pub fn door_of(width: i64, height: i64, axis: &str) -> Recipe {
    recipe(
        "door.single",
        json!({ "width": width, "thickness": 300, "height": height, "axis": axis }),
    )
}

/// The grid point `mm` millimetres from the wall's origin.
fn at<E: Engine + Send + 'static>(k: &Kernel<E>, mm: [i64; 3]) -> P3 {
    k.units().point(mm).unwrap()
}

/// The grid point `mm` millimetres and `steps` more steps along each axis from the wall's origin.
fn at_plus<E: Engine + Send + 'static>(k: &Kernel<E>, mm: [i64; 3], steps: [i64; 3]) -> P3 {
    let per = i64::try_from(k.units().steps_per_mm()).unwrap();
    P3::new(
        mm[0] * per + steps[0],
        mm[1] * per + steps[1],
        mm[2] * per + steps[2],
    )
    .unwrap()
}

fn hosted<'a>(key: &'a str, recipe: &'a Recipe, at: P3) -> Hosted<'a> {
    Hosted { key, recipe, at }
}

fn hosting<E: Engine + Send + 'static>(
    k: &Kernel<E>,
    wall: &Recipe,
    doors: &[Hosted<'_>],
) -> Built {
    k.build_hosting(wall, doors)
        .unwrap_or_else(|e| panic!("the wall with its doors builds: {e}"))
}

/// Checks that `mesh` is closed and that six times its volume is `expected` (six times cubic
/// millimetres), within the engine's stated tolerance over a surface of `surface` square
/// millimetres.
#[allow(
    clippy::cast_precision_loss,
    reason = "a volume's difference set against a tolerance, both far under 2^53"
)]
fn holds<E: Engine + Send + 'static>(k: &Kernel<E>, mesh: &Mesh, expected: i128, surface: i128) {
    let six = closed_mesh_six_volume(mesh);
    let allowed = 6.0 * surface as f64 * tolerance_mm(k);
    assert!(
        ((six - expected).abs() as f64) <= allowed,
        "six times the volume is {six}, not {expected} within {allowed}"
    );
}

/// Six times the volume of a box, in cubic millimetres, and its surface.
fn box_of([a, b, c]: [i128; 3]) -> (i128, i128) {
    (6 * a * b * c, 2 * (a * b + b * c + a * c))
}

/// A door hosted in a wall cuts a real opening: the wall's finest mesh is closed and holds the
/// wall less the opening, the door's width and height through the wall's thickness, whatever the
/// door's own thickness; its bounds and outline are the wall's. A door to the floor (the common
/// case) and a raised opening wholly inside the wall both build, along x and along y. The coarse
/// level is the wall before its openings were cut, closed, straying by at most half the opening's
/// width or half the wall's thickness.
pub fn a_door_cuts_a_real_opening<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let (whole, surface) = box_of([4000, 200, 2700]);
    for axis in ["x", "y"] {
        let wall = wall(4000, axis);
        let plain = built(&k, &wall);
        let place = |along: i64, up: i64| {
            if axis == "y" {
                at(&k, [-50, along, up])
            } else {
                at(&k, [along, -50, up])
            }
        };
        let to_floor = door_of(900, 2100, axis);
        let raised = door_of(800, 1000, axis);
        for (door, up, [w, h]) in [(&to_floor, 0, [900, 2100]), (&raised, 900, [800, 1000])] {
            let cut = hosting(&k, &wall, &[hosted("D-1", door, place(1000, up))]);
            assert_eq!(cut.bounds, plain.bounds, "{axis}: the wall's bounds");
            assert_eq!(cut.displays.len(), 2);
            assert_eq!(cut.displays[0].lod, 0);
            assert_eq!(cut.displays[0].deviation, 0);
            assert_eq!(cut.displays[0].outline, plain.displays[0].outline);
            let reveals = 2 * 200 * (h + w);
            holds(
                &k,
                &cut.displays[0].mesh,
                whole - 6 * w * 200 * h,
                surface + reveals,
            );
            assert_eq!(cut.displays[1].lod, 1);
            assert_eq!(
                cut.displays[1].mesh, plain.displays[1].mesh,
                "{axis}: the coarse level"
            );
            holds(&k, &cut.displays[1].mesh, whole, surface);
            assert_eq!(
                i128::from(cut.displays[1].deviation),
                (w / 2).max(100),
                "{axis}"
            );
            assert_ne!(cut.body_id(), plain.body_id());
        }
    }
}

/// A wall with two doors cuts both, one after another in the order of their keys, so the same
/// model gives the same bytes in whatever order the doors are given, and on a fresh builder: the
/// body, the bounds, the names and every display record. The body records each cutter by its
/// door's key, in that order; a wall with no doors writes none.
pub fn a_wall_with_two_doors_cuts_both_in_the_order_of_their_keys<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let k = kernel(make());
    let wall = wall(4000, "x");
    let door = door();
    let (one, two) = (at(&k, [500, -50, 0]), at(&k, [2500, -50, 0]));
    let bytes = |b: &Built| {
        let mut out = b.body.clone();
        out.extend(serde_json::to_vec(&b.bounds).unwrap());
        out.extend(serde_json::to_vec(&b.names).unwrap());
        for d in &b.displays {
            out.extend(d.bytes());
        }
        out
    };
    let forward = hosting(
        &k,
        &wall,
        &[hosted("D-1", &door, one), hosted("D-2", &door, two)],
    );
    let backward = hosting(
        &k,
        &wall,
        &[hosted("D-2", &door, two), hosted("D-1", &door, one)],
    );
    assert_eq!(bytes(&forward), bytes(&backward));
    let fresh = kernel(make());
    let again = hosting(
        &fresh,
        &wall,
        &[hosted("D-1", &door, one), hosted("D-2", &door, two)],
    );
    assert_eq!(bytes(&forward), bytes(&again));
    let (whole, surface) = box_of([4000, 200, 2700]);
    holds(
        &k,
        &forward.displays[0].mesh,
        whole - 2 * 6 * 900 * 200 * 2100,
        surface + 2 * 2 * 200 * (900 + 2100),
    );
    let body: Value = serde_json::from_slice(&forward.body).unwrap();
    let keys: Vec<&str> = body["subtract"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["D-1", "D-2"]);
    let plain: Value = serde_json::from_slice(&built(&k, &wall).body).unwrap();
    assert!(
        plain.get("subtract").is_none(),
        "a wall without openings keeps its bytes"
    );
}

/// A cut wall keeps its six names, the recorded ones, and the faces each opening makes are named
/// by its door: its jambs and head, and its sill when the opening stands above the base, the
/// recorded names for door `D-1`. They stay when the wall is lengthened or turned and when another
/// door is added; two doors name their faces apart. The names are the builder's own, so this runs
/// on any engine whose subtraction answers, whatever shape it gives.
pub fn the_walls_names_survive_its_openings<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let door = door();
    let raised = door_of(800, 1000, "x");
    let pinned = |names: &[(&str, &str)]| -> BTreeMap<String, String> {
        names
            .iter()
            .map(|&(r, n)| (r.to_owned(), n.to_owned()))
            .collect()
    };
    let (walls, d1) = (pinned(&WALL_NAMES), pinned(&D1_NAMES));
    let floor = by_role(&hosting(
        &k,
        &wall(4000, "x"),
        &[hosted("D-1", &door, at(&k, [1000, -50, 0]))],
    ));
    let mut expected = walls.clone();
    expected.extend(d1.clone().into_iter().filter(|(r, _)| !r.ends_with("sill")));
    assert_eq!(
        floor, expected,
        "a door to the floor: the wall's six and its jambs and head"
    );
    let up = by_role(&hosting(
        &k,
        &wall(4000, "x"),
        &[hosted("D-1", &raised, at(&k, [1000, -50, 900]))],
    ));
    let mut expected = walls;
    expected.extend(d1);
    assert_eq!(up, expected, "a raised opening: its sill too");
    let longer = by_role(&hosting(
        &k,
        &wall(5000, "x"),
        &[hosted("D-1", &door, at(&k, [1000, -50, 0]))],
    ));
    assert_eq!(longer, floor, "lengthened");
    let turned = by_role(&hosting(
        &k,
        &wall(4000, "y"),
        &[hosted(
            "D-1",
            &door_of(900, 2100, "y"),
            at(&k, [-50, 1000, 0]),
        )],
    ));
    assert_eq!(turned, floor, "turned");
    let two = by_role(&hosting(
        &k,
        &wall(4000, "x"),
        &[
            hosted("D-1", &door, at(&k, [1000, -50, 0])),
            hosted("D-2", &door, at(&k, [2500, -50, 0])),
        ],
    ));
    for (role, name) in &floor {
        assert_eq!(two.get(role), Some(name), "{role} after another door");
    }
    let theirs: BTreeSet<&String> = two
        .iter()
        .filter(|(r, _)| r.starts_with("tool(D-2)."))
        .map(|(_, n)| n)
        .collect();
    assert_eq!(theirs.len(), 3);
    let names: BTreeSet<&String> = two.values().collect();
    assert_eq!(names.len(), two.len(), "two faces share a name");
}

/// The cases at the edge that are not built are refused before the engine is asked, each with a
/// code for its reason and a message that names the door: an opening touching or crossing either
/// end, as tall as the wall or taller, reaching below its base, two openings that overlap or
/// touch, an opening with a face within the engine's tolerance of a wall's face or of another
/// opening's without being on it (named fault SC-4's case), one across its wall or beside it, one
/// key twice, a host that is not a wall, a hosted element that is not a door, a door that can't
/// build, and corners that are not whole millimetres. Nothing panics.
#[allow(clippy::too_many_lines, reason = "one table of cases, each a line")]
pub fn an_opening_at_the_edge_is_refused_with_its_door_and_reason<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    type Case<'a> = (&'a str, &'a Recipe, Vec<Hosted<'a>>, &'a str, Vec<&'a str>);
    let k = kernel(make());
    let wall = wall(4000, "x");
    let door = door();
    let full = door_of(900, 2700, "x");
    let taller = door_of(900, 2800, "x");
    let across = door_of(900, 2100, "y");
    let block = block();
    let bad = recipe("door.single", json!({ "thickness": 300, "height": 2100 }));
    let mm = |x: i64, z: i64| at(&k, [x, -50, z]);
    let step = |x: i64, z: i64, steps: [i64; 3]| at_plus(&k, [x, -50, z], steps);
    let one = |key, recipe, at| vec![hosted(key, recipe, at)];
    let cases: Vec<Case<'_>> = vec![
        (
            "touching end0",
            &wall,
            one("D-1", &door, mm(0, 0)),
            "OpeningAtEnd",
            vec!["D-1"],
        ),
        (
            "touching end1",
            &wall,
            one("D-1", &door, mm(3100, 0)),
            "OpeningAtEnd",
            vec!["D-1"],
        ),
        (
            "crossing end0",
            &wall,
            one("D-1", &door, mm(-100, 0)),
            "OpeningPastEnd",
            vec!["D-1"],
        ),
        (
            "crossing end1",
            &wall,
            one("D-1", &door, mm(3500, 0)),
            "OpeningPastEnd",
            vec!["D-1"],
        ),
        (
            "as tall as the wall",
            &wall,
            one("D-1", &full, mm(1000, 0)),
            "OpeningTooTall",
            vec!["D-1"],
        ),
        (
            "taller than the wall",
            &wall,
            one("D-1", &taller, mm(1000, 0)),
            "OpeningTooTall",
            vec!["D-1"],
        ),
        (
            "its head at the top",
            &wall,
            one("D-1", &door, mm(1000, 600)),
            "OpeningTooTall",
            vec!["D-1"],
        ),
        (
            "below the base",
            &wall,
            one("D-1", &door, mm(1000, -10)),
            "OpeningBelowBase",
            vec!["D-1"],
        ),
        (
            "overlapping",
            &wall,
            vec![
                hosted("D-2", &door, mm(1500, 0)),
                hosted("D-1", &door, mm(1000, 0)),
            ],
            "OpeningsOverlap",
            vec!["D-1", "D-2", "overlap"],
        ),
        (
            "touching",
            &wall,
            vec![
                hosted("D-1", &door, mm(1000, 0)),
                hosted("D-2", &door, mm(1900, 0)),
            ],
            "OpeningsOverlap",
            vec!["D-1", "D-2", "touch"],
        ),
        (
            "head a step under the top",
            &wall,
            one("D-1", &door, step(1000, 600, [0, 0, -1])),
            "OpeningNearFace",
            vec!["D-1", "head", "top"],
        ),
        (
            "sill a step over the base",
            &wall,
            one("D-1", &door, step(1000, 0, [0, 0, 1])),
            "OpeningNearFace",
            vec!["D-1", "sill", "base"],
        ),
        (
            "jamb a step from end0",
            &wall,
            one("D-1", &door, step(0, 0, [1, 0, 0])),
            "OpeningNearFace",
            vec!["D-1", "jamb.start", "end0"],
        ),
        (
            "jamb a step past end1",
            &wall,
            one("D-1", &door, step(3100, 0, [1, 0, 0])),
            "OpeningNearFace",
            vec!["D-1", "jamb.end", "end1"],
        ),
        (
            "two openings a step apart",
            &wall,
            vec![
                hosted("D-1", &door, mm(1000, 0)),
                hosted("D-2", &door, step(1900, 0, [1, 0, 0])),
            ],
            "OpeningNearFace",
            vec!["D-1", "D-2"],
        ),
        (
            "half a millimetre off",
            &wall,
            one("D-1", &door, step(1000, 0, [3200, 0, 0])),
            "NotWholeMillimetres",
            vec!["D-1"],
        ),
        (
            "across its wall",
            &wall,
            one("D-1", &across, mm(1000, 0)),
            "OpeningAcrossWall",
            vec!["D-1"],
        ),
        (
            "beside its wall",
            &wall,
            one("D-1", &door, at(&k, [1000, 500, 0])),
            "OpeningNotInWall",
            vec!["D-1"],
        ),
        (
            "one key twice",
            &wall,
            vec![
                hosted("D-1", &door, mm(500, 0)),
                hosted("D-1", &door, mm(2500, 0)),
            ],
            "DuplicateOpening",
            vec!["D-1"],
        ),
        (
            "in a block",
            &block,
            one("D-1", &door, mm(100, 0)),
            "NotAHost",
            vec!["D-1"],
        ),
        (
            "a block hosted",
            &wall,
            one("B-1", &block, mm(1000, 0)),
            "NotAnOpening",
            vec!["B-1"],
        ),
        (
            "a door with no width",
            &wall,
            one("D-1", &bad, mm(1000, 0)),
            "MissingArgument",
            vec!["D-1"],
        ),
    ];
    for (case, host, doors, code, words) in &cases {
        let error = k.build_hosting(host, doors).expect_err(case);
        assert_eq!(&error.code, code, "{case}: {}", error.message);
        assert!(
            error.message.starts_with("door "),
            "{case}: {}",
            error.message
        );
        for word in words {
            assert!(error.message.contains(word), "{case}: {}", error.message);
        }
    }
}

/// Named fault SC-4's case asked of the engine itself, past the builder, which refuses it: a wall
/// of 4000 by 200 by 2700 mm less a door's cutter whose head is one grid step under the wall's
/// top, inside the engine's tolerance. What the engine answers today is pinned here: its
/// subtraction keeps the sliver of one step over the head, the result's mesh closed and its six
/// volume, in cubic steps, exactly the wall less the opening (the interim register's row says so).
#[allow(
    clippy::cast_possible_truncation,
    reason = "mesh positions are whole steps under 2^40, which an i64 holds"
)]
#[allow(
    clippy::many_single_char_names,
    reason = "a box's length, thickness and height, and the engine"
)]
pub fn the_engine_keeps_a_sliver_within_its_tolerance<E: Engine>(mut e: E) {
    use engine_geometry::value::engine::Profile;
    let per = i64::try_from(
        construct_regen::kernel::Units::on(e.edition().step)
            .unwrap()
            .steps_per_mm(),
    )
    .unwrap();
    let p = |x: i64, y: i64, z: i64| P3::new(x, y, z).unwrap();
    let rect = |x0: i64, y0: i64, x1: i64, y1: i64, z: i64| Profile {
        outer: vec![p(x0, y0, z), p(x1, y0, z), p(x1, y1, z), p(x0, y1, z)],
        holes: vec![],
    };
    let (l, t, h) = (4000 * per, 200 * per, 2700 * per);
    let wall = e
        .extrude(&rect(0, 0, l, t, 0), [0, 0, h])
        .unwrap_or_else(|e| panic!("the wall: {e:?}"));
    let (x0, x1) = (1000 * per, 1900 * per);
    let head = h - 1;
    let cutter = e
        .extrude(
            &rect(x0, -100 * per, x1, 300 * per, -100 * per),
            [0, 0, head + 100 * per],
        )
        .unwrap_or_else(|e| panic!("the cutter: {e:?}"));
    let cut = e.subtract(&wall, &cutter);
    let cut = cut.unwrap_or_else(|e| panic!("the engine refused the cut: {e:?}"));
    let mesh = e
        .mesh(&cut)
        .unwrap_or_else(|e| panic!("the cut's mesh: {e:?}"));
    let positions = mesh
        .positions
        .iter()
        .flat_map(|q| q.map(|c| c as i64))
        .collect();
    let indices = mesh.triangles.iter().flatten().copied().collect();
    let six = closed_mesh_six_volume(&Mesh { positions, indices });
    let (l, t, h, w, head) = (
        i128::from(l),
        i128::from(t),
        i128::from(h),
        i128::from(x1 - x0),
        i128::from(head),
    );
    assert_eq!(
        six,
        6 * (l * t * h - w * t * head),
        "the sliver is kept exactly"
    );
}

/// A filled floor recipe (construct-slabs' `filled` form): `loops` of corners `(edge ID, x, y)` in
/// whole millimetres, the outer loop anticlockwise and each hole clockwise, written in grid steps;
/// and `layers` from the top down, `(function, thickness in mm)`, each of one material.
pub fn floor_of(loops: &[&[(&str, i64, i64)]], layers: &[(&str, i64)]) -> Recipe {
    let steps = |mm: i64| mm * 6400;
    let loops: Vec<Value> = loops
        .iter()
        .map(|ring| {
            Value::Array(
                ring.iter()
                    .map(|&(id, x, y)| json!({ "id": id, "at": [steps(x), steps(y)] }))
                    .collect(),
            )
        })
        .collect();
    let layers: Vec<Value> = layers
        .iter()
        .map(|&(function, mm)| {
            json!({ "function": function, "material": "concrete-regular", "thickness": steps(mm) })
        })
        .collect();
    recipe("floor", json!({ "loops": loops, "layers": layers }))
}

/// A 4 m by 3 m rectangle, its edges `e0` to `e3` anticlockwise from the origin.
pub const RECTANGLE: [(&str, i64, i64); 4] = [
    ("e0", 0, 0),
    ("e1", 4000, 0),
    ("e2", 4000, 3000),
    ("e3", 0, 3000),
];

/// The L-shaped floor of construct-slabs' tests: 8 m along x by 4 m, and 4 m by 4 m more above
/// its left half, 48 m2, its edges `e0` to `e5`.
pub const L_OUTER: [(&str, i64, i64); 6] = [
    ("e0", 0, 0),
    ("e1", 8000, 0),
    ("e2", 8000, 4000),
    ("e3", 4000, 4000),
    ("e4", 4000, 8000),
    ("e5", 0, 8000),
];

/// Its 2 m square hole at (1 m, 1 m), clockwise as the builder takes it (construct-slabs'
/// `Boundary::profile`), its edges `h0` to `h3` as drawn, so 44 m2 of floor in all.
pub const L_HOLE: [(&str, i64, i64); 4] = [
    ("h3", 1000, 1000),
    ("h2", 1000, 3000),
    ("h1", 3000, 3000),
    ("h0", 3000, 1000),
];

/// Six times the signed volume a mesh holds, in cubic millimetres, closed or not: the sum over
/// its triangles, which adds up shell by shell.
pub fn signed_six_volume(mesh: &Mesh) -> i128 {
    let point = |i: u32| -> [i128; 3] {
        let i = usize::try_from(i).unwrap() * 3;
        [
            i128::from(mesh.positions[i]),
            i128::from(mesh.positions[i + 1]),
            i128::from(mesh.positions[i + 2]),
        ]
    };
    let mut six = 0_i128;
    for t in mesh.indices.as_chunks::<3>().0 {
        let [p, q, r] = [point(t[0]), point(t[1]), point(t[2])];
        let cross = [
            q[1] * r[2] - q[2] * r[1],
            q[2] * r[0] - q[0] * r[2],
            q[0] * r[1] - q[1] * r[0],
        ];
        six += p[0] * cross[0] + p[1] * cross[1] + p[2] * cross[2];
    }
    six
}

/// The heights a mesh's points stand at, lowest first.
fn heights(mesh: &Mesh) -> BTreeSet<i64> {
    mesh.positions
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| p[2])
        .collect()
}

/// A rectangle floor is its plan extruded from its bottom face up to its top at 0: one layer is
/// one closed solid of its volume, its bounds the stand-in's box; its faces are named `top`,
/// `bottom` and a side per edge, the same names however long its edges; two layers are two
/// solids one under the other, `face.1` between them, the body listing each.
pub fn a_rectangle_floor_is_its_layers<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let one = floor_of(&[&RECTANGLE], &[("structure", 200)]);
    let b = built(&k, &one);
    assert_eq!(b.bounds.min, [0, 0, -200]);
    assert_eq!(b.bounds.max, [4000, 3000, 0]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry().build(&one).unwrap().bounds
    );
    assert_eq!(
        b.displays[0].outline,
        vec![[0, 0], [4000, 0], [4000, 3000], [0, 3000]]
    );
    let (six, surface) = box_of([4000, 3000, 200]);
    holds(&k, &b.displays[0].mesh, six, surface);
    assert_eq!(b.displays[0].mesh, b.displays[1].mesh);
    let names = by_role(&b);
    let roles: BTreeSet<&str> = names.keys().map(String::as_str).collect();
    assert_eq!(
        roles,
        BTreeSet::from(["top", "bottom", "side.e0", "side.e1", "side.e2", "side.e3"])
    );
    let wider = [
        ("e0", 0, 0),
        ("e1", 5000, 0),
        ("e2", 5000, 3000),
        ("e3", 0, 3000),
    ];
    assert_eq!(
        by_role(&built(&k, &floor_of(&[&wider], &[("structure", 150)]))),
        names,
        "a floor keeps its faces' names when its edges and its thickness change"
    );
    let wall = by_role(&built(&k, &wall(4000, "x")));
    assert!(names.values().all(|n| !wall.values().any(|w| w == n)));

    let two = built(
        &k,
        &floor_of(&[&RECTANGLE], &[("finish", 50), ("structure", 150)]),
    );
    assert_eq!(two.bounds.min, [0, 0, -200]);
    assert_eq!(
        heights(&two.displays[0].mesh),
        BTreeSet::from([-200, -50, 0])
    );
    assert_eq!(
        signed_six_volume(&two.displays[0].mesh),
        6 * 4000 * 3000 * 200
    );
    // Each layer is a part, top first, in its material: its own triangles of the mesh, between
    // its faces, the outer loop seen from above.
    let parts = &two.displays[0].parts;
    let functions: Vec<&str> = parts.iter().map(|p| p.function.as_str()).collect();
    assert_eq!(functions, ["finish", "structure"]);
    assert!(parts.iter().all(|p| p.material == "concrete-regular"));
    assert!(parts.iter().all(|p| p.outline == two.displays[0].outline));
    let mesh = &two.displays[0].mesh;
    let mut next = 0;
    for (part, faces) in parts.iter().zip([[-50, 0], [-200, -50]]) {
        let [first, count] = part.triangles;
        assert_eq!(first, next, "{}", part.function);
        next = first + count;
        let from = usize::try_from(first).unwrap() * 3;
        let to = usize::try_from(next).unwrap() * 3;
        let at: BTreeSet<i64> = mesh.indices[from..to]
            .iter()
            .map(|&i| mesh.positions[usize::try_from(i).unwrap() * 3 + 2])
            .collect();
        assert_eq!(at, BTreeSet::from(faces), "{}", part.function);
    }
    assert_eq!(usize::try_from(next).unwrap() * 3, mesh.indices.len());
    assert_eq!(two.displays[1].parts, *parts);
    let names = by_role(&two);
    assert!(
        names.contains_key("face.1") && names.len() == 7,
        "{names:?}"
    );
    let body: Value = serde_json::from_slice(&two.body).unwrap();
    let layers = body["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0]["function"], "finish");
    assert_eq!(layers[0]["by"], json!([0, 0, 50 * 6400]));
    assert_eq!(layers[1]["extrude"][0], json!([0, 0, -200 * 6400]));
    assert!(layers[0].get("holes").is_none());
    let again = built(
        &kernel(make()),
        &floor_of(&[&RECTANGLE], &[("finish", 50), ("structure", 150)]),
    );
    assert_eq!(again.body_id(), two.body_id());
    assert_eq!(again.displays[0].bytes(), two.displays[0].bytes());
}

/// The L-shaped floor with a hole, on an engine that extrudes a profile with holes: one layer is
/// one closed solid of 44 m2 by its thickness, within the engine's tolerance, its outline the
/// outer loop and its bounds the box around it; its sides are named by the edges of both loops;
/// two layers hold the volume of both, `face.1` between them.
pub fn an_l_shaped_floor_with_a_hole_is_its_layers<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let k = kernel(make());
    let one = floor_of(&[&L_OUTER, &L_HOLE], &[("structure", 200)]);
    let b = built(&k, &one);
    assert_eq!(b.bounds.min, [0, 0, -200]);
    assert_eq!(b.bounds.max, [8000, 8000, 0]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry().build(&one).unwrap().bounds
    );
    assert_eq!(
        b.displays[0].outline,
        L_OUTER.iter().map(|&(_, x, y)| [x, y]).collect::<Vec<_>>()
    );
    let area = 44_000_000_i128;
    let perimeter = 40_000_i128;
    holds(
        &k,
        &b.displays[0].mesh,
        6 * area * 200,
        2 * area + perimeter * 200,
    );
    let names = by_role(&b);
    assert_eq!(names.len(), 2 + 6 + 4, "{names:?}");
    for edge in ["e0", "e5", "h0", "h3"] {
        assert!(names.contains_key(&format!("side.{edge}")), "{edge}");
    }
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    assert_eq!(body["layers"][0]["holes"][0].as_array().unwrap().len(), 4);

    let two = built(
        &k,
        &floor_of(&[&L_OUTER, &L_HOLE], &[("finish", 50), ("structure", 150)]),
    );
    assert_eq!(
        heights(&two.displays[0].mesh),
        BTreeSet::from([-200, -50, 0])
    );
    assert_eq!(signed_six_volume(&two.displays[0].mesh), 6 * area * 200);
    assert!(by_role(&two).contains_key("face.1"));
}

/// A filled floor recipe of a sloped floor or one with slab edges (construct-slabs' `filled` form,
/// version 2): `floor_of`'s with `extra` arguments, every length in whole millimetres written
/// in grid steps by the caller.
pub fn shaped_floor(base: Recipe, extra: &[(&str, Value)]) -> Recipe {
    let mut recipe = base;
    recipe.op_version = 2;
    for (key, value) in extra {
        recipe.args.insert((*key).to_owned(), value.clone());
    }
    recipe
}

/// A slab edge's filled form: its edge, its kind, its plan at the underside `top` at height
/// `under` and at its foot at `foot`, all in whole millimetres.
pub fn slab_edge(
    id: &str,
    kind: &str,
    (top, under): ([[i64; 2]; 4], i64),
    (foot, down): ([[i64; 2]; 4], i64),
) -> Value {
    let at = |p: &[[i64; 2]; 4], z: i64| -> Vec<[i64; 3]> {
        p.iter()
            .map(|&[x, y]| [x * 6400, y * 6400, z * 6400])
            .collect()
    };
    json!({ "id": id, "kind": kind, "top": at(&top, under), "foot": at(&foot, down) })
}

/// A rectangle floor falling 40 mm towards x (construct-slabs' tops: 0 at x = 0, -40 mm at
/// x = 4 m), of a 50 mm and a 150 mm layer: each layer is the rectangle lifted onto the sloped
/// plane and extruded straight up, so the floor holds its plan area times its thickness exactly,
/// 12 m2 by 200 mm; its corners stand at 0 and -40 mm on top and -200 and -240 mm below; its
/// bounds are the stand-in's box, 240 mm deep; it keeps a flat floor's names; and the body lists
/// each layer's sloped profile.
pub fn a_sloped_floor_is_its_layers<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let flat = floor_of(&[&RECTANGLE], &[("finish", 50), ("structure", 150)]);
    let tops = json!([[0, -40 * 6400, -40 * 6400, 0]]);
    let sloped = shaped_floor(flat.clone(), &[("tops", tops)]);
    let b = built(&k, &sloped);
    assert_eq!(b.bounds.min, [0, 0, -240]);
    assert_eq!(b.bounds.max, [4000, 3000, 0]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry().build(&sloped).unwrap().bounds
    );
    let area = 12_000_000_i128;
    // One layer is one closed solid; two are two shells sharing a face, measured together.
    let one = floor_of(&[&RECTANGLE], &[("structure", 200)]);
    let tops_one = json!([[0, -40 * 6400, -40 * 6400, 0]]);
    let single = built(&k, &shaped_floor(one, &[("tops", tops_one)]));
    holds(
        &k,
        &single.displays[0].mesh,
        6 * area * 200,
        2 * area + 14_000 * 240,
    );
    assert_eq!(signed_six_volume(&b.displays[0].mesh), 6 * area * 200);
    assert_eq!(
        heights(&b.displays[0].mesh),
        BTreeSet::from([-240, -200, -90, -50, -40, 0])
    );
    assert_eq!(by_role(&b), by_role(&built(&k, &flat)));
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    assert_eq!(
        body["layers"][0]["extrude"][1],
        json!([4000 * 6400, 0, -90 * 6400])
    );
    assert_eq!(body["layers"][0]["by"], json!([0, 0, 50 * 6400]));
    assert_eq!(body["layers"][0]["lofted"], json!(true));

    // The L-shaped floor with its hole, sloped along its diagonal from 100 mm at the origin
    // falling 160 mm to (8 m, 8 m): z = 100 - (x + y) / 100 mm at each corner, the hole cut
    // through each layer; 44 m2 by 200 mm, exactly.
    let z = |x: i64, y: i64| (100 - (x + y) / 100) * 6400;
    let ring = |r: &[(&str, i64, i64)]| r.iter().map(|&(_, x, y)| z(x, y)).collect::<Vec<_>>();
    let tops = json!([ring(&L_OUTER), ring(&L_HOLE)]);
    let l = floor_of(&[&L_OUTER, &L_HOLE], &[("structure", 200)]);
    let b = built(&k, &shaped_floor(l, &[("tops", tops)]));
    assert_eq!(b.bounds.max[2], 100);
    assert_eq!(b.bounds.min[2], -220);
    assert_eq!(signed_six_volume(&b.displays[0].mesh), 6 * 44_000_000 * 200);
}

/// A rectangle floor 200 mm thick with a thickened edge all round, 400 mm wide at its underside
/// tapering to 200 mm at its foot 300 mm down, mitred at each corner: one solid, the slab united
/// with the four pieces (construct-slabs' volumes: 1.148 m3 of edges), its faces named
/// `edge.<edge ID>` besides the slab's; and a single 300 by 400 mm downturn, square at its ends.
pub fn a_floor_with_slab_edges_is_one_solid<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let flat = floor_of(&[&RECTANGLE], &[("structure", 200)]);
    let thick = |id: &str, top: [[i64; 2]; 4], foot: [[i64; 2]; 4]| {
        slab_edge(id, "thickened", (top, -200), (foot, -500))
    };
    let edges = json!([
        thick(
            "e0",
            [[0, 0], [4000, 0], [3600, 400], [400, 400]],
            [[0, 0], [4000, 0], [3800, 200], [200, 200]]
        ),
        thick(
            "e1",
            [[4000, 0], [4000, 3000], [3600, 2600], [3600, 400]],
            [[4000, 0], [4000, 3000], [3800, 2800], [3800, 200]]
        ),
        thick(
            "e2",
            [[4000, 3000], [0, 3000], [400, 2600], [3600, 2600]],
            [[4000, 3000], [0, 3000], [200, 2800], [3800, 2800]]
        ),
        thick(
            "e3",
            [[0, 3000], [0, 0], [400, 400], [400, 2600]],
            [[0, 3000], [0, 0], [200, 200], [200, 2800]]
        ),
    ]);
    let all_round = shaped_floor(flat.clone(), &[("edges", edges)]);
    let b = built(&k, &all_round);
    assert_eq!(b.bounds.min, [0, 0, -500]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry()
            .build(&all_round)
            .unwrap()
            .bounds
    );
    let slab = 12_000_000_i128 * 200;
    let six = 6 * (slab + 1_148_000_000);
    holds(&k, &b.displays[0].mesh, six, 2 * 12_000_000 + 14_000 * 500);
    assert_eq!(signed_six_volume(&b.displays[0].mesh), six);
    assert_eq!(b.displays[0].parts.len(), 1);
    let names = by_role(&b);
    for edge in ["e0", "e1", "e2", "e3"] {
        assert!(names.contains_key(&format!("edge.{edge}")), "{edge}");
    }
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    assert_eq!(body["layers"][0]["edges"].as_array().unwrap().len(), 4);
    assert_eq!(
        body["layers"][0]["edges"][0]["loft"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let down = slab_edge(
        "e0",
        "downturn",
        ([[0, 0], [4000, 0], [4000, 300], [0, 300]], -200),
        ([[0, 0], [4000, 0], [4000, 300], [0, 300]], -600),
    );
    let one = built(&k, &shaped_floor(flat, &[("edges", json!([down]))]));
    let six = 6 * (slab + 4000 * 300 * 400);
    assert_eq!(signed_six_volume(&one.displays[0].mesh), six);
    let body: Value = serde_json::from_slice(&one.body).unwrap();
    assert_eq!(
        body["layers"][0]["edges"][0]["by"],
        json!([0, 0, 400 * 6400])
    );
}

/// The default partition's layers (construct-walls' `wall-partition-stud-90`), from the exterior:
/// function, material and thickness in grid steps (12.5 mm, 90 mm, 12.5 mm; 115 mm in all).
pub const PARTITION: [(&str, &str, i64); 3] = [
    ("finish", "plasterboard-gypsum", 80_000),
    ("structure", "timber-douglas-fir", 576_000),
    ("finish", "plasterboard-gypsum", 80_000),
];

/// A stored length of `steps` grid steps, as a filled recipe holds it.
pub fn q0(steps: i64) -> String {
    construct_params::Value::Length(construct_params::Length::from_steps(steps)).stored()
}

/// A filled `wall.layered` recipe: its line from `start` to `end` and its height in whole
/// millimetres, and `layers` (function, material, thickness in steps) laid out from the exterior
/// face, centred on the line (the wall centreline as its location line).
pub fn layered_wall(
    start: [i64; 2],
    end: [i64; 2],
    height: i64,
    layers: &[(&str, &str, i64)],
) -> Recipe {
    let total: i64 = layers.iter().map(|l| l.2).sum();
    let mut face = total / 2;
    let mut listed = Vec::new();
    for &(function, material, thickness) in layers {
        listed.push(json!({
            "function": function, "material": material,
            "exterior": q0(face), "interior": q0(face - thickness),
        }));
        face -= thickness;
    }
    let mm = |v: i64| q0(v * 6400);
    recipe(
        "wall.layered",
        json!({
            "start": [mm(start[0]), mm(start[1])],
            "end": [mm(end[0]), mm(end[1])],
            "height": mm(height),
            "layers": listed,
        }),
    )
}

/// The default partition, 4000 mm long from the origin along x (or y), 2700 mm high.
pub fn partition(axis: &str) -> Recipe {
    let end = if axis == "y" { [0, 4000] } else { [4000, 0] };
    layered_wall([0, 0], end, 2700, &PARTITION)
}

/// One part's triangles of a display, as a mesh of their own.
pub fn part_mesh(display: &construct_model::geometry::Display, at: usize) -> Mesh {
    let [first, count] = display.parts[at].triangles;
    let (first, count) = (first as usize * 3, count as usize * 3);
    Mesh {
        positions: display.mesh.positions.clone(),
        indices: display.mesh.indices[first..first + count].to_vec(),
    }
}

/// A layered wall is one solid per layer: each layer's part of the finest display is a closed
/// mesh holding its layer's box, its faces each the nearest millimetre (12.5 mm lies 0.5 mm from
/// one, so the level strays by 1 mm), and the parts are listed from the exterior with their
/// materials and functions. Along x the exterior is on the right, at y below zero; along y at x
/// above zero. Its bounds are rounded outward, the stand-in's for the same recipe, and the body
/// lists what each layer asked of the engine, exactly in steps. Its names are the six roles and
/// the two faces between its layers, the same however long the wall is, and as
/// `construct_regen::kernel::names` gives them without building.
pub fn a_layered_wall_is_one_solid_per_layer<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let stand_in = construct_regen::geometry();
    for (axis, min, max) in [
        ("x", [0, -58, 0], [4000, 58, 2700]),
        ("y", [-58, 0, 0], [58, 4000, 2700]),
    ] {
        let wall = partition(axis);
        let b = built(&k, &wall);
        assert_eq!(b.bounds.min, min, "{axis}");
        assert_eq!(b.bounds.max, max, "{axis}");
        assert_eq!(b.bounds, stand_in.build(&wall).unwrap().bounds, "{axis}");
        let fine = &b.displays[0];
        assert_eq!(fine.deviation, 1, "{axis}: 12.5 mm is not on a millimetre");
        assert_eq!(fine.parts.len(), 3);
        let held: Vec<(&str, &str)> = fine
            .parts
            .iter()
            .map(|p| (p.function.as_str(), p.material.as_str()))
            .collect();
        let layers: Vec<(&str, &str)> = PARTITION.iter().map(|l| (l.0, l.1)).collect();
        assert_eq!(held, layers);
        // The faces at 57.5 and 45 mm from the line, each to its nearest millimetre: 13, 90 and
        // 13 mm thick.
        for (at, thick) in [(0, 13), (1, 90), (2, 13)] {
            let (whole, surface) = box_of([4000, thick, 2700]);
            holds(&k, &part_mesh(fine, at), whole, surface);
        }
        let parts: u32 = fine.parts.iter().map(|p| p.triangles[1]).sum();
        assert_eq!(
            parts as usize * 3,
            fine.mesh.indices.len(),
            "every triangle is a layer's"
        );
        assert_eq!(
            b.displays[1].mesh, fine.mesh,
            "no openings: one mesh at both levels"
        );
        let body: Value = serde_json::from_slice(&b.body).unwrap();
        let asked = body["layers"].as_array().unwrap();
        assert_eq!(asked.len(), 3);
        assert_eq!(asked[1]["material"], "timber-douglas-fir");
        assert_eq!(asked[1]["by"], json!([0, 0, 2700 * 6400]));
        assert!(body.get("subtract").is_none());
        let named = by_role(&b);
        let roles: Vec<&str> = named.keys().map(String::as_str).collect();
        assert_eq!(
            roles,
            [
                "base", "end0", "end1", "exterior", "face.1", "face.2", "interior", "top"
            ]
        );
        assert_eq!(construct_regen::kernel::names(&wall).unwrap(), b.names);
        let longer = layered_wall([0, 0], [6000, 0], 3000, &PARTITION);
        assert_eq!(
            by_role(&built(&k, &longer)),
            named,
            "names stay when it is lengthened"
        );
    }
    // Along x the exterior plasterboard is the layer at y below zero.
    let b = built(&k, &partition("x"));
    let exterior = part_mesh(&b.displays[0], 0);
    let ys: BTreeSet<i64> = exterior
        .indices
        .iter()
        .map(|&i| exterior.positions[i as usize * 3 + 1])
        .collect();
    assert_eq!(ys, BTreeSet::from([-58, -45]));
}

/// A door hosted in a layered wall cuts its opening through every layer: each layer's part of the
/// finest display is closed and holds its layer less the opening's width and height through its
/// thickness; the coarse level is the wall before its openings, straying by half the opening's
/// width and the rounding; the opening's faces are named by its door as in the old box; the body
/// records its cutter once, by the door's key. Along x and along y.
pub fn a_door_cuts_every_layer_of_a_layered_wall<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    for axis in ["x", "y"] {
        let wall = partition(axis);
        let plain = built(&k, &wall);
        let door = door_of(900, 2100, axis);
        // A door 300 mm thick from 150 mm outside the exterior face: the opening goes through the
        // whole wall whatever the door's own thickness.
        let place = if axis == "y" {
            [-150, 1000, 0]
        } else {
            [1000, -150, 0]
        };
        let cut = hosting(&k, &wall, &[hosted("D-1", &door, at(&k, place))]);
        assert_eq!(cut.bounds, plain.bounds, "{axis}");
        let fine = &cut.displays[0];
        assert_eq!(fine.parts.len(), 3);
        for (at, thick) in [(0, 13), (1, 90), (2, 13)] {
            let (whole, surface) = box_of([4000, thick, 2700]);
            holds(
                &k,
                &part_mesh(fine, at),
                whole - 6 * 900 * thick * 2100,
                surface + 2 * thick * (2100 + 900),
            );
        }
        assert_eq!(
            cut.displays[1].mesh, plain.displays[1].mesh,
            "{axis}: the coarse level"
        );
        assert_eq!(cut.displays[1].deviation, 450 + 1, "{axis}");
        let named = by_role(&cut);
        for role in [
            "tool(D-1).jamb.start",
            "tool(D-1).jamb.end",
            "tool(D-1).head",
        ] {
            assert!(named.contains_key(role), "{axis}: {role}");
        }
        assert!(!named.contains_key("tool(D-1).sill"));
        let body: Value = serde_json::from_slice(&cut.body).unwrap();
        let keys: Vec<&str> = body["subtract"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["key"].as_str().unwrap())
            .collect();
        assert_eq!(keys, ["D-1"]);
    }
}

/// A door is refused in a layered wall as in the old box, before the engine is asked, and
/// `fits` says the same, word for word: an opening touching or crossing an end, as tall as the
/// wall, two openings that overlap, an element that is not a door, one beside its wall. A wall at
/// an angle hosts no door yet: a door is a box along an axis until doors are families.
pub fn a_door_in_a_layered_wall_is_refused_as_in_the_box<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    type Case<'a> = (&'a str, &'a Recipe, Vec<Hosted<'a>>, &'a str);
    let k = kernel(make());
    let wall = partition("x");
    let door = door();
    let full = door_of(900, 2700, "x");
    let block = block();
    let mm = |x: i64, z: i64| at(&k, [x, -150, z]);
    let one = |key, recipe, at| vec![hosted(key, recipe, at)];
    let angled = layered_wall([0, 0], [2400, 3200], 2700, &PARTITION);
    let cases: Vec<Case<'_>> = vec![
        (
            "touching end0",
            &wall,
            one("D-1", &door, mm(0, 0)),
            "OpeningAtEnd",
        ),
        (
            "crossing end1",
            &wall,
            one("D-1", &door, mm(3500, 0)),
            "OpeningPastEnd",
        ),
        (
            "as tall as the wall",
            &wall,
            one("D-1", &full, mm(1000, 0)),
            "OpeningTooTall",
        ),
        (
            "overlapping",
            &wall,
            vec![
                hosted("D-2", &door, mm(1500, 0)),
                hosted("D-1", &door, mm(1000, 0)),
            ],
            "OpeningsOverlap",
        ),
        (
            "a block hosted",
            &wall,
            one("B-1", &block, mm(1000, 0)),
            "NotAnOpening",
        ),
        (
            "beside its wall",
            &wall,
            one("D-1", &door, at(&k, [1000, 500, 0])),
            "OpeningNotInWall",
        ),
        (
            "at an angle",
            &angled,
            one("D-1", &door, mm(1000, 0)),
            "NotAHost",
        ),
    ];
    let edition = k.edition();
    for (case, host, doors, code) in &cases {
        let error = k.build_hosting(host, doors).expect_err(case);
        assert_eq!(&error.code, code, "{case}: {}", error.message);
        assert!(
            error.message.starts_with("door "),
            "{case}: {}",
            error.message
        );
        let placed: Vec<construct_model::geometry::Hosted<'_>> = doors
            .iter()
            .map(|h| construct_model::geometry::Hosted {
                key: h.key,
                recipe: h.recipe,
                at: construct_model::element::Placement {
                    x: k.units().millimetres(h.at.x.get()).unwrap(),
                    y: k.units().millimetres(h.at.y.get()).unwrap(),
                    z: k.units().millimetres(h.at.z.get()).unwrap(),
                },
            })
            .collect();
        let said = construct_regen::kernel::fits(&edition, host, &placed).expect_err(case);
        assert_eq!(said, error, "{case}: fits says what building says");
    }
}

/// A layered wall at an angle: each layer is the parallelogram the plan gives
/// (`construct_regen::wall_plan::Plan::strip`), asked of the engine exactly, corner for corner;
/// its display is each corner's nearest millimetre, each layer's part closed, and its bounds hold
/// every corner, rounded outward. A 3-4-5 wall's faces fall on whole steps.
pub fn a_layered_wall_at_an_angle_is_its_strips_extruded<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    let k = kernel(make());
    let wall = layered_wall([0, 0], [2400, 3200], 2700, &PARTITION);
    let b = built(&k, &wall);
    let plan = construct_regen::wall_plan::Layered::of(&wall).unwrap();
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    let asked = body["layers"].as_array().unwrap();
    assert_eq!(asked.len(), plan.layers.len());
    for (layer, asked) in plan.layers.iter().zip(asked) {
        let (corners, _) = plan.plan.strip(layer.exterior, layer.interior).unwrap();
        let expected: Vec<[i64; 3]> = corners.iter().map(|&[x, y]| [x, y, 0]).collect();
        assert_eq!(asked["extrude"], json!(expected));
    }
    // The exterior face is 57.5 mm to the right of (3, 4) / 5: (46, -34.5) mm from the line.
    assert_eq!(b.bounds.min, [-46, -35, 0]);
    assert_eq!(b.bounds.max, [2446, 3235, 2700]);
    assert_eq!(b.displays[0].parts.len(), 3);
    assert_eq!(b.displays[0].deviation, 1);
    for at in 0..3 {
        let six = closed_mesh_six_volume(&part_mesh(&b.displays[0], at));
        assert!(six > 0, "layer {at} holds a volume");
    }
}

/// A filled roof recipe (construct-roofs' `filled` form) of the 6 m by 4 m footprint with 500 mm
/// over its eaves and 300 mm over its verges, every point a whole millimetre, written in grid
/// steps: `form` `gable` rising at 1 in 4 from `o0` (y = 0) and `o2` (y = 4 m) to a ridge at
/// y = 2 m, `mono` rising at 1 in 4 from `o0` to a high side 300 mm past `o2`, or `flat`; `faces`
/// the upright heights of its layer faces above its bottom face, top first and 0 last, each layer
/// between two of them (as construct-roofs gives them, a face of a slope stands higher upright
/// than square to it: 206 mm upright is 200 mm square at 1 in 4).
pub fn roof_of(form: &str, faces: &[i64]) -> Recipe {
    let steps = |mm: i64| mm * 6400;
    let outline = json!([
        { "id": "o0", "at": [steps(-300), steps(-500)] },
        { "id": "o1", "at": [steps(6300), steps(-500)] },
        { "id": "o2", "at": [steps(6300), steps(4500)] },
        { "id": "o3", "at": [steps(-300), steps(4500)] },
    ]);
    // The bottom face across the roof, (y, z) in mm.
    let bottom: Vec<(i64, i64)> = match form {
        "gable" => vec![(-500, -125), (2000, 500), (4500, -125)],
        _ => vec![(-500, -125), (4300, 1075)],
    };
    let functions = ["finish", "substrate", "structure"];
    let mut layers = Vec::new();
    for (at, pair) in faces.windows(2).enumerate() {
        let (high, low) = (pair[0], pair[1]);
        let solid = if form == "flat" {
            let corner = |x: i64, y: i64| [steps(x), steps(y), steps(low)];
            json!({
                "extrude": [corner(-300, -500), corner(6300, -500), corner(6300, 4500), corner(-300, 4500)],
                "by": [0, 0, steps(high - low)],
            })
        } else {
            let section = |x: i64| {
                let mut out: Vec<[i64; 3]> = bottom
                    .iter()
                    .map(|&(y, z)| [steps(x), steps(y), steps(z + low)])
                    .collect();
                out.extend(
                    bottom
                        .iter()
                        .rev()
                        .map(|&(y, z)| [steps(x), steps(y), steps(z + high)]),
                );
                out
            };
            json!({ "loft": [section(-300), section(6300)] })
        };
        layers.push(json!({
            "function": functions[at % 3], "material": "concrete-regular",
            "thickness": steps(high - low), "solid": solid,
        }));
    }
    let slopes = match form {
        "gable" => json!(["o0", "o2"]),
        "mono" => json!(["o0"]),
        _ => json!([]),
    };
    recipe(
        "roof",
        json!({ "form": form, "slopes": slopes, "outline": outline, "layers": layers }),
    )
}

/// A gable roof is one solid per layer, each its chevron lofted from verge to verge: each layer's
/// part is a closed mesh holding exactly its volume (5 m across by its upright thickness, 6.6 m
/// long), the parts top first in their materials, the bounds the stand-in's box; its faces are
/// named by the edges its slopes rise from and by every edge's side, the same names when its
/// layers change, and as `construct_regen::kernel::names` gives them without building. The body
/// lists each layer's two sections exactly in steps.
pub fn a_gable_roof_is_its_layers<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let two = roof_of("gable", &[210, 206, 0]);
    let b = built(&k, &two);
    assert_eq!(b.bounds.min, [-300, -500, -125]);
    assert_eq!(b.bounds.max, [6300, 4500, 710]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry().build(&two).unwrap().bounds
    );
    assert_eq!(
        b.displays[0].outline,
        vec![[-300, -500], [6300, -500], [6300, 4500], [-300, 4500]]
    );
    assert_eq!(b.displays[0].deviation, 0);
    let parts = &b.displays[0].parts;
    let functions: Vec<&str> = parts.iter().map(|p| p.function.as_str()).collect();
    assert_eq!(functions, ["finish", "substrate"]);
    for (at, upright) in [(0, 4_i128), (1, 206)] {
        let six = closed_mesh_six_volume(&part_mesh(&b.displays[0], at));
        assert_eq!(six, 6 * 5000 * upright * 6600, "layer {at}");
    }
    assert_eq!(b.displays[1].mesh, b.displays[0].mesh);
    let names = by_role(&b);
    let roles: BTreeSet<&str> = names.keys().map(String::as_str).collect();
    assert_eq!(
        roles,
        BTreeSet::from([
            "top.o0",
            "bottom.o0",
            "face.1.o0",
            "top.o2",
            "bottom.o2",
            "face.1.o2",
            "side.o0",
            "side.o1",
            "side.o2",
            "side.o3",
        ])
    );
    let mut sorted = b.names.clone();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(construct_regen::kernel::names(&two).unwrap(), sorted);
    let thicker = built(&k, &roof_of("gable", &[300, 250, 0]));
    assert_eq!(by_role(&thicker), names, "a roof keeps its faces' names");
    let floor = by_role(&built(&k, &floor_of(&[&RECTANGLE], &[("structure", 200)])));
    assert!(names.values().all(|n| !floor.values().any(|f| f == n)));
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    let asked = body["layers"].as_array().unwrap();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[1]["loft"], two.args["layers"][1]["solid"]["loft"]);
    assert!(asked[1].get("extrude").is_none());
}

/// A mono-pitch roof is one solid per layer, its parallelogram lofted from verge to verge: a
/// closed mesh holding exactly 4.8 m across by 206 mm upright by 6.6 m; its slope named by its
/// eaves edge.
pub fn a_mono_roof_is_its_layers<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let one = roof_of("mono", &[206, 0]);
    let b = built(&k, &one);
    assert_eq!(b.bounds.min, [-300, -500, -125]);
    assert_eq!(b.bounds.max, [6300, 4300, 1281]);
    assert_eq!(
        b.bounds,
        construct_regen::geometry().build(&one).unwrap().bounds
    );
    assert_eq!(closed_six_volume(&b), 6 * 4800 * 206 * 6600);
    let names = by_role(&b);
    let roles: BTreeSet<&str> = names.keys().map(String::as_str).collect();
    assert_eq!(
        roles,
        BTreeSet::from([
            "top.o0",
            "bottom.o0",
            "side.o0",
            "side.o1",
            "side.o2",
            "side.o3"
        ])
    );
}

/// A flat roof is one solid per layer, its eaves outline extruded straight up from the base: two
/// closed meshes of 6.6 m by 5 m by 100 and 200 mm, `face.1` between them.
pub fn a_flat_roof_is_its_layers<E: Engine + Send + 'static>(make: impl Fn() -> E) {
    let k = kernel(make());
    let two = roof_of("flat", &[300, 200, 0]);
    let b = built(&k, &two);
    assert_eq!(b.bounds.min, [-300, -500, 0]);
    assert_eq!(b.bounds.max, [6300, 4500, 300]);
    for (at, thick) in [(0, 100_i128), (1, 200)] {
        let six = closed_mesh_six_volume(&part_mesh(&b.displays[0], at));
        assert_eq!(six, 6 * 6600 * 5000 * thick, "layer {at}");
    }
    let names = by_role(&b);
    assert!(
        names.contains_key("top") && names.contains_key("face.1") && names.len() == 7,
        "{names:?}"
    );
    let body: Value = serde_json::from_slice(&b.body).unwrap();
    assert_eq!(body["layers"][0]["by"], json!([0, 0, 100 * 6400]));
    assert_eq!(
        body["layers"][1]["extrude"][0],
        json!([-300 * 6400, -500 * 6400, 0])
    );
}

// Placed families (job 6.4, CON-A7-F3-P2): a door and a window made from the library's recipes
// in a three-layer wall.

/// A three-layer wall's layers, from the exterior: 15 mm of plasterboard, 200 mm of timber, 15 mm
/// of plasterboard, in grid steps; 230 mm in all, so every face is on a whole millimetre.
pub const THREE: [(&str, &str, i64); 3] = [
    ("finish", "plasterboard-gypsum", 15 * 6400),
    ("structure", "timber-douglas-fir", 200 * 6400),
    ("finish", "plasterboard-gypsum", 15 * 6400),
];

/// The library's single-swing door and fixed window, by their permanent ids.
pub const DOOR_FAMILY: &str = "library.doors-windows.single-swing-door";
pub const WINDOW_FAMILY: &str = "library.doors-windows.fixed-window";

/// A placed family's recipe as a builder is handed it: the library family `family`, its type
/// `ty`, turned by `turn`.
pub fn placed(family: &str, ty: &str, turn: construct_families::Turn) -> Recipe {
    let (id, _) = construct_regen::families::Library::baked()
        .family(family)
        .unwrap_or_else(|| panic!("{family} is in the library"));
    let ty = construct_families::Key::parse(ty).unwrap();
    construct_regen::families::turned(&construct_regen::families::recipe(&id, &ty), turn)
}

/// Each layer's six times its volume in a display, in cubic millimetres.
fn layer_volumes(display: &construct_model::geometry::Display) -> Vec<i128> {
    (0..display.parts.len())
        .map(|at| signed_six_volume(&part_mesh(display, at)))
        .collect()
}

/// Six times the volume of the three-layer wall `length` long and 2700 mm high, each layer less
/// the openings `cut` (each a width and a height in millimetres, through the whole wall).
fn three_less(length: i128, cut: &[(i128, i128)]) -> Vec<i128> {
    let open: i128 = cut.iter().map(|(w, h)| w * h).sum();
    [15, 200, 15]
        .iter()
        .map(|&t| 6 * t * (length * 2700 - open))
        .collect()
}

/// The lowest and highest along, across and up the cutter of the opening `key` in a wall's body.
fn cutter(body: &Value, key: &str) -> ([i64; 3], [i64; 3]) {
    let cut = body["subtract"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == key)
        .unwrap_or_else(|| panic!("{key} is cut"));
    let corners: Vec<[i64; 3]> = serde_json::from_value(cut["extrude"].clone()).unwrap();
    let by: [i64; 3] = serde_json::from_value(cut["by"].clone()).unwrap();
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for c in &corners {
        for p in [*c, [c[0] + by[0], c[1] + by[1], c[2] + by[2]]] {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a] / 6400);
                hi[a] = hi[a].max(p[a] / 6400);
            }
        }
    }
    (lo, hi)
}

/// A placed door (900 by 2100) and a window (1200 by 1200 at its 900 mm sill) on the exterior of
/// a three-layer wall each cut their opening through every layer: each layer holds exactly its
/// box less the two openings' width by height through its thickness; the cutters are the
/// openings, reaching 100 mm past both sides (and below the base for the door); the openings'
/// faces are named by the families that made them (no sill for the door). Each family's solids
/// are its own: the door's frame and leaf and the window's frame and glass, each a part with its
/// material and exactly its volume; they stand inside their openings, across the wall's
/// thickness, the stand-in's bounds the same. Where the engine answers a subtraction, taking the
/// cut wall from each solid leaves the whole solid: none of a family is in its wall.
#[allow(
    clippy::too_many_lines,
    reason = "one wall's layers, its two families' solids and the engine's own answer, in order"
)]
pub fn a_family_cuts_every_layer_and_stands_in_its_opening<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    use construct_families::Turn;
    let k = kernel(make());
    let wall = layered_wall([0, 0], [4000, 0], 2700, &THREE);
    let door = placed(DOOR_FAMILY, "d900x2100", Turn::Half);
    let window = placed(WINDOW_FAMILY, "w1200x1200", Turn::Half);
    let plain = built(&k, &wall);
    let cut = hosting(
        &k,
        &wall,
        &[
            hosted("F-2", &window, at(&k, [3000, 0, 0])),
            hosted("F-1", &door, at(&k, [1500, 0, 0])),
        ],
    );
    assert_eq!(cut.bounds, plain.bounds);
    assert_eq!(layer_volumes(&plain.displays[0]), three_less(4000, &[]));
    assert_eq!(
        layer_volumes(&cut.displays[0]),
        three_less(4000, &[(900, 2100), (1200, 1200)]),
        "each layer loses exactly the two openings"
    );
    let body: Value = serde_json::from_slice(&cut.body).unwrap();
    let keys: Vec<&str> = body["subtract"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["F-1", "F-2"], "cut in the order of their keys");
    assert_eq!(
        cutter(&body, "F-1"),
        ([1050, -215, -100], [1950, 215, 2100])
    );
    assert_eq!(cutter(&body, "F-2"), ([2400, -215, 900], [3600, 215, 2100]));
    let named = by_role(&cut);
    for role in ["tool(F-1).jamb.start", "tool(F-1).head", "tool(F-2).sill"] {
        assert!(named.contains_key(role), "{role}");
    }
    assert!(
        !named.contains_key("tool(F-1).sill"),
        "the door is to the floor"
    );

    let stand_in = construct_regen::geometry();
    for (family, origin, opening, solids) in [
        (
            &door,
            [1500, 0, 0],
            ([1050, 0], [1950, 2100]),
            vec![
                (
                    "frame",
                    "timber-douglas-fir",
                    6 * (900 * 2100 - 820 * 2060) * 100,
                ),
                ("leaf", "timber-white-oak", 6 * 820 * 2060 * 40),
            ],
        ),
        (
            &window,
            [3000, 0, 0],
            ([2400, 900], [3600, 2100]),
            vec![
                (
                    "frame",
                    "aluminium-6061",
                    6 * (1200 * 1200 - 1100 * 1100) * 70,
                ),
                ("glass", "glass-plate", 6 * 1100 * 1100 * 24),
            ],
        ),
    ] {
        let b = built(&k, family);
        let fine = &b.displays[0];
        let held: Vec<(&str, &str, i128)> = fine
            .parts
            .iter()
            .enumerate()
            .map(|(at, p)| {
                (
                    p.function.as_str(),
                    p.material.as_str(),
                    signed_six_volume(&part_mesh(fine, at)),
                )
            })
            .collect();
        assert_eq!(
            held, solids,
            "each solid is a part with its material and volume"
        );
        // In the wall's frame: inside the opening along and up, within the wall across it.
        let (lo, hi) = (b.bounds.min, b.bounds.max);
        let placed_lo = [lo[0] + origin[0], lo[1] + origin[1], lo[2] + origin[2]];
        let placed_hi = [hi[0] + origin[0], hi[1] + origin[1], hi[2] + origin[2]];
        let ((x0, z0), (x1, z1)) = ((opening.0[0], opening.0[1]), (opening.1[0], opening.1[1]));
        assert!(
            placed_lo[0] >= x0 && placed_hi[0] <= x1,
            "{placed_lo:?} {placed_hi:?}"
        );
        assert!(
            placed_lo[2] >= z0 && placed_hi[2] <= z1,
            "{placed_lo:?} {placed_hi:?}"
        );
        assert!(
            placed_lo[1] > -115 && placed_hi[1] < 115,
            "{placed_lo:?} {placed_hi:?}"
        );
        assert_eq!(
            stand_in.build(family).unwrap().bounds,
            b.bounds,
            "the stand-in's box is the kernel's bounds"
        );
        assert_eq!(b.names.len(), 0, "a placed family names no face yet");
    }

    // Through the engine itself, where it answers: each of the door's solids, less the wall's
    // layers with the door's opening cut, is the whole solid. Everything is in the door's own
    // frame, its origin at 1500 mm along the wall.
    let mut e = make();
    let (_, recipe) = construct_regen::families::Library::baked()
        .family(DOOR_FAMILY)
        .unwrap();
    let values = construct_families::TypeValues::of(
        &recipe,
        &construct_families::Key::parse("d900x2100").unwrap(),
    )
    .unwrap();
    let solids = construct_families::evaluate_turned(
        &mut e,
        &recipe,
        &values,
        construct_families::Limits::DEFAULT,
        Turn::Half,
    )
    .unwrap();
    let mm = |v: i64| v * 6400;
    let slab = |lo: [i64; 3], hi: [i64; 3]| {
        let corner = |x: i64, y: i64| P3::new(mm(x), mm(y), mm(lo[2])).unwrap();
        engine_geometry::value::engine::Profile {
            outer: vec![
                corner(lo[0], lo[1]),
                corner(hi[0], lo[1]),
                corner(hi[0], hi[1]),
                corner(lo[0], hi[1]),
            ],
            holes: Vec::new(),
        }
    };
    let opening = e
        .extrude(&slab([-450, -215, -100], [450, 215, 0]), [0, 0, mm(2200)])
        .unwrap();
    let mut answered = 0;
    for (y0, y1) in [(-115, -100), (-100, 100), (100, 115)] {
        let layer = e
            .extrude(&slab([-1500, y0, 0], [2500, y1, 0]), [0, 0, mm(2700)])
            .unwrap();
        let Ok(cut) = e.subtract(&layer, &opening) else {
            continue;
        };
        for solid in &solids.solids {
            let (Ok(rest), Ok(whole)) = (e.subtract(&solid.solid, &cut), e.volume(&solid.solid))
            else {
                continue;
            };
            if let Ok(left) = e.volume(&rest) {
                assert_eq!(left, whole, "{} keeps its whole volume", solid.step);
                answered += 1;
            }
        }
    }
    eprintln!("the engine answered {answered} of 6 subtractions of a wall layer from a solid");
}

/// Moving the wall moves its family's opening with it: the same family on a wall drawn 1000 mm
/// further along x and 500 mm along y, at the same offset along it, is cut there, exactly as far
/// away, every layer the same volume; the family itself builds to the same bytes, its place being
/// its placement's. Changing the family's type resizes its opening: a 1000 by 2400 door cuts
/// 1000 by 2400 from every layer.
pub fn a_wall_moved_or_a_type_changed_moves_or_resizes_the_opening<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    use construct_families::Turn;
    let k = kernel(make());
    let door = placed(DOOR_FAMILY, "d900x2100", Turn::Half);
    let here = layered_wall([0, 0], [4000, 0], 2700, &THREE);
    let there = layered_wall([1000, 500], [5000, 500], 2700, &THREE);
    let a = hosting(&k, &here, &[hosted("F-1", &door, at(&k, [1500, 0, 0]))]);
    let b = hosting(&k, &there, &[hosted("F-1", &door, at(&k, [2500, 500, 0]))]);
    let (ab, bb): (Value, Value) = (
        serde_json::from_slice(&a.body).unwrap(),
        serde_json::from_slice(&b.body).unwrap(),
    );
    let ((alo, ahi), (blo, bhi)) = (cutter(&ab, "F-1"), cutter(&bb, "F-1"));
    assert_eq!(
        [
            blo[0] - alo[0],
            blo[1] - alo[1],
            bhi[0] - ahi[0],
            bhi[1] - ahi[1]
        ],
        [1000, 500, 1000, 500]
    );
    assert_eq!(layer_volumes(&b.displays[0]), layer_volumes(&a.displays[0]));
    assert_eq!(built(&k, &door), built(&k, &door));
    let builds = k.family_builds().unwrap();
    built(&k, &door);
    assert_eq!(
        k.family_builds().unwrap(),
        builds,
        "a type is built once and kept"
    );

    let wider = placed(DOOR_FAMILY, "d1000x2400", Turn::Half);
    let c = hosting(&k, &here, &[hosted("F-1", &wider, at(&k, [1500, 0, 0]))]);
    let cb: Value = serde_json::from_slice(&c.body).unwrap();
    assert_eq!(cutter(&cb, "F-1"), ([1000, -215, -100], [2000, 215, 2400]));
    assert_eq!(
        layer_volumes(&c.displays[0]),
        three_less(4000, &[(1000, 2400)])
    );
    let shape = built(&k, &wider);
    assert_eq!(shape.bounds.min, [-500, -50, 0]);
    assert_eq!(shape.bounds.max, [500, 50, 2400]);
    assert_eq!(k.family_builds().unwrap(), builds + 1);
}

/// A family that does not fit is refused before the engine is asked, naming the family and why,
/// and `fits` says the same word for word: one wider than its wall, one off its end, one touching
/// it, one as tall as the wall, two whose openings overlap, one turned across its wall, one on
/// the old box wall, one on a wall at an angle, one whose recipe is not loaded and one whose type
/// its recipe does not name.
#[allow(
    clippy::too_many_lines,
    reason = "one case per refusal, each asked of the builder and of fits"
)]
pub fn a_family_that_does_not_fit_is_refused_with_a_reason<E: Engine + Send + 'static>(
    make: impl Fn() -> E,
) {
    use construct_families::Turn;
    type Case<'a> = (&'a str, &'a Recipe, Vec<Hosted<'a>>, &'a str);
    let k = kernel(make());
    let boxed = wall(4000, "x");
    let wall = layered_wall([0, 0], [4000, 0], 2700, &THREE);
    let short = layered_wall([0, 0], [900, 0], 2700, &THREE);
    let low = layered_wall([0, 0], [4000, 0], 2400, &THREE);
    let angled = layered_wall([0, 0], [2400, 3200], 2700, &THREE);
    let door = placed(DOOR_FAMILY, "d900x2100", Turn::Half);
    let wide = placed(DOOR_FAMILY, "d1000x2100", Turn::Half);
    let tall = placed(DOOR_FAMILY, "d900x2400", Turn::Half);
    let window = placed(WINDOW_FAMILY, "w1200x1200", Turn::Half);
    let across = placed(DOOR_FAMILY, "d900x2100", Turn::Quarter);
    let mut missing = door.clone();
    missing.args.insert(
        "family".to_owned(),
        json!(format!("sha256:{}", "0".repeat(64))),
    );
    let unnamed = placed(DOOR_FAMILY, "d1x1", Turn::Half);
    let mm = |x: i64, z: i64| at(&k, [x, 0, z]);
    let one = |key, recipe, at| vec![hosted(key, recipe, at)];
    let cases: Vec<Case<'_>> = vec![
        (
            "wider than its wall",
            &short,
            one("F-1", &wide, mm(450, 0)),
            "OpeningPastEnd",
        ),
        (
            "off its end",
            &wall,
            one("F-1", &door, mm(3800, 0)),
            "OpeningPastEnd",
        ),
        (
            "touching its end",
            &wall,
            one("F-1", &door, mm(450, 0)),
            "OpeningAtEnd",
        ),
        (
            "as tall as its wall",
            &low,
            one("F-1", &tall, mm(1500, 0)),
            "OpeningTooTall",
        ),
        (
            "overlapping",
            &wall,
            vec![
                hosted("F-1", &door, mm(1500, 0)),
                hosted("F-2", &window, mm(2000, 0)),
            ],
            "OpeningsOverlap",
        ),
        (
            "turned across",
            &wall,
            one("F-1", &across, mm(1500, 0)),
            "OpeningAcrossWall",
        ),
        (
            "on the box wall",
            &boxed,
            one("F-1", &door, at(&k, [1500, 100, 0])),
            "NotAHost",
        ),
        (
            "on a wall at an angle",
            &angled,
            one("F-1", &door, mm(1500, 0)),
            "NotAHost",
        ),
        (
            "not loaded",
            &wall,
            one("F-1", &missing, mm(1500, 0)),
            "FamilyMissing",
        ),
        (
            "no such type",
            &wall,
            one("F-1", &unnamed, mm(1500, 0)),
            "FamilyType",
        ),
    ];
    let edition = k.edition();
    for (case, host, families, code) in &cases {
        let error = k.build_hosting(host, families).expect_err(case);
        assert_eq!(&error.code, code, "{case}: {}", error.message);
        assert!(
            error.message.starts_with("family F-1"),
            "{case}: {}",
            error.message
        );
        let placed: Vec<construct_model::geometry::Hosted<'_>> = families
            .iter()
            .map(|h| construct_model::geometry::Hosted {
                key: h.key,
                recipe: h.recipe,
                at: construct_model::element::Placement {
                    x: k.units().millimetres(h.at.x.get()).unwrap(),
                    y: k.units().millimetres(h.at.y.get()).unwrap(),
                    z: k.units().millimetres(h.at.z.get()).unwrap(),
                },
            })
            .collect();
        let said = construct_regen::kernel::fits(&edition, host, &placed).expect_err(case);
        assert_eq!(said, error, "{case}: fits says what building says");
    }
    let message = k.build_hosting(&short, &cases[0].2).unwrap_err().message;
    assert_eq!(message, "family F-1: it crosses the wall's end");
    // A placed family is no host, and one whose type is unknown does not build on its own.
    let hosts = k
        .build_hosting(&door, &one("D-1", &door, mm(0, 0)))
        .unwrap_err();
    assert_eq!(hosts.code, "NotAHost");
    assert_eq!(k.build(&unnamed).unwrap_err().code, "FamilyType");
}
