//! A curtain wall laid out: its type's grid and its own edits on its length and height, giving
//! every grid line, every panel and every mullion where it stands, exactly, in the wall's own
//! frame (along the wall from its start, across it toward its exterior, up from its base). Both
//! builders and the quantities read a curtain wall through this one module, so they cannot
//! disagree.
//!
//! The rules ([`Curtain::grid`]):
//!
//! - **Lines.** In each direction: the type's lines ([`crate::Layout::lines`]) less those the wall
//!   removes, with those it adds, and the two borders (the wall's start and end for vertical
//!   lines, its base and top for horizontal ones). An added line within the model's tolerance of
//!   another, or not inside the borders, is unused (the type changed under it).
//! - **Bands.** Each line carries the mullion profile of its kind (interior or border, vertical or
//!   horizontal) and the band that profile's width covers across the line: centred on an interior
//!   line, inside the wall on a border (from the start to its width, or back from the end), so a
//!   curtain wall never reaches past its own line or its own height. A line without a profile has
//!   a band of no width.
//! - **Panels.** Each cell between neighbouring lines is a panel, and cells either side of a
//!   left-out segment are one panel; every panel must be a rectangle of cells (otherwise the edit
//!   is refused). A panel fills its cells to its edge lines' bands, whether or not a mullion stands
//!   there, and holds the type's panel or the one the wall swaps in at the corner its cell starts
//!   at.
//! - **Mullions.** A mullion stands on each segment of a line with a profile that is not left out
//!   and that lies on a panel's edge: between two panels, or on a border. One inside a panel (its
//!   neighbours merged across other segments) is not built.
//! - **Crossings.** Where lines cross, the type's direction ([`crate::Crossing`]) runs through if
//!   it has a mullion there, else the other one does: a mullion that runs through meets the next
//!   along its line at the crossing, or covers the whole crossing to the far side of the other
//!   line's band where none follows it; a mullion that does not run through stops at the other's
//!   band. So no two solids overlap and every meeting is face to face.
//!
//! Everything is exact on construct-params' grid; the only rounding is a layout's equal divisions
//! ([`crate::Layout`]). Nothing here reads a clock or panics.

use crate::edits::{Edits, Segment};
use crate::error::CurtainError;
use crate::grid::{Direction, MOST_LINES};
use crate::length::{self, TOLERANCE, ZERO};
use crate::spec::{CurtainSpec, DoorPanel, Panel, PanelKind, Profile};
use construct_materials::MaterialId;
use construct_params::Length;

/// The longest and highest a curtain wall may be: 1 km.
pub const MOST_EXTENT: Length = Length::from_steps(6_400_000_000);

/// The most cells a curtain wall's grid makes.
pub const MOST_CELLS: usize = 20_000;

/// A curtain wall as its layout reads it: its length and height, its type's grid and parts, and
/// its own edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curtain {
    length: Length,
    height: Length,
    spec: CurtainSpec,
    edits: Edits,
}

/// One segment of a grid line, between two neighbouring crossings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentState {
    /// The crossing it starts at.
    pub from: Length,
    /// The crossing it ends at.
    pub to: Length,
    /// Whether the wall leaves it out.
    pub omitted: bool,
    /// Whether a mullion stands on it.
    pub mullion: bool,
}

/// One grid line, as laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridLine {
    /// Which way it runs.
    pub direction: Direction,
    /// Where it stands.
    pub at: Length,
    /// Whether it is a border.
    pub border: bool,
    /// Whether the wall adds it (rather than its type laying it out).
    pub added: bool,
    /// The profile of the mullions on it.
    pub profile: Option<Profile>,
    /// What its profile's width covers across it, low then high.
    pub band: [Length; 2],
    /// Its segments, in order.
    pub segments: Vec<SegmentState>,
}

/// One panel, as laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelPlace {
    /// The corner its first cell starts at: along, then up.
    pub at: [Length; 2],
    /// Its grid lines: from and to along, then from and to up.
    pub grid: [[Length; 2]; 2],
    /// Its own extent, between its edge lines' bands: from and to along, then from and to up.
    pub extent: [[Length; 2]; 2],
    /// What it holds.
    pub panel: Panel,
    /// Whether the wall swaps it in, rather than its type giving it.
    pub swapped: bool,
}

impl PanelPlace {
    /// Its width along the wall.
    #[must_use]
    pub fn width(&self) -> Length {
        let [[a, b], _] = self.extent;
        length::difference(b, a).unwrap_or(ZERO)
    }

