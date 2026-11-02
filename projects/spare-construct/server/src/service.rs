//! Everything a person can do, each step behind Cedar. The web layer holds this and nothing else:
//! the store is private to this file, so there is no way for a handler to read a row without the
//! check, and no write here but through an orderer. Generic over the store, so the same rules run
//! in-memory in `tests/service.rs`.

use crate::directory::DirectoryCommands;
use crate::shapes::{self, fresh_client};
use construct_api::cloud::spaces::folders;
use construct_api::cloud::{self, ProjectCommands, Target};
use construct_model::element::Element;
use construct_model::geometry::{Display, Edition, Geometry};
use construct_model::keys::{Place, list};
use construct_model::rows::{
    BuiltElement, BuiltResult, Hub, HubRole, Incoming, Item, Link, LinkKind, Model, Project,
    Stored, Team,
};
use construct_regen::regenerator::{Regenerator, cells_between};
use kit_accounts::rows::Person;
use kit_accounts::{Accounts, Cedar};
use kit_core::Id;
use kit_ops::{Edit, Entry, sha256_hex};
use kit_store::key::list as kit_list;
use kit_store::{
    FileStore, Kv, Place as KitPlace, Reader, Scope, StoreError, from_row, read_row, scan_rows,
};
use kit_sync::orderer::ClientMark;
use kit_sync::{Clock, Orderer, Orderers, Reply, read_log};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Cedar, answering Spare Construct's questions.
pub type Access<K> = cloud::Access<K, Cedar>;

/// How wide a view may be, so one request can't ask for a million cells.
const WIDEST_VIEW_MM: i64 = 500_000;

#[derive(Debug, Error)]
pub enum Refusal {
    #[error("Please sign in.")]
    SignIn,
    #[error("{0}")]
    Refused(String),
    #[error("Not found.")]
    NotFound,
    #[error("{0}")]
    Bad(String),
    #[error(transparent)]
    Failed(#[from] StoreError),
}

const NO_ACCESS: &str = "You don't have access to this.";

/// The whole model at one moment, as a package file holds it. Finished shapes travel with it: a
/// host draws them and never rebuilds them, so it doesn't need this model's edition.
#[derive(Serialize)]
struct PackageFile {
    project: String,
    document: String,
    edition: Edition,
    seq: u64,
    elements: Vec<Value>,
}

/// The pieces in view. `through` is how far regeneration had got when these were read, which may
/// be behind the log: the browser asks for what came after it, so it misses nothing.
#[derive(Serialize, Debug)]
pub struct AreaView {
    pub through: u64,
    pub pieces: BTreeMap<String, String>,
}

/// A stored shape, for someone who can't build this document's edition themselves.
#[derive(Serialize, Debug)]
pub struct ShapeView {
    pub built: BuiltElement,
    pub display: Option<Display>,
}

/// An area of a level.
pub struct View {
    pub level: i32,
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

pub struct Service<K: Kv, F: FileStore> {
    kv: K,
    pub access: Arc<Access<K>>,
    pub accounts: Accounts<K, DirectoryCommands<K>>,
    orderers: Arc<Orderers<K, ProjectCommands<K, Cedar>>>,
    clock: Arc<dyn Clock>,
    files: F,
    regen: Arc<Regenerator<K, F>>,
    regenerating: Mutex<HashSet<Id>>,
}

impl<K: Kv, F: FileStore> Service<K, F> {
    pub fn new(
        kv: K,
        access: Arc<Access<K>>,
        accounts: Accounts<K, DirectoryCommands<K>>,
        orderers: Orderers<K, ProjectCommands<K, Cedar>>,
        clock: Arc<dyn Clock>,
        files: F,
        geometry: Arc<dyn Geometry>,
    ) -> Self {
        let regen = Regenerator::new(kv.clone(), files.clone(), geometry);
        Self {
            kv,
            access,
            accounts,
            orderers: Arc::new(orderers),
            clock,
            files,
            regen: Arc::new(regen),
            regenerating: Mutex::new(HashSet::new()),
        }
    }

