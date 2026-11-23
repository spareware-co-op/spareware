//! A roof through the log and the regenerator, on the test store: its layers read from its roof
//! type and its shape written as the solids the builders take, built again when its type's layers
//! change, standing on its level (its base at the level's elevation plus its offset) and built
//! again when that level moves, and refused in words when its type or its footprint cannot be
//! read. The records go in as they are, through a command of this file's own that only stores
//! them: the commands that make roofs and roof types are the server's (a later job).
//!
//! The builder is the stand-in, which builds a roof as the box around its layers and draws each
//! layer; the kernel's own roof is the kernel suite's (`tests/kernel_suite`).

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

use construct_categories::{CategoryTable, ROOFS};
use construct_levels::{Length, Level, Tie};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Model};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_regen::{roofs, slabs};
use construct_roofs::{
    Roof, default_roof_types, filled as roof_filled, layers_of, new_roof_type, roof_record,
    with_layers,
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
const STEEL: &str = "roof-steel-rafter-150";

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

/// The 6 m by 4 m gable at 1 in 4 of construct-roofs' tests: 500 mm over its eaves (`o0` and
/// `o2`), 300 mm over its verges.
fn gable_roof() -> Roof {
    let at = |x: i64, y: i64| json!({ "at": [x * 6400, y * 6400] });
    let edge = |overhang: i64, pitch: Option<&str>| match pitch {
        Some(pitch) => json!({ "overhang": overhang * 6400, "pitch": pitch }),
        None => json!({ "overhang": overhang * 6400 }),
    };
    Roof::from_plain(&json!({
        "footprint": { "outer": [at(0, 0), at(6000, 0), at(6000, 4000), at(0, 4000)] },
        "edges": {
            "o0": edge(500, Some("slope:1/4")),
            "o1": edge(300, None),
            "o2": edge(500, Some("slope:1/4")),
            "o3": edge(300, None),
        }
    }))
    .unwrap()
}

/// The default roof type `id`.
fn roof_type(id: &str) -> ElementType {
    default_roof_types(&CategoryTable::new())
        .unwrap()
        .into_iter()
        .find(|ty| ty.id().as_str() == id)
        .unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// Roof `id` of type `ty`, its base tied to level `level` at `offset` mm, the gable.
fn roof(id: &str, ty: &str, level: &str, offset: i64) -> Element {
    let tie = Tie {
        level: i(level),
        offset: mm(offset),
    };
    roof_record(&i(id), &i(DOCUMENT), id, &i(ty), &tie, &gable_roof())
}

/// A roof's recipe is filled from its type: its form, slopes, eaves outline and each layer's
/// solid, as construct-roofs' `filled` writes it; nothing of the stored footprint is left. With no
/// type, a type that is not a roof type, or a footprint that does not read, it fails in words.
#[test]
fn a_roofs_layers_are_read_from_its_type() {
    let ty = roof_type(STEEL);
    let element = roof("R-1", STEEL, "1", 0);
    assert!(roofs::reads_type(&element.recipe));
    assert!(!construct_regen::slabs::reads_type(&element.recipe));
    let filled = roofs::filled(&element.recipe, Some(&ty)).unwrap();
    assert!(!roofs::reads_type(&filled));
    assert_eq!(
        filled,
        roof_filled(&element.recipe, &layers_of(&ty).unwrap()).unwrap()
    );
    assert_eq!(filled.args["form"], json!("gable"));
    let built = Boxes.build(&filled).unwrap();
    assert_eq!(built.bounds.min, [-300, -500, -125]);
    assert_eq!(
        built.displays[0].parts.len(),
        2,
        "the membrane has no solid"
    );

    let none = roofs::filled(&element.recipe, None).unwrap_err();
    assert_eq!(none.code, "NoRoofType");
    let plain = ElementType::new(i("plain"), "Plain", ROOFS, &CategoryTable::new()).unwrap();
    let refused = roofs::filled(&element.recipe, Some(&plain)).unwrap_err();
    assert_eq!(refused.code, "BadRoofType");
    assert!(refused.message.contains("plain"), "{}", refused.message);
    let mut broken = element.recipe.clone();
    broken.args.insert("edges".to_owned(), json!([]));
    assert_eq!(
        roofs::filled(&broken, Some(&ty)).unwrap_err().code,
        "BadRoof"
    );
    let wall = construct_model::geometry::Recipe::construct(
        "wall.straight",
        std::collections::BTreeMap::default(),
    );
    assert_eq!(roofs::filled(&wall, None).unwrap(), wall);
}

/// Through the log: a roof is built from its type's layers, its base at 0 in its own frame and
/// its built row standing it on its level plus its offset; a change to its type's layers builds
/// it again with the new ones, its own record unchanged; a moved level moves it, its record
/// unchanged; a roof whose type is missing fails, naming it.
#[tokio::test]
async fn a_roof_is_built_again_when_its_type_or_its_level_changes() {
    let stack = Stack::new().await;
    let ty = roof_type(STEEL);
    stack.put(&ty.to_record(&i(DOCUMENT))).await;
    let level = Level::new(i("L-2"), "Roof", mm(5600), true).unwrap();
    stack.put(&level.to_record(&i(DOCUMENT))).await;
    stack.put(&roof("R-1", STEEL, "L-2", 100)).await;
    // The steel roof stands 150.55 mm square to its 1 in 4 slope: 155.18 mm upright, so its
    // ridge's top is 655.18 mm up and its box's top 656 mm.
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([-300, -500, -125], [6300, 4500, 656]))
    );
    let row = stack.built("R-1").await;
    assert_eq!(row.level, Some(i("L-2")));
    assert_eq!(row.elevation.as_deref(), Some("length:q0:36480000"));
    let input = row.input;

    // The concrete roof's layers: 300 mm, 309.2 mm upright.
    let thicker = layers_of(&roof_type("roof-concrete-200-insulated")).unwrap();
    stack
        .put(
            &with_layers(ty.clone(), &thicker)
                .unwrap()
                .to_record(&i(DOCUMENT)),
        )
        .await;
    assert_eq!(
        stack.bounds("R-1").await,
        Ok(([-300, -500, -125], [6300, 4500, 810])),
        "its type's new layers"
    );
    assert_eq!(
        stack.built("R-1").await.input,
        input,
        "its record is not written"
    );

    let moved = Level::new(i("L-2"), "Roof", mm(6000), true).unwrap();
    stack.put(&moved.to_record(&i(DOCUMENT))).await;
    let row = stack.built("R-1").await;
    assert_eq!(row.elevation.as_deref(), Some("length:q0:39040000"));
    assert_eq!(row.input, input, "its record is not written");

    stack.put(&roof("R-2", "no-such-type", "L-2", 0)).await;
    let failed = stack.bounds("R-2").await.unwrap_err();
    assert!(failed.starts_with("BadParameter"), "{failed}");
    assert!(failed.contains("no-such-type"), "{failed}");

    // A roof type made by hand reads the same way.
    let own = new_roof_type(i("roof-own"), "Own", &thicker, &CategoryTable::new()).unwrap();
    stack.put(&own.to_record(&i(DOCUMENT))).await;
    stack.put(&roof("R-3", "roof-own", "L-2", 0)).await;
    assert_eq!(
        stack.bounds("R-3").await,
        Ok(([-300, -500, -125], [6300, 4500, 810]))
    );
}

