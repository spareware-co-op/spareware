//! Everything a person can do, each step behind Cedar. The web layer holds this and nothing else:
//! the store is private to this file, so there is no way for a handler to read a row without the
//! check, and no write here but through an orderer. Generic over the store, so the same rules run
//! in-memory in `tests/service.rs`.

use crate::directory::DirectoryCommands;
use crate::plan::{Drawer, HostedOn, PlanCrop, SectionAsk, Skipped, ToDraw, ViewLooks};
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
use construct_params::{Length, Units, Value as StoredValue};
use construct_regen::catalog::{self, Catalog, TableView, TypesView};
use construct_regen::hosting::{self, Hosting};
use construct_regen::levels::{self as levels, LevelView};
use construct_regen::parameters::{self as records, ElementView};
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

/// Why the service said no. Its words are the ones a person sees.
#[derive(Debug, Error)]
pub enum Refusal {
    /// Nobody is signed in, or the person is unknown.
    #[error("Please sign in.")]
    SignIn,
    /// This person may not do this, and why.
    #[error("{0}")]
    Refused(String),
    /// What was asked for isn't there, or isn't this person's to see.
    #[error("Not found.")]
    NotFound,
    /// The request itself is wrong, and how.
    #[error("{0}")]
    Bad(String),
    /// The store failed.
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
    /// How far regeneration had got when these were read.
    pub through: u64,
    /// Each element in view, by ID, with its record's hash.
    pub pieces: BTreeMap<String, String>,
}

/// A stored shape, for someone who can't build this document's edition themselves.
#[derive(Serialize, Debug)]
pub struct ShapeView {
    /// What regeneration made of the element.
    pub built: BuiltElement,
    /// The level of detail asked for, when it was built.
    pub display: Option<Display>,
}

/// An element's recipe as it is built now, every argument that reads a record filled.
#[derive(Serialize, Debug)]
pub struct FilledView {
    /// How far the project's log had got when it was read.
    pub head: u64,
    /// The filled recipe, when it fills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<construct_model::geometry::Recipe>,
    /// Why it doesn't, as a build failure: `code` and `message`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<Value>,
    /// For a floor, what cuts it: the outline of each opening and shaft, and the shafts as its
    /// builder is handed them ([`cloud::openings::FloorCuts`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_by: Option<cloud::openings::FloorCuts>,
    /// For a room, what became of it and what it measures, in the document's units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<construct_regen::rooms::RoomView>,
    /// For a curtain wall, its grid as it is laid out now (every line and panel, for the page to edit
    /// by), in the document's units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curtain: Option<construct_regen::curtains::CurtainView>,
    /// For a curtain wall embedded in a wall, where its record stands in the model: its host's
    /// place and its own height above it. Its `recipe` is then its recipe as seated, with its line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<construct_model::element::Placement>,
    /// For a curtain wall embedded in a wall, what it cuts in its host: the opening's place in the
    /// host's frame and its recipe, as the host's builder is handed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut: Option<FilledCut>,
}

/// What an embedded curtain wall cuts in its host ([`cloud::curtains::Seat`]).
#[derive(Serialize, Debug)]
pub struct FilledCut {
    /// Where the opening is, in the host's frame, in whole millimetres.
    pub placement: construct_model::element::Placement,
    /// The opening's recipe.
    pub recipe: construct_model::geometry::Recipe,
}

const NO_DRAWER: &str =
    "This server was built without the geometry kernel, so it cannot draw plans.";

const NO_SECTION_DRAWER: &str = "This server was built without the geometry kernel, so it cannot \
                                 draw sections or elevations.";

/// A floor plan as the server draws it from the stored model.
#[derive(Serialize, Debug)]
pub struct PlanDrawing {
    /// The view that was drawn.
    pub view: cloud::views::ViewSummary,
    /// How far regeneration had got when it was read; the plan is of the model as of that point.
    pub through: u64,
    /// The drawing, as one SVG document in model millimetres, y up the sheet.
    pub svg: String,
    /// The elements the plan cuts that it could not draw, each with the reason.
    pub skipped: Vec<Skipped>,
}

