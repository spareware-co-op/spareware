//! Recipe evaluation (CON-A7-F1-P2, P3 and P6): the evaluation suite (`evaluate_suite`) on a test
//! engine of this file's own, then every refusal, each with its place and reason (an unsolvable
//! sketch among them), named types and the type table, the limits, and the cache per type.
//! Everything is judged through this crate's types and the `Engine` trait only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;
mod evaluate_suite;

use common::{EXAMPLES, Fixture, door_table, read};
use construct_families::{
    CacheKey, EvalError, Evaluator, Limit, Limits, Place, Reason, Recipe, Sort, TypeValues,
    evaluate, evaluate_within,
};
use construct_solver::{ConstraintId, SolveError};
use engine_geometry::value::engine::{
    Answer, Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Tolerance,
    Volume,
};
use engine_geometry::value::grid::GridStep;
use evaluate_suite::{
    BLOCK, DOOR, SIDE, SQUARE, block, d820, deep, door, extrude, key, length, on_two,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// One prism of a test solid: a flat loop and its holes swept along `by`, added (`sign` 1) or
/// taken away (`sign` -1).
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    sign: i8,
    outer: Vec<[i64; 3]>,
    holes: Vec<Vec<[i64; 3]>>,
    by: [i64; 3],
}

/// A test solid: prisms added and taken away.
#[derive(Clone, Debug, PartialEq)]
struct Shape {
    pieces: Vec<Piece>,
}

/// A test engine: it keeps an extrusion as the prism it is, a subtraction as the target with the
/// tool taken away (only when the tool lies within the target's bounds) and a union as both
/// (only when they share no space), and meshes each prism as its two caps and its sides, so the
/// mesh encloses exactly the volume meant. It is judged by the suite like any engine; it is not a
/// geometry engine. It counts the extrusions it is asked for.
struct Prisms {
    step: GridStep,
    extrusions: u32,
}

impl Prisms {
    fn new() -> Self {
        Self {
            step: GridStep::Q0,
            extrusions: 0,
        }
    }
}

fn unsupported(operation: &'static str, what: &str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: what.to_owned(),
    }
}

fn add(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A loop's normal by Newell's method, unscaled.
fn normal(points: &[[i64; 3]]) -> [i128; 3] {
    let mut n = [0i128; 3];
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        let (p, q) = (p.map(i128::from), q.map(i128::from));
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    n
}

fn dot(a: [i128; 3], b: [i64; 3]) -> i128 {
    a[0] * i128::from(b[0]) + a[1] * i128::from(b[1]) + a[2] * i128::from(b[2])
}

/// The loop turned so that its normal runs along `by` (`along`) or against it.
fn turned(points: &[[i64; 3]], by: [i64; 3], along: bool) -> Vec<[i64; 3]> {
    let mut points = points.to_vec();
    if (dot(normal(&points), by) > 0) != along {
        points.reverse();
    }
    points
}

/// Adds one triangle to a mesh, turned over for a piece taken away, with its unit normal.
fn triangle(mesh: &mut DisplayMesh, t: [[i64; 3]; 3], sign: i8) {
    let t = if sign > 0 { t } else { [t[0], t[2], t[1]] };
    let u = [0, 1, 2].map(|a| (t[1][a] - t[0][a]) as f64);
    let v = [0, 1, 2].map(|a| (t[2][a] - t[0][a]) as f64);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let base = mesh.positions.len() as u32;
    for p in t {
        mesh.positions.push(p.map(|c| c as f64));
        mesh.normals
            .push(n.map(|c| if len > 0.0 { c / len } else { 0.0 }));
    }
    mesh.triangles.push([base, base + 1, base + 2]);
}

/// The bounds of what a shape adds.
fn bounds(shape: &Shape) -> ([i64; 3], [i64; 3]) {
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for piece in shape.pieces.iter().filter(|p| p.sign > 0) {
        for p in &piece.outer {
            for q in [*p, add(*p, piece.by)] {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(q[axis]);
                    hi[axis] = hi[axis].max(q[axis]);
                }
            }
        }
    }
    (lo, hi)
}

