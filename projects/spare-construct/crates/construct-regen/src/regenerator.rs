//! Keeps a project's derived shapes up to date. The orderer commits an edit's inputs at once and
//! leaves regeneration to this, as note A05 says, so a slow rebuild never holds up the log.
//!
//! Everything written here is derived: bodies and display records under their content IDs, what
//! each element built to, the area index, each document's elements listed by kind, and which
//! elements build from parameter values. It isn't in any log, because the log can make it again. [`Regenerator::rebuild`] does exactly that, and
//! the tests check it gives the same bytes.
//!
//! Large records go to file storage, linked by hash, because a store holds small values well: note
//! A06 measured a 16 MB record taking 60 ms to write, and a view rarely needs all of it.
//!
//! Note A05's checkpoints start here: each pass returns the bodies it built, and the server appends
//! them to the log in a checkpoint entry, which every browser compares with what it built itself.
//!
//! Types, a document's parameter table and its levels have no shape: they are listed by kind and
//! never built. A recipe that names a parameter is filled first ([`crate::parameters::filled`]),
//! and one that reads its top tie next ([`crate::levels::filled`]); an element's place is its
//! base level's ([`crate::levels::placed`]). Each element is listed under `reads` with every
//! record it reads (the parameter table, its type, its levels), so a change to any of them, or
//! its deletion, builds it again in the same pass. A shape is then the same however the log was
//! reached.
//!
//! A host is built with what is hosted on it ([`Geometry::build_hosting`]): a wall with each door
//! whose record names it as host ([`crate::hosting`]), in the order of their keys, each where its
//! record puts it in the wall's frame. The same `reads` rows keep this up to date: a hosted
//! element's row names its host, and its host's row lists it. So a change to a door builds its
//! wall again (the host it had and the host it has), and a change to a wall builds each of its
//! doors again, because a door's place is read from its wall: a door moves with its wall and its
//! record is not rewritten. Each such rebuild is one step deep, from the record each built row
//! names, as the elements of a changed type are.
//!
//! A room is found from the walls of its level ([`crate::rooms`]): its recipe is filled with its
//! loop as regeneration has reached those walls, and its `reads` row lists the walls the loop runs
//! along. A wall that changes, is made or is deleted builds again every room on the levels it was
//! and is on and every room whose row lists it; a room that changes builds again the other rooms
//! of its levels (which of two in one loop comes first). One step deep, as above. A room that is
//! not enclosed, or redundant, keeps its record and its built row says so.

use crate::hosting::{self, Hosting};
use crate::levels;
use crate::parameters::{self, At};
use crate::rooms;
use construct_model::element::{Element, Placement};
use construct_model::geometry::{Built, Edition, Geometry, GeometryError, Hosted, Recipe};
use construct_model::keys::{Place, list};
use construct_model::rows::{BuiltElement, BuiltResult, DisplayRef, Model, Reads, Stored};
use kit_core::Id;
use kit_ops::sha256_hex;
use kit_store::key::list as kit_list;
use kit_store::{FileStore, Kv, Reader, Scope, StoreError, Txn, from_row, read_row, to_row};
use kit_sync::read_log;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Entries taken in one transaction.
const BATCH: u64 = 256;
/// Rows cleared in one transaction by a rebuild.
const CLEAR_BATCH: usize = 512;
/// The most tries at reaching the end of a log others keep adding to.
const CATCH_UP_TRIES: usize = 8;
/// The most cells one element may index under. One request must not be able to ask for millions.
const MOST_CELLS: i64 = 4096;
/// Bytes kept in the store itself. Above this, a record goes to file storage.
const IN_STORE_BYTES: usize = 256 * 1024;
/// Cells are 5 m squares of a level of a document.
const CELL_MM: i64 = 5000;

fn damaged(what: impl std::fmt::Display) -> StoreError {
    StoreError::Damaged(what.to_string())
}

fn id(hash: &str) -> Result<Id, StoreError> {
    Id::parse(hash).map_err(damaged)
}

/// Every cell of a rectangle, as (x, y).
#[must_use]
pub fn cells_between(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let mut found = Vec::new();
    for x in x0.div_euclid(CELL_MM)..=x1.div_euclid(CELL_MM) {
        for y in y0.div_euclid(CELL_MM)..=y1.div_euclid(CELL_MM) {
            found.push((x, y));
        }
    }
    found
}

/// What one pass did: the body each element built to, by document, and how far it got.
#[derive(Debug, Default, PartialEq)]
pub struct Pass {
    /// The log number it regenerated through.
    pub through: u64,
    /// The body content ID each element built to, by document and then by element.
    pub bodies: BTreeMap<Id, BTreeMap<Id, Id>>,
}

/// Keeps one store's derived shapes up to date with its logs.
pub struct Regenerator<K: Kv, F: FileStore> {
    kv: K,
    files: F,
    geometry: Arc<dyn Geometry>,
}