/// The built elements of a model with a shape: each record, the box of its shape and the height
/// of its base.
async fn shaped_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    model: &Id,
) -> Result<Vec<cloud::views::Shaped>, Refusal> {
    let mut shaped = Vec::new();
    for (_, (_, record)) in records::read_document(reader, project, model).await? {
        if !records::has_shape(&record) {
            continue;
        }
        let place = Place::Built {
            project: project.clone(),
            document: model.clone(),
            element: record.id.clone(),
        };
        let built: Option<BuiltElement> = read_row(reader, &place.key()).await?;
        let Some(built) = built else { continue };
        if let BuiltResult::Shape { bounds, .. } = built.result {
            shaped.push(cloud::views::Shaped {
                record,
                bounds,
                elevation: built.elevation,
            });
        }
    }
    Ok(shaped)
}

/// What the drawer needs of one element: its category, its place with its base's height added
/// (whole millimetres) and its recipe as the regenerator fills it. The inner error says, in
/// words, why this element cannot be drawn.
async fn to_draw<R: Reader>(
    reader: &mut R,
    project: &Id,
    model: &Id,
    one: &cloud::views::Shaped,
    shaped: &[cloud::views::Shaped],
) -> Result<Result<ToDraw, String>, StoreError> {
    let record = &one.record;
    let Some(category) =
        records::stored_category(record).or_else(|| records::category_of(&record.recipe).ok())
    else {
        return Ok(Err("it has no category".to_owned()));
    };
    let mut recipe =
        match levels::recipe_at(reader, project, model, record, records::At::Now).await? {
            Ok(recipe) => recipe,
            Err(failed) => return Ok(Err(failed.message)),
        };
    // A curtain wall embedded in a wall is drawn where it is seated there.
    let mut placement = record.placement;
    let mut curtain_seated = false;
    if let Some(seat) =
        match cloud::curtains::read_seat(reader, project, model, record, &recipe).await? {
            Ok(seat) => seat,
            Err(why) => return Ok(Err(why)),
        }
    {
        recipe = seat.recipe;
        placement = seat.placement;
        curtain_seated = true;
    }
    let base = match one.elevation.as_deref().map(StoredValue::from_stored) {
        None => 0,
        Some(Some(StoredValue::Length(length)))
            if length.steps().rem_euclid(Length::STEPS_PER_MM) == 0 =>
        {
            length.steps().div_euclid(Length::STEPS_PER_MM)
        }
        Some(_) => {
            return Ok(Err(
                "its base is not a length on a whole millimetre".to_owned()
            ));
        }
    };
    let Some(z) = placement.z.checked_add(base) else {
        return Ok(Err("it stands too high to be drawn".to_owned()));
    };
    placement.z = z;
    // An embedded curtain wall is seated by its own commands' rule above: the plan's seat in an
    // angled wall (piece A83) is for doors, windows and openings.
    let hosted_on = if curtain_seated {
        None
    } else {
        match hosted_on(reader, project, model, record, shaped).await? {
            Ok(hosted_on) => hosted_on,
            Err(why) => return Ok(Err(why)),
        }
    };
    Ok(Ok(ToDraw {
        id: record.id.clone(),
        category,
        placement,
        recipe,
        hosted_on,
    }))
}

/// A curtain wall's part of its filled view: its grid as it is laid out, and, embedded in a wall,
/// where it stands there (its recipe as seated, its place, and the opening it cuts), so the page
/// builds what the regenerator builds. Nothing for any other element.
async fn add_curtain<R: Reader>(
    reader: &mut R,
    project: &Id,
    model: &Id,
    record: &Element,
    view: &mut FilledView,
) -> Result<(), StoreError> {
    let Some(filled) = view
        .recipe
        .clone()
        .filter(construct_regen::curtain_plan::is_curtain)
    else {
        return Ok(());
    };
    view.curtain = construct_regen::curtains::CurtainView::of(&filled, &Units::default());
    match cloud::curtains::read_seat(reader, project, model, record, &filled).await? {
        Ok(None) => {}
        Ok(Some(seat)) => {
            view.recipe = Some(seat.recipe);
            view.placement = Some(seat.placement);
            view.cut = Some(FilledCut {
                placement: seat.cut.0,
                recipe: seat.cut.1,
            });
        }
        Err(message) => {
            view.recipe = None;
            view.failed = Some(json!({ "code": "BadCurtain", "message": message }));
        }
    }
    Ok(())
}

