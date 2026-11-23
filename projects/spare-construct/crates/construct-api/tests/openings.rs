//! Openings and shafts through the edit log, on the test store (job 4.2, piece A66): an opening in
//! a wall (`opening.place`), one in a floor (`opening.draw`) and a shaft through the floors in its
//! reach (`shaft.create`), each changed in size, place or reach and deleted by its own commands,
//! each with undo and redo, every refusal in words, the generic commands turned away naming the
//! right one, a host deleted under an opening refused as under a door, and a floor's area and
//! volume after the cut as the page reads them.
//!
//! The builder is the stand-in; what the kernel's builder cuts on the same rules is measured in
//! the server's `tests/kernel.rs`. The fit each command asks is the builders' own rules, asked
//! without an engine, so what is refused here is refused on both.

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

use construct_api::cloud::{Access, ProjectCommands, openings, walls};
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_regen::parameters::{self as records};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
const CLAIM_MS: u64 = 30_000;
const CONCRETE: &str = "floor-concrete-200";

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

/// The orderer's time, set by the test.
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
    async fn new() -> Self {
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
        let stack = Self {
            store,
            orderers,
            regen,
            clock,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
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
        match self
            .orderers
            .run(&kit_store::Scope::Project(i("p")), edit)
            .await
            .unwrap()
        {
            Reply::Ack { seq, .. } => Ok(seq),
            Reply::Rejected { reason, .. } => Err(reason),
            Reply::Retry { .. } => Err("retry".to_owned()),
        }
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

    /// Whether an element builds, or its failure in words.
    async fn builds(&self, id: &str) -> Result<(), String> {
        match self.built(id).await.result {
            BuiltResult::Shape { .. } => Ok(()),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    /// What cuts a floor, as the page is told with its filled recipe.
    async fn cut_by(&self, id: &str) -> openings::FloorCuts {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let floor = self.record(id).await.unwrap();
        openings::floor_view(&mut snapshot, &i("p"), &i(DOCUMENT), &floor)
            .await
            .unwrap()
    }

    /// What a page shows of a floor after the cut: its area and volume, as shown.
    async fn quantities(&self, id: &str) -> (String, String) {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let floor = self.record(id).await.unwrap();
        let context = records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap();
        let ty = records::type_of(&floor).unwrap();
        let ty = records::read_type(&mut snapshot, &i("p"), &i(DOCUMENT), &ty, &context)
            .await
            .unwrap()
            .unwrap();
        let cuts = openings::floor_cuts(&mut snapshot, &i("p"), &i(DOCUMENT), &floor)
            .await
            .unwrap();
        let view = context.view_cut(&floor, Some(&ty), &cuts).unwrap();
        let shown = |key: &str| {
            view.parameters
                .iter()
                .find(|p| p.id == key)
                .and_then(|p| p.shown.clone())
                .unwrap()
        };
        (shown("floor.area"), shown("floor.volume"))
    }
}

fn at(id: &str, x: i64, y: i64) -> Value {
    json!({ "id": id, "at": [x.to_string(), y.to_string()] })
}

/// An outline as the commands take it: the rectangle from `(x0, y0)` to `(x1, y1)` millimetres.
fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Value {
    json!([
        [x0.to_string(), y0.to_string()],
        [x1.to_string(), y0.to_string()],
        [x1.to_string(), y1.to_string()],
        [x0.to_string(), y1.to_string()]
    ])
}

/// Ground at 0, First at 3 m, Second at 6 m; a concrete floor F-1 on First, 8 m by 6 m (200 mm
/// thick); a layered wall W-1, 4 m along x from (1000, 2000), on First, 2.7 m high.
async fn a_storey() -> Stack {
    let stack = Stack::new().await;
    for (id, name, elevation) in [
        ("L-g", "Ground", "0"),
        ("L-1", "First", "3 m"),
        ("L-2", "Second", "6 m"),
    ] {
        let args = json!({ "level": id, "name": name, "elevation": elevation });
        stack.edit("level.create", args).await.unwrap();
    }
    stack.edit("floor.defaults", json!({})).await.unwrap();
    let boundary = json!({ "outer": [at("r0", 0, 0), at("r1", 8000, 0), at("r2", 8000, 6000),
                                     at("r3", 0, 6000)] });
    let floor = json!({ "element": "F-1", "type": CONCRETE, "level": "L-1",
                        "boundary": boundary });
    stack.edit("floor.create", floor).await.unwrap();
    let partition = walls::default_type_args().unwrap().remove(0);
    let ty = partition["type"].clone();
    stack.edit("wall.type.create", partition).await.unwrap();
    let draw = json!({ "element": "W-1", "type": ty, "start": ["1000 mm", "2000 mm"],
                       "end": ["5000 mm", "2000 mm"], "level": "L-1", "height": "2700 mm" });
    stack.edit("wall.draw", draw).await.unwrap();
    stack
}

/// `opening.place`'s arguments: `id` in `host`'s `side`, `along` from its start, sill `sill`,
/// `width` by `height`.
fn place(id: &str, along: &str, sill: &str, width: &str, height: &str) -> Value {
    json!({ "element": id, "host": "W-1", "side": "exterior", "along": along, "sill": sill,
            "width": width, "height": height })
}

/// An opening in a wall is made on a side of the wall, hosted as a door is, in the openings
/// category on its wall's level; changed in size, moved along the wall and to its other side, and
/// deleted, each one undo step and redone to the same bytes; its refusals are in the kernel
/// builder's words naming it.
#[tokio::test]
async fn an_opening_in_a_wall_is_made_changed_and_deleted() {
    let stack = a_storey().await;
    stack
        .edit(
            "opening.place",
            place("O-1", "1 m", "900 mm", "1200 mm", "1200 mm"),
        )
        .await
        .unwrap();
    let opening = stack.record("O-1").await.unwrap();
    assert_eq!(opening.kind, "opening");
    assert_eq!(opening.props["@category"], "1600");
    assert_eq!(opening.props["@host.along"], "length:q0:6400000");
    assert_eq!(opening.props["@host.up"], "length:q0:5760000");
    assert_eq!(opening.props["@level"], "L-1");
    assert_eq!(opening.recipe.op, "opening.wall");
    assert_eq!(opening.recipe.args["width"], "length:q0:7680000");
    assert_eq!(opening.recipe.args["height"], "length:q0:7680000");
    assert_eq!(opening.recipe.args["axis"], "x");
    assert_eq!(opening.references.len(), 1);
    assert_eq!(opening.references[0].element.as_str(), "W-1");
    assert_eq!(opening.references[0].signature.as_deref(), Some("exterior"));
    // On the wall's line (y 2000 less nothing: it is as thick as its wall), 1 m along from x 1000.
    assert_eq!(opening.placement.x, 2000);
    stack.builds("W-1").await.unwrap();
    stack.builds("O-1").await.unwrap();

    // Undo takes it away and redo puts it back, the same record.
    let hash = stack.record("O-1").await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("O-1").await.is_none());
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("O-1").await.unwrap(), hash);

    // Its size, and its place along its wall and up it, then to the wall's other side.
    stack
        .edit(
            "opening.size",
            json!({ "element": "O-1", "width": "1.5 m" }),
        )
        .await
        .unwrap();
    let wider = stack.record("O-1").await.unwrap();
    assert_eq!(wider.recipe.args["width"], "length:q0:9600000");
    assert_eq!(wider.recipe.args["height"], "length:q0:7680000");
    stack
        .edit(
            "opening.move",
            json!({ "element": "O-1", "along": "2 m", "sill": "600 mm" }),
        )
        .await
        .unwrap();
    let moved = stack.record("O-1").await.unwrap();
    assert_eq!(moved.props["@host.along"], "length:q0:12800000");
    assert_eq!(moved.props["@host.up"], "length:q0:3840000");
    stack
        .edit(
            "opening.move",
            json!({ "element": "O-1", "side": "interior" }),
        )
        .await
        .unwrap();
    assert_eq!(
        stack.record("O-1").await.unwrap().references[0]
            .signature
            .as_deref(),
        Some("interior")
    );
    stack.builds("W-1").await.unwrap();

    // Each is one step: three undos take the move, the side and the size back.
    for _ in 0..3 {
        stack.edit("edit.undo", json!({})).await.unwrap();
    }
    assert_eq!(stack.record("O-1").await.unwrap(), hash);

    stack
        .edit("opening.delete", json!({ "element": "O-1" }))
        .await
        .unwrap();
    assert!(stack.record("O-1").await.is_none());
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("O-1").await.unwrap(), hash);
    stack.builds("W-1").await.unwrap();
}

