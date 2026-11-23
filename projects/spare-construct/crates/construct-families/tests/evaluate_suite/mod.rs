//! The evaluation suite: what a family recipe evaluated through the kernel's engine seam must
//! give, written over `engine_geometry`'s `Engine` trait and this crate's own types alone, so any
//! engine is judged by the same cases. A test file runs an engine through it by calling each case
//! with a way to make a fresh engine: this crate's `tests/evaluate.rs` with a test engine of its
//! own; a program that links a real engine (the server, under its `kernel` feature) includes this
//! file through `#[path]` and calls the same cases with it.
//!
//! Every size is read from the engine's display mesh (its corners' bounds, and six times the
//! volume it encloses, from exact integer products of its grid positions), and checked against
//! the engine's own volume where it gives one. The cases: the single-swing door example at its
//! 820 by 2040 mm type, at a deeper type of this file's own, and with a changed formula; a block
//! cut by a void; a block with a hole; two blocks united.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test code; each test file that includes the suite runs the cases it needs"
)]

use construct_families::{Evaluated, Key, Recipe, TypeValues, evaluate};
use engine_geometry::value::engine::{Engine, Volume};
use serde_json::{Value, json};

/// The single-swing door example recipe.
pub const DOOR: &str = include_str!("../../recipes/single-swing-door.json");

/// Grid steps in one millimetre.
pub const MM: i64 = 6_400;

pub fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

pub fn door() -> Recipe {
    Recipe::from_json(DOOR).unwrap()
}

/// A length's stored text, from whole millimetres.
pub fn length(mm: i64) -> String {
    format!("length:q0:{}", mm * MM)
}

/// The door's 820 by 2040 mm type, the one its sketches are drawn for.
pub fn d820() -> TypeValues {
    TypeValues::of(&door(), &key("d820")).unwrap()
}

/// A type of this suite's own: the 820 mm door with a 120 mm deep frame and a 40 mm panel. Its
/// values do not move the sketch (only the extrusions' heights read them).
pub fn deep() -> TypeValues {
    d820()
        .with(key("frame_depth"), length(120))
        .with(key("panel"), length(40))
}

/// A solid's size as its mesh gives it: its bounds and six times its volume, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub lo: [i64; 3],
    pub hi: [i64; 3],
    pub six_volume: i128,
}

/// A box's size from its corners and its volume, in millimetres.
pub fn size_mm(lo: [i64; 3], hi: [i64; 3], volume_mm3: i128) -> Size {
    let step = i128::from(MM);
    Size {
        lo: lo.map(|v| v * MM),
        hi: hi.map(|v| v * MM),
        six_volume: 6 * volume_mm3 * step * step * step,
    }
}

/// The size of a solid, read from the engine's mesh, and checked against the engine's volume
/// when the engine gives one.
pub fn measure<E: Engine>(engine: &mut E, solid: &E::Solid) -> Size {
    let mesh = engine.mesh(solid).unwrap();
    let points: Vec<[i64; 3]> = mesh
        .positions
        .iter()
        .map(|p| {
            p.map(|c| {
                assert_eq!(c, c.round(), "a position off the grid: {p:?}");
                c as i64
            })
        })
        .collect();
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for p in &points {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    let mut six: i128 = 0;
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| points[i as usize].map(i128::from));
        let cross = [
            b[1] * c[2] - b[2] * c[1],
            b[2] * c[0] - b[0] * c[2],
            b[0] * c[1] - b[1] * c[0],
        ];
        six += a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2];
    }
    if let Ok(Volume::SixTimes { cubic_steps, .. }) = engine.volume(solid) {
        assert_eq!(
            cubic_steps.to_i128(),
            Some(six),
            "the engine's volume and its mesh's"
        );
    }
    Size {
        lo,
        hi,
        six_volume: six,
    }
}

/// The step keys of an evaluation's solids, in order.
pub fn steps<S>(evaluated: &Evaluated<S>) -> Vec<&str> {
    evaluated.solids.iter().map(|b| b.step.as_str()).collect()
}

/// The size of the solid a step made.
pub fn size_of<E: Engine>(engine: &mut E, evaluated: &Evaluated<E::Solid>, step: &str) -> Size {
    let built = evaluated
        .solids
        .iter()
        .find(|b| b.step.as_str() == step)
        .unwrap_or_else(|| panic!("no solid {step}"));
    measure(engine, &built.solid)
}

