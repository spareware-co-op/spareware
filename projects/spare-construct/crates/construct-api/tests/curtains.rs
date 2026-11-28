//! Curtain wall types and curtain walls through the edit log, on the test store: the default types
//! made where the document lacks them, and types made and changed by their own commands; a curtain
//! wall drawn on its own line, its grid lines added and removed, segments left out and put back,
//! panels swapped (a door of the library among them), its quantities read back; one embedded in a
//! layered wall, cutting the opening the kernel builder's rules for openings allow; the generic
//! commands refused where a curtain wall's own exist; every refusal in words; undo and redo of
//! every curtain command; and the log replayed from empty to the same bytes. The builder is the
//! stand-in; the kernel's own solids are construct-regen's kernel suite's.

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
use construct_api::cloud::{Access, ProjectCommands, curtains, walls};
use construct_categories::CURTAIN_WALLS;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records, At, Context};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
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
/// The default 1.5 m by 1 m type, with 50 by 150 mm mullions everywhere.
const GRID: &str = "curtain-1500x1000";
/// The default storefront type.
const STOREFRONT: &str = "curtain-storefront";
/// The library's single-swing door, by its permanent ID.
const DOOR: &str = "library.doors-windows.single-swing-door";

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
    async fn type_of(&self, id: &str) -> ElementType {
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

impl Stack {
    /// A curtain wall's read-only quantities, by parameter ID, in stored text: what a page shows
    /// and a schedule lists, from its filled recipe as it is now.
    async fn quantities(&self, id: &str) -> BTreeMap<String, String> {
        let record = self.record(id).await.unwrap();
        let mut snapshot = self.store.snapshot().await.unwrap();
        let filled = construct_regen::levels::recipe_at(
            &mut snapshot,
            &i("p"),
            &i(DOCUMENT),
            &record,
            At::Now,
        )
        .await
        .unwrap()
        .unwrap();
        construct_regen::curtains::read_only_view(&filled, &construct_params::Units::default())
            .into_iter()
            .map(|row| (row.id, row.value.unwrap()))
            .collect()
    }

    /// The document's curtain wall types view.
    async fn view(&self) -> curtains::CurtainTypesView {
        let mut snapshot = self.store.snapshot().await.unwrap();
        curtains::read_curtain_types(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap()
    }
}

/// A stored count.
fn count(n: i64) -> String {
    construct_params::Value::number(construct_params::Decimal::new(n, 0)).stored()
}

/// A stored area of whole square millimetres.
fn mm2(n: i64) -> String {
    construct_params::Value::Area(construct_params::Area::from_mm2(n)).stored()
}

/// A stored length of whole millimetres.
fn mm(n: i64) -> String {
    construct_params::Value::Length(construct_params::Length::checked_from_mm(n).unwrap()).stored()
}

/// `curtain.draw`'s arguments: `id` of type `ty`, from the origin `length` along x, 3 m high, on
/// no level.
fn draw(id: &str, ty: &str, length: &str) -> Value {
    json!({ "element": id, "type": ty, "start": ["0", "0"], "end": [length, "0"], "height": "3 m" })
}

/// A stack with the default curtain wall types and the 6 m by 3 m wall `CW-1` of the 1.5 m by
/// 1 m type.
async fn with_wall() -> Stack {
    let stack = Stack::new().await;
    stack.edit("curtain.defaults", json!({})).await.unwrap();
    stack
        .edit("curtain.draw", draw("CW-1", GRID, "6 m"))
        .await
        .unwrap();
    stack
}

/// A grid of 4 equal columns and no rows, solid 50 mm panels and no mullions, as a person sends it.
fn four_columns() -> Value {
    json!({ "vertical": { "layout": "count", "count": 4 }, "horizontal": { "layout": "none" },
            "panel": { "kind": "solid", "thickness": "50 mm", "material": "plaster-gypsum" } })
}

/// The default types are made where the document lacks them, in one entry; asked again, nothing is
/// written. A type is made, its grid and parts changed, renamed, copied and deleted by its own
/// commands; the view lists each with its grid and parts as the commands take them, and the
/// defaults. The type commands do not go behind them, and a grid the data crate refuses is refused
/// in its words.
#[tokio::test]
async fn curtain_types_are_made_and_changed_by_their_own_commands() {
    let stack = Stack::new().await;
    stack.edit("curtain.defaults", json!({})).await.unwrap();
    let pieces = stack.rows("piece").await;
    stack.edit("curtain.defaults", json!({})).await.unwrap();
    assert_eq!(stack.rows("piece").await, pieces, "nothing more to make");
    let view = stack.view().await;
    let names: Vec<&str> = view.types.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Curtain wall 1500 x 1000", "Glazing", "Storefront"]);
    assert_eq!(view.defaults.len(), 3);
    assert_eq!(
        view.types[0].spec["vertical"],
        json!({ "layout": "spacing", "spacing": "1500 mm", "justify": "centre" })
    );
    assert_eq!(
        view.types[0].spec["mullions"]["border-vertical"]["width"],
        "50 mm"
    );

    let own = json!({ "type": "CW-own", "name": "Own", "spec": four_columns() });
    stack.edit("curtain.type.create", own).await.unwrap();
    let ty = stack.type_of("CW-own").await;
    assert_eq!(ty.category(), CURTAIN_WALLS);
    let spec = construct_curtain::spec_of(&ty).unwrap();
    assert_eq!(
        spec.layout(construct_curtain::Direction::Vertical),
        construct_curtain::Layout::Count { count: 4 }
    );
    let mut most = four_columns();
    most["vertical"] = json!({ "layout": "most", "spacing": "1.2 m" });
    most["crossing"] = json!("horizontal");
    stack
        .edit(
            "curtain.type.spec",
            json!({ "type": "CW-own", "spec": most }),
        )
        .await
        .unwrap();
    let spec = construct_curtain::spec_of(&stack.type_of("CW-own").await).unwrap();
    assert_eq!(spec.crossing(), construct_curtain::Crossing::Horizontal);

    stack
        .edit(
            "curtain.type.rename",
            json!({ "type": GRID, "name": "Grid" }),
        )
        .await
        .unwrap();
    stack
        .edit(
            "curtain.type.duplicate",
            json!({ "type": "CW-own", "new": "CW-copy", "name": "Copy" }),
        )
        .await
        .unwrap();
    assert_eq!(
        construct_curtain::spec_of(&stack.type_of("CW-copy").await).unwrap(),
        spec
    );
    stack
        .edit("curtain.type.delete", json!({ "type": "CW-copy" }))
        .await
        .unwrap();
    assert!(stack.record("CW-copy").await.is_none());

    let generic = stack
        .edit(
            "type.create",
            json!({ "type": "CW-x", "name": "X", "category": CURTAIN_WALLS.0 }),
        )
        .await
        .unwrap_err();
    assert!(generic.contains("curtain.type.create"), "{generic}");
    let rename = stack
        .edit("type.rename", json!({ "type": GRID, "name": "Other" }))
        .await
        .unwrap_err();
    assert!(rename.contains("curtain.type.rename"), "{rename}");
    let mut zero = four_columns();
    zero["vertical"] = json!({ "layout": "spacing", "spacing": "0" });
    let refused = stack
        .edit(
            "curtain.type.create",
            json!({ "type": "CW-0", "name": "Zero", "spec": zero }),
        )
        .await
        .unwrap_err();
    assert!(refused.contains("vertical spacing"), "{refused}");
    let partition = walls::default_type_args().unwrap().remove(0);
    let wall_type = partition["type"].as_str().unwrap().to_owned();
    stack.edit("wall.type.create", partition).await.unwrap();
    let other = stack
        .edit(
            "curtain.type.spec",
            json!({ "type": wall_type, "spec": four_columns() }),
        )
        .await
        .unwrap_err();
    assert!(other.contains("category"), "{other}");
}

/// Worked by hand: the 6 m by 3 m wall of the 1.5 m by 1 m type, 50 mm mullions everywhere, is 12
/// panels (16.1 m2 between the mullions) and 31 mullions (38 m), built as the stand-in's box of its
/// whole extent; each grid edit changes what it reads back, and a type's change builds it again.
#[tokio::test]
async fn a_curtain_wall_is_drawn_and_its_grid_changed_by_its_own_commands() {
    let stack = with_wall().await;
    assert_eq!(
        stack.bounds("CW-1").await,
        Ok(([0, -75, 0], [6000, 75, 3000]))
    );
    let q = stack.quantities("CW-1").await;
    assert_eq!(q["curtain.length"], mm(6000));
    assert_eq!(q["curtain.panels"], count(12));
    assert_eq!(q["curtain.panel-area"], mm2(16_100_000));
    assert_eq!(q["curtain.glazed-area"], mm2(16_100_000));
    assert_eq!(q["curtain.mullions"], count(31));
    assert_eq!(q["curtain.mullion-length"], mm(38_000));
    let record = stack.record("CW-1").await.unwrap();
    assert_eq!(record.kind, "curtain");
    assert_eq!(records::stored_category(&record), Some(CURTAIN_WALLS));

    let edit = |cmd: &'static str, args: Value| {
        let mut args = args;
        args["element"] = json!("CW-1");
        (cmd, args)
    };
    for ((cmd, args), panels) in [
        (
            edit(
                "curtain.grid.add",
                json!({ "direction": "vertical", "at": "750 mm" }),
            ),
            15,
        ),
        (
            edit(
                "curtain.grid.remove",
                json!({ "direction": "vertical", "at": "3 m" }),
            ),
            12,
        ),
        (
            edit(
                "curtain.segment.omit",
                json!({ "direction": "vertical", "at": "1.5 m", "from": "0" }),
            ),
            11,
        ),
        (
            edit(
                "curtain.segment.restore",
                json!({ "direction": "vertical", "at": "1.5 m", "from": "0" }),
            ),
            12,
        ),
        (
            edit(
                "curtain.grid.add",
                json!({ "direction": "horizontal", "at": "2.5 m" }),
            ),
            16,
        ),
    ] {
        stack
            .edit(cmd, args)
            .await
            .unwrap_or_else(|e| panic!("{cmd}: {e}"));
        let q = stack.quantities("CW-1").await;
        assert_eq!(q["curtain.panels"], count(panels), "after {cmd}");
        assert!(stack.bounds("CW-1").await.is_ok(), "{cmd} builds");
    }
    let solid = json!({ "kind": "solid", "thickness": "50 mm", "material": "plaster-gypsum" });
    let before = stack.quantities("CW-1").await;
    stack
        .edit(
            "curtain.panel",
            json!({ "element": "CW-1", "at": ["4.5 m", "0"], "panel": solid }),
        )
        .await
        .unwrap();
    let after = stack.quantities("CW-1").await;
    assert_ne!(after["curtain.glazed-area"], before["curtain.glazed-area"]);
    assert_eq!(after["curtain.panel-area"], before["curtain.panel-area"]);
    stack
        .edit(
            "curtain.panel",
            json!({ "element": "CW-1", "at": ["4.5 m", "0"], "panel": null }),
        )
        .await
        .unwrap();
    assert_eq!(stack.quantities("CW-1").await, before);

    // The type's change builds every curtain wall of it again: no mullions, no room taken.
    let mut bare = four_columns();
    bare["panel"] = json!({ "kind": "glazed", "thickness": "24 mm", "material": "glass-plate" });
    stack
        .edit("curtain.type.spec", json!({ "type": GRID, "spec": bare }))
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("CW-1").await,
        Ok(([0, -12, 0], [6000, 12, 3000]))
    );
    let q = stack.quantities("CW-1").await;
    assert_eq!(q["curtain.mullions"], count(0));
    assert_eq!(q["curtain.panel-area"], mm2(18_000_000));
}