    /// Starts this project's shapes keeping up with its log, the first time it is used.
    fn keep_up(&self, project: &Id) {
        let mut running = self
            .regenerating
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if running.insert(project.clone()) {
            let (regen, orderers) = (self.regen.clone(), self.orderers.clone());
            shapes::keep_up(regen, orderers, self.clock.clone(), project.clone());
        }
    }

    async fn require(&self, person: &Id, action: &str, target: Target<'_>) -> Result<(), Refusal> {
        if self.access.allowed(person, action, target, None).await? {
            Ok(())
        } else {
            Err(Refusal::Refused(NO_ACCESS.to_owned()))
        }
    }

    /// Runs one edit from the server itself: it gets a client of its own, and an answer.
    pub async fn edit(
        &self,
        person: &Id,
        project: &Id,
        target: &Id,
        cmd: &str,
        args: Value,
    ) -> Result<u64, Refusal> {
        let now = self.clock.now_ms();
        let edit = Edit {
            person: person.clone(),
            client: fresh_client(now),
            counter: 1,
            base: 0,
            target: target.clone(),
            cmd: cmd.to_owned(),
            args,
            author_ms: now,
        };
        self.keep_up(project);
        let scope = Scope::Project(project.clone());
        loop {
            match self.orderers.run(&scope, edit.clone()).await? {
                Reply::Ack { seq, .. } => return Ok(seq),
                Reply::Rejected { reason, .. } => return Err(Refusal::Refused(reason)),
                Reply::Retry { .. } => {}
            }
        }
    }

    // --- Who and where ---

    /// A person's own row and hubs, home first. Everyone may read their own.
    pub async fn me(&self, person: &Id) -> Result<Value, Refusal> {
        let mut snapshot = self.kv.snapshot().await?;
        let row: Option<Person> =
            read_row(&mut snapshot, &KitPlace::Person(person.clone()).key()).await?;
        let row = row.ok_or(Refusal::SignIn)?;
        let home: Option<String> =
            read_row(&mut snapshot, &Place::Home(person.clone()).key()).await?;
        let mut hubs = Vec::new();
        let prefix = list::hubs_of(person);
        for (key, role) in snapshot.scan(&prefix).await? {
            let Some(Place::MemberOfHub { hub, .. }) = Place::parse(&key) else {
                continue;
            };
            let role: HubRole = from_row(&role)?;
            let hub_row: Option<Hub> = read_row(&mut snapshot, &Place::Hub(hub).key()).await?;
            if let Some(hub_row) = hub_row {
                hubs.push(json!({ "id": hub_row.id.as_str(), "name": hub_row.name, "role": role }));
            }
        }
        hubs.sort_by_key(|hub| hub["id"].as_str() != home.as_deref());
        Ok(json!({ "person": {
            "id": row.id.as_str(), "name": row.name, "home_hub": home,
        }, "hubs": hubs }))
    }

    /// A hub's people, and the projects this person is on.
    pub async fn hub(&self, person: &Id, id: &Id) -> Result<Value, Refusal> {
        self.require(person, "hub.open", Target::Hub(id)).await?;
        let mut snapshot = self.kv.snapshot().await?;
        let row: Option<Hub> = read_row(&mut snapshot, &Place::Hub(id.clone()).key()).await?;
        let row = row.ok_or(Refusal::NotFound)?;
        let (mut users, mut externals) = (Vec::new(), Vec::new());
        for (key, role) in snapshot.scan(&list::hub_users(id)).await? {
            let Some(Place::HubUser { person: member, .. }) = Place::parse(&key) else {
                continue;
            };
            let role: HubRole = from_row(&role)?;
            let member: Option<Person> =
                read_row(&mut snapshot, &KitPlace::Person(member).key()).await?;
            let Some(member) = member else { continue };
            match role {
                HubRole::User => users.push(member.name),
                HubRole::External => externals.push(member.name),
            }
        }
        let mut projects = Vec::new();
        for (key, _) in snapshot.scan(&list::hub_projects(id)).await? {
            let Some(Place::HubProject { project, .. }) = Place::parse(&key) else {
                continue;
            };
            if self
                .access
                .allowed(person, "project.open", Target::Project(&project), None)
                .await?
            {
                let row: Option<Project> =
                    read_row(&mut snapshot, &Place::Project(project).key()).await?;
                projects.extend(row);
            }
        }
        Ok(json!({ "hub": row, "users": users, "externals": externals, "projects": projects }))
    }

