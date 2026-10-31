//! Models, packages and links: WIP, Shared and Consumed folders, numbered packages, bridges
//! between hubs, and the three ways one model links another. These are ordered commands like any
//! edit, so the log says who shared, bridged or took what, and when.
//!
//! This is Spare Construct's way of working, so by the layering rule it isn't in the kit yet. It
//! moves down when a second program needs it (EXTENDING.md, section 1).

use kit_accounts::rows::{Hub, Model, Project};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::{Place, Scope, StoreError, Txn, read_row, to_row};
use kit_sync::{Applied, Context, Elsewhere, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Where a consumed package came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Origin {
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
    pub hub_name: String,
    #[serde(with = "kit_ops::id")]
    pub project: Id,
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    #[serde(with = "kit_ops::id")]
    pub package: Id,
}

/// A numbered package in a Shared or Consumed folder. The bytes are a file; this says where it
/// is and what its hash must be.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    pub version: u32,
    #[serde(with = "kit_ops::id")]
    pub model: Id,
    /// The log number the package froze the model at.
    pub seq: u64,
    pub file: String,
    pub sha256: String,
    pub elements: usize,
    #[serde(with = "kit_ops::id")]
    pub by: Id,
    pub from: Option<Origin>,
    pub bridged_to: Vec<String>,
    /// The log entry that made this package.
    pub entry: u64,
}

/// A package bridged to a hub, waiting for the team to take it into Consumed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Incoming {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    #[serde(with = "kit_ops::id")]
    pub to_team: Id,
    pub item: Item,
    pub origin: Origin,
}

/// The three ways a model can link another.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// Follows a model in the same team's WIP.
    Live,
    /// A fixed package from a Shared folder.
    Shared,
    /// A fixed package from the team's Consumed folder.
    Consumed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Link {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub kind: LinkKind,
    pub name: String,
    /// The model a live link follows.
    #[serde(with = "kit_ops::id::option")]
    pub model: Option<Id>,
    /// The team and package a fixed link points at.
    #[serde(with = "kit_ops::id::option")]
    pub team: Option<Id>,
    #[serde(with = "kit_ops::id::option")]
    pub package: Option<Id>,
}

#[derive(Deserialize)]
pub struct AddModel {
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    pub name: String,
    pub package: String,
    /// Where package numbers carry on from, for a model that started elsewhere.
    #[serde(default)]
    pub last_version: u32,
}

#[derive(Deserialize)]
pub struct Publish {
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    pub seq: u64,
    pub file: String,
    pub sha256: String,
    pub elements: usize,
}

#[derive(Deserialize)]
pub struct Bridge {
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    #[serde(with = "kit_ops::id")]
    pub to_project: Id,
    #[serde(with = "kit_ops::id")]
    pub to_team: Id,
}

#[derive(Deserialize)]
pub struct Take {
    #[serde(with = "kit_ops::id")]
    pub team: Id,
}

#[derive(Deserialize)]
pub struct AddLink {
    pub kind: LinkKind,
    #[serde(default, with = "kit_ops::id::option")]
    pub team: Option<Id>,
    #[serde(with = "kit_ops::id")]
    pub source: Id,
}

fn fault(e: impl std::fmt::Display) -> StoreError {
    StoreError::Damaged(e.to_string())
}

macro_rules! args {
    ($edit:expr, $kind:ty) => {
        match serde_json::from_value::<$kind>($edit.args.clone()) {
            Ok(args) => args,
            Err(e) => return Ok(Err(format!("That edit isn't valid: {e}"))),
        }
    };
}

/// Turns "Arch package v12" into "arch-package-v12".
fn slug(name: &str) -> Result<Id, StoreError> {
    Id::parse(&name.to_lowercase().replace(' ', "-")).map_err(fault)
}

fn shared(project: &Id, team: &Id, item: &Id) -> Place {
    Place::Item {
        project: project.clone(),
        team: team.clone(),
        folder: folders::shared(),
        item: item.clone(),
    }
}

/// The folders' names.
pub mod folders {
    use kit_core::Id;

    fn known(name: &str) -> Id {
        // These are constants, and valid IDs.
        Id::parse(name).unwrap_or_else(|_| Id::from_time_and_random(0, [0; 10]))
    }