impl<K: Kv, F: FileStore> Regenerator<K, F> {
    /// A regenerator over a store and its file storage, building with `geometry`.
    pub fn new(kv: K, files: F, geometry: Arc<dyn Geometry>) -> Self {
        Self {
            kv,
            files,
            geometry,
        }
    }

    /// Brings a project's shapes up to the end of its log. Safe to run on any server, any number
    /// of times: the store lets one pass win and the others start again from where it stopped.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails or holds a record it can't read.
    pub async fn catch_up(&self, project: &Id) -> Result<Pass, StoreError> {
        let mut pass = Pass::default();
        loop {
            let done = self.step(project, &mut pass).await?;
            if done {
                return Ok(pass);
            }
        }
    }

    /// Throws a project's derived rows away and makes them again from its log, which is only safe
    /// because nothing here is the truth. Files stay: they are named by their hash, so the same
    /// bytes come back to the same place. An operator's tool, for a project nobody is editing.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails or holds a record it can't read.
    pub async fn rebuild(&self, project: &Id) -> Result<Pass, StoreError> {
        let mut snapshot = self.kv.snapshot().await?;
        let mut keys = vec![
            Place::Regenerated {
                project: project.clone(),
            }
            .key(),
        ];
        for table in list::derived(project) {
            keys.extend(snapshot.scan(&table).await?.into_iter().map(|(key, _)| key));
        }
        for chunk in keys.chunks(CLEAR_BATCH) {
            let mut txn = self.kv.begin().await?;
            for key in chunk {
                txn.delete(key.clone()).await?;
            }
            txn.commit().await?;
        }
        self.catch_up(project).await
    }

    /// Builds each element's recipe again and compares it with what is stored, bytes included.
    /// Returns how many shapes it checked: types and the parameter table have none, and are
    /// passed over. A mismatch here is the failure checkpoints look for.
    ///
    /// # Errors
    ///
    /// What differs, or what could not be read, in words.
    pub async fn verify(&self, project: &Id) -> Result<usize, String> {
        let fault = |e: StoreError| e.to_string();
        let mut snapshot = self.caught_up(project).await.map_err(fault)?;
        let pairs = snapshot
            .scan(&kit_list::all_pieces(project))
            .await
            .map_err(fault)?;
        let mut checked = 0_usize;
        for (key, row) in &pairs {
            let Some(kit_store::Place::Piece {
                document, piece, ..
            }) = key.parse()
            else {
                return Err(format!("{key} isn't a piece"));
            };
            let hash = id(&from_row::<String>(row).map_err(fault)?).map_err(fault)?;
            let record = kit_store::Place::Record {
                project: project.clone(),
                hash: hash.clone(),
            };
            let element: Option<Element> = read_row(&mut snapshot, &record.key())
                .await
                .map_err(fault)?;
            let element = element.ok_or_else(|| format!("{piece}: its record is missing"))?;
            if !parameters::has_shape(&element) {
                continue;
            }
            let reads = Place::Reads {
                project: project.clone(),
                document: document.clone(),
                element: piece.clone(),
            };
            let reads: Option<Reads> =
                read_row(&mut snapshot, &reads.key()).await.map_err(fault)?;
            let hosted = reads.map(|r| r.hosted).unwrap_or_default();
            let place = Place::Built {
                project: project.clone(),
                document: document.clone(),
                element: piece.clone(),
            };
            let row: Option<BuiltElement> =
                read_row(&mut snapshot, &place.key()).await.map_err(fault)?;
            let row = row.ok_or_else(|| format!("{piece}: never regenerated"))?;
            if row.input != hash {
                return Err(format!("{piece}: built from an old record"));
            }
            let wanted = edition_of(&mut snapshot, project, &document)
                .await
                .map_err(fault)?;
            let inputs = inputs(
                &mut snapshot,
                project,
                &document,
                &wanted,
                &element,
                &hosted,
            )
            .await
            .map_err(fault)?;
            match (self.make(&wanted, &inputs), row.result) {
                (Ok((built, _)), BuiltResult::Shape { body, displays, .. }) => {
                    if built.body_id() != body.as_str() {
                        return Err(format!("{piece}: its body doesn't match its recipe"));
                    }
                    self.verify_stored(&mut snapshot, project, &built, &displays)
                        .await?;
                }
                (Err(error), BuiltResult::Failed { code, .. }) if error.code == code => {}
                _ => return Err(format!("{piece}: it built differently this time")),
            }
            checked = checked.saturating_add(1);
        }
        Ok(checked)
    }