/// What a plan needs to seat `record` in its wall, if it is hosted on a face of one that is on
/// the page (piece A83): the wall as the regenerator builds it. `None` for anything else.
async fn hosted_on<R: Reader>(
    reader: &mut R,
    project: &Id,
    model: &Id,
    record: &Element,
    shaped: &[cloud::views::Shaped],
) -> Result<Result<Option<HostedOn>, String>, StoreError> {
    let Ok(Some(hosting @ Hosting::OnFace { .. })) = hosting::read(record) else {
        return Ok(Ok(None));
    };
    let Some(host) = shaped.iter().find(|s| s.record.id == *hosting.host()) else {
        return Ok(Ok(None));
    };
    let host_recipe =
        match levels::recipe_at(reader, project, model, &host.record, records::At::Now).await? {
            Ok(recipe) => recipe,
            Err(failed) => return Ok(Err(failed.message)),
        };
    Ok(Ok(Some(HostedOn {
        record: record.clone(),
        host: host.record.clone(),
        host_recipe,
        hosting,
    })))
}

/// An area of a level.
pub struct View {
    /// The level, by its permanent ID. A level an old record's integer implies has that integer
    /// as its ID (`1`), so a page that asks by the integer still finds what it found.
    pub level: Id,
    /// One corner's x, in millimetres.
    pub x0: i64,
    /// One corner's y, in millimetres.
    pub y0: i64,
    /// The opposite corner's x, in millimetres.
    pub x1: i64,
    /// The opposite corner's y, in millimetres.
    pub y1: i64,
}

/// Everything a person can do, each step behind Cedar.
pub struct Service<K: Kv, F: FileStore> {
    kv: K,
    /// Cedar, answering Spare Construct's questions.
    pub access: Arc<Access<K>>,
    /// People, sign-in and sessions.
    pub accounts: Accounts<K, DirectoryCommands<K>>,
    orderers: Arc<Orderers<K, ProjectCommands<K, Cedar>>>,
    clock: Arc<dyn Clock>,
    files: F,
    regen: Arc<Regenerator<K, F>>,
    regenerating: Mutex<HashSet<Id>>,
    /// What draws a view's plan; none on a server built without the geometry kernel.
    drawer: Option<Arc<dyn Drawer>>,
}