/// What is refused for an opening in a wall is in words that name it and say why, and nothing is
/// written: past the wall's end, taller than the wall, over a door's or another opening's place,
/// on something that is not a wall, in a wall at an angle, a length that is not one, a size of
/// nothing; and a change that would not fit.
#[tokio::test]
async fn an_opening_in_a_wall_is_refused_in_words() {
    let stack = a_storey().await;
    stack
        .edit(
            "opening.place",
            place("O-1", "1 m", "900 mm", "1200 mm", "1200 mm"),
        )
        .await
        .unwrap();
    let wall = json!({ "element": "O-2", "host": "W-1", "side": "exterior", "along": "3.5 m",
                       "width": "1200 mm", "height": "1200 mm" });
    let mut cases: Vec<(&str, Value, &str)> = vec![
        (
            "opening.place",
            wall,
            "opening O-2: it crosses the wall's end",
        ),
        (
            "opening.place",
            place("O-2", "1.5 m", "1 m", "1 m", "1 m"),
            "opening O-1 and opening O-2: their openings overlap",
        ),
        (
            "opening.place",
            place("O-2", "2.8 m", "1 m", "1 m", "2 m"),
            "opening O-2: its head is at or above the wall's top, which leaves no wall over it",
        ),
        (
            "opening.place",
            place("O-1", "2.8 m", "1 m", "1 m", "1 m"),
            "That element already exists.",
        ),
        (
            "opening.place",
            place("O-2", "ahead", "0", "1 m", "1 m"),
            "",
        ),
        (
            "opening.place",
            place("O-2", "2 m", "0", "0 mm", "1 m"),
            "opening O-2: its width must be above 0",
        ),
        (
            "opening.place",
            json!({ "element": "O-2", "host": "F-1", "side": "exterior", "along": "1 m",
                    "width": "1 m", "height": "1 m" }),
            "F-1 is not a wall: only a wall hosts an opening.",
        ),
        (
            "opening.place",
            json!({ "element": "O-2", "host": "nothing", "side": "exterior", "along": "1 m",
                    "width": "1 m", "height": "1 m" }),
            "There is no element nothing in this model to host it.",
        ),
        (
            "opening.size",
            json!({ "element": "O-1", "width": "4 m" }),
            "opening O-1: it crosses the wall's end",
        ),
        (
            "opening.size",
            json!({ "element": "O-1" }),
            "A new size needs a width, a height or both.",
        ),
        (
            "opening.size",
            json!({ "element": "O-1", "width": "1200 mm" }),
            "O-1 is already that size.",
        ),
        (
            "opening.move",
            json!({ "element": "O-1", "along": "3.5 m" }),
            "opening O-1: it crosses the wall's end",
        ),
        (
            "opening.move",
            json!({ "element": "O-1" }),
            "A move needs a position along the wall, a sill height, another wall or another side.",
        ),
        (
            "opening.outline",
            json!({ "element": "O-1", "outline": rect(0, 0, 100, 100) }),
            "O-1 is an opening in a wall: change its size with opening.size, move it with \
             opening.move, delete it with opening.delete.",
        ),
        (
            "shaft.reach",
            json!({ "element": "O-1", "level": "L-g" }),
            "O-1 is an opening in a wall: change its size with opening.size, move it with \
             opening.move, delete it with opening.delete.",
        ),
    ];
    // The words of a length that is not one, and of a size of nothing, are construct-params' and
    // construct-openings': named by what they say about it.
    for (cmd, args, words) in cases.drain(..) {
        let before = stack.record("O-1").await.unwrap();
        let refused = stack.edit(cmd, args).await.unwrap_err();
        if words.is_empty() {
            // A length that is not one is construct-params' own words, which name the text.
            assert!(refused.contains("ahead"), "{refused}");
        } else {
            assert_eq!(refused, words, "{cmd}");
        }
        assert_eq!(stack.record("O-1").await.unwrap(), before, "{cmd}");
        assert!(stack.record("O-2").await.is_none(), "{cmd}");
    }
}

