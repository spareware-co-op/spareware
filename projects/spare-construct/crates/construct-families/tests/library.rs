//! The door and window templates of the family library (`projects/spare-construct/library/
//! doors-windows`, piece A62, CON-A7-F4-P2): each validates against the published schema, reads,
//! passes the checker and is in the one written form; every type of every type table evaluates,
//! within the default limits, to solids of the sizes the type gives (read back from the built
//! bounds); the plan and elevation symbols are where they belong at two sizes; and everything is
//! the same on a second run. Judged through this crate's types and the `Engine` trait only, on a
//! test engine of this file's own that keeps the bounds of what it builds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::Fixture;
use construct_families::{
    DrawnSymbol, Edge, Evaluated, Evaluator, Key, Recipe, SCHEMA, TypeValues, evaluate, symbols,
};
use construct_params::{Length, Value};
use engine_geometry::value::engine::{
    Answer, Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Tolerance,
    Volume,
};
use engine_geometry::value::grid::GridStep;
use std::collections::BTreeMap;

const DOOR: &str = include_str!("../../../library/doors-windows/single-swing-door.json");
const FIXED: &str = include_str!("../../../library/doors-windows/fixed-window.json");
const SLIDING: &str = include_str!("../../../library/doors-windows/sliding-window.json");

/// Every template, by its file name.
const TEMPLATES: [(&str, &str); 3] = [
    ("single-swing-door", DOOR),
    ("fixed-window", FIXED),
    ("sliding-window", SLIDING),
];

/// A solid of the test engine: the bounds of what was built, low and high corners, in grid
/// steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bounds {
    lo: [i64; 3],
    hi: [i64; 3],
}

/// A test engine keeping the bounds of each extrusion, a subtraction's target's bounds, and a
/// union's joint bounds. It is not a geometry engine; it reads sizes back.
struct Boxes;

fn unsupported(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything: this engine only keeps bounds".to_owned(),
    }
}

impl Engine for Boxes {
    type Solid = Bounds;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.boxes".to_owned(),
            version: "this test file".to_owned(),
            step: GridStep::Q0,
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

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Bounds, EngineError> {
        if profile.outer.len() < 3 || by == [0, 0, 0] {
            return Err(EngineError::Invalid {
                operation: "extrude",
                what: "a loop of three or more corners, swept".to_owned(),
            });
        }
        let mut lo = [i64::MAX; 3];
        let mut hi = [i64::MIN; 3];
        for p in &profile.outer {
            let p = [p.x.get(), p.y.get(), p.z.get()];
            for q in [p, [p[0] + by[0], p[1] + by[1], p[2] + by[2]]] {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(q[axis]);
                    hi[axis] = hi[axis].max(q[axis]);
                }
            }
        }
        Ok(Bounds { lo, hi })
    }

    fn subtract(&mut self, target: &Bounds, _: &Bounds) -> Result<Bounds, EngineError> {
        Ok(*target)
    }

    fn subtract_pieces(&mut self, _: &Bounds, _: &Bounds) -> Result<Vec<Bounds>, EngineError> {
        Err(unsupported("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Bounds, EngineError> {
        Err(unsupported("loft"))
    }

    fn unite(&mut self, a: &Bounds, b: &Bounds) -> Result<Bounds, EngineError> {
        Ok(Bounds {
            lo: [0, 1, 2].map(|i| a.lo[i].min(b.lo[i])),
            hi: [0, 1, 2].map(|i| a.hi[i].max(b.hi[i])),
        })
    }

    fn volume(&mut self, _: &Bounds) -> Result<Volume, EngineError> {
        Err(unsupported("volume"))
    }

    fn mesh(&mut self, _: &Bounds) -> Result<DisplayMesh, EngineError> {
        Err(unsupported("mesh"))
    }

    fn section(&mut self, _: &Bounds, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unsupported("section"))
    }
}