impl<K: Kv, F: FileStore> Service<K, F> {
    /// A service over a store, its file storage and its orderers, building shapes with `geometry`.
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
            drawer: None,
        }
    }

    /// This service with `drawer` drawing the plans people open ([`Service::plan`]).
    #[must_use]
    pub fn with_drawer(mut self, drawer: Option<Arc<dyn Drawer>>) -> Self {
        self.drawer = drawer;
        self
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the orderer rejects the edit.
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
    ///
    /// # Errors
    ///
    /// [`Refusal::SignIn`] when the person is unknown, or a [`Refusal`] when the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the view is too big.
    pub async fn area(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &View,
    ) -> Result<AreaView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let widest = WIDEST_VIEW_MM.unsigned_abs();
        if view.x1.abs_diff(view.x0) > widest || view.y1.abs_diff(view.y0) > widest {
            return Err(Refusal::Bad("That view is too big.".to_owned()));
        }
        let mut snapshot = self.kv.snapshot().await?;
        let mut pieces = BTreeMap::new();
        for (x, y) in cells_between(view.x0, view.y0, view.x1, view.y1) {
            let prefix = list::cell(project, model, &view.level, x, y);
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

    /// The elements of a model of one kind, built or not, as `area` gives them: a page that works
    /// on one kind (Workshop's parts and assemblies) finds them without a place in the plan.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
    pub async fn kind(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        kind: &Id,
    ) -> Result<AreaView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let mut pieces = BTreeMap::new();
        for (key, hash) in snapshot.scan(&list::kind(project, model, kind)).await? {
            let Some(Place::Kind { element, .. }) = Place::parse(&key) else {
                continue;
            };
            pieces.insert(element.to_string(), from_row::<String>(&hash)?);
        }
        let cursor = Place::Regenerated {
            project: project.clone(),
        };
        let through: u64 = read_row(&mut snapshot, &cursor.key()).await?.unwrap_or(0);
        Ok(AreaView { through, pieces })
    }

    /// What the page shows of an element: its category, its type, and its parameters with the
    /// values it reads and where each came from, shown in units. The same reader the commands
    /// check against and the regenerator builds from (`construct_regen::parameters`). A floor's
    /// area, volume and mass are those after the openings and shafts that cut it
    /// ([`cloud::openings::floor_cuts`]); a stair's quantities are its layout's as its levels
    /// stand ([`construct_regen::stairs::read_only_at`]).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, the store fails, or there is no such
    /// element; [`Refusal::Bad`] when its stored values cannot be read, saying why.
    pub async fn parameters(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        element: &Id,
    ) -> Result<ElementView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let Some((_, record)) =
            records::read_element(&mut snapshot, project, model, element).await?
        else {
            return Err(Refusal::NotFound);
        };
        let context = records::read_context(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let ty = match records::type_of(&record) {
            None => None,
            Some(type_id) => Some(
                records::read_type(&mut snapshot, project, model, &type_id, &context)
                    .await?
                    .map_err(Refusal::Bad)?,
            ),
        };
        let cuts = if construct_regen::floor_plan::is_floor(&record.recipe) {
            cloud::openings::floor_cuts(&mut snapshot, project, model, &record).await?
        } else {
            Vec::new()
        };
        let mut view = context
            .view_cut(&record, ty.as_ref(), &cuts)
            .map_err(Refusal::Bad)?;
        // A wall's length, area and volume, openings taken out and joints made, and each layer's
        // volume, worked out of the document as it stands and never stored.
        view.parameters.extend(
            construct_regen::wall_measure::read_only_view(
                &mut snapshot,
                project,
                model,
                &record,
                &context.units,
            )
            .await?,
        );
        // A curtain wall's quantities, from its recipe as it is built now: its type's grid read in
        // and its top tie read.
        if construct_regen::curtain_plan::is_curtain(&record.recipe)
            && let Ok(filled) = construct_regen::levels::recipe_at(
                &mut snapshot,
                project,
                model,
                &record,
                records::At::Now,
            )
            .await?
        {
            view.parameters
                .extend(construct_regen::curtains::read_only_view(
                    &filled,
                    &context.units,
                ));
        }
        if construct_regen::rooms::is_room(&record) {
            let worked = construct_regen::rooms::worked_at(
                &mut snapshot,
                project,
                model,
                &record,
                records::At::Now,
            )
            .await?;
            if let Ok(worked) = worked {
                let room = construct_regen::rooms::RoomView::of(&worked, &context.units);
                view.parameters.extend(room.rows());
            }
        }
        // A stair's rise, risers, riser height, treads, going, total run, pitch and width, read
        // only, from its layout as its levels stand.
        view.parameters.extend(
            construct_regen::stairs::read_only_at(
                &mut snapshot,
                project,
                model,
                &record,
                &context.units,
                records::At::Now,
            )
            .await?,
        );
        Ok(view)
    }

    /// A model's stairs as the page's stair tool needs them: the rule set's categories, and each
    /// stair with its layout as its levels stand, or why it has none
    /// ([`cloud::stairs::read_stairs`]).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails.
    pub async fn stairs(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::stairs::StairsView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(cloud::stairs::read_stairs(&mut snapshot, project, model).await?)
    }

    /// The kinds of parameter and the categories of a model, for the page's lists: read from the
    /// data crates, so the page holds no list of its own.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when its stored parameters cannot be read, saying why.
    pub async fn catalog(&self, person: &Id, project: &Id, model: &Id) -> Result<Catalog, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        catalog::read_catalog(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's project parameters: each with its kind, who holds its value, its default and the
    /// categories it is bound to, bound or not.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when its stored parameters cannot be read, saying why.
    pub async fn project_parameters(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<TableView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        catalog::read_table_view(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's types, or those of one category: each with the parameters it lists, the values it
    /// holds in units, and how many elements are of it.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when the category is unknown or stored values cannot be read, saying why.
    pub async fn types(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        category: Option<u32>,
    ) -> Result<TypesView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        catalog::read_types_view(&mut snapshot, project, model, category)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's levels, lowest first: each with its name, its elevation stored and shown in units,
    /// whether it is a storey, whether an old record's integer implies it, and how many elements
    /// are tied to it. The same reader the level commands check against
    /// (`construct_regen::levels`).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a level record or a tie cannot be read, saying why.
    pub async fn levels(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<LevelView>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        levels::read_views(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's wall types with their layers, the materials those are drawn in, and the default
    /// wall types' `wall.type.create` arguments, for the page's wall tool
    /// ([`cloud::walls::read_wall_types`]).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a type or the parameter table cannot be read, saying why.
    pub async fn wall_types(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::walls::WallTypesView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::walls::read_wall_types(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's families with their types, the values each placed one holds, where each came
    /// from and how many are placed, then the library's families it does not hold, for the page's
    /// family tool ([`cloud::loading::read_families`]).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a family's record cannot be read, saying why.
    pub async fn families(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<cloud::families::FamilyView>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::loading::read_families(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's floor types with their layers and the materials those are drawn in, for the
    /// page's floor tool ([`cloud::floors::read_floor_types`]); where it has none, the page sends
    /// `floor.defaults`.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a type or the parameter table cannot be read, saying why.
    pub async fn floor_types(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::floors::FloorTypesView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::floors::read_floor_types(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's roof types with their layers and the materials those are drawn in, for the
    /// page's roof tool ([`cloud::roofs::read_roof_types`]); where it has none, the page sends
    /// `roof.defaults`.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a type or the parameter table cannot be read, saying why.
    pub async fn roof_types(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::roofs::RoofTypesView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::roofs::read_roof_types(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's curtain wall types with their grid and parts, the materials those are drawn in
    /// and the default types, for the page's curtain wall tool
    /// ([`cloud::curtains::read_curtain_types`]); where it has none, the page sends
    /// `curtain.defaults`.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a type or the parameter table cannot be read, saying why.
    pub async fn curtain_types(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::curtains::CurtainTypesView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::curtains::read_curtain_types(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)
    }

    /// A model's rooms as the page's room tool needs them: the next free number.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails.
    pub async fn rooms(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::rooms::RoomsView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(cloud::rooms::read_rooms(&mut snapshot, project, model).await?)
    }

    /// The problems a model holds now (a room not enclosed, two walls overlapping), worked out
    /// from the document as it stands, never stored: by kind, then by the elements involved.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails.
    pub async fn warnings(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::warnings::WarningsView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(cloud::warnings::read_warnings(&mut snapshot, project, model).await?)
    }

    /// The recipe an element is built from now, each argument that reads a record filled (its
    /// parameters, its wall type's layers, its top tie) and a layered wall's joints with the walls
    /// it meets, as the regenerator fills it (`construct_regen::joins::recipe_at`), with how far
    /// the log had got when it was read: a page builds an
    /// element whose recipe reads records from this, and compares what it built with the server's
    /// bodies from that point of the log on. A recipe that cannot be filled is the build failure,
    /// in its words.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails, or there is no
    /// such element.
    pub async fn filled(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        element: &Id,
    ) -> Result<FilledView, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let scope = Scope::Project(project.clone());
        let head = read_row::<u64, _>(&mut snapshot, &scope.head())
            .await?
            .unwrap_or(0);
        let Some((_, record)) =
            records::read_element(&mut snapshot, project, model, element).await?
        else {
            return Err(Refusal::NotFound);
        };
        let at = records::At::Now;
        if construct_regen::rooms::is_room(&record) {
            let worked =
                construct_regen::rooms::worked_at(&mut snapshot, project, model, &record, at)
                    .await?;
            return Ok(match worked {
                Ok(worked) => {
                    let room = construct_regen::rooms::RoomView::of(&worked, &Units::default());
                    match worked.0 {
                        Ok(recipe) => FilledView {
                            head,
                            recipe: Some(recipe),
                            failed: None,
                            cut_by: None,
                            room: Some(room),
                            curtain: None,
                            placement: None,
                            cut: None,
                        },
                        Err(failed) => FilledView {
                            head,
                            recipe: None,
                            failed: Some(json!({ "code": failed.code, "message": failed.message })),
                            cut_by: None,
                            room: Some(room),
                            curtain: None,
                            placement: None,
                            cut: None,
                        },
                    }
                }
                Err(failed) => FilledView {
                    head,
                    recipe: None,
                    failed: Some(json!({ "code": failed.code, "message": failed.message })),
                    cut_by: None,
                    room: None,
                    curtain: None,
                    placement: None,
                    cut: None,
                },
            });
        }
        Ok(
            match construct_regen::joins::recipe_at(&mut snapshot, project, model, &record, at)
                .await?
            {
                Ok(recipe) => {
                    let cut_by = if construct_regen::floor_plan::is_floor(&record.recipe) {
                        Some(
                            cloud::openings::floor_view(&mut snapshot, project, model, &record)
                                .await?,
                        )
                    } else {
                        None
                    };
                    let mut view = FilledView {
                        head,
                        recipe: Some(recipe),
                        failed: None,
                        cut_by,
                        room: None,
                        curtain: None,
                        placement: None,
                        cut: None,
                    };
                    add_curtain(&mut snapshot, project, model, &record, &mut view).await?;
                    view
                }
                Err(failed) => FilledView {
                    head,
                    recipe: None,
                    failed: Some(json!({ "code": failed.code, "message": failed.message })),
                    cut_by: None,
                    room: None,
                    curtain: None,
                    placement: None,
                    cut: None,
                },
            },
        )
    }

    /// A model's views: each with its kind, name, level (a plan's), scale and detail level, by name.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a view record cannot be read, saying why.
    pub async fn views(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<cloud::views::ViewSummary>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let world = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        Ok(cloud::views::summaries(&world))
    }

    /// A model's sheets: each with its number, name, paper and how many viewports it holds, by
    /// number.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a record cannot be read, saying why.
    pub async fn sheets(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<cloud::sheets::SheetSummary>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let world = cloud::sheets::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        Ok(cloud::sheets::summaries(&world))
    }

    /// One sheet with its viewports as they lie on the paper: each view's scale from the view, each
    /// outline from the view's extent (a plan's is the box of what it shows, as of the log's head)
    /// and every alignment worked out.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// sheet; [`Refusal::Bad`] when a record cannot be read or placed, saying why.
    pub async fn sheet(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        sheet: &Id,
    ) -> Result<cloud::sheets::SheetListing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.regen.caught_up(project).await?;
        let world = cloud::sheets::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let views = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let shaped = shaped_of(&mut snapshot, project, model).await?;
        cloud::sheets::listing_of_sheet(&world, &views, sheet, &shaped)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)
    }

    /// What one view shows: the elements it cuts and the ones it sees, from construct-views' own
    /// rule over the model's stored shapes, as of the log's head.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// view; [`Refusal::Bad`] when the view does not stand in the model, saying why.
    pub async fn view(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &Id,
    ) -> Result<cloud::views::ViewListing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.regen.caught_up(project).await?;
        let world = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let shaped = shaped_of(&mut snapshot, project, model).await?;
        cloud::views::listing_of(&world, view, &shaped)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)
    }

    /// A model's schedules: each with its category, fields, filters and sorting, by name. Read
    /// only; schedules are made and changed by the schedule commands.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a schedule record cannot be read, saying why.
    pub async fn schedules(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<cloud::schedules::ScheduleSummary>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let world = cloud::schedules::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        Ok(cloud::schedules::summaries(&world))
    }

    /// The categories a schedule may list (what a person chooses from when making one).
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model or the store fails;
    /// [`Refusal::Bad`] when a schedule record cannot be read, saying why.
    pub async fn schedule_categories(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<Vec<cloud::schedules::ListableView>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let world = cloud::schedules::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        Ok(cloud::schedules::listable(&world))
    }

    /// One schedule with its rows: a live query over the elements as they are now (a row for
    /// each element of its category that passes its filters, in its sorting), worked out on each
    /// read and stored nowhere.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// schedule; [`Refusal::Bad`] when a record cannot be read, saying why.
    pub async fn schedule(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        schedule: &Id,
    ) -> Result<cloud::schedules::ScheduleTable, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        cloud::schedules::read_table(&mut snapshot, project, model, schedule)
            .await?
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)
    }

    /// The dimensions of one view, each with its value worked out now from the references it is
    /// bound to (a layered wall's faces, a grid line, a door's or window's centre or edges), its
    /// witness lines, its stored equalities and its warnings: a reference that was deleted is
    /// named, and the segments it ends have no value. Read as of the log's head; nothing is
    /// stored but the dimension's references and its line.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// view; [`Refusal::Bad`] when a record cannot be read, saying why.
    pub async fn dimensions(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &Id,
    ) -> Result<cloud::dimensions::DimensionListing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.regen.caught_up(project).await?;
        cloud::dimensions::listing(&mut snapshot, project, model, view)
            .await?
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)
    }

    /// A view's graphics rules: how each category or subcategory it says anything about looks in
    /// it, for what it cuts and what it sees beyond. A category it says nothing about looks as the
    /// project's object styles say. Read as of the log's head.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// view; [`Refusal::Bad`] when a view record cannot be read, saying why.
    pub async fn graphics(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &Id,
    ) -> Result<cloud::views::GraphicsListing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        let world = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        cloud::views::graphics_of(&world, view).ok_or(Refusal::NotFound)
    }

    /// A floor plan drawn from the stored model as of the log's head: the cut edges of what its
    /// cut plane crosses, hatched by material, as the geometry kernel's drawing makes them
    /// ([`crate::plan`]). The page asks for it when a person opens the view and again when the
    /// model changes, and draws nothing of its own.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// view; [`Refusal::Bad`] when this server has no drawer (it was built without the geometry
    /// kernel), when the view is not a floor plan, or when the drawing cannot be made, saying why.
    pub async fn plan(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &Id,
    ) -> Result<PlanDrawing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.regen.caught_up(project).await?;
        let world = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let cut = cloud::views::plan_cut(&world, view)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)?;
        let Some(drawer) = &self.drawer else {
            return Err(Refusal::Bad(NO_DRAWER.to_owned()));
        };
        let shaped = shaped_of(&mut snapshot, project, model).await?;
        let listing = cloud::views::listing_of(&world, view, &shaped)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)?;
        let mut elements = Vec::new();
        let mut left_out = Vec::new();
        for one in &shaped {
            if !listing.cut.contains(&one.record.id.to_string()) {
                continue;
            }
            match to_draw(&mut snapshot, project, model, one, &shaped).await? {
                Ok(element) => elements.push(element),
                Err(why) => left_out.push(Skipped {
                    element: one.record.id.to_string(),
                    why,
                }),
            }
        }
        let looks = ViewLooks::new(&world, view);
        let crop = world.views().get(view).and_then(|v| v.crop());
        let drawn = drawer
            .draw(
                cut.cut,
                cut.scale,
                &elements,
                Some(&looks),
                crop.map(|crop| PlanCrop { view, crop }),
            )
            .map_err(Refusal::Bad)?;
        left_out.extend(drawn.skipped);
        let cursor = Place::Regenerated {
            project: project.clone(),
        };
        let through: u64 = read_row(&mut snapshot, &cursor.key()).await?.unwrap_or(0);
        Ok(PlanDrawing {
            view: listing.view,
            through,
            svg: drawn.svg,
            skipped: left_out,
        })
    }

    /// A section or an elevation drawn from the stored model as of the log's head
    /// ([`crate::section`]): what its plane cuts, the outlines of what it sees beyond to its depth,
    /// kept to its width and its crop, and the levels and grids that cross it with their names.
    /// The page asks for it as it asks for a plan, with the same answer's shape.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, the store fails or there is no such
    /// view; [`Refusal::Bad`] when this server has no drawer, when the view is not a section or an
    /// elevation, or when the drawing cannot be made (a line not along a grid axis among them),
    /// saying why.
    pub async fn section(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
        view: &Id,
    ) -> Result<PlanDrawing, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.regen.caught_up(project).await?;
        let world = cloud::views::read_world(&mut snapshot, project, model)
            .await?
            .map_err(Refusal::Bad)?;
        let cut = cloud::views::section_cut(&world, view)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)?;
        let Some(drawer) = &self.drawer else {
            return Err(Refusal::Bad(NO_SECTION_DRAWER.to_owned()));
        };
        let shaped = shaped_of(&mut snapshot, project, model).await?;
        let listing = cloud::views::listing_of(&world, view, &shaped)
            .map_err(Refusal::Bad)?
            .ok_or(Refusal::NotFound)?;
        let mut elements = Vec::new();
        let mut left_out = Vec::new();
        for one in &shaped {
            let id = one.record.id.to_string();
            if !listing.cut.contains(&id) && !listing.seen.contains(&id) {
                continue;
            }
            match to_draw(&mut snapshot, project, model, one).await? {
                Ok(element) => elements.push(element),
                Err(why) => left_out.push(Skipped { element: id, why }),
            }
        }
        let datums = cloud::views::datums_of(&mut snapshot, project, model, &world)
            .await?
            .map_err(Refusal::Bad)?;
        let looks = ViewLooks::new(&world, view);
        let crop = world.views().get(view).and_then(|v| v.crop());
        let drawn = drawer
            .section(&SectionAsk {
                line: &cut.line,
                cuts: cut.cuts,
                scale: cut.scale,
                elements: &elements,
                datums: &datums,
                looks: Some(&looks),
                crop: crop.map(|crop| PlanCrop { view, crop }),
            })
            .map_err(Refusal::Bad)?;
        left_out.extend(drawn.skipped);
        let cursor = Place::Regenerated {
            project: project.clone(),
        };
        let through: u64 = read_row(&mut snapshot, &cursor.key()).await?.unwrap_or(0);
        Ok(PlanDrawing {
            view: listing.view,
            through,
            svg: drawn.svg,
            skipped: left_out,
        })
    }

    /// What this person can undo and redo in a model now: the step an undo would reverse and the
    /// undo a redo would, read from the log the way `edit.undo` and `edit.redo` read it
    /// ([`construct_api::cloud::undo`]). Nothing about it is kept anywhere but the log.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not read the model, or the store fails.
    pub async fn undo(
        &self,
        person: &Id,
        project: &Id,
        model: &Id,
    ) -> Result<cloud::undo::State, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(cloud::undo::state(&mut snapshot, project, model, person).await?)
    }

    /// Who is working on what in a model: each claimed element, with its claim.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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

    /// An element's record, by its hash, if it belongs to this model.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the record isn't this model's.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or there is no such shape.
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

    /// A model's links.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
    pub async fn links(&self, person: &Id, project: &Id, model: &Id) -> Result<Vec<Link>, Refusal> {
        self.require(person, "model.read", Target::Model(project, model))
            .await?;
        let mut snapshot = self.kv.snapshot().await?;
        Ok(scan_rows(&mut snapshot, &list::links(project, model)).await?)
    }

    /// A package's file. Its row says where the bytes are, in this project's own folder.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or there is no such package.
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
            // A type (a wall or floor type among them), the parameter table, a level and a view
            // have no shape: a package carries what is drawn, and an element reads them when it
            // is built, not when it is drawn.
            if !records::has_shape(&element) {
                continue;
            }
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the orderer rejects it.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or either hub doesn't permit it.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the file doesn't match its hash.
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

    /// Links a model to another model, or to a package in a folder.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails, or the orderer rejects it.
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

    /// Whether this person may open a project, for the live channel.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
    pub async fn may_open_project(&self, person: &Id, project: &Id) -> Result<(), Refusal> {
        self.require(person, "project.open", Target::Project(project))
            .await
    }

    /// Whether this person may watch a model's edits on the live channel.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
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

    /// The orderer of a project's log, starting its shapes keeping up with it.
    pub async fn orderer(&self, project: &Id) -> Arc<Orderer> {
        self.keep_up(project);
        self.orderers.get(&Scope::Project(project.clone())).await
    }

    /// Brings a project's shapes up to the end of its log now. A running server does this by
    /// itself; the seed and the operators' tools call it to know it's done.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
    pub async fn regenerate(&self, project: &Id) -> Result<(), StoreError> {
        self.regen.catch_up(project).await.map(|_| ())
    }

    /// How far a project's log has got.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store doesn't answer.
    pub async fn healthy(&self) -> Result<(), StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        snapshot.get(&KitPlace::Format.key()).await.map(|_| ())
    }

    /// How far a browser tab got. `person` is the one signed in on the connection.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails.
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
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when this person may not do it, or the store fails.
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