/// An opening in a floor is drawn inside its floor, hosted in its plan; its outline changed
/// (which is its size and place) and deleted; undo and redo; the floor's area and volume after the
/// cut are exact (48 m2 less 1 m2, by 200 mm), as the page reads them; each refusal names it:
/// outside, across an edge, over another opening, in what is not a floor.
#[tokio::test]
async fn an_opening_in_a_floor_is_drawn_and_cuts_its_area() {
    let stack = a_storey().await;
    assert_eq!(
        stack.quantities("F-1").await,
        ("48.00 m²".to_owned(), "9.60 m³".to_owned())
    );
    let draw = json!({ "element": "O-1", "host": "F-1", "outline": rect(1000, 1000, 2000, 2000) });
    stack.edit("opening.draw", draw).await.unwrap();
    let opening = stack.record("O-1").await.unwrap();
    assert_eq!(opening.kind, "opening");
    assert_eq!(opening.recipe.op, "opening.floor");
    assert_eq!(opening.props["@category"], "1600");
    assert!(!opening.props.contains_key("@host.along"));
    assert_eq!(opening.references.len(), 1);
    assert_eq!(opening.references[0].element.as_str(), "F-1");
    assert_eq!(opening.references[0].signature.as_deref(), Some("top"));
    assert_eq!(
        stack.quantities("F-1").await,
        ("47.00 m²".to_owned(), "9.40 m³".to_owned())
    );
    stack.builds("F-1").await.unwrap();
    stack.builds("O-1").await.unwrap();

    let hash = stack.record("O-1").await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    stack.edit("edit.redo", json!({})).await.unwrap();
    assert_eq!(stack.record("O-1").await.unwrap(), hash);

    // Its outline is its size and its place.
    let bigger = json!({ "element": "O-1", "outline": rect(1000, 1000, 3000, 2500) });
    stack.edit("opening.outline", bigger).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "45.00 m²");
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.record("O-1").await.unwrap(), hash);

    let refusals = [
        (
            "O-2",
            rect(7500, 1000, 8500, 2000),
            "opening O-2 crosses or touches the edge of floor F-1: it must lie wholly inside the floor, clear of its edge",
        ),
        (
            "O-2",
            rect(9000, 1000, 9500, 2000),
            "opening O-2 lies outside floor F-1: an opening is cut wholly inside the floor it is in",
        ),
        (
            "O-2",
            rect(1500, 1500, 2500, 2500),
            "opening O-1 and opening O-2 overlap or touch; move one clear of the other",
        ),
    ];
    for (id, outline, words) in refusals {
        let draw = json!({ "element": id, "host": "F-1", "outline": outline });
        assert_eq!(stack.edit("opening.draw", draw).await.unwrap_err(), words);
        assert!(stack.record(id).await.is_none());
    }
    let bad = json!({ "element": "O-2", "host": "W-1", "outline": rect(0, 0, 10, 10) });
    assert_eq!(
        stack.edit("opening.draw", bad).await.unwrap_err(),
        "W-1 is not a floor: an opening is drawn in a floor's plan."
    );
    let line = json!({ "element": "O-2", "host": "F-1",
                       "outline": [["1000", "1000"], ["2000", "1000"]] });
    assert!(stack.edit("opening.draw", line).await.is_err());
    let same = json!({ "element": "O-1", "outline": rect(1000, 1000, 2000, 2000) });
    assert_eq!(
        stack.edit("opening.outline", same).await.unwrap_err(),
        "O-1 already has that outline."
    );

    stack
        .edit("opening.delete", json!({ "element": "O-1" }))
        .await
        .unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "47.00 m²");
}

