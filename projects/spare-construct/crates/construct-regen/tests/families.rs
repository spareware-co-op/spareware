//! Placed families (job 6.4, CON-A7-F3-P2) through the log and the regenerator, on the test
//! store: a door and a window from the library's recipes hosted on a layered wall's sides are
//! seated on its centre plane, turned so their front faces out of the side they are on; the wall
//! is built with them; a wall moved, or its type changed, builds the family again with it, and a
//! family's type or recipe changed builds its wall again, and nothing else is built; a family
//! whose recipe is not loaded fails, naming it. The records go in as they are, through a command
//! of this file's own that only stores them: the commands that place families are a later job's
//! (7.1).
//!
//! The builder is the stand-in on its own edition, except that a host built with what is hosted
//! on it lists each hosted element's key, place and turn in its body, so a test reads where a
//! family sat and how it was turned.

#![cfg(feature = "server")]
// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, WALLS};
use construct_families::{Key, Turn};
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model};
use construct_params::{Choice, Definition, Holder, Kind, Length, ProjectParams, Value};
use construct_regen::families::{self, Library};
use construct_regen::hosting;
use construct_regen::parameters::{self, Context, TYPE_KEY};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_regen::walls::{self, type_arg};
use construct_types::ElementType;
use construct_walls::{
    FLIPPED, LOCATION_LINE, LocationLine, assembly_of, default_wall_types, location_line_value,
    with_assembly,
};
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";
const PARTITION: &str = "wall-partition-stud-90";
const DOOR: &str = "library.doors-windows.single-swing-door";
const WINDOW: &str = "library.doors-windows.fixed-window";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// The stand-in, on its own edition, except that a host built with what is hosted on it lists
/// each hosted element's key, where it sits in the host's frame, and its turn.
struct Cutter;

impl Geometry for Cutter {
    fn edition(&self) -> Edition {
        Boxes.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        Boxes.build(recipe)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = Boxes.build(host)?;
        let cuts: Vec<Json> = hosted
            .iter()
            .map(|h| {
                json!({
                    "key": h.key,
                    "at": [h.at.x, h.at.y, h.at.z],
                    "turn": h.recipe.args.get(families::TURN_ARG).cloned().unwrap_or(json!(0)),
                })
            })
            .collect();
        built.body = serde_json::to_vec(&json!({ "cut": cuts })).unwrap();
        Ok(built)
    }
}

/// The only commands here: `put` stores a record as it is (`{"record": ...}`), and `model`
/// makes the document on the stand-in's edition.
struct Puts;

impl Commands<Memory> for Puts {
    fn version(&self) -> u32 {
        1
    }

    fn permit(
        &self,
        _: &Scope,
        _: &Edit,
    ) -> impl Future<Output = Result<Result<(), String>, StoreError>> + Send {
        std::future::ready(Ok(Ok(())))
    }

    async fn apply(
        &self,
        txn: &mut <Memory as Kv>::Txn,
        _: &Moment<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        if edit.cmd == "model" {
            let place = Place::Model {
                project: i(PROJECT),
                model: edit.target.clone(),
            };
            let model = Model {
                id: edit.target.clone(),
                name: "Doc".to_owned(),
                team: i("t"),
                package: "Doc package".to_owned(),
                edition: Boxes.edition(),
            };
            txn.put(place.key(), to_row(&model)?).await?;
            return Ok(Ok(Applied::default()));
        }
        let record: Element = serde_json::from_value(edit.args["record"].clone()).unwrap();
        let bytes = to_row(&record)?;
        let hash = i(&kit_ops::sha256_hex(&bytes));
        let place = kit_store::Place::Record {
            project: i(PROJECT),
            hash: hash.clone(),
        };
        txn.put(place.key(), bytes).await?;
        let pointer = kit_store::Place::Piece {
            project: i(PROJECT),
            document: edit.target.clone(),
            piece: record.id.clone(),
        };
        let before: Option<String> = read_row(txn, &pointer.key()).await?;
        txn.put(pointer.key(), to_row(&hash.as_str())?).await?;
        Ok(Ok(Applied {
            changes: vec![Change {
                piece: record.id,
                before: before.map(|h| i(&h)),
                after: Some(hash),
            }],
            ..Applied::default()
        }))
    }
}

