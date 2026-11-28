//! The house, exported to IFC 4.3 (job 12.1, CON-A18-F1-P2, first half): the house script
//! (`house/house.json`, built through the edit log as `house.rs` builds it) is read out by
//! `cloud::ifc` and written by construct-interop, and the file is checked three ways: counts and
//! `GlobalId`s against what the script makes, numbers worked out by hand from the script's lengths,
//! and the whole text against the golden file `house/house.ifc` (header stamp fixed), so a change
//! to any byte of the export is seen and read.
//!
//! The golden file is the house as the writer gives it today, read through by hand once for the
//! numbers below and opened in another tool (the pull request says which). When a later job adds
//! to the house script or to the writer, regenerate it with
//! `WRITE_GOLDEN=1 cargo test -p construct-api --test house_ifc`, read the diff, and keep it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_api::cloud::{Access, ProjectCommands, ifc};
use construct_interop::ifc_spatial;
use construct_model::geometry::Geometry;
use construct_model::keys::Place;
use construct_model::rows::{Project, Team};
use construct_regen::stand_in::Boxes;
use kit_core::Id;
use kit_ops::{Decider, Edit, Question};
use kit_store::memory::Memory;
use kit_store::{Kv, Scope, Txn, to_row};
use kit_sync::{Clock, Orderers, Reply};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const DOCUMENT: &str = "doc";
const SCRIPT: &str = include_str!("house/house.json");
const GOLDEN: &str = "tests/house/house.ifc";
const STAMP: &str = "2026-01-01T00:00:00";

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

/// The store and orderer the script runs through, as `house.rs` makes them.
struct Stack {
    store: Memory,
    orderers: Orderers<Memory, ProjectCommands<Memory, Yes>>,
    clock: Arc<Fixed>,
    next: AtomicU64,
}

impl Stack {
    async fn built() -> Self {
        let store = Memory::new();
        let geometry: Arc<dyn Geometry> = Arc::new(Boxes);
        let access = Arc::new(Access::new(store.clone(), Yes));
        let commands = ProjectCommands::new(store.clone(), access, 30_000, geometry.edition());
        let clock = Arc::new(Fixed(AtomicU64::new(1_000_000)));
        let orderers = Orderers::new(store.clone(), Arc::new(commands), clock.clone(), 50);
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
            clock,
            next: AtomicU64::new(0),
        };
        let args = json!({ "team": "t", "name": "Doc", "package": "Doc package" });
        stack.edit("model.add", args).await.unwrap();
        let script: Value = serde_json::from_str(SCRIPT).unwrap();
        for step in script["steps"].as_array().unwrap() {
            let (cmd, args) = (step["cmd"].as_str().unwrap(), step["args"].clone());
            if let Err(why) = stack.edit(cmd, args).await {
                panic!("{cmd} ({}) was refused: {why}", step["note"]);
            }
        }
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

    /// The house as the writer gives it.
    async fn export(&self) -> (ifc_spatial::Export, String) {
        let mut snapshot = self.store.snapshot().await.unwrap();
        let export = ifc::read_export(&mut snapshot, &i("p"), &i(DOCUMENT), "House", STAMP)
            .await
            .unwrap()
            .unwrap();
        let text = ifc_spatial::write(&export).unwrap();
        (export, text)
    }
}

/// The text of every instance of `entity`, in file order.
fn instances<'a>(text: &'a str, entity: &str) -> Vec<&'a str> {
    let head = format!("={entity}(");
    text.lines().filter(|line| line.contains(&head)).collect()
}

/// The first quoted string of an instance: its `GlobalId`, for the entities that have one.
fn global_id(line: &str) -> &str {
    let start = line.find('\'').unwrap() + 1;
    &line[start..start + 22]
}

