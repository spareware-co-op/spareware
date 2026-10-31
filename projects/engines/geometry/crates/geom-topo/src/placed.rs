//! Placed instances: a definition stored once and placed many times, copying no geometry (19.12,
//! 23.9; piece G145).
//!
//! What it does: [`Placed`] is the kernel's one instance record, at most 88 bytes: a handle to its
//! definition, its [`Placement`] and its integer box. A definition is a body in the caller's table,
//! in its own frame. An exact placement is a grid shift with one of the 48 signed axis
//! permutations, held packed; any other is an index into the document's side table of general
//! transforms. On an exact placement the queries here move the question into the definition's frame
//! and answer it there: [`to_shared`] and [`from_shared`] carry a point across, [`face_area`] and
//! [`six_volume`] read the shared body, [`side_of_shell`] takes the point back first, and [`faces`]
//! lists the shared rows with a mirror's senses turned. A general placement has no query here: its
//! geometry is its materialisation, the body rounded to the grid and re-validated, which needs the
//! operations above this crate (`geom_make::placed`), and each of these answers
//! [`PlacedError::General`] for it.
//!
//! Guarantees:
//!
//! - **88 bytes.** A test holds [`Placed`]'s size at most 88 on every target, and its logical size
//!   is 88 whatever the target's layout (23.10).
//! - **Exact placements are exact.** Every step is integer arithmetic on signed permutations, so an
//!   answer equals the same query on the materialised body, bit for bit; `geom-make`'s tests hold
//!   every query to it for all 48 permutations. Twice a face's area vector turns with the body and
//!   ignores the shift; a mirror reverses the loops it reflects, so outward stays outward and the
//!   area vector is the plain image; six times the volume is unchanged; a side of a shell is
//!   unchanged.
//! - **The box is held once and holds the body.** An exact placement's box is the definition's box
//!   moved, exact where that box is; a general one's holds every exact image of the definition's
//!   box and every rounding of it, one step out.
//! - **Refused, never guessed.** A packed permutation past 47, or a point the placement sends out
//!   of the grid's range, is a [`PlacedError`].
//!
//! Method: chapter 19.12's definitions and instances, as IFC's mapped items place them, and 23.9's
//! rule that an instance is defined as its materialisation
//! (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`,
//! `projects/engines/geometry/architecture/23-performance.md`), on `geom_math`'s signed
//! permutations (`geom_math::GridTransform`).
//!
//! Cost: constant per point; a shared query's cost otherwise, charged as `crate::query` says.
//! Nothing allocated but [`faces`]'s list. Recursion: none.

use alloc::vec::Vec;
use core::fmt;

use geom_math::{Axis, Box3, GridTransform, TypedTransform};
use geom_num::wide::I256;
use geom_num::{Coord, NumError, P3};
use geom_run::{LogicalSize, WorkMeter};

use crate::body::Body;
use crate::entity::{FaceIx, Sense, ShellIx};
use crate::query::{self, FaceRow, QueryError, Side};

/// A handle to a definition in the caller's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Definition(pub u32);

/// Where an instance goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Placement {
    /// A grid shift with one of the 48 signed axis permutations.
    Exact {
        /// The shift, in steps.
        shift: [i64; 3],
        /// The permutation: 8 × the arrangement of the source axes + the three sign bits.
        axes: u8,
    },
    /// An index into the document's side table of general transforms.
    General(u32),
}

/// Why a placement could not answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacedError {
    /// The placement is general, and its answers come from its materialisation.
    General,
    /// A general placement that does not keep lengths: a scale makes a new definition (19.12).
    NotRigid,
    /// A packed permutation past 47.
    Permutation(u8),
    /// A value out of the grid's range or the transform's budget.
    Num(NumError),
    /// The query on the shared body.
    Query(QueryError),
}

impl fmt::Display for PlacedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacedError::General => f.write_str("a general placement answers through its body"),
            PlacedError::NotRigid => f.write_str("a placement that does not keep lengths"),
            PlacedError::Permutation(p) => write!(f, "{p} is not one of the 48 permutations"),
            PlacedError::Num(e) => write!(f, "{e}"),
            PlacedError::Query(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for PlacedError {}

impl From<NumError> for PlacedError {
    fn from(e: NumError) -> Self {
        PlacedError::Num(e)
    }
}

impl From<QueryError> for PlacedError {
    fn from(e: QueryError) -> Self {
        PlacedError::Query(e)
    }
}

/// The six arrangements of the source axes, in the order [`Placement::Exact`]'s `axes` counts them.
const ARRANGEMENTS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

impl Placement {
    /// The packed form of `t`.
    #[must_use]
    pub fn exact(t: &GridTransform) -> Placement {
        let source = t.source();
        let arrangement = ARRANGEMENTS
            .iter()
            .zip(0_u8..)
            .find_map(|(a, n)| (*a == source).then_some(n))
            .unwrap_or(0);
        let signs = t
            .negate()
            .iter()
            .zip(0_u8..)
            .fold(0_u8, |bits, (&n, at)| bits | (u8::from(n) << at));
        Placement::Exact {
            shift: t.translation_part(),
            axes: arrangement.saturating_mul(8).saturating_add(signs),
        }
    }

    /// The exact transform.
    ///
    /// # Errors
    ///
    /// [`PlacedError::General`] for a general placement, [`PlacedError::Permutation`] and
    /// [`PlacedError::Num`] for one that is not a grid transform.
    pub fn grid(&self) -> Result<GridTransform, PlacedError> {
        let Placement::Exact { shift, axes } = *self else {
            return Err(PlacedError::General);
        };
        let source = ARRANGEMENTS
            .get(usize::from(axes >> 3))
            .ok_or(PlacedError::Permutation(axes))?;
        let negate = [axes & 1 != 0, axes & 2 != 0, axes & 4 != 0];
        Ok(GridTransform::from_parts(*source, negate, shift)?)
    }
}

/// One instance: at most 88 bytes (19.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Placed {
    definition: Definition,
    placement: Placement,
    bound: Box3,
}