    /// Its height up the wall.
    #[must_use]
    pub fn height(&self) -> Length {
        let [_, [a, b]] = self.extent;
        length::difference(b, a).unwrap_or(ZERO)
    }
}

/// One mullion, as laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MullionPlace {
    /// Its line's direction.
    pub direction: Direction,
    /// Its line.
    pub at: Length,
    /// The crossing its segment starts at.
    pub from: Length,
    /// The crossing its segment ends at.
    pub to: Length,
    /// Whether it stands on a border.
    pub border: bool,
    /// Its profile.
    pub profile: Profile,
    /// Where it runs along its line, low then high, after the crossings.
    pub run: [Length; 2],
    /// Its line's band: where it lies across its line in the wall's face.
    pub band: [Length; 2],
}

impl MullionPlace {
    /// Its length along its line.
    #[must_use]
    pub fn length(&self) -> Length {
        let [a, b] = self.run;
        length::difference(b, a).unwrap_or(ZERO)
    }
}

/// A box in the wall's frame: along, across and up, each low then high.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameBox {
    /// Along the wall from its start.
    pub along: [Length; 2],
    /// Across the wall, toward its exterior.
    pub across: [Length; 2],
    /// Up from its base.
    pub up: [Length; 2],
}

/// What a solid of a curtain wall is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// A glazed or solid panel.
    Panel(PanelKind),
    /// A mullion.
    Mullion,
}

impl Part {
    /// Its word, as a display's part names what a solid does.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Part::Panel(PanelKind::Glazed) => "panel.glazed",
            Part::Panel(PanelKind::Solid) => "panel.solid",
            Part::Panel(PanelKind::Empty) => "panel.empty",
            Part::Panel(PanelKind::Door) => "panel.door",
            Part::Mullion => "mullion",
        }
    }
}

/// One solid of a curtain wall: a box in its frame, what it is, its material, and a key that
/// names it within the wall, made of where it stands, so it keeps its key while it stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solid {
    /// What it is.
    pub part: Part,
    /// Its key within the wall.
    pub key: String,
    /// Its material.
    pub material: MaterialId,
    /// Where it is.
    pub frame: FrameBox,
}

/// A door panel, as its builder places it: its family and type, the size its family is set to,
/// and where its family's origin stands (the middle of its foot, on the wall's line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorPlace {
    /// Its key within the wall.
    pub key: String,
    /// The family and type.
    pub door: DoorPanel,
    /// The width its family is set to: its panel's.
    pub width: Length,
    /// The height its family is set to: its panel's.
    pub height: Length,
    /// Where its middle stands along the wall.
    pub middle: Length,
    /// Where its foot stands up the wall.
    pub foot: Length,
}

/// A curtain wall laid out ([`Curtain::grid`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    /// Its length.
    pub length: Length,
    /// Its height.
    pub height: Length,
    /// Its vertical lines, from its start, the borders first and last.
    pub vertical: Vec<GridLine>,
    /// Its horizontal lines, from its base, the borders first and last.
    pub horizontal: Vec<GridLine>,
    /// Its panels, by the corner their first cell starts at (along, then up).
    pub panels: Vec<PanelPlace>,
    /// Its mullions: the vertical lines' in order, each line's from its base, then the horizontal
    /// lines' from the start.
    pub mullions: Vec<MullionPlace>,
    /// The edits that have no place in this grid, in words.
    pub unused: Vec<String>,
}

/// A two-dimensional table, read without indexing.
#[derive(Clone, Debug)]
struct Table<T> {
    columns: usize,
    cells: Vec<T>,
}

impl<T: Clone> Table<T> {
    fn new(columns: usize, rows: usize, fill: T) -> Option<Table<T>> {
        let count = columns.checked_mul(rows)?;
        Some(Table {
            columns,
            cells: vec![fill; count],
        })
    }

    fn index(&self, column: usize, row: usize) -> Option<usize> {
        if column >= self.columns {
            return None;
        }
        row.checked_mul(self.columns)?.checked_add(column)
    }

    fn get(&self, column: usize, row: usize) -> Option<&T> {
        self.cells.get(self.index(column, row)?)
    }

    fn set(&mut self, column: usize, row: usize, value: T) {
        if let Some(at) = self.index(column, row)
            && let Some(slot) = self.cells.get_mut(at)
        {
            *slot = value;
        }
    }
}