impl Engine for Prisms {
    type Solid = Shape;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.prisms".to_owned(),
            version: "this test file".to_owned(),
            step: self.step,
            tolerances: vec![Tolerance {
                name: "none".to_owned(),
                value: 0.0,
                unit: "m".to_owned(),
            }],
            answers: vec![Answer {
                operation: "extrude".to_owned(),
                case: "every case".to_owned(),
                by: "the test engine".to_owned(),
            }],
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Shape, EngineError> {
        self.extrusions += 1;
        let corners = |l: &[engine_geometry::value::grid::P3]| -> Vec<[i64; 3]> {
            l.iter()
                .map(|p| [p.x.get(), p.y.get(), p.z.get()])
                .collect()
        };
        let outer = corners(&profile.outer);
        if outer.len() < 3 || by == [0, 0, 0] || dot(normal(&outer), by) == 0 {
            return Err(EngineError::Invalid {
                operation: "extrude",
                what: "a flat profile with an area, swept across its plane".to_owned(),
            });
        }
        Ok(Shape {
            pieces: vec![Piece {
                sign: 1,
                outer,
                holes: profile.holes.iter().map(|h| corners(h)).collect(),
                by,
            }],
        })
    }

    fn subtract(&mut self, target: &Shape, tool: &Shape) -> Result<Shape, EngineError> {
        let (tl, th) = bounds(target);
        let (cl, ch) = bounds(tool);
        if (0..3).any(|a| cl[a] < tl[a] || ch[a] > th[a]) {
            return Err(unsupported(
                "subtract",
                "a tool reaching outside its target",
            ));
        }
        let mut pieces = target.pieces.clone();
        pieces.extend(tool.pieces.iter().map(|p| Piece {
            sign: -p.sign,
            ..p.clone()
        }));
        Ok(Shape { pieces })
    }

