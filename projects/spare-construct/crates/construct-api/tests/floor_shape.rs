//! A floor's top face shape-edited through the edit log, on the test store (job 14.15): a waste
//! lowered 20 mm, built 20 mm deeper; the screed made the type's variable layer, the bottom flat
//! again and the screed's volume less the waste's pyramid, worked by hand; a point too deep for the
//! variable layer refused; a split line and its height making a valley; a wall picked as a
//! support; a point removed with its lines; a reset back to the plain floor's bytes; a slope and
//! shape edits kept apart in words; and undo and redo of every shape command.

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
use construct_materials::Materials;
use construct_model::element::Element;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{BuiltElement, BuiltResult, Project, Team};
use construct_params::Volume;
use construct_regen::parameters::{self as records, Context};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use construct_slabs::{Point, default_floors, layers_of, shape_layer_quantities, shape_of};
use construct_types::ElementType;
use construct_walls::default_walls;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
const CLAIM_MS: u64 = 30_000;
const CONCRETE: &str = "floor-concrete-200";
const SCREED: &str = "floor-concrete-150-screed-50";
/// Cubic steps in a cubic millimetre.
const MM3: i128 = 6400 * 6400 * 6400;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

struct Yes;

impl Decider for Yes {
    fn decide(&self, _: &Question) -> Result<bool, String> {
        Ok(true)
    }
}

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
    /// A store with a project, a team and a model, Ground at 0 and First at 3 m, and the default
    /// floor types.
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
        for (id, name, elevation) in [("L-g", "Ground", "0"), ("L-1", "First", "3 m")] {
            let args = json!({ "level": id, "name": name, "elevation": elevation });
            stack.edit("level.create", args).await.unwrap();
        }
        stack.edit("floor.defaults", json!({})).await.unwrap();
        stack
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        self.clock.0.fetch_add(10, Ordering::SeqCst);
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

    async fn record(&self, id: &str) -> Element {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_element(&mut snapshot, &i("p"), &i(DOCUMENT), &i(id))
            .await
            .unwrap()
            .map(|(_, record)| record)
            .unwrap()
    }

    async fn context(&self) -> Context {
        let mut snapshot = self.store.snapshot().await.unwrap();
        records::read_context(&mut snapshot, &i("p"), &i(DOCUMENT))
            .await
            .unwrap()
            .unwrap()
    }

    async fn floor_type(&self, id: &str) -> ElementType {
        let context = self.context().await;
        let record = self.record(id).await;
        ElementType::from_record(&record, &context.params, &context.categories).unwrap()
    }

    /// An element's built bounds in its own frame, whole millimetres, or its failure in words.
    async fn bounds(&self, id: &str) -> Result<([i64; 3], [i64; 3]), String> {
        self.regen.catch_up(&i("p")).await.unwrap();
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        let built: BuiltElement = read_row(&mut snapshot, &place.key())
            .await
            .unwrap()
            .unwrap();
        match built.result {
            BuiltResult::Shape { bounds, .. } => Ok((bounds.min, bounds.max)),
            BuiltResult::Failed { code, message } => Err(format!("{code}: {message}")),
        }
    }

    async fn pieces(&self) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = Scope::Project(i("p")).table_prefix("piece", &[]);
        snapshot
            .scan(&prefix)
            .await
            .unwrap()
            .into_iter()
            .map(|(key, value)| (key.as_str().to_owned(), value))
            .collect()
    }

    async fn state(&self) -> State {
        let mut snapshot = self.store.snapshot().await.unwrap();
        undo::state(&mut snapshot, &i("p"), &i(DOCUMENT), &i("anna"))
            .await
            .unwrap()
    }

    /// Runs a command, undoes it and redoes it: the undo puts every piece back as it was before
    /// the command, and the redo as it was after.
    async fn undone_and_redone(&self, cmd: &str, args: Value) {
        let before = self.pieces().await;
        self.edit(cmd, args)
            .await
            .unwrap_or_else(|why| panic!("{cmd}: {why}"));
        let after = self.pieces().await;
        assert_ne!(before, after, "{cmd} changed something");
        assert_eq!(
            self.state().await.undo.map(|step| step.cmd),
            Some(cmd.to_owned())
        );
        self.edit("edit.undo", json!({})).await.unwrap();
        assert_eq!(self.pieces().await, before, "{cmd} undone");
        self.edit("edit.redo", json!({})).await.unwrap();
        assert_eq!(self.pieces().await, after, "{cmd} redone");
    }
}

/// A default floor type's layers as the commands take them, with layer `variable` marked.
fn layers(default: &str, variable: Option<usize>) -> Value {
    let floors = default_floors().unwrap();
    let floor = floors.iter().find(|floor| floor.id == default).unwrap();
    let layers: Vec<Value> = floor
        .layers
        .layers()
        .iter()
        .enumerate()
        .map(|(at, layer)| {
            let mut sent = json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                    "material": layer.material.as_str() });
            if variable == Some(at) {
                sent["variable"] = json!(true);
            }
            sent
        })
        .collect();
    Value::Array(layers)
}