/// A shaft from Ground to Second, 1.8 m by 1 m, cuts the floor on First through every layer:
/// the floor's area drops by exactly that; changed in outline and reach; a reach that ends inside
/// the floor's thickness, or one over an opening, or beyond the floor's edge, is refused where it
/// is made, naming the shaft and the floor.
#[tokio::test]
async fn a_shaft_cuts_every_floor_in_its_reach() {
    let stack = a_storey().await;
    let make = json!({ "element": "S-1", "outline": rect(5000, 1000, 6800, 2000), "level": "L-g",
                       "top": "L-2" });
    stack.edit("shaft.create", make).await.unwrap();
    let shaft = stack.record("S-1").await.unwrap();
    assert_eq!(shaft.kind, "shaft");
    assert_eq!(shaft.props["@category"], "1600");
    assert_eq!(shaft.props["@level"], "L-g");
    assert_eq!(shaft.props["@top.level"], "L-2");
    assert_eq!(shaft.recipe.op, "shaft");
    assert_eq!(shaft.references.len(), 0);
    assert_eq!(
        stack.quantities("F-1").await,
        ("46.20 m²".to_owned(), "9.24 m³".to_owned())
    );
    stack.builds("F-1").await.unwrap();
    stack.builds("S-1").await.unwrap();
    // The page is told what cuts the floor: the shaft's outline, and the shaft as the builder gets it.
    let cut_by = stack.cut_by("F-1").await;
    assert_eq!(cut_by.cuts.len(), 1);
    assert_eq!(cut_by.cuts[0].key, "S-1");
    assert_eq!(cut_by.cuts[0].corners[0], [5000 * 6400, 1000 * 6400]);
    assert_eq!(cut_by.shafts.len(), 1);
    assert_eq!(cut_by.shafts[0].key, "S-1");
    assert_eq!(cut_by.shafts[0].recipe.args["height"], "length:q0:38400000");

    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    stack.edit("edit.redo", json!({})).await.unwrap();

    // Its outline and its reach.
    let smaller = json!({ "element": "S-1", "outline": rect(5000, 1000, 6000, 2000) });
    stack.edit("shaft.outline", smaller).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "47.00 m²");
    // Starting on First stops it inside the floor: First is the floor's top, so it misses.
    stack
        .edit("shaft.reach", json!({ "element": "S-1", "level": "L-1" }))
        .await
        .unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    stack
        .edit("shaft.reach", json!({ "element": "S-1", "level": "L-g" }))
        .await
        .unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "47.00 m²");

    let ends_inside = json!({ "element": "S-1", "offset": "2.9 m" });
    assert_eq!(
        stack.edit("shaft.reach", ends_inside).await.unwrap_err(),
        "shaft S-1: its base lies inside the thickness of floor F-1; a shaft passes wholly \
         through a floor or misses it, so move its base to the floor's top or bottom face or \
         past it"
    );
    let over = json!({ "element": "S-1", "outline": rect(-100, -100, 8100, 6100) });
    assert_eq!(
        stack.edit("shaft.outline", over).await.unwrap_err(),
        "shaft S-1 takes the whole of floor F-1: a shaft cuts a hole in a floor, not the floor away"
    );
    let top = json!({ "element": "S-1", "top": "L-g" });
    assert_eq!(
        stack.edit("shaft.reach", top).await.unwrap_err(),
        "S-1's top would be 0 mm from its base: a top is above its base."
    );
    let across = json!({ "element": "S-1", "outline": rect(7500, 1000, 8500, 2000) });
    assert_eq!(
        stack.edit("shaft.outline", across).await.unwrap_err(),
        "shaft S-1 crosses or touches the edge of floor F-1: it must lie wholly inside the floor, clear of its edge"
    );
    // An opening under a shaft is refused where it is drawn.
    let under = json!({ "element": "O-1", "host": "F-1", "outline": rect(5200, 1200, 5800, 1800) });
    assert_eq!(
        stack.edit("opening.draw", under).await.unwrap_err(),
        "opening O-1 and shaft S-1 overlap or touch; move one clear of the other"
    );
    let no_level =
        json!({ "element": "S-2", "outline": rect(1000, 1000, 2000, 2000), "top": "L-2" });
    assert_eq!(
        stack.edit("shaft.create", no_level).await.unwrap_err(),
        "A shaft starts on a level: this command needs a level."
    );
    let wrong = json!({ "element": "S-2", "outline": rect(1000, 1000, 2000, 2000),
                        "level": "L-nope", "top": "L-2" });
    assert!(stack.edit("shaft.create", wrong).await.is_err());
    assert!(stack.record("S-2").await.is_none());

    stack
        .edit("shaft.delete", json!({ "element": "S-1" }))
        .await
        .unwrap();
    assert!(stack.record("S-1").await.is_none());
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "47.00 m²");
}

