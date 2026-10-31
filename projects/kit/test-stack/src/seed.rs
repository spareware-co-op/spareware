//! The example data: three hubs, five people, and the projects in the owner's sketch. Rows go in
//! directly; models are built through the edit log and packages through the real publish, bridge
//! and consume steps, so the seed is itself a first test of the stack.

use crate::api::{ApiError, Shared};
use crate::folders;
use crate::keys;
use crate::orderer::{Reply, Submit, now_ms};
use crate::rows::{Hub, LinkKind, Model, Person, Project, Team};
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tokio::sync::mpsc;

const SEEDED: &str = "dir/seeded";

fn person(id: &str, name: &str, home_hub: &str) -> Person {
    Person {
        id: id.to_owned(),
        name: name.to_owned(),
        email: format!("{id}@example.test"),
        home_hub: home_hub.to_owned(),
    }
}

fn refused(e: ApiError) -> anyhow::Error {
    match e {
        ApiError::Internal(e) => e,
        ApiError::Refused(why) | ApiError::Bad(why) => anyhow!("seed refused: {why}"),
        ApiError::NotFound => anyhow!("seed: not found"),
        ApiError::SignIn => anyhow!("seed: sign-in"),
    }
}

/// Writes a project with its teams and models. Each team's three folders exist by being named.
async fn add_project(
    st: &Shared,
    id: &str,
    name: &str,
    hub: &str,
    teams: &[(&str, &str)],
    models: &[(&str, &str, &str, &str)],
) -> Result<()> {
    let row = Project {
        id: id.to_owned(),
        name: name.to_owned(),
        hub: hub.to_owned(),
    };
    st.kv.put(keys::project(id), &row).await?;
    st.kv.put(keys::hub_project(hub, id), &true).await?;
    for (team, team_name) in teams {
        let row = Team {
            id: (*team).to_owned(),
            name: (*team_name).to_owned(),
        };
        st.kv.put(keys::team(id, team), &row).await?;
    }
    for (model, model_name, team, package) in models {
        let row = Model {
            id: (*model).to_owned(),
            name: (*model_name).to_owned(),
            team: (*team).to_owned(),
            package: (*package).to_owned(),
        };
        st.kv.put(keys::model(id, model), &row).await?;
    }
    Ok(())
}

/// Makes someone a hub's user or external user.
async fn add_to_hub(st: &Shared, person: &str, hub: &str, role: &str) -> Result<()> {
    st.kv.put(keys::acl_hub(person, hub), &role).await?;
    st.kv.put(keys::acl_hub_user(hub, person), &role).await?;
    Ok(())
}

/// Adds someone to a project. They must already be a user or external user of the project's hub.
async fn add_to_project(st: &Shared, person: &str, project: &str) -> Result<()> {
    let row: Project = st
        .kv
        .get(keys::project(project))
        .await?
        .ok_or(anyhow!("no project"))?;
    let role: Option<String> = st.kv.get(keys::acl_hub(person, &row.hub)).await?;
    if role.is_none() {
        bail!("{person} isn't in the hub of {project}");
    }
    st.kv
        .put(keys::acl_project(person, project), &"member")
        .await
}

async fn add_to_team(st: &Shared, person: &str, project: &str, team: &str) -> Result<()> {
    add_to_project(st, person, project).await?;
    st.kv
        .put(keys::acl_team(person, project, team), &"member")
        .await
}

