//! The geometry interface really is the only join. The whole server runs here on a second, different
//! shape generator: the same seed through the same doors, every rule, packages and bridges, in a
//! few lines more than the normal setup. Then the stores are compared: the data layout and the edit
//! log are identical, and only the shapes differ.

mod support;

use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Mesh, Recipe,
};
use construct_model::rows::BuiltResult;
use construct_regen::regenerator::Regenerator;
use kit_core::Id;
use kit_store::memory::{Memory, MemoryFiles};
use kit_store::{Kv, Scope};
use kit_sync::read_log;
use serde_json::Value;
use spare_server::seed::seed;
use spare_server::service::View;
use std::collections::BTreeSet;
use std::sync::Arc;
use support::build;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// Every shape is a three-sided prism, in a format nothing else has seen.
struct Prisms;

impl Geometry for Prisms {
    fn edition(&self) -> Edition {
        Edition {
            engine: "test.prisms".to_owned(),
            version: 9,
            name_scheme: 4,
        }
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let arg = |name: &str| recipe.args.get(name).and_then(Value::as_i64);
        let side = arg("length").or(arg("width")).ok_or(GeometryError {
            code: "NoSide".to_owned(),
            message: "a prism needs a side".to_owned(),
        })?;
        let height = arg("height").unwrap_or(1);
        let mesh = Mesh {
            positions: vec![0, 0, 0, side, 0, 0, 0, side, 0, 0, 0, height],
            indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3, 2, 0, 3],
        };
        let display = |lod| Display {
            lod,
            deviation: i64::from(lod),
            mesh: mesh.clone(),
            outline: vec![[0, 0], [side, 0], [0, side]],
        };
        Ok(Built {
            body: format!("prism/{side}/{height}").into_bytes(),
            bounds: Bounds {
                min: [0, 0, 0],
                max: [side, side, height],
            },
            names: vec![
                FaceName {
                    name: "p.a".to_owned(),
                    role: "exterior".to_owned(),
                },
                FaceName {
                    name: "p.b".to_owned(),
                    role: "interior".to_owned(),
                },
                FaceName {
                    name: "p.c".to_owned(),
                    role: "top".to_owned(),
                },
            ],
            displays: (0..4).map(display).collect(),
        })
    }
}

/// The store's keys with what can't match taken out: hashes of shapes, the cells they cover, random
/// client IDs and log
/// numbers, which move with when a checkpoint happened to land.
fn layout(store: &Memory) -> BTreeSet<String> {
    let mut tables = BTreeSet::new();
    for key in store.keys() {
        let parts: Vec<&str> = key.split('/').collect();
        let keep = match parts.get(1) {
            Some(&"body" | &"display" | &"area" | &"client" | &"log") => 2,
            _ => parts.len(),
        };
        tables.insert(parts[..keep].join("/"));
    }
    tables
}

async fn edits(store: &Memory, project: &str) -> Vec<Value> {
    let mut snapshot = store.snapshot().await.unwrap();
    let scope = Scope::Project(i(project));
    let log = read_log(&mut snapshot, &scope, 0, u64::MAX - 1)
        .await
        .unwrap();
    log.iter()
        .filter(|entry| entry.cmd != "document.checkpoint")
        .map(|entry| {
            let mut args = entry.args.clone();
            // A package's bytes hold the shapes, so its hash differs. That's the file, not the edit.
            if entry.cmd == "package.publish" {
                args["file"] = Value::Null;
                args["sha256"] = Value::Null;
                args["seq"] = Value::Null;
            }
            serde_json::json!({
                "cmd": entry.cmd, "target": entry.target.as_str(), "args": args,
                "changes": entry.changes, "author": entry.author.as_str(),
            })
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_generator_runs_the_whole_server_and_changes_only_the_shapes() {
    let (plain, plain_files) = (Memory::new(), MemoryFiles::default());
    let service = build(&plain, &plain_files, construct_regen::geometry());
    seed(&plain, &service).await.unwrap();

    let (store, files) = (Memory::new(), MemoryFiles::default());
    let prisms: Arc<dyn Geometry> = Arc::new(Prisms);
    let service = build(&store, &files, prisms.clone());
    seed(&store, &service).await.unwrap();

    let (anna, project, model) = (i("anna"), i("example-1"), i("arch-model"));
    let view = View {
        level: 1,
        x0: 0,
        y0: 0,
        x1: 9000,
        y1: 9000,
    };
    let area = service.area(&anna, &project, &model, &view).await.unwrap();
    assert!(
        !area.pieces.is_empty(),
        "the area index needs nothing but bounds"
    );

    let shown = service
        .shape(&anna, &project, &model, &i("W-101"), Some(3))
        .await
        .unwrap();
    assert_eq!(shown.display.as_ref().unwrap().outline.len(), 3);
    let BuiltResult::Shape {
        names, displays, ..
    } = &shown.built.result
    else {
        panic!("it builds");
    };
    assert_eq!(names.len(), 3);
    assert_eq!(displays.len(), 4);
    assert_eq!(shown.built.edition, prisms.edition());

    let regen = Regenerator::new(store.clone(), files.clone(), prisms);
    assert_eq!(regen.verify(&project).await.unwrap(), 31);

    for project in [
        "example-1",
        "example-1-bridged",
        "riverside",
        "riverside-bridged",
    ] {
        assert_eq!(
            edits(&store, project).await,
            edits(&plain, project).await,
            "{project}"
        );
    }
    assert_eq!(layout(&store), layout(&plain));
    let plain_bodies = plain.keys().iter().filter(|k| k.contains("/body/")).count();
    let bodies = store.keys().iter().filter(|k| k.contains("/body/")).count();
    assert_ne!((plain_bodies, bodies), (0, 0));
}

/// Nothing above the interface names the stand-in, so swapping it is the one line in
/// `construct_regen::geometry`.
#[test]
fn only_the_regen_crate_knows_the_stand_in() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let mut files = vec![format!("{root}/app/connect.js")];
    for folder in [
        "server/src",
        "crates/construct-api/src/cloud",
        "crates/construct-model/src",
    ] {
        for entry in std::fs::read_dir(format!("{root}/{folder}")).unwrap() {
            files.push(entry.unwrap().path().display().to_string());
        }
    }
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for word in ["stand_in", "Boxes", "BoxGeometry", "shape.w", "BoxBody"] {
            assert!(!text.contains(word), "{file} knows about {word}");
        }
    }
}
