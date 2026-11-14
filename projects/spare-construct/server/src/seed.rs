//! The example data: three hubs, five people, and the projects in the owner's sketch. It goes in
//! through the same doors as everything else: directory commands, the edit log, and the real
//! publish, bridge and consume steps, each with Cedar's yes. So the seed is itself a first test.

use crate::service::{Refusal, Service};
use construct_model::geometry::Recipe;
use construct_model::keys::Place;
use construct_model::rows::LinkKind;
use kit_accounts::SYSTEM;
use kit_accounts::sign_in::email_hash;
use kit_core::Id;
use kit_store::{FileStore, Kv, Reader, StoreError};
use serde_json::{Value, json};

fn id(text: &str) -> Id {
    // Every ID in this file is a constant, and valid.
    Id::parse(text).unwrap_or_else(|_| Id::from_time_and_random(0, [0; 10]))
}

/// Walls run along their long side, and turn by swapping the two sides of the plan.
fn wall(id: &str, level: i32, x: i64, y: i64, w: i64, d: i64) -> Value {
    let (length, thickness, axis) = if w >= d { (w, d, "x") } else { (d, w, "y") };
    let recipe = recipe(
        "wall.straight",
        json!({ "length": length, "thickness": thickness, "height": 3000, "axis": axis }),
    );
    json!({ "element": id, "kind": "wall", "level": level, "placement": { "x": x, "y": y, "z": 0 },
            "recipe": recipe, "props": { "category": "Walls", "type": "Basic wall" } })
}

fn door(id: &str, level: i32, x: i64, y: i64, w: i64, d: i64, fire: &str) -> Value {
    let (width, thickness, axis) = if w >= d { (w, d, "x") } else { (d, w, "y") };
    let recipe = recipe(
        "door.single",
        json!({ "width": width, "thickness": thickness, "height": 2100, "axis": axis }),
    );
    json!({ "element": id, "kind": "door", "level": level, "placement": { "x": x, "y": y, "z": 0 },
            "recipe": recipe,
            "props": { "category": "Doors", "type": "Single 900", "fire rating": fire } })
}

fn thing(id: &str, kind: &str, level: i32, x: i64, y: i64, size: (i64, i64, i64)) -> Value {
    let args = json!({ "width": size.0, "depth": size.1, "height": size.2 });
    json!({ "element": id, "kind": kind, "level": level, "placement": { "x": x, "y": y, "z": 0 },
            "recipe": recipe("block", args), "props": { "category": kind } })
}

/// A recipe for one of Spare Construct's operations, as an edit carries it.
fn recipe(op: &str, args: Value) -> Value {
    // A map of strings to numbers always serialises.
    serde_json::to_value(Recipe::construct(
        op,
        serde_json::from_value(args).unwrap_or_default(),
    ))
    .unwrap_or_default()
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
///
/// # Errors
///
/// A [`Refusal`] when any step of it is refused, or the store fails.
pub async fn seed<K: Kv, F: FileStore>(kv: &K, service: &Service<K, F>) -> Result<(), Refusal> {
    service.accounts.start_directory().await?;
    if seeded(kv).await? {
        println!("already seeded");
        return Ok(());
    }
    hubs_and_people(service).await?;
    projects_and_teams(service).await?;
    models(service).await?;
    elements(service).await?;
    packages_and_links(service).await?;
    println!("seeded");
    Ok(())
}

/// One directory command, from the system.
async fn directory<K: Kv, F: FileStore>(
    service: &Service<K, F>,
    cmd: &str,
    args: Value,
) -> Result<(), StoreError> {
    service
        .accounts
        .edit_directory(&id(SYSTEM), cmd, args)
        .await
}

/// The three hubs, the five people, and which hubs permit bridges with which.
async fn hubs_and_people<K: Kv, F: FileStore>(service: &Service<K, F>) -> Result<(), StoreError> {
    for (hub, name) in [
        ("company-1", "Company 1"),
        ("company-2", "Company 2"),
        ("company-3", "Company 3"),
    ] {
        directory(service, "hub.add", json!({ "id": hub, "name": name })).await?;
    }
    for (person, name, home) in [
        ("anna", "Anna", "company-1"),
        ("ben", "Ben", "company-1"),
        ("cara", "Cara", "company-1"),
        ("dan", "Dan", "company-2"),
        ("eve", "Eve", "company-3"),
    ] {
        let email_hash = email_hash(&format!("{person}@example.test"));
        let args = json!({ "id": person, "name": name, "email_hash": email_hash });
        directory(service, "person.add", args).await?;
        let args = json!({ "hub": home, "person": person });
        directory(service, "hub.user", args).await?;
    }
    for person in ["dan", "eve"] {
        let args = json!({ "hub": "company-1", "person": person });
        directory(service, "hub.external", args).await?;
    }
    // Company 1 and 2 permit each other. Company 1 permits Company 3, but not the other way round.
    for (hub, other) in [
        ("company-1", "company-2"),
        ("company-2", "company-1"),
        ("company-1", "company-3"),
    ] {
        let args = json!({ "hub": hub, "other": other });
        directory(service, "hub.permit", args).await?;
    }
    Ok(())
}

/// The four projects, their teams, and who is on each.
async fn projects_and_teams<K: Kv, F: FileStore>(
    service: &Service<K, F>,
) -> Result<(), StoreError> {
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
        let args = json!({ "id": project, "name": name, "hub": hub });
        directory(service, "project.add", args).await?;
    }
    for (project, team, name) in [
        ("example-1", "arch", "Architecture team"),
        ("example-1", "civil", "Civil team"),
        ("example-1-bridged", "structure", "Structure team"),
        ("riverside", "arch", "Architecture team"),
        ("riverside-bridged", "landscape", "Landscape team"),
    ] {
        let args = json!({ "project": project, "id": team, "name": name });
        directory(service, "team.add", args).await?;
    }
    for (project, team, person) in [
        ("example-1", "arch", "anna"),
        ("example-1", "arch", "ben"),
        ("example-1", "civil", "ben"),
        ("example-1-bridged", "structure", "dan"),
        ("riverside", "arch", "cara"),
        ("riverside-bridged", "landscape", "eve"),
    ] {
        let args = json!({ "project": project, "team": team, "person": person });
        directory(service, "team.person", args).await?;
    }
    for (project, person) in [("example-1", "dan"), ("riverside", "eve")] {
        let args = json!({ "project": project, "person": person });
        directory(service, "project.person", args).await?;
    }
    Ok(())
}