/// Sends a list of edits through the orderer and waits until they are all in the log.
async fn edit(
    st: &Shared,
    person: &str,
    project: &str,
    model: &str,
    elements: Vec<Value>,
) -> Result<()> {
    let orderer = st.orderers.get(project).await;
    let (reply, mut replies) = mpsc::unbounded_channel();
    let count = elements.len();
    for (index, args) in elements.into_iter().enumerate() {
        let submit = Submit {
            person: person.to_owned(),
            client: format!("seed.{model}"),
            counter: index as u64 + 1,
            base: 0,
            model: model.to_owned(),
            cmd: "element.create".to_owned(),
            args,
            author_ms: now_ms(),
            reply: reply.clone(),
        };
        orderer.submit(submit).await?;
    }
    for _ in 0..count {
        match replies.recv().await {
            Some(Reply::Ack { .. }) => {}
            other => bail!("seed edit failed: {other:?}"),
        }
    }
    Ok(())
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

pub async fn seed(st: &Shared) -> Result<()> {
    if st.kv.get::<bool>(SEEDED.to_owned()).await?.is_some() {
        println!("already seeded");
        return Ok(());
    }

    for (id, name) in [
        ("company-1", "Company 1"),
        ("company-2", "Company 2"),
        ("company-3", "Company 3"),
    ] {
        st.kv
            .put(
                keys::hub(id),
                &Hub {
                    id: id.to_owned(),
                    name: name.to_owned(),
                },
            )
            .await?;
    }
    let people = [
        person("anna", "Anna", "company-1"),
        person("ben", "Ben", "company-1"),
        person("cara", "Cara", "company-1"),
        person("dan", "Dan", "company-2"),
        person("eve", "Eve", "company-3"),
    ];
    for row in &people {
        st.kv.put(keys::person(&row.id), row).await?;
        st.kv.put(keys::email(&row.email), &row.id).await?;
        add_to_hub(st, &row.id, &row.home_hub, "user").await?;
    }
    add_to_hub(st, "dan", "company-1", "external").await?;
    add_to_hub(st, "eve", "company-1", "external").await?;

    // Company 1 and 2 permit each other. Company 1 permits Company 3, but not the other way round.
    st.kv
        .put(keys::acl_bridge("company-1", "company-2"), &true)
        .await?;
    st.kv
        .put(keys::acl_bridge("company-2", "company-1"), &true)
        .await?;
    st.kv
        .put(keys::acl_bridge("company-1", "company-3"), &true)
        .await?;

    add_project(
        st,
        "example-1",
        "Example model 1",
        "company-1",
        &[("arch", "Architecture team"), ("civil", "Civil team")],
        &[
            ("arch-model", "Arch model", "arch", "Arch package"),
            (
                "interiors-model",
                "Interiors model",
                "arch",
                "Interiors package",
            ),
            ("site-setout", "Site setout", "civil", "Site setout"),
        ],
    )
    .await?;
    add_project(
        st,
        "example-1-bridged",
        "Example model 1 (bridged)",
        "company-2",
        &[("structure", "Structure team")],
        &[(
            "structure-model",
            "Structure model",
            "structure",
            "Structure package",
        )],
    )
    .await?;
    add_project(
        st,
        "riverside",
        "Riverside Flats",
        "company-1",
        &[("arch", "Architecture team")],
        &[(
            "riverside-model",
            "Riverside model",
            "arch",
            "Riverside package",
        )],
    )
    .await?;
    add_project(
        st,
        "riverside-bridged",
        "Riverside Flats (bridged)",
        "company-3",
        &[("landscape", "Landscape team")],
        &[(
            "landscape-model",
            "Landscape model",
            "landscape",
            "Landscape package",
        )],
    )
    .await?;

    add_to_team(st, "anna", "example-1", "arch").await?;
    add_to_team(st, "ben", "example-1", "arch").await?;
    add_to_team(st, "ben", "example-1", "civil").await?;
    add_to_project(st, "dan", "example-1").await?;
    add_to_team(st, "dan", "example-1-bridged", "structure").await?;
    add_to_team(st, "cara", "riverside", "arch").await?;
    add_to_project(st, "eve", "riverside").await?;
    add_to_team(st, "eve", "riverside-bridged", "landscape").await?;

    // Packages carry on from the numbers in the sketch.
    st.kv
        .put(keys::version("example-1", "arch-model"), &10u32)
        .await?;
    st.kv
        .put(keys::version("example-1", "site-setout"), &2u32)
        .await?;
    st.kv
        .put(keys::version("example-1-bridged", "structure-model"), &7u32)
        .await?;

    let mut arch = Vec::new();
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
    }
    arch.push(door("D-104", 2, 8800, -50, 900, 300, "FD30"));
    edit(st, "anna", "example-1", "arch-model", arch).await?;

    let mut interiors = Vec::new();
    let mut setout = Vec::new();
    let mut structure = Vec::new();
    for level in [1, 2] {
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
    edit(st, "anna", "example-1", "interiors-model", interiors).await?;
    edit(st, "ben", "example-1", "site-setout", setout).await?;
    edit(st, "dan", "example-1-bridged", "structure-model", structure).await?;

    let mut riverside = arch_floor(1);
    riverside.push(door("D-101", 1, 2500, -50, 900, 300, "FD30"));
    edit(st, "cara", "riverside", "riverside-model", riverside).await?;
    edit(
        st,
        "eve",
        "riverside-bridged",
        "landscape-model",
        vec![thing("P-1", "planter", 1, 0, 0, (2000, 800, 600))],
    )
    .await?;

    folders::publish(st, "anna", "example-1", "arch", "arch-model")
        .await
        .map_err(refused)?;
    folders::publish(st, "ben", "example-1", "civil", "site-setout")
        .await
        .map_err(refused)?;
    let structure = folders::publish(
        st,
        "dan",
        "example-1-bridged",
        "structure",
        "structure-model",
    )
    .await
    .map_err(refused)?;
    let bridged = folders::bridge(
        st,
        "dan",
        "example-1-bridged",
        "structure",
        &structure.id,
        "example-1",
        "arch",
    )
    .await
    .map_err(refused)?;
    folders::consume(st, "anna", "example-1", "arch", &bridged.id)
        .await
        .map_err(refused)?;

    let host = "arch-model";
    folders::add_link(
        st,
        "anna",
        "example-1",
        host,
        LinkKind::Live,
        None,
        "interiors-model",
    )
    .await
    .map_err(refused)?;
    folders::add_link(
        st,
        "anna",
        "example-1",
        host,
        LinkKind::Shared,
        Some("civil"),
        "site-setout-v3",
    )
    .await
    .map_err(refused)?;
    let structure_id = bridged.id.clone();
    folders::add_link(
        st,
        "anna",
        "example-1",
        host,
        LinkKind::Consumed,
        None,
        &structure_id,
    )
    .await
    .map_err(refused)?;

    st.kv.put(SEEDED.to_owned(), &true).await?;
    let counts: BTreeMap<&str, u64> =
        BTreeMap::from([("hubs", 3), ("people", people.len() as u64)]);
    println!("seeded {counts:?}");
    Ok(())
}
