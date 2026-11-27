//! A layered wall through the log and the regenerator, on the test store: its layers read from
//! its wall type and laid out round its location line on its side, built again when its type's
//! layers change, a top tied to a level filled as a stored length, and a door hosted on it placed
//! flush with its side and moving with it, its record unwritten. The records go in as they are,
//! through a command of this file's own that only stores them: the commands that make walls and
//! wall types are the server's (construct-api).
//!
//! The builder is the stand-in on its own edition, except that a host built with what is hosted
//! on it lists each hosted element's key and place in its body, so a test reads where a door sat.

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
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model, Reads};
use construct_params::{Choice, Definition, Holder, Kind, Length, ProjectParams, Value};
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
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";
const PARTITION: &str = "wall-partition-stud-90";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// The stand-in, on its own edition, except that a host built with what is hosted on it lists
/// each hosted element's key and where it sits in the host's frame, and a layered wall built with
/// joints lists them.
struct Cutter;

impl Geometry for Cutter {
    fn edition(&self) -> Edition {
        Boxes.edition()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let mut built = Boxes.build(recipe)?;
        if let Some(joins) = recipe.args.get("joins") {
            built.body = serde_json::to_vec(&json!({ "joins": joins })).unwrap();
        }
        Ok(built)
    }

    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let mut built = Boxes.build(host)?;
        let cuts: Vec<Json> = hosted
            .iter()
            .map(|h| json!({ "key": h.key, "at": [h.at.x, h.at.y, h.at.z] }))
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

