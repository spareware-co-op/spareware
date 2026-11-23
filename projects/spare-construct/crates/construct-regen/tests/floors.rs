//! A floor through the log and the regenerator, on the test store: its layers read from its
//! floor type and its boundary written as the loops the builders take, built again when its
//! type's layers change, standing on its level (its top face at the level's elevation plus its
//! offset) and built again when that level moves, and refused in words when its type or its
//! boundary cannot be read. The records go in as they are, through a command of this file's own
//! that only stores them: the commands that make floors and floor types are the server's
//! (construct-api).
//!
//! The builder is the stand-in, which builds a floor as the box around its outer loop; the
//! kernel's own floor is the kernel suite's (`tests/kernel_suite`).

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

use construct_categories::{CategoryTable, FLOORS};
use construct_levels::{Length, Level, Tie};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model};
use construct_regen::regenerator::Regenerator;
use construct_regen::slabs;
use construct_regen::stand_in::Boxes;
use construct_slabs::{
    Boundary, default_floor_types, filled as slab_filled, floor_record, layers_of, new_floor_type,
    reads_layers, with_layers,
};
use construct_types::ElementType;
use kit_core::Id;
use kit_ops::{Change, Edit};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Clock, Commands, Context as Moment, Orderers, Outcome, Reply};
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &str = "p";
const DOCUMENT: &str = "doc";
const SLAB: &str = "floor-concrete-150-screed-50";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
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
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), Arc::new(Boxes));
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

    /// An element's built bounds, or its failure's code and words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }
}

/// The L-shaped floor of construct-slabs' tests, 8 m by 8 m less its notch, with a 2 m square
/// hole at (1 m, 1 m), in a command's plain form.
fn l_with_hole() -> Boundary {
    let at = |id: &str, x: i64, y: i64| json!({ "id": id, "at": [x * 6400, y * 6400] });
    Boundary::from_plain(&json!({
        "outer": [
            at("e0", 0, 0),
            at("e1", 8000, 0),
            at("e2", 8000, 4000),
            at("e3", 4000, 4000),
            at("e4", 4000, 8000),
            at("e5", 0, 8000),
        ],
        "holes": [[
            at("h0", 1000, 1000),
            at("h1", 3000, 1000),
            at("h2", 3000, 3000),
            at("h3", 1000, 3000),
        ]],
    }))
    .unwrap()
}

/// The default floor type `id`.
fn floor_type(id: &str) -> ElementType {
    default_floor_types(&CategoryTable::new())
        .unwrap()
        .into_iter()
        .find(|ty| ty.id().as_str() == id)
        .unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// Floor `id` of type `ty`, its top tied to level `level` at `offset` mm, over the L with a hole.
fn floor(id: &str, ty: &str, level: &str, offset: i64) -> Element {
    let tie = Tie {
        level: i(level),
        offset: mm(offset),
    };
    floor_record(&i(id), &i(DOCUMENT), id, &i(ty), &tie, &l_with_hole())
}

/// A floor's recipe is filled from its type: its loops (the outer one anticlockwise, the hole
/// clockwise, each corner with its edge's ID in grid steps) and its type's layers, each its
/// function, material and thickness; nothing of the stored boundary is left. With no type, a type
/// that is not a floor type, or a boundary that does not read, it fails in words.
#[test]
fn a_floors_layers_are_read_from_its_type() {
    let ty = floor_type(SLAB);
    let element = floor("F-1", SLAB, "1", 0);
    assert!(slabs::reads_type(&element.recipe));
    let filled = slabs::filled(&element.recipe, Some(&ty)).unwrap();
    assert!(!slabs::reads_type(&filled));
    assert!(!reads_layers(&filled));
    let layers: Vec<(&str, &str, i64)> = filled.args["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["function"].as_str().unwrap(),
                l["material"].as_str().unwrap(),
                l["thickness"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        layers,
        [
            ("finish", "screed-cement", 320_000),
            ("structure", "concrete-regular", 960_000)
        ]
    );
    let slabs_own = layers_of(&ty).unwrap();
    assert_eq!(filled, slab_filled(&element.recipe, &slabs_own).unwrap());
    assert_eq!(
        filled.args["loops"][1][0],
        json!({ "id": "h3", "at": [6_400_000, 6_400_000] })
    );
    let built = Boxes.build(&filled).unwrap();
    assert_eq!(built.bounds.min, [0, 0, -200]);
    assert_eq!(built.bounds.max, [8000, 8000, 0]);

    let none = slabs::filled(&element.recipe, None).unwrap_err();
    assert_eq!(none.code, "NoFloorType");
    let plain = ElementType::new(i("plain"), "Plain", FLOORS, &CategoryTable::new()).unwrap();
    let refused = slabs::filled(&element.recipe, Some(&plain)).unwrap_err();
    assert_eq!(refused.code, "BadFloorType");
    assert!(refused.message.contains("plain"), "{}", refused.message);
    let mut broken = element.recipe.clone();
    broken.args.insert("boundary".to_owned(), json!({}));
    assert_eq!(
        slabs::filled(&broken, Some(&ty)).unwrap_err().code,
        "BadBoundary"
    );
    // A recipe that reads no floor type is left as it is.
    let wall = construct_model::geometry::Recipe::construct(
        "wall.straight",
        std::collections::BTreeMap::default(),
    );
    assert_eq!(slabs::filled(&wall, None).unwrap(), wall);
}

/// Through the log: a floor is built from its type's layers, its top face at 0 in its own frame
/// and its built row standing it on its level plus its offset; a change to its type's layers
/// builds it again with the new ones, its own record unchanged; a moved level moves it, its record
/// unchanged; a floor whose type is missing fails, naming it.
#[tokio::test]
async fn a_floor_is_built_again_when_its_type_or_its_level_changes() {
    let stack = Stack::new().await;
    let ty = floor_type(SLAB);
    stack.put(&ty.to_record(&i(DOCUMENT))).await;
    let level = Level::new(i("L-1"), "First floor", mm(3200), true).unwrap();
    stack.put(&level.to_record(&i(DOCUMENT))).await;
    stack.put(&floor("F-1", SLAB, "L-1", -20)).await;
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -200], [8000, 8000, 0]))
    );
    let row = stack.built("F-1").await;
    assert_eq!(row.level, Some(i("L-1")));
    assert_eq!(row.elevation.as_deref(), Some("length:q0:20352000"));
    let input = row.input;

    // The timber joist floor's layers: 260 mm in all.
    let thicker = layers_of(&floor_type("floor-timber-joist-240")).unwrap();
    stack
        .put(
            &with_layers(ty.clone(), &thicker)
                .unwrap()
                .to_record(&i(DOCUMENT)),
        )
        .await;
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -260], [8000, 8000, 0])),
        "its type's new layers"
    );
    assert_eq!(
        stack.built("F-1").await.input,
        input,
        "its record is not written"
    );

    let moved = Level::new(i("L-1"), "First floor", mm(3500), true).unwrap();
    stack.put(&moved.to_record(&i(DOCUMENT))).await;
    let row = stack.built("F-1").await;
    assert_eq!(row.elevation.as_deref(), Some("length:q0:22272000"));
    assert_eq!(row.input, input, "its record is not written");

    stack.put(&floor("F-2", "no-such-type", "L-1", 0)).await;
    let failed = stack.bounds("F-2").await.unwrap_err();
    assert!(failed.starts_with("BadParameter"), "{failed}");
    assert!(failed.contains("no-such-type"), "{failed}");

    // A floor type made by hand reads the same way.
    let own = new_floor_type(i("floor-own"), "Own", &thicker, &CategoryTable::new()).unwrap();
    stack.put(&own.to_record(&i(DOCUMENT))).await;
    stack.put(&floor("F-3", "floor-own", "L-1", 0)).await;
    assert_eq!(
        stack.bounds("F-3").await,
        Ok(([0, 0, -260], [8000, 8000, 0]))
    );
}