/// The door's frame: 900 by 2080 mm outside, 820 by 2040 mm inside, open at the floor,
/// `depth` mm deep about the wall's centre.
pub fn frame(depth: i64) -> Size {
    let half = depth / 2;
    size_mm(
        [-450, -half, 0],
        [450, half, 2080],
        i128::from(900 * 2080 - 820 * 2040) * i128::from(depth),
    )
}

/// The door's panel: 820 by 2040 mm, from `back` to `front` mm across the wall.
pub fn panel(back: i64, front: i64) -> Size {
    size_mm(
        [-410, back, 0],
        [410, front, 2040],
        i128::from(820 * 2040) * i128::from(front - back),
    )
}

/// The door at the type it is drawn for: its frame and its panel, each of its own size,
/// subcategory and material, with every parameter's value.
pub fn the_door_is_built_for_the_type_it_is_drawn_for<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let built = evaluate(&mut engine, &door(), &d820()).unwrap();
    assert_eq!(steps(&built), ["frame", "panel"]);
    assert_eq!(size_of(&mut engine, &built, "frame"), frame(110));
    assert_eq!(size_of(&mut engine, &built, "panel"), panel(20, 55));
    let frame_solid = &built.solids[0];
    assert_eq!(frame_solid.subcategory.0, 302);
    assert_eq!(frame_solid.material.as_str(), "timber-douglas-fir");
    assert!(!frame_solid.visibility.coarse && frame_solid.visibility.fine);
    let panel_solid = &built.solids[1];
    assert_eq!(panel_solid.subcategory.0, 301);
    assert_eq!(panel_solid.material.as_str(), "timber-white-oak");
    let value = |k: &str| {
        built
            .values
            .iter()
            .find(|(key, _)| key.as_str() == k)
            .map(|(_, v)| v.stored())
            .unwrap()
    };
    assert_eq!(value("opening_width"), length(900));
    assert_eq!(value("opening_height"), length(2080));
    assert_eq!(value("swing"), "choice:left");
    assert_eq!(built.values.len(), 9, "every parameter, in recipe order");
}

/// Two types give two solids, each of its own size.
pub fn two_types_give_two_solids_of_their_sizes<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let recipe = door();
    let a = evaluate(&mut engine, &recipe, &d820()).unwrap();
    let b = evaluate(&mut engine, &recipe, &deep()).unwrap();
    let (fa, fb) = (
        size_of(&mut engine, &a, "frame"),
        size_of(&mut engine, &b, "frame"),
    );
    assert_eq!(fa, frame(110));
    assert_eq!(fb, frame(120));
    assert_ne!(fa, fb);
    assert_eq!(size_of(&mut engine, &b, "panel"), panel(20, 60));
}

/// A changed formula moves the face it places: the panel twice as thick, towards the back.
pub fn a_formula_change_moves_a_face<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let mut value: Value = serde_json::from_str(DOOR).unwrap();
    value["steps"][1]["start"] = json!("frame_depth / 2 - 2 * panel");
    let changed = Recipe::from_value(&value).unwrap();
    assert_ne!(changed.content_id(), door().content_id());
    let built = evaluate(&mut engine, &changed, &d820()).unwrap();
    assert_eq!(size_of(&mut engine, &built, "panel"), panel(-15, 55));
    assert_eq!(size_of(&mut engine, &built, "frame"), frame(110));
}

