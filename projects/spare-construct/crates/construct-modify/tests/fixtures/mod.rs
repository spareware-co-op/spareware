//! Records for the modify tests, each made by the crate that owns its form (or, for a layered wall
//! and the boxes, as their commands write them), and a [`Sense`] that answers from fixed values
//! and the library the server places families from.

// The crate denies these outside tests (Cargo.toml); a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    dead_code,
    reason = "test code; each test file uses its own part of this module"
)]

use construct_families::Key;
use construct_grids::{Grid, PlaneShape, Point as GridPoint, ReferencePlane, Shape};
use construct_levels::Tie;
use construct_model::element::{Element, Placement};
use construct_model::geometry::{FaceName, Recipe};
use construct_modify::{Hand, Sense, hands_of};
use construct_params::{Angle, Length, Value};
use construct_regen::families::{self, FamilySource, Instance, Library};
use construct_regen::hosting;
use construct_roofs::Roof;
use construct_rooms::Limit;
use construct_slabs::{Boundary, Corner};
use construct_solver::Point;
use kit_core::Id;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

/// Steps in a millimetre.
pub const MM: i64 = Length::STEPS_PER_MM;

pub fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

pub fn doc() -> Id {
    i("doc")
}

pub fn mm(v: i64) -> Length {
    Length::from_steps(v * MM)
}

pub fn degrees(d: i64) -> Angle {
    Angle::from_units(d * Angle::UNITS_PER_DEGREE)
}

pub fn stored_point([x, y]: [i64; 2]) -> Json {
    json!([Value::Length(mm(x)).stored(), Value::Length(mm(y)).stored()])
}

/// A record's point as a layered wall's recipe holds it, in grid steps.
pub fn point_of(record: &Element, arg: &str) -> [i64; 2] {
    let pair = record.recipe.args[arg].as_array().unwrap();
    let read = |v: &Json| match Value::from_stored(v.as_str().unwrap()) {
        Some(Value::Length(l)) => l.steps(),
        other => panic!("{other:?}"),
    };
    [read(&pair[0]), read(&pair[1])]
}

fn props(category: &str) -> BTreeMap<String, String> {
    let mut props = BTreeMap::new();
    props.insert("@category".to_owned(), category.to_owned());
    props
}

/// A layered wall as `wall.draw` writes it, from `start` to `end` in millimetres.
pub fn wall(id: &str, start: [i64; 2], end: [i64; 2]) -> Element {
    let mut args = BTreeMap::new();
    args.insert("start".to_owned(), stored_point(start));
    args.insert("end".to_owned(), stored_point(end));
    args.insert("height".to_owned(), json!(Value::Length(mm(2700)).stored()));
    args.insert("layers".to_owned(), construct_regen::walls::type_arg());
    let mut props = props("100");
    props.insert("@type".to_owned(), "wall-partition-stud-90".to_owned());
    Element {
        id: i(id),
        document: doc(),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("wall.layered", args),
        props,
        references: Vec::new(),
    }
}

/// The exterior face of a wall, as a hosted element names it.
pub fn exterior() -> FaceName {
    FaceName {
        name: "exterior".to_owned(),
        role: "exterior".to_owned(),
    }
}

/// A box door hosted on `host`'s exterior, `along` millimetres from its start.
pub fn door(id: &str, host: &str, along: i64) -> Element {
    let args = json!({ "width": 900, "thickness": 90, "height": 2100 });
    let args: BTreeMap<String, Json> = serde_json::from_value(args).unwrap();
    let mut door = Element {
        id: i(id),
        document: doc(),
        kind: "door".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("door.single", args),
        props: props("300"),
        references: Vec::new(),
    };
    hosting::write(&mut door, &i(host), &exterior(), mm(along), mm(0));
    door
}

/// An older door, hosted by its position.
pub fn old_door(id: &str, host: &str) -> Element {
    let mut door = door(id, host, 0);
    door.props.remove(hosting::ALONG_KEY);
    door.props.remove(hosting::UP_KEY);
    door
}

/// The library's single swing door, by content id and a type of it.
pub fn swing_door_recipe() -> Recipe {
    let library = Library::baked();
    let (content, recipe) = library
        .family("library.doors-windows.single-swing-door")
        .unwrap();
    let ty = recipe.type_table.as_ref().unwrap().rows[0].key.clone();
    families::recipe(&content, &ty)
}