    fn subtract_pieces(&mut self, _: &Shape, _: &Shape) -> Result<Vec<Shape>, EngineError> {
        Err(unsupported("subtract_pieces", "anything"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Shape, EngineError> {
        Err(unsupported("loft", "anything"))
    }

    fn unite(&mut self, a: &Shape, b: &Shape) -> Result<Shape, EngineError> {
        let (al, ah) = bounds(a);
        let (bl, bh) = bounds(b);
        if !(0..3).any(|x| ah[x] <= bl[x] || bh[x] <= al[x]) {
            return Err(unsupported("unite", "two solids sharing space"));
        }
        let mut pieces = a.pieces.clone();
        pieces.extend(b.pieces.iter().cloned());
        Ok(Shape { pieces })
    }

    fn volume(&mut self, _: &Shape) -> Result<Volume, EngineError> {
        Err(unsupported("volume", "anything: the suite reads the mesh"))
    }

    fn mesh(&mut self, shape: &Shape) -> Result<DisplayMesh, EngineError> {
        let mut mesh = DisplayMesh::default();
        for piece in &shape.pieces {
            let mut loops = vec![turned(&piece.outer, piece.by, true)];
            loops.extend(piece.holes.iter().map(|h| turned(h, piece.by, false)));
            for l in &loops {
                for i in 1..l.len() - 1 {
                    triangle(&mut mesh, [l[0], l[i + 1], l[i]], piece.sign);
                    let top = [l[0], l[i], l[i + 1]].map(|p| add(p, piece.by));
                    triangle(&mut mesh, top, piece.sign);
                }
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    let (a2, b2) = (add(a, piece.by), add(b, piece.by));
                    triangle(&mut mesh, [a, b, b2], piece.sign);
                    triangle(&mut mesh, [a, b2, a2], piece.sign);
                }
            }
        }
        Ok(mesh)
    }

    fn section(&mut self, _: &Shape, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section", "anything: evaluation never cuts a section"))
    }
}

// The suite, on the test engine.

#[test]
fn the_door_is_built_for_the_type_it_is_drawn_for() {
    evaluate_suite::the_door_is_built_for_the_type_it_is_drawn_for(Prisms::new);
}

#[test]
fn two_types_give_two_solids_of_their_sizes() {
    evaluate_suite::two_types_give_two_solids_of_their_sizes(Prisms::new);
}

#[test]
fn a_formula_change_moves_a_face() {
    evaluate_suite::a_formula_change_moves_a_face(Prisms::new);
}

#[test]
fn every_door_type_is_solved_to_its_size() {
    evaluate_suite::every_door_type_is_solved_to_its_size(Prisms::new);
}

#[test]
fn a_taller_type_moves_the_head() {
    evaluate_suite::a_taller_type_moves_the_head(Prisms::new);
}

#[test]
fn a_free_part_of_a_sketch_stays_as_drawn() {
    evaluate_suite::a_free_part_of_a_sketch_stays_as_drawn(Prisms::new);
}

#[test]
fn a_void_cuts_its_target() {
    evaluate_suite::a_void_cuts_its_target(Prisms::new);
}

#[test]
fn a_hole_runs_through_the_extrusion() {
    evaluate_suite::a_hole_runs_through_the_extrusion(Prisms::new);
}

#[test]
fn two_steps_unite_into_one_solid() {
    evaluate_suite::two_steps_unite_into_one_solid(Prisms::new);
}

/// The suite's own block recipe is a recipe every check passes, as the examples are.
#[test]
fn the_suites_block_passes_every_check() {
    let fixture = Fixture::new();
    let recipe = block(&json!([
        extrude("block", &BLOCK, &[]),
        extrude("void", &SQUARE, &[]),
        on_two("cut", "subtract", ("target", "block"), ("tool", "void")),
        extrude("side", &SIDE, &[]),
        on_two("whole", "unite", ("a", "cut"), ("b", "side")),
    ]));
    assert_eq!(recipe.check(&fixture.library()), Ok(()));
}

/// Every example evaluates for every type it names, and at its defaults, well within the
/// default limits.
#[test]
fn every_example_evaluates_for_every_type_it_names() {
    let mut evaluated = Vec::new();
    for (name, text) in EXAMPLES {
        let recipe = read(text);
        let mut evaluator = Evaluator::new(Prisms::new());
        for (ty, result) in evaluator.evaluate_types(&recipe) {
            let built = result.unwrap_or_else(|e| panic!("{name} {ty}: {e}"));
            assert!(!built.solids.is_empty(), "{name} {ty}");
            evaluated.push(format!("{name} {ty}"));
        }
        let defaults = evaluate(&mut Prisms::new(), &recipe, &TypeValues::new());
        assert!(defaults.is_ok(), "{name} at its defaults: {defaults:?}");
    }
    for ty in ["d720", "d820", "d920"] {
        assert!(
            evaluated.contains(&format!("single-swing-door {ty}")),
            "{evaluated:?}"
        );
    }
}

/// The same recipe and values give the same solids and the same counts on every run.
#[test]
fn solving_gives_the_same_answer_every_run() {
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    let first = evaluate(&mut Prisms::new(), &recipe, &values).unwrap();
    for _ in 0..3 {
        assert_eq!(
            evaluate(&mut Prisms::new(), &recipe, &values).unwrap(),
            first
        );
    }
    assert!(first.used.solver_steps > 0, "{:?}", first.used);
}

// Refusals: each with its place and reason.

fn refusal(recipe: &Recipe, values: &TypeValues) -> EvalError {
    evaluate(&mut Prisms::new(), recipe, values).unwrap_err()
}

/// The door with one change to its JSON.
fn door_with(change: impl FnOnce(&mut Value)) -> Recipe {
    let mut value: Value = serde_json::from_str(DOOR).unwrap();
    change(&mut value);
    Recipe::from_value(&value).unwrap()
}

#[test]
fn a_type_the_recipe_does_not_have_is_refused() {
    let e = TypeValues::of(&door(), &key("d999")).unwrap_err();
    assert_eq!(e.place(), &Place::Type(key("d999")));
    assert_eq!(e.reason(), &Reason::NoSuchType);
}

#[test]
fn a_value_for_no_parameter_is_refused_naming_it() {
    let e = refusal(&door(), &d820().with(key("depth"), length(100)));
    assert_eq!(e.place(), &Place::Parameter(key("depth")));
    assert_eq!(
        e.reason(),
        &Reason::Missing {
            sort: Sort::Parameter,
            key: "depth".into()
        }
    );
    assert_eq!(e.step(), None);
}

#[test]
fn a_type_cannot_set_what_each_instance_holds() {
    let e = refusal(&door(), &d820().with(key("swing"), "choice:right"));
    assert_eq!(e.place(), &Place::Parameter(key("swing")));
    assert_eq!(e.reason(), &Reason::HeldByInstance);
}

#[test]
fn a_type_cannot_set_what_a_formula_works_out() {
    let e = refusal(&door(), &d820().with(key("opening_width"), length(900)));
    assert_eq!(e.place(), &Place::Parameter(key("opening_width")));
    assert_eq!(e.reason(), &Reason::SetsFormula);
}

#[test]
fn a_value_not_in_its_one_spelling_or_kind_is_refused() {
    for text in ["length:820", "material:glass-plate", "12 mm"] {
        let e = refusal(&door(), &d820().with(key("width"), text));
        assert_eq!(e.place(), &Place::Parameter(key("width")), "{text}");
        assert!(matches!(e.reason(), Reason::BadValue(_)), "{text}: {e}");
    }
}

#[test]
fn a_parameters_formula_with_the_wrong_units_is_refused_naming_it() {
    let recipe = door_with(|v| v["parameters"][7]["formula"] = json!("width * frame"));
    let e = refusal(&recipe, &d820());
    assert_eq!(e.place(), &Place::Parameter(key("opening_width")));
    let Reason::Formula(f) = e.reason() else {
        panic!("{e}");
    };
    assert_eq!(f.code, "Units", "{f:?}");
}

#[test]
fn a_steps_formula_with_the_wrong_units_is_refused_naming_the_step() {
    let recipe = door_with(|v| v["steps"][0]["end"] = json!("frame_depth * frame"));
    let e = refusal(&recipe, &d820());
    assert_eq!(e.place(), &Place::Step(key("frame")));
    assert_eq!(e.step(), Some(&key("frame")));
    assert!(
        matches!(e.reason(), Reason::Formula(f) if f.code == "Units"),
        "{e}"
    );
    assert!(e.to_string().starts_with("step frame: "), "{e}");
}

#[test]
fn a_formula_that_reads_a_material_is_refused() {
    let recipe = door_with(|v| v["steps"][0]["end"] = json!("panel_material"));
    let e = refusal(&recipe, &d820());
    assert_eq!(
        e.reason(),
        &Reason::NotNumeric {
            key: "panel_material".into()
        }
    );
}

/// The door with one more constraint: its two fixed points 1000 mm apart are also to be the
/// door's width and 180 mm apart. That holds for the 820 mm type and for no other: the 720 mm
/// type is over-constrained, and is refused naming the sketch and that constraint.
#[test]
fn a_sketch_that_cannot_be_solved_at_a_types_values_is_refused_naming_the_constraint() {
    let recipe = door_with(|v| {
        v["sketches"][0]["constraints"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind": "distance", "a": "o", "b": "e", "length": "width + 180 mm"}));
    });
    assert_eq!(recipe.check(&Fixture::new().library()), Ok(()));
    let d820 = TypeValues::of(&recipe, &key("d820")).unwrap();
    assert!(evaluate(&mut Prisms::new(), &recipe, &d820).is_ok());
    let d720 = TypeValues::of(&recipe, &key("d720")).unwrap();
    let e = refusal(&recipe, &d720);
    assert_eq!(e.step(), Some(&key("frame")));
    assert_eq!(
        e.place(),
        &Place::Constraint {
            sketch: key("elevation"),
            index: 22
        }
    );
    assert_eq!(
        e.reason(),
        &Reason::NotSolved(SolveError::OverConstrained {
            conflicting: vec![ConstraintId(22)]
        })
    );
    let text = e.to_string();
    assert!(
        text.starts_with(
            "step frame: sketch elevation, constraint 22: the sketch cannot be solved"
        ),
        "{text}"
    );
}

#[test]
fn a_profile_that_is_not_a_loop_is_refused() {
    let recipe = door_with(|v| {
        v["steps"][0]["profile"].as_array_mut().unwrap().pop();
    });
    let e = refusal(&recipe, &d820());
    assert_eq!(e.place(), &Place::Step(key("frame")));
    assert_eq!(e.reason(), &Reason::NotALoop);
    let recipe = door_with(|v| v["steps"][0]["profile"][0] = json!("o"));
    assert!(matches!(
        refusal(&recipe, &d820()).reason(),
        Reason::WrongSort {
            wanted: Sort::Line,
            found: Sort::Point,
            ..
        }
    ));
}

#[test]
fn a_missing_sketch_is_refused_naming_it() {
    let recipe = door_with(|v| v["steps"][0]["sketch"] = json!("nothing"));
    let e = refusal(&recipe, &d820());
    assert_eq!(e.step(), Some(&key("frame")));
    assert_eq!(e.place(), &Place::Sketch(key("nothing")));
}

#[test]
fn a_point_not_in_its_stored_spelling_is_refused_naming_it() {
    let recipe = door_with(|v| v["sketches"][0]["points"][0]["x"] = json!("length:0"));
    let e = refusal(&recipe, &d820());
    assert_eq!(
        e.place(),
        &Place::Item {
            sketch: key("elevation"),
            item: key("o")
        }
    );
    assert!(matches!(e.reason(), Reason::NotALength { .. }));
}

#[test]
fn a_material_from_a_parameter_that_is_not_one_is_refused() {
    let recipe = door_with(|v| v["steps"][1]["material"] = json!({"parameter": "width"}));
    let e = refusal(&recipe, &d820());
    assert_eq!(e.place(), &Place::Step(key("panel")));
    assert_eq!(
        e.reason(),
        &Reason::NotAMaterial {
            key: "width".into()
        }
    );
}

#[test]
fn a_step_must_use_earlier_steps() {
    let later = block(&json!([
        on_two("cut", "subtract", ("target", "block"), ("tool", "cut")),
        extrude("block", &BLOCK, &[]),
    ]));
    let e = refusal(&later, &TypeValues::new());
    assert_eq!(e.place(), &Place::Step(key("cut")));
    assert_eq!(
        e.reason(),
        &Reason::NotEarlier {
            key: "block".into()
        }
    );
    let missing = block(&json!([
        extrude("block", &BLOCK, &[]),
        on_two("cut", "subtract", ("target", "block"), ("tool", "void")),
    ]));
    assert_eq!(
        refusal(&missing, &TypeValues::new()).reason(),
        &Reason::Missing {
            sort: Sort::Step,
            key: "void".into()
        }
    );
}

#[test]
fn an_extrusion_with_no_thickness_is_refused() {
    let recipe = block(&json!([extrude("block", &BLOCK, &[])]));
    let e = refusal(&recipe, &TypeValues::new().with(key("height"), length(0)));
    assert_eq!(e.place(), &Place::Step(key("block")));
    assert_eq!(e.reason(), &Reason::NoThickness);
}

/// Heights may be written either way round: the extrusion runs from the lower to the higher.
#[test]
fn heights_either_way_round_give_one_solid() {
    let mut up = extrude("block", &BLOCK, &[]);
    let mut down = up.clone();
    down["start"] = json!("height");
    down["end"] = json!("0 mm");
    up["end"] = json!("height");
    let a = evaluate(&mut Prisms::new(), &block(&json!([up])), &TypeValues::new()).unwrap();
    let b = evaluate(
        &mut Prisms::new(),
        &block(&json!([down])),
        &TypeValues::new(),
    )
    .unwrap();
    assert_eq!(a.solids[0].solid, b.solids[0].solid);
}

#[test]
fn the_engines_refusal_is_kept_with_its_step() {
    let recipe = block(&json!([
        extrude("block", &BLOCK, &[]),
        extrude("square", &SQUARE, &[]),
        on_two("whole", "unite", ("a", "block"), ("b", "square")),
    ]));
    let e = refusal(&recipe, &TypeValues::new());
    assert_eq!(e.place(), &Place::Step(key("whole")));
    assert_eq!(
        e.reason(),
        &Reason::Engine(unsupported("unite", "two solids sharing space"))
    );
    assert_eq!(
        e.to_string(),
        "step whole: the engine's unite does not answer two solids sharing space yet"
    );
}

#[test]
fn an_engine_on_another_grid_step_is_refused_before_any_step() {
    let mut engine = Prisms {
        step: GridStep::Q1,
        extrusions: 0,
    };
    let e = evaluate(&mut engine, &door(), &d820()).unwrap_err();
    assert_eq!(e.place(), &Place::Engine);
    assert_eq!(e.reason(), &Reason::WrongGrid { step: GridStep::Q1 });
    assert_eq!(engine.extrusions, 0);
}

// The cache per type.

#[test]
fn the_same_type_is_built_once() {
    let recipe = door();
    let mut evaluator = Evaluator::new(Prisms::new());
    let first = evaluator.evaluate(&recipe, &d820()).unwrap();
    let again = evaluator.evaluate(&recipe, &d820()).unwrap();
    assert!(Arc::ptr_eq(&first, &again));
    assert_eq!(evaluator.builds(), 1);
    assert_eq!(
        evaluator.engine().extrusions,
        2,
        "the frame and the panel, once"
    );
    // The defaults are the 820 mm type's values: the same key, the same solids.
    let defaults = evaluator.evaluate(&recipe, &TypeValues::new()).unwrap();
    assert!(Arc::ptr_eq(&first, &defaults));
    assert_eq!(evaluator.kept(), 1);
}

#[test]
fn a_changed_value_rebuilds_only_its_type() {
    let recipe = door();
    let mut evaluator = Evaluator::new(Prisms::new());
    let a = evaluator.evaluate(&recipe, &d820()).unwrap();
    let b = evaluator.evaluate(&recipe, &deep()).unwrap();
    assert_eq!((evaluator.builds(), evaluator.engine().extrusions), (2, 4));
    let thicker = deep().with(key("panel"), length(45));
    let c = evaluator.evaluate(&recipe, &thicker).unwrap();
    assert_eq!((evaluator.builds(), evaluator.engine().extrusions), (3, 6));
    assert_ne!(b.solids, c.solids);
    // The others are found as they were.
    assert!(Arc::ptr_eq(
        &a,
        &evaluator.evaluate(&recipe, &d820()).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &b,
        &evaluator.evaluate(&recipe, &deep()).unwrap()
    ));
    assert_eq!((evaluator.builds(), evaluator.engine().extrusions), (3, 6));
    // A sized solid is measured through the engine the evaluator holds.
    let frame = evaluate_suite::measure(evaluator.engine_mut(), &c.solids[0].solid);
    assert_eq!(frame, evaluate_suite::frame(120));
}

#[test]
fn the_key_is_the_recipe_version_and_the_values_read_in_recipe_order() {
    let recipe = door();
    let one_way = TypeValues::new()
        .with(key("panel"), length(40))
        .with(key("frame_depth"), length(120));
    let other_way = TypeValues::new()
        .with(key("frame_depth"), length(120))
        .with(key("panel"), length(40));
    let key_a = CacheKey::of(&recipe, &one_way);
    assert_eq!(key_a, CacheKey::of(&recipe, &other_way));
    assert_eq!(key_a, CacheKey::of(&recipe, &deep()));
    assert_eq!(key_a.recipe(), &recipe.content_id());
    let read: Vec<(&str, &str)> = key_a
        .values()
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        read,
        [
            ("width", "length:q0:5248000"),
            ("height", "length:q0:13056000"),
            ("frame", "length:q0:256000"),
            ("frame_depth", "length:q0:768000"),
            ("panel", "length:q0:256000"),
            ("panel_material", "material:timber-white-oak"),
            ("swing", "choice:left"),
        ]
    );
    // A value for a key the recipe has no such parameter for is part of the key, so it is never
    // found in place of the refusal it gets.
    let stray = deep().with(key("opening_width"), length(900));
    assert_ne!(CacheKey::of(&recipe, &stray), key_a);
}

#[test]
fn a_refusal_is_not_kept_and_a_new_recipe_version_is_built() {
    let recipe = door();
    let mut evaluator = Evaluator::new(Prisms::new());
    evaluator.evaluate(&recipe, &d820()).unwrap();
    let stray = d820().with(key("depth"), length(100));
    assert!(evaluator.evaluate(&recipe, &stray).is_err());
    assert_eq!((evaluator.builds(), evaluator.kept()), (1, 1));
    let changed = door_with(|v| v["steps"][1]["start"] = json!("frame_depth / 2 - 2 * panel"));
    evaluator.evaluate(&changed, &d820()).unwrap();
    assert_eq!((evaluator.builds(), evaluator.kept()), (2, 2));
    evaluator.forget(&recipe.content_id());
    assert_eq!(evaluator.kept(), 1);
    assert!(evaluator.has(&CacheKey::of(&changed, &d820())));
    assert!(!evaluator.has(&CacheKey::of(&recipe, &d820())));
}

// Named types and the type table.

/// The door with its type table (`common::door_table`).
fn door_with_table() -> Recipe {
    door_with(|v| v["type_table"] = door_table())
}

/// The door with its type table, after one more change to the table.
fn table_with(change: impl FnOnce(&mut Value)) -> Recipe {
    door_with(|v| {
        v["type_table"] = door_table();
        change(&mut v["type_table"]);
    })
}

/// A row is a type like a built-in one: its values, column by column, solved and built to its
/// size.
#[test]
fn a_type_table_row_is_evaluated_like_a_built_in_type() {
    let recipe = door_with_table();
    let values = TypeValues::of(&recipe, &key("d921")).unwrap();
    assert_eq!(values.get("width"), Some(length(920).as_str()));
    assert_eq!(values.get("height"), Some(length(2100).as_str()));
    let mut evaluator = Evaluator::new(Prisms::new());
    let built = evaluator.evaluate_type(&recipe, &key("d921")).unwrap();
    let frame = evaluate_suite::measure(evaluator.engine_mut(), &built.solids[0].solid);
    assert_eq!(frame, evaluate_suite::frame_of(920, 2100, 110));
    let d620 = evaluator.evaluate_type(&recipe, &key("d620")).unwrap();
    let frame = evaluate_suite::measure(evaluator.engine_mut(), &d620.solids[0].solid);
    assert_eq!(frame, evaluate_suite::frame_of(620, 2040, 110));
}

/// A table is evaluated row by row through the per-type cache: every type once, a row with a
/// built-in type's values shares its evaluation, and asking again builds nothing.
#[test]
fn a_type_table_is_built_once_per_type() {
    let recipe = door_with_table();
    let mut evaluator = Evaluator::new(Prisms::new());
    let all = evaluator.evaluate_types(&recipe);
    let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["d720", "d820", "d920", "d620", "d921", "d820t"]);
    assert!(all.iter().all(|(_, r)| r.is_ok()));
    assert_eq!((evaluator.builds(), evaluator.kept()), (5, 5));
    assert_eq!(evaluator.engine().extrusions, 10, "two for each type built");
    let d820 = all[1].1.as_ref().unwrap();
    let d820t = all[5].1.as_ref().unwrap();
    assert!(Arc::ptr_eq(d820, d820t));
    let again = evaluator.evaluate_types(&recipe);
    assert_eq!((evaluator.builds(), evaluator.engine().extrusions), (5, 10));
    for ((_, a), (_, b)) in all.iter().zip(&again) {
        assert!(Arc::ptr_eq(a.as_ref().unwrap(), b.as_ref().unwrap()));
    }
    // A changed type is the only one built again.
    let d921 = TypeValues::of(&recipe, &key("d921")).unwrap();
    evaluator
        .evaluate(&recipe, &d921.with(key("frame"), length(50)))
        .unwrap();
    assert_eq!((evaluator.builds(), evaluator.engine().extrusions), (6, 12));
}

/// One row's refusal does not stop the others.
#[test]
fn one_rows_refusal_leaves_the_others() {
    let recipe = table_with(|t| t["rows"][0]["values"][0] = json!("angle:405000000"));
    let mut evaluator = Evaluator::new(Prisms::new());
    let all = evaluator.evaluate_types(&recipe);
    let refused: Vec<&str> = all
        .iter()
        .filter(|(_, r)| r.is_err())
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(refused, ["d620"]);
    assert_eq!(evaluator.builds(), 4);
}

#[test]
fn a_row_without_a_value_names_the_type_and_the_column() {
    let recipe = table_with(|t| {
        t["rows"][0]["values"].as_array_mut().unwrap().pop();
    });
    let e = TypeValues::of(&recipe, &key("d620")).unwrap_err();
    assert_eq!(
        e.place(),
        &Place::TypeValue {
            ty: key("d620"),
            parameter: key("height")
        }
    );
    assert_eq!(e.reason(), &Reason::NoValue);
    assert_eq!(
        e.to_string(),
        "type d620, parameter height: the type table row has no value for it"
    );
    let recipe = table_with(|t| {
        t["rows"][0]["values"]
            .as_array_mut()
            .unwrap()
            .push(json!(length(1)));
    });
    let e = TypeValues::of(&recipe, &key("d620")).unwrap_err();
    assert_eq!(e.place(), &Place::Type(key("d620")));
    assert_eq!(
        e.reason(),
        &Reason::ExtraValues {
            values: 3,
            columns: 2
        }
    );
}

/// A value of another kind or units, or not in its one spelling, names the type, the parameter
/// and why, whether it is a row's or a built-in type's.
#[test]
fn a_wrong_unit_value_names_the_type_and_the_parameter() {
    for bad in ["angle:405000000", "length:620", "material:glass-plate"] {
        let recipe = table_with(|t| t["rows"][0]["values"][0] = json!(bad));
        let e = TypeValues::of(&recipe, &key("d620")).unwrap_err();
        assert_eq!(
            e.place(),
            &Place::TypeValue {
                ty: key("d620"),
                parameter: key("width")
            },
            "{bad}"
        );
        assert!(matches!(e.reason(), Reason::BadValue(_)), "{bad}: {e}");
        assert!(
            e.to_string().starts_with("type d620, parameter width: "),
            "{e}"
        );
    }
    let recipe = door_with(|v| v["types"][0]["values"][0]["value"] = json!("angle:405000000"));
    let e = TypeValues::of(&recipe, &key("d720")).unwrap_err();
    assert_eq!(
        e.place(),
        &Place::TypeValue {
            ty: key("d720"),
            parameter: key("width")
        }
    );
    let recipe = table_with(|t| t["columns"][1] = json!("swing"));
    let e = TypeValues::of(&recipe, &key("d620")).unwrap_err();
    assert_eq!(
        e.place(),
        &Place::TypeValue {
            ty: key("d620"),
            parameter: key("swing")
        }
    );
    assert_eq!(e.reason(), &Reason::HeldByInstance);
}

// The limits.

/// The door uses far less than every default limit: under a hundredth of each.
#[test]
fn the_door_passes_the_default_limits_easily() {
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    let used = evaluate(&mut Prisms::new(), &recipe, &values).unwrap().used;
    let limits = Limits::DEFAULT;
    assert_eq!(Limits::default(), limits);
    assert_eq!((used.solid_steps, used.corners), (2, 12));
    assert!(used.solver_steps > 0 && used.solver_steps * 100 < limits.solver_steps);
    assert!(used.formula_bytes > 0 && used.formula_bytes * 100 < limits.formula_bytes);
    assert!(u64::from(used.solid_steps) * 100 < u64::from(limits.solid_steps));
    assert!(used.corners * 100 < limits.corners);
}

fn within(limits: Limits) -> EvalError {
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    let e = evaluate_within(&mut Prisms::new(), &recipe, &values, limits).unwrap_err();
    // The same limit is reached at the same place on every run.
    assert_eq!(
        evaluate_within(&mut Prisms::new(), &recipe, &values, limits).unwrap_err(),
        e
    );
    e
}

#[test]
fn running_out_of_solver_steps_is_refused_at_the_sketch() {
    let mut limits = Limits::DEFAULT;
    limits.solver_steps = 1;
    let e = within(limits);
    assert_eq!(e.step(), Some(&key("frame")));
    assert_eq!(e.place(), &Place::Sketch(key("elevation")));
    assert_eq!(
        e.reason(),
        &Reason::LimitReached {
            limit: Limit::SolverSteps,
            allowed: 1
        }
    );
    assert_eq!(
        e.to_string(),
        "step frame: sketch elevation: evaluating it would pass the limit of 1 sketch solver \
         steps for one type"
    );
}

#[test]
fn too_many_solid_steps_are_refused_at_the_step() {
    let mut limits = Limits::DEFAULT;
    limits.solid_steps = 1;
    let e = within(limits);
    assert_eq!(e.place(), &Place::Step(key("panel")));
    assert_eq!(
        e.reason(),
        &Reason::LimitReached {
            limit: Limit::SolidSteps,
            allowed: 1
        }
    );
}

#[test]
fn too_many_corners_are_refused_at_the_step() {
    let mut limits = Limits::DEFAULT;
    limits.corners = 11;
    let e = within(limits);
    assert_eq!(e.place(), &Place::Step(key("panel")));
    assert_eq!(
        e.reason(),
        &Reason::LimitReached {
            limit: Limit::ResultSize,
            allowed: 11
        }
    );
    limits.corners = 12;
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    assert!(evaluate_within(&mut Prisms::new(), &recipe, &values, limits).is_ok());
}

#[test]
fn too_much_formula_text_is_refused_where_it_is_reached() {
    let mut limits = Limits::DEFAULT;
    limits.formula_bytes = 10;
    let e = within(limits);
    assert!(matches!(e.place(), Place::Parameter(_)), "{e}");
    assert_eq!(
        e.reason(),
        &Reason::LimitReached {
            limit: Limit::FormulaWork,
            allowed: 10
        }
    );
    // One byte short of what the door uses is refused; exactly what it uses is enough.
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    let used = evaluate(&mut Prisms::new(), &recipe, &values)
        .unwrap()
        .used
        .formula_bytes;
    limits.formula_bytes = used - 1;
    let e = within(limits);
    assert!(
        matches!(
            e.reason(),
            Reason::LimitReached {
                limit: Limit::FormulaWork,
                ..
            }
        ),
        "{e}"
    );
    limits.formula_bytes = used;
    assert!(evaluate_within(&mut Prisms::new(), &recipe, &values, limits).is_ok());
}

/// An evaluator keeps its limits for every type it builds, and a type refused by a limit is not
/// kept.
#[test]
fn an_evaluator_evaluates_within_its_limits() {
    let mut limits = Limits::DEFAULT;
    limits.solid_steps = 1;
    let mut evaluator = Evaluator::with_limits(Prisms::new(), limits);
    assert_eq!(evaluator.limits(), limits);
    let e = evaluator.evaluate(&door(), &d820()).unwrap_err();
    assert!(matches!(e.reason(), Reason::LimitReached { .. }));
    assert_eq!((evaluator.builds(), evaluator.kept()), (0, 0));
    assert_eq!(Evaluator::new(Prisms::new()).limits(), Limits::DEFAULT);
}
