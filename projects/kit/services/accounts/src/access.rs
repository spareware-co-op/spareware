//! Access checks. Rows in the store say who belongs where; Cedar decides from them on every
//! request. Each check reads the rows fresh, so taking someone off a project works at once. A row
//! that isn't there means no: the answer to an outsider is never an error.

use crate::rows::{Model, Project};
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request, Schema, ValidationMode,
    Validator,
};
use kit_core::Id;
use kit_store::key::list;
use kit_store::{Key, Kv, Place, Prefix, Reader, StoreError, read_row};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::str::FromStr;

const POLICY: &str = include_str!("../policy/access.cedar");
const SCHEMA: &str = include_str!("../policy/access.cedarschema");

pub struct Access<K: Kv> {
    kv: K,
    policies: PolicySet,
    schema: Schema,
    authorizer: Authorizer,
}

/// What a request acts on.
#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    Directory,
    Person(&'a Id),
    Hub(&'a Id),
    Project(&'a Id),
    Team(&'a Id, &'a Id),
    Folder(&'a Id, &'a Id, &'a Id),
    Model(&'a Id, &'a Id),
    Package(&'a Id, &'a Id, &'a Id, &'a Id),
}

fn fault(what: impl std::fmt::Display) -> StoreError {
    StoreError::Damaged(what.to_string())
}

fn uid(kind: &str, id: &str) -> Value {
    json!({ "type": format!("Spare::{kind}"), "id": id })
}

fn entity_ref(kind: &str, id: &str) -> Value {
    json!({ "__entity": uid(kind, id) })
}

fn entity_uid(kind: &str, id: &str) -> Result<EntityUid, StoreError> {
    // The ID's alphabet holds nothing that needs quoting.
    EntityUid::from_str(&format!("Spare::{kind}::\"{id}\"")).map_err(fault)
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

    async fn add_person<R: Reader>(
        &mut self,
        reader: &mut R,
        person: &Id,
    ) -> Result<(), StoreError> {
        let mut parents = Vec::new();
        for (key, _) in reader.scan(&list::memberships(person)).await? {
            match key.parse() {
                Some(Place::MemberOfHub { hub, .. }) => parents.push(uid("Hub", hub.as_str())),
                Some(Place::MemberOfProject { project, .. }) => {
                    parents.push(uid("Project", project.as_str()));
                }
                Some(Place::MemberOfTeam { project, team, .. }) => {
                    parents.push(uid("Team", &format!("{project}/{team}")));
                }
                _ => return Err(fault(format!("{key} isn't an access row"))),
            }
        }
        let admin = reader
            .get(&Place::Admin(person.clone()).key())
            .await?
            .is_some();
        self.add(
            "Person",
            person.as_str(),
            json!({ "admin": admin }),
            parents,
        );
        Ok(())
    }

    async fn add_hub<R: Reader>(&mut self, reader: &mut R, hub: &Id) -> Result<(), StoreError> {
        let prefix: Prefix = list::bridges(hub);
        let mut permits = Vec::new();
        for (key, _) in reader.scan(&prefix).await? {
            match key.parse() {
                Some(Place::Bridge { other, .. }) => {
                    permits.push(entity_ref("Hub", other.as_str()))
                }
                _ => return Err(fault(format!("{key} isn't a bridge row"))),
            }
        }
        self.add("Hub", hub.as_str(), json!({ "permits": permits }), vec![]);
        Ok(())
    }

    async fn add_project<R: Reader>(
        &mut self,
        reader: &mut R,
        project: &Id,
    ) -> Result<bool, StoreError> {
        let row: Option<Project> = read_row(reader, &Place::Project(project.clone()).key()).await?;
        let Some(row) = row else { return Ok(false) };
        self.add_hub(reader, &row.hub).await?;
        let attrs = json!({ "hub": entity_ref("Hub", row.hub.as_str()) });
        self.add("Project", project.as_str(), attrs, vec![]);
        Ok(true)
    }

    async fn add_team<R: Reader>(
        &mut self,
        reader: &mut R,
        project: &Id,
        team: &Id,
    ) -> Result<bool, StoreError> {
        if !self.add_project(reader, project).await? {
            return Ok(false);
        }
        let place = Place::Team {
            project: project.clone(),
            team: team.clone(),
        };
        if reader.get(&place.key()).await?.is_none() {
            return Ok(false);
        }
        let attrs = json!({ "project": entity_ref("Project", project.as_str()) });
        self.add("Team", &format!("{project}/{team}"), attrs, vec![]);
        Ok(true)
    }

    async fn add_folder<R: Reader>(
        &mut self,
        reader: &mut R,
        project: &Id,
        team: &Id,
        kind: &Id,
    ) -> Result<bool, StoreError> {
        if !self.add_team(reader, project, team).await? {
            return Ok(false);
        }
        let team_id = format!("{project}/{team}");
        let attrs = json!({ "team": entity_ref("Team", &team_id), "kind": kind.as_str() });
        self.add("Folder", &format!("{team_id}/{kind}"), attrs, vec![]);
        Ok(true)
    }
}

impl<K: Kv> Access<K> {
    /// Loads the rules, and refuses to start if they don't fit the schema.
    pub fn new(kv: K) -> Result<Self, StoreError> {
        let (schema, _warnings) = Schema::from_cedarschema_str(SCHEMA).map_err(fault)?;
        let policies = PolicySet::from_str(POLICY).map_err(fault)?;
        let check = Validator::new(schema.clone()).validate(&policies, ValidationMode::Strict);
        if !check.validation_passed() {
            return Err(fault(format!(
                "the access rules don't fit the schema: {check}"
            )));
        }
        Ok(Self {
            kv,
            policies,
            schema,
            authorizer: Authorizer::new(),
        })
    }

    /// Asks Cedar.
    pub async fn allowed(
        &self,
        person: &Id,
        action: &str,
        target: Target<'_>,
        to_hub: Option<&Id>,
    ) -> Result<bool, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let reader = &mut snapshot;
        let mut slice = Slice::default();
        slice.add_person(reader, person).await?;

        let (kind, id, found) = match target {
            Target::Directory => {
                slice.add("Directory", "all", json!({}), vec![]);
                ("Directory", "all".to_owned(), true)
            }
            Target::Person(other) => {
                slice.add_person(reader, other).await?;
                ("Person", other.to_string(), true)
            }
            Target::Hub(hub) => {
                slice.add_hub(reader, hub).await?;
                ("Hub", hub.to_string(), true)
            }
            Target::Project(project) => {
                let found = slice.add_project(reader, project).await?;
                ("Project", project.to_string(), found)
            }
            Target::Team(project, team) => {
                let found = slice.add_team(reader, project, team).await?;
                ("Team", format!("{project}/{team}"), found)
            }
            Target::Folder(project, team, folder) => {
                let found = slice.add_folder(reader, project, team, folder).await?;
                ("Folder", format!("{project}/{team}/{folder}"), found)
            }
            Target::Model(project, model) => {
                let place = Place::Model {
                    project: project.clone(),
                    model: model.clone(),
                };
                let key: Key = place.key();
                let row: Option<Model> = read_row(reader, &key).await?;
                let id = format!("{project}/{model}");
                match row {
                    Some(row) => {
                        let wip = Id::parse("wip").map_err(fault)?;
                        let found = slice.add_folder(reader, project, &row.team, &wip).await?;
                        let folder = format!("{project}/{}/wip", row.team);
                        let attrs = json!({ "folder": entity_ref("Folder", &folder) });
                        slice.add("Model", &id, attrs, vec![]);
                        ("Model", id, found)
                    }
                    None => ("Model", id, false),
                }
            }
            Target::Package(project, team, folder, item) => {
                let found = slice.add_folder(reader, project, team, folder).await?;
                let place = Place::Item {
                    project: project.clone(),
                    team: team.clone(),
                    folder: folder.clone(),
                    item: item.clone(),
                };
                let exists = reader.get(&place.key()).await?.is_some();
                let id = format!("{project}/{team}/{folder}/{item}");
                let folder_id = format!("{project}/{team}/{folder}");
                let attrs = json!({ "folder": entity_ref("Folder", &folder_id) });
                slice.add("Package", &id, attrs, vec![]);
                ("Package", id, found && exists)
            }
        };
        if !found {
            return Ok(false);
        }

        let mut context = json!({});
        if let Some(hub) = to_hub {
            slice.add_hub(reader, hub).await?;
            context = json!({ "to_hub": entity_ref("Hub", hub.as_str()) });
        }

        let action_uid =
            EntityUid::from_str(&format!("Spare::Action::\"{action}\"")).map_err(fault)?;
        let context =
            Context::from_json_value(context, Some((&self.schema, &action_uid))).map_err(fault)?;
        let request = Request::new(
            entity_uid("Person", person.as_str())?,
            action_uid,
            entity_uid(kind, &id)?,
            context,
            Some(&self.schema),
        )
        .map_err(fault)?;
        let entities = Value::Array(slice.entities.into_values().collect());
        let entities = Entities::from_json_value(entities, Some(&self.schema)).map_err(fault)?;
        let answer = self
            .authorizer
            .is_authorized(&request, &self.policies, &entities);
        Ok(answer.decision() == Decision::Allow)
    }
}