    async fn put(&self, record: &Element) {
        self.run("put", json!({ "record": record })).await;
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

    /// An element's built bounds, or its failure's code.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// An element's body, as JSON.
    async fn body(&self, id: &str) -> Json {
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
        serde_json::from_slice(&self.regen.read_bytes(&stored, &body).await.unwrap()).unwrap()
    }

    /// The joints a layered wall was built with: its recipe's `joins`, or null.
    async fn joins(&self, id: &str) -> Json {
        self.body(id).await["joins"].clone()
    }

    /// The walls an element's `reads` row lists for its joints.
    async fn joins_read(&self, id: &str) -> Vec<String> {
        self.regen.catch_up(&i(PROJECT)).await.unwrap();
        let place = Place::Reads {
            project: i(PROJECT),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let row: Option<Reads> = read_row(&mut snapshot, &place.key()).await.unwrap();
        row.map(|r| r.joins.into_iter().collect())
            .unwrap_or_default()
    }

    /// Where each door sat in a wall it was built with.
    async fn cuts(&self, id: &str) -> Vec<(String, [i64; 3])> {
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
                        (c["key"].as_str().unwrap().to_owned(), [at[0], at[1], at[2]])
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

/// A layered wall's recipe is filled from its type: each layer's function, material and two faces
/// as stored lengths, laid out round the location line the element chose, on its side; the
/// recipe of an element with no type, or a type that is not a wall type, fails in words.
#[test]
fn a_layered_walls_layers_are_read_from_its_type() {
    let context = context();
    let ty = wall_type(&context, PARTITION);
    let element = wall("W-1", PARTITION, [0, 0], [4000, 0]);
    let filled = walls::filled(&element.recipe, &context, &element, Some(&ty)).unwrap();
    let layers = filled.args["layers"].as_array().unwrap();
    let faces: Vec<(&str, &str, &str, &str)> = layers
        .iter()
        .map(|l| {
            (
                l["function"].as_str().unwrap(),
                l["material"].as_str().unwrap(),
                l["exterior"].as_str().unwrap(),
                l["interior"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        faces,
        [
            (
                "finish",
                "plasterboard-gypsum",
                "length:q0:368000",
                "length:q0:288000"
            ),
            (
                "structure",
                "timber-douglas-fir",
                "length:q0:288000",
                "length:q0:-288000"
            ),
            (
                "finish",
                "plasterboard-gypsum",
                "length:q0:-288000",
                "length:q0:-368000"
            ),
        ]
    );
    assert!(walls::reads_type(&element.recipe));
    assert!(!walls::reads_type(&filled));
    // On its exterior finish face, flipped: its exterior faces the line's left, so the wall lies
    // on the line's right, at y below zero along x.
    let mut chosen = element.clone();
    chosen.props.insert(
        LOCATION_LINE.to_owned(),
        location_line_value(LocationLine::ExteriorFinishFace).stored(),
    );
    chosen
        .props
        .insert(FLIPPED.to_owned(), Value::YesNo(true).stored());
    let flipped = walls::filled(&chosen.recipe, &context, &chosen, Some(&ty)).unwrap();
    let first = &flipped.args["layers"][0];
    assert_eq!(first["exterior"], "length:q0:0");
    assert_eq!(first["interior"], "length:q0:80000");
    let built = Boxes.build(&flipped).unwrap();
    assert_eq!(built.bounds.min, [0, -115, 0]);
    assert_eq!(built.bounds.max, [4000, 0, 2700]);
    assert_eq!(
        walls::filled(&element.recipe, &context, &element, None)
            .unwrap_err()
            .code,
        "NoWallType"
    );
    let plain = ElementType::new(i("plain"), "Plain", WALLS, &context.categories).unwrap();
    let refused = walls::filled(&element.recipe, &context, &element, Some(&plain)).unwrap_err();
    assert_eq!(refused.code, "BadWallType");
    assert!(refused.message.contains("plain"), "{}", refused.message);
}

/// A wall's wrapping is filled from its type, each choice replaced by the wall's own where its
/// record gives one, with each layer's flag and the core; a wall that wraps nothing is filled as
/// before, with no `wrap`; a wall's own wrapping that does not read fails in words.
#[test]
fn a_walls_wrapping_is_filled_from_its_type_and_its_own() {
    use construct_walls::{AtEnds, AtInserts, Wrapping};
    let context = context();
    let ty = wall_type(&context, PARTITION);
    let element = wall("W-1", PARTITION, [0, 0], [4000, 0]);
    let filled = walls::filled(&element.recipe, &context, &element, Some(&ty)).unwrap();
    assert!(
        !filled.args.contains_key("wrap"),
        "the default partition wraps nothing"
    );
    let wrapping = Wrapping {
        ends: AtEnds::Exterior,
        inserts: AtInserts::Both,
    };
    let assembly = assembly_of(&ty).unwrap().with_wrapping(wrapping);
    let wrapped_type = with_assembly(ty.clone(), &assembly).unwrap();
    let filled = walls::filled(&element.recipe, &context, &element, Some(&wrapped_type)).unwrap();
    assert_eq!(
        filled.args["wrap"],
        json!({ "core": [1, 2], "ends": "exterior", "inserts": "both",
                "layers": [true, false, true] })
    );
    // The wall's own choice over its type's, one at a time.
    let mut own = element.clone();
    own.recipe
        .args
        .insert("wrapping".to_owned(), json!({ "inserts": "none" }));
    let filled = walls::filled(&own.recipe, &context, &own, Some(&wrapped_type)).unwrap();
    assert_eq!(filled.args["wrap"]["inserts"], "none");
    assert_eq!(filled.args["wrap"]["ends"], "exterior");
    // Its own wrapping where its type wraps nothing.
    own.recipe
        .args
        .insert("wrapping".to_owned(), json!({ "ends": "interior" }));
    let filled = walls::filled(&own.recipe, &context, &own, Some(&ty)).unwrap();
    assert_eq!(filled.args["wrap"]["ends"], "interior");
    assert_eq!(filled.args["wrap"]["inserts"], "none");
    // Both its own choices none over a type that wraps: nothing wraps.
    own.recipe.args.insert(
        "wrapping".to_owned(),
        json!({ "ends": "none", "inserts": "none" }),
    );
    let filled = walls::filled(&own.recipe, &context, &own, Some(&wrapped_type)).unwrap();
    assert!(!filled.args.contains_key("wrap"));
    own.recipe
        .args
        .insert("wrapping".to_owned(), json!({ "ends": "sideways" }));
    let refused = walls::filled(&own.recipe, &context, &own, Some(&ty)).unwrap_err();
    assert_eq!(refused.code, "BadWrapping");
}

/// Walls joined at an angle off every whole ratio keep, at their free ends, the corners their
/// builders' strips have: the join rule meets the faces the builders build (each face's one
/// rounding is the same in construct-walls and in `wall_plan`), and every joined piece reads back
/// as the builders take it.
#[test]
fn joins_at_an_angle_meet_the_faces_the_builders_build() {
    use construct_regen::wall_plan::{Layered, Plan};
    let faces: [(&str, i64, i64); 3] = [
        ("finish", 368_000, 288_000),
        ("structure", 288_000, -288_000),
        ("finish", -288_000, -368_000),
    ];
    let recipe = |start: [i64; 2], end: [i64; 2]| {
        let layers: Vec<_> = faces
            .iter()
            .map(|&(f, a, b)| {
                json!({ "function": f, "material": "m", "exterior": q0(a), "interior": q0(b) })
            })
            .collect();
        let mut args = BTreeMap::new();
        args.insert("start".to_owned(), json!([q0(start[0]), q0(start[1])]));
        args.insert("end".to_owned(), json!([q0(end[0]), q0(end[1])]));
        args.insert("height".to_owned(), json!(q0(17_280_000)));
        args.insert("layers".to_owned(), json!(layers));
        Recipe::construct("wall.layered", args)
    };
    for (da, db) in [
        ([7, 4], [-2, 9]),
        ([1, 3], [-5, 2]),
        ([123_457, 98_765], [-1, 1]),
    ] {
        let start = [-da[0] * 3_000_001, -da[1] * 3_000_001];
        let end = [db[0] * 2_000_003, db[1] * 2_000_003];
        let (a, b) = (recipe(start, [0, 0]), recipe([0, 0], end));
        let joined = construct_regen::joins::among(&[("A", &a), ("B", &b)]);
        let wall = Layered::of(&joined[0]).unwrap();
        let pieces = &wall
            .joined
            .as_ref()
            .unwrap_or_else(|| panic!("{da:?}: {:?}", joined[0].args.get("joins")))
            .layers;
        let plan = Plan::new(start, [0, 0]).unwrap();
        for (k, &(_, ext, int)) in faces.iter().enumerate() {
            let first = &pieces[k][0];
            for o in [ext, int] {
                let corner = plan.face(o).unwrap()[0];
                assert!(
                    first.contains(&corner),
                    "{da:?} layer {k}: {first:?} has {corner:?}"
                );
            }
        }
        assert!(Layered::of(&joined[1]).unwrap().joined.is_some(), "{da:?}");
    }
}

/// Through the log: a layered wall is built from its type's layers; a change to its type's layers
/// builds it again with the new ones, its own record unchanged; a wall changed to another type is
/// built with that type's; a wall whose type is missing fails, naming it.
#[tokio::test]
async fn a_layered_wall_is_built_again_when_its_types_layers_change() {
    let stack = Stack::new().await;
    let context = context();
    stack
        .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
        .await;
    let ty = wall_type(&context, PARTITION);
    stack.put(&ty.to_record(&i(DOCUMENT))).await;
    stack.put(&wall("W-1", PARTITION, [0, 0], [4000, 0])).await;
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -58, 0], [4000, 58, 2700]))
    );
    let input = stack.built("W-1").await.input;

    let concrete = assembly_of(&wall_type(&context, "wall-concrete-200")).unwrap();
    let thicker = with_assembly(ty.clone(), &concrete).unwrap();
    stack.put(&thicker.to_record(&i(DOCUMENT))).await;
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -100, 0], [4000, 100, 2700])),
        "its type's new layers"
    );
    assert_eq!(
        stack.built("W-1").await.input,
        input,
        "its record is not written"
    );

    let veneer = wall_type(&context, "wall-brick-veneer-90");
    stack.put(&veneer.to_record(&i(DOCUMENT))).await;
    stack
        .put(&wall("W-1", "wall-brick-veneer-90", [0, 0], [4000, 0]))
        .await;
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -125, 0], [4000, 125, 2700]))
    );

    stack
        .put(&wall("W-2", "no-such-type", [0, 0], [4000, 0]))
        .await;
    let failed = stack.bounds("W-2").await.unwrap_err();
    assert!(failed.starts_with("BadParameter"), "{failed}");
    assert!(failed.contains("no-such-type"), "{failed}");
}