/// A placed single swing door on `host`'s exterior, `along` millimetres to its middle.
pub fn swing_door(id: &str, host: &str, along: i64) -> Element {
    let mut door = Element {
        id: i(id),
        document: doc(),
        kind: "door".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: swing_door_recipe(),
        props: props("300"),
        references: Vec::new(),
    };
    hosting::write(&mut door, &i(host), &exterior(), mm(along), mm(0));
    door
}

/// A wall opening 1 m by 1 m on `host`, along x or y.
pub fn wall_opening(id: &str, host: &str, along: i64, along_y: bool) -> Element {
    let named = construct_openings::named(id, &Recipe::construct("opening.wall", BTreeMap::new()));
    let opening =
        construct_openings::WallOpening::new(&named, mm(1000), mm(1000), along_y).unwrap();
    let mut record = construct_openings::wall_opening_record(&i(id), &doc(), id, &opening);
    hosting::write(&mut record, &i(host), &exterior(), mm(along), mm(900));
    record
}

pub fn tie() -> Tie {
    Tie {
        level: i("L-g"),
        offset: mm(0),
    }
}

pub fn corners(points: &[[i64; 2]]) -> Vec<Corner> {
    points
        .iter()
        .enumerate()
        .map(|(n, [x, y])| Corner {
            id: Some(i(&format!("e{n}"))),
            at: Point {
                x: x * MM,
                y: y * MM,
            },
        })
        .collect()
}

/// A floor on the corners `outer`, in millimetres.
pub fn floor(id: &str, outer: &[[i64; 2]]) -> Element {
    let boundary = Boundary::from_corners(&corners(outer), &[]).unwrap();
    construct_slabs::floor_record(&i(id), &doc(), id, &i("floor-slab"), &tie(), &boundary)
}

/// A floor's outer corners, in grid steps, with their edges' IDs.
pub fn floor_corners(record: &Element) -> Vec<(String, [i64; 2])> {
    let boundary = construct_slabs::boundary_of(&record.recipe).unwrap();
    boundary
        .corners(construct_slabs::Ring::Outer)
        .unwrap()
        .into_iter()
        .map(|(id, p)| (id.to_string(), [p.x, p.y]))
        .collect()
}

/// An outline of corners in millimetres.
pub fn outline(id: &str, corners: &[[i64; 2]]) -> construct_openings::Outline {
    let named = construct_openings::named(id, &Recipe::construct("shaft", BTreeMap::new()));
    construct_openings::Outline::from_mm(&named, corners).unwrap()
}

/// An opening drawn in the plan of the floor `host`.
pub fn floor_opening(id: &str, host: &str, corners: &[[i64; 2]]) -> Element {
    let mut record =
        construct_openings::floor_opening_record(&i(id), &doc(), id, &outline(id, corners));
    let top = FaceName {
        name: "top".to_owned(),
        role: "top".to_owned(),
    };
    hosting::write_in_plan(&mut record, &i(host), &top);
    record
}

/// A shaft through the levels.
pub fn shaft(id: &str, corners: &[[i64; 2]]) -> Element {
    let top = Tie {
        level: i("L-1"),
        offset: mm(0),
    };
    construct_openings::shaft_record(&i(id), &doc(), id, &outline(id, corners), &tie(), &top)
}

/// An outline's corners in grid steps.
pub fn outline_corners(record: &Element) -> Vec<[i64; 2]> {
    construct_openings::outline_of(record.id.as_str(), &record.recipe)
        .unwrap()
        .corners()
        .iter()
        .map(|[x, y]| [x.steps(), y.steps()])
        .collect()
}

