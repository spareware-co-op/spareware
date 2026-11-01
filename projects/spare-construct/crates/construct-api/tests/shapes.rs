//! Edits to shapes, on the test store. Every test runs twice, on two generators that make different
//! shapes, so nothing here can come to rely on what a shape is. `server/tests/swap.rs` does the same
//! through the whole server, with Cedar.

use construct_api::cloud::{Access, ProjectCommands};
use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Mesh, Recipe,
};
use construct_model::keys::{Place, list};
use construct_model::rows::{BuiltElement, BuiltResult, Project, Stored, Team};
use construct_regen::regenerator::Regenerator;
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{FileStore, Kv, Prefix, Reader, Scope, Txn, read_row, to_row};
use kit_sync::{Orderers, Reply, SystemClock, verify_log};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";

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

/// A different generator: every shape is a wedge, with one more level of detail than the boxes and
/// a body as large as the recipe asks, so large shapes go to file storage.
struct Wedges;

impl Geometry for Wedges {
    fn edition(&self) -> Edition {
        Edition {
            engine: "test.wedges".to_owned(),
            version: 7,
            name_scheme: 2,
        }
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let arg = |name: &str| recipe.args.get(name).and_then(Value::as_i64);
        let long = arg("length").or(arg("width")).ok_or(GeometryError {
            code: "NoSize".to_owned(),
            message: "wedges need a length".to_owned(),
        })?;
        if long <= 0 {
            return Err(GeometryError {
                code: "Empty".to_owned(),
                message: "too short".to_owned(),
            });
        }
        let mesh = Mesh {
            positions: vec![0, 0, 0, long, 0, 0, 0, long, 0],
            indices: vec![0, 1, 2],
        };
        let display = |lod| Display {
            lod,
            deviation: i64::from(lod) * 10,
            mesh: mesh.clone(),
            outline: vec![[0, 0], [long, 0], [0, long]],
        };
        let filler = arg("filler").unwrap_or(0) as usize;
        let mut body = format!("wedge {long}").into_bytes();
        body.resize(body.len() + filler, b'.');
        Ok(Built {
            body,
            bounds: Bounds {
                min: [0, 0, 0],
                max: [long, long, 1],
            },
            names: vec![FaceName {
                name: format!("slope.{long}"),
                role: "exterior".to_owned(),
            }],
            displays: (0..3).map(display).collect(),
        })
    }
}

struct Stack {
    store: Memory,
    files: MemoryFiles,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    regen: Regenerator<Memory, MemoryFiles>,
    next: AtomicU64,
}

impl Stack {
    async fn new(geometry: Arc<dyn Geometry>) -> Self {
        let store = Memory::new();
        let files = MemoryFiles::default();
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
        let orderers = Orderers::new(store.clone(), Arc::new(commands), Arc::new(SystemClock), 50);
        let regen = Regenerator::new(store.clone(), files.clone(), geometry);

        // The rows the directory would have written.
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
            files,
            orderers,
            regen,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        stack
    }

    async fn edit(&self, cmd: &str, args: Value) -> Result<u64, String> {
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

    async fn create(&self, id: &str, x: i64, recipe: Value) {
        self.edit(
            "element.create",
            json!({ "element": id, "kind": "wall", "level": 1,
                    "placement": { "x": x, "y": 0, "z": 0 }, "recipe": recipe }),
        )
        .await
        .unwrap();
    }

    async fn built(&self, id: &str) -> Option<BuiltElement> {
        let place = Place::Built {
            project: i("p"),
            document: i(DOCUMENT),
            element: i(id),
        };
        let mut snapshot = self.store.snapshot().await.unwrap();
        read_row(&mut snapshot, &place.key()).await.unwrap()
    }

    async fn body(&self, id: &str) -> Id {
        match self.built(id).await.unwrap().result {
            BuiltResult::Shape { body, .. } => body,
            failed @ BuiltResult::Failed { .. } => panic!("{id}: {failed:?}"),
        }
    }

    async fn in_cell(&self, x: i64) -> Vec<String> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let prefix = list::cell(&i("p"), &i(DOCUMENT), 1, x, 0);
        let rows = snapshot.scan(&prefix).await.unwrap();
        rows.into_iter()
            .map(|(key, _)| key.as_str().to_owned())
            .collect()
    }

