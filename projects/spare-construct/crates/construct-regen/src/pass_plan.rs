//! What the builds of a pass read their neighbours from, read once and kept in step: each
//! document's walls (layered ones by where their lines run, [`WallIndex`]; the old box walls) and
//! rooms, what was worked out of them with the rows it read ([`crate::read_memo`]), each wall's
//! outline as a room reads it and each room's loop. A wall's joints ([`crate::joins`]) and a
//! room's loop ([`crate::rooms`]) are found among them, so a build reads what is near it and not
//! every record of the document.
//!
//! What it promises:
//! - A [`Plan`] holds exactly the walls and rooms a full read of the document gives at the state
//!   it was read at ([`Plan::read`]: as regeneration has reached them, by the kind index, or as
//!   they are now, by their pointers), each by ID and by the level its base stands on.
//! - Its [`Outlines`] hold each wall's outline as a room reads it, as it was last worked out: an
//!   old box wall's from its record, when it is listed; a layered wall's when it is built, and
//!   when a room reads its level whole. A layered wall listed again has none until one of those,
//!   so an outline held is the one working it out now would give: everything a layered wall's
//!   outline is made from (its record, its type, the parameters, its levels, the walls within two
//!   meetings of it) builds it again when it changes, before any room round it is found again
//!   (construct-regen's `regenerator`). Its loops hold each room's loop, or that it has none, as
//!   it was last built.
//! - The regenerator's [`Documents`] reads a document's plan through the pass's own transaction
//!   the first time a build needs it, and changes it with every kind row the pass writes
//!   ([`Documents::listed`], [`Documents::unlisted`]), so at any point of the pass it holds what
//!   reading the kind index again would give.
//! - [`Kept`] keeps each project's [`Documents`] between passes, as of the log number
//!   regeneration had reached when its pass committed. A pass takes them only when it reads that
//!   same number as where regeneration stands, in its own transaction: only a pass that moves
//!   that number on writes the kind index and the built rows, and clearing a project's derived
//!   rows takes the number away, so the kind index is then what it was when they were kept.
//!   Otherwise the pass reads afresh. A pass that does not commit keeps nothing; the least
//!   recently kept project goes first when more than [`KEPT_PROJECTS`] are kept.
//!
//! Depends on construct-model (records), construct-params (lengths), construct-rooms (whether
//! the room search takes an outline) and this crate's readers. Nothing here is interim.

use crate::joins::Fills;
use crate::parameters::{At, read_document};
use crate::read_memo::Written;
use crate::rooms::{Recipes, WALL_KIND, box_outline, level_of};
use crate::wall_index::{Grid, WallIndex};
use construct_model::element::Element;
use construct_model::keys::{Place, list};
use construct_params::Length;
use construct_rooms::{Bound, Outline, ROOM_KIND};
use kit_core::Id;
use kit_store::{Key, Reader, StoreError, from_row, read_row};
use std::collections::{BTreeMap, BTreeSet};

/// The operation of the old box wall, read by its box.
const STRAIGHT: &str = "wall.straight";

/// The most projects a regenerator keeps the plans of between passes.
pub const KEPT_PROJECTS: usize = 16;

/// Records by ID and by the level their base stands on.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    records: BTreeMap<Id, Element>,
    levels: BTreeMap<Option<Id>, BTreeSet<Id>>,
}

impl Listing {
    /// Holds `record` in place of what its element held.
    pub fn put(&mut self, record: Element) {
        self.remove(&record.id);
        self.levels
            .entry(level_of(&record))
            .or_default()
            .insert(record.id.clone());
        self.records.insert(record.id.clone(), record);
    }

    /// Holds element `id` no longer.
    pub fn remove(&mut self, id: &Id) {
        let Some(record) = self.records.remove(id) else {
            return;
        };
        let level = level_of(&record);
        if let Some(set) = self.levels.get_mut(&level) {
            set.remove(id);
            if set.is_empty() {
                self.levels.remove(&level);
            }
        }
    }