/// What a page shows of a roof after its category's parameters: its plan and slope areas,
/// thickness, volume, mass, pitch and ridge height, then each layer's volume and mass, shown in
/// the document's units and marked read-only; a roof with no type, and an element that is not a
/// roof, show none.
#[test]
fn a_roofs_read_only_quantities_are_worked_out_from_its_shape_and_type() {
    use construct_params::{Area, Units, Value};
    let units = Units::default();
    let ty = roof_type(STEEL);
    let element = roof("R-1", STEEL, "1", 0);
    let rows = roofs::read_only_view(&element, Some(&ty), &units);
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "roof.area",
            "roof.slope-area",
            "roof.thickness",
            "roof.volume",
            "roof.mass",
            "roof.pitch",
            "roof.ridge-height",
            "roof.layer-1.volume",
            "roof.layer-1.mass",
            "roof.layer-2.volume",
            "roof.layer-2.mass",
            "roof.layer-3.volume",
            "roof.layer-3.mass"
        ]
    );
    assert!(rows.iter().all(|row| row.from == slabs::READ_ONLY));
    let area = Value::Area(Area::from_mm2(33_000_000)).stored();
    assert_eq!(rows[0].value.as_deref(), Some(area.as_str()));
    assert_eq!(rows[0].shown.as_deref(), Some("33.00 m²"));
    assert_eq!(rows[1].shown.as_deref(), Some("34.02 m²"));
    assert_eq!(rows[5].value.as_deref(), Some("slope:1/4"));
    assert!(
        rows[4].value.is_some(),
        "every default material is in the library"
    );

    assert_eq!(roofs::read_only_view(&element, None, &units).len(), 0);
    let level = Level::new(i("L-1"), "First floor", mm(3200), true).unwrap();
    let level = level.to_record(&i(DOCUMENT));
    assert_eq!(roofs::read_only_view(&level, None, &units).len(), 0);
}