/// The root of `cell` in a union of cells.
fn root(parents: &mut [usize], cell: usize) -> usize {
    let mut at = cell;
    while let Some(&up) = parents.get(at) {
        if up == at {
            break;
        }
        // Halving the path keeps every later search short.
        let grand = parents.get(up).copied().unwrap_or(up);
        if let Some(slot) = parents.get_mut(at) {
            *slot = grand;
        }
        at = grand;
    }
    at
}

/// Joins the cells `a` and `b`, the smaller root kept so the answer is the same in any order.
fn join(parents: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (root(parents, a), root(parents, b));
    let (keep, drop) = if ra <= rb { (ra, rb) } else { (rb, ra) };
    if let Some(slot) = parents.get_mut(drop) {
        *slot = keep;
    }
}

/// A line's place, or zero for one past the list.
fn place(lines: &[GridLine], i: usize) -> Length {
    lines.get(i).map_or(ZERO, |l| l.at)
}

/// A line's band, or none for one past the list.
fn band(lines: &[GridLine], i: usize) -> [Length; 2] {
    lines.get(i).map_or([ZERO, ZERO], |l| l.band)
}

/// A step's key text: a length in whole grid steps.
fn steps(l: Length) -> String {
    l.steps().to_string()
}

impl Curtain {
    /// A curtain wall of `length` and `height`, its type's grid and parts `spec`, and its own
    /// `edits`.
    ///
    /// # Errors
    /// [`CurtainError::NotPositive`] for a length or height at or below the model's tolerance;
    /// [`CurtainError::TooLarge`] for one beyond [`MOST_EXTENT`].
    pub fn new(
        length: Length,
        height: Length,
        spec: CurtainSpec,
        edits: Edits,
    ) -> Result<Curtain, CurtainError> {
        for (what, value) in [
            ("curtain wall's length", length),
            ("curtain wall's height", height),
        ] {
            if value <= TOLERANCE {
                return Err(CurtainError::NotPositive {
                    what: format!("{what} (above 0.01 mm)"),
                    value,
                });
            }
            if value > MOST_EXTENT {
                return Err(CurtainError::TooLarge {
                    what: what.to_owned(),
                    value,
                    most: MOST_EXTENT,
                });
            }
        }
        Ok(Curtain {
            length,
            height,
            spec,
            edits,
        })
    }

    /// Its length along its line.
    #[must_use]
    pub fn length(&self) -> Length {
        self.length
    }

    /// Its height.
    #[must_use]
    pub fn height(&self) -> Length {
        self.height
    }

    /// Its type's grid and parts.
    #[must_use]
    pub fn spec(&self) -> &CurtainSpec {
        &self.spec
    }

    /// Its own edits.
    #[must_use]
    pub fn edits(&self) -> &Edits {
        &self.edits
    }

    /// The same wall of another type's grid and parts; its edits stay.
    #[must_use]
    pub fn with_spec(&self, spec: CurtainSpec) -> Curtain {
        Curtain {
            spec,
            ..self.clone()
        }
    }

    /// How far its lines of `direction` reach: its length for vertical lines (they stand along
    /// it), its height for horizontal ones.
    #[must_use]
    pub fn extent(&self, direction: Direction) -> Length {
        match direction {
            Direction::Vertical => self.length,
            Direction::Horizontal => self.height,
        }
    }