    async fn verify_stored(
        &self,
        snapshot: &mut K::Snapshot,
        project: &Id,
        built: &Built,
        displays: &[DisplayRef],
    ) -> Result<(), String> {
        let fault = |e: StoreError| e.to_string();
        let mut records = vec![(
            Place::Body {
                project: project.clone(),
                id: id(&built.body_id()).map_err(fault)?,
            },
            built.body.clone(),
        )];
        for (display, built) in displays.iter().zip(&built.displays) {
            let place = Place::Display {
                project: project.clone(),
                id: display.id.clone(),
            };
            records.push((place, built.bytes()));
        }
        for (place, bytes) in records {
            let stored: Option<Stored> = read_row(snapshot, &place.key()).await.map_err(fault)?;
            let stored = stored.ok_or_else(|| format!("{}: not stored", place.key()))?;
            let hash = id(&sha256_hex(&bytes)).map_err(fault)?;
            if self.read_bytes(&stored, &hash).await.map_err(fault)? != bytes {
                return Err(format!("{}: stored bytes differ", place.key()));
            }
        }
        Ok(())
    }

    /// A snapshot in which regeneration has reached the end of the log. A running server may add
    /// to the log meanwhile, so this looks again a few times before giving up.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when the store fails, or when the log kept growing past every try.
    pub async fn caught_up(&self, project: &Id) -> Result<K::Snapshot, StoreError> {
        let scope = Scope::Project(project.clone());
        let cursor = Place::Regenerated {
            project: project.clone(),
        }
        .key();
        for _ in 0..CATCH_UP_TRIES {
            self.catch_up(project).await?;
            let mut snapshot = self.kv.snapshot().await?;
            let head: u64 = read_row(&mut snapshot, &scope.head()).await?.unwrap_or(0);
            let through: u64 = read_row(&mut snapshot, &cursor).await?.unwrap_or(0);
            if through >= head {
                return Ok(snapshot);
            }
        }
        Err(StoreError::Unavailable(
            "this project is being edited too quickly to stop and look at".to_owned(),
        ))
    }

    /// Builds an element for a document, with what is hosted on it, and works out where it sits. A
    /// document runs one edition; if this server has another, the element stays unbuilt and says
    /// so, because building it here would give different bytes under the same recipe.
    fn make(
        &self,
        wanted: &Edition,
        inputs: &Inputs,
    ) -> Result<(Built, Vec<(i64, i64)>), GeometryError> {
        if *wanted != self.geometry.edition() {
            return Err(GeometryError {
                code: "EditionUnavailable".to_owned(),
                message: format!(
                    "this server can't build {} {}",
                    wanted.engine, wanted.version
                ),
            });
        }
        let recipe = inputs.recipe.as_ref().map_err(Clone::clone)?;
        let placement = *inputs.placement.as_ref().map_err(Clone::clone)?;
        let hosted = inputs.hosted.as_ref().map_err(Clone::clone)?;
        let built = if hosted.is_empty() {
            self.geometry.build(recipe)?
        } else {
            let hosted: Vec<Hosted<'_>> = hosted
                .iter()
                .map(|h| Hosted {
                    key: &h.key,
                    recipe: &h.recipe,
                    at: h.at,
                })
                .collect();
            self.geometry.build_hosting(recipe, &hosted)?
        };
        let cells = occupied(placement, &built).ok_or_else(|| GeometryError {
            code: "TooBig".to_owned(),
            message: format!("an element may cover {MOST_CELLS} cells at most"),
        })?;
        Ok((built, cells))
    }