    #[must_use]
    pub fn wip() -> Id {
        known("wip")
    }
    #[must_use]
    pub fn shared() -> Id {
        known("shared")
    }
    #[must_use]
    pub fn consumed() -> Id {
        known("consumed")
    }
}

async fn add_model<T: Txn>(txn: &mut T, project: &Id, edit: &Edit) -> Result<Outcome, StoreError> {
    let args = args!(edit, AddModel);
    let place = Place::Model {
        project: project.clone(),
        model: edit.target.clone(),
    };
    if txn.get(&place.key()).await?.is_some() {
        return Ok(Err("That model already exists.".to_owned()));
    }
    let row = Model {
        id: edit.target.clone(),
        name: args.name,
        team: args.team,
        package: args.package,
    };
    txn.put(place.key(), to_row(&row)?).await?;
    let place = Place::Version {
        project: project.clone(),
        model: edit.target.clone(),
    };
    txn.put(place.key(), to_row(&args.last_version)?).await?;
    Ok(Ok(Applied::default()))
}

async fn publish<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
) -> Result<Outcome, StoreError> {
    let args = args!(edit, Publish);
    let model_id = &edit.target;
    let place = Place::Model {
        project: project.clone(),
        model: model_id.clone(),
    };
    let model: Option<Model> = read_row(txn, &place.key()).await?;
    let Some(model) = model else {
        return Ok(Err("There's no such model.".to_owned()));
    };
    if model.team != args.team {
        return Ok(Err("That model belongs to another team.".to_owned()));
    }
    let place = Place::Version {
        project: project.clone(),
        model: model_id.clone(),
    };
    let version = read_row::<u32, T>(txn, &place.key()).await?.unwrap_or(0) + 1;
    txn.put(place.key(), to_row(&version)?).await?;
    let name = format!("{} v{version}", model.package);
    let item = Item {
        id: slug(&name)?,
        name,
        version,
        model: model_id.clone(),
        seq: args.seq,
        file: args.file,
        sha256: args.sha256,
        elements: args.elements,
        by: edit.person.clone(),
        from: None,
        bridged_to: Vec::new(),
        entry: context.head + 1,
    };
    txn.put(shared(project, &args.team, &item.id).key(), to_row(&item)?)
        .await?;
    Ok(Ok(Applied::default()))
}

async fn bridge<T: Txn>(txn: &mut T, project: &Id, edit: &Edit) -> Result<Outcome, StoreError> {
    let args = args!(edit, Bridge);
    let item_key = shared(project, &args.team, &edit.target).key();
    let item: Option<Item> = read_row(txn, &item_key).await?;
    let Some(mut item) = item else {
        return Ok(Err("There's no such package.".to_owned()));
    };

    let target: Option<Project> =
        read_row(txn, &Place::Project(args.to_project.clone()).key()).await?;
    let Some(target) = target else {
        return Ok(Err("There's no such project.".to_owned()));
    };
    let team = Place::Team {
        project: args.to_project.clone(),
        team: args.to_team.clone(),
    };
    if txn.get(&team.key()).await?.is_none() {
        return Ok(Err("There's no such team.".to_owned()));
    }
    let from: Option<Project> = read_row(txn, &Place::Project(project.clone()).key()).await?;
    let from = from.ok_or_else(|| fault("project row missing"))?;
    let hub: Option<Hub> = read_row(txn, &Place::Hub(from.hub.clone()).key()).await?;
    let hub = hub.ok_or_else(|| fault("hub row missing"))?;
    let to_hub: Option<Hub> = read_row(txn, &Place::Hub(target.hub.clone()).key()).await?;
    let to_hub = to_hub.ok_or_else(|| fault("hub row missing"))?;

    let id = Id::parse(&format!("{}.{}", from.hub, item.id)).map_err(fault)?;
    let origin = Origin {
        hub: from.hub.clone(),
        hub_name: hub.name,
        project: project.clone(),
        team: args.team.clone(),
        package: item.id.clone(),
    };
    let incoming = Incoming {
        id: id.clone(),
        to_team: args.to_team,
        item: item.clone(),
        origin,
    };
    let place = Place::Incoming {
        project: args.to_project.clone(),
        id: id.clone(),
    };
    txn.put(place.key(), to_row(&incoming)?).await?;
    if !item.bridged_to.contains(&to_hub.name) {
        item.bridged_to.push(to_hub.name);
    }
    txn.put(item_key, to_row(&item)?).await?;
    let arrival = Elsewhere {
        scope: Scope::Project(args.to_project),
        target: id,
        cmd: "package.arrived".to_owned(),
        args: json!({ "from_project": project.as_str(), "package": item.id.as_str() }),
    };
    Ok(Ok(Applied {
        elsewhere: vec![arrival],
        ..Applied::default()
    }))
}