    /// Its lines of `direction`, borders and all, and the edits among them that have no place.
    fn lines(&self, direction: Direction) -> Result<(Vec<GridLine>, Vec<String>), CurtainError> {
        let extent = self.extent(direction);
        let typed = self.spec.layout(direction).lines(direction, extent)?;
        let mut unused = Vec::new();
        for removed in self.edits.removed(direction) {
            if !typed.contains(&removed) {
                unused.push(format!(
                    "the removed {direction} grid line at {removed} is not one its type lays out"
                ));
            }
        }
        let mut kept: Vec<(Length, bool)> = typed
            .into_iter()
            .filter(|at| !self.edits.removes(direction, *at))
            .map(|at| (at, false))
            .collect();
        let high = length::difference(extent, TOLERANCE).ok_or(CurtainError::OutOfRange)?;
        for added in self.edits.added(direction) {
            if added <= TOLERANCE || added >= high {
                unused.push(format!(
                    "the added {direction} grid line at {added} is not inside the curtain wall"
                ));
                continue;
            }
            let near = kept.iter().find(|(at, _)| {
                let gap = if *at > added {
                    length::difference(*at, added)
                } else {
                    length::difference(added, *at)
                };
                gap.is_some_and(|gap| gap <= TOLERANCE)
            });
            if let Some((near, _)) = near {
                unused.push(format!(
                    "the added {direction} grid line at {added} is within 0.01 mm of the line at \
                     {near}"
                ));
                continue;
            }
            kept.push((added, true));
        }
        kept.sort();
        if kept.len() > MOST_LINES {
            return Err(CurtainError::TooMany {
                what: format!("{direction} grid lines"),
                count: u64::try_from(kept.len()).unwrap_or(u64::MAX),
                most: u64::try_from(MOST_LINES).unwrap_or(u64::MAX),
            });
        }
        let mut lines = Vec::with_capacity(kept.len().saturating_add(2));
        let ends = [(ZERO, false)];
        let last = [(extent, false)];
        let count = kept.len().saturating_add(2);
        for (n, (at, added)) in ends.iter().chain(&kept).chain(&last).enumerate() {
            let border = n == 0 || n.saturating_add(1) == count;
            let profile = self.spec.mullions().of(direction, border).cloned();
            let width = profile.as_ref().map_or(ZERO, Profile::width);
            let band = if n == 0 {
                [ZERO, width]
            } else if border {
                [
                    length::difference(extent, width).ok_or(CurtainError::OutOfRange)?,
                    extent,
                ]
            } else {
                let half = length::half(width).ok_or(CurtainError::OutOfRange)?;
                [
                    length::difference(*at, half).ok_or(CurtainError::OutOfRange)?,
                    length::sum(*at, half).ok_or(CurtainError::OutOfRange)?,
                ]
            };
            lines.push(GridLine {
                direction,
                at: *at,
                border,
                added: *added,
                profile,
                band,
                segments: Vec::new(),
            });
        }
        Ok((lines, unused))
    }