    /// The records whose base stands on `level`, in the order of their IDs.
    pub fn on_level(&self, level: Option<&Id>) -> impl Iterator<Item = &Element> {
        self.levels
            .get(&level.cloned())
            .into_iter()
            .flatten()
            .filter_map(|id| self.records.get(id))
    }

    /// How many records stand on `level`.
    #[must_use]
    pub fn count_on(&self, level: Option<&Id>) -> usize {
        self.levels.get(&level.cloned()).map_or(0, BTreeSet::len)
    }

    /// Every record held, in the order of their IDs.
    pub fn all(&self) -> impl Iterator<Item = &Element> {
        self.records.values()
    }
}

/// A plan box, its least and greatest corner in grid steps.
pub type Box2 = ([i64; 2], [i64; 2]);

/// The box of `corners`, if there are any.
#[must_use]
pub fn box_of(corners: &[[i64; 2]]) -> Option<Box2> {
    let first = *corners.first()?;
    let (mut low, mut high) = (first, first);
    for &[x, y] in corners {
        low = [low[0].min(x), low[1].min(y)];
        high = [high[0].max(x), high[1].max(y)];
    }
    Some((low, high))
}

/// Whether two boxes meet, edges included.
#[must_use]
pub fn boxes_meet(a: Box2, b: Box2) -> bool {
    let ((a0, a1), (b0, b1)) = (a, b);
    a0[0] <= b1[0] && b0[0] <= a1[0] && a0[1] <= b1[1] && b0[1] <= a1[1]
}

/// A box's two corners as the lengths a [`Grid`] lists.
fn as_lengths((low, high): Box2) -> [[Length; 2]; 2] {
    [low.map(Length::from_steps), high.map(Length::from_steps)]
}

/// A wall's outline as a room reads it, when it was last worked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The level its base stands on.
    pub level: Option<Id>,
    /// Its corners in the model's plan, in grid steps; `None` for a wall a room passes over.
    pub outline: Option<Vec<[i64; 2]>>,
    /// Whether the room search refuses it ([`construct_rooms::check`]): every room of its level
    /// then fails on it.
    pub bad: bool,
}

impl Shape {
    /// The box of its outline.
    #[must_use]
    pub fn bounds(&self) -> Option<Box2> {
        self.outline.as_deref().and_then(box_of)
    }
}

/// Each wall's outline as a room reads it, by ID and by where its box lies (the module header).
#[derive(Clone, Debug, Default)]
pub struct Outlines {
    shapes: BTreeMap<Id, Shape>,
    /// The walls with an outline, by its box.
    grid: Grid<Id>,
    /// How many walls of each level have a shape held.
    counts: BTreeMap<Option<Id>, usize>,
    /// The walls of each level the room search refuses.
    bad: BTreeMap<Option<Id>, BTreeSet<Id>>,
}

impl Outlines {
    /// Holds wall `id`'s outline, worked out now, in place of what it held.
    pub fn set(&mut self, id: &Id, level: Option<Id>, outline: Option<Vec<[i64; 2]>>) {
        self.forget(id);
        let bad = outline.as_ref().is_some_and(|corners| {
            let bound = Bound {
                wall: id.clone(),
                outline: Outline::Polygon(corners.clone()),
            };
            construct_rooms::check(&bound).is_err()
        });
        let shape = Shape {
            level,
            outline,
            bad,
        };
        if let Some(bounds) = shape.bounds() {
            self.grid
                .insert(id.clone(), shape.level.clone(), Some(as_lengths(bounds)));
        }
        if bad {
            self.bad
                .entry(shape.level.clone())
                .or_default()
                .insert(id.clone());
        }
        let count = self.counts.entry(shape.level.clone()).or_default();
        *count = count.saturating_add(1);
        self.shapes.insert(id.clone(), shape);
    }