/// A shaft beside a floor in plan passes it by, and a floor made after the shaft inside its reach is
/// cut by it: the shaft is hosted on nothing, and what it cuts is read each time.
#[tokio::test]
async fn a_floor_made_after_a_shaft_is_cut_by_it() {
    let stack = a_storey().await;
    let make = json!({ "element": "S-1", "outline": rect(9500, 1000, 10500, 2000), "level": "L-g",
                       "top": "L-2" });
    stack.edit("shaft.create", make).await.unwrap();
    assert_eq!(stack.quantities("F-1").await.0, "48.00 m²");
    assert_eq!(stack.cut_by("F-1").await.cuts.len(), 0);
    let boundary = json!({ "outer": [at("r0", 9000, 0), at("r1", 12000, 0), at("r2", 12000, 6000),
                                     at("r3", 9000, 6000)] });
    let floor = json!({ "element": "F-2", "type": CONCRETE, "level": "L-1",
                        "boundary": boundary });
    stack.edit("floor.create", floor).await.unwrap();
    assert_eq!(stack.quantities("F-2").await.0, "17.00 m²");
    assert_eq!(stack.cut_by("F-2").await.shafts.len(), 1);
    stack.builds("F-2").await.unwrap();
    stack.edit("edit.undo", json!({})).await.unwrap();
    assert!(stack.record("F-2").await.is_none());
}

