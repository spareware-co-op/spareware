//! The commands that change the directory. "Every admin action is an operation in the log", says
//! the kit's map, so the directory has an edit log like a project's, and these commands are the
//! only way to write its rows. Each needs Cedar's yes.
//!
//! A new store has nobody who could say yes, so the first entry is special: `directory.start`
//! creates the format record and the first administrator, and is turned back once the log isn't
//! empty. There is no other way in, for the seed data or anything else.

use crate::access::{Access, Target};
use crate::rows::{Hub, HubRole, Person, Project, Team};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{FORMAT, Kv, Place, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Commands, Context, Outcome};
use serde::Deserialize;
use std::sync::Arc;

/// The server's own account: the first administrator, and the author of what the server itself
/// does to the directory, such as applying `STACK_PEOPLE`.
pub const SYSTEM: &str = "system";

/// The directory's command version, recorded in each entry.
const VERSION: u32 = 1;

pub struct DirectoryCommands<K: Kv> {
    access: Arc<Access<K>>,
}

impl<K: Kv> DirectoryCommands<K> {
    #[must_use]
    pub fn new(access: Arc<Access<K>>) -> Self {
        Self { access }
    }
}

#[derive(Deserialize)]
struct AddPerson {
    #[serde(with = "kit_ops::id")]
    id: Id,
    name: String,
    #[serde(with = "kit_ops::id")]
    home_hub: Id,
    /// The SHA-256 of the lowercased address. The address itself is never stored.
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
}

#[derive(Deserialize)]
struct SetEmail {
    #[serde(with = "kit_ops::id")]
    person: Id,
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
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

#[derive(Deserialize)]
struct BindLogin {
    #[serde(with = "kit_ops::id")]
    login: Id,
    #[serde(with = "kit_ops::id")]
    email_hash: Id,
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

impl<K: Kv> Commands<K> for DirectoryCommands<K> {
    fn version(&self) -> u32 {
        VERSION
    }

    async fn permit(&self, _scope: &Scope, edit: &Edit) -> Result<Result<(), String>, StoreError> {
        let allowed = match edit.cmd.as_str() {
            // Allowed once, on an empty log: `apply` checks that.
            "directory.start" => true,
            "login.bind" => {
                let target = Target::Person(&edit.person);
                self.access
                    .allowed(&edit.person, "login.bind", target, None)
                    .await?
            }
            _ => {
                self.access
                    .allowed(&edit.person, "directory.edit", Target::Directory, None)
                    .await?
            }
        };
        Ok(if allowed {
            Ok(())
        } else {
            Err("You can't change the directory.".to_owned())
        })
    }

    fn permit_group(&self, edit: &Edit) -> String {
        format!("{}/{}", edit.person, edit.cmd)
    }

    async fn apply(
        &self,
        txn: &mut K::Txn,
        context: &Context<'_>,
        edit: &Edit,
    ) -> Result<Outcome, StoreError> {
        match edit.cmd.as_str() {
            "directory.start" => {
                if context.head != 0 {
                    return Ok(Err("The directory has already been started.".to_owned()));
                }
                put(txn, Place::Format, &FORMAT).await?;
                let admin = Person {
                    id: edit.person.clone(),
                    name: "System".to_owned(),
                    home_hub: edit.person.clone(),
                };
                put(txn, Place::Person(admin.id.clone()), &admin).await?;
                put(txn, Place::Admin(admin.id), &true).await?;
            }
            "person.add" => {
                let args = args!(edit, AddPerson);
                if !has(txn, Place::Hub(args.home_hub.clone())).await? {
                    return Ok(Err("There's no such hub.".to_owned()));
                }
                let row = Person {
                    id: args.id.clone(),
                    name: args.name,
                    home_hub: args.home_hub.clone(),
                };
                put(txn, Place::Person(args.id.clone()), &row).await?;
                put(txn, Place::Email(args.email_hash), &args.id.as_str()).await?;
                add_to_hub(txn, &args.home_hub, &args.id, HubRole::User).await?;
            }
            "person.email" => {
                let args = args!(edit, SetEmail);
                if !has(txn, Place::Person(args.person.clone())).await? {
                    return Ok(Err("There's no such person.".to_owned()));
                }
                put(txn, Place::Email(args.email_hash), &args.person.as_str()).await?;
            }
            "hub.add" => {
                let args = args!(edit, AddHub);
                let row = Hub {
                    id: args.id.clone(),
                    name: args.name,
                };
                put(txn, Place::Hub(args.id), &row).await?;
            }
            "hub.external" => {
                let args = args!(edit, HubPerson);
                if !has(txn, Place::Person(args.person.clone())).await? {
                    return Ok(Err("There's no such person.".to_owned()));
                }
                add_to_hub(txn, &args.hub, &args.person, HubRole::External).await?;
            }
            "hub.permit" => {
                let args = args!(edit, PermitBridge);
                put(
                    txn,
                    Place::Bridge {
                        hub: args.hub,
                        other: args.other,
                    },
                    &true,
                )
                .await?;
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
                put(
                    txn,
                    Place::HubProject {
                        hub: args.hub,
                        project: args.id,
                    },
                    &true,
                )
                .await?;
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
                put(
                    txn,
                    Place::Team {
                        project: args.project,
                        team: args.id,
                    },
                    &row,
                )
                .await?;
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
            "login.bind" => {
                let args = args!(edit, BindLogin);
                // The address must already belong to the person binding it.
                let owner: Option<String> =
                    read_row(txn, &Place::Email(args.email_hash).key()).await?;
                if owner.as_deref() != Some(edit.person.as_str()) {
                    return Ok(Err("That address belongs to someone else.".to_owned()));
                }
                put(txn, Place::Login(args.login), &edit.person.as_str()).await?;
            }
            other => return Ok(Err(format!("Unknown command {other}."))),
        }
        Ok(Ok(Applied::default()))
    }
}
