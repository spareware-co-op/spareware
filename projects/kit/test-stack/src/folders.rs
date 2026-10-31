//! Packages and links: publishing to Shared, bridging to another hub, taking into Consumed, and
//! the three ways one model links another. A package's bytes are a file in the object store; the
//! rows only say where it is and what its hash must be.

use crate::access::Target;
use crate::api::{ApiError, Shared};
use crate::keys;
use crate::kv;
use crate::rows::{
    Element, Hub, Incoming, Item, Link, LinkKind, Model, Origin, Project, Shape, sha256_hex,
};
use anyhow::{Context as _, Result};
use object_store::ObjectStoreExt;
use object_store::path::Path;
use serde_json::{Value, json};

const ATTEMPTS: u32 = 8;

/// Turns "Arch package v12" into "arch-package-v12".
fn slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

/// The whole model at this moment, as one file.
async fn package_bytes(st: &Shared, project: &str, model: &Model) -> Result<(Vec<u8>, u64, usize)> {
    let mut snap = st.kv.snapshot().await?;
    let seq: u64 = kv::get(&mut snap, keys::head(project)).await?.unwrap_or(0);
    let mut elements = Vec::new();
    for (_, hash) in kv::scan(&mut snap, &keys::elem_prefix(project, &model.id)).await? {
        let hash: String = serde_json::from_slice(&hash)?;
        let element: Element = kv::get(&mut snap, keys::record(project, &hash))
            .await?
            .context("record missing")?;
        let shape: Shape = kv::get(&mut snap, keys::shape(project, &element.shape))
            .await?
            .context("shape missing")?;
        elements.push(json!({ "element": element, "shape": shape }));
    }
    let count = elements.len();
    let file = json!({ "project": project, "model": model.id, "seq": seq, "elements": elements });
    Ok((serde_json::to_vec(&file)?, seq, count))
}

/// Publishes a team's model as the next numbered package in its Shared folder.
pub async fn publish(
    st: &Shared,
    person: &str,
    project: &str,
    team: &str,
    model_id: &str,
) -> Result<Item, ApiError> {
    st.require(person, "package.publish", Target::Team(project, team))
        .await?;
    st.require(person, "model.read", Target::Model(project, model_id))
        .await?;
    let model: Model = st
        .kv
        .get(keys::model(project, model_id))
        .await?
        .ok_or(ApiError::NotFound)?;
    if model.team != team {
        return Err(ApiError::Bad(
            "That model belongs to another team.".to_owned(),
        ));
    }

    let (bytes, seq, elements) = package_bytes(st, project, &model).await?;
    let sha256 = sha256_hex(&bytes);
    let mut last = None;
    for _ in 0..ATTEMPTS {
        let mut txn = st.kv.begin().await?;
        let key = keys::version(project, model_id);
        let version: u32 = match txn.get(key.clone()).await? {
            Some(bytes) => serde_json::from_slice::<u32>(&bytes).map_err(anyhow::Error::from)? + 1,
            None => 1,
        };
        let name = format!("{} v{version}", model.package);
        let id = slug(&name);
        let file = format!("{project}/{team}/{id}.json");
        st.files
            .put(&Path::from(file.as_str()), bytes.clone().into())
            .await
            .map_err(anyhow::Error::from)?;
        let item = Item {
            id: id.clone(),
            name,
            version,
            model: model_id.to_owned(),
            seq,
            file,
            sha256: sha256.clone(),
            elements,
            by: person.to_owned(),
            from: None,
            bridged_to: Vec::new(),
        };
        txn.put(
            key,
            serde_json::to_vec(&version).map_err(anyhow::Error::from)?,
        )
        .await
        .map_err(anyhow::Error::from)?;
        let row = serde_json::to_vec(&item).map_err(anyhow::Error::from)?;
        txn.put(keys::item(project, team, "shared", &id), row)
            .await
            .map_err(anyhow::Error::from)?;
        match txn.commit().await {
            Ok(_) => return Ok(item),
            Err(e) => last = Some(e),
        }
    }
    Err(ApiError::Internal(last.map_or_else(
        || anyhow::anyhow!("publish failed"),
        Into::into,
    )))
}