/// A block family of this suite's own: a 1000 by 600 mm plan with a 200 by 200 mm square inside
/// it and a 500 by 600 mm plan beside it, on the floor plane, and the steps given.
pub fn block(steps: &Value) -> Recipe {
    let p = |key: &str, x: i64, y: i64| json!({"key": key, "x": length(x), "y": length(y)});
    let l = |key: &str, start: &str, end: &str| json!({"key": key, "start": start, "end": end});
    Recipe::from_value(&json!({
        "format": 1,
        "id": "01KAXF3M1Y0TE5TB10CK000001",
        "name": "Test block",
        "category": 1700,
        "parameters": [{
            "key": "height",
            "definition": {
                "id": "01KAXF3M1Y0TE5TB10CK000001.height",
                "name": "Height",
                "kind": "length",
                "holder": "type",
                "choices": []
            },
            "default": length(900)
        }],
        "planes": [{"key": "floor", "base": "xy", "offset": "0 mm"}],
        "sketches": [{
            "key": "plan",
            "plane": "floor",
            "points": [
                p("a", 0, 0), p("b", 1000, 0), p("c", 1000, 600), p("d", 0, 600),
                p("h1", 400, 200), p("h2", 600, 200), p("h3", 600, 400), p("h4", 400, 400),
                p("f", 1500, 0), p("g", 1500, 600)
            ],
            "lines": [
                l("bottom", "a", "b"), l("right", "b", "c"), l("top", "c", "d"), l("left", "d", "a"),
                l("hb", "h1", "h2"), l("hr", "h2", "h3"), l("ht", "h3", "h4"), l("hl", "h4", "h1"),
                l("sb", "b", "f"), l("sr", "f", "g"), l("st", "g", "c")
            ],
            "arcs": [],
            "circles": [],
            "constraints": [
                {"kind": "fixed", "point": "a"},
                {"kind": "horizontal", "line": "bottom"},
                {"kind": "distance", "a": "a", "b": "b", "length": "1000 mm"}
            ]
        }],
        "steps": steps,
        "symbols": [],
        "hosting": {"host": "unhosted"},
        "types": []
    }))
    .unwrap()
}

/// An extrusion step of the block's plan, from the floor to the block's height.
pub fn extrude(key: &str, profile: &[&str], holes: &[&[&str]]) -> Value {
    json!({
        "key": key,
        "op": "extrude",
        "sketch": "plan",
        "profile": profile,
        "holes": holes,
        "start": "0 mm",
        "end": "height",
        "subcategory": 1700,
        "material": {"fixed": "timber-white-oak"},
        "visibility": {"coarse": true, "medium": true, "fine": true}
    })
}

/// A step on two earlier steps.
pub fn on_two(key: &str, op: &str, first: (&str, &str), second: (&str, &str)) -> Value {
    let mut step = json!({
        "key": key,
        "op": op,
        "subcategory": 1700,
        "material": {"fixed": "timber-white-oak"},
        "visibility": {"coarse": true, "medium": true, "fine": true}
    });
    step[first.0] = json!(first.1);
    step[second.0] = json!(second.1);
    step
}

pub const BLOCK: [&str; 4] = ["bottom", "right", "top", "left"];
pub const SQUARE: [&str; 4] = ["hb", "hr", "ht", "hl"];
pub const SIDE: [&str; 4] = ["sb", "sr", "st", "right"];

/// An extrusion used as a subtraction's tool is a void: it cuts its target, and neither is drawn
/// on its own.
pub fn a_void_cuts_its_target<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let recipe = block(&json!([
        extrude("block", &BLOCK, &[]),
        extrude("void", &SQUARE, &[]),
        on_two("cut", "subtract", ("target", "block"), ("tool", "void")),
    ]));
    let built = evaluate(&mut engine, &recipe, &TypeValues::new()).unwrap();
    assert_eq!(steps(&built), ["cut"]);
    let cut = size_of(&mut engine, &built, "cut");
    assert_eq!(
        cut,
        size_mm([0, 0, 0], [1000, 600, 900], (1000 * 600 - 200 * 200) * 900)
    );
}

/// A profile's holes are cut through the whole extrusion.
pub fn a_hole_runs_through_the_extrusion<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let recipe = block(&json!([extrude("block", &BLOCK, &[&SQUARE])]));
    let built = evaluate(&mut engine, &recipe, &TypeValues::new()).unwrap();
    assert_eq!(
        size_of(&mut engine, &built, "block"),
        size_mm([0, 0, 0], [1000, 600, 900], (1000 * 600 - 200 * 200) * 900)
    );
}

/// Two steps united are one solid, and only it is drawn.
pub fn two_steps_unite_into_one_solid<E: Engine>(make: impl Fn() -> E) {
    let mut engine = make();
    let recipe = block(&json!([
        extrude("block", &BLOCK, &[]),
        extrude("side", &SIDE, &[]),
        on_two("whole", "unite", ("a", "block"), ("b", "side")),
    ]));
    let values = TypeValues::new().with(key("height"), length(750));
    let built = evaluate(&mut engine, &recipe, &values).unwrap();
    assert_eq!(steps(&built), ["whole"]);
    assert_eq!(
        size_of(&mut engine, &built, "whole"),
        size_mm([0, 0, 0], [1500, 600, 750], 1500 * 600 * 750)
    );
}