    /// One batch of the log, in one transaction. True when there's nothing more to do.
    async fn step(&self, project: &Id, pass: &mut Pass) -> Result<bool, StoreError> {
        let scope = Scope::Project(project.clone());
        let cursor = Place::Regenerated {
            project: project.clone(),
        }
        .key();
        let mut snapshot = self.kv.snapshot().await?;
        let through: u64 = read_row(&mut snapshot, &cursor).await?.unwrap_or(0);
        pass.through = through;
        let head: u64 = read_row(&mut snapshot, &scope.head()).await?.unwrap_or(0);
        if through >= head {
            return Ok(true);
        }
        let to = head.min(through.saturating_add(BATCH));
        let entries = read_log(&mut snapshot, &scope, through, to).await?;

        let mut txn = self.kv.begin().await?;
        let mut built = Vec::new();
        for entry in &entries {
            for change in &entry.changes {
                let result = self
                    .regenerate_with_readers(&mut txn, project, &entry.target, change, &mut built)
                    .await;
                if let Err(e) = result {
                    let _ = txn.rollback().await;
                    return Err(e);
                }
            }
        }
        if let Err(e) = txn.put(cursor, to_row(&to)?).await {
            let _ = txn.rollback().await;
            return Err(e);
        }
        match txn.commit().await {
            Ok(()) => {
                pass.through = to;
                for (document, element, body) in built {
                    let bodies = pass.bodies.entry(document).or_default();
                    match body {
                        Some(body) => bodies.insert(element, body),
                        None => bodies.remove(&element),
                    };
                }
                Ok(to >= head)
            }
            // Another server is on the same work, or the store blinked: look again.
            Err(StoreError::Conflict) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Rebuilds the element a change names, and then what reads it: if it is a record with no
    /// shape (a type, the parameter table, a level), every element listed as reading it; if it has
    /// a shape, the hosts it had and has, and the elements hosted on it. Each body built goes on
    /// `built`.
    async fn regenerate_with_readers(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        document: &Id,
        change: &kit_ops::Change,
        built: &mut Vec<(Id, Id, Option<Id>)>,
    ) -> Result<(), StoreError> {
        let (body, fed) = self
            .regenerate(
                txn,
                project,
                document,
                &change.piece,
                change.before.as_ref(),
                change.after.as_ref(),
            )
            .await?;
        built.push((document.clone(), change.piece.clone(), body));
        let read = match fed {
            None => return Ok(()),
            Some(Fed::Shape {
                hosts,
                hosted,
                spaces,
            }) => {
                self.dependents(txn, project, document, hosts.iter().chain(&hosted), built)
                    .await?;
                return self
                    .rooms_around(txn, project, document, &change.piece, &spaces, built)
                    .await;
            }
            Some(Fed::Read(read)) => read,
        };
        for (key, row) in txn.scan(&list::reads(project, document)).await? {
            let Some(Place::Reads { element, .. }) = Place::parse(&key) else {
                continue;
            };
            let reads: Reads = from_row(&row)?;
            if !reads.reads(read.as_str()) {
                continue;
            }
            let Some((body, fed)) = self.again(txn, project, document, &element).await? else {
                continue;
            };
            built.push((document.clone(), element, body));
            if let Some(Fed::Shape { hosts, hosted, .. }) = fed {
                self.dependents(txn, project, document, hosts.iter().chain(&hosted), built)
                    .await?;
            }
        }
        Ok(())
    }

    /// Builds again, one step deep, every room other than `changed` that stands on one of
    /// `spaces` (the levels a changed wall or room was and is on) or whose `reads` row lists
    /// `changed` (a wall its loop runs along), in key order ([`rooms`]).
    async fn rooms_around(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        document: &Id,
        changed: &Id,
        spaces: &[Id],
        built: &mut Vec<(Id, Id, Option<Id>)>,
    ) -> Result<(), StoreError> {
        let Ok(kind) = Id::parse(construct_rooms::ROOM_KIND) else {
            return Ok(());
        };
        let mut again = Vec::new();
        for (key, _) in txn.scan(&list::kind(project, document, &kind)).await? {
            let Some(Place::Kind { element, .. }) = Place::parse(&key) else {
                continue;
            };
            if element == *changed {
                continue;
            }
            let place = Place::Built {
                project: project.clone(),
                document: document.clone(),
                element: element.clone(),
            };
            let row: Option<BuiltElement> = read_row(txn, &place.key()).await?;
            let on = row
                .and_then(|row| row.level)
                .is_some_and(|level| spaces.contains(&level));
            let reads = Place::Reads {
                project: project.clone(),
                document: document.clone(),
                element: element.clone(),
            };
            let listed: Option<Reads> = read_row(txn, &reads.key()).await?;
            if on || listed.is_some_and(|r| r.reads(changed.as_str())) {
                again.push(element);
            }
        }
        self.dependents(txn, project, document, again.iter(), built)
            .await
    }

    /// Builds an element again from the record it had at this point in the log, which its built
    /// row names: its body, and what it feeds. Nothing when it has no built row.
    async fn again(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        document: &Id,
        element: &Id,
    ) -> Result<Option<(Option<Id>, Option<Fed>)>, StoreError> {
        let place = Place::Built {
            project: project.clone(),
            document: document.clone(),
            element: element.clone(),
        };
        let row: Option<BuiltElement> = read_row(txn, &place.key()).await?;
        let Some(row) = row else { return Ok(None) };
        let hash = row.input;
        let again = self
            .regenerate(txn, project, document, element, Some(&hash), Some(&hash))
            .await?;
        Ok(Some(again))
    }

    /// Builds each of `elements` again ([`Regenerator::again`]), one step deep: what they feed in
    /// turn is not built again for this.
    async fn dependents<'a>(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        document: &Id,
        elements: impl Iterator<Item = &'a Id>,
        built: &mut Vec<(Id, Id, Option<Id>)>,
    ) -> Result<(), StoreError> {
        for element in elements {
            if let Some((body, _)) = self.again(txn, project, document, element).await? {
                built.push((document.clone(), element.clone(), body));
            }
        }
        Ok(())
    }

    /// Rebuilds one element, returning its body if it has one, and what it feeds: for a record
    /// with no shape, its readers; for an element with a shape, its hosts and what it hosts.
    #[allow(
        clippy::too_many_lines,
        reason = "one element's derived rows, written in the order a reader of them needs"
    )]
    async fn regenerate(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        document: &Id,
        element_id: &Id,
        before: Option<&Id>,
        after: Option<&Id>,
    ) -> Result<(Option<Id>, Option<Fed>), StoreError> {
        let reads = Place::Reads {
            project: project.clone(),
            document: document.clone(),
            element: element_id.clone(),
        };
        // What is hosted on it is listed by what is hosted, so it stays through its own rebuild.
        let kept: Option<Reads> = read_row(txn, &reads.key()).await?;
        let hosted = kept.map(|kept| kept.hosted).unwrap_or_default();
        txn.delete(reads.key()).await?;
        let place = Place::Built {
            project: project.clone(),
            document: document.clone(),
            element: element_id.clone(),
        };
        let old: Option<BuiltElement> = read_row(txn, &place.key()).await?;
        if let Some(old) = &old
            && let Some(level) = &old.level
        {
            for cell in area_of(project, document, level, &old.cells, element_id) {
                txn.delete(cell.key()).await?;
            }
        }
        // Records are never deleted, so the old one still says what kind it was. A record with no
        // shape has no built row, so the change's own `before` names it.
        let mut old_host = None;
        let mut was_read = None;
        let mut spaces: Vec<Id> = Vec::new();
        if let Some(hash) = old.as_ref().map(|old| &old.input).or(before) {
            let record = kit_store::Place::Record {
                project: project.clone(),
                hash: hash.clone(),
            };
            let before: Option<Element> = read_row(txn, &record.key()).await?;
            let before = before.ok_or_else(|| damaged("an element's old record is missing"))?;
            if let Some(kind) = kind_row(project, document, &before) {
                txn.delete(kind.key()).await?;
            }
            old_host = hosting::host_of(&before);
            if rooms::is_spatial(&before.kind)
                && let Some(level) = old.as_ref().and_then(|old| old.level.clone())
            {
                spaces.push(level);
            }
            if !parameters::has_shape(&before) {
                was_read = Some(before.id);
            }
        }
        let Some(after) = after else {
            txn.delete(place.key()).await?;
            // Whatever read a record that is gone reads what stands in its place now.
            if let Some(read) = was_read {
                return Ok((None, Some(Fed::Read(read))));
            }
            if let Some(host) = &old_host {
                list_hosted(txn, project, document, host, element_id, false).await?;
            }
            // What is still hosted on it stays listed, so it is cut again if it comes back.
            let rest = Reads {
                hosted: hosted.clone(),
                ..Reads::default()
            };
            if !rest.is_empty() {
                txn.put(reads.key(), to_row(&rest)?).await?;
            }
            let hosts = old_host.into_iter().collect();
            return Ok((
                None,
                Some(Fed::Shape {
                    hosts,
                    hosted: ids(&hosted)?,
                    spaces,
                }),
            ));
        };

        let record = kit_store::Place::Record {
            project: project.clone(),
            hash: after.clone(),
        };
        let element: Option<Element> = read_row(txn, &record.key()).await?;
        let element = element.ok_or_else(|| damaged("an element's record is missing"))?;
        if let Some(kind) = kind_row(project, document, &element) {
            txn.put(kind.key(), to_row(&after.as_str())?).await?;
        }
        if !parameters::has_shape(&element) {
            txn.delete(place.key()).await?;
            return Ok((None, Some(Fed::Read(element.id.clone()))));
        }
        let new_host = hosting::host_of(&element);
        if let Some(host) = &old_host
            && old_host != new_host
        {
            list_hosted(txn, project, document, host, element_id, false).await?;
        }
        if let Some(host) = &new_host {
            list_hosted(txn, project, document, host, element_id, true).await?;
        }
        let edition = edition_of(txn, project, document).await?;
        let inputs = inputs(txn, project, document, &edition, &element, &hosted).await?;
        let mut records = reads_of(&element);
        // A room reads the walls its loop runs along, as its filled recipe names them.
        if let Ok(recipe) = &inputs.recipe {
            records.extend(rooms::walls_read(recipe));
        }
        let row = Reads {
            records,
            host: new_host.as_ref().map(ToString::to_string),
            hosted: hosted.clone(),
        };
        if !row.is_empty() {
            txn.put(reads.key(), to_row(&row)?).await?;
        }
        let made = self.make(&edition, &inputs);
        let placed = inputs.placed;
        let (result, cells, body) = match made {
            // An element on no level has a shape and no place in any plan.
            Ok((built, cells)) => {
                let cells = if placed.level.is_some() {
                    cells
                } else {
                    Vec::new()
                };
                let body = id(&built.body_id())?;
                let result = self.store_shape(txn, project, built).await?;
                (result, cells, Some(body))
            }
            Err(error) => {
                let result = BuiltResult::Failed {
                    code: error.code,
                    message: error.message,
                };
                (result, Vec::new(), None)
            }
        };
        if let Some(level) = &placed.level {
            for cell in area_of(project, document, level, &cells, element_id) {
                txn.put(cell.key(), to_row(&after.as_str())?).await?;
            }
        }
        let row = BuiltElement {
            input: after.clone(),
            edition,
            level: placed.level.clone(),
            elevation: placed.elevation,
            cells,
            result,
        };
        txn.put(place.key(), to_row(&row)?).await?;
        let mut hosts: Vec<Id> = old_host.into_iter().chain(new_host).collect();
        hosts.dedup();
        if rooms::is_spatial(&element.kind)
            && let Some(level) = &placed.level
            && !spaces.contains(level)
        {
            spaces.push(level.clone());
        }
        Ok((
            body,
            Some(Fed::Shape {
                hosts,
                hosted: ids(&hosted)?,
                spaces,
            }),
        ))
    }

    async fn store_shape(
        &self,
        txn: &mut K::Txn,
        project: &Id,
        built: Built,
    ) -> Result<BuiltResult, StoreError> {
        let body = id(&built.body_id())?;
        let key = Place::Body {
            project: project.clone(),
            id: body.clone(),
        }
        .key();
        let stored = self
            .place_bytes("bodies", body.as_str(), &built.body)
            .await?;
        txn.put(key, to_row(&stored)?).await?;
        let mut displays = Vec::new();
        for display in &built.displays {
            let bytes = display.bytes();
            let display_id = id(&sha256_hex(&bytes))?;
            let stored = self
                .place_bytes("displays", display_id.as_str(), &bytes)
                .await?;
            let key = Place::Display {
                project: project.clone(),
                id: display_id.clone(),
            }
            .key();
            txn.put(key, to_row(&stored)?).await?;
            displays.push(DisplayRef {
                lod: display.lod,
                deviation: display.deviation,
                id: display_id,
            });
        }
        Ok(BuiltResult::Shape {
            body,
            bounds: built.bounds,
            names: built.names,
            displays,
        })
    }

    /// Decides where bytes live. A file is named by its hash, so writing it twice does nothing.
    async fn place_bytes(
        &self,
        folder: &str,
        id: &str,
        bytes: &[u8],
    ) -> Result<Stored, StoreError> {
        if bytes.len() <= IN_STORE_BYTES {
            return Ok(Stored::Here {
                hex: hex::encode(bytes),
            });
        }
        let path = format!("{folder}/{id}");
        self.files.put(&path, bytes.to_vec()).await?;
        Ok(Stored::File {
            path,
            size: bytes.len() as u64,
        })
    }

    /// Reads bytes back, checking them against their name.
    ///
    /// # Errors
    ///
    /// A [`StoreError`] when they can't be read, or when they don't match their content ID.
    pub async fn read_bytes(&self, stored: &Stored, id: &Id) -> Result<Vec<u8>, StoreError> {
        let bytes = match stored {
            Stored::Here { hex } => hex::decode(hex).map_err(damaged)?,
            Stored::File { path, .. } => self.files.get(path).await?,
        };
        if sha256_hex(&bytes) != id.as_str() {
            return Err(damaged("a stored shape doesn't match its content ID"));
        }
        Ok(bytes)
    }
}