    /// A project's teams, with only the folders this person may open.
    pub async fn project(&self, person: &Id, id: &Id) -> Result<Value, Refusal> {
        self.require(person, "project.open", Target::Project(id))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let row: Option<Project> =
            read_row(&mut snapshot, &Place::Project(id.clone()).key()).await?;
        let row = row.ok_or(Refusal::NotFound)?;
        let hub: Option<Hub> = read_row(&mut snapshot, &Place::Hub(row.hub.clone()).key()).await?;
        let hub = hub.ok_or(Refusal::NotFound)?;
        let scope = Scope::Project(id.clone());
        let head: u64 = read_row(&mut snapshot, &scope.head()).await?.unwrap_or(0);
        let models: Vec<Model> = scan_rows(&mut snapshot, &list::models(id)).await?;
        let teams: Vec<Team> = scan_rows(&mut snapshot, &list::teams(id)).await?;
        let mine: Vec<Id> = snapshot
            .scan(&list::teams_of(person, id))
            .await?
            .into_iter()
            .filter_map(|(key, _)| match Place::parse(&key) {
                Some(Place::MemberOfTeam { team, .. }) => Some(team),
                _ => None,
            })
            .collect();

        let mut listed = Vec::new();
        for team in teams {
            let mut folders_seen = Vec::new();
            for kind in [folders::wip(), folders::shared(), folders::consumed()] {
                let folder = Target::Folder(id, &team.id, &kind);
                if !self
                    .access
                    .allowed(person, "folder.open", folder, None)
                    .await?
                {
                    continue;
                }
                if kind == folders::wip() {
                    let in_team: Vec<&Model> =
                        models.iter().filter(|m| m.team == team.id).collect();
                    folders_seen.push(json!({ "kind": kind.as_str(), "models": in_team }));
                } else {
                    let prefix = list::items(id, &team.id, &kind);
                    let items: Vec<Item> = scan_rows(&mut snapshot, &prefix).await?;
                    folders_seen.push(json!({ "kind": kind.as_str(), "packages": items }));
                }
            }
            listed.push(json!({
                "id": team.id.as_str(),
                "name": team.name,
                "member": mine.contains(&team.id),
                "folders": folders_seen,
            }));
        }
        let waiting: Vec<Incoming> = scan_rows(&mut snapshot, &list::incoming(id)).await?;
        let waiting: Vec<Incoming> = waiting
            .into_iter()
            .filter(|i| mine.contains(&i.to_team))
            .collect();
        Ok(json!({
            "project": row, "hub": hub, "head": head, "teams": listed, "incoming": waiting,
        }))
    }

    /// Where this project could bridge to. Lists hubs this one permits, saying whether they
    /// permit back. Cedar checks again when a bridge is made.
    pub async fn bridge_targets(&self, person: &Id, id: &Id) -> Result<Value, Refusal> {
        self.require(person, "project.open", Target::Project(id))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let row: Option<Project> =
            read_row(&mut snapshot, &Place::Project(id.clone()).key()).await?;
        let row = row.ok_or(Refusal::NotFound)?;
        let mut found = Vec::new();
        for (key, _) in snapshot.scan(&list::bridges(&row.hub)).await? {
            let Some(Place::Bridge { other, .. }) = Place::parse(&key) else {
                continue;
            };
            let hub: Option<Hub> =
                read_row(&mut snapshot, &Place::Hub(other.clone()).key()).await?;
            let Some(hub) = hub else { continue };
            let back = Place::Bridge {
                hub: other.clone(),
                other: row.hub.clone(),
            };
            let permitted = snapshot.get(&back.key()).await?.is_some();
            for (key, _) in snapshot.scan(&list::hub_projects(&other)).await? {
                let Some(Place::HubProject { project, .. }) = Place::parse(&key) else {
                    continue;
                };
                let target: Option<Project> =
                    read_row(&mut snapshot, &Place::Project(project.clone()).key()).await?;
                let Some(target) = target else { continue };
                let teams: Vec<Team> = scan_rows(&mut snapshot, &list::teams(&project)).await?;
                found.push(json!({
                    "hub": hub.id.as_str(),
                    "hub_name": hub.name,
                    "project": target.id.as_str(),
                    "project_name": target.name,
                    "teams": teams,
                    "permitted": permitted,
                }));
            }
        }
        Ok(Value::Array(found))
    }

