//! The example data: three hubs, five people, and the projects in the owner's sketch. It goes in
//! through the same doors as everything else: directory commands, the edit log, and the real
//! publish, bridge and consume steps, each with Cedar's yes. So the seed is itself a first test.

use crate::service::{Refusal, Service};
use crate::spaces::LinkKind;
use kit_accounts::SYSTEM;
use kit_accounts::sign_in::email_hash;
use kit_core::Id;
use kit_store::{FileStore, Kv, Place, Reader, StoreError};
use serde_json::{Value, json};

fn id(text: &str) -> Id {
    // Every ID in this file is a constant, and valid.
    Id::parse(text).unwrap_or_else(|_| Id::from_time_and_random(0, [0; 10]))
}

fn wall(id: &str, level: i32, x: i64, y: i64, w: i64, d: i64) -> Value {
    json!({ "element": id, "kind": "wall", "level": level, "x": x, "y": y, "w": w, "d": d, "h": 3000,
            "props": { "category": "Walls", "type": "Basic wall" } })
}

fn door(id: &str, level: i32, x: i64, y: i64, w: i64, d: i64, fire: &str) -> Value {
    json!({ "element": id, "kind": "door", "level": level, "x": x, "y": y, "w": w, "d": d, "h": 2100,
            "props": { "category": "Doors", "type": "Single 900", "fire rating": fire } })
}

fn thing(id: &str, kind: &str, level: i32, x: i64, y: i64, size: (i64, i64, i64)) -> Value {
    json!({ "element": id, "kind": kind, "level": level, "x": x, "y": y,
            "w": size.0, "d": size.1, "h": size.2, "props": { "category": kind } })
}

/// A plain two-room floor.
fn arch_floor(level: i32) -> Vec<Value> {
    let tag = |n: i32| format!("W-{level}0{n}");
    vec![
        wall(&tag(1), level, 0, 0, 12_000, 200),
        wall(&tag(2), level, 0, 7800, 12_000, 200),
        wall(&tag(3), level, 0, 200, 200, 7600),
        wall(&tag(4), level, 11_800, 200, 200, 7600),
        wall(&tag(5), level, 6000, 200, 200, 7600),
    ]
}

/// Whether this store has the example data.
async fn seeded<K: Kv>(kv: &K) -> Result<bool, StoreError> {
    let mut snapshot = kv.snapshot().await?;
    Ok(snapshot
        .get(&Place::Hub(id("company-1")).key())
        .await?
        .is_some())
}