/// The generic commands turn these kinds away naming the right command, and the commands of one
/// kind refuse another's element, naming its own: `element.create` of an opening recipe,
/// `element.set`, `element.move` and `element.delete` of an opening or a shaft, `hosted.place` and
/// `hosted.move`.
#[tokio::test]
async fn the_generic_commands_name_the_right_one() {
    let stack = a_storey().await;
    stack
        .edit(
            "opening.place",
            place("O-1", "1 m", "900 mm", "1200 mm", "1200 mm"),
        )
        .await
        .unwrap();
    let draw = json!({ "element": "O-2", "host": "F-1", "outline": rect(1000, 1000, 2000, 2000) });
    stack.edit("opening.draw", draw).await.unwrap();
    let make = json!({ "element": "S-1", "outline": rect(5000, 1000, 6800, 2000), "level": "L-g",
                       "top": "L-2" });
    stack.edit("shaft.create", make).await.unwrap();
    let recipe = stack.record("O-1").await.unwrap().recipe;
    let refused_kind = "An opening in a wall is made with opening.place, one in a floor with \
                opening.draw and a shaft with shaft.create.";
    for cmd in ["element.create", "hosted.place"] {
        let args = json!({ "element": "O-9", "kind": "opening", "name": "x", "level": 0,
                           "recipe": recipe, "host": "W-1", "face": "x" });
        assert_eq!(
            stack.edit(cmd, args).await.unwrap_err(),
            refused_kind,
            "{cmd}"
        );
    }
    let wall_words = "O-1 is an opening in a wall: change its size with opening.size, move it \
                      with opening.move, delete it with opening.delete.";
    let floor_words = "O-2 is an opening in a floor: draw it again with opening.outline, delete \
                       it with opening.delete.";
    let shaft_words = "S-1 is a shaft: change its outline with shaft.outline, where it starts \
                       and ends with shaft.reach, delete it with shaft.delete.";
    for (id, words) in [
        ("O-1", wall_words),
        ("O-2", floor_words),
        ("S-1", shaft_words),
    ] {
        for (cmd, args) in [
            ("element.delete", json!({ "element": id })),
            (
                "element.move",
                json!({ "element": id, "x": 1, "y": 1, "z": 0 }),
            ),
            ("element.set", json!({ "element": id, "name": "Renamed" })),
            ("hosted.move", json!({ "element": id, "along": "1 m" })),
        ] {
            assert_eq!(
                stack.edit(cmd, args).await.unwrap_err(),
                words,
                "{id} {cmd}"
            );
        }
    }
    // Each kind's own commands refuse another kind's element, naming that kind's.
    for (cmd, id, words) in [
        ("opening.delete", "S-1", shaft_words),
        ("shaft.delete", "O-2", floor_words),
        ("opening.size", "O-2", floor_words),
        ("opening.outline", "S-1", shaft_words),
        ("shaft.outline", "O-1", wall_words),
    ] {
        let args = json!({ "element": id, "width": "1 m", "outline": rect(0, 0, 10, 10) });
        assert_eq!(
            stack.edit(cmd, args).await.unwrap_err(),
            words,
            "{cmd} {id}"
        );
    }
    let wall = stack.record("W-1").await.unwrap();
    let not_one = stack
        .edit("opening.size", json!({ "element": "W-1", "width": "1 m" }))
        .await
        .unwrap_err();
    assert_eq!(
        not_one,
        format!("{} is not an opening or a shaft.", wall.name)
    );
    assert_eq!(
        stack
            .edit("opening.delete", json!({ "element": "nothing" }))
            .await
            .unwrap_err(),
        "That element no longer exists."
    );
}