/// Through the log: a door hosted on a layered wall's exterior sits flush with that side, along
/// the wall from its start; the wall is built with it; when the wall moves (its line drawn again)
/// the door moves with it and its record is not written.
#[tokio::test]
async fn a_door_on_a_layered_wall_moves_with_it() {
    let stack = Stack::new().await;
    let context = context();
    stack
        .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
        .await;
    stack
        .put(&wall_type(&context, PARTITION).to_record(&i(DOCUMENT)))
        .await;
    stack.put(&wall("W-1", PARTITION, [0, 0], [4000, 0])).await;
    let host = wall("W-1", PARTITION, [0, 0], [4000, 0]);
    let ty = wall_type(&context, PARTITION);
    let filled = walls::filled(&host.recipe, &context, &host, Some(&ty)).unwrap();
    let face = construct_regen::stand_in::names(&filled)
        .unwrap()
        .into_iter()
        .find(|n| n.role == "exterior")
        .unwrap();
    let mut door = Element {
        id: i("D-1"),
        document: i(DOCUMENT),
        kind: "door".to_owned(),
        name: "D-1".to_owned(),
        level: 0,
        placement: Placement { x: 0, y: 0, z: 0 },
        recipe: Recipe::construct(
            "door.single",
            serde_json::from_value(json!({ "width": 900, "thickness": 100, "height": 2100 }))
                .unwrap(),
        ),
        props: BTreeMap::new(),
        references: Vec::new(),
    };
    hosting::write(
        &mut door,
        &i("W-1"),
        &face,
        Length::from_mm(1000),
        Length::from_mm(0),
    );
    stack.put(&door).await;
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1000, -58, 0])]
    );
    let door_input = stack.built("D-1").await.input;
    stack
        .put(&wall("W-1", PARTITION, [500, 300], [4500, 300]))
        .await;
    assert_eq!(
        stack.cuts("W-1").await,
        [("D-1".to_owned(), [1500, 243, 0])],
        "the door moves with its wall"
    );
    assert_eq!(stack.built("D-1").await.input, door_input);
}