/// What a record feeds: for a record with no shape (a type, the parameter table, a level), every
/// element whose `reads` row lists it; for an element with a shape, the hosts it had and has, and
/// what is hosted on it.
enum Fed {
    Read(Id),
    Shape {
        /// The elements it was or is hosted on.
        hosts: Vec<Id>,
        /// The elements hosted on it, in key order.
        hosted: Vec<Id>,
        /// For a wall or a room ([`rooms::is_spatial`]), the levels it was and is on: the rooms
        /// on them are built again.
        spaces: Vec<Id>,
    },
}

/// IDs as a reads row lists them.
fn ids(listed: &[String]) -> Result<Vec<Id>, StoreError> {
    listed.iter().map(|key| id(key)).collect()
}

/// Lists `element` among those hosted on `host` (`hosted` true), or takes it off that list, in
/// `host`'s reads row, kept in key order.
async fn list_hosted<T: Txn>(
    txn: &mut T,
    project: &Id,
    document: &Id,
    host: &Id,
    element: &Id,
    hosted: bool,
) -> Result<(), StoreError> {
    let place = Place::Reads {
        project: project.clone(),
        document: document.clone(),
        element: host.clone(),
    };
    let row: Option<Reads> = read_row(txn, &place.key()).await?;
    let mut row = row.unwrap_or_default();
    let key = element.to_string();
    match (row.hosted.binary_search(&key), hosted) {
        (Err(at), true) => row.hosted.insert(at, key),
        (Ok(at), false) => {
            row.hosted.remove(at);
        }
        _ => return Ok(()),
    }
    if row.is_empty() {
        txn.delete(place.key()).await
    } else {
        txn.put(place.key(), to_row(&row)?).await
    }
}