/// A gable roof on a rectangle `w` by `d` millimetres at the origin, rising from edges e0 and
/// e2 at 1:2, with a 300 mm overhang all round.
pub fn gable(id: &str, w: i64, d: i64) -> Element {
    let plain = json!({
        "footprint": { "outer": [
            { "id": "e0", "at": [0, 0] },
            { "id": "e1", "at": [w * MM, 0] },
            { "id": "e2", "at": [w * MM, d * MM] },
            { "id": "e3", "at": [0, d * MM] },
        ] },
        "edges": {
            "e0": { "overhang": 300 * MM, "pitch": "slope:1/2" },
            "e1": { "overhang": 300 * MM },
            "e2": { "overhang": 300 * MM, "pitch": "slope:1/2" },
            "e3": { "overhang": 300 * MM },
        },
    });
    let roof = Roof::from_plain(&plain).unwrap();
    construct_roofs::roof_record(&i(id), &doc(), id, &i("roof-tiles"), &tie(), &roof)
}

/// A room at `at` millimetres, numbered `number`.
pub fn room(id: &str, number: &str, at: [i64; 2]) -> Element {
    construct_rooms::room_record(
        &i(id),
        &doc(),
        "Room",
        number,
        [mm(at[0]), mm(at[1])],
        &tie(),
        &Limit::Height(mm(2700)),
    )
    .unwrap()
}

pub fn grid_line(id: &str, name: &str, from: [i64; 2], to: [i64; 2]) -> Element {
    let p = |[x, y]: [i64; 2]| GridPoint::new(mm(x), mm(y));
    let shape = Shape::line(p(from), p(to)).unwrap();
    Grid::new(i(id), name, shape).unwrap().to_record(&doc())
}

pub fn grid_arc(id: &str, name: &str, from: i64, to: i64) -> Element {
    let shape = Shape::arc(
        GridPoint::new(mm(1000), mm(0)),
        mm(6000),
        degrees(from),
        degrees(to),
    )
    .unwrap();
    Grid::new(i(id), name, shape).unwrap().to_record(&doc())
}

pub fn plane(id: &str, name: Option<&str>, shape: PlaneShape) -> Element {
    ReferencePlane::new(i(id), name, shape)
        .unwrap()
        .to_record(&doc())
}

/// A block `w` by `d` millimetres with its corner at `x`, `y`.
pub fn block(id: &str, x: i64, y: i64, w: i64, d: i64) -> Element {
    let args = json!({ "width": w, "depth": d, "height": 1000 });
    Element {
        id: i(id),
        document: doc(),
        kind: "block".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x, y, z: 0 },
        recipe: Recipe::construct("block", serde_json::from_value(args).unwrap()),
        props: props("2000"),
        references: Vec::new(),
    }
}

/// An older box wall along x, `length` millimetres from `x`, `y`.
pub fn old_wall(id: &str, x: i64, y: i64, length: i64) -> Element {
    let args = json!({ "length": length, "thickness": 200, "height": 2700 });
    Element {
        id: i(id),
        document: doc(),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x, y, z: 0 },
        recipe: Recipe::construct("wall.straight", serde_json::from_value(args).unwrap()),
        props: props("100"),
        references: Vec::new(),
    }
}

/// A record of no shape: a type.
pub fn a_type(id: &str) -> Element {
    Element {
        kind: "type".to_owned(),
        ..block(id, 0, 0, 1, 1)
    }
}

/// Answers from fixed values: every wall is flipped as `flipped` says, and a placed family's hands
/// are read from the library the server places them from.
pub struct Fixed {
    pub flipped: bool,
}

impl Sense for Fixed {
    fn flipped(&self, _: &Element) -> Result<bool, String> {
        Ok(self.flipped)
    }

    fn hands(&self, family: &Element) -> Result<Vec<Hand>, String> {
        let instance = Instance::of(&family.recipe).map_err(|e| e.message)?;
        let source: Arc<dyn FamilySource> = families::baked();
        let recipe = source.recipe(&instance.family).ok_or("no such family")?;
        hands_of(&recipe, &instance)
    }
}

use std::sync::Arc;

/// The value a placed family holds for `key`, in stored text, if it holds its own.
pub fn value_of(record: &Element, key: &str) -> Option<String> {
    let key = Key::parse(key).unwrap();
    Instance::of(&record.recipe)
        .unwrap()
        .values
        .get(&key)
        .cloned()
}

/// A document of these records, by ID.
pub fn document(records: Vec<Element>) -> BTreeMap<Id, Element> {
    records.into_iter().map(|r| (r.id.clone(), r)).collect()
}