/// The six models. Package numbers carry on from the numbers in the sketch.
async fn models<K: Kv, F: FileStore>(service: &Service<K, F>) -> Result<(), Refusal> {
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
    Ok(())
}

/// Each floor of Example model 1, split between the models that draw it: the architecture with
/// its doors, the joinery, the grid lines and the columns.
fn example_floors() -> [Vec<Value>; 4] {
    let mut arch = Vec::new();
    let mut interiors = Vec::new();
    let mut setout = Vec::new();
    let mut structure = Vec::new();
    for level in [1, 2] {
        arch.extend(arch_floor(level));
        for (n, x, y, w, d, fire) in [
            (1, 2500, -50, 900, 300, "FD30"),
            (2, 5950, 3000, 300, 900, "none"),
            (3, 9000, 7750, 900, 300, "FD30"),
        ] {
            arch.push(door(&format!("D-{level}0{n}"), level, x, y, w, d, fire));
        }
        for (n, x, y, width) in [(1, 700, 700, 3000), (2, 7000, 6500, 2400)] {
            let name = format!("B-{level}0{n}");
            interiors.push(thing(&name, "joinery", level, x, y, (width, 600, 900)));
        }
        for (name, x) in [("A", 100), ("B", 6100), ("C", 11_900)] {
            let name = format!("GL-{name}{level}");
            setout.push(thing(&name, "grid", level, x, -1500, (40, 11_000, 10)));
        }
        for (name, y) in [("1", 100), ("2", 7900)] {
            let name = format!("GL-{name}{level}");
            setout.push(thing(&name, "grid", level, -1500, y, (15_000, 40, 10)));
        }
        for (index, (x, y)) in [(0, 0), (11_600, 0), (0, 7600), (11_600, 7600), (5900, 3900)]
            .iter()
            .enumerate()
        {
            let name = format!("C-{level}0{index}");
            structure.push(thing(&name, "column", level, *x, *y, (400, 400, 3000)));
        }
    }
    arch.push(door("D-104", 2, 8800, -50, 900, 300, "FD30"));
    [arch, interiors, setout, structure]
}

/// Every element of every model.
async fn elements<K: Kv, F: FileStore>(service: &Service<K, F>) -> Result<(), Refusal> {
    let [arch, interiors, setout, structure] = example_floors();
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
    Ok(())
}

/// The packages published, bridged and taken in, the links between models, and every project's
/// shapes built.
async fn packages_and_links<K: Kv, F: FileStore>(service: &Service<K, F>) -> Result<(), Refusal> {
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

    for (kind, team, source) in [
        (LinkKind::Live, None, id("interiors-model")),
        (LinkKind::Shared, Some(&civil), id("site-setout-v3")),
        (LinkKind::Consumed, None, incoming.clone()),
    ] {
        service
            .add_link(&anna, &example, &arch_model, kind, team, &source)
            .await?;
    }
    for project in [
        &example,
        &bridged,
        &id("riverside"),
        &id("riverside-bridged"),
    ] {
        service.regenerate(project).await?;
    }
    Ok(())
}