impl LogicalSize for Placed {
    /// Chapter 19.12's budget for the record.
    const LOGICAL_BYTES: u64 = 88;
}

impl Placed {
    /// An instance of `definition`, whose body has box `shared`, placed exactly by `t`.
    ///
    /// # Errors
    ///
    /// [`PlacedError::Num`] for a box the transform sends out of the grid's range.
    pub fn exact(
        definition: Definition,
        t: &GridTransform,
        shared: &Box3,
    ) -> Result<Placed, PlacedError> {
        let (a, b) = (t.apply(shared.min())?, t.apply(shared.max())?);
        Ok(Placed {
            definition,
            placement: Placement::exact(t),
            bound: Box3::from_points([a, b]).ok_or(NumError::Degenerate)?,
        })
    }

    /// An instance of `definition`, whose body has box `shared`, placed by `t`, entry `index` of
    /// the document's side table.
    ///
    /// # Errors
    ///
    /// [`PlacedError::NotRigid`] for a transform that does not keep lengths, and
    /// [`PlacedError::Num`] for a box the transform sends out of the grid's range.
    pub fn general(
        definition: Definition,
        index: u32,
        t: &TypedTransform,
        shared: &Box3,
    ) -> Result<Placed, PlacedError> {
        if !t.classify()?.keeps_lengths() {
            return Err(PlacedError::NotRigid);
        }
        let (lo, hi) = (shared.min(), shared.max());
        let corners = [0_u8, 1, 2, 3, 4, 5, 6, 7].map(|bits| {
            let pick = |bit: u8, low: Coord, high: Coord| {
                if bits & bit == 0 { low } else { high }
            };
            P3 {
                x: pick(1, lo.x, hi.x),
                y: pick(2, lo.y, hi.y),
                z: pick(4, lo.z, hi.z),
            }
        });
        let mut bound: Option<Box3> = None;
        for corner in corners {
            let image = t.apply(corner)?.value();
            for d in [-1, 1] {
                let moved = |c: Coord| c.get().saturating_add(d);
                let far = P3::new(moved(image.x), moved(image.y), moved(image.z))?;
                let one = Box3::new(far, far)?;
                bound = Some(bound.map_or(one, |b| b.union(&one)));
            }
        }
        Ok(Placed {
            definition,
            placement: Placement::General(index),
            bound: bound.ok_or(NumError::Degenerate)?,
        })
    }

    /// The definition.
    #[must_use]
    pub const fn definition(&self) -> Definition {
        self.definition
    }

    /// The placement.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// The box, held once.
    #[must_use]
    pub const fn bound(&self) -> &Box3 {
        &self.bound
    }
}

/// `p` of the placed instance, in the definition's frame.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, and a point out of range.
pub fn to_shared(placed: &Placed, p: P3) -> Result<P3, PlacedError> {
    Ok(placed.placement.grid()?.inverse()?.apply(p)?)
}

/// `p` of the definition, where the placed instance has it.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, and a point out of range.
pub fn from_shared(placed: &Placed, p: P3) -> Result<P3, PlacedError> {
    Ok(placed.placement.grid()?.apply(p)?)
}

/// Every face of `shared` as `placed` has it: a mirror turns each sense.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, and the query's errors.
pub fn faces(
    shared: &Body,
    placed: &Placed,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<FaceRow>, PlacedError> {
    let mirror = placed.placement.grid()?.reverses_orientation();
    let mut rows = query::faces(shared, meter)?;
    if mirror {
        for row in &mut rows {
            row.sense = match row.sense {
                Sense::Forward => Sense::Reversed,
                Sense::Reversed => Sense::Forward,
            };
        }
    }
    Ok(rows)
}

/// Twice the area vector of `face` as `placed` has it.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, and the query's errors.
pub fn face_area(
    shared: &Body,
    placed: &Placed,
    face: FaceIx,
    meter: &mut WorkMeter<'_>,
) -> Result<[i128; 3], PlacedError> {
    let t = placed.placement.grid()?;
    let area = query::face_area(shared, face, meter)?;
    let source = t.source();
    let negate = t.negate();
    let image = |i: usize| -> Result<i128, PlacedError> {
        let from = source
            .get(i)
            .copied()
            .ok_or(QueryError::NoSuchRow)?
            .pick(area);
        Ok(if negate.get(i).copied().unwrap_or(false) {
            from.checked_neg().ok_or(QueryError::Overflow)?
        } else {
            from
        })
    };
    Ok([image(0)?, image(1)?, image(2)?])
}

/// Six times the volume of `shared` as `placed` has it: the same.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, and the query's errors.
pub fn six_volume(
    shared: &Body,
    placed: &Placed,
    meter: &mut WorkMeter<'_>,
) -> Result<I256, PlacedError> {
    placed.placement.grid()?;
    Ok(query::volume(shared, meter)?)
}

/// Which side of shell `s` of the placed instance `p` lies on.
///
/// # Errors
///
/// [`PlacedError::General`] for a general placement, a point out of range, and the query's errors.
pub fn side_of_shell(
    shared: &Body,
    placed: &Placed,
    s: ShellIx,
    p: P3,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Side>, PlacedError> {
    Ok(query::side_of_shell(
        shared,
        s,
        to_shared(placed, p)?,
        meter,
    )?)
}
