//! Access checks. Rows in TiKV say who belongs where; Cedar decides from them on every request.
//! Each check reads the rows fresh, so taking someone off a project works at once.

use crate::keys;
use crate::kv::{self, Kv};
use crate::rows::{Model, Project};
use anyhow::{Context as _, Result, anyhow, bail};
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request, Schema, ValidationMode,
    Validator,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::str::FromStr;
use tikv_client::Snapshot;

const POLICY: &str = include_str!("../policy/access.cedar");
const SCHEMA: &str = include_str!("../policy/access.cedarschema");

pub struct Access {
    kv: Kv,
    policies: PolicySet,
    schema: Schema,
    authorizer: Authorizer,
}

/// What a request acts on.
#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    Hub(&'a str),
    Project(&'a str),
    Team(&'a str, &'a str),
    Folder(&'a str, &'a str, &'a str),
    Model(&'a str, &'a str),
    Package(&'a str, &'a str, &'a str, &'a str),
}

fn uid(kind: &str, id: &str) -> Value {
    json!({ "type": format!("Spare::{kind}"), "id": id })
}

fn entity_ref(kind: &str, id: &str) -> Value {
    json!({ "__entity": uid(kind, id) })
}

fn entity_uid(kind: &str, id: &str) -> Result<EntityUid> {
    let quoted = serde_json::to_string(id)?;
    EntityUid::from_str(&format!("Spare::{kind}::{quoted}")).map_err(|e| anyhow!("{e}"))
}

/// The entities one request needs, collected as Cedar's JSON.
#[derive(Default)]
struct Slice {
    entities: BTreeMap<(String, String), Value>,
}

impl Slice {
    fn add(&mut self, kind: &str, id: &str, attrs: Value, parents: Vec<Value>) {
        let entity = json!({ "uid": uid(kind, id), "attrs": attrs, "parents": parents });
        self.entities
            .insert((kind.to_owned(), id.to_owned()), entity);
    }

    async fn add_person(&mut self, snap: &mut Snapshot, person: &str) -> Result<()> {
        let prefix = keys::acl_person_prefix(person);
        let mut parents = Vec::new();
        for (key, _) in kv::scan(snap, &prefix).await? {
            let tail = key.strip_prefix(&prefix).context("acl key")?;
            match tail.split_once('/') {
                Some(("hub", hub)) => parents.push(uid("Hub", hub)),
                Some(("project", project)) => parents.push(uid("Project", project)),
                Some(("team", team)) => parents.push(uid("Team", team)),
                _ => bail!("unknown access row {key}"),
            }
        }
        self.add("Person", person, json!({}), parents);
        Ok(())
    }

    async fn add_hub(&mut self, snap: &mut Snapshot, hub: &str) -> Result<()> {
        let prefix = keys::acl_bridge_prefix(hub);
        let mut permits = Vec::new();
        for (key, _) in kv::scan(snap, &prefix).await? {
            permits.push(entity_ref(
                "Hub",
                key.strip_prefix(&prefix).context("acl key")?,
            ));
        }
        self.add("Hub", hub, json!({ "permits": permits }), vec![]);
        Ok(())
    }

    async fn add_project(&mut self, snap: &mut Snapshot, project: &str) -> Result<bool> {
        let row: Option<Project> = kv::get(snap, keys::project(project)).await?;
        let Some(row) = row else { return Ok(false) };
        self.add_hub(snap, &row.hub).await?;
        self.add(
            "Project",
            project,
            json!({ "hub": entity_ref("Hub", &row.hub) }),
            vec![],
        );
        Ok(true)
    }

    async fn add_team(&mut self, snap: &mut Snapshot, project: &str, team: &str) -> Result<bool> {
        if !self.add_project(snap, project).await? {
            return Ok(false);
        }
        let row: Option<Value> = kv::get(snap, keys::team(project, team)).await?;
        if row.is_none() {
            return Ok(false);
        }
        let id = format!("{project}/{team}");
        self.add(
            "Team",
            &id,
            json!({ "project": entity_ref("Project", project) }),
            vec![],
        );
        Ok(true)
    }

    async fn add_folder(
        &mut self,
        snap: &mut Snapshot,
        project: &str,
        team: &str,
        kind: &str,
    ) -> Result<bool> {
        if !self.add_team(snap, project, team).await? {
            return Ok(false);
        }
        let team_id = format!("{project}/{team}");
        let id = format!("{team_id}/{kind}");
        let attrs = json!({ "team": entity_ref("Team", &team_id), "kind": kind });
        self.add("Folder", &id, attrs, vec![]);
        Ok(true)
    }
}

impl Access {
    /// Loads the rules, and refuses to start if they don't fit the schema.
    pub fn new(kv: Kv) -> Result<Self> {
        let (schema, warnings) =
            Schema::from_cedarschema_str(SCHEMA).map_err(|e| anyhow!("schema: {e}"))?;
        for warning in warnings {
            tracing::warn!("schema: {warning}");
        }
        let policies = PolicySet::from_str(POLICY).map_err(|e| anyhow!("policy: {e}"))?;
        let check = Validator::new(schema.clone()).validate(&policies, ValidationMode::Strict);
        if !check.validation_passed() {
            bail!("policy does not fit the schema: {check}");
        }
        Ok(Self {
            kv,
            policies,
            schema,
            authorizer: Authorizer::new(),
        })
    }

    /// Asks Cedar. A row that isn't there means no: the answer is never an error for an outsider.
    pub async fn allowed(
        &self,
        person: &str,
        action: &str,
        target: Target<'_>,
        to_hub: Option<&str>,
    ) -> Result<bool> {
        let mut snap = self.kv.snapshot().await?;
        let mut slice = Slice::default();
        slice.add_person(&mut snap, person).await?;

        let (kind, id, found) = match target {
            Target::Hub(hub) => {
                slice.add_hub(&mut snap, hub).await?;
                ("Hub", hub.to_owned(), true)
            }
            Target::Project(project) => {
                let found = slice.add_project(&mut snap, project).await?;
                ("Project", project.to_owned(), found)
            }
            Target::Team(project, team) => {
                let found = slice.add_team(&mut snap, project, team).await?;
                ("Team", format!("{project}/{team}"), found)
            }
            Target::Folder(project, team, folder) => {
                let found = slice.add_folder(&mut snap, project, team, folder).await?;
                ("Folder", format!("{project}/{team}/{folder}"), found)
            }
            Target::Model(project, model) => {
                let row: Option<Model> = kv::get(&mut snap, keys::model(project, model)).await?;
                let id = format!("{project}/{model}");
                match row {
                    Some(row) => {
                        let found = slice
                            .add_folder(&mut snap, project, &row.team, "wip")
                            .await?;
                        let folder = format!("{project}/{}/wip", row.team);
                        let attrs = json!({ "folder": entity_ref("Folder", &folder) });
                        slice.add("Model", &id, attrs, vec![]);
                        ("Model", id, found)
                    }
                    None => ("Model", id, false),
                }
            }
            Target::Package(project, team, folder, item) => {
                let found = slice.add_folder(&mut snap, project, team, folder).await?;
                let row: Option<Value> =
                    kv::get(&mut snap, keys::item(project, team, folder, item)).await?;
                let id = format!("{project}/{team}/{folder}/{item}");
                let folder_id = format!("{project}/{team}/{folder}");
                let attrs = json!({ "folder": entity_ref("Folder", &folder_id) });
                slice.add("Package", &id, attrs, vec![]);
                ("Package", id, found && row.is_some())
            }
        };
        if !found {
            return Ok(false);
        }

        let mut context = json!({});
        if let Some(hub) = to_hub {
            slice.add_hub(&mut snap, hub).await?;
            context = json!({ "to_hub": entity_ref("Hub", hub) });
        }

        let action_uid: EntityUid = EntityUid::from_str(&format!("Spare::Action::\"{action}\""))
            .map_err(|e| anyhow!("{e}"))?;
        let context = Context::from_json_value(context, Some((&self.schema, &action_uid)))
            .map_err(|e| anyhow!("context: {e}"))?;
        let request = Request::new(
            entity_uid("Person", person)?,
            action_uid,
            entity_uid(kind, &id)?,
            context,
            Some(&self.schema),
        )
        .map_err(|e| anyhow!("request: {e}"))?;
        let entities = Value::Array(slice.entities.into_values().collect());
        let entities = Entities::from_json_value(entities, Some(&self.schema))
            .map_err(|e| anyhow!("entities: {e}"))?;
        let answer = self
            .authorizer
            .is_authorized(&request, &self.policies, &entities);
        Ok(answer.decision() == Decision::Allow)
    }
}
