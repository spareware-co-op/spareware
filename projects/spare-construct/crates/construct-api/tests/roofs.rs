//! Roof types and roofs through the edit log, on the test store: the default roof types made where
//! the document lacks them, and roof types made and changed by their own commands; a flat, a
//! mono-pitch and a gable roof on a rectangle, of a chosen type, on a level, as the builders take
//! it; its footprint, form, pitch, overhang, level and type changed; a type edit and a level move
//! building it again; the generic commands refused where a roof's own exist; every refusal in
//! words; undo and redo of every roof command; and the log replayed from empty to the same bytes.

// The crate denies these outside tests (Cargo.toml); clippy.toml's allow-*-in-tests settings don't
// reach helpers outside `#[test]` functions, and a broken helper must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::undo::{self, State};
use construct_api::cloud::{Access, ProjectCommands};
use construct_categories::ROOFS;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records, Context};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_roofs::{default_roofs, layers_of, roof_of};
use construct_types::ElementType;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply, read_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
/// How long a claim lasts here, as the server's setting would.
const CLAIM_MS: u64 = 30_000;
const STEEL: &str = "roof-steel-rafter-150";
const CONCRETE: &str = "roof-concrete-200-insulated";

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Says yes. Access has tests of its own, in the server, against the real rules.
struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

/// The orderer's time, set by the test, so a replay can run every edit at its recorded time.
struct Fixed(AtomicU64);

impl Clock for Fixed {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    /// A store with the rows the directory would have written, and no model yet.
    async fn bare() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, CLAIM_MS, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
        let regen = Regenerator::new(store.clone(), MemoryFiles::default(), geometry);
        let mut txn = store.begin().await.unwrap();
        let project = Project {
            id: i("p"),
            name: "P".to_owned(),
            hub: i("h"),
        };
        txn.put(Place::Project(i("p")).key(), to_row(&project).unwrap())
            .await
            .unwrap();
        let team = Team {
            id: i("t"),
            name: "T".to_owned(),
        };
        let place = Place::Team {
            project: i("p"),
            team: i("t"),
        };
        txn.put(place.key(), to_row(&team).unwrap()).await.unwrap();
        txn.commit().await.unwrap();
        Self {
            store,
            orderers,
            regen,
            clock,
            next: AtomicU64::new(0),
        }
    }

    async fn new() -> Self {
        let stack = Self::bare().await;
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn run(&self, edit: Edit) -> Result<u64, String> {
        match self
            .orderers
            .run(&Scope::Project(i("p")), edit)
            .await
            .unwrap()
        {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
    }

    async fn edit_as(&self, person: &str, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
        let edit = Edit {
            person: i(person),
            client: i(&format!("c{n}")),
            counter: 1,
            base: 0,
            target: i(DOCUMENT),
            cmd: cmd.to_owned(),
            args,
            author_ms: 0,
        };
        self.run(edit).await
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        self.edit_as("anna", cmd, args).await
    }

    /// Lets every claim run out.
    fn later(&self) {
        self.clock.0.fetch_add(CLAIM_MS + 1, Ordering::SeqCst);
    }

    async fn record(&self, id: &str) -> Option<Element> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .map(|(_, record)| record)
    }

    async fn context(&self) -> Context {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap()
    }

    /// A type of the document, read as construct-types reads it.
    async fn roof_type(&self, id: &str) -> ElementType {
        let context = self.context().await;
        let record = self.record(id).await.unwrap();
        ElementType::from_record(&record, &context.params, &context.categories).unwrap()
    }

    async fn built(&self, id: &str) -> BuiltElement {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap()
    }

    /// An element's built bounds, in whole millimetres, or its failure in words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// Every row under a table of the project, as bytes.
    async fn rows(&self, table: &str) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = Scope::Project(i("p")).table_prefix(table, &[]);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut rows = BTreeMap::new();
        for table in ["body", "display", "built", "area", "kind", "reads"] {
            rows.extend(self.rows(table).await);
        }
        rows
    }

    async fn state(&self, person: &str) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i(person))
            .await
            .unwrap()
    }
}

/// A default roof type's layers, as `roof.type.create` and `roof.type.layers` take them: each
/// thickness as typed text.
fn layers(default: &str) -> Value {
    let roofs = default_roofs().unwrap();
    let roof = roofs.iter().find(|roof| roof.id == default).unwrap();
    let layers: Vec<Value> = roof
        .layers
        .layers()
        .iter()
        .map(|layer| {
            json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str() })
        })
        .collect();
    Value::Array(layers)
}