    /// The wall laid out, as the module header says.
    ///
    /// # Errors
    /// Its type's layouts' refusals; [`CurtainError::TooMany`] for more than [`MOST_CELLS`] cells;
    /// [`CurtainError::PanelNotRectangle`] where left-out segments make a panel that is not a
    /// rectangle of cells; [`CurtainError::NoRoomForPanel`] and
    /// [`CurtainError::NoRoomForMullion`] where the mullions leave a panel or a mullion no room;
    /// [`CurtainError::OutOfRange`] for arithmetic beyond a length.
    #[allow(
        clippy::too_many_lines,
        reason = "the lines, the cells, the panels and the mullions, in the order they depend on each other"
    )]
    pub fn grid(&self) -> Result<Grid, CurtainError> {
        let (mut vertical, mut unused) = self.lines(Direction::Vertical)?;
        let (mut horizontal, unused_h) = self.lines(Direction::Horizontal)?;
        unused.extend(unused_h);
        let columns = vertical.len().saturating_sub(1);
        let rows = horizontal.len().saturating_sub(1);
        let cells = columns.checked_mul(rows).ok_or(CurtainError::OutOfRange)?;
        if cells > MOST_CELLS {
            return Err(CurtainError::TooMany {
                what: "cells".to_owned(),
                count: u64::try_from(cells).unwrap_or(u64::MAX),
                most: u64::try_from(MOST_CELLS).unwrap_or(u64::MAX),
            });
        }
        let table = |c: usize, r: usize| Table::new(c, r, false).ok_or(CurtainError::OutOfRange);
        // Left-out segments: vertical line i, segment j (rows); horizontal line j, segment i.
        let mut v_out = table(vertical.len(), rows)?;
        let mut h_out = table(columns, horizontal.len())?;
        for segment in self.edits.omitted() {
            let (lines, crossings) = match segment.direction {
                Direction::Vertical => (&vertical, &horizontal),
                Direction::Horizontal => (&horizontal, &vertical),
            };
            let line = lines.iter().position(|l| l.at == segment.at);
            let from = crossings
                .iter()
                .take(crossings.len().saturating_sub(1))
                .position(|l| l.at == segment.from);
            match (line, from, segment.direction) {
                (Some(i), Some(j), Direction::Vertical) => v_out.set(i, j, true),
                (Some(j), Some(i), Direction::Horizontal) => h_out.set(i, j, true),
                _ => unused.push(format!(
                    "the left-out segment of the {} grid line at {} from {} is not on the grid",
                    segment.direction, segment.at, segment.from
                )),
            }
        }
        // Cells joined across every left-out interior segment.
        let mut parents: Vec<usize> = (0..cells).collect();
        let cell = |c: usize, r: usize| r.checked_mul(columns).and_then(|x| x.checked_add(c));
        for i in 1..columns {
            for j in 0..rows {
                if v_out.get(i, j).copied().unwrap_or(false)
                    && let (Some(a), Some(b)) = (cell(i.saturating_sub(1), j), cell(i, j))
                {
                    join(&mut parents, a, b);
                }
            }
        }
        for j in 1..rows {
            for i in 0..columns {
                if h_out.get(i, j).copied().unwrap_or(false)
                    && let (Some(a), Some(b)) = (cell(i, j.saturating_sub(1)), cell(i, j))
                {
                    join(&mut parents, a, b);
                }
            }
        }
        // Each panel: its cells' span, which must be its whole count.
        let mut groups: std::collections::BTreeMap<usize, [usize; 5]> =
            std::collections::BTreeMap::new();
        let mut group = Table::new(columns, rows, 0_usize).ok_or(CurtainError::OutOfRange)?;
        for r in 0..rows {
            for c in 0..columns {
                let Some(at) = cell(c, r) else { continue };
                let g = root(&mut parents, at);
                group.set(c, r, g);
                let entry = groups.entry(g).or_insert([c, c, r, r, 0]);
                entry[0] = entry[0].min(c);
                entry[1] = entry[1].max(c);
                entry[2] = entry[2].min(r);
                entry[3] = entry[3].max(r);
                entry[4] = entry[4].saturating_add(1);
            }
        }
        let mut panels = Vec::with_capacity(groups.len());
        for [c0, c1, r0, r1, count] in groups.into_values() {
            let at = [place(&vertical, c0), place(&horizontal, r0)];
            let span = c1
                .saturating_sub(c0)
                .saturating_add(1)
                .saturating_mul(r1.saturating_sub(r0).saturating_add(1));
            if span != count {
                return Err(CurtainError::PanelNotRectangle { at });
            }
            let (c_end, r_end) = (c1.saturating_add(1), r1.saturating_add(1));
            let extent = [
                [band(&vertical, c0)[1], band(&vertical, c_end)[0]],
                [band(&horizontal, r0)[1], band(&horizontal, r_end)[0]],
            ];
            if extent[0][0] >= extent[0][1] || extent[1][0] >= extent[1][1] {
                return Err(CurtainError::NoRoomForPanel { at });
            }
            let (panel, swapped) = match self.edits.panel_at(&at) {
                Some(panel) => (panel.clone(), true),
                None => (self.spec.panel().clone(), false),
            };
            panels.push(PanelPlace {
                at,
                grid: [
                    [place(&vertical, c0), place(&vertical, c_end)],
                    [place(&horizontal, r0), place(&horizontal, r_end)],
                ],
                extent,
                panel,
                swapped,
            });
        }
        panels.sort_by_key(|p| p.at);
        for (at, _) in self.edits.panels() {
            if !panels.iter().any(|p| p.at == *at) {
                let [along, up] = at;
                unused.push(format!(
                    "the panel swapped at {along} along and {up} up is not a panel's corner"
                ));
            }
        }
        // Which segments carry a mullion: a profile, kept, and on a panel's edge.
        let group_of = |c: usize, r: usize| group.get(c, r).copied();
        let mut v_on = table(vertical.len(), rows)?;
        for (i, line) in vertical.iter().enumerate() {
            for j in 0..rows {
                let edge = line.border || group_of(i.saturating_sub(1), j) != group_of(i, j);
                let kept = !v_out.get(i, j).copied().unwrap_or(false);
                v_on.set(i, j, line.profile.is_some() && kept && edge);
            }
        }
        let mut h_on = table(columns, horizontal.len())?;
        for (j, line) in horizontal.iter().enumerate() {
            for i in 0..columns {
                let edge = line.border || group_of(i, j.saturating_sub(1)) != group_of(i, j);
                let kept = !h_out.get(i, j).copied().unwrap_or(false);
                h_on.set(i, j, line.profile.is_some() && kept && edge);
            }
        }
        let v_has = |i: usize, j: usize| v_on.get(i, j).copied().unwrap_or(false);
        let h_has = |i: usize, j: usize| h_on.get(i, j).copied().unwrap_or(false);
        // Which direction runs through the crossing of vertical line i and horizontal line j.
        let through = self.spec.crossing().direction();
        let winner = |i: usize, j: usize| {
            let below = j.checked_sub(1).is_some_and(|b| v_has(i, b));
            let vertical_here = below || v_has(i, j);
            let left = i.checked_sub(1).is_some_and(|l| h_has(l, j));
            let horizontal_here = left || h_has(i, j);
            match (through, vertical_here, horizontal_here) {
                (Direction::Vertical, true, _) | (Direction::Horizontal, true, false) => {
                    Some(Direction::Vertical)
                }
                (Direction::Horizontal, _, true) | (Direction::Vertical, false, true) => {
                    Some(Direction::Horizontal)
                }
                _ => None,
            }
        };
        let mut mullions = Vec::new();
        for (i, line) in vertical.iter().enumerate() {
            let Some(profile) = &line.profile else {
                continue;
            };
            for j in 0..rows {
                if !v_has(i, j) {
                    continue;
                }
                let up = j.saturating_add(1);
                let low = if winner(i, j) == Some(Direction::Vertical) {
                    if j.checked_sub(1).is_some_and(|b| v_has(i, b)) {
                        place(&horizontal, j)
                    } else {
                        band(&horizontal, j)[0]
                    }
                } else {
                    band(&horizontal, j)[1]
                };
                let high = if winner(i, up) == Some(Direction::Vertical) {
                    if v_has(i, up) {
                        place(&horizontal, up)
                    } else {
                        band(&horizontal, up)[1]
                    }
                } else {
                    band(&horizontal, up)[0]
                };
                let (from, to) = (place(&horizontal, j), place(&horizontal, up));
                if low >= high {
                    return Err(CurtainError::NoRoomForMullion {
                        direction: Direction::Vertical,
                        at: line.at,
                        from,
                    });
                }
                mullions.push(MullionPlace {
                    direction: Direction::Vertical,
                    at: line.at,
                    from,
                    to,
                    border: line.border,
                    profile: profile.clone(),
                    run: [low, high],
                    band: line.band,
                });
            }
        }
        for (j, line) in horizontal.iter().enumerate() {
            let Some(profile) = &line.profile else {
                continue;
            };
            for i in 0..columns {
                if !h_has(i, j) {
                    continue;
                }
                let next = i.saturating_add(1);
                let low = if winner(i, j) == Some(Direction::Horizontal) {
                    if i.checked_sub(1).is_some_and(|l| h_has(l, j)) {
                        place(&vertical, i)
                    } else {
                        band(&vertical, i)[0]
                    }
                } else {
                    band(&vertical, i)[1]
                };
                let high = if winner(next, j) == Some(Direction::Horizontal) {
                    if h_has(next, j) {
                        place(&vertical, next)
                    } else {
                        band(&vertical, next)[1]
                    }
                } else {
                    band(&vertical, next)[0]
                };
                let (from, to) = (place(&vertical, i), place(&vertical, next));
                if low >= high {
                    return Err(CurtainError::NoRoomForMullion {
                        direction: Direction::Horizontal,
                        at: line.at,
                        from,
                    });
                }
                mullions.push(MullionPlace {
                    direction: Direction::Horizontal,
                    at: line.at,
                    from,
                    to,
                    border: line.border,
                    profile: profile.clone(),
                    run: [low, high],
                    band: line.band,
                });
            }
        }
        // Each line's segments, as the page lists them.
        let v_places: Vec<Length> = horizontal.iter().map(|l| l.at).collect();
        let h_places: Vec<Length> = vertical.iter().map(|l| l.at).collect();
        for (i, line) in vertical.iter_mut().enumerate() {
            line.segments = (0..rows)
                .map(|j| SegmentState {
                    from: v_places.get(j).copied().unwrap_or(ZERO),
                    to: v_places.get(j.saturating_add(1)).copied().unwrap_or(ZERO),
                    omitted: v_out.get(i, j).copied().unwrap_or(false),
                    mullion: v_has(i, j),
                })
                .collect();
        }
        for (j, line) in horizontal.iter_mut().enumerate() {
            line.segments = (0..columns)
                .map(|i| SegmentState {
                    from: h_places.get(i).copied().unwrap_or(ZERO),
                    to: h_places.get(i.saturating_add(1)).copied().unwrap_or(ZERO),
                    omitted: h_out.get(i, j).copied().unwrap_or(false),
                    mullion: h_has(i, j),
                })
                .collect();
        }
        Ok(Grid {
            length: self.length,
            height: self.height,
            vertical,
            horizontal,
            panels,
            mullions,
            unused,
        })
    }

    /// The type's own lines of `direction`.
    fn typed(&self, direction: Direction) -> Result<Vec<Length>, CurtainError> {
        self.spec
            .layout(direction)
            .lines(direction, self.extent(direction))
    }

    /// The same wall with its edits `edits`, laid out to check it.
    fn edited(&self, edits: Edits) -> Result<Curtain, CurtainError> {
        let next = Curtain {
            edits,
            ..self.clone()
        };
        next.grid()?;
        Ok(next)
    }

    /// The wall with a grid line of `direction` added at `at`: a line of its type it had removed
    /// there comes back; otherwise the line is its own.
    ///
    /// # Errors
    /// [`CurtainError::LineOutside`] for a place not more than the tolerance inside the borders;
    /// [`CurtainError::LineTaken`] within the tolerance of a line it has; and the refusals of
    /// [`Curtain::grid`] for the grid that would make.
    pub fn with_line(&self, direction: Direction, at: Length) -> Result<Curtain, CurtainError> {
        let extent = self.extent(direction);
        let high = length::difference(extent, TOLERANCE).ok_or(CurtainError::OutOfRange)?;
        if at <= TOLERANCE || at >= high {
            return Err(CurtainError::LineOutside { direction, at });
        }
        let mut edits = self.edits.clone();
        if self.typed(direction)?.contains(&at) && edits.removes(direction, at) {
            edits.unremove(direction, at);
            return self.edited(edits);
        }
        let grid = self.grid()?;
        let lines = match direction {
            Direction::Vertical => &grid.vertical,
            Direction::Horizontal => &grid.horizontal,
        };
        for line in lines {
            let gap = if line.at > at {
                length::difference(line.at, at)
            } else {
                length::difference(at, line.at)
            };
            if gap.is_some_and(|gap| gap <= TOLERANCE) {
                return Err(CurtainError::LineTaken {
                    direction,
                    at,
                    near: line.at,
                });
            }
        }
        edits.add(direction, at);
        self.edited(edits)
    }

    /// The wall without its grid line of `direction` at `at`, its own or its type's, and without
    /// that line's left-out segments.
    ///
    /// # Errors
    /// [`CurtainError::NoSuchLine`] where it has no such line (a border is no grid line); the
    /// refusals of [`Curtain::grid`] for the grid that would make.
    pub fn without_line(&self, direction: Direction, at: Length) -> Result<Curtain, CurtainError> {
        let mut edits = self.edits.clone();
        if edits.adds(direction, at) {
            edits.unadd(direction, at);
        } else if self.typed(direction)?.contains(&at) && !edits.removes(direction, at) {
            edits.remove(direction, at);
        } else {
            return Err(CurtainError::NoSuchLine { direction, at });
        }
        edits.restore_line(direction, at);
        self.edited(edits)
    }

    /// Whether `segment` is a segment of the grid now.
    fn check_segment(&self, segment: &Segment) -> Result<(), CurtainError> {
        let grid = self.grid()?;
        let (lines, crossings) = match segment.direction {
            Direction::Vertical => (&grid.vertical, &grid.horizontal),
            Direction::Horizontal => (&grid.horizontal, &grid.vertical),
        };
        if !lines.iter().any(|l| l.at == segment.at) {
            return Err(CurtainError::NoSuchLine {
                direction: segment.direction,
                at: segment.at,
            });
        }
        let starts = crossings.iter().take(crossings.len().saturating_sub(1));
        if !starts.into_iter().any(|l| l.at == segment.from) {
            return Err(CurtainError::NoSuchSegment {
                direction: segment.direction,
                at: segment.at,
                from: segment.from,
            });
        }
        Ok(())
    }

    /// The wall with `segment` left out: the panels either side become one, and no mullion stands
    /// on it.
    ///
    /// # Errors
    /// [`CurtainError::NoSuchLine`] or [`CurtainError::NoSuchSegment`] where the grid has no such
    /// segment; [`CurtainError::AlreadyOmitted`]; the refusals of [`Curtain::grid`] for the grid
    /// that would make ([`CurtainError::PanelNotRectangle`] among them).
    pub fn with_segment_omitted(&self, segment: Segment) -> Result<Curtain, CurtainError> {
        self.check_segment(&segment)?;
        let mut edits = self.edits.clone();
        if !edits.omit(segment) {
            return Err(CurtainError::AlreadyOmitted {
                direction: segment.direction,
                at: segment.at,
                from: segment.from,
            });
        }
        self.edited(edits)
    }

    /// The wall with `segment` kept again.
    ///
    /// # Errors
    /// [`CurtainError::NotOmitted`]; the refusals of [`Curtain::grid`] for the grid that would
    /// make.
    pub fn with_segment_restored(&self, segment: Segment) -> Result<Curtain, CurtainError> {
        let mut edits = self.edits.clone();
        if !edits.restore(&segment) {
            return Err(CurtainError::NotOmitted {
                direction: segment.direction,
                at: segment.at,
                from: segment.from,
            });
        }
        self.edited(edits)
    }

    /// The wall with `panel` in the panel whose cell starts at `at`, or its type's panel again for
    /// `None`.
    ///
    /// # Errors
    /// [`CurtainError::NoSuchPanel`] where no panel's cell starts there; the refusals of
    /// [`Curtain::grid`] for the grid that would make.
    pub fn with_panel(
        &self,
        at: [Length; 2],
        panel: Option<Panel>,
    ) -> Result<Curtain, CurtainError> {
        let grid = self.grid()?;
        if !grid.panels.iter().any(|p| p.at == at) {
            return Err(CurtainError::NoSuchPanel { at });
        }
        let mut edits = self.edits.clone();
        edits.swap(at, panel);
        self.edited(edits)
    }
}