#[tokio::test]
async fn the_house_is_written_with_its_storeys_walls_and_slab() {
    let stack = Stack::built().await;
    let (export, text) = stack.export().await;

    // By hand from the script: two levels (Ground at 0, First at 3 m), five walls (four outer, one
    // inner), one floor. The levels are storeys; the grids, doors, windows, opening, roof and room
    // are not this job's.
    assert_eq!(export.storeys.len(), 2);
    assert_eq!(export.walls.len(), 5);
    assert_eq!(export.slabs.len(), 1);
    assert_eq!(instances(&text, "IFCPROJECT").len(), 1);
    assert_eq!(instances(&text, "IFCSITE").len(), 1);
    assert_eq!(instances(&text, "IFCBUILDING").len(), 1);
    assert_eq!(instances(&text, "IFCBUILDINGSTOREY").len(), 2);
    assert_eq!(instances(&text, "IFCWALL").len(), 5);
    assert_eq!(instances(&text, "IFCSLAB").len(), 1);

    // Elevations in millimetres, exact: 0 and 3000.
    let storeys = instances(&text, "IFCBUILDINGSTOREY");
    assert!(storeys[0].contains("'Ground'") && storeys[0].ends_with(",.ELEMENT.,0.);"));
    assert!(storeys[1].contains("'First'") && storeys[1].ends_with(",.ELEMENT.,3000.);"));

    // Each wall is one extrusion 3000 high (Ground to First); the floor is one extrusion 200 thick
    // (200 mm of concrete). Six extrusions in all.
    let extrusions = instances(&text, "IFCEXTRUDEDAREASOLID");
    assert_eq!(extrusions.len(), 6);
    assert_eq!(
        extrusions
            .iter()
            .filter(|e| e.ends_with(",3000.);"))
            .count(),
        5
    );
    assert_eq!(
        extrusions.iter().filter(|e| e.ends_with(",200.);")).count(),
        1
    );
    // The floor's own boundary has no voids here (its opening is job 12.2's), so six plain
    // profiles; two wall types (outer five layers, inner three) and the floor's type make three
    // layer sets, and each of the six elements has a usage of one.
    assert_eq!(instances(&text, "IFCARBITRARYCLOSEDPROFILEDEF").len(), 6);
    assert_eq!(instances(&text, "IFCMATERIALLAYERSET").len(), 3);
    assert_eq!(instances(&text, "IFCMATERIALLAYERSETUSAGE").len(), 6);
    // 110 + 40 + 0 + 90 + 10 for the outer walls, and the inner wall's 12.5 mm linings, exact.
    assert!(text.contains(",110.,.F.,$,$,'finish',$);"));
    assert!(text.contains(",12.5,.F.,$,$,'finish',$);"));
    assert!(text.contains(",0.,.F.,$,$,'membrane',$);"));

    // Every wall and the slab stands in the Ground storey: one containment.
    assert_eq!(
        instances(&text, "IFCRELCONTAINEDINSPATIALSTRUCTURE").len(),
        1
    );
}

#[tokio::test]
async fn global_ids_are_distinct_and_the_same_on_every_export() {
    let stack = Stack::built().await;
    let (_, first) = stack.export().await;
    let (_, second) = stack.export().await;
    assert_eq!(first, second, "no clock and no random number in the text");
    let mut seen = BTreeSet::new();
    for entity in [
        "IFCPROJECT",
        "IFCSITE",
        "IFCBUILDING",
        "IFCBUILDINGSTOREY",
        "IFCWALL",
        "IFCSLAB",
        "IFCRELAGGREGATES",
        "IFCRELCONTAINEDINSPATIALSTRUCTURE",
        "IFCRELASSOCIATESMATERIAL",
    ] {
        for line in instances(&first, entity) {
            assert!(seen.insert(global_id(line).to_owned()), "{entity}: {line}");
        }
    }
    // 1 project, 1 site, 1 building, 2 storeys, 5 walls, 1 slab, 3 aggregations, 1 containment,
    // 6 material associations.
    assert_eq!(seen.len(), 21);
}

#[tokio::test]
async fn the_house_export_is_the_golden_file() {
    let stack = Stack::built().await;
    let (_, text) = stack.export().await;
    assert!(!text.contains('\r'), "LF line endings");
    if std::env::var_os("WRITE_GOLDEN").is_some() {
        std::fs::write(GOLDEN, &text).unwrap();
    }
    let golden = std::fs::read_to_string(GOLDEN).unwrap();
    assert_eq!(
        text, golden,
        "the export changed: regenerate the golden file and read the diff"
    );
}