/// A door of the library is swapped into a cell, sized to it: a storefront 6 m by 3 m with a
/// horizontal line added at 2.1 m (its type has no horizontal mullions), the door in the cell from
/// 1.2 m along, its family's steps drawn as parts. A family the library lacks, or a type its family
/// lacks, is refused, naming it.
#[tokio::test]
async fn a_door_of_the_library_is_swapped_into_a_cell() {
    let stack = Stack::new().await;
    stack.edit("curtain.defaults", json!({})).await.unwrap();
    stack
        .edit("curtain.draw", draw("CW-1", STOREFRONT, "6 m"))
        .await
        .unwrap();
    stack
        .edit(
            "curtain.grid.add",
            json!({ "element": "CW-1", "direction": "horizontal", "at": "2.1 m" }),
        )
        .await
        .unwrap();
    let door = json!({ "kind": "door", "family": DOOR, "type": "d900x2100" });
    stack
        .edit(
            "curtain.panel",
            json!({ "element": "CW-1", "at": ["1.2 m", "0"], "panel": door }),
        )
        .await
        .unwrap();
    assert!(stack.bounds("CW-1").await.is_ok());
    let q = stack.quantities("CW-1").await;
    assert_eq!(q["curtain.panels"], count(10));
    for (panel, words) in [
        (
            json!({ "kind": "door", "family": "no-such-family", "type": "d900x2100" }),
            "no family",
        ),
        (
            json!({ "kind": "door", "family": DOOR, "type": "x1" }),
            "has no type",
        ),
    ] {
        let refused = stack
            .edit(
                "curtain.panel",
                json!({ "element": "CW-1", "at": ["2.4 m", "0"], "panel": panel }),
            )
            .await
            .unwrap_err();
        assert!(refused.contains(words), "{refused}");
    }
}

