//! The directory commands that are Spare Construct's: hubs, projects and teams (CON-A25-F1). They
//! sit in the directory's log beside the kit's commands for people, and the server runs both from
//! one list, each behind Cedar. Every project's people come from its hub's users and external users.

use construct_model::keys::Place;
use construct_model::rows::{Hub, HubRole, Project, Team};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{Place as KitPlace, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Outcome};
use serde::Deserialize;

/// Whether a command is one of these.
#[must_use]
pub fn handles(cmd: &str) -> bool {
    matches!(
        cmd,
        "hub.add"
            | "hub.user"
            | "hub.external"
            | "hub.permit"
            | "project.add"
            | "project.person"
            | "team.add"
            | "team.person"
    )
}

#[derive(Deserialize)]
struct AddHub {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
}

#[derive(Deserialize)]
struct HubPerson {
    #[serde(with = "kit_ops::id")]
    hub: Id,
    #[serde(with = "kit_ops::id")]
    person: Id,
}

#[derive(Deserialize)]
struct PermitBridge {
    #[serde(with = "kit_ops::id")]
    hub: Id,
    #[serde(with = "kit_ops::id")]
    other: Id,
}

#[derive(Deserialize)]
struct AddProject {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
    #[serde(with = "kit_ops::id")]
    hub: Id,
}

#[derive(Deserialize)]
struct ProjectPerson {
    #[serde(with = "kit_ops::id")]
    project: Id,
    #[serde(with = "kit_ops::id")]
    person: Id,
}

#[derive(Deserialize)]
struct AddTeam {
    #[serde(with = "kit_ops::id")]
    project: Id,
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
}

#[derive(Deserialize)]
struct TeamPerson {
    #[serde(with = "kit_ops::id")]
    project: Id,
    #[serde(with = "kit_ops::id")]
    team: Id,
    #[serde(with = "kit_ops::id")]
    person: Id,
}

/// Reads a command's arguments, or turns the edit back saying why.
macro_rules! args {
    ($edit:expr, $kind:ty) => {
        match serde_json::from_value::<$kind>($edit.args.clone()) {
            Ok(args) => args,
            Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
        }
    };
}

async fn put<T: Txn>(
    txn: &mut T,
    place: Place,
    row: &impl serde::Serialize,
) -> Result<(), StoreError> {
    txn.put(place.key(), to_row(row)?).await
}

async fn has<T: Txn>(txn: &mut T, place: Place) -> Result<bool, StoreError> {
    Ok(txn.get(&place.key()).await?.is_some())
}

async fn is_person<T: Txn>(txn: &mut T, person: &Id) -> Result<bool, StoreError> {
    Ok(txn
        .get(&KitPlace::Person(person.clone()).key())
        .await?
        .is_some())
}

async fn add_to_hub<T: Txn>(
    txn: &mut T,
    hub: &Id,
    person: &Id,
    role: HubRole,
) -> Result<(), StoreError> {
    put(
        txn,
        Place::MemberOfHub {
            person: person.clone(),
            hub: hub.clone(),
        },
        &role,
    )
    .await?;
    put(
        txn,
        Place::HubUser {
            hub: hub.clone(),
            person: person.clone(),
        },
        &role,
    )
    .await
}

/// People are added to a project from its hub's users and external users, and no other way.
async fn add_to_project<T: Txn>(
    txn: &mut T,
    project: &Id,
    person: &Id,
) -> Result<Result<(), String>, StoreError> {
    let row: Option<Project> = read_row(txn, &Place::Project(project.clone()).key()).await?;
    let Some(row) = row else {
        return Ok(Err("There's no such project.".to_owned()));
    };
    let member = Place::MemberOfHub {
        person: person.clone(),
        hub: row.hub.clone(),
    };
    if !has(txn, member).await? {
        return Ok(Err(format!("{person} isn't in the project's hub.")));
    }
    let place = Place::MemberOfProject {
        person: person.clone(),
        project: project.clone(),
    };
    put(txn, place, &true).await?;
    Ok(Ok(()))
}

/// Runs one of these commands.
pub async fn apply<T: Txn>(txn: &mut T, edit: &Edit) -> Result<Outcome, StoreError> {
    match edit.cmd.as_str() {
        "hub.add" => {
            let args = args!(edit, AddHub);
            let row = Hub {
                id: args.id.clone(),
                name: args.name,
            };
            put(txn, Place::Hub(args.id), &row).await?;
        }
        // Someone's first hub is their home.
        "hub.user" => {
            let args = args!(edit, HubPerson);
            if !is_person(txn, &args.person).await? {
                return Ok(Err("There's no such person.".to_owned()));
            }
            if !has(txn, Place::Hub(args.hub.clone())).await? {
                return Ok(Err("There's no such hub.".to_owned()));
            }
            add_to_hub(txn, &args.hub, &args.person, HubRole::User).await?;
            let home = Place::Home(args.person.clone());
            if !has(txn, home.clone()).await? {
                put(txn, home, &args.hub.as_str()).await?;
            }
        }
        "hub.external" => {
            let args = args!(edit, HubPerson);
            if !is_person(txn, &args.person).await? {
                return Ok(Err("There's no such person.".to_owned()));
            }
            add_to_hub(txn, &args.hub, &args.person, HubRole::External).await?;
        }
        "hub.permit" => {
            let args = args!(edit, PermitBridge);
            let place = Place::Bridge {
                hub: args.hub,
                other: args.other,
            };
            put(txn, place, &true).await?;
        }
        "project.add" => {
            let args = args!(edit, AddProject);
            if !has(txn, Place::Hub(args.hub.clone())).await? {
                return Ok(Err("There's no such hub.".to_owned()));
            }
            let row = Project {
                id: args.id.clone(),
                name: args.name,
                hub: args.hub.clone(),
            };
            put(txn, Place::Project(args.id.clone()), &row).await?;
            let place = Place::HubProject {
                hub: args.hub,
                project: args.id,
            };
            put(txn, place, &true).await?;
        }
        "project.person" => {
            let args = args!(edit, ProjectPerson);
            if let Err(reason) = add_to_project(txn, &args.project, &args.person).await? {
                return Ok(Err(reason));
            }
        }
        "team.add" => {
            let args = args!(edit, AddTeam);
            let row = Team {
                id: args.id.clone(),
                name: args.name,
            };
            let place = Place::Team {
                project: args.project,
                team: args.id,
            };
            put(txn, place, &row).await?;
        }
        "team.person" => {
            let args = args!(edit, TeamPerson);
            if let Err(reason) = add_to_project(txn, &args.project, &args.person).await? {
                return Ok(Err(reason));
            }
            let place = Place::MemberOfTeam {
                person: args.person,
                project: args.project,
                team: args.team,
            };
            put(txn, place, &true).await?;
        }
        other => return Ok(Err(format!("Unknown command {other}."))),
    }
    Ok(Ok(Applied::default()))
}