    // --- Reading a model ---

    /// The pieces in view: element IDs and record hashes only. The browser fetches each record it
    /// hasn't already got, and its own cache keeps them by hash.
    pub async fn area(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &View,
    ) -> Result<AreaView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        if (view.x1 - view.x0).abs() > WIDEST_VIEW_MM || (view.y1 - view.y0).abs() > WIDEST_VIEW_MM
        {
            return Err(Refusal::Bad("That view is too big.".to_owned()));
        }
        let mut snapshot = self.kv.snapshot().await?;
        let mut pieces = BTreeMap::new();
        for (x, y) in cells_between(view.x0, view.y0, view.x1, view.y1) {
            let prefix = list::cell(project, model, view.level, x, y);
            for (key, hash) in snapshot.scan(&prefix).await? {
                let Some(Place::Area { element, .. }) = Place::parse(&key) else {
                    continue;
                };
                pieces.insert(element.to_string(), from_row::<String>(&hash)?);
            }
        }
        let cursor = Place::Regenerated {
            project: project.clone(),
        };
        let through: u64 = read_row(&mut snapshot, &cursor.key()).await?.unwrap_or(0);
        Ok(AreaView { through, pieces })
    }

    pub async fn claims(&self, person: &Id, project: &Id, model: &Id) -> Result<Value, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let mut found = serde_json::Map::new();
        for (key, value) in snapshot.scan(&kit_list::claims(project, model)).await? {
            let Some(KitPlace::Claim { piece, .. }) = key.parse() else {
                continue;
            };
            found.insert(piece.to_string(), from_row(&value)?);
        }
        Ok(Value::Object(found))
    }