/// A curtain wall embedded in a layered wall stands on the wall's line from where its hosting puts
/// it and is built there; the wall is built with it. One that would reach past the wall's end, or
/// be as high as the wall, is refused by the kernel builder's rules for openings, naming it; a
/// curtain wall hosts nothing.
#[tokio::test]
async fn a_curtain_wall_is_embedded_in_a_wall() {
    let stack = Stack::new().await;
    stack.edit("curtain.defaults", json!({})).await.unwrap();
    let partition = walls::default_type_args().unwrap().remove(0);
    let wall_type = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    let wall = json!({ "element": "W-1", "type": wall_type, "start": ["0", "0"],
                       "end": ["6 m", "0"], "height": "3 m" });
    stack.edit("wall.draw", wall).await.unwrap();
    let embed = |id: &str, length: &str, height: &str| {
        json!({ "element": id, "type": GRID, "host": "W-1", "along": "1 m", "length": length,
                "height": height })
    };
    stack
        .edit("curtain.embed", embed("CW-1", "4 m", "2.4 m"))
        .await
        .unwrap();
    assert_eq!(
        stack.bounds("CW-1").await,
        Ok(([1000, -75, 0], [5000, 75, 2400]))
    );
    assert!(stack.bounds("W-1").await.is_ok());
    let q = stack.quantities("CW-1").await;
    assert_eq!(q["curtain.length"], mm(4000));
    assert_eq!(q["curtain.height"], mm(2400));
    stack
        .edit(
            "curtain.grid.add",
            json!({ "element": "CW-1", "direction": "vertical", "at": "1 m" }),
        )
        .await
        .unwrap();
    assert!(stack.bounds("CW-1").await.is_ok());
    for (id, length, height) in [("CW-2", "5.5 m", "2.4 m"), ("CW-3", "1 m", "3 m")] {
        let refused = stack
            .edit("curtain.embed", embed(id, length, height))
            .await
            .unwrap_err();
        assert!(refused.contains(id), "{refused}");
    }
    let on_curtain = json!({ "element": "CW-4", "type": GRID, "host": "CW-1", "along": "0",
                             "length": "1 m", "height": "1 m" });
    let refused = stack.edit("curtain.embed", on_curtain).await.unwrap_err();
    assert!(refused.contains("not a wall"), "{refused}");
}

