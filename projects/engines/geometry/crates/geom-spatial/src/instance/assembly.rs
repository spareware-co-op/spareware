//! An assembly: the caller's instance records, their instance tree, and queries that descend
//! through nested assemblies into each definition's own tree in its own frame (19.12; piece G295).
//!
//! What it does: [`Assembly::build`] takes a frame and the records placed in it, checks every
//! record against the table of definitions (its definition exists, the two frames have an exact
//! shift, its box holds its definition as placed), and builds the [`InstanceTree`] over the
//! records' boxes. [`Assembly::overlapping`] answers one level: the records whose boxes meet a
//! region. [`Assembly::query`] answers every level: for each record whose box meets the region it
//! enters the definition, moving the region into the definition's frame (an exact placement's
//! inverse; a general placement's enclosure; the frames' shift, [`super::enter`]), then queries the
//! definition's parts tree, or, for a nested assembly, goes on in it. Nested levels wait on an
//! explicit stack, never a recursive call, and no path is longer than [`MAX_NESTING`].
//!
//! Guarantees:
//!
//! - **The answer of brute force, through every level.** A [`Hit`] is reported for exactly each
//!   part whose box meets the region moved into its definition's frame along its path, as testing
//!   every part of every instance would find; through an exact placement and a frame that fits,
//!   that is exactly where the materialised part meets the region (23.9). Hits are sorted by path,
//!   record index by record index, then by part key, so no tree's shape, pool or thread count
//!   changes an answer.
//! - **Never a miss.** Through a general placement, or a frame shift that falls back to the coarse
//!   frame, the hit set holds every part a materialisation could put in the region, and each such
//!   hit says it is not exact ([`Hit::exact`]): its comparison is Certified (19.12).
//! - **Checked where it is made.** Every record's box holds its definition's box as its placement
//!   moves it (outward through a general transform), so the instance tree cannot miss an instance;
//!   a record that fails is refused with its index, and nothing is built.
//! - **Bounded.** Paths are at most [`MAX_NESTING`] records long; the pending levels are a vector;
//!   the meter is charged one unit per tree node visited and per instance entered, and polls the
//!   caller's cancellation. A stopped query returns no hits.
//!
//! Method: the two-level tree of chapter 19.12 (instance boxes above, each definition's tree in
//! its own frame below, the query moved into the definition's frame), as evidence report 08
//! section C proposes after IFC's mapped items
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`,
//! `projects/kit/notes/G01-evidence/08-standards-coverage-testing.md`).
//! Consulted: NVIDIA USD `BRep` `SMLib`, Apache-2.0,
//!   <https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmAssemblyInstance.h>,
//!   for its model of assemblies and instances (a placement, a scale, a mirror flag and a
//!   component per instance); it has no spatial index of instances, and its per-instance scale is
//!   what 19.12 makes a new definition instead. No code copied.
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/78d1eb12d222652219313c12787b65389c4d9db8/crates/solidcore/src/topology/proto_topology/p1/assembly.rs>,
//!   the Rust port of the same model; no spatial index of instances there either. No code copied.
//!
//! Cost: building: one check per record (a transform of two corners, or an interval enclosure for
//! a general placement), one frame shift per distinct definition, and the instance tree's build.
//! Memory kept: the records, the tree, and one entry per distinct definition placed. A query: the
//! tree walks, plus a constant per instance entered (an interval enclosure, about 100 operations,
//! through a general placement). Recursion: none.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use geom_math::{Box3, FrameRecord, TypedTransform};
use geom_run::{Exec, WorkMeter};

use super::definitions::{Content, DefinitionKey, Definitions, MAX_NESTING};
use super::enter::{Relation, backward, forward};
use super::error::InstanceError;
use super::record::{Place, Record};
use super::region::Region;
use super::tree::{InstanceTree, parts_meeting, put};

/// One definition an assembly places: its handle, its key when the assembly was built, how its
/// frame stands to the assembly's, and its box in the assembly's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Child {
    handle: u32,
    key: DefinitionKey,
    relation: Relation,
    image: Box3,
}

/// An assembly (module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly<R> {
    frame: FrameRecord,
    records: Vec<R>,
    tree: InstanceTree,
    /// One per distinct definition placed, by handle.
    children: Vec<Child>,
}

/// The record indices from a queried assembly down to one instance: at most [`MAX_NESTING`].
#[derive(Clone, Copy, Debug)]
pub struct Path {
    len: u8,
    records: [u32; MAX_NESTING],
}

impl Path {
    /// The empty path.
    pub const EMPTY: Path = Path {
        len: 0,
        records: [0; MAX_NESTING],
    };

    /// The record indices, outermost first.
    #[must_use]
    pub fn records(&self) -> &[u32] {
        self.records.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// This path and then `record`.
    ///
    /// # Errors
    ///
    /// [`InstanceError::TooDeep`] past [`MAX_NESTING`] records.
    pub fn then(&self, record: u32) -> Result<Path, InstanceError> {
        let mut next = *self;
        *next
            .records
            .get_mut(usize::from(self.len))
            .ok_or(InstanceError::TooDeep)? = record;
        next.len = self.len.checked_add(1).ok_or(InstanceError::TooDeep)?;
        Ok(next)
    }
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.records() == other.records()
    }
}

impl Eq for Path {}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Path {
    /// Record by record, outermost first; a path before every path it starts.
    fn cmp(&self, other: &Self) -> Ordering {
        self.records().cmp(other.records())
    }
}

impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.records().hash(state);
    }
}

/// One part of one instance that a query found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    /// The record indices from the queried assembly to the instance whose definition holds the
    /// part.
    pub path: Path,
    /// The part's key in its definition's tree.
    pub part: u64,
    /// The definition's handle.
    pub definition: u32,
    /// Whether every placement and frame shift on the path was exact: the instance is its
    /// definition moved exactly, and the part was compared in its own frame. Otherwise the path
    /// crossed a general placement or a frame that did not fit, and comparisons on the part are
    /// Certified (19.12).
    pub exact: bool,
}

/// A level of a query waiting on the stack: an assembly, the region in its frame, the path to it,
/// and whether that path was exact.
struct Level<'a, R> {
    assembly: &'a Assembly<R>,
    region: Region,
    path: Path,
    exact: bool,
}

impl<R: Record> Assembly<R> {
    /// The assembly of `records` in `frame` (module documentation). `defs` holds every definition
    /// the records place; `side` is the document's side table of general transforms.
    ///
    /// # Errors
    ///
    /// [`InstanceError::NoDefinition`], [`InstanceError::NoTransform`] or
    /// [`InstanceError::Placement`] for a record that names what is not there;
    /// [`InstanceError::Frame`] for a definition whose frame has no exact shift to `frame`;
    /// [`InstanceError::Unbounded`] for a record whose box does not hold its definition as placed;
    /// [`InstanceError::Num`] for a definition that does not fit `frame`'s range;
    /// [`InstanceError::TooDeep`] past [`MAX_NESTING`]; the instance tree's errors.
    pub fn build(
        frame: FrameRecord,
        records: Vec<R>,
        defs: &Definitions<R>,
        side: &[TypedTransform],
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Assembly<R>, InstanceError> {
        if u32::try_from(records.len()).is_err() {
            return Err(InstanceError::TooManyRecords);
        }
        meter.poll()?;
        let mut children: BTreeMap<u32, Child> = BTreeMap::new();
        let mut deepest = 0u8;
        let mut last: Option<Child> = None;
        for (index, r) in (0u32..).zip(&records) {
            meter.charge(1)?;
            let handle = r.definition();
            let child = match (last, children.get(&handle)) {
                (Some(c), _) if c.handle == handle => c,
                (_, Some(c)) => *c,
                (_, None) => {
                    let d = defs
                        .get(handle)
                        .ok_or(InstanceError::NoDefinition(handle))?;
                    let relation = Relation::between(&frame, d.frame())?;
                    let c = Child {
                        handle,
                        key: d.key(),
                        relation,
                        image: relation.outward(d.bound())?,
                    };
                    deepest = deepest.max(d.depth());
                    children.insert(handle, c);
                    c
                }
            };
            last = Some(child);
            let need = match r.place()? {
                Place::Exact(t) => child.image.transformed(&t)?,
                Place::General(k) => forward(
                    side.get(usize::try_from(k).map_err(|_| InstanceError::NoTransform(k))?)
                        .ok_or(InstanceError::NoTransform(k))?,
                    &child.image,
                )?,
            };
            if !r.bound().contains_box(&need) {
                return Err(InstanceError::Unbounded(index));
            }
        }
        if usize::from(deepest) >= MAX_NESTING {
            return Err(InstanceError::TooDeep);
        }
        let tree = InstanceTree::build(&records, exec, meter)?;
        let mut kept = Vec::new();
        kept.try_reserve_exact(children.len())
            .map_err(|_| InstanceError::OutOfMemory)?;
        kept.extend(children.into_values());
        Ok(Assembly {
            frame,
            records,
            tree,
            children: kept,
        })
    }

    /// The records whose boxes meet `region`, by index, ascending: this level only.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Stopped`] if the meter stops; [`InstanceError::OutOfMemory`].
    pub fn overlapping(
        &self,
        region: &Box3,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<u32>, InstanceError> {
        self.tree.overlapping(&self.records, region, meter)
    }

    /// Every part of every instance, through every nested level, whose box meets `region` (in this
    /// assembly's frame) moved into its definition's frame (module documentation). `defs` and
    /// `side` are those the assembly was built with.
    ///
    /// # Errors
    ///
    /// [`InstanceError::Stale`] if `defs` holds another definition under a handle the assembly
    /// placed; [`InstanceError::NoTransform`] for a side table without a placement's entry;
    /// [`InstanceError::Transform`] for a general placement with no inverse;
    /// [`InstanceError::Stopped`] if the meter stops; [`InstanceError::OutOfMemory`].
    pub fn query(
        &self,
        defs: &Definitions<R>,
        side: &[TypedTransform],
        region: &Box3,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Hit>, InstanceError> {
        let mut hits = Vec::new();
        let mut pending = Vec::new();
        put(
            &mut pending,
            Level {
                assembly: self,
                region: Region::from_box(region),
                path: Path::EMPTY,
                exact: true,
            },
        )?;
        while let Some(level) = pending.pop() {
            // Each level reads the caller's cancellation once, however few nodes it visits.
            meter.poll()?;
            let a = level.assembly;
            for r in a.tree.meeting(&a.records, &level.region, meter)? {
                meter.charge(1)?;
                let record = a
                    .records
                    .get(usize::try_from(r).map_err(|_| InstanceError::Defect)?)
                    .ok_or(InstanceError::Defect)?;
                let child = a.child(record.definition())?;
                let def = defs
                    .get(child.handle)
                    .ok_or(InstanceError::NoDefinition(child.handle))?;
                if def.key() != child.key {
                    return Err(InstanceError::Stale(child.handle));
                }
                let moved = match record.place()? {
                    Place::Exact(t) => level
                        .region
                        .clip(&child.image.transformed(&t)?)?
                        .map(|kept| kept.unplace(&t))
                        .transpose()?
                        .map(|b| (b, true)),
                    Place::General(k) => {
                        let t = side
                            .get(usize::try_from(k).map_err(|_| InstanceError::NoTransform(k))?)
                            .ok_or(InstanceError::NoTransform(k))?;
                        match level.region.clip(record.bound())? {
                            Some(r1) => backward(t, &r1.outward()?, &child.image)?
                                .map(|b| (Region::from_box(&b), false)),
                            None => None,
                        }
                    }
                };
                let Some((in_assembly, placed_exactly)) = moved else {
                    continue;
                };
                let Some((in_frame, frame_exactly)) = child.relation.enter(&in_assembly)? else {
                    continue;
                };
                let Some(in_def) = in_frame.clip(def.bound())? else {
                    continue;
                };
                let path = level.path.then(r)?;
                let exact = level.exact && placed_exactly && frame_exactly;
                match def.content() {
                    Content::Parts { tree, .. } => {
                        for part in parts_meeting(tree, &in_def, meter)? {
                            put(
                                &mut hits,
                                Hit {
                                    path,
                                    part,
                                    definition: child.handle,
                                    exact,
                                },
                            )?;
                        }
                    }
                    Content::Assembly(inner) => put(
                        &mut pending,
                        Level {
                            assembly: inner,
                            region: in_def,
                            path,
                            exact,
                        },
                    )?,
                }
            }
        }
        hits.sort_unstable();
        Ok(hits)
    }
}

impl<R> Assembly<R> {
    /// Its frame.
    #[must_use]
    pub const fn frame(&self) -> &FrameRecord {
        &self.frame
    }

    /// Its records, in the order given: a record's index is its position here.
    #[must_use]
    pub fn records(&self) -> &[R] {
        &self.records
    }

    /// Its instance tree.
    #[must_use]
    pub const fn tree(&self) -> &InstanceTree {
        &self.tree
    }

    /// How many records it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The box of every record, or `None` for no records.
    ///
    /// # Errors
    ///
    /// As [`InstanceTree::bound`].
    pub fn bound(&self) -> Result<Option<Box3>, InstanceError> {
        self.tree.bound()
    }

    /// The handle and key of every definition it places, by handle.
    pub fn placed(&self) -> impl Iterator<Item = (u32, DefinitionKey)> + '_ {
        self.children.iter().map(|c| (c.handle, c.key))
    }

    /// The heap bytes it holds at this target's sizes: its records', its tree's and its table of
    /// placed definitions' capacities.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.records
            .capacity()
            .saturating_mul(size_of::<R>())
            .saturating_add(self.tree.heap_bytes())
            .saturating_add(self.children.capacity().saturating_mul(size_of::<Child>()))
    }

    /// The entry for the definition with handle `handle`.
    fn child(&self, handle: u32) -> Result<&Child, InstanceError> {
        let i = self
            .children
            .binary_search_by_key(&handle, |c| c.handle)
            .map_err(|_| InstanceError::Defect)?;
        self.children.get(i).ok_or(InstanceError::Defect)
    }
}