    /// Holds no outline of wall `id`: it is to be worked out again.
    pub fn forget(&mut self, id: &Id) {
        let Some(shape) = self.shapes.remove(id) else {
            return;
        };
        self.grid.remove(id);
        if let Some(set) = self.bad.get_mut(&shape.level) {
            set.remove(id);
            if set.is_empty() {
                self.bad.remove(&shape.level);
            }
        }
        if let Some(count) = self.counts.get_mut(&shape.level) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.counts.remove(&shape.level);
            }
        }
    }

    /// What is held of wall `id`.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Shape> {
        self.shapes.get(id)
    }

    /// How many walls of `level` have a shape held.
    #[must_use]
    pub fn count_on(&self, level: Option<&Id>) -> usize {
        self.counts.get(&level.cloned()).copied().unwrap_or(0)
    }

    /// Whether the room search refuses a wall of `level`.
    #[must_use]
    pub fn any_bad(&self, level: Option<&Id>) -> bool {
        self.bad.contains_key(&level.cloned())
    }

    /// The walls of `level` with an outline whose box meets `area`, in the order of their IDs.
    #[must_use]
    pub fn near(&self, level: Option<&Id>, area: Box2) -> BTreeSet<Id> {
        let [low, high] = as_lengths(area);
        self.grid.in_box(level, low, high)
    }

    /// The walls of `level` with an outline, in the order of their IDs.
    pub fn outlined_on(&self, level: Option<&Id>) -> impl Iterator<Item = &Id> {
        self.grid.on_level(level)
    }
}

/// A room's loop as it was last built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loop {
    /// Its loop's box (its outer boundary's), in grid steps.
    Around(Box2),
    /// It has no loop: its build failed, with this code (`NotEnclosed`, `Redundant`, a
    /// refusal's).
    Failed(String),
}

/// One document's walls and rooms, and what was worked out of them (the module header).
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Its layered walls, by where their lines run.
    pub walls: WallIndex,
    /// Its old box walls (`wall.straight`), which a room reads by their box.
    pub boxes: Listing,
    /// Its rooms.
    pub rooms: Listing,
    /// Each neighbour's plan as the join rule reads it ([`crate::joins`]).
    pub fills: Fills,
    /// Each layered wall's recipe as a room reads it ([`crate::rooms`]).
    pub recipes: Recipes,
    /// Each wall's outline as a room reads it.
    pub outlines: Outlines,
    /// Each room's loop as it was last built.
    pub loops: BTreeMap<Id, Loop>,
    /// The rows of the kind index written since it was read ([`crate::read_memo`]).
    pub written: Written,
}

impl Plan {
    /// A document's walls and rooms as `at` that state: as regeneration has reached them, by the
    /// kind index (each listed record read once), or as they are now, by their pointers.
    ///
    /// # Errors
    /// A [`StoreError`] when the store fails or holds a record it can't read.
    pub async fn read<R: Reader>(
        reader: &mut R,
        project: &Id,
        document: &Id,
        at: At,
    ) -> Result<Self, StoreError> {
        let mut plan = Self::default();
        if at == At::Now {
            for (_, (_, record)) in read_document(reader, project, document).await? {
                plan.listed(&record, None);
            }
            return Ok(plan);
        }
        for kind in [WALL_KIND, ROOM_KIND] {
            let Ok(kind) = Id::parse(kind) else {
                continue;
            };
            for (key, row) in reader.scan(&list::kind(project, document, &kind)).await? {
                if !matches!(Place::parse(&key), Some(Place::Kind { .. })) {
                    continue;
                }
                let hash: String = from_row(&row)?;
                let hash = Id::parse(&hash).map_err(|e| StoreError::Damaged(e.to_string()))?;
                let record = kit_store::Place::Record {
                    project: project.clone(),
                    hash,
                };
                let record: Option<Element> = read_row(reader, &record.key()).await?;
                let record = record.ok_or_else(|| StoreError::Damaged("record missing".into()))?;
                plan.listed(&record, None);
            }
        }
        Ok(plan)
    }

    /// `record` is the one its element is listed by now, its kind row `written` when a pass wrote
    /// it. A layered wall's outline is to be worked out again; an old box wall's is its box.
    pub fn listed(&mut self, record: &Element, written: Option<&Key>) {
        if let Some(key) = written {
            self.written.wrote(key.clone());
        }
        self.walls.put(record.clone());
        self.outlines.forget(&record.id);
        if record.kind == WALL_KIND && record.recipe.op == STRAIGHT {
            self.boxes.put(record.clone());
            self.outlines
                .set(&record.id, level_of(record), box_outline(record));
        } else {
            self.boxes.remove(&record.id);
        }
        self.loops.remove(&record.id);
        if record.kind == ROOM_KIND {
            self.rooms.put(record.clone());
        } else {
            self.rooms.remove(&record.id);
        }
    }