/// An element hosted on another, as its host is built with it.
struct HostedInput {
    key: String,
    recipe: Recipe,
    at: Placement,
}

/// What an element is built from: its recipe, the elements hosted on it, where it sits, and the
/// level and base height its built row records.
struct Inputs {
    recipe: Result<Recipe, GeometryError>,
    placement: Result<Placement, GeometryError>,
    hosted: Result<Vec<HostedInput>, GeometryError>,
    placed: Placed,
}

/// What `element` is built from, as regeneration has reached: its filled recipe ([`input_of`]),
/// where it sits (its own placement, or for an element hosted on a face, read from its host),
/// each element `hosted` lists as hosted on it, and its level. A failure of any is the element's
/// build failure.
async fn inputs<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    edition: &Edition,
    element: &Element,
    hosted: &[String],
) -> Result<Inputs, StoreError> {
    let (recipe, placed) = input_of(reader, project, document, element).await?;
    let (placement, hosted) = match &recipe {
        Ok(own) => (
            placement_of(reader, project, document, edition, element, own).await?,
            hosted_in(reader, project, document, edition, element, own, hosted).await?,
        ),
        Err(_) => (Ok(element.placement), Ok(Vec::new())),
    };
    Ok(Inputs {
        recipe,
        placement,
        hosted,
        placed,
    })
}