/// Sends a shared package to another hub's project. Cedar checks that both hubs permit it.
pub async fn bridge(
    st: &Shared,
    person: &str,
    project: &str,
    team: &str,
    item_id: &str,
    to_project: &str,
    to_team: &str,
) -> Result<Incoming, ApiError> {
    let target: Project = st
        .kv
        .get(keys::project(to_project))
        .await?
        .ok_or(ApiError::NotFound)?;
    let package = Target::Package(project, team, "shared", item_id);
    if !st
        .access
        .allowed(person, "package.bridge", package, Some(&target.hub))
        .await?
    {
        return Err(ApiError::Refused(
            "Both hubs must permit this bridge.".to_owned(),
        ));
    }
    let to_row: Option<Value> = st.kv.get(keys::team(to_project, to_team)).await?;
    if to_row.is_none() {
        return Err(ApiError::NotFound);
    }
    let from: Project = st
        .kv
        .get(keys::project(project))
        .await?
        .ok_or(ApiError::NotFound)?;
    let hub: Hub = st
        .kv
        .get(keys::hub(&from.hub))
        .await?
        .ok_or(ApiError::NotFound)?;
    let to_hub: Hub = st
        .kv
        .get(keys::hub(&target.hub))
        .await?
        .ok_or(ApiError::NotFound)?;
    let item_key = keys::item(project, team, "shared", item_id);
    let mut item: Item = st
        .kv
        .get(item_key.clone())
        .await?
        .ok_or(ApiError::NotFound)?;

    let origin = Origin {
        hub: from.hub.clone(),
        hub_name: hub.name,
        project: project.to_owned(),
        team: team.to_owned(),
        package: item_id.to_owned(),
    };
    let id = format!("{}.{}", from.hub, item_id);
    let incoming = Incoming {
        id: id.clone(),
        to_team: to_team.to_owned(),
        item: item.clone(),
        origin,
    };
    if !item.bridged_to.contains(&to_hub.name) {
        item.bridged_to.push(to_hub.name);
    }
    let mut txn = st.kv.begin().await?;
    let row = serde_json::to_vec(&incoming).map_err(anyhow::Error::from)?;
    txn.put(keys::incoming(to_project, &id), row)
        .await
        .map_err(anyhow::Error::from)?;
    let row = serde_json::to_vec(&item).map_err(anyhow::Error::from)?;
    txn.put(item_key, row).await.map_err(anyhow::Error::from)?;
    txn.commit().await.map_err(anyhow::Error::from)?;
    Ok(incoming)
}

/// Takes a bridged package into the team's Consumed folder, after checking the file against the
/// hash it was published with.
pub async fn consume(
    st: &Shared,
    person: &str,
    project: &str,
    team: &str,
    incoming_id: &str,
) -> Result<Item, ApiError> {
    st.require(person, "package.consume", Target::Team(project, team))
        .await?;
    let key = keys::incoming(project, incoming_id);
    let incoming: Incoming = st.kv.get(key.clone()).await?.ok_or(ApiError::NotFound)?;
    if incoming.to_team != team {
        return Err(ApiError::Refused(
            "That package was sent to another team.".to_owned(),
        ));
    }
    let path = Path::from(incoming.item.file.as_str());
    let file = st.files.get(&path).await.map_err(anyhow::Error::from)?;
    let bytes = file.bytes().await.map_err(anyhow::Error::from)?;
    if sha256_hex(&bytes) != incoming.item.sha256 {
        return Err(ApiError::Bad(
            "That package's file doesn't match its hash.".to_owned(),
        ));
    }
    let item = Item {
        id: incoming_id.to_owned(),
        from: Some(incoming.origin),
        ..incoming.item
    };
    let mut txn = st.kv.begin().await?;
    let row = serde_json::to_vec(&item).map_err(anyhow::Error::from)?;
    txn.put(keys::item(project, team, "consumed", &item.id), row)
        .await
        .map_err(anyhow::Error::from)?;
    txn.delete(key).await.map_err(anyhow::Error::from)?;
    txn.commit().await.map_err(anyhow::Error::from)?;
    Ok(item)
}

