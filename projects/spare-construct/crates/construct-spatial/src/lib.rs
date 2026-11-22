//! Spare Construct: spatial index of element bounds (CON-A2-F4-P4, piece A31).
//!
//! "What is in this box, or this view?" answered from the boxes the model already stores, without
//! reading every element. The index holds, for each element id, its axis-aligned box in whole
//! millimetres ([`construct_model::geometry::Bounds`], the box regeneration stores) and
//! optionally a [`Scope`], a level or a document. Lengths elsewhere are stored in the geometry
//! kernel's grid steps (1/6,400 mm, note CON-A2-F3-P1); a box stays in whole millimetres because
//! it is a cache for finding candidates, not a measurement: a box made from grid geometry is
//! rounded outward (its low corner down and its high corner up to whole millimetres), so it still
//! holds its shape and no query misses an element, and a caller that needs the exact answer
//! checks the few candidates against their shapes. It offers:
//!
//! - [`SpatialIndex::insert`], [`SpatialIndex::update`] (an element moved or resized) and
//!   [`SpatialIndex::remove`];
//! - [`SpatialIndex::meeting`]: every element whose box meets a box (touching counts);
//! - [`SpatialIndex::inside`]: every element whose box lies wholly inside a box (touching counts);
//! - [`SpatialIndex::nearest`]: the elements nearest a point, up to a count.
//!
//! Every query takes an optional scope and returns element ids sorted by id, never in the order a
//! tree happens to walk. So the same model gives the same answer on every machine and every run.
//! Nearest ranks by exact integer squared distance from the point to the box (zero when the point
//! is inside it), ties broken by id; if several elements tie at the last place, those with the
//! smaller ids are kept, and the answer is then sorted by id like every other.
//!
//! This is a cache and never the truth (main's ruling 7: the records in the database are the
//! truth). It has no stored form of its own: nothing here is serialised and no file or key is
//! reserved for it. It is rebuilt from the elements' stored bounds, with [`SpatialIndex::from_entries`]
//! when a document is opened and by [`SpatialIndex::update`] as edits arrive, and a rebuilt index
//! answers exactly as the one it replaces.
//!
//! Coordinates must be within [`MAX_COORDINATE`] (about 1,100 km, far beyond any site), so that
//! every comparison and distance is exact; a box outside that, or with a low corner above its high
//! corner, is refused with a [`SpatialError`], never clamped.
//!
//! It depends on `construct-model` (the box and id types), `kit-core` (the id) and, inside
//! `tree.rs` alone, the `rstar` R-tree crate (MIT OR Apache-2.0). Nothing here is interim.
//! Replacing the tree changes `tree.rs` and no caller; the tests are written against this file's
//! types and would judge a replacement unchanged.
//!
//! Speed is measured, not assumed: `tests/speed.rs` times 100,000 elements and prints the numbers.
//!
//! - [`tree`] (private): the R-tree.

mod tree;

use construct_model::geometry::Bounds;
use kit_core::Id;
use std::collections::BTreeMap;
use std::fmt;
use tree::Tree;

/// The largest coordinate, in millimetres, in either direction (2^40). Within it every coordinate
/// is held exactly by the tree, and a squared distance between two points fits an `i128` many times
/// over.
pub const MAX_COORDINATE: i64 = 1 << 40;

/// What an element is grouped under, if the caller wants queries limited to one group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// A level, by the integer the element records carry today.
    Level(i64),
    /// A document (a model), by its permanent id.
    Document(Id),
}

/// One element as the index holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The element's permanent id.
    pub id: Id,
    /// Its box, in whole millimetres.
    pub bounds: Bounds,
    /// The level or document it is grouped under, if any.
    pub scope: Option<Scope>,
}

/// Why the index refused something.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpatialError {
    /// A box whose low corner is above its high corner on some axis.
    Inverted,
    /// A coordinate beyond [`MAX_COORDINATE`] in either direction.
    OutOfRange,
    /// An insert of an id the index already holds.
    Duplicate(Id),
    /// An update or remove of an id the index does not hold.
    Missing(Id),
}