/// Whole millimetres in grid steps.
fn mm(mm: i64) -> i64 {
    Length::checked_from_mm(mm).unwrap().steps()
}

/// A box from x, y and z ranges in whole millimetres.
fn boxed(x: (i64, i64), y: (i64, i64), z: (i64, i64)) -> Bounds {
    Bounds {
        lo: [mm(x.0), mm(y.0), mm(z.0)],
        hi: [mm(x.1), mm(y.1), mm(z.1)],
    }
}

fn read(text: &str) -> Recipe {
    Recipe::from_json(text).unwrap()
}

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

/// A length parameter's value a type sets, in whole millimetres.
fn set_mm(values: &TypeValues, parameter: &str) -> i64 {
    match Value::from_stored(values.get(parameter).unwrap()) {
        Some(Value::Length(length)) => length.whole_mm().unwrap(),
        other => panic!("{parameter} is not a length: {other:?}"),
    }
}

/// Every solid's bounds, by its step.
fn sizes(built: &Evaluated<Bounds>) -> BTreeMap<String, Bounds> {
    built
        .solids
        .iter()
        .map(|b| (b.step.as_str().to_owned(), b.solid))
        .collect()
}

/// The solids a template's type should give, for a width and height in millimetres, from the
/// template's own defaults (frame, depths, thicknesses, sill), worked out here by hand.
fn expected(template: &str, w: i64, h: i64) -> BTreeMap<String, Bounds> {
    let mut out = BTreeMap::new();
    let mut put = |step: &str, b: Bounds| out.insert(step.to_owned(), b);
    match template {
        "single-swing-door" => {
            // Frame 40 wide and 100 deep; a 40 mm leaf flush with the face it opens towards.
            put("frame", boxed((-w / 2, w / 2), (-50, 50), (0, h)));
            put(
                "leaf",
                boxed((-w / 2 + 40, w / 2 - 40), (10, 50), (0, h - 40)),
            );
        }
        "fixed-window" => {
            // Frame 50 wide and 70 deep, 24 mm glass in its middle, the sill at 900.
            put("frame", boxed((-w / 2, w / 2), (-35, 35), (900, 900 + h)));
            put(
                "glass",
                boxed((-w / 2 + 50, w / 2 - 50), (-12, 12), (950, 850 + h)),
            );
        }
        "sliding-window" => {
            // Frame 50 wide and 100 deep; 50 mm sashes 40 deep on two tracks 10 apart, meeting
            // over one sash width at the middle; 20 mm glass in the middle of each sash.
            put("frame", boxed((-w / 2, w / 2), (-50, 50), (900, 900 + h)));
            put(
                "sash_fixed",
                boxed((-w / 2 + 50, 25), (-45, -5), (950, 850 + h)),
            );
            put(
                "glass_fixed",
                boxed((-w / 2 + 100, -25), (-35, -15), (1000, 800 + h)),
            );
            put(
                "sash_sliding",
                boxed((-25, w / 2 - 50), (5, 45), (950, 850 + h)),
            );
            put(
                "glass_sliding",
                boxed((25, w / 2 - 100), (15, 35), (1000, 800 + h)),
            );
        }
        other => panic!("no expectation for {other}"),
    }
    out
}

#[test]
fn every_template_validates_reads_checks_and_is_written_in_its_one_form() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let fixture = Fixture::new();
    for (name, text) in TEMPLATES {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
        let recipe = read(text);
        assert_eq!(recipe.check(&fixture.library()), Ok(()), "{name}");
        assert_eq!(recipe.to_json(), text, "{name} is not in the written form");
        assert_eq!(
            value["id"],
            format!("library.doors-windows.{name}"),
            "{name}: the id follows the naming rule"
        );
        assert!(
            recipe.named_types().len() >= 6,
            "{name} has a table of sizes"
        );
    }
}