/// The record an element was built from, as its built row names it: what regeneration has
/// reached, in log order.
async fn built_record<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Id,
) -> Result<Option<Element>, StoreError> {
    let place = Place::Built {
        project: project.clone(),
        document: document.clone(),
        element: element.clone(),
    };
    let row: Option<BuiltElement> = read_row(reader, &place.key()).await?;
    let Some(row) = row else { return Ok(None) };
    let record = kit_store::Place::Record {
        project: project.clone(),
        hash: row.input,
    };
    let record: Option<Element> = read_row(reader, &record.key()).await?;
    record
        .ok_or_else(|| damaged("an element's record is missing"))
        .map(Some)
}

/// Where `element`, of the filled recipe `own`, sits: its own placement, or, hosted on a face,
/// its host's placement and its offsets ([`hosting::at`]).
async fn placement_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    edition: &Edition,
    element: &Element,
    own: &Recipe,
) -> Result<Result<Placement, GeometryError>, StoreError> {
    let hosting = match hosting::read(element) {
        Ok(Some(hosting @ Hosting::OnFace { .. })) => hosting,
        Ok(_) => return Ok(Ok(element.placement)),
        Err(why) => {
            return Ok(Err(GeometryError {
                code: "BadHost".to_owned(),
                message: why,
            }));
        }
    };
    let Some(host) = built_record(reader, project, document, hosting.host()).await? else {
        return Ok(Err(GeometryError {
            code: "HostMissing".to_owned(),
            message: format!(
                "door {}: its host {} is not in the model",
                element.id,
                hosting.host()
            ),
        }));
    };
    let host_recipe = match input_of(reader, project, document, &host).await?.0 {
        Ok(recipe) => recipe,
        Err(e) => {
            return Ok(Err(GeometryError {
                code: e.code,
                message: format!("door {}: its host {}: {}", element.id, host.name, e.message),
            }));
        }
    };
    Ok(
        hosting::at(edition, &host, &host_recipe, element, own, &hosting)
            .and_then(|at| hosting::in_model(host.placement, at)),
    )
}

/// The elements `listed` as hosted on `host`, of the filled recipe `own`, each with its filled
/// recipe and where it sits in the host's frame, in key order. One that no longer names `host`
/// is passed over; one that cannot be placed is the host's build failure, naming it.
async fn hosted_in<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    edition: &Edition,
    host: &Element,
    own: &Recipe,
    listed: &[String],
) -> Result<Result<Vec<HostedInput>, GeometryError>, StoreError> {
    let mut found = Vec::new();
    for key in listed {
        let Some(record) = built_record(reader, project, document, &id(key)?).await? else {
            continue;
        };
        let hosting = match hosting::read(&record) {
            Ok(Some(hosting)) if *hosting.host() == host.id => hosting,
            _ => continue,
        };
        let recipe = match input_of(reader, project, document, &record).await?.0 {
            Ok(recipe) => recipe,
            Err(e) => {
                return Ok(Err(GeometryError {
                    code: e.code,
                    message: format!("door {key}: {}", e.message),
                }));
            }
        };
        match hosting::at(edition, host, own, &record, &recipe, &hosting) {
            Ok(at) => found.push(HostedInput {
                key: key.clone(),
                recipe,
                at,
            }),
            Err(e) => return Ok(Err(e)),
        }
    }
    Ok(Ok(found))
}