impl fmt::Display for SpatialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inverted => f.write_str("a box has its low corner above its high corner"),
            Self::OutOfRange => write!(f, "a coordinate is beyond {MAX_COORDINATE} mm"),
            Self::Duplicate(id) => write!(f, "element {} is already indexed", id.as_str()),
            Self::Missing(id) => write!(f, "element {} is not indexed", id.as_str()),
        }
    }
}

impl std::error::Error for SpatialError {}

/// Checks a box: ordered corners, coordinates in range.
fn check(bounds: &Bounds) -> Result<(), SpatialError> {
    let in_range = |c: &i64| (-MAX_COORDINATE..=MAX_COORDINATE).contains(c);
    if !bounds.min.iter().chain(&bounds.max).all(in_range) {
        return Err(SpatialError::OutOfRange);
    }
    if bounds
        .min
        .iter()
        .zip(&bounds.max)
        .any(|(low, high)| low > high)
    {
        return Err(SpatialError::Inverted);
    }
    Ok(())
}

/// Whether an entry passes the optional scope filter.
fn in_scope(entry: &Entry, scope: Option<&Scope>) -> bool {
    scope.is_none_or(|wanted| entry.scope.as_ref() == Some(wanted))
}

/// Squared distance, in square millimetres, from a point to a box, zero inside it. Saturating
/// only to satisfy the arithmetic lints: within [`MAX_COORDINATE`] nothing can reach the limit.
fn distance_squared(point: [i64; 3], bounds: &Bounds) -> i128 {
    let mut total: i128 = 0;
    for axis in 0..3 {
        let (p, low, high) = match (point.get(axis), bounds.min.get(axis), bounds.max.get(axis)) {
            (Some(p), Some(low), Some(high)) => {
                (i128::from(*p), i128::from(*low), i128::from(*high))
            }
            _ => continue,
        };
        let gap = low.saturating_sub(p).max(p.saturating_sub(high)).max(0);
        total = total.saturating_add(gap.saturating_mul(gap));
    }
    total
}

/// Ids sorted ascending: the one order every answer has.
fn sorted<'a>(entries: impl Iterator<Item = &'a Entry>) -> Vec<&'a Id> {
    let mut ids: Vec<&Id> = entries.map(|entry| &entry.id).collect();
    ids.sort_unstable();
    ids
}

/// The index. A cache of element boxes, rebuilt from the records; never stored.
#[derive(Clone, Debug, Default)]
pub struct SpatialIndex {
    tree: Tree,
    by_id: BTreeMap<Id, Entry>,
}