/// A rectangle from (0, 0) to `x` by `y` mm, its edges `r0` to `r3`.
fn rectangle(x: i64, y: i64) -> Value {
    let at = |id: &str, x: i64, y: i64| json!({ "id": id, "at": [x.to_string(), y.to_string()] });
    json!({ "outer": [at("r0", 0, 0), at("r1", x, 0), at("r2", x, y), at("r3", 0, y)] })
}

fn create(id: &str, ty: &str, boundary: &Value) -> Value {
    json!({ "element": id, "type": ty, "level": "L-1", "boundary": boundary })
}

/// The floor's layers' volumes, worked out from its record and its type.
async fn volumes(stack: &Stack, floor: &str, ty: &str) -> Vec<Volume> {
    let shape = shape_of(&stack.record(floor).await.recipe).unwrap();
    let layers = layers_of(&stack.floor_type(ty).await).unwrap();
    shape_layer_quantities(&shape, &layers, &Materials::default_metric())
        .unwrap()
        .layers
        .into_iter()
        .map(|q| q.volume)
        .collect()
}

/// A waste 20 mm down in the middle of a 4 m by 3 m floor of 50 mm screed over 150 mm concrete:
/// built 220 mm deep under it (every layer follows the top). With the screed the type's variable
/// layer, the bottom is flat again, 200 mm down, and the screed is 12 m2 x 50 mm less a third of
/// 12 m2 x 20 mm: 0.52 m3; the concrete 1.8 m3 either way. A waste 60 mm down would leave the 50 mm
/// screed no thickness: refused in words, nothing changed. A point off the floor is refused.
#[tokio::test]
async fn a_waste_and_a_variable_screed() {
    let stack = Stack::new().await;
    stack
        .edit(
            "floor.create",
            create("F-1", SCREED, &rectangle(4000, 3000)),
        )
        .await
        .unwrap();
    let plain = stack.record("F-1").await.recipe;
    let waste =
        json!({ "element": "F-1", "point": "w", "at": ["2 m", "1500"], "height": "-20 mm" });
    stack.undone_and_redone("floor.shape.point", waste).await;
    assert_eq!(stack.record("F-1").await.recipe.op_version, 3);
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -220], [4000, 3000, 0]))
    );
    let mm3 = |n: i128| Volume::from_cubic_steps(n * MM3);
    assert_eq!(
        volumes(&stack, "F-1", SCREED).await,
        [mm3(600_000_000), mm3(1_800_000_000)]
    );

    let variable = json!({ "type": SCREED, "layers": layers(SCREED, Some(0)) });
    stack.undone_and_redone("floor.type.layers", variable).await;
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -200], [4000, 3000, 0]))
    );
    assert_eq!(
        volumes(&stack, "F-1", SCREED).await,
        [mm3(520_000_000), mm3(1_800_000_000)]
    );

    let deep = json!({ "element": "F-1", "point": "w", "height": "-60 mm" });
    let before = stack.pieces().await;
    let refused = stack.edit("floor.shape.height", deep).await.unwrap_err();
    assert_eq!(
        refused,
        "F-1: point w is lowered so far that layer 0, the variable one, would be no thickness there: raise it, or give the type a thicker variable layer"
    );
    assert_eq!(stack.pieces().await, before);
    let off = json!({ "element": "F-1", "at": ["5 m", "1 m"], "height": "0" });
    assert_eq!(
        stack.edit("floor.shape.point", off).await.unwrap_err(),
        "F-1's shape: point p0 is not on the floor: it is outside it or in a hole"
    );

    // Reset: the plain floor's recipe, byte for byte.
    stack
        .undone_and_redone("floor.shape.reset", json!({ "element": "F-1" }))
        .await;
    assert_eq!(stack.record("F-1").await.recipe, plain);
    assert_eq!(
        stack.bounds("F-1").await,
        Ok(([0, 0, -200], [4000, 3000, 0]))
    );

    // A roof type has no variable layer; a floor type has at most one.
    let two = json!({ "type": SCREED, "layers": [
        { "function": "finish", "thickness": "50", "material": "screed-cement", "variable": true },
        { "function": "structure", "thickness": "150", "material": "concrete-regular", "variable": true },
    ] });
    assert_eq!(
        stack.edit("floor.type.layers", two).await.unwrap_err(),
        "A floor type has at most one variable layer."
    );
}

