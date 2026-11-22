//! Placed instances that are not exact: an instance's geometry is its materialisation, the shared
//! body made into its own body by the placement, rounded to the grid, re-validated and labelled
//! with its class, made once into a caller-owned cache (23.9, 19.12; piece G145).
//!
//! What it does: [`materialise`] makes the body of a placement: `crate::transform`'s copy, in name
//! order, its history kept, then every checker group at the full level whatever the build. A
//! [`Library`] is the caller's cache of those bodies, keyed by the shared body's content hash and
//! the placement, and answers the queries of `geom_topo::placed` for every instance: an exact
//! placement moves the question into the shared frame, a general one asks its materialised body.
//!
//! Guarantees:
//!
//! - **One answer either way.** Every query on an instance equals the same query on its
//!   materialisation: `tests/placed.rs` holds that for all 48 signed permutations and for rotations
//!   that round.
//! - **Made once, and only when asked.** A placement's body is made the first time a query needs
//!   it, and an exact placement never makes one.
//! - **Never an unchecked body.** A materialised body has passed every checker group, in every
//!   build; one that does not is [`MakeDetail::ResultInvalid`] or
//!   [`MakeDetail::NotCertifiedYet`], and nothing is cached.
//! - **A rotation only.** A placement that does not keep lengths is refused where it is made
//!   (`geom_topo::placed::Placed::general`); a scale makes a new definition (19.12).
//!
//! Method: chapter 23.9's "What a placed body is" and 19.12's placements
//! (`projects/engines/geometry/architecture/23-performance.md`,
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`).
//!
//! Cost: a transform and a full check the first time a placement is asked of, one ordered-map
//! lookup after that. The cache's size is the caller's. Recursion: none.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use geom_check::{Level, check};
use geom_math::TypedTransform;
use geom_num::wide::I256;
use geom_num::{Exactness, P3};
use geom_op::{MakeDetail, OpContext, OpError, OpOutput};
use geom_topo::placed::{self, Placed, PlacedError, Placement};
use geom_topo::query::{self, FaceRow, Side};
use geom_topo::query_ops::failed;
use geom_topo::{Body, FaceIx, ShellIx};

use crate::build::{Made, Motion, PHASE, fail};
use crate::transform::{Moved, placed_copy};

/// The body of `shared` under `t`, checked at the full level whatever the build.
///
/// # Errors
///
/// The transform's errors; [`MakeDetail::ResultInvalid`] for a body the checker refuses, and the
/// context's budget and cancellation.
pub fn materialise(
    ctx: &OpContext<'_>,
    shared: Moved<'_>,
    t: &TypedTransform,
) -> Result<OpOutput<Made>, OpError> {
    let made = placed_copy(ctx, shared, t, Motion::Rotate)?;
    let mut meter = ctx.meter();
    let report = check(&made.result.body, made.exactness, Level::Full, &mut meter)
        .map_err(|stop| ctx.stopped(&meter, stop, PHASE))?;
    if report.passed() {
        Ok(made)
    } else {
        let first = report.faults().first().map_or_else(
            || alloc::string::String::from("the checker reported faults"),
            alloc::string::ToString::to_string,
        );
        Err(fail(ctx, MakeDetail::ResultInvalid).with_message(first))
    }
}

/// One definition, as a [`Library`] asks of it.
#[derive(Clone, Copy, Debug)]
pub struct Shared<'b, K> {
    /// Its content hash, or whatever else the caller keys it by.
    pub key: K,
    /// The body, in its own frame.
    pub body: &'b Body,
    /// Its exactness record.
    pub exactness: Exactness,
    /// The class and bound the caller accepts for a rounded body; `None` accepts none.
    pub accept: Option<Exactness>,
}

/// The caller's cache of materialised bodies, by definition and placement.
#[derive(Clone, Debug)]
pub struct Library<'t, K> {
    table: &'t [TypedTransform],
    bodies: BTreeMap<(K, Placement), OpOutput<Made>>,
}

impl<'t, K: Ord + Copy> Library<'t, K> {
    /// A library over the document's side table of general transforms, holding no body yet.
    #[must_use]
    pub const fn new(table: &'t [TypedTransform]) -> Self {
        Library {
            table,
            bodies: BTreeMap::new(),
        }
    }

    /// How many bodies it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// The body of a general placement, made if it has not been.
    ///
    /// # Errors
    ///
    /// [`materialise`]'s, and [`MakeDetail::NoSuchPlacement`] for an index past the table.
    pub fn body(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        placement: Placement,
    ) -> Result<&OpOutput<Made>, OpError> {
        let key = (shared.key, placement);
        if !self.bodies.contains_key(&key) {
            let Placement::General(index) = placement else {
                let t = placement.grid().map_err(|e| trouble(ctx, e))?;
                return self.keep(ctx, shared, key, &TypedTransform::Exact(t));
            };
            let row = usize::try_from(index).ok().and_then(|i| self.table.get(i));
            let t = *row.ok_or_else(|| fail(ctx, MakeDetail::NoSuchPlacement { index }))?;
            return self.keep(ctx, shared, key, &t);
        }
        self.bodies
            .get(&key)
            .ok_or_else(|| fail(ctx, MakeDetail::BuildFailed))
    }

    /// Makes and keeps the body at `key`.
    fn keep(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        key: (K, Placement),
        t: &TypedTransform,
    ) -> Result<&OpOutput<Made>, OpError> {
        let moved = Moved {
            body: shared.body,
            exactness: shared.exactness,
            accept: shared.accept,
        };
        let made = materialise(ctx, moved, t)?;
        Ok(self.bodies.entry(key).or_insert(made))
    }

    /// Twice the area vector of `face` of the instance.
    ///
    /// # Errors
    ///
    /// The query's, and a general placement's [`Library::body`].
    pub fn face_area(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        instance: &Placed,
        face: FaceIx,
    ) -> Result<[i128; 3], OpError> {
        let mut meter = ctx.meter();
        if matches!(instance.placement(), Placement::Exact { .. }) {
            return placed::face_area(shared.body, instance, face, &mut meter)
                .map_err(|e| trouble(ctx, e));
        }
        let made = self.body(ctx, shared, instance.placement())?;
        query::face_area(&made.result.body, face, &mut meter).map_err(|e| failed(ctx, &meter, e))
    }

    /// Six times the volume of the instance.
    ///
    /// # Errors
    ///
    /// The query's, and a general placement's [`Library::body`].
    pub fn six_volume(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        instance: &Placed,
    ) -> Result<I256, OpError> {
        let mut meter = ctx.meter();
        if matches!(instance.placement(), Placement::Exact { .. }) {
            return placed::six_volume(shared.body, instance, &mut meter)
                .map_err(|e| trouble(ctx, e));
        }
        let made = self.body(ctx, shared, instance.placement())?;
        query::volume(&made.result.body, &mut meter).map_err(|e| failed(ctx, &meter, e))
    }

    /// Which side of shell `s` of the instance `p` lies on.
    ///
    /// # Errors
    ///
    /// The query's, and a general placement's [`Library::body`].
    pub fn side_of_shell(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        instance: &Placed,
        s: ShellIx,
        p: P3,
    ) -> Result<Option<Side>, OpError> {
        let mut meter = ctx.meter();
        if matches!(instance.placement(), Placement::Exact { .. }) {
            return placed::side_of_shell(shared.body, instance, s, p, &mut meter)
                .map_err(|e| trouble(ctx, e));
        }
        let made = self.body(ctx, shared, instance.placement())?;
        query::side_of_shell(&made.result.body, s, p, &mut meter)
            .map_err(|e| failed(ctx, &meter, e))
    }

    /// Every face of the instance.
    ///
    /// # Errors
    ///
    /// The query's, and a general placement's [`Library::body`].
    pub fn faces(
        &mut self,
        ctx: &OpContext<'_>,
        shared: &Shared<'_, K>,
        instance: &Placed,
    ) -> Result<Vec<FaceRow>, OpError> {
        let mut meter = ctx.meter();
        if matches!(instance.placement(), Placement::Exact { .. }) {
            return placed::faces(shared.body, instance, &mut meter).map_err(|e| trouble(ctx, e));
        }
        let made = self.body(ctx, shared, instance.placement())?;
        query::faces(&made.result.body, &mut meter).map_err(|e| failed(ctx, &meter, e))
    }
}

/// The contract's error for a placed query's `error`.
fn trouble(ctx: &OpContext<'_>, error: PlacedError) -> OpError {
    let meter = ctx.meter();
    match error {
        PlacedError::Query(e) => failed(ctx, &meter, e),
        PlacedError::Num(e) => OpError::new(ctx.path().clone(), e),
        PlacedError::General | PlacedError::Permutation(_) | PlacedError::NotRigid => {
            fail(ctx, MakeDetail::BuildFailed)
        }
    }
}