/// Writes the example data. Does nothing to a store that has it.
pub async fn seed<K: Kv, F: FileStore>(kv: &K, service: &Service<K, F>) -> Result<(), Refusal> {
    service.accounts.start_directory().await?;
    if seeded(kv).await? {
        println!("already seeded");
        return Ok(());
    }
    let system = id(SYSTEM);
    let directory = async |cmd: &str, args: Value| -> Result<(), StoreError> {
        service.accounts.edit_directory(&system, cmd, args).await
    };

    for (hub, name) in [
        ("company-1", "Company 1"),
        ("company-2", "Company 2"),
        ("company-3", "Company 3"),
    ] {
        directory("hub.add", json!({ "id": hub, "name": name })).await?;
    }
    for (person, name, home) in [
        ("anna", "Anna", "company-1"),
        ("ben", "Ben", "company-1"),
        ("cara", "Cara", "company-1"),
        ("dan", "Dan", "company-2"),
        ("eve", "Eve", "company-3"),
    ] {
        let email_hash = email_hash(&format!("{person}@example.test"));
        let args =
            json!({ "id": person, "name": name, "home_hub": home, "email_hash": email_hash });
        directory("person.add", args).await?;
    }
    for person in ["dan", "eve"] {
        directory(
            "hub.external",
            json!({ "hub": "company-1", "person": person }),
        )
        .await?;
    }
    // Company 1 and 2 permit each other. Company 1 permits Company 3, but not the other way round.
    for (hub, other) in [
        ("company-1", "company-2"),
        ("company-2", "company-1"),
        ("company-1", "company-3"),
    ] {
        directory("hub.permit", json!({ "hub": hub, "other": other })).await?;
    }
    for (project, name, hub) in [
        ("example-1", "Example model 1", "company-1"),
        (
            "example-1-bridged",
            "Example model 1 (bridged)",
            "company-2",
        ),
        ("riverside", "Riverside Flats", "company-1"),
        (
            "riverside-bridged",
            "Riverside Flats (bridged)",
            "company-3",
        ),
    ] {
        directory(
            "project.add",
            json!({ "id": project, "name": name, "hub": hub }),
        )
        .await?;
    }
    for (project, team, name) in [
        ("example-1", "arch", "Architecture team"),
        ("example-1", "civil", "Civil team"),
        ("example-1-bridged", "structure", "Structure team"),
        ("riverside", "arch", "Architecture team"),
        ("riverside-bridged", "landscape", "Landscape team"),
    ] {
        directory(
            "team.add",
            json!({ "project": project, "id": team, "name": name }),
        )
        .await?;
    }
    for (project, team, person) in [
        ("example-1", "arch", "anna"),
        ("example-1", "arch", "ben"),
        ("example-1", "civil", "ben"),
        ("example-1-bridged", "structure", "dan"),
        ("riverside", "arch", "cara"),
        ("riverside-bridged", "landscape", "eve"),
    ] {
        directory(
            "team.person",
            json!({ "project": project, "team": team, "person": person }),
        )
        .await?;
    }
    for (project, person) in [("example-1", "dan"), ("riverside", "eve")] {
        directory(
            "project.person",
            json!({ "project": project, "person": person }),
        )
        .await?;
    }

    // Package numbers carry on from the numbers in the sketch.
    for (person, project, model, name, team, package, last) in [
        (
            "anna",
            "example-1",
            "arch-model",
            "Arch model",
            "arch",
            "Arch package",
            10,
        ),
        (
            "anna",
            "example-1",
            "interiors-model",
            "Interiors model",
            "arch",
            "Interiors package",
            0,
        ),
        (
            "ben",
            "example-1",
            "site-setout",
            "Site setout",
            "civil",
            "Site setout",
            2,
        ),
        (
            "dan",
            "example-1-bridged",
            "structure-model",
            "Structure model",
            "structure",
            "Structure package",
            7,
        ),
        (
            "cara",
            "riverside",
            "riverside-model",
            "Riverside model",
            "arch",
            "Riverside package",
            0,
        ),
        (
            "eve",
            "riverside-bridged",
            "landscape-model",
            "Landscape model",
            "landscape",
            "Landscape package",
            0,
        ),
    ] {
        let args = json!({ "team": team, "name": name, "package": package, "last_version": last });
        service
            .edit(&id(person), &id(project), &id(model), "model.add", args)
            .await?;
    }

    let mut arch = Vec::new();
    let mut interiors = Vec::new();
    let mut setout = Vec::new();
    let mut structure = Vec::new();
    for level in [1, 2] {
        arch.extend(arch_floor(level));
        arch.push(door(
            &format!("D-{level}01"),
            level,
            2500,
            -50,
            900,
            300,
            "FD30",
        ));
        arch.push(door(
            &format!("D-{level}02"),
            level,
            5950,
            3000,
            300,
            900,
            "none",
        ));
        arch.push(door(
            &format!("D-{level}03"),
            level,
            9000,
            7750,
            900,
            300,
            "FD30",
        ));
        interiors.push(thing(
            &format!("B-{level}01"),
            "joinery",
            level,
            700,
            700,
            (3000, 600, 900),
        ));
        interiors.push(thing(
            &format!("B-{level}02"),
            "joinery",
            level,
            7000,
            6500,
            (2400, 600, 900),
        ));
        for (name, x) in [("A", 100), ("B", 6100), ("C", 11_900)] {
            setout.push(thing(
                &format!("GL-{name}{level}"),
                "grid",
                level,
                x,
                -1500,
                (40, 11_000, 10),
            ));
        }
        for (name, y) in [("1", 100), ("2", 7900)] {
            setout.push(thing(
                &format!("GL-{name}{level}"),
                "grid",
                level,
                -1500,
                y,
                (15_000, 40, 10),
            ));
        }
        for (index, (x, y)) in [(0, 0), (11_600, 0), (0, 7600), (11_600, 7600), (5900, 3900)]
            .iter()
            .enumerate()
        {
            structure.push(thing(
                &format!("C-{level}0{index}"),
                "column",
                level,
                *x,
                *y,
                (400, 400, 3000),
            ));
        }
    }
    arch.push(door("D-104", 2, 8800, -50, 900, 300, "FD30"));
    let mut riverside = arch_floor(1);
    riverside.push(door("D-101", 1, 2500, -50, 900, 300, "FD30"));
    let planter = vec![thing("P-1", "planter", 1, 0, 0, (2000, 800, 600))];

    for (person, project, model, elements) in [
        ("anna", "example-1", "arch-model", arch),
        ("anna", "example-1", "interiors-model", interiors),
        ("ben", "example-1", "site-setout", setout),
        ("dan", "example-1-bridged", "structure-model", structure),
        ("cara", "riverside", "riverside-model", riverside),
        ("eve", "riverside-bridged", "landscape-model", planter),
    ] {
        for args in elements {
            service
                .edit(
                    &id(person),
                    &id(project),
                    &id(model),
                    "element.create",
                    args,
                )
                .await?;
        }
    }

    let (anna, ben, dan) = (id("anna"), id("ben"), id("dan"));
    let (example, bridged) = (id("example-1"), id("example-1-bridged"));
    let (arch_team, civil, structure_team) = (id("arch"), id("civil"), id("structure"));
    let arch_model = id("arch-model");
    service
        .publish(&anna, &example, &arch_team, &arch_model)
        .await?;
    service
        .publish(&ben, &example, &civil, &id("site-setout"))
        .await?;
    let package = service
        .publish(&dan, &bridged, &structure_team, &id("structure-model"))
        .await?;
    service
        .bridge(
            &dan,
            &bridged,
            &structure_team,
            &package.id,
            &example,
            &arch_team,
        )
        .await?;
    let incoming = id(&format!("company-2.{}", package.id));
    service
        .consume(&anna, &example, &arch_team, &incoming)
        .await?;

    service
        .add_link(
            &anna,
            &example,
            &arch_model,
            LinkKind::Live,
            None,
            &id("interiors-model"),
        )
        .await?;
    service
        .add_link(
            &anna,
            &example,
            &arch_model,
            LinkKind::Shared,
            Some(&civil),
            &id("site-setout-v3"),
        )
        .await?;
    service
        .add_link(
            &anna,
            &example,
            &arch_model,
            LinkKind::Consumed,
            None,
            &incoming,
        )
        .await?;
    println!("seeded");
    Ok(())
}