/// A corner of a footprint as a command takes it: its edge's ID and where it is, in millimetres as
/// typed text.
fn at(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

/// A rectangle from (0, 0) to `x` by `y` mm, its edges `r0` (along x at the bottom) to `r3`.
fn rectangle(x: i64, y: i64) -> Value {
    json!({ "outer": [at("r0", 0, 0), at("r1", x, 0), at("r2", x, y), at("r3", 0, y)] })
}

/// Ground at 0 and First at 3 m.
async fn two_levels(stack: &Stack) {
    for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
}

/// `roof.create`'s arguments: roof `id` of type `ty` on First, over `footprint`, of `form`.
fn create(id: &str, ty: &str, footprint: &Value, form: &str, pitch: Option<&str>) -> Value {
    let mut args = json!({ "element": id, "type": ty, "level": "L-1", "footprint": footprint,
                           "form": form });
    if let Some(pitch) = pitch {
        args["pitch"] = json!(pitch);
    }
    args
}

/// A stack with two levels, the default roof types and an 8 m by 6 m gable roof `R-1` of the steel
/// type at 1:4 over the rectangle.
async fn with_gable() -> Stack {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack.edit("roof.defaults", json!({})).await.unwrap();
    let args = create("R-1", STEEL, &rectangle(8000, 6000), "gable", Some("1:4"));
    stack.edit("roof.create", args).await.unwrap();
    stack
}

/// The edges of a roof's record as `(id, overhang, pitch)`, from its stored form.
fn edges(record: &Element) -> Vec<(String, String, Option<String>)> {
    record.recipe.args["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            (
                edge["id"].as_str().unwrap().to_owned(),
                edge["overhang"].as_str().unwrap().to_owned(),
                edge["pitch"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

/// The default roof types are made where the document lacks them, in one entry, each listing every
/// parameter bound to roofs, and the parameter table is left as it is; asked again, nothing is
/// written and it is no step. A roof type is made, its layers changed, renamed, copied with its
/// layers and deleted only when no roof is of it, by its own commands, each refusal in words; the
/// generic type commands refuse a roof type, naming the roof command.
#[tokio::test]
async fn roof_types_are_made_and_changed_by_their_own_commands() {
    let stack = Stack::new().await;
    let note = json!({ "definition": { "id": "P-note", "name": "Note", "kind": "text",
                                       "holder": "type", "choices": [] } });
    stack.edit("parameter.define", note).await.unwrap();
    let bind = json!({ "parameter": "P-note", "categories": [ROOFS.0], "default": "none" });
    stack.edit("parameter.bind", bind).await.unwrap();
    let table = stack.record("parameters").await;
    stack.edit("roof.defaults", json!({})).await.unwrap();
    assert_eq!(stack.record("parameters").await, table);
    for default in default_roofs().unwrap() {
        let ty = stack.roof_type(default.id).await;
        assert_eq!(ty.name(), default.name);
        assert_eq!(ty.category(), ROOFS);
        assert_eq!(layers_of(&ty).unwrap(), default.layers);
        assert!(
            ty.has(&i("P-note")),
            "{} lists the roofs' parameters",
            default.id
        );
    }
    let made = stack.state("anna").await.undo;
    let rows = stack.rows("piece").await;
    stack.edit("roof.defaults", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, rows, "nothing more to make");
    assert_eq!(stack.state("anna").await.undo, made, "and no step");

    let own = json!({ "type": "R-own", "name": "Own", "layers": layers(CONCRETE) });
    stack.edit("roof.type.create", own).await.unwrap();
    assert_eq!(
        layers_of(&stack.roof_type("R-own").await).unwrap(),
        layers_of(&stack.roof_type(CONCRETE).await).unwrap()
    );
    for (args, words) in [
        (
            json!({ "type": "R-2", "name": "Concrete 200, 100 insulation",
                    "layers": layers(CONCRETE) }),
            "category 600 already has a type named \"Concrete 200, 100 insulation\"",
        ),
        (
            json!({ "type": CONCRETE, "name": "Again", "layers": layers(CONCRETE) }),
            "That element already exists.",
        ),
        (
            json!({ "type": "R-3", "name": "Empty", "layers": [] }),
            "a floor type needs at least one layer",
        ),
        (
            json!({ "type": "R-4", "name": "Flat", "layers": [{ "function": "structure",
                    "thickness": "0", "material": "concrete-regular" }] }),
            "layer 0 is 0 thick; only a membrane may be 0",
        ),
        (
            json!({ "type": "R-5", "name": "Odd", "layers": [{ "function": "structure",
                    "thickness": "two hundred", "material": "concrete-regular" }] }),
            "",
        ),
        (
            json!({ "type": "R-6", "name": "No layers" }),
            "This command needs the layers.",
        ),
    ] {
        let refused = stack.edit("roof.type.create", args).await.unwrap_err();
        if words.is_empty() {
            assert!(refused.starts_with("layer 0's thickness: "), "{refused}");
        } else {
            assert_eq!(refused, words);
        }
    }

    stack
        .edit(
            "roof.type.layers",
            json!({ "type": "R-own", "layers": layers(STEEL) }),
        )
        .await
        .unwrap();
    assert_eq!(
        layers_of(&stack.roof_type("R-own").await).unwrap(),
        layers_of(&stack.roof_type(STEEL).await).unwrap()
    );
    stack
        .edit(
            "roof.type.rename",
            json!({ "type": "R-own", "name": "Mine" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.roof_type("R-own").await.name(), "Mine");
    stack
        .edit(
            "roof.type.duplicate",
            json!({ "type": "R-own", "new": "R-copy", "name": "Copy" }),
        )
        .await
        .unwrap();
    assert_eq!(
        layers_of(&stack.roof_type("R-copy").await).unwrap(),
        layers_of(&stack.roof_type(STEEL).await).unwrap()
    );
    stack
        .edit("roof.type.delete", json!({ "type": "R-copy" }))
        .await
        .unwrap();
    assert!(stack.record("R-copy").await.is_none());
    two_levels(&stack).await;
    stack
        .edit(
            "roof.create",
            create("R-1", "R-own", &rectangle(4000, 3000), "flat", None),
        )
        .await
        .unwrap();
    let refused = stack
        .edit("roof.type.delete", json!({ "type": "R-own" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "type \"Mine\" cannot be deleted: 1 instances use it"
    );

    // The roof type commands refuse a type that is not a roof type.
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    for (cmd, args) in [
        (
            "roof.type.layers",
            json!({ "type": "T-door", "layers": layers(STEEL) }),
        ),
        ("roof.type.rename", json!({ "type": "T-door", "name": "D" })),
        (
            "roof.type.duplicate",
            json!({ "type": "T-door", "new": "T-d2", "name": "D2" }),
        ),
        ("roof.type.delete", json!({ "type": "T-door" })),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(
            refused, "a type of category 300 is not a roof type",
            "{cmd}"
        );
    }

    // The generic type commands do not go behind them.
    for (cmd, args, words) in [
        (
            "type.create",
            json!({ "type": "T-r", "name": "Plain roof", "category": ROOFS.0 }),
            "A roof type is made with roof.type.create, which checks it as a roof.",
        ),
        (
            "type.rename",
            json!({ "type": STEEL, "name": "C" }),
            "A roof type is renamed with roof.type.rename, which checks it as a roof.",
        ),
        (
            "type.duplicate",
            json!({ "type": STEEL, "new": "T-c2", "name": "C2" }),
            "A roof type is copied with roof.type.duplicate, which checks it as a roof.",
        ),
        (
            "type.delete",
            json!({ "type": STEEL }),
            "A roof type is deleted with roof.type.delete, which checks it as a roof.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, words, "{cmd}");
    }
    stack
        .edit("type.rename", json!({ "type": "T-door", "name": "Doors" }))
        .await
        .unwrap();

    // Ben may not change a roof type while Anna is working on it.
    let refused = stack
        .edit_as(
            "ben",
            "roof.type.layers",
            json!({ "type": "R-own", "layers": layers(CONCRETE) }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
}

/// A roof is made in each form on a rectangle, of a chosen type, on a level: its record holds its
/// footprint and edges in their stored form, its layers read from its type, its tie and its type,
/// and it is built from them; a change to its type's layers and a move of its level build it
/// again, its record unwritten.
#[tokio::test]
async fn a_roof_is_made_flat_mono_or_gable_on_a_rectangle() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack.edit("roof.defaults", json!({})).await.unwrap();

    // Flat, with an overhang of 300 mm all round.
    let args = json!({ "element": "R-flat", "name": "Flat roof", "type": CONCRETE,
                       "level": "L-1", "offset": "-20 mm", "footprint": rectangle(8000, 6000),
                       "overhang": "300 mm" });
    stack.edit("roof.create", args).await.unwrap();
    let flat = stack.record("R-flat").await.unwrap();
    assert_eq!(flat.kind, "roof");
    assert_eq!(flat.name, "Flat roof");
    assert_eq!(flat.level, 0);
    assert_eq!(flat.recipe.op, "roof");
    assert_eq!(flat.recipe.args["layers"], json!({ "type": "layers" }));
    assert_eq!(flat.props["@type"], CONCRETE);
    assert_eq!(flat.props["@category"], records::write_category(ROOFS));
    assert_eq!(flat.props["@level"], "L-1");
    let roof = roof_of(&flat.recipe).unwrap();
    assert_eq!(roof.form().name(), "flat");
    assert!(
        edges(&flat)
            .iter()
            .all(|(_, overhang, pitch)| { overhang == "length:q0:1920000" && pitch.is_none() })
    );
    let row = stack.built("R-flat").await;
    assert_eq!(row.level, Some(i("L-1")));
    assert_eq!(row.elevation.as_deref(), Some("length:q0:19072000"));

    // Mono-pitch, rising from its first edge when none is named.
    let mono = create("R-mono", STEEL, &rectangle(8000, 6000), "mono", Some("25%"));
    stack.edit("roof.create", mono).await.unwrap();
    let record = stack.record("R-mono").await.unwrap();
    let rows = edges(&record);
    assert_eq!(rows[0].2.as_deref(), Some("slope:1/4"));
    assert!(rows[1..].iter().all(|(_, _, pitch)| pitch.is_none()));
    assert_eq!(roof_of(&record.recipe).unwrap().form().name(), "mono");

    // Gable: the longer pair of opposite edges, then a named pair; an angle is held as typed.
    let gable = create(
        "R-gable",
        STEEL,
        &rectangle(8000, 6000),
        "gable",
        Some("30°"),
    );
    stack.edit("roof.create", gable).await.unwrap();
    let record = stack.record("R-gable").await.unwrap();
    let rows = edges(&record);
    assert_eq!(rows[0].2.as_deref(), Some("angle:270000000"));
    assert_eq!(rows[2].2.as_deref(), Some("angle:270000000"));
    assert!(rows[1].2.is_none() && rows[3].2.is_none());
    let mut across = create(
        "R-across",
        STEEL,
        &rectangle(8000, 6000),
        "gable",
        Some("1:4"),
    );
    across["eaves"] = json!(["r1", "r3"]);
    stack.edit("roof.create", across).await.unwrap();
    let rows = edges(&stack.record("R-across").await.unwrap());
    assert!(rows[0].2.is_none() && rows[1].2.is_some() && rows[3].2.is_some());
    // Of a taller footprint the longer pair is the other one.
    let tall = create(
        "R-tall",
        STEEL,
        &rectangle(6000, 8000),
        "gable",
        Some("1:4"),
    );
    stack.edit("roof.create", tall).await.unwrap();
    let rows = edges(&stack.record("R-tall").await.unwrap());
    assert!(rows[1].2.is_some() && rows[3].2.is_some() && rows[0].2.is_none());

    // Built: each has a body, none failed.
    for id in ["R-flat", "R-mono", "R-gable", "R-across", "R-tall"] {
        let bounds = stack.bounds(id).await;
        assert!(bounds.is_ok(), "{id}: {bounds:?}");
    }
    let (low, high) = stack.bounds("R-flat").await.unwrap();
    assert_eq!([low[0], low[1], high[0], high[1]], [-300, -300, 8300, 6300]);
    let (_, high) = stack.bounds("R-gable").await.unwrap();
    assert!(high[2] > 0);

    // Its type's layers changed: built again; its record is not written.
    stack
        .edit(
            "roof.type.layers",
            json!({ "type": CONCRETE, "layers": layers(STEEL) }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("R-flat").await.unwrap(), flat);
    // First moved up: the roof stands on it again; its record is not written.
    stack
        .edit(
            "level.elevation",
            json!({ "level": "L-1", "elevation": "3.2 m" }),
        )
        .await
        .unwrap();
    assert_eq!(stack.record("R-flat").await.unwrap(), flat);
    let row = stack.built("R-flat").await;
    assert_eq!(row.elevation.as_deref(), Some("length:q0:20352000"));
}

/// Every refusal of a roof's making is in words and writes nothing: the command's own, and
/// construct-roofs' in its words, naming the edges and the job that lifts it.
#[tokio::test]
async fn roofs_refuse_what_the_data_crate_refuses_in_its_words() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    stack.edit("roof.defaults", json!({})).await.unwrap();
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "roof.create",
            create("R-1", STEEL, &rectangle(4000, 3000), "flat", None),
        )
        .await
        .unwrap();
    let base = create("R-9", STEEL, &rectangle(4000, 3000), "gable", Some("1:4"));
    let with = |changes: &[(&str, Value)]| {
        let mut args = base.clone();
        for (key, value) in changes {
            if value.is_null() {
                args.as_object_mut().unwrap().remove(*key);
            } else {
                args[*key] = value.clone();
            }
        }
        args
    };
    let hole = json!({ "outer": rectangle(4000, 3000)["outer"],
                       "holes": [[at("h0", 1000, 1000), at("h1", 2000, 1000),
                                  at("h2", 2000, 2000)]] });
    let slanted = json!({ "outer": [at("s0", 0, 0), at("s1", 4000, 0), at("s2", 4000, 3000),
                                    at("s3", 1000, 3000)] });
    let triangle = json!({ "outer": [at("t0", 0, 0), at("t1", 4000, 0), at("t2", 0, 3000)] });
    for (args, words) in [
        (with(&[("type", json!("T-none"))]), "no type has id T-none"),
        (
            with(&[("type", json!("T-door"))]),
            "an instance of category 600 cannot take a type of category 300",
        ),
        (
            with(&[("level", Value::Null)]),
            "A roof stands on a level: this command needs a level.",
        ),
        (
            with(&[("level", json!("L-none"))]),
            "There is no level L-none in this model.",
        ),
        (
            with(&[("element", json!("R-1"))]),
            "That element already exists.",
        ),
        (
            with(&[("footprint", Value::Null)]),
            "This command needs a footprint.",
        ),
        (
            with(&[(
                "footprint",
                json!({ "outer": [at("t0", 0, 0), at("t1", 1000, 0)] }),
            )]),
            "R-9's footprint: the outer loop has 2 edges; a loop needs at least three",
        ),
        (
            with(&[
                ("footprint", hole),
                ("form", json!("flat")),
                ("pitch", Value::Null),
            ]),
            "R-9: a roof's footprint has no holes, and this one has 1: an opening through a roof \
             is cut by job 4.2's openings",
        ),
        (
            with(&[("footprint", slanted)]),
            "R-9: edges s0 and s2 slope, and a sloped roof's footprint is a rectangle along the \
             plan's axes for now: other footprints come with job 6.2",
        ),
        (
            with(&[("footprint", triangle), ("eaves", json!(["t0", "t1"]))]),
            "R-9: edges t0 and t1 slope, and a sloped roof's footprint is a rectangle along the \
             plan's axes for now: other footprints come with job 6.2",
        ),
        (
            with(&[("eaves", json!(["r0", "r1"]))]),
            "R-9: edges r0 and r1 slope and meet at a corner: hips come with job 6.2",
        ),
        (
            with(&[("eaves", json!(["r0"]))]),
            "A gable rises from two opposite edges.",
        ),
        (
            with(&[("eaves", json!(["r0", "r9"]))]),
            "R-9: edge r9 is not an edge of the roof's footprint",
        ),
        (
            with(&[("form", json!("mono")), ("eaves", json!(["r0", "r2"]))]),
            "A mono-pitch roof rises from one edge.",
        ),
        (
            with(&[("form", json!("hip"))]),
            "A roof's form is flat, mono or gable.",
        ),
        (
            with(&[("form", json!("flat"))]),
            "A flat roof does not slope: leave the pitch out, or choose mono or gable.",
        ),
        (
            with(&[("pitch", Value::Null)]),
            "A roof that slopes needs a pitch, such as 1:4, 25% or 30°.",
        ),
        (
            with(&[("pitch", json!("0%"))]),
            "R-9: edge r0's slope does not rise: give no slope for an edge that does not slope",
        ),
        (
            with(&[("pitch", json!("11:1"))]),
            "R-9: edge r0 slopes steeper than 10 in 1 (about 84.3 degrees): a face that steep is \
             a wall",
        ),
        (
            with(&[("overhang", json!("-1 m"))]),
            "R-9: edge r0's overhang is less than nothing",
        ),
    ] {
        let refused = stack.edit("roof.create", args.clone()).await.unwrap_err();
        assert_eq!(refused, words, "{args}");
    }
    for (key, text) in [("pitch", "steep"), ("overhang", "a lot")] {
        let refused = stack
            .edit("roof.create", with(&[(key, json!(text))]))
            .await
            .unwrap_err();
        assert!(
            refused.to_lowercase().contains("pitch") || refused.contains(text),
            "{key}: {refused}"
        );
    }
    let refused = stack
        .edit(
            "roof.create",
            with(&[
                ("form", json!("flat")),
                ("pitch", Value::Null),
                ("eaves", json!(["r0"])),
            ]),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "A flat roof rises from no edge: leave eaves out, or choose mono or gable."
    );
    // A flat roof's overhang needs every edge along an axis.
    let off = json!({ "outer": [at("s0", 0, 0), at("s1", 4000, 0), at("s2", 4000, 3000),
                                at("s3", 1000, 3000)] });
    let overhang = json!({ "element": "R-8", "type": STEEL, "level": "L-1", "footprint": off,
                           "overhang": "200 mm" });
    let refused = stack.edit("roof.create", overhang).await.unwrap_err();
    assert_eq!(
        refused,
        "R-8: edge s3 runs off the plan's axes, so this footprint's overhangs have no exact eaves \
         outline yet: they come with job 6.2"
    );
    // A flat roof on a footprint off the axes is fine without an overhang.
    let flat = json!({ "element": "R-8", "type": STEEL, "level": "L-1", "footprint": off });
    stack.edit("roof.create", flat).await.unwrap();
    assert!(stack.record("R-9").await.is_none());
}

/// A roof's footprint, form, pitch and overhang are changed by their own commands, each built
/// again, each refusal in words; an uneven gable is refused in construct-roofs' words; another
/// person's claim turns it back.
#[tokio::test]
async fn a_roof_is_changed_by_its_own_commands() {
    let stack = with_gable().await;
    let before = stack.record("R-1").await.unwrap();

    // The pitch: every edge the roof rises from, then one.
    stack
        .edit("roof.pitch", json!({ "element": "R-1", "pitch": "30%" }))
        .await
        .unwrap();
    let rows = edges(&stack.record("R-1").await.unwrap());
    assert_eq!(rows[0].2.as_deref(), Some("slope:3/10"));
    assert_eq!(rows[2].2.as_deref(), Some("slope:3/10"));
    assert!(rows[1].2.is_none());
    let refused = stack
        .edit(
            "roof.pitch",
            json!({ "element": "R-1", "pitch": "1:3", "edge": "r0" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "R-1: edges r0 and r2 slope at different pitches: an uneven gable's ridge is solved with \
         job 6.2"
    );
    for (args, words) in [
        (
            json!({ "element": "R-1", "pitch": "1:3", "edge": "r1" }),
            "R-1 does not rise from edge r1: change its form with roof.form.",
        ),
        (
            json!({ "element": "R-1", "pitch": "1:3", "edge": "r9" }),
            "\"r9\" is not an edge of this roof.",
        ),
        (json!({ "element": "R-1" }), "This command needs a pitch."),
        (
            json!({ "element": "R-1", "pitch": "0%" }),
            "R-1: edge r0's slope does not rise: give no slope for an edge that does not slope",
        ),
    ] {
        let refused = stack.edit("roof.pitch", args).await.unwrap_err();
        assert_eq!(refused, words);
    }

    // The overhang: all edges, then one.
    stack
        .edit(
            "roof.overhang",
            json!({ "element": "R-1", "overhang": "400 mm" }),
        )
        .await
        .unwrap();
    assert!(
        edges(&stack.record("R-1").await.unwrap())
            .iter()
            .all(|(_, overhang, _)| overhang == "length:q0:2560000")
    );
    stack
        .edit(
            "roof.overhang",
            json!({ "element": "R-1", "overhang": "0", "edge": "r3" }),
        )
        .await
        .unwrap();
    let rows = edges(&stack.record("R-1").await.unwrap());
    assert_eq!(rows[3].1, "length:q0:0");
    assert_eq!(rows[0].1, "length:q0:2560000");
    let refused = stack
        .edit(
            "roof.overhang",
            json!({ "element": "R-1", "overhang": "-5 mm" }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "R-1: edge r0's overhang is less than nothing");

    // The form: mono keeps the pitch it had and every overhang; flat drops the pitch.
    stack
        .edit("roof.form", json!({ "element": "R-1", "form": "mono" }))
        .await
        .unwrap();
    let rows = edges(&stack.record("R-1").await.unwrap());
    assert_eq!(rows[0].2.as_deref(), Some("slope:3/10"));
    assert!(rows[1..].iter().all(|(_, _, pitch)| pitch.is_none()));
    assert_eq!(rows[0].1, "length:q0:2560000");
    assert_eq!(rows[3].1, "length:q0:0");
    stack
        .edit(
            "roof.form",
            json!({ "element": "R-1", "form": "gable", "pitch": "20°", "eaves": ["r1", "r3"] }),
        )
        .await
        .unwrap();
    let rows = edges(&stack.record("R-1").await.unwrap());
    assert!(rows[0].2.is_none() && rows[2].2.is_none());
    assert_eq!(rows[1].2.as_deref(), Some("angle:180000000"));
    stack
        .edit("roof.form", json!({ "element": "R-1", "form": "flat" }))
        .await
        .unwrap();
    assert!(
        edges(&stack.record("R-1").await.unwrap())
            .iter()
            .all(|(_, _, pitch)| pitch.is_none())
    );
    let refused = stack
        .edit("roof.pitch", json!({ "element": "R-1", "pitch": "1:4" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "R-1 is flat, so it has no pitch to change: give it a form with roof.form."
    );
    let refused = stack
        .edit("roof.form", json!({ "element": "R-1", "form": "mono" }))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "A roof that slopes needs a pitch, such as 1:4, 25% or 30°."
    );
    let refused = stack
        .edit(
            "roof.form",
            json!({ "element": "R-1", "form": "mono", "pitch": "1:4", "eaves": ["r0", "r1"] }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "A mono-pitch roof rises from one edge.");
    let refused = stack
        .edit("roof.form", json!({ "element": "R-1" }))
        .await
        .unwrap_err();
    assert_eq!(refused, "This command needs a form.");

    // The footprint: the same edges keep what they do; one the roof rises from may not go.
    stack
        .edit(
            "roof.form",
            json!({ "element": "R-1", "form": "gable", "pitch": "1:4" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "roof.footprint",
            json!({ "element": "R-1", "footprint": rectangle(10_000, 5000) }),
        )
        .await
        .unwrap();
    let record = stack.record("R-1").await.unwrap();
    assert_eq!(edges(&record)[0].2.as_deref(), Some("slope:1/4"));
    assert_eq!(roof_of(&record.recipe).unwrap().form().name(), "gable");
    let renamed = json!({ "outer": [at("q0", 0, 0), at("q1", 6000, 0), at("q2", 6000, 4000),
                                    at("q3", 0, 4000)] });
    let refused = stack
        .edit(
            "roof.footprint",
            json!({ "element": "R-1", "footprint": renamed }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "R-1 rises from edge r0, which the new footprint lacks: change its form with roof.form first."
    );
    let off = json!({ "outer": [at("r0", 0, 0), at("r1", 4000, 0), at("r2", 4000, 3000),
                                at("r3", 1000, 3000)] });
    let refused = stack
        .edit(
            "roof.footprint",
            json!({ "element": "R-1", "footprint": off }),
        )
        .await
        .unwrap_err();
    assert!(
        refused.starts_with(
            "R-1: edges r0 and r2 slope, and a sloped roof's footprint is a rectangle"
        ),
        "{refused}"
    );
    let refused = stack
        .edit(
            "roof.footprint",
            json!({ "element": "R-1", "footprint": rectangle(0, 0) }),
        )
        .await
        .unwrap_err();
    assert!(refused.starts_with("R-1's footprint: "), "{refused}");

    // What is not a roof, or is gone.
    stack
        .edit(
            "level.create",
            json!({ "level": "L-2", "name": "Second", "elevation": "6 m" }),
        )
        .await
        .unwrap();
    for cmd in [
        "roof.footprint",
        "roof.form",
        "roof.pitch",
        "roof.overhang",
        "roof.level",
        "roof.retype",
        "roof.delete",
    ] {
        let refused = stack
            .edit(cmd, json!({ "element": "L-2" }))
            .await
            .unwrap_err();
        assert_eq!(refused, "Second is not a roof.", "{cmd}");
        let refused = stack
            .edit(cmd, json!({ "element": "R-none" }))
            .await
            .unwrap_err();
        assert_eq!(refused, "That element no longer exists.", "{cmd}");
    }

    // Another person's claim turns a change back; once it has run out, it goes through.
    let refused = stack
        .edit_as(
            "ben",
            "roof.pitch",
            json!({ "element": "R-1", "pitch": "1:5" }),
        )
        .await
        .unwrap_err();
    assert_eq!(refused, "anna is working on this.");
    stack.later();
    stack
        .edit_as(
            "ben",
            "roof.pitch",
            json!({ "element": "R-1", "pitch": "1:5" }),
        )
        .await
        .unwrap();
    assert_ne!(stack.record("R-1").await.unwrap(), before);
}

/// A roof's level and type are changed by roof commands that check it as a roof, and it is deleted
/// by its own command; the generic element commands do not go behind the roof's own.
#[tokio::test]
async fn a_roofs_level_type_and_end_are_its_own_commands() {
    let stack = with_gable().await;
    let level = json!({ "element": "R-1", "level": "L-g", "offset": "100 mm" });
    stack.edit("roof.level", level).await.unwrap();
    let record = stack.record("R-1").await.unwrap();
    assert_eq!(record.props["@level"], "L-g");
    let row = stack.built("R-1").await;
    assert_eq!(row.level, Some(i("L-g")));
    for (args, words) in [
        (
            json!({ "element": "R-1" }),
            "A roof stands on a level: this command needs a level.",
        ),
        (
            json!({ "element": "R-1", "level": "L-none" }),
            "There is no level L-none in this model.",
        ),
    ] {
        let refused = stack.edit("roof.level", args).await.unwrap_err();
        assert_eq!(refused, words);
    }

    stack
        .edit("roof.retype", json!({ "element": "R-1", "type": CONCRETE }))
        .await
        .unwrap();
    assert_eq!(stack.record("R-1").await.unwrap().props["@type"], CONCRETE);
    stack
        .edit(
            "type.create",
            json!({ "type": "T-door", "name": "Door", "category": 300 }),
        )
        .await
        .unwrap();
    for (ty, words) in [
        ("T-door", "a type of category 300 is not a roof type"),
        ("T-none", "no type has id T-none"),
    ] {
        let refused = stack
            .edit("roof.retype", json!({ "element": "R-1", "type": ty }))
            .await
            .unwrap_err();
        assert_eq!(refused, words);
    }
    let refused = stack
        .edit("roof.retype", json!({ "element": "R-1" }))
        .await
        .unwrap_err();
    assert_eq!(refused, "This command needs a roof type.");

    // The generic element commands do not go behind the roof's own.
    let recipe = stack.record("R-1").await.unwrap().recipe;
    for (cmd, args, words) in [
        (
            "element.create",
            json!({ "element": "R-x", "kind": "roof", "placement": { "x": 0, "y": 0, "z": 0 },
                    "recipe": recipe }),
            "A roof is made with roof.create, which checks it as a roof.",
        ),
        (
            "element.set",
            json!({ "element": "R-1", "recipe": recipe }),
            "R-1's footprint, form, pitch and overhangs are changed with roof.footprint, \
             roof.form, roof.pitch or roof.overhang, which check them as a roof.",
        ),
        (
            "element.delete",
            json!({ "element": "R-1" }),
            "R-1 is a roof: delete it with roof.delete.",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert_eq!(refused, words, "{cmd}");
    }
    stack
        .edit(
            "element.set",
            json!({ "element": "R-1", "name": "Main roof" }),
        )
        .await
        .unwrap();

    stack
        .edit("roof.delete", json!({ "element": "R-1" }))
        .await
        .unwrap();
    assert!(stack.record("R-1").await.is_none());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("R-1").await.unwrap().name, "Main roof");
}

/// Runs a command, undoes it and redoes it: the undo puts every piece back as it was before the
/// command, and the redo as it was after.
async fn undone_and_redone(stack: &Stack, cmd: &str, args: Value) {
    let before = stack.rows("piece").await;
    stack
        .edit(cmd, args)
        .await
        .unwrap_or_else(|why| panic!("{cmd}: {why}"));
    let after = stack.rows("piece").await;
    assert_ne!(before, after, "{cmd} changed something");
    assert_eq!(
        stack.state("anna").await.undo.map(|step| step.cmd),
        Some(cmd.to_owned())
    );
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, before, "{cmd} undone");
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, after, "{cmd} redone");
}

#[tokio::test]
async fn every_roof_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    undone_and_redone(&stack, "roof.defaults", json!({})).await;
    for (cmd, args) in [
        (
            "roof.type.create",
            json!({ "type": "R-own", "name": "Own", "layers": layers(STEEL) }),
        ),
        (
            "roof.create",
            create(
                "R-1",
                CONCRETE,
                &rectangle(8000, 6000),
                "gable",
                Some("1:4"),
            ),
        ),
        ("roof.pitch", json!({ "element": "R-1", "pitch": "30°" })),
        (
            "roof.overhang",
            json!({ "element": "R-1", "overhang": "250 mm" }),
        ),
        (
            "roof.form",
            json!({ "element": "R-1", "form": "mono", "pitch": "1:5" }),
        ),
        (
            "roof.footprint",
            json!({ "element": "R-1", "footprint": rectangle(9000, 6000) }),
        ),
        (
            "roof.level",
            json!({ "element": "R-1", "level": "L-g", "offset": "0" }),
        ),
        ("roof.retype", json!({ "element": "R-1", "type": STEEL })),
        (
            "roof.type.layers",
            json!({ "type": CONCRETE, "layers": layers(STEEL) }),
        ),
        (
            "roof.type.rename",
            json!({ "type": CONCRETE, "name": "Renamed" }),
        ),
        (
            "roof.type.duplicate",
            json!({ "type": "R-own", "new": "R-copy", "name": "Copy" }),
        ),
        ("roof.type.delete", json!({ "type": "R-copy" })),
        ("roof.delete", json!({ "element": "R-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    two_levels(&stack).await;
    for (cmd, args) in [
        ("roof.defaults", json!({})),
        (
            "roof.create",
            create("R-1", STEEL, &rectangle(8000, 6000), "gable", Some("1:4")),
        ),
        (
            "roof.create",
            create("R-2", CONCRETE, &rectangle(0, 3000), "flat", None),
        ),
        (
            "roof.create",
            create("R-3", CONCRETE, &rectangle(6000, 3000), "mono", Some("30°")),
        ),
        (
            "roof.type.layers",
            json!({ "type": STEEL, "layers": layers(CONCRETE) }),
        ),
        ("edit.undo", json!({})),
        ("roof.pitch", json!({ "element": "R-3", "pitch": "1:3" })),
        (
            "roof.overhang",
            json!({ "element": "R-1", "overhang": "300 mm" }),
        ),
        ("roof.delete", json!({ "element": "R-3" })),
        ("roof.defaults", json!({})),
        (
            "level.elevation",
            json!({ "level": "L-1", "elevation": "2.95 m" }),
        ),
    ] {
        let _ = stack.edit(cmd, args).await;
    }
    stack.regen.catch_up(&i("p")).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let head: u64 = read_row(&mut snapshot, &Scope::Project(i("p")).head())
        .await
        .unwrap()
        .unwrap();
    let entries = read_log(&mut snapshot, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert!(entries.iter().any(|entry| entry.failed.is_some()));

    let again = Stack::bare().await;
    for entry in &entries {
        again.clock.0.store(entry.orderer_ms, Ordering::SeqCst);
        let edit = Edit {
            person: entry.author.clone(),
            client: entry.client.clone(),
            counter: entry.counter,
            base: entry.base,
            target: entry.target.clone(),
            cmd: entry.cmd.clone(),
            args: entry.args.clone(),
            author_ms: entry.author_ms,
        };
        let _ = again.run(edit).await;
    }
    again.regen.catch_up(&i("p")).await.unwrap();
    for table in ["record", "piece"] {
        assert_eq!(again.rows(table).await, stack.rows(table).await, "{table}");
    }
    assert_eq!(again.derived().await, stack.derived().await);
    let mut replayed = again.store.snapshot().await.unwrap();
    let after = read_log(&mut replayed, &Scope::Project(i("p")), 0, head)
        .await
        .unwrap();
    assert_eq!(after, entries);
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
