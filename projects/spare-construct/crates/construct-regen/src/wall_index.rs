//! The layered walls of one document by where their lines run: what the joins search
//! ([`crate::joins`]) and a room's walls ([`crate::rooms`]) are read from, so that building a wall
//! reads the walls near it and not every wall of the document.
//!
//! What it promises:
//! - A [`WallIndex`] holds exactly the walls a full read of the document's walls gives at the
//!   state it was read at ([`WallIndex::read`]: as regeneration has reached them, by the kind
//!   index, or as they are now, by their pointers): every record listed under the kind `wall`
//!   whose recipe is a layered wall's (`wall.layered`), by ID. The regenerator reads one once and
//!   keeps it in step with every kind row it writes ([`WallIndex::put`], [`WallIndex::remove`]),
//!   so at any point of a pass it holds what a full read would give there.
//! - [`WallIndex::within_two`] gives the walls of a wall's base level within two meetings of it
//!   (the walls its line meets, and the walls theirs meet), found by construct-walls'
//!   [`lines_meet`], the test a full read applies to every wall: the same set. The walls tested
//!   are those listed in the cells of a grid (5 m squares of each level) that the line's box
//!   touches, each line's box grown by a millimetre before it is listed. Two lines meet only where
//!   they come within the joins tolerance (0.01 mm) of each other, so their grown boxes share a
//!   point and both are listed in that point's cell: no wall that meets is passed over. A line
//!   whose box would cover more than [`MOST_CELLS`] cells is listed apart and tested by every
//!   search of its level.
//! - Everything is kept and answered in the order of the walls' IDs, so what is read from it is
//!   what a full read gives, in the same order.
//!
//! The same grid ([`Grid`]) finds the walls within two meetings of each wall of a level that a
//! reader holds in hand (construct-regen's `joins::on_level`).
//!
//! Depends on construct-model (records), construct-params (lengths), construct-levels (an
//! element's base level), construct-walls (the meeting test) and this crate's readers. Nothing
//! here is interim.

use crate::parameters::{At, read_document};
use crate::wall_plan;
use construct_model::element::{Element, Placement};
use construct_model::keys::{Place, list};
use construct_params::Length;
use construct_walls::lines_meet;
use kit_core::Id;
use kit_store::{Reader, StoreError, from_row, read_row};
use std::collections::{BTreeMap, BTreeSet};

/// The kind layered walls are listed under.
pub const WALL_KIND: &str = "wall";

/// The most cells one line is listed in; a line whose box covers more is listed apart.
pub const MOST_CELLS: i64 = 1024;

/// A wall's line in the model's plan, in steps.
pub type Line = [[Length; 2]; 2];

/// The side of a cell of the grid: 5 m.
const CELL: Length = Length::from_mm(5000);
/// How far a line's box is grown before it is listed: a millimetre, beyond the joins tolerance.
const GROWN: Length = Length::from_mm(1);

/// A cell of the grid: the level (`None` on no level) and the cell's column and row.
type Cell = (Option<Id>, i64, i64);

/// A placement's plan corner in steps.
fn placed_at(placement: &Placement) -> Option<[i64; 2]> {
    Some([
        Length::checked_from_mm(placement.x)?.steps(),
        Length::checked_from_mm(placement.y)?.steps(),
    ])
}

/// `point` of an element's frame in the model's plan.
fn in_model(point: [i64; 2], at: [i64; 2]) -> Option<[Length; 2]> {
    let ([x, y], [ax, ay]) = (point, at);
    Some([
        Length::from_steps(x.checked_add(ax)?),
        Length::from_steps(y.checked_add(ay)?),
    ])
}

/// The line of a layered wall's record in the model's plan, read without filling anything.
#[must_use]
pub fn line_in_model(element: &Element) -> Option<Line> {
    let (start, end) = wall_plan::line_of(&element.recipe)?;
    let at = placed_at(&element.placement)?;
    Some([in_model(start, at)?, in_model(end, at)?])
}

/// The level an element's base stands on, as its record says.
#[must_use]
pub fn base_level(element: &Element) -> Option<Id> {
    construct_levels::base_of(element)
        .ok()
        .flatten()
        .map(|tie| tie.level)
}

/// Whether a record is one a [`WallIndex`] holds: a layered wall listed under the kind `wall`.
fn is_listed(element: &Element) -> bool {
    element.kind == WALL_KIND && wall_plan::is_layered(&element.recipe)
}