async fn take<T: Txn>(txn: &mut T, project: &Id, edit: &Edit) -> Result<Outcome, StoreError> {
    let args = args!(edit, Take);
    let key = Place::Incoming {
        project: project.clone(),
        id: edit.target.clone(),
    }
    .key();
    let incoming: Option<Incoming> = read_row(txn, &key).await?;
    let Some(incoming) = incoming else {
        return Ok(Err("That package isn't waiting.".to_owned()));
    };
    if incoming.to_team != args.team {
        return Ok(Err("That package was sent to another team.".to_owned()));
    }
    let item = Item {
        id: edit.target.clone(),
        from: Some(incoming.origin),
        ..incoming.item
    };
    let place = Place::Item {
        project: project.clone(),
        team: args.team,
        folder: folders::consumed(),
        item: item.id.clone(),
    };
    txn.put(place.key(), to_row(&item)?).await?;
    txn.delete(key).await?;
    Ok(Ok(Applied::default()))
}

async fn add_link<T: Txn>(txn: &mut T, project: &Id, edit: &Edit) -> Result<Outcome, StoreError> {
    let args = args!(edit, AddLink);
    let place = Place::Model {
        project: project.clone(),
        model: edit.target.clone(),
    };
    let host: Option<Model> = read_row(txn, &place.key()).await?;
    let Some(host) = host else {
        return Ok(Err("There's no such model.".to_owned()));
    };
    let link = match args.kind {
        LinkKind::Live => {
            let place = Place::Model {
                project: project.clone(),
                model: args.source.clone(),
            };
            let other: Option<Model> = read_row(txn, &place.key()).await?;
            let Some(other) = other.filter(|other| other.team == host.team && other.id != host.id)
            else {
                return Ok(Err(
                    "A live link needs another model in the same team's WIP.".to_owned(),
                ));
            };
            Link {
                id: Id::parse(&format!("live.{}", args.source)).map_err(fault)?,
                kind: args.kind,
                name: other.name,
                model: Some(args.source),
                team: None,
                package: None,
            }
        }
        LinkKind::Shared | LinkKind::Consumed => {
            let (folder, team) = if args.kind == LinkKind::Consumed {
                (folders::consumed(), host.team.clone())
            } else {
                let Some(team) = args.team else {
                    return Ok(Err("Which team?".to_owned()));
                };
                (folders::shared(), team)
            };
            let place = Place::Item {
                project: project.clone(),
                team: team.clone(),
                folder: folder.clone(),
                item: args.source.clone(),
            };
            let item: Option<Item> = read_row(txn, &place.key()).await?;
            let Some(item) = item else {
                return Ok(Err("There's no such package.".to_owned()));
            };
            Link {
                id: Id::parse(&format!("{folder}.{team}.{}", args.source)).map_err(fault)?,
                kind: args.kind,
                name: item.name,
                model: None,
                team: Some(team),
                package: Some(args.source),
            }
        }
    };
    let place = Place::Link {
        project: project.clone(),
        model: edit.target.clone(),
        link: link.id.clone(),
    };
    txn.put(place.key(), to_row(&link)?).await?;
    Ok(Ok(Applied::default()))
}

/// Runs a folder command.
pub async fn apply<T: Txn>(
    txn: &mut T,
    project: &Id,
    context: &Context<'_>,
    edit: &Edit,
) -> Result<Outcome, StoreError> {
    match edit.cmd.as_str() {
        "model.add" => add_model(txn, project, edit).await,
        "package.publish" => publish(txn, project, context, edit).await,
        "package.bridge" => bridge(txn, project, edit).await,
        "package.take" => take(txn, project, edit).await,
        "link.add" => add_link(txn, project, edit).await,
        other => Ok(Err(format!("Unknown command {other}."))),
    }
}