impl SpatialIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An index holding `entries`, built in one pass (faster than inserting them one by one, and
    /// the way a document's index is rebuilt when it is opened). Refuses the lot if any entry is
    /// refused: a bad box, or an id given twice.
    ///
    /// # Errors
    /// The first entry's [`SpatialError`], in the order given.
    pub fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Result<Self, SpatialError> {
        let mut by_id = BTreeMap::new();
        for entry in entries {
            check(&entry.bounds)?;
            if by_id.contains_key(&entry.id) {
                return Err(SpatialError::Duplicate(entry.id));
            }
            by_id.insert(entry.id.clone(), entry);
        }
        let tree = Tree::bulk(by_id.values().cloned().collect());
        Ok(Self { tree, by_id })
    }

    /// How many elements are indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether nothing is indexed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// What the index holds for an element.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Entry> {
        self.by_id.get(id)
    }

    /// Adds an element.
    ///
    /// # Errors
    /// [`SpatialError::Duplicate`] if the id is already indexed; a box error if the box is bad.
    pub fn insert(
        &mut self,
        id: Id,
        bounds: Bounds,
        scope: Option<Scope>,
    ) -> Result<(), SpatialError> {
        check(&bounds)?;
        if self.by_id.contains_key(&id) {
            return Err(SpatialError::Duplicate(id));
        }
        let entry = Entry {
            id: id.clone(),
            bounds,
            scope,
        };
        self.tree.insert(entry.clone());
        self.by_id.insert(id, entry);
        Ok(())
    }

    /// Replaces an element's box and scope: it moved or was resized.
    ///
    /// # Errors
    /// [`SpatialError::Missing`] if the id is not indexed; a box error if the box is bad. On an
    /// error the index is unchanged.
    pub fn update(
        &mut self,
        id: &Id,
        bounds: Bounds,
        scope: Option<Scope>,
    ) -> Result<(), SpatialError> {
        check(&bounds)?;
        let Some(old) = self.by_id.get_mut(id) else {
            return Err(SpatialError::Missing(id.clone()));
        };
        let new = Entry {
            id: id.clone(),
            bounds,
            scope,
        };
        if *old == new {
            return Ok(());
        }
        self.tree.remove(old);
        self.tree.insert(new.clone());
        *old = new;
        Ok(())
    }

    /// Takes an element out and returns what was held for it.
    ///
    /// # Errors
    /// [`SpatialError::Missing`] if the id is not indexed.
    pub fn remove(&mut self, id: &Id) -> Result<Entry, SpatialError> {
        let Some(old) = self.by_id.remove(id) else {
            return Err(SpatialError::Missing(id.clone()));
        };
        self.tree.remove(&old);
        Ok(old)
    }

    /// The ids of every element whose box meets `query` (sharing a face, edge or corner counts),
    /// sorted by id. With a scope, only elements in that scope.
    ///
    /// # Errors
    /// A box error if `query` is bad.
    pub fn meeting(&self, query: &Bounds, scope: Option<&Scope>) -> Result<Vec<&Id>, SpatialError> {
        check(query)?;
        Ok(sorted(
            self.tree.meeting(query).filter(|e| in_scope(e, scope)),
        ))
    }

    /// The ids of every element whose box lies wholly inside `query` (touching its faces counts),
    /// sorted by id. With a scope, only elements in that scope.
    ///
    /// # Errors
    /// A box error if `query` is bad.
    pub fn inside(&self, query: &Bounds, scope: Option<&Scope>) -> Result<Vec<&Id>, SpatialError> {
        check(query)?;
        Ok(sorted(
            self.tree.inside(query).filter(|e| in_scope(e, scope)),
        ))
    }

    /// The ids of the `count` elements nearest `point`, sorted by id. Nearest means the least
    /// distance from the point to the element's box (zero for a box holding the point); elements
    /// at the same distance are ranked by id, so the smaller ids are kept at a tie. With a scope,
    /// only elements in that scope. Fewer than `count` come back if the index holds fewer.
    ///
    /// # Errors
    /// [`SpatialError::OutOfRange`] if the point is beyond [`MAX_COORDINATE`].
    pub fn nearest(
        &self,
        point: [i64; 3],
        count: usize,
        scope: Option<&Scope>,
    ) -> Result<Vec<&Id>, SpatialError> {
        if point.iter().any(|c| c.abs() > MAX_COORDINATE) {
            return Err(SpatialError::OutOfRange);
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        // The best `count` so far, as (exact squared distance, id), kept ascending.
        let mut best: Vec<(i128, &Id)> = Vec::with_capacity(count.saturating_add(1));
        for (entry, approx) in self.tree.by_distance(point) {
            if !in_scope(entry, scope) {
                continue;
            }
            if best.len() >= count {
                // The tree's float is within rounding of the true value and never falls as the
                // walk goes on; a relative margin of 1e-9 is far wider than the rounding, so
                // nothing that could still tie or win is left unread when this stops.
                if let Some((worst, _)) = best.last()
                    && approx > exact_as_float(*worst) * (1.0 + 1e-9) + 1.0
                {
                    break;
                }
            }
            let key = (distance_squared(point, &entry.bounds), &entry.id);
            let at = best.partition_point(|held| *held < key);
            best.insert(at, key);
            best.truncate(count);
        }
        let mut ids: Vec<&Id> = best.into_iter().map(|(_, id)| id).collect();
        ids.sort_unstable();
        Ok(ids)
    }
}

/// A squared distance as a float, for comparing with the tree's own. Squares reach 2^82, so the
/// float is a bound, not the value; the value stays an `i128` everywhere it ranks anything.
#[allow(clippy::cast_precision_loss)]
fn exact_as_float(value: i128) -> f64 {
    value as f64
}