/// Through the log: a layered wall whose top is tied to the level above is as tall as the span
/// between its ties, held exactly as a stored length (12.5 mm over the level is no whole
/// millimetre, which the old box would refuse); when that level moves, the wall is built again.
#[tokio::test]
async fn a_layered_walls_top_tie_is_filled_exactly() {
    use construct_levels::{Level, Tie, write_base, write_top};
    let stack = Stack::new().await;
    let context = context();
    stack
        .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
        .await;
    stack
        .put(&wall_type(&context, PARTITION).to_record(&i(DOCUMENT)))
        .await;
    let ground = Level::new(i("L-0"), "Ground", Length::from_mm(0), true).unwrap();
    let first = Level::new(i("L-1"), "First", Length::from_mm(2700), true).unwrap();
    stack.put(&ground.to_record(&i(DOCUMENT))).await;
    stack.put(&first.to_record(&i(DOCUMENT))).await;
    let mut tied = wall("W-1", PARTITION, [0, 0], [4000, 0]);
    tied.recipe
        .args
        .insert("height".to_owned(), construct_regen::levels::top_arg());
    write_base(
        &mut tied.props,
        Some(&Tie {
            level: i("L-0"),
            offset: Length::from_mm(0),
        }),
    );
    write_top(
        &mut tied.props,
        Some(&Tie {
            level: i("L-1"),
            offset: Length::from_steps(80_000),
        }),
    );
    stack.put(&tied).await;
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -58, 0], [4000, 58, 2713]))
    );
    let higher = Level::new(i("L-1"), "First", Length::from_mm(3000), true).unwrap();
    stack.put(&higher.to_record(&i(DOCUMENT))).await;
    assert_eq!(
        stack.bounds("W-1").await,
        Ok(([0, -58, 0], [4000, 58, 3013]))
    );
}