#[test]
fn every_type_of_every_table_builds_to_its_sizes_within_the_default_limits() {
    for (name, text) in TEMPLATES {
        let recipe = read(text);
        for ty in recipe.named_types() {
            let values = TypeValues::of(&recipe, ty.key()).unwrap();
            let (w, h) = (set_mm(&values, "width"), set_mm(&values, "height"));
            let built = evaluate(&mut Boxes, &recipe, &values)
                .unwrap_or_else(|e| panic!("{name} {}: {e}", ty.key()));
            assert_eq!(sizes(&built), expected(name, w, h), "{name} {}", ty.key());
            // Well inside the default limits.
            assert!(built.used.solver_steps < 400, "{name}: {:?}", built.used);
            assert!(built.used.solid_steps < 100, "{name}: {:?}", built.used);
        }
    }
}

/// A symbol by key.
fn symbol<'s>(all: &'s [DrawnSymbol], key: &str) -> &'s DrawnSymbol {
    all.iter().find(|s| s.symbol.as_str() == key).unwrap()
}

/// A point's two coordinates in whole millimetres.
fn at(p: [Length; 2]) -> (i64, i64) {
    (p[0].whole_mm().unwrap(), p[1].whole_mm().unwrap())
}

/// A point in whole millimetres.
type Mm = (i64, i64);

/// Every line of a symbol, as end points in whole millimetres.
fn lines(s: &DrawnSymbol) -> Vec<(&str, Mm, Mm)> {
    s.edges
        .iter()
        .filter_map(|e| match e {
            Edge::Line { key, start, end } => Some((key.as_str(), at(*start), at(*end))),
            _ => None,
        })
        .collect()
}

#[test]
fn the_door_swing_arc_is_the_leaf_width_and_the_marks_point_to_the_hinges_at_two_sizes() {
    let recipe = read(DOOR);
    for (ty, w, h) in [("d700x2100", 700, 2100), ("d1000x2400", 1000, 2400)] {
        let values = TypeValues::of(&recipe, &key(ty)).unwrap();
        let drawn = symbols(&recipe, &values).unwrap().symbols;
        let leaf = w - 2 * 40;
        let jamb = w / 2 - 40;
        // Hinged on the left: the arc about the hinge at the left jamb's face, from the closed
        // leaf's edge at the right jamb round to the open leaf, a quarter turn.
        for (key, hinge, latch) in [("swing_left", -jamb, jamb), ("swing_right", jamb, -jamb)] {
            let s = symbol(&drawn, key);
            let arc = s
                .edges
                .iter()
                .find_map(|e| match e {
                    Edge::Arc {
                        centre,
                        start,
                        end,
                        radius,
                        ..
                    } => Some((at(*centre), at(*start), at(*end), *radius)),
                    _ => None,
                })
                .unwrap();
            let (centre, start, end, radius) = arc;
            assert_eq!(
                radius.whole_mm(),
                Some(leaf),
                "{ty} {key}: radius is the leaf"
            );
            assert_eq!(centre, (hinge, 50), "{ty} {key}");
            let (closed, open) = if hinge < 0 {
                (start, end)
            } else {
                (end, start)
            };
            assert_eq!(closed, (latch, 50), "{ty} {key}");
            assert_eq!(open, (hinge, 50 + leaf), "{ty} {key}");
            let leaf_line = lines(s);
            assert_eq!(
                leaf_line,
                vec![(leaf_line[0].0, (hinge, 50), (hinge, 50 + leaf))]
            );
            assert!(s.when.is_some());
        }
        // The elevation marks meet at the hinge side, half way up the leaf.
        for (key, hinge, latch) in [("mark_left", -jamb, jamb), ("mark_right", jamb, -jamb)] {
            let marks = lines(symbol(&drawn, key));
            let apex = (hinge, (h - 40) / 2);
            assert_eq!(marks.len(), 2);
            assert_eq!(marks[0].1, (latch, 0), "{ty} {key}");
            assert_eq!(marks[0].2, apex, "{ty} {key}");
            assert_eq!(marks[1].1, apex, "{ty} {key}");
            assert_eq!(marks[1].2, (latch, h - 40), "{ty} {key}");
        }
        // The jambs in plan: the frame's width and depth at each side.
        for (_, a, b) in lines(symbol(&drawn, "jambs")) {
            for (x, y) in [a, b] {
                assert!(x.abs() == w / 2 || x.abs() == jamb, "{ty}: {x}");
                assert!(y.abs() == 50, "{ty}: {y}");
            }
        }
    }
}