/// Every refusal is the command's result, in words, and writes nothing.
#[tokio::test]
async fn curtain_walls_refuse_in_words() {
    let stack = with_wall().await;
    let pieces = stack.rows("piece").await;
    let partition = walls::default_type_args().unwrap().remove(0);
    let wall_type = partition["type"].as_str().unwrap().to_owned();
    stack.edit("wall.type.create", partition).await.unwrap();
    let pieces_with_wall_type = stack.rows("piece").await;
    assert_ne!(pieces, pieces_with_wall_type);
    let on = |args: Value| {
        let mut args = args;
        args["element"] = json!("CW-1");
        args
    };
    let curtain_recipe = construct_curtain::curtain_recipe(
        [construct_params::Length::from_mm(0); 2],
        [
            construct_params::Length::from_mm(1000),
            construct_params::Length::from_mm(0),
        ],
        json!(mm(3000)),
        &construct_curtain::Edits::new(),
    )
    .unwrap();
    for (cmd, args, words) in [
        (
            "curtain.draw",
            json!({ "element": "CW-2", "type": GRID, "start": ["1 m", "0"], "end": ["1 m", "0"],
                    "height": "3 m" }),
            "has a length",
        ),
        ("curtain.draw", draw("CW-2", &wall_type, "6 m"), "category"),
        (
            "curtain.draw",
            json!({ "element": "CW-2", "type": GRID, "start": ["0", "0"], "end": ["6 m", "0"] }),
            "top tied to a level or a height",
        ),
        ("curtain.draw", draw("CW-1", GRID, "6 m"), "already exists"),
        (
            "curtain.grid.add",
            on(json!({ "direction": "vertical", "at": "6 m" })),
            "not inside the curtain wall",
        ),
        (
            "curtain.grid.add",
            on(json!({ "direction": "vertical", "at": "1.5 m" })),
            "within 0.01 mm",
        ),
        (
            "curtain.grid.add",
            on(json!({ "direction": "diagonal", "at": "1 m" })),
            "not a direction",
        ),
        (
            "curtain.grid.remove",
            on(json!({ "direction": "horizontal", "at": "1.5 m" })),
            "no horizontal grid line at 1500 mm",
        ),
        (
            "curtain.segment.omit",
            on(json!({ "direction": "vertical", "at": "1.5 m", "from": "500 mm" })),
            "no segment starting at 500 mm",
        ),
        (
            "curtain.panel",
            on(json!({ "at": ["1 m", "0"], "panel": null })),
            "no panel's cell starts at",
        ),
        (
            "curtain.grid.add",
            json!({ "element": "missing", "direction": "vertical", "at": "1 m" }),
            "no longer exists",
        ),
        (
            "element.create",
            json!({ "element": "CW-9", "kind": "curtain", "recipe": curtain_recipe }),
            "curtain.draw",
        ),
        (
            "element.set",
            on(json!({ "recipe": curtain_recipe })),
            "curtain.grid",
        ),
        (
            "hosted.place",
            json!({ "element": "CW-9", "host": "CW-1", "face": "box.exterior", "along": "0",
                    "recipe": curtain_recipe }),
            "curtain.embed",
        ),
    ] {
        let refused = stack.edit(cmd, args).await.unwrap_err();
        assert!(refused.contains(words), "{cmd}: {refused}");
        assert_eq!(
            stack.rows("piece").await,
            pieces_with_wall_type,
            "{cmd} wrote nothing"
        );
    }
    // Leaving out two segments round one corner would make a panel that is no rectangle.
    stack
        .edit(
            "curtain.segment.omit",
            on(json!({ "direction": "vertical", "at": "1.5 m", "from": "0" })),
        )
        .await
        .unwrap();
    let refused = stack
        .edit(
            "curtain.segment.omit",
            on(json!({ "direction": "horizontal", "at": "1 m", "from": "0" })),
        )
        .await
        .unwrap_err();
    assert!(refused.contains("would not be a rectangle"), "{refused}");
}

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