/// Through the log: two layered walls drawn to meet at an L are each built with their joint (the
/// first built again when the second is drawn), and each `reads` row lists the other; a wall
/// drawn to abut the first's side makes a T with it; a change to the second's type builds the
/// first again with the new joint; moving the second away builds the first without it. The
/// stored shapes match their recipes, and a rebuild from empty gives the same joints.
#[tokio::test]
async fn walls_drawn_to_meet_are_joined_and_a_change_to_one_rebuilds_the_other() {
    let stack = Stack::new().await;
    let context = context();
    stack
        .put(&parameters::table_record(&i(DOCUMENT), &table()).unwrap())
        .await;
    stack
        .put(&wall_type(&context, PARTITION).to_record(&i(DOCUMENT)))
        .await;
    let concrete = wall_type(&context, "wall-concrete-200");
    stack.put(&concrete.to_record(&i(DOCUMENT))).await;
    stack.put(&wall("W-1", PARTITION, [0, 0], [4000, 0])).await;
    assert_eq!(stack.joins("W-1").await, Json::Null, "nothing meets it yet");
    stack
        .put(&wall("W-2", "wall-concrete-200", [4000, 0], [4000, 3000]))
        .await;
    let first = stack.joins("W-1").await;
    assert_eq!(first["with"], json!(["W-2"]), "{first}");
    assert_eq!(stack.joins("W-2").await["with"], json!(["W-1"]));
    assert_eq!(stack.joins_read("W-1").await, ["W-2"]);
    assert_eq!(stack.joins_read("W-2").await, ["W-1"]);
    // The partition's exterior lining runs to the concrete's outer face, 100 mm past the corner.
    let lining: Vec<&str> = first["layers"][0][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p[0].as_str().unwrap())
        .collect();
    assert!(lining.contains(&q0(4100 * 6400).as_str()), "{lining:?}");

    stack
        .put(&wall("W-3", PARTITION, [2000, -3000], [2000, 0]))
        .await;
    let tee = stack.joins("W-1").await;
    assert_eq!(tee["with"], json!(["W-2", "W-3"]), "{tee}");
    assert_eq!(
        tee["layers"][0].as_array().unwrap().len(),
        2,
        "its exterior lining is cut back for W-3's stud"
    );
    assert_eq!(stack.joins("W-3").await["with"], json!(["W-1"]));

    let veneer = assembly_of(&wall_type(&context, "wall-brick-veneer-90")).unwrap();
    stack
        .put(
            &with_assembly(concrete, &veneer)
                .unwrap()
                .to_record(&i(DOCUMENT)),
        )
        .await;
    let retyped = stack.joins("W-1").await;
    assert_eq!(retyped["with"], json!(["W-2", "W-3"]));
    assert_ne!(retyped["layers"], tee["layers"], "the new type's joint");

    stack
        .put(&wall("W-2", "wall-concrete-200", [6000, 0], [6000, 3000]))
        .await;
    let moved = stack.joins("W-1").await;
    assert_eq!(moved["with"], json!(["W-3"]), "{moved}");
    assert_eq!(stack.joins("W-2").await, Json::Null);
    assert_eq!(stack.joins_read("W-2").await, Vec::<String>::new());

    assert_eq!(stack.regen.verify(&i(PROJECT)).await.unwrap(), 3);
    let before = [
        stack.joins("W-1").await,
        stack.joins("W-2").await,
        stack.joins("W-3").await,
    ];
    stack.regen.rebuild(&i(PROJECT)).await.unwrap();
    let after = [
        stack.joins("W-1").await,
        stack.joins("W-2").await,
        stack.joins("W-3").await,
    ];
    assert_eq!(after, before, "a rebuild from empty gives the same joints");
}