/// A host deleted, or changed, under an opening behaves as under a door: a floor that holds an
/// opening is not deleted, a wall that holds one is not deleted, a level a shaft is tied to is not
/// deleted, and each goes when the opening is taken away. A wall shortened or lowered under an
/// opening is built as failed by the kernel's builder, as under a door (the server's kernel suite).
#[tokio::test]
async fn a_host_changed_under_an_opening_behaves_as_under_a_door() {
    let stack = a_storey().await;
    stack
        .edit(
            "opening.place",
            place("O-1", "2 m", "900 mm", "1200 mm", "1200 mm"),
        )
        .await
        .unwrap();
    let draw = json!({ "element": "O-2", "host": "F-1", "outline": rect(1000, 1000, 2000, 2000) });
    stack.edit("opening.draw", draw).await.unwrap();
    let make = json!({ "element": "S-1", "outline": rect(5000, 1000, 6800, 2000), "level": "L-g",
                       "top": "L-2" });
    stack.edit("shaft.create", make).await.unwrap();
    assert_eq!(
        stack
            .edit("element.delete", json!({ "element": "W-1" }))
            .await
            .unwrap_err(),
        "W-1 still hosts 1 element: delete them, or host them elsewhere, first."
    );
    assert_eq!(
        stack
            .edit("element.delete", json!({ "element": "F-1" }))
            .await
            .unwrap_err(),
        "F-1 still hosts 1 element: delete them, or host them elsewhere, first."
    );
    assert!(
        stack
            .edit("level.delete", json!({ "level": "L-2" }))
            .await
            .is_err()
    );
    // Taken away, the opening lets its wall go.
    stack
        .edit("opening.delete", json!({ "element": "O-1" }))
        .await
        .unwrap();
    stack
        .edit("element.delete", json!({ "element": "W-1" }))
        .await
        .unwrap();
    assert!(stack.record("W-1").await.is_none());
}

/// Every command is one step of the person's: undone, redone, and a second person's claim on an
/// element turns another's edit of it away in words.
#[tokio::test]
async fn another_persons_claim_turns_an_edit_away() {
    let stack = a_storey().await;
    stack
        .edit(
            "opening.place",
            place("O-1", "1 m", "900 mm", "1200 mm", "1200 mm"),
        )
        .await
        .unwrap();
    stack
        .edit("opening.size", json!({ "element": "O-1", "height": "1 m" }))
        .await
        .unwrap();
    let refused = stack
        .edit_as(
            "ben",
            "opening.size",
            json!({ "element": "O-1", "height": "1.1 m" }),
        )
        .await
        .unwrap_err();
    assert!(refused.ends_with("is working on this."), "{refused}");
    stack
        .edit_as("ben", "opening.delete", json!({ "element": "O-1" }))
        .await
        .unwrap_err();
    stack.later();
    stack
        .edit_as(
            "ben",
            "opening.size",
            json!({ "element": "O-1", "height": "1.1 m" }),
        )
        .await
        .unwrap();
    assert!(openings::is_command("shaft.reach"));
    assert!(!openings::is_command("floor.create"));
}