/// The cells of the grid a line's box, grown by [`GROWN`], covers on `level`, or `None` when it
/// would cover more than [`MOST_CELLS`].
fn cells_of(level: Option<&Id>, line: Line) -> Option<Vec<Cell>> {
    let [[x0, y0], [x1, y1]] = line.map(|point| point.map(Length::steps));
    let grow = GROWN.steps();
    let side = CELL.steps();
    let cell = |v: i64| v.checked_div_euclid(side);
    let (low_x, high_x) = (x0.min(x1).checked_sub(grow)?, x0.max(x1).checked_add(grow)?);
    let (low_y, high_y) = (y0.min(y1).checked_sub(grow)?, y0.max(y1).checked_add(grow)?);
    let (cx0, cx1) = (cell(low_x)?, cell(high_x)?);
    let (cy0, cy1) = (cell(low_y)?, cell(high_y)?);
    let across = cx1.checked_sub(cx0)?.checked_add(1)?;
    let up = cy1.checked_sub(cy0)?.checked_add(1)?;
    if across.checked_mul(up)? > MOST_CELLS {
        return None;
    }
    let mut cells = Vec::new();
    for x in cx0..=cx1 {
        for y in cy0..=cy1 {
            cells.push((level.cloned(), x, y));
        }
    }
    Some(cells)
}

/// One line as a [`Grid`] holds it.
#[derive(Clone, Debug)]
struct Entry {
    level: Option<Id>,
    line: Option<Line>,
    /// The cells it is listed in; `None` when it is listed apart, or has no line.
    cells: Option<Vec<Cell>>,
}

/// Lines by key, on levels, listed in the cells of the grid their boxes touch (the module
/// header): which lines meet a line is answered from the lines near it.
#[derive(Clone, Debug)]
pub struct Grid<K> {
    entries: BTreeMap<K, Entry>,
    /// The keys listed in each cell.
    cells: BTreeMap<Cell, BTreeSet<K>>,
    /// The keys of each level whose lines' boxes cover too many cells to list.
    apart: BTreeMap<Option<Id>, BTreeSet<K>>,
    /// The keys of each level, line or none.
    levels: BTreeMap<Option<Id>, BTreeSet<K>>,
}

impl<K> Default for Grid<K> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            cells: BTreeMap::new(),
            apart: BTreeMap::new(),
            levels: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone> Grid<K> {
    /// Lists `key` on `level` with its line, if it has one, in place of what it was listed with.
    pub fn insert(&mut self, key: K, level: Option<Id>, line: Option<Line>) {
        self.remove(&key);
        let cells = line.and_then(|line| cells_of(level.as_ref(), line));
        match &cells {
            Some(cells) => {
                for cell in cells {
                    self.cells
                        .entry(cell.clone())
                        .or_default()
                        .insert(key.clone());
                }
            }
            None if line.is_some() => {
                self.apart
                    .entry(level.clone())
                    .or_default()
                    .insert(key.clone());
            }
            None => {}
        }
        self.levels
            .entry(level.clone())
            .or_default()
            .insert(key.clone());
        self.entries.insert(key, Entry { level, line, cells });
    }

    /// Takes `key` off every list.
    pub fn remove(&mut self, key: &K) {
        let Some(entry) = self.entries.remove(key) else {
            return;
        };
        for cell in entry.cells.iter().flatten() {
            if let Some(set) = self.cells.get_mut(cell) {
                set.remove(key);
                if set.is_empty() {
                    self.cells.remove(cell);
                }
            }
        }
        for map in [&mut self.apart, &mut self.levels] {
            if let Some(set) = map.get_mut(&entry.level) {
                set.remove(key);
                if set.is_empty() {
                    map.remove(&entry.level);
                }
            }
        }
    }

    /// The keys listed on `level`, in order.
    pub fn on_level(&self, level: Option<&Id>) -> impl Iterator<Item = &K> {
        self.levels.get(&level.cloned()).into_iter().flatten()
    }

    /// The keys of `level` other than `me` whose lines meet `line`, in order.
    #[must_use]
    pub fn meeting(&self, level: Option<&Id>, line: Line, me: &K) -> BTreeSet<K> {
        let key = level.cloned();
        let mut tested: BTreeSet<&K> = BTreeSet::new();
        match cells_of(level, line) {
            Some(cells) => {
                for cell in &cells {
                    tested.extend(self.cells.get(cell).into_iter().flatten());
                }
            }
            // A line too long to look up cell by cell is tested against every line of its level.
            None => tested.extend(self.levels.get(&key).into_iter().flatten()),
        }
        tested.extend(self.apart.get(&key).into_iter().flatten());
        tested
            .into_iter()
            .filter(|other| *other != me)
            .filter(|other| {
                self.entries
                    .get(*other)
                    .and_then(|entry| entry.line)
                    .is_some_and(|theirs| lines_meet(line, theirs))
            })
            .cloned()
            .collect()
    }

    /// The keys of `level` whose lines' boxes meet the box from `low` to `high` (edges included),
    /// in order.
    #[must_use]
    pub fn in_box(&self, level: Option<&Id>, low: [Length; 2], high: [Length; 2]) -> BTreeSet<K> {
        let key = level.cloned();
        let mut tested: BTreeSet<&K> = BTreeSet::new();
        match cells_of(level, [low, high]) {
            Some(cells) => {
                for cell in &cells {
                    tested.extend(self.cells.get(cell).into_iter().flatten());
                }
            }
            None => tested.extend(self.levels.get(&key).into_iter().flatten()),
        }
        tested.extend(self.apart.get(&key).into_iter().flatten());
        let meets = |line: Line| {
            let [[x0, y0], [x1, y1]] = line;
            x0.min(x1) <= high[0]
                && low[0] <= x0.max(x1)
                && y0.min(y1) <= high[1]
                && low[1] <= y0.max(y1)
        };
        tested
            .into_iter()
            .filter(|other| {
                self.entries
                    .get(*other)
                    .and_then(|entry| entry.line)
                    .is_some_and(meets)
            })
            .cloned()
            .collect()
    }

    /// How many keys are listed on `level`.
    #[must_use]
    pub fn count_on(&self, level: Option<&Id>) -> usize {
        self.levels.get(&level.cloned()).map_or(0, BTreeSet::len)
    }

    /// The keys of `level` within two meetings of the line `line` of `me`: the lines it meets, and
    /// the lines theirs meet, other than `me`, in order.
    #[must_use]
    pub fn within_two(&self, level: Option<&Id>, line: Line, me: &K) -> BTreeSet<K> {
        let first = self.meeting(level, line, me);
        let mut all = first.clone();
        for key in &first {
            if let Some(theirs) = self.entries.get(key).and_then(|entry| entry.line) {
                all.extend(self.meeting(level, theirs, me));
            }
        }
        all
    }
}