    pub async fn record(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        hash: &Id,
    ) -> Result<Element, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let place = KitPlace::Record {
            project: project.clone(),
            hash: hash.clone(),
        };
        let row: Option<Element> = read_row(&mut snapshot, &place.key()).await?;
        // A reader of one model must not reach another model's records by naming a hash.
        row.filter(|row| row.document == *model)
            .ok_or(Refusal::NotFound)
    }

    /// An element's stored shape at a level of detail, or its coarsest. By element, not by hash,
    /// so a reader of one model can't reach another's shapes by naming a content ID.
    pub async fn shape(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        element: &Id,
        lod: Option<u32>,
    ) -> Result<ShapeView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        self.stored_shape(&mut snapshot, project, model, element, lod)
            .await
    }

    async fn stored_shape(
        &self,
        snapshot: &mut K::Snapshot,
        project: &Id,
        model: &Id,
        element: &Id,
        lod: Option<u32>,
    ) -> Result<ShapeView, Refusal> {
        let place = Place::Built {
            project: project.clone(),
            document: model.clone(),
            element: element.clone(),
        };
        let built: Option<BuiltElement> = read_row(snapshot, &place.key()).await?;
        let built = built.ok_or(Refusal::NotFound)?;
        let BuiltResult::Shape { displays, .. } = &built.result else {
            return Ok(ShapeView {
                built,
                display: None,
            });
        };
        let wanted = match lod {
            Some(lod) => displays.iter().find(|display| display.lod == lod),
            None => displays.iter().max_by_key(|display| display.lod),
        };
        let Some(wanted) = wanted else {
            return Err(Refusal::NotFound);
        };
        let place = Place::Display {
            project: project.clone(),
            id: wanted.id.clone(),
        };
        let stored: Option<Stored> = read_row(snapshot, &place.key()).await?;
        let stored = stored.ok_or_else(|| StoreError::Damaged("display missing".into()))?;
        let bytes = self.regen.read_bytes(&stored, &wanted.id).await?;
        let display =
            serde_json::from_slice(&bytes).map_err(|e| StoreError::Damaged(e.to_string()))?;
        Ok(ShapeView {
            built,
            display: Some(display),
        })
    }

    pub async fn links(&self, person: &Id, project: &Id, model: &Id) -> Result<Vec<Link>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(scan_rows(&mut snapshot, &list::links(project, model)).await?)
    }

    /// A package's file. Its row says where the bytes are, in this project's own folder.
    pub async fn package_file(
        &self,
        person: &Id,
        project: &Id,
        team: &Id,
        folder: &Id,
        package: &Id,
    ) -> Result<Vec<u8>, Refusal> {
        let target = Target::Package(project, team, folder, package);
        self.require(person, "package.read", target).await?;
        let mut snapshot = self.kv.snapshot().await?;
        let place = Place::Item {
            project: project.clone(),
            team: team.clone(),
            folder: folder.clone(),
            item: package.clone(),
        };
        let item: Option<Item> = read_row(&mut snapshot, &place.key()).await?;
        let item = item.ok_or(Refusal::NotFound)?;
        Ok(self.files.get(&item.file).await?)
    }

    // --- Changing folders ---

    /// The whole model at one moment, as one file, under the hash of its bytes.
    async fn package_bytes(
        &self,
        project: &Id,
        model: &Id,
    ) -> Result<(Vec<u8>, u64, usize), Refusal> {
        let mut snapshot = self.regen.caught_up(project).await?;
        let scope = Scope::Project(project.clone());
        let seq: u64 = read_row(&mut snapshot, &scope.head()).await?.unwrap_or(0);
        let place = Place::Model {
            project: project.clone(),
            model: model.clone(),
        };
        let row: Option<Model> = read_row(&mut snapshot, &place.key()).await?;
        let row = row.ok_or(Refusal::NotFound)?;
        let mut elements = Vec::new();
        for (_, hash) in snapshot.scan(&kit_list::pieces(project, model)).await? {
            let hash = from_row::<String>(&hash)?;
            let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
            let place = KitPlace::Record {
                project: project.clone(),
                hash,
            };
            let element: Option<Element> = read_row(&mut snapshot, &place.key()).await?;
            let element = element.ok_or_else(|| StoreError::Damaged("record missing".into()))?;
            let shape = self
                .stored_shape(&mut snapshot, project, model, &element.id, None)
                .await?;
            elements.push(
                json!({ "element": element, "built": shape.built, "display": shape.display }),
            );
        }
        let count = elements.len();
        let file = PackageFile {
            project: project.to_string(),
            document: model.to_string(),
            edition: row.edition,
            seq,
            elements,
        };
        let bytes = serde_json::to_vec(&file).map_err(|e| StoreError::Damaged(e.to_string()))?;
        Ok((bytes, seq, count))
    }

    /// Publishes a team's model as the next numbered package in its Shared folder. The file goes
    /// up first, under its own hash, so a refused publish leaves only an unused file.
    pub async fn publish(
        &self,
        person: &Id,
        project: &Id,
        team: &Id,
        model: &Id,
    ) -> Result<Item, Refusal> {
        self.require(person, "package.publish", Target::Team(project, team))
            .await?;
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let (bytes, seq, elements) = self.package_bytes(project, model).await?;
        let sha256 = sha256_hex(&bytes);
        let file = format!("packages/{sha256}.json");
        self.files.put(&file, bytes).await?;
        let args = json!({
            "team": team.as_str(), "seq": seq, "file": file, "sha256": sha256, "elements": elements,
        });
        let entry = self
            .edit(person, project, model, "package.publish", args)
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let items: Vec<Item> = scan_rows(
            &mut snapshot,
            &list::items(project, team, &folders::shared()),
        )
        .await?;
        let item = items.into_iter().find(|item| item.entry == entry);
        item.ok_or_else(|| Refusal::Failed(StoreError::Damaged("published item missing".into())))
    }

    /// Sends a shared package to another hub's project. The orderer asks Cedar whether both hubs
    /// permit it, and writes both logs in one transaction.
    pub async fn bridge(
        &self,
        person: &Id,
        project: &Id,
        team: &Id,
        package: &Id,
        to_project: &Id,
        to_team: &Id,
    ) -> Result<(), Refusal> {
        let args = json!({
            "team": team.as_str(), "to_project": to_project.as_str(), "to_team": to_team.as_str(),
        });
        self.edit(person, project, package, "package.bridge", args)
            .await?;
        Ok(())
    }

    /// Takes a bridged package into a team's Consumed folder, after checking the file against the
    /// hash it was published with.
    pub async fn consume(
        &self,
        person: &Id,
        project: &Id,
        team: &Id,
        incoming: &Id,
    ) -> Result<Item, Refusal> {
        self.require(person, "package.consume", Target::Team(project, team))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let place = Place::Incoming {
            project: project.clone(),
            id: incoming.clone(),
        };
        let waiting: Option<Incoming> = read_row(&mut snapshot, &place.key()).await?;
        let waiting = waiting.ok_or(Refusal::NotFound)?;
        let bytes = self.files.get(&waiting.item.file).await?;
        if sha256_hex(&bytes) != waiting.item.sha256 {
            return Err(Refusal::Bad(
                "That package's file doesn't match its hash.".to_owned(),
            ));
        }
        let args = json!({ "team": team.as_str() });
        self.edit(person, project, incoming, "package.take", args)
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let place = Place::Item {
            project: project.clone(),
            team: team.clone(),
            folder: folders::consumed(),
            item: incoming.clone(),
        };
        let item: Option<Item> = read_row(&mut snapshot, &place.key()).await?;
        item.ok_or_else(|| Refusal::Failed(StoreError::Damaged("consumed item missing".into())))
    }

    pub async fn add_link(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        kind: LinkKind,
        team: Option<&Id>,
        source: &Id,
    ) -> Result<(), Refusal> {
        let args = json!({ "kind": kind, "team": team.map(Id::as_str), "source": source.as_str() });
        self.edit(person, project, model, "link.add", args).await?;
        Ok(())
    }

    // --- The live channel ---

    pub async fn may_open_project(&self, person: &Id, project: &Id) -> Result<(), Refusal> {
        self.require(person, "project.open", Target::Project(project))
            .await
    }

    pub async fn may_watch(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<bool, StoreError> {
        self.access
            .allowed(person, "model.read", Target::Model(project, model), None)
            .await
    }

    pub async fn orderer(&self, project: &Id) -> Arc<Orderer> {
        self.keep_up(project);
        self.orderers.get(&Scope::Project(project.clone())).await
    }

    /// Brings a project's shapes up to the end of its log now. A running server does this by
    /// itself; the seed and the operators' tools call it to know it's done.
    pub async fn regenerate(&self, project: &Id) -> Result<(), StoreError> {
        self.regen.catch_up(project).await.map(|_| ())
    }

    /// How far a project's log has got.
    pub async fn head(&self, person: &Id, project: &Id) -> Result<u64, Refusal> {
        self.require(person, "project.open", Target::Project(project))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let scope = Scope::Project(project.clone());
        Ok(read_row::<u64, _>(&mut snapshot, &scope.head())
            .await?
            .unwrap_or(0))
    }

    /// Whether the store answers, for the health check. It reads nobody's data.
    pub async fn healthy(&self) -> Result<(), StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        snapshot.get(&KitPlace::Format.key()).await.map(|_| ())
    }

    /// How far a browser tab got. `person` is the one signed in on the connection.
    pub async fn last_counter(
        &self,
        person: &Id,
        project: &Id,
        client: &Id,
    ) -> Result<u64, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let key = Scope::Project(project.clone()).client(person, client);
        let mark: Option<ClientMark> = read_row(&mut snapshot, &key).await?;
        Ok(mark.map_or(0, |mark| mark.counter))
    }

    /// Log entries for the live channel to filter: the project must be open to this person, and
    /// the channel checks each entry against what they may watch before sending it.
    pub async fn entries(
        &self,
        person: &Id,
        project: &Id,
        from: u64,
        to: u64,
    ) -> Result<Vec<Entry>, Refusal> {
        self.require(person, "project.open", Target::Project(project))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let scope = Scope::Project(project.clone());
        Ok(read_log(&mut snapshot, &scope, from, to).await?)
    }
}