/// Every record an element's shape or place reads, by ID: the parameter table and its type when
/// its recipe names a parameter or reads its wall type's layers ([`parameters::reads_records`]),
/// and its levels ([`levels::levels_read`]). Its `reads` row; a room's adds the walls its loop
/// runs along ([`rooms::walls_read`]), once its recipe is filled.
fn reads_of(element: &Element) -> BTreeSet<String> {
    let mut read: BTreeSet<String> = levels::levels_read(element)
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    if parameters::reads_records(&element.recipe) {
        read.insert(parameters::TABLE_ID.to_owned());
        if let Some(type_id) = parameters::type_of(element) {
            read.insert(type_id.to_string());
        }
    }
    read
}

/// Where a built row says an element stands: the level its cells are under, and its base height
/// in stored length text.
struct Placed {
    level: Option<Id>,
    elevation: Option<String>,
}

/// What an element is built from and where it stands, as of how far regeneration has got: its
/// recipe with every argument that names a parameter or a tie filled, and its level and base
/// height. A failure to read its levels is its shape's failure; its level is then still the one
/// its tie names, where that can be read.
async fn input_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
) -> Result<(Result<Recipe, GeometryError>, Placed), StoreError> {
    let at = parameters::At::Regenerated;
    let placed = levels::placed(reader, project, document, element, at).await?;
    let recipe = recipe_of(reader, project, document, element).await?;
    Ok(match placed {
        Ok(placed) => {
            let recipe = recipe.and_then(|recipe| levels::filled(&recipe, &placed));
            // A room's loop is found from the walls of its level once its height is read.
            let recipe = match recipe {
                Ok(recipe) => {
                    rooms::filled(
                        reader,
                        project,
                        document,
                        element,
                        recipe,
                        placed.level.as_ref(),
                    )
                    .await?
                }
                Err(e) => Err(e),
            };
            let elevation = placed.stored_elevation();
            (
                recipe,
                Placed {
                    level: placed.level,
                    elevation,
                },
            )
        }
        Err(why) => {
            let level = construct_levels::base_of(element)
                .ok()
                .flatten()
                .map(|tie| tie.level);
            (
                Err(why),
                Placed {
                    level,
                    elevation: None,
                },
            )
        }
    })
}

/// The recipe an element is built from: its own, with every argument that names a parameter
/// filled as the document stood when regeneration reached it ([`parameters::recipe_at`]).
async fn recipe_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
    element: &Element,
) -> Result<Result<Recipe, GeometryError>, StoreError> {
    parameters::recipe_at(reader, project, document, element, At::Regenerated).await
}

/// The edition a document records.
async fn edition_of<R: Reader>(
    reader: &mut R,
    project: &Id,
    document: &Id,
) -> Result<Edition, StoreError> {
    let place = Place::Model {
        project: project.clone(),
        model: document.clone(),
    };
    let model: Option<Model> = read_row(reader, &place.key()).await?;
    Ok(model
        .ok_or_else(|| damaged("an element's document is missing"))?
        .edition)
}

/// Where an element is listed by its kind. Every element is, built or not; a kind that isn't a
/// plain name (letters, digits, `.`, `_` and `-`) can't be part of a key, so it isn't listed.
/// The plan index rows of an element in `cells` on `level`.
fn area_of<'a>(
    project: &'a Id,
    document: &'a Id,
    level: &'a Id,
    cells: &'a [(i64, i64)],
    element: &'a Id,
) -> impl Iterator<Item = Place> + 'a {
    cells.iter().map(move |&(x, y)| Place::Area {
        project: project.clone(),
        document: document.clone(),
        level: level.clone(),
        x,
        y,
        element: element.clone(),
    })
}

fn kind_row(project: &Id, document: &Id, element: &Element) -> Option<Place> {
    Some(Place::Kind {
        project: project.clone(),
        document: document.clone(),
        kind: Id::parse(&element.kind).ok()?,
        element: element.id.clone(),
    })
}

/// The cells a shape placed at `at` touches, unless there are too many.
fn occupied(at: Placement, built: &Built) -> Option<Vec<(i64, i64)>> {
    let (x0, y0) = (
        at.x.saturating_add(built.bounds.min[0]),
        at.y.saturating_add(built.bounds.min[1]),
    );
    let (x1, y1) = (
        at.x.saturating_add(built.bounds.max[0]),
        at.y.saturating_add(built.bounds.max[1]),
    );
    let across = |a: i64, b: i64| {
        b.div_euclid(CELL_MM)
            .saturating_sub(a.div_euclid(CELL_MM))
            .saturating_add(1)
    };
    (across(x0, x1).saturating_mul(across(y0, y1)) <= MOST_CELLS)
        .then(|| cells_between(x0, y0, x1, y1))
}