/// The orderer's time, fixed.
struct Fixed;

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        1_000_000
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, Puts>,
    regen: Regenerator<Memory, MemoryFiles>,
    next: AtomicU64,
}

impl Stack {
    async fn new() -> Self {
        let store = Memory::new();
        let orderers = Orderers::new(store.clone(), Arc::new(Puts), Arc::new(Fixed), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), Arc::new(Cutter));
        let stack = Self {
            store,
            orderers,
            regen,
            next: AtomicU64::new(0),
        };
        stack.run("model", json!({})).await;
        stack
    }

    async fn run(&self, cmd: &str, args: Json) {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let edit = Edit {
            person: i("anna"),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        let reply = self
            .orderers
            .run(&Scope::Project(i(PROJECT)), edit)
            .await
            .unwrap();
        assert!(matches!(reply, Reply::Ack { .. }), "{reply:?}");
    }

    /// Stores `record` and brings the shapes up to date: the elements that one pass built.
    async fn put(&self, record: &Element) -> BTreeSet<String> {
        self.run("put", json!({ "record": record })).await;
        let pass = self.regen.catch_up(&i(PROJECT)).await.unwrap();
        pass.bodies
            .get(&i(DOCUMENT))
            .map(|built| built.keys().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Built {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, or its failure's code and words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// Where each hosted element sat in a wall it was built with, and its turn.
    async fn cuts(&self, id: &str) -> Vec<(String, [i64; 3], u64)> {
        let BuiltResult::Shape { body, .. } = self.built(id).await.result else {
            panic!("{id} did not build");
        };
        let place = Place::Body {
            project: i(PROJECT),
            id: body.clone(),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let stored = read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap();
        let bytes = self.regen.read_bytes(&stored, &body).await.unwrap();
        let body: Json = serde_json::from_slice(&bytes).unwrap();
        body["cut"]
            .as_array()
            .map(|cuts| {
                cuts.iter()
                    .map(|c| {
                        let at: Vec<i64> = c["at"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_i64().unwrap())
                            .collect();
                        (
                            c["key"].as_str().unwrap().to_owned(),
                            [at[0], at[1], at[2]],
                            c["turn"].as_u64().unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A document's parameters with the two wall instance parameters bound to walls.
fn table() -> ProjectParams {
    let categories = CategoryTable::new();
    let choices = LocationLine::ALL
        .iter()
        .map(|line| Choice::new(line.key(), line.label()))
        .collect();
    let line =
        Definition::choice(i(LOCATION_LINE), "Location line", Holder::Instance, choices).unwrap();
    let flipped = Definition::new(i(FLIPPED), "Flipped", Kind::YesNo, Holder::Instance).unwrap();
    ProjectParams::new()
        .add_project_parameter(line)
        .unwrap()
        .add_project_parameter(flipped)
        .unwrap()
        .bind(
            &categories,
            &i(LOCATION_LINE),
            &[WALLS],
            Holder::Instance,
            Some(location_line_value(LocationLine::WallCentre)),
        )
        .unwrap()
        .bind(
            &categories,
            &i(FLIPPED),
            &[WALLS],
            Holder::Instance,
            Some(Value::YesNo(false)),
        )
        .unwrap()
}

fn context() -> Context {
    Context::new(table()).unwrap()
}

/// The default wall type `id`, read against `context`.
fn wall_type(context: &Context, id: &str) -> ElementType {
    default_wall_types(&context.params, &context.categories)
        .unwrap()
        .into_iter()
        .find(|ty| ty.id().as_str() == id)
        .unwrap()
}

fn q0(steps: i64) -> String {
    Value::Length(Length::from_steps(steps)).stored()
}

/// A layered wall of type `ty`, its line from `start` to `end` in whole millimetres, 2700 mm high,
/// its layers read from its type.
fn wall(id: &str, ty: &str, start: [i64; 2], end: [i64; 2]) -> Element {
    let mm = |v: i64| q0(v * 6400);
    let mut args = BTreeMap::new();
    args.insert("start".to_owned(), json!([mm(start[0]), mm(start[1])]));
    args.insert("end".to_owned(), json!([mm(end[0]), mm(end[1])]));
    args.insert("height".to_owned(), json!(mm(2700)));
    args.insert("layers".to_owned(), type_arg());
    let mut props = BTreeMap::new();
    props.insert(
        parameters::CATEGORY_KEY.to_owned(),
        parameters::write_category(WALLS),
    );
    props.insert(TYPE_KEY.to_owned(), ty.to_owned());
    Element {
        id: i(id),
        document: i(DOCUMENT),
        kind: "wall".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct("wall.layered", args),
        props,
        references: Vec::new(),
    }
}

/// The face of the wall `host` (of the type `ty`) in the role `role`, as a hosted element names
/// it.
fn side(
    context: &Context,
    host: &Element,
    ty: &str,
    role: &str,
) -> construct_model::geometry::FaceName {
    let filled = walls::filled(&host.recipe, context, host, Some(&wall_type(context, ty))).unwrap();
    construct_regen::stand_in::names(&filled)
        .unwrap()
        .into_iter()
        .find(|n| n.role == role)
        .unwrap()
}

/// A placed family: the library family `family` of type `ty`, hosted on `face` of the wall
/// `host`, `along` its line and `up` from its base, in millimetres.
fn placed(
    id: &str,
    family: &str,
    ty: &str,
    host: &str,
    face: &construct_model::geometry::FaceName,
    along: i32,
    up: i32,
) -> Element {
    let (content, _) = Library::baked().family(family).unwrap();
    let mut element = Element {
        id: i(id),
        document: i(DOCUMENT),
        kind: "door".to_owned(),
        name: id.to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: families::recipe(&content, &Key::parse(ty).unwrap()),
        props: BTreeMap::new(),
        references: Vec::new(),
    };
    hosting::write(
        &mut element,
        &i(host),
        face,
        Length::from_mm(along),
        Length::from_mm(up),
    );
    element
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

/// A family is seated on its wall's centre plane, its insertion point along the wall's line from
/// its start and its origin up from its base, turned so that its front (its `+y`) faces out of
/// the side it is hosted on, never mirrored: on a wall drawn along +x the exterior is on the
/// line's right (−y), so a family there is turned half a turn and one on the interior not at
/// all; along +y the exterior is at +x (three quarters) and the interior at −x (a quarter). The
/// stand-in draws each as its solids' boxes in its own frame, turned.
#[test]
fn a_family_is_seated_on_its_walls_centre_plane_facing_out_of_its_side() {
    let context = context();
    let edition = Boxes.edition();
    for (end, role, expected, turn) in [
        ([4000, 0], "exterior", [1500, 0, 900], Turn::Half),
        ([4000, 0], "interior", [1500, 0, 900], Turn::None),
        ([0, 4000], "exterior", [0, 1500, 900], Turn::ThreeQuarters),
        ([0, 4000], "interior", [0, 1500, 900], Turn::Quarter),
        ([-4000, 0], "exterior", [-1500, 0, 900], Turn::None),
    ] {
        let host = wall("W-1", PARTITION, [0, 0], end);
        let filled = walls::filled(
            &host.recipe,
            &context,
            &host,
            Some(&wall_type(&context, PARTITION)),
        )
        .unwrap();
        let face = side(&context, &host, PARTITION, role);
        let family = placed("F-1", DOOR, "d900x2100", "W-1", &face, 1500, 900);
        let hosting = hosting::read(&family).unwrap().unwrap();
        let (at, seated) =
            hosting::seat(&edition, &host, &filled, &family, &family.recipe, &hosting).unwrap();
        assert_eq!([at.x, at.y, at.z], expected, "{end:?} {role}");
        let instance = families::Instance::of(&seated).unwrap();
        assert_eq!(instance.turn, turn, "{end:?} {role}");
        assert_eq!(
            hosting::at(&edition, &host, &filled, &family, &family.recipe, &hosting).unwrap(),
            at
        );
        let drawn = Boxes.build(&seated).unwrap();
        let leaf = drawn.displays[0]
            .parts
            .iter()
            .find(|p| p.function == "leaf")
            .unwrap();
        // The leaf is flush with the family's front, 10 to 50 mm in front of its origin.
        let front: Vec<[i64; 2]> = leaf.outline.clone();
        let reach = match turn {
            Turn::None => (front[0][1], front[2][1]),
            Turn::Half => (-front[2][1], -front[0][1]),
            Turn::Quarter => (-front[2][0], -front[0][0]),
            Turn::ThreeQuarters => (front[0][0], front[2][0]),
        };
        assert_eq!(reach, (10, 50), "{end:?} {role}: {front:?}");
    }
    // Only a layered wall hosts a family.
    let angled = wall("W-1", PARTITION, [0, 0], [3000, 4000]);
    let mut boxed = angled.clone();
    boxed.recipe = Recipe::construct(
        "wall.straight",
        serde_json::from_value(json!({ "length": 4000, "thickness": 200, "height": 2700 }))
            .unwrap(),
    );
    let face = construct_regen::stand_in::names(&boxed.recipe).unwrap()[2].clone();
    let family = placed("F-1", DOOR, "d900x2100", "W-1", &face, 1500, 0);
    let read = hosting::read(&family).unwrap().unwrap();
    let refused = hosting::seat(
        &edition,
        &boxed,
        &boxed.recipe,
        &family,
        &family.recipe,
        &read,
    )
    .unwrap_err();
    assert_eq!(refused.code, "NotAHost");
}

/// On a wall at a 3:4:5 slope (piece A83) a family is seated in the wall's frame, its origin
/// 1500 mm along the line on its centre plane, exactly (900, 1200) mm; facing the exterior, the
/// line's right, its x runs against the line, and it is seated in place of a turn. The stand-in
/// draws its leaf turned with the wall, 10 to 50 mm right of the line, each corner to the nearest
/// millimetre.
#[test]
fn a_family_on_a_wall_at_an_angle_is_seated_in_its_frame() {
    let context = context();
    let edition = Boxes.edition();
    let angled = wall("W-1", PARTITION, [0, 0], [3000, 4000]);
    let filled = walls::filled(
        &angled.recipe,
        &context,
        &angled,
        Some(&wall_type(&context, PARTITION)),
    )
    .unwrap();
    let face = side(&context, &angled, PARTITION, "exterior");
    let family = placed("F-1", DOOR, "d900x2100", "W-1", &face, 1500, 0);
    let read = hosting::read(&family).unwrap().unwrap();
    let (at, seated) =
        hosting::seat(&edition, &angled, &filled, &family, &family.recipe, &read).unwrap();
    assert_eq!([at.x, at.y, at.z], [900, 1200, 0]);
    let instance = families::Instance::of(&seated).unwrap();
    assert_eq!(instance.turn, Turn::None);
    assert_eq!(
        instance.seat,
        construct_families::Seat::new(
            [-19_200_000, -25_600_000],
            [-5_760_000, -7_680_000],
            [-9_600_000, 0]
        )
    );
    let drawn = Boxes.build(&seated).unwrap();
    let leaf = drawn.displays[0]
        .parts
        .iter()
        .find(|p| p.function == "leaf")
        .unwrap();
    // Right of the line (3, 4) / 5 from the wall's start: (4x - 3y) / 5, the family at (900, 1200).
    let right: Vec<i64> = leaf
        .outline
        .iter()
        .map(|&[x, y]| (4 * (x + 900) - 3 * (y + 1200)) / 5)
        .collect();
    let (near, far) = (*right.iter().min().unwrap(), *right.iter().max().unwrap());
    assert!(
        (9..=11).contains(&near) && (49..=51).contains(&far),
        "{right:?}"
    );
}

/// Through the log: a door and a window on a layered wall are seated in it and the wall is built
/// with them. The `reads` rows build what they must and nothing else: a wall moved builds it and
/// its families (their records unwritten, their places moved with it); its type changed builds the
/// same; a family's type changed, or the recipe it names, builds it and its wall; another wall
/// and its own family are never built for any of these. A family whose recipe is not loaded
/// fails, naming it.
#[tokio::test]
async fn a_family_moves_with_its_wall_and_only_what_reads_a_change_is_built() {
    let stack = Stack::new().await;
    let context = context();
    stack
        .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
        .await;
    stack
        .put(&wall_type(&context, PARTITION).to_record(&i(DOCUMENT)))
        .await;
    stack
        .put(&wall_type(&context, "wall-concrete-200").to_record(&i(DOCUMENT)))
        .await;
    let w1 = wall("W-1", PARTITION, [0, 0], [4000, 0]);
    let w2 = wall("W-2", "wall-concrete-200", [0, 5000], [4000, 5000]);
    stack.put(&w1).await;
    stack.put(&w2).await;
    let exterior = side(&context, &w1, PARTITION, "exterior");
    let door = placed("F-1", DOOR, "d900x2100", "W-1", &exterior, 1000, 0);
    let window = placed("F-2", WINDOW, "w1200x1200", "W-1", &exterior, 2800, 0);
    let other = placed(
        "F-3",
        DOOR,
        "d800x2100",
        "W-2",
        &side(&context, &w2, "wall-concrete-200", "interior"),
        2000,
        0,
    );
    assert_eq!(stack.put(&door).await, set(&["F-1", "W-1"]));
    assert_eq!(stack.put(&window).await, set(&["F-2", "W-1"]));
    assert_eq!(stack.put(&other).await, set(&["F-3", "W-2"]));
    assert_eq!(
        stack.cuts("W-1").await,
        [
            ("F-1".to_owned(), [1000, 0, 0], 2),
            ("F-2".to_owned(), [2800, 0, 0], 2)
        ]
    );
    // The stand-in draws the door as its frame's box, in its own frame.
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([-450, -50, 0], [450, 50, 2100]))
    );
    let (door_input, window_input) = (
        stack.built("F-1").await.input,
        stack.built("F-2").await.input,
    );

    // The wall moved: it and its two families, nothing else.
    let moved = wall("W-1", PARTITION, [500, 300], [4500, 300]);
    assert_eq!(stack.put(&moved).await, set(&["F-1", "F-2", "W-1"]));
    assert_eq!(
        stack.cuts("W-1").await,
        [
            ("F-1".to_owned(), [1500, 300, 0], 2),
            ("F-2".to_owned(), [3300, 300, 0], 2)
        ],
        "the families move with their wall"
    );
    assert_eq!(
        stack.built("F-1").await.input,
        door_input,
        "its record is not written"
    );
    assert_eq!(stack.built("F-2").await.input, window_input);

    // Its type's layers changed: the wall and its families again, and nothing else.
    let concrete = assembly_of(&wall_type(&context, "wall-concrete-200")).unwrap();
    let thicker = with_assembly(wall_type(&context, PARTITION), &concrete).unwrap();
    assert_eq!(
        stack.put(&thicker.to_record(&i(DOCUMENT))).await,
        set(&["F-1", "F-2", "W-1"])
    );

    // The door's type changed: the door and its wall; the window and the other wall are not.
    let wider = placed("F-1", DOOR, "d1000x2400", "W-1", &exterior, 1000, 0);
    assert_eq!(stack.put(&wider).await, set(&["F-1", "W-1"]));
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([-500, -50, 0], [500, 50, 2400]))
    );

    // The recipe it names changed: the door names the fixed window's recipe now.
    let recipe_changed = placed("F-1", WINDOW, "w900x900", "W-1", &exterior, 1000, 0);
    assert_eq!(stack.put(&recipe_changed).await, set(&["F-1", "W-1"]));
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([-450, -35, 900], [450, 35, 1800]))
    );

    // A recipe that is not loaded: the family fails, naming it; its wall still builds.
    let mut missing = placed("F-4", DOOR, "d900x2100", "W-1", &exterior, 3500, 0);
    missing.recipe.args.insert(
        families::FAMILY_ARG.to_owned(),
        json!(format!("sha256:{}", "1".repeat(64))),
    );
    // A failure has no body, so of the two built only the wall's is listed.
    assert_eq!(stack.put(&missing).await, set(&["W-1"]));
    let failed = stack.bounds("F-4").await.unwrap_err();
    assert!(
        failed.starts_with("FamilyMissing: family sha256:"),
        "{failed}"
    );
    assert!(stack.bounds("W-1").await.is_ok());
}