#[test]
fn the_window_glass_and_sash_lines_sit_inside_the_frame_at_two_sizes() {
    let cases = [
        (
            "fixed-window",
            FIXED,
            [("w600x600", 600), ("w1800x1200", 1800)],
            35,
        ),
        (
            "sliding-window",
            SLIDING,
            [("s1200x900", 1200), ("s2400x1200", 2400)],
            50,
        ),
    ];
    for (name, text, types, half_depth) in cases {
        let recipe = read(text);
        for (ty, w) in types {
            let values = TypeValues::of(&recipe, &key(ty)).unwrap();
            let drawn = symbols(&recipe, &values).unwrap().symbols;
            let inner = w / 2 - 50;
            let plan = lines(symbol(&drawn, "plan_lines"));
            let mut glass = 0;
            for (line, a, b) in &plan {
                match *line {
                    "face_out" | "face_in" => {
                        assert_eq!(a.0.abs(), w / 2, "{name} {ty} {line}");
                        assert_eq!(a.1.abs(), half_depth, "{name} {ty} {line}");
                        assert_eq!(a.1, b.1, "{name} {ty} {line}");
                    }
                    "end_left" | "end_right" | "jamb_left" | "jamb_right" => {
                        assert_eq!(a.0, b.0, "{name} {ty} {line}");
                    }
                    _ => {
                        glass += 1;
                        for (x, y) in [a, b] {
                            assert!(x.abs() <= inner, "{name} {ty} {line}: {x}");
                            assert!(y.abs() < half_depth, "{name} {ty} {line}: {y}");
                        }
                        assert_eq!(a.1, b.1, "{name} {ty} {line}: parallel to the wall");
                    }
                }
            }
            if name == "fixed-window" {
                assert_eq!(glass, 1);
                assert!(plan.contains(&("glass_line", (-inner, 0), (inner, 0))));
            } else {
                assert_eq!(glass, 2);
                assert!(plan.contains(&("sash_line_fixed", (-inner, -25), (25, -25))));
                assert!(plan.contains(&("sash_line_sliding", (-25, 25), (inner, 25))));
                // The arrow lies on the sliding sash's glass and points to the fixed sash.
                let arrow = lines(symbol(&drawn, "slide_mark"));
                let (_, tail, tip) = arrow[0];
                assert!(tip.0 < tail.0, "{ty}: it points left");
                for (x, _) in arrow.iter().flat_map(|(_, a, b)| [*a, *b]) {
                    assert!(x > 25 && x < inner - 50, "{ty}: {x}");
                }
            }
        }
    }
}

#[test]
fn the_same_templates_give_the_same_bytes_and_shapes_on_a_second_run() {
    for (name, text) in TEMPLATES {
        let first = read(text);
        let second = read(&first.to_json());
        assert_eq!(first.to_json(), second.to_json(), "{name}");
        assert_eq!(first.content_id(), second.content_id(), "{name}");
        let mut a = Evaluator::new(Boxes);
        let mut b = Evaluator::new(Boxes);
        let one = a.evaluate_types(&first);
        let two = b.evaluate_types(&second);
        assert_eq!(one, two, "{name}");
        assert!(one.iter().all(|(_, r)| r.is_ok()), "{name}");
        for ty in first.named_types() {
            let values = TypeValues::of(&first, ty.key()).unwrap();
            assert_eq!(
                symbols(&first, &values),
                symbols(&second, &values),
                "{name}"
            );
        }
    }
}