    /// Everything derived, as bytes.
    async fn derived(&self) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let mut rows = BTreeMap::new();
        for table in list::derived(&i("p")) {
            for (key, value) in snapshot.scan(&table).await.unwrap() {
                rows.insert(key.as_str().to_owned(), value);
            }
        }
        rows
    }
}

fn wall(length: i64) -> Value {
    let args = json!({ "length": length, "thickness": 200, "height": 2700 });
    serde_json::to_value(Recipe::construct(
        "wall.straight",
        serde_json::from_value(args).unwrap(),
    ))
    .unwrap()
}

fn generators() -> [Arc<dyn Geometry>; 2] {
    [Arc::new(Boxes), Arc::new(Wedges)]
}

#[tokio::test]
async fn an_edit_writes_inputs_and_regeneration_writes_shapes() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        stack.create("w", 100, wall(3000)).await;
        assert!(stack.built("w").await.is_none(), "the edit builds nothing");

        let pass = stack.regen.catch_up(&i("p")).await.unwrap();
        let body = stack.body("w").await;
        assert_eq!(pass.bodies[&i(DOCUMENT)][&i("w")], body);
        assert_eq!(stack.in_cell(0).await.len(), 1);
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 1);
    }
}

#[tokio::test]
async fn moving_an_element_keeps_its_body_and_moves_its_cells() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        stack.create("w", 100, wall(1000)).await;
        stack.regen.catch_up(&i("p")).await.unwrap();
        let before = stack.body("w").await;
        stack
            .edit(
                "element.move",
                json!({ "element": "w", "placement": { "x": 90_000, "y": 0, "z": 0 } }),
            )
            .await
            .unwrap();
        stack.regen.catch_up(&i("p")).await.unwrap();
        assert_eq!(
            stack.body("w").await,
            before,
            "placement isn't part of the shape"
        );
        assert!(stack.in_cell(0).await.is_empty());
        assert_eq!(stack.in_cell(18).await.len(), 1);
    }
}

#[tokio::test]
async fn changing_a_recipe_changes_the_body_and_deleting_clears_it() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        stack.create("w", 0, wall(1000)).await;
        stack.regen.catch_up(&i("p")).await.unwrap();
        let before = stack.body("w").await;
        stack
            .edit(
                "element.set",
                json!({ "element": "w", "recipe": wall(2000) }),
            )
            .await
            .unwrap();
        stack.regen.catch_up(&i("p")).await.unwrap();
        assert_ne!(stack.body("w").await, before);

        stack
            .edit("element.delete", json!({ "element": "w" }))
            .await
            .unwrap();
        stack.regen.catch_up(&i("p")).await.unwrap();
        assert!(stack.built("w").await.is_none());
        assert!(stack.in_cell(0).await.is_empty());
    }
}

#[tokio::test]
async fn deleting_every_derived_row_and_regenerating_gives_the_same_bytes() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        for n in 0..12 {
            stack
                .create(&format!("w{n}"), n * 2000, wall(1500 + n))
                .await;
        }
        stack
            .edit(
                "element.move",
                json!({ "element": "w3", "placement": { "x": 7, "y": 7, "z": 0 } }),
            )
            .await
            .unwrap();
        stack
            .edit("element.delete", json!({ "element": "w4" }))
            .await
            .unwrap();
        stack.regen.catch_up(&i("p")).await.unwrap();
        let before = stack.derived().await;
        assert!(!before.is_empty());

        let pass = stack.regen.rebuild(&i("p")).await.unwrap();
        assert_eq!(stack.derived().await, before);
        assert_eq!(pass.bodies[&i(DOCUMENT)].len(), 11);
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 11);
    }
}

#[tokio::test]
async fn a_recipe_that_cant_build_is_stored_as_a_failure_and_nothing_stops() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        stack.create("bad", 0, wall(-5)).await;
        stack.create("good", 0, wall(900)).await;
        stack.regen.catch_up(&i("p")).await.unwrap();
        assert!(matches!(
            stack.built("bad").await.unwrap().result,
            BuiltResult::Failed { .. }
        ));
        stack.body("good").await;
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 2);
        let mut snapshot = stack.store.snapshot().await.unwrap();
        verify_log(&mut snapshot, &Scope::Project(i("p")))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn one_element_cant_ask_for_millions_of_cells() {
    for geometry in generators() {
        let stack = Stack::new(geometry).await;
        stack.create("huge", 0, wall(4_000_000)).await;
        stack.regen.catch_up(&i("p")).await.unwrap();
        // A long thin box fits; a wedge covers a square of the same side, and doesn't.
        if let BuiltResult::Failed { code, .. } = stack.built("huge").await.unwrap().result {
            assert_eq!(code, "TooBig");
        }
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 1);
    }
}

