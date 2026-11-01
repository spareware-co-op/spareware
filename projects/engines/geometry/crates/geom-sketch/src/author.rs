//! Authoring macros: the plain constraints a named operation writes for its caller.
//!
//! What it does: [`fix_entity`] gives the constraints that hold a whole entity at its values, and
//! [`offset_chain`] the ones that tie each piece of a chain of lines and arcs to its offset copy:
//! parallel and a distance apart for a line, concentric and a radius apart for an arc (chapter 15.2).
//!
//! Guarantees:
//!
//! - **Plain constraints.** A macro returns constraints a caller could write by hand, and changes
//!   nothing, so a refusal leaves the sketch as it was.
//! - **Replayable.** Keys run up from the caller's `first_key`, in the pieces' order, so one replay
//!   authors one set of ids.
//! - **One distance.** Every piece of an offset takes one [`Amount`], so changing a chain's distance
//!   is a change of value (chapter 15.7).
//!
//! Method: section 15.2's offset chains and section 15.3's fixing
//! (`projects/engines/geometry/architecture/15-sketches.md`).
//!
//! Cost: [`fix_entity`] is linear in the sketch's variables and [`offset_chain`] in its pieces. No
//! recursion.

extern crate alloc;

use alloc::vec::Vec;

use crate::model::{
    Amount, Constraint, ConstraintKind, EntityKind, ItemId, Side, Sketch, SketchError, Standing,
};

/// Keys in order from a first one.
struct Author {
    sequence: u64,
    key: u64,
    list: Vec<Constraint>,
}

impl Author {
    const fn new(sequence: u64, first_key: u64) -> Author {
        Author {
            sequence,
            key: first_key,
            list: Vec::new(),
        }
    }

    fn add(&mut self, kind: ConstraintKind) -> Result<(), SketchError> {
        self.list
            .try_reserve(1)
            .map_err(|_| SketchError::OutOfMemory)?;
        self.list.push(Constraint {
            id: ItemId::new(self.sequence, self.key),
            kind,
            standing: Standing::Driving,
            priority: 0,
        });
        self.key = self.key.checked_add(1).ok_or(SketchError::Internal)?;
        Ok(())
    }
}

/// The entities whose variables make up `id`: its points as well as itself.
fn parts(sketch: &Sketch, id: ItemId) -> Result<Vec<ItemId>, SketchError> {
    let mut list = Vec::new();
    let mut add = |ids: &[ItemId]| {
        list.try_reserve(ids.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        list.extend_from_slice(ids);
        Ok::<(), SketchError>(())
    };
    match &sketch.entity(id)?.kind {
        EntityKind::Line { start, end, .. } => add(&[*start, *end])?,
        EntityKind::Circle { centre, .. } | EntityKind::Ellipse { centre, .. } => {
            add(&[id, *centre])?;
        }
        EntityKind::Arc {
            centre, start, end, ..
        } => add(&[id, *centre, *start, *end])?,
        EntityKind::Spline { points, .. } => add(points)?,
        EntityKind::Parameter { curve, .. } => add(&[id, *curve])?,
        _ => add(&[id])?,
    }
    Ok(list)
}

/// The constraints that hold `entity` at its values: a fix for each of its variables and of its
/// points' variables. A reference has none, and needs none.
///
/// # Errors
///
/// [`SketchError::Unknown`]; [`SketchError::OutOfMemory`].
pub fn fix_entity(
    sketch: &Sketch,
    entity: ItemId,
    sequence: u64,
    first_key: u64,
) -> Result<Vec<Constraint>, SketchError> {
    let owners = parts(sketch, entity)?;
    let mut author = Author::new(sequence, first_key);
    for variable in sketch.variables()? {
        if owners.contains(&variable.owner) {
            author.add(ConstraintKind::Fix {
                entity: variable.owner,
                slot: variable.slot,
                value: variable.value,
            })?;
        }
    }
    Ok(author.list)
}

/// The constraints that tie each of `pieces`, a line or an arc and its copy, to `by` from it on `side`
/// of its direction: left of an anticlockwise arc is towards its centre.
///
/// # Errors
///
/// [`SketchError::WrongKind`] for a piece that is not a line or a curve of a centre with a copy of
/// its own kind; [`SketchError::Unknown`]; [`SketchError::OutOfMemory`].
pub fn offset_chain(
    sketch: &Sketch,
    pieces: &[(ItemId, ItemId)],
    by: Amount,
    side: Side,
    sequence: u64,
    first_key: u64,
) -> Result<Vec<Constraint>, SketchError> {
    let mut author = Author::new(sequence, first_key);
    for &(from, to) in pieces {
        if sketch.line(from).is_ok() {
            sketch.line(to)?;
            author.add(ConstraintKind::Parallel { a: from, b: to })?;
            author.add(ConstraintKind::LinesApart {
                a: from,
                b: to,
                distance: by,
                side,
            })?;
        } else {
            sketch.centre(from)?;
            sketch.centre(to)?;
            author.add(ConstraintKind::Concentric { a: from, b: to })?;
            author.add(ConstraintKind::Offset {
                from,
                to,
                by,
                grows: side == Side::Right,
            })?;
        }
    }
    Ok(author.list)
}