/// The layered walls of one document, by ID and by where their lines run (the module header).
#[derive(Clone, Debug, Default)]
pub struct WallIndex {
    walls: BTreeMap<Id, Element>,
    grid: Grid<Id>,
}

impl WallIndex {
    /// The walls of a document as `at` that state: as regeneration has reached them, by the kind
    /// index (each listed record read once), or as they are now, by their pointers.
    ///
    /// # Errors
    /// A [`StoreError`] when the store fails or holds a record it can't read.
    pub async fn read<R: Reader>(
        reader: &mut R,
        project: &Id,
        document: &Id,
        at: At,
    ) -> Result<Self, StoreError> {
        let mut index = Self::default();
        if at == At::Now {
            for (_, (_, record)) in read_document(reader, project, document).await? {
                index.put(record);
            }
            return Ok(index);
        }
        let Ok(kind) = Id::parse(WALL_KIND) else {
            return Ok(index);
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
            index.put(record);
        }
        Ok(index)
    }

    /// Takes `record` as the one its element is listed by now: held when it is a layered wall,
    /// else no longer held.
    pub fn put(&mut self, record: Element) {
        if !is_listed(&record) {
            self.remove(&record.id);
            return;
        }
        let (level, line) = (base_level(&record), line_in_model(&record));
        self.grid.insert(record.id.clone(), level, line);
        self.walls.insert(record.id.clone(), record);
    }

    /// Takes the element `id` out: its kind row is gone.
    pub fn remove(&mut self, id: &Id) {
        self.grid.remove(id);
        self.walls.remove(id);
    }

    /// The record held for the wall `id`.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Element> {
        self.walls.get(id)
    }

    /// Every wall it holds, in the order of their IDs.
    pub fn all(&self) -> impl Iterator<Item = &Element> {
        self.walls.values()
    }

    /// How many walls it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.walls.len()
    }

    /// Whether it holds no wall.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
    }

    /// The walls whose base stands on `level` (`None`: on no level), in the order of their IDs.
    pub fn on_level(&self, level: Option<&Id>) -> impl Iterator<Item = &Element> {
        self.grid.on_level(level).filter_map(|id| self.get(id))
    }

    /// How many walls stand on `level`.
    #[must_use]
    pub fn count_on(&self, level: Option<&Id>) -> usize {
        self.grid.count_on(level)
    }

    /// The walls within two meetings of `element` on its base level: the walls its line meets,
    /// and the walls theirs meet, other than itself, by ID. Nothing for a record with no line.
    #[must_use]
    pub fn within_two(&self, element: &Element) -> BTreeSet<Id> {
        let Some(line) = line_in_model(element) else {
            return BTreeSet::new();
        };
        self.grid
            .within_two(base_level(element).as_ref(), line, &element.id)
    }
}