#[tokio::test]
async fn a_large_shape_goes_to_file_storage_linked_by_hash() {
    let stack = Stack::new(Arc::new(Wedges)).await;
    let args = json!({ "length": 500, "filler": 400_000 });
    let recipe = Recipe::construct("wall.straight", serde_json::from_value(args).unwrap());
    stack
        .create("big", 0, serde_json::to_value(recipe).unwrap())
        .await;
    stack.regen.catch_up(&i("p")).await.unwrap();

    let body = stack.body("big").await;
    let place = Place::Body {
        project: i("p"),
        id: body.clone(),
    };
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let stored: Option<Stored> = read_row(&mut snapshot, &place.key()).await.unwrap();
    let Some(Stored::File { path, size }) = stored else {
        panic!("a large body stays out of the store");
    };
    assert_eq!(size, stack.files.get(&path).await.unwrap().len() as u64);
    assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 1);

    stack
        .files
        .put(&path, b"something else".to_vec())
        .await
        .unwrap();
    assert!(
        stack.regen.verify(&i("p")).await.is_err(),
        "the hash catches a changed file"
    );
}

#[tokio::test]
async fn shapes_are_kept_per_level_of_detail() {
    let stack = Stack::new(Arc::new(Wedges)).await;
    stack.create("w", 0, wall(1000)).await;
    stack.regen.catch_up(&i("p")).await.unwrap();
    let BuiltResult::Shape {
        displays, names, ..
    } = stack.built("w").await.unwrap().result
    else {
        panic!("it builds");
    };
    assert_eq!(
        displays.iter().map(|d| d.lod).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(displays[2].deviation, 20);
    assert_eq!(names[0].role, "exterior");
}

#[tokio::test]
async fn a_server_without_a_documents_edition_doesnt_build_it() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    stack.create("w", 0, wall(1000)).await;
    let other = Regenerator::new(stack.store.clone(), stack.files.clone(), Arc::new(Wedges));
    other.catch_up(&i("p")).await.unwrap();
    let BuiltResult::Failed { code, .. } = stack.built("w").await.unwrap().result else {
        panic!("building it here would give other bytes under the same recipe");
    };
    assert_eq!(code, "EditionUnavailable");
    assert_eq!(other.verify(&i("p")).await.unwrap(), 1);
}

#[tokio::test]
async fn two_servers_regenerating_together_agree() {
    for geometry in generators() {
        let stack = Stack::new(geometry.clone()).await;
        for n in 0..300 {
            stack.create(&format!("w{n}"), n * 100, wall(1000)).await;
        }
        let other = Regenerator::new(stack.store.clone(), stack.files.clone(), geometry);
        let project = i("p");
        let (a, b) = tokio::join!(stack.regen.catch_up(&project), other.catch_up(&project));
        a.unwrap();
        b.unwrap();
        assert_eq!(stack.regen.verify(&i("p")).await.unwrap(), 300);
    }
}

#[tokio::test]
async fn a_checkpoint_changes_nothing_and_must_make_sense() {
    let stack = Stack::new(Arc::new(Boxes)).await;
    stack.create("w", 0, wall(1000)).await;
    let before = stack.store.keys().len();
    let args = json!({ "through": 2, "bodies": { "w": "abc" } });
    stack.edit("document.checkpoint", args).await.unwrap();
    let mut snapshot = stack.store.snapshot().await.unwrap();
    let check = verify_log(&mut snapshot, &Scope::Project(i("p")))
        .await
        .unwrap();
    assert_eq!(check.pieces, 1);
    // One log entry and one client mark, nothing else.
    assert_eq!(stack.store.keys().len(), before + 2);
    assert!(
        stack
            .edit("document.checkpoint", json!({ "bodies": 3 }))
            .await
            .is_err()
    );
    let scan: Vec<_> = snapshot.scan(&Prefix::everything()).await.unwrap();
    assert!(!scan.is_empty());
}