/// A split line across a 4 m square of 200 mm concrete, from one corner to the other: its ends are
/// new points at the top's height there (0), so nothing moves until its height is set; set to
/// -20 mm, the top folds into a valley along it and the body is 220 mm deep. A point removed takes
/// its lines with it. A wall standing across the floor, picked as a support, is a split line where
/// its location line crosses the floor. A slope on a shape-edited floor, and a shape on a sloped
/// one, are refused in words: one or the other.
#[tokio::test]
async fn split_lines_supports_and_slopes() {
    let stack = Stack::new().await;
    stack
        .edit(
            "floor.create",
            create("F-2", CONCRETE, &rectangle(4000, 4000)),
        )
        .await
        .unwrap();
    let valley = json!({ "element": "F-2", "from": ["0", "0"], "to": ["4 m", "4 m"] });
    stack.undone_and_redone("floor.shape.split", valley).await;
    let record = stack.record("F-2").await;
    assert_eq!(
        serde_json::to_string(&record.recipe.args["top"]).unwrap(),
        r#"{"points":[{"at":[0,0],"height":"length:q0:0","id":"p0"},{"at":[25600000,25600000],"height":"length:q0:0","id":"p1"}],"splits":[{"from":"p0","id":"s0","to":"p1"}]}"#
    );
    assert_eq!(
        stack.bounds("F-2").await,
        Ok(([0, 0, -200], [4000, 4000, 0]))
    );
    let low = json!({ "element": "F-2", "split": "s0", "height": "-20 mm" });
    stack.undone_and_redone("floor.shape.height", low).await;
    assert_eq!(
        stack.bounds("F-2").await,
        Ok(([0, 0, -220], [4000, 4000, 0]))
    );
    let shape = shape_of(&stack.record("F-2").await.recipe).unwrap();
    // On the face through (0, 0, -20), (4, 0, 0) and (4, 4, -20): -20 + (3 - 0.5) x 5 = -7.5 mm.
    let (num, den) = shape
        .top()
        .unwrap()
        .height_at(Point::mm(3000, 500))
        .unwrap();
    assert_eq!(num, -48_000 * den);

    let remove = json!({ "element": "F-2", "point": "p1" });
    stack.undone_and_redone("floor.shape.remove", remove).await;
    let shape = shape_of(&stack.record("F-2").await.recipe).unwrap();
    let edits = shape.edits().unwrap();
    assert_eq!((edits.points().len(), edits.splits().len()), (1, 0));

    // A wall across the floor, from (2 m, -1 m) to (2 m, 5 m): the support runs from (2 m, 0) to
    // (2 m, 4 m), its ends new points at the top's height there.
    let walls = default_walls().unwrap();
    let wall = &walls[0];
    let assembly = {
        let layers: Vec<Value> = wall
            .assembly
            .layers()
            .iter()
            .map(|layer| {
                json!({ "function": layer.function, "thickness": layer.thickness.to_string(),
                        "material": layer.material.as_str(), "wraps": layer.wraps })
            })
            .collect();
        json!({
            "core": {
                "exterior": wall.assembly.boundary(construct_walls::Side::Exterior),
                "interior": wall.assembly.boundary(construct_walls::Side::Interior),
            },
            "layers": layers,
        })
    };
    stack
        .edit(
            "wall.type.create",
            json!({ "type": wall.id, "name": wall.name, "assembly": assembly }),
        )
        .await
        .unwrap();
    let draw = json!({ "element": "W-1", "type": wall.id, "start": ["2 m", "-1 m"],
                       "end": ["2 m", "5 m"], "level": "L-g", "top": "L-1" });
    stack.edit("wall.draw", draw).await.unwrap();
    let support = json!({ "element": "F-2", "wall": "W-1" });
    stack
        .undone_and_redone("floor.shape.support", support)
        .await;
    let shape = shape_of(&stack.record("F-2").await.recipe).unwrap();
    let edits = shape.edits().unwrap();
    let held = edits.splits().last().unwrap();
    let at = |id: &Id| edits.point(id).unwrap().at;
    assert_eq!(
        (at(&held.from), at(&held.to)),
        (Point::mm(2000, 0), Point::mm(2000, 4000))
    );
    let not_a_wall = json!({ "element": "F-2", "wall": "F-2" });
    assert!(
        stack
            .edit("floor.shape.support", not_a_wall)
            .await
            .unwrap_err()
            .contains("only a wall is picked as a support (beams are not in the model yet)")
    );

    // A slope on it, and a shape on a sloped floor: one or the other.
    let arrow = json!({ "element": "F-2", "slope": { "tail": ["0", "0"], "head": ["4 m", "0"],
                        "at_tail": "0", "at_head": "-40 mm" } });
    assert_eq!(
        stack.edit("floor.slope", arrow.clone()).await.unwrap_err(),
        "F-2's slope: a floor either slopes by an arrow or has its shape edited, not both: clear the slope, or reset the shape"
    );
    stack
        .edit(
            "floor.create",
            create("F-3", CONCRETE, &rectangle(4000, 3000)),
        )
        .await
        .unwrap();
    let mut sloped = arrow;
    sloped["element"] = json!("F-3");
    stack.edit("floor.slope", sloped).await.unwrap();
    let point = json!({ "element": "F-3", "at": ["1 m", "1 m"], "height": "-5 mm" });
    assert_eq!(
        stack.edit("floor.shape.point", point).await.unwrap_err(),
        "F-3's shape: a floor either slopes by an arrow or has its shape edited, not both: clear the slope, or reset the shape"
    );
}