/// Every curtain command is one step: undone, it puts back every piece it wrote; redone, it
/// writes them again.
#[tokio::test]
async fn every_curtain_command_is_undone_and_redone() {
    let stack = Stack::new().await;
    undone_and_redone(&stack, "curtain.defaults", json!({})).await;
    let partition = walls::default_type_args().unwrap().remove(0);
    let wall_type = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    let wall = json!({ "element": "W-1", "type": wall_type, "start": ["0", "0"],
                       "end": ["6 m", "0"], "height": "3 m" });
    stack.edit("wall.draw", wall).await.unwrap();
    for (cmd, args) in [
        (
            "curtain.type.create",
            json!({ "type": "CW-own", "name": "Own", "spec": four_columns() }),
        ),
        ("curtain.draw", draw("CW-1", GRID, "6 m")),
        (
            "curtain.embed",
            json!({ "element": "CW-2", "type": GRID, "host": "W-1", "along": "1 m",
                    "length": "3 m", "height": "2 m" }),
        ),
        (
            "curtain.grid.add",
            json!({ "element": "CW-1", "direction": "vertical", "at": "750 mm" }),
        ),
        (
            "curtain.grid.remove",
            json!({ "element": "CW-1", "direction": "horizontal", "at": "1 m" }),
        ),
        (
            "curtain.segment.omit",
            json!({ "element": "CW-1", "direction": "vertical", "at": "1.5 m", "from": "0" }),
        ),
        (
            "curtain.segment.restore",
            json!({ "element": "CW-1", "direction": "vertical", "at": "1.5 m", "from": "0" }),
        ),
        (
            "curtain.panel",
            json!({ "element": "CW-1", "at": ["3 m", "0"], "panel": { "kind": "empty" } }),
        ),
        (
            "curtain.retype",
            json!({ "element": "CW-1", "type": "CW-own" }),
        ),
        (
            "curtain.type.spec",
            json!({ "type": "CW-own", "spec": { "vertical": { "layout": "most", "spacing": "1.2 m" },
                    "horizontal": { "layout": "none" }, "panel": { "kind": "empty" } } }),
        ),
        (
            "curtain.type.rename",
            json!({ "type": "CW-own", "name": "Renamed" }),
        ),
        (
            "curtain.type.duplicate",
            json!({ "type": "CW-own", "new": "CW-copy", "name": "Copy" }),
        ),
        ("curtain.type.delete", json!({ "type": "CW-copy" })),
        ("curtain.delete", json!({ "element": "CW-2" })),
        ("curtain.delete", json!({ "element": "CW-1" })),
    ] {
        undone_and_redone(&stack, cmd, args).await;
    }
}