/// Links one model into another: live from the same team's WIP, or a fixed package from a Shared
/// or the team's Consumed folder.
pub async fn add_link(
    st: &Shared,
    person: &str,
    project: &str,
    model_id: &str,
    kind: LinkKind,
    team: Option<&str>,
    source: &str,
) -> Result<Link, ApiError> {
    st.require(person, "model.link", Target::Model(project, model_id))
        .await?;
    let host: Model = st
        .kv
        .get(keys::model(project, model_id))
        .await?
        .ok_or(ApiError::NotFound)?;
    let link = match kind {
        LinkKind::Live => {
            let other: Model = st
                .kv
                .get(keys::model(project, source))
                .await?
                .ok_or(ApiError::NotFound)?;
            if other.team != host.team || other.id == host.id {
                let reason = "A live link needs another model in the same team's WIP.";
                return Err(ApiError::Refused(reason.to_owned()));
            }
            st.require(person, "model.read", Target::Model(project, source))
                .await?;
            Link {
                id: format!("live.{source}"),
                kind,
                name: other.name,
                model: Some(source.to_owned()),
                team: None,
                package: None,
            }
        }
        LinkKind::Shared | LinkKind::Consumed => {
            let (folder, team) = match kind {
                LinkKind::Consumed => ("consumed", host.team.as_str()),
                _ => ("shared", team.ok_or(ApiError::NotFound)?),
            };
            st.require(
                person,
                "package.read",
                Target::Package(project, team, folder, source),
            )
            .await?;
            let item: Item = st
                .kv
                .get(keys::item(project, team, folder, source))
                .await?
                .ok_or(ApiError::NotFound)?;
            Link {
                id: format!("{folder}.{team}.{source}"),
                kind,
                name: item.name,
                model: None,
                team: Some(team.to_owned()),
                package: Some(source.to_owned()),
            }
        }
    };
    st.kv
        .put(keys::link(project, model_id, &link.id), &link)
        .await?;
    Ok(link)
}

/// Hubs this project's hub can bridge to: both sides must permit it, as Cedar will check again
/// when a bridge is made. Lists their projects and teams to send to.
pub async fn bridge_targets(st: &Shared, project: &str) -> Result<Value> {
    let row: Project = st
        .kv
        .get(keys::project(project))
        .await?
        .context("project row")?;
    let prefix = keys::acl_bridge_prefix(&row.hub);
    let mut found = Vec::new();
    for (key, _) in st.kv.scan(&prefix).await? {
        let other = key.strip_prefix(&prefix).context("acl key")?;
        let hub: Option<Hub> = st.kv.get(keys::hub(other)).await?;
        let Some(hub) = hub else { continue };
        let back: Option<Value> = st.kv.get(keys::acl_bridge(other, &row.hub)).await?;
        for (key, _) in st.kv.scan(&keys::hub_project_prefix(other)).await? {
            let id = key
                .strip_prefix(&keys::hub_project_prefix(other))
                .context("hub key")?;
            let target: Option<Project> = st.kv.get(keys::project(id)).await?;
            let Some(target) = target else { continue };
            let teams: Vec<crate::rows::Team> = st.kv.scan_rows(&keys::team_prefix(id)).await?;
            found.push(json!({
                "hub": hub.id,
                "hub_name": hub.name,
                "project": target.id,
                "project_name": target.name,
                "teams": teams,
                "permitted": back.is_some(),
            }));
        }
    }
    Ok(Value::Array(found))
}