/// What a page shows of a floor after its category's parameters: its area, perimeter, thickness,
/// volume and mass, then each layer's volume and mass, worked out from its boundary and its type,
/// shown in the document's units and marked read-only. A layer of a material the default library
/// lacks has no mass, and then nor has the floor; a floor with no type, and an element that is not
/// a floor, show none.
#[test]
fn a_floors_read_only_quantities_are_worked_out_from_its_boundary_and_type() {
    use construct_params::{Area, Units, Value, Volume};
    let units = Units::default();
    let ty = floor_type("floor-concrete-200");
    let element = floor("F-1", "floor-concrete-200", "1", 0);
    let rows = slabs::read_only_view(&element, Some(&ty), &units);
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "floor.area",
            "floor.perimeter",
            "floor.thickness",
            "floor.volume",
            "floor.mass",
            "floor.layer-1.volume",
            "floor.layer-1.mass"
        ]
    );
    assert!(rows.iter().all(|row| row.from == slabs::READ_ONLY));
    let area = Value::Area(Area::from_mm2(44_000_000)).stored();
    assert_eq!(rows[0].value.as_deref(), Some(area.as_str()));
    assert_eq!(rows[0].shown.as_deref(), Some("44.00 m²"));
    assert_eq!(rows[1].shown.as_deref(), Some("40000 mm"));
    assert_eq!(rows[2].shown.as_deref(), Some("200 mm"));
    let volume = Value::Volume(Volume::from_mm3(8_800_000_000)).stored();
    assert_eq!(rows[3].value.as_deref(), Some(volume.as_str()));
    assert_eq!(rows[3].shown.as_deref(), Some("8.80 m³"));
    assert!(rows[4].value.is_some(), "concrete is in the library");
    assert_eq!(rows[4].value, rows[6].value, "one layer is the whole floor");

    let screed = floor_type(SLAB);
    let rows = slabs::read_only_view(&floor("F-2", SLAB, "1", 0), Some(&screed), &units);
    assert_eq!(rows.len(), 5 + 4);
    assert_eq!(rows[4].value, None, "no screed in the library yet");
    assert_eq!(rows[6].value, None);
    assert!(rows[8].value.is_some());

    assert_eq!(slabs::read_only_view(&element, None, &units).len(), 0);
    let level = Level::new(i("L-1"), "First floor", mm(3200), true).unwrap();
    let level = level.to_record(&i(DOCUMENT));
    assert_eq!(slabs::read_only_view(&level, None, &units).len(), 0);
}