#[tokio::test]
async fn replaying_the_log_from_empty_gives_the_same_bytes() {
    let stack = Stack::new().await;
    let partition = walls::default_type_args().unwrap().remove(0);
    let wall_type = partition["type"].clone();
    for (cmd, args) in [
        ("curtain.defaults", json!({})),
        ("wall.type.create", partition),
        (
            "wall.draw",
            json!({ "element": "W-1", "type": wall_type, "start": ["0", "0"],
                    "end": ["6 m", "0"], "height": "3 m" }),
        ),
        ("curtain.draw", draw("CW-1", GRID, "6 m")),
        (
            "curtain.embed",
            json!({ "element": "CW-2", "type": STOREFRONT, "host": "W-1", "along": "1 m",
                    "length": "3 m", "height": "2 m" }),
        ),
        (
            "curtain.grid.add",
            json!({ "element": "CW-1", "direction": "vertical", "at": "750 mm" }),
        ),
        (
            "curtain.segment.omit",
            json!({ "element": "CW-1", "direction": "vertical", "at": "1.5 m", "from": "0" }),
        ),
        ("edit.undo", json!({})),
        (
            "curtain.grid.add",
            json!({ "element": "CW-1", "direction": "vertical", "at": "6 m" }),
        ),
        (
            "curtain.panel",
            json!({ "element": "CW-1", "at": ["3 m", "1 m"], "panel": { "kind": "empty" } }),
        ),
        (
            "curtain.type.spec",
            json!({ "type": GRID, "spec": four_columns() }),
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
    let incremental = stack.derived().await;
    stack.regen.rebuild(&i("p")).await.unwrap();
    assert_eq!(stack.derived().await, incremental);
}