impl Grid {
    /// Its lines of `direction`.
    #[must_use]
    pub fn lines(&self, direction: Direction) -> &[GridLine] {
        match direction {
            Direction::Vertical => &self.vertical,
            Direction::Horizontal => &self.horizontal,
        }
    }

    /// Every solid it is built of: each glazed or solid panel, then each mullion, in the grid's
    /// order. An empty panel has none, and a door panel is its family's ([`Grid::doors`]).
    ///
    /// # Errors
    /// [`CurtainError::OutOfRange`] never, for a grid this module laid out.
    pub fn solids(&self) -> Result<Vec<Solid>, CurtainError> {
        let mut out = Vec::new();
        for panel in &self.panels {
            let Some(infill) = panel.panel.infill() else {
                continue;
            };
            let [along, up] = panel.extent;
            let [u, v] = panel.at;
            out.push(Solid {
                part: Part::Panel(panel.panel.kind()),
                key: format!("panel.{}.{}", steps(u), steps(v)),
                material: infill.material().clone(),
                frame: FrameBox {
                    along,
                    across: infill.across()?,
                    up,
                },
            });
        }
        for mullion in &self.mullions {
            let (along, up, letter) = match mullion.direction {
                Direction::Vertical => (mullion.band, mullion.run, "v"),
                Direction::Horizontal => (mullion.run, mullion.band, "h"),
            };
            out.push(Solid {
                part: Part::Mullion,
                key: format!(
                    "mullion.{letter}.{}.{}",
                    steps(mullion.at),
                    steps(mullion.from)
                ),
                material: mullion.profile.material().clone(),
                frame: FrameBox {
                    along,
                    across: mullion.profile.across()?,
                    up,
                },
            });
        }
        Ok(out)
    }

    /// Every door panel, as its builder places its family.
    ///
    /// # Errors
    /// [`CurtainError::DoorOffGrid`] for a door panel whose middle falls between two steps.
    pub fn doors(&self) -> Result<Vec<DoorPlace>, CurtainError> {
        let mut out = Vec::new();
        for panel in &self.panels {
            let Panel::Door(door) = &panel.panel else {
                continue;
            };
            let [[a, b], [foot, _]] = panel.extent;
            let twice = length::sum(a, b).ok_or(CurtainError::OutOfRange)?;
            let middle = length::half(twice).ok_or(CurtainError::DoorOffGrid { at: panel.at })?;
            let [u, v] = panel.at;
            out.push(DoorPlace {
                key: format!("door.{}.{}", steps(u), steps(v)),
                door: door.clone(),
                width: panel.width(),
                height: panel.height(),
                middle,
                foot,
            });
        }
        Ok(out)
    }
}