/// A floor with a slope or slab edges is not cut by openings and shafts yet (job 5.2), and both
/// ways round it is refused by name where it is made, never built wrongly: an opening or a shaft
/// that would cut a sloped floor or one with slab edges, and a slope or slab edges on a floor an
/// opening or a shaft cuts, each in words naming both. A shaft beside the floor in plan cuts
/// nothing and is made; so is an opening in a floor with a flat outline of its own (a sketched
/// one), and a slope taken away lets the opening be drawn.
#[tokio::test]
async fn a_floor_with_a_slope_or_slab_edges_is_not_cut_and_says_so() {
    let stack = a_storey().await;
    let fall = json!({ "element": "F-1", "slope": { "tail": ["0", "0"], "head": ["8000", "0"],
                       "at_tail": "0", "at_head": "-40 mm" } });
    stack.edit("floor.slope", fall).await.unwrap();
    let draw = json!({ "element": "O-1", "host": "F-1", "outline": rect(1000, 1000, 2000, 2000) });
    let refused = stack.edit("opening.draw", draw.clone()).await.unwrap_err();
    assert_eq!(
        refused,
        "opening O-1: it would be cut through floor F-1, which has a slope, and a floor with a \
         slope is not cut by openings and shafts yet: put it elsewhere, or take the slope off the \
         floor"
    );
    assert!(stack.record("O-1").await.is_none());
    let make = json!({ "element": "S-1", "outline": rect(5000, 1000, 6800, 2000), "level": "L-g",
                       "top": "L-2" });
    let refused = stack.edit("shaft.create", make).await.unwrap_err();
    assert_eq!(
        refused,
        "shaft S-1: it would be cut through floor F-1, which has a slope, and a floor with a \
         slope is not cut by openings and shafts yet: put it elsewhere, or take the slope off the \
         floor"
    );
    let beside = json!({ "element": "S-2", "outline": rect(20000, 1000, 21000, 2000),
                         "level": "L-g", "top": "L-2" });
    stack.edit("shaft.create", beside).await.unwrap();
    stack
        .edit("shaft.delete", json!({ "element": "S-2" }))
        .await
        .unwrap();

    // Slab edges.
    stack
        .edit("floor.slope.clear", json!({ "element": "F-1" }))
        .await
        .unwrap();
    let edge = json!({ "element": "F-1", "edge": "r0",
                       "profile": { "kind": "downturn", "width": "300 mm", "depth": "400 mm" } });
    stack.edit("floor.edge", edge.clone()).await.unwrap();
    let refused = stack.edit("opening.draw", draw.clone()).await.unwrap_err();
    assert_eq!(
        refused,
        "opening O-1: it would be cut through floor F-1, which has slab edges, and a floor with \
         slab edges is not cut by openings and shafts yet: put it elsewhere, or take the slab \
         edges off the floor"
    );
    assert!(stack.record("O-1").await.is_none());

    // The other way round: the opening first, then the slope or the edges.
    stack
        .edit(
            "floor.edge.remove",
            json!({ "element": "F-1", "edge": "r0" }),
        )
        .await
        .unwrap();
    stack.edit("opening.draw", draw).await.unwrap();
    let before = stack.record("F-1").await.unwrap();
    let fall = json!({ "element": "F-1", "slope": { "tail": ["0", "0"], "head": ["8000", "0"],
                       "at_tail": "0", "at_head": "-40 mm" } });
    let refused = stack.edit("floor.slope", fall).await.unwrap_err();
    assert_eq!(
        refused,
        "F-1's slope: O-1 cuts through this floor, and a floor with a slope is not cut by \
         openings and shafts yet: take it away first"
    );
    let refused = stack.edit("floor.edge", edge).await.unwrap_err();
    assert_eq!(
        refused,
        "F-1's slab edge: O-1 cuts through this floor, and a floor with slab edges is not cut by \
         openings and shafts yet: take it away first"
    );
    assert_eq!(stack.record("F-1").await.unwrap(), before);
    // The floor is as it was: cut, 47 m2 by 200 mm.
    assert_eq!(
        stack.quantities("F-1").await,
        ("47.00 m²".to_owned(), "9.40 m³".to_owned())
    );
}
