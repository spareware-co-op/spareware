//! Recipe evaluation (CON-A7-F1-P2, first half): the evaluation suite (`evaluate_suite`) on a test
//! engine of this file's own, then every refusal, each with its place and reason, and the cache
//! per type. Everything is judged through this crate's types and the `Engine` trait only.

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

use common::{EXAMPLES, Fixture, read};
use construct_families::{
    CacheKey, EvalError, Evaluator, Place, Reason, Recipe, Sort, TypeValues, evaluate,
};
use construct_solver::{ConstraintId, SolveError};
use engine_geometry::value::engine::{
    Answer, DisplayMesh, Edition, Engine, EngineError, Profile, Section, Tolerance, Volume,
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

/// Every example evaluates for every built-in type its sketches are drawn for, and the others are
/// refused as not solved.
#[test]
fn every_example_evaluates_where_its_sketches_are_drawn() {
    let mut evaluated = Vec::new();
    for (name, text) in EXAMPLES {
        let recipe = read(text);
        for ty in &recipe.types {
            let values = TypeValues::of(&recipe, &ty.key).unwrap();
            match evaluate(&mut Prisms::new(), &recipe, &values) {
                Ok(built) => {
                    assert!(!built.solids.is_empty(), "{name} {}", ty.key);
                    evaluated.push(format!("{name} {}", ty.key));
                }
                Err(e) => assert!(
                    matches!(e.reason(), Reason::NotSolved(_)),
                    "{name} {}: {e}",
                    ty.key
                ),
            }
        }
        let defaults = evaluate(&mut Prisms::new(), &recipe, &TypeValues::new());
        assert!(defaults.is_ok(), "{name} at its defaults: {defaults:?}");
    }
    assert!(
        evaluated.contains(&"single-swing-door d820".to_owned()),
        "{evaluated:?}"
    );
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

/// The 720 mm door moves its elevation sketch: this half takes sketches as stored, so it is
/// refused at the first constraint the stored sketch does not meet (the 360 mm half width, drawn
/// 410 mm), saying solving is not built yet.
#[test]
fn a_type_that_moves_a_sketch_is_refused_as_not_solved() {
    let recipe = door();
    let values = TypeValues::of(&recipe, &key("d720")).unwrap();
    let e = refusal(&recipe, &values);
    assert_eq!(e.step(), Some(&key("frame")));
    assert_eq!(
        e.place(),
        &Place::Constraint {
            sketch: key("elevation"),
            index: 6
        }
    );
    assert_eq!(
        e.reason(),
        &Reason::NotSolved(SolveError::RoundingRefused {
            constraint: ConstraintId(6),
            residual_mm: 50.0
        })
    );
    let text = e.to_string();
    assert!(
        text.starts_with("step frame: sketch elevation, constraint 6: "),
        "{text}"
    );
    assert!(text.contains("not built yet"), "{text}");
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
    let d720 = TypeValues::of(&recipe, &key("d720")).unwrap();
    assert!(evaluator.evaluate(&recipe, &d720).is_err());
    assert_eq!((evaluator.builds(), evaluator.kept()), (1, 1));
    let changed = door_with(|v| v["steps"][1]["start"] = json!("frame_depth / 2 - 2 * panel"));
    evaluator.evaluate(&changed, &d820()).unwrap();
    assert_eq!((evaluator.builds(), evaluator.kept()), (2, 2));
    evaluator.forget(&recipe.content_id());
    assert_eq!(evaluator.kept(), 1);
    assert!(evaluator.has(&CacheKey::of(&changed, &d820())));
    assert!(!evaluator.has(&CacheKey::of(&recipe, &d820())));
}