    /// Element `id` is listed no longer: a pass deleted its kind row `key`. What was worked out
    /// of its record stays: it is kept with that record, and given again only for it.
    pub fn unlisted(&mut self, id: &Id, key: &Key) {
        self.written.wrote(key.clone());
        self.walls.remove(id);
        self.boxes.remove(id);
        self.rooms.remove(id);
        self.outlines.forget(id);
        self.loops.remove(id);
    }

    /// Whether every wall of `level` has its outline held and the room search takes each: a room
    /// on it is then found among them without reading a wall ([`crate::rooms`]).
    #[must_use]
    pub fn knows(&self, level: &Id) -> bool {
        let on = Some(level);
        let walls = self
            .walls
            .count_on(on)
            .saturating_add(self.boxes.count_on(on));
        self.outlines.count_on(on) == walls && !self.outlines.any_bad(on)
    }
}

/// The plans of the documents of a project as regeneration has reached them, for one pass of the
/// regenerator (the module header).
#[derive(Clone, Debug, Default)]
pub struct Documents {
    documents: BTreeMap<Id, Plan>,
}

impl Documents {
    /// The plan of `document` as regeneration has reached it, read through `reader` (the pass's
    /// own transaction) the first time it is asked for.
    ///
    /// # Errors
    /// A [`StoreError`] when the store fails or holds a record it can't read.
    pub async fn plan<R: Reader>(
        &mut self,
        reader: &mut R,
        project: &Id,
        document: &Id,
    ) -> Result<&mut Plan, StoreError> {
        if !self.documents.contains_key(document) {
            let read = Plan::read(reader, project, document, At::Regenerated).await?;
            self.documents.insert(document.clone(), read);
        }
        self.documents
            .get_mut(document)
            .ok_or_else(|| StoreError::Damaged("a document's plan was not kept".to_owned()))
    }

    /// The plan of `document`, if it was read.
    pub fn read_plan(&mut self, document: &Id) -> Option<&mut Plan> {
        self.documents.get_mut(document)
    }

    /// `record`'s kind row `key` of `document` is written: what is held of it follows. A document
    /// whose plan was not read yet needs nothing, since it will be read with this row in place.
    pub fn listed(&mut self, document: &Id, record: &Element, key: &Key) {
        if let Some(plan) = self.documents.get_mut(document) {
            plan.listed(record, Some(key));
        }
    }

    /// The kind row `key` of element `id` of `document` is deleted.
    pub fn unlisted(&mut self, document: &Id, id: &Id, key: &Key) {
        if let Some(plan) = self.documents.get_mut(document) {
            plan.unlisted(id, key);
        }
    }
}

/// What a regenerator keeps between passes (the module header).
#[derive(Debug, Default)]
pub struct Kept {
    tick: u64,
    /// By project: the log number reached, when they were last kept, and the plans.
    projects: BTreeMap<Id, (u64, u64, Documents)>,
}

impl Kept {
    /// The plans kept for `project` at log number `through`, taken out; none kept there, nothing.
    pub fn take(&mut self, project: &Id, through: u64) -> Documents {
        match self.projects.remove(project) {
            Some((at, _, documents)) if at == through => documents,
            _ => Documents::default(),
        }
    }

    /// Keeps `documents` as `project`'s plans at log number `through`.
    pub fn keep(&mut self, project: &Id, through: u64, documents: Documents) {
        self.tick = self.tick.saturating_add(1);
        self.projects
            .insert(project.clone(), (through, self.tick, documents));
        while self.projects.len() > KEPT_PROJECTS {
            let oldest = self
                .projects
                .iter()
                .min_by_key(|(_, (_, used, _))| *used)
                .map(|(id, _)| id.clone());
            match oldest {
                Some(id) => {
                    self.projects.remove(&id);
                }
                None => break,
            }
        }
    }
}
