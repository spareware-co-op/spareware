//! The 2D index: one tree of integer boxes over a drawing, and the snap query on it (chapter 14.8).
//!
//! What it does: [`SnapIndex::build`] puts a box round each snappable entity of a [`Drawing`] into
//! `geom-bvh`'s box tree, keyed by the entity's place in name order. [`SnapIndex::snap`] takes a
//! cursor as a grid point and an aperture in steps, asks the tree for the entities whose boxes meet
//! the square of that aperture, makes the snaps of the kinds asked for on each (an entity's own
//! points, and the crossings of the entities near the cursor in pairs), and returns the closest
//! that is within the aperture. A tie goes to the lower name. There is no scan of the drawing and no
//! cap on candidates (14.8; `CADCraft` tests at most 200).
//!
//! Guarantees:
//!
//! - **Never a miss.** An entity's box holds everything a snap of it can be: the box is rounded
//!   outward to whole steps from the exact ends, centre and radius, so an entity with a snap point
//!   in the aperture has a box that meets the aperture's square. An unbounded line gets the whole
//!   frame; an arc, the box of its circle; an ellipse, the square of its major axis; a polyline,
//!   its vertices grown by each bulge's sagitta; a spline, its control points (the convex hull
//!   property); a text or insert, its anchor, the only snap it has.
//! - **One answer, whatever the tree.** The candidates come back sorted by key, which is name
//!   order, so the result does not depend on the tree's shape or the threads that built it
//!   (`geom-bvh`'s guarantee).
//! - **Closest wins; ties go to the lower name.** Distances are enclosed in intervals from the exact
//!   points; a candidate wins only if its whole interval is below the best's, and otherwise ties,
//!   so two equal distances (a symmetric pair of quadrants) are ordered by name, and two that
//!   differ by less than the width of the enclosure (a few units in the last place) are too.
//! - **No recursion** (the tree's walk is `geom-bvh`'s explicit stack) and **no unsafe code**.
//!
//! Method: chapter 14.8 and 19.9 of the kernel architecture
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); the box tree is `geom-bvh`'s
//! (piece G54).
//!
//! Cost: building is O(n log n) for n entities, a box for each; a query is the tree's walk over the
//! aperture's square plus the snaps of the entities in it, none for those outside.

#![allow(
    clippy::many_single_char_names,
    reason = "a, b and c are points and lines, d a direction, n and t numerators and parameters, and x, y and w the coordinates of a point, as in the formulas"
)]

use alloc::vec::Vec;

use geom_bvh::{Aabb, BoxTree, BvhError, Entry};
use geom_num::grid::{P2, Steps};
use geom_num::interval::Interval;
use geom_op::NameId;
use geom_run::{Exec, WorkMeter};

use crate::carrier::Carriers;
use crate::entity::{Ends, Entity, Shape};
use crate::point::{Place, PointError, Pt2};
use crate::snap::{Drawing, SnapError, SnapKind, SnapRef, Subject};

/// The frame's limit on a coordinate in steps, as `geom-bvh`'s boxes hold.
const FRAME: i64 = (1 << 40) - 1;

/// Why the index could not be built or asked.
#[derive(Clone, Debug, PartialEq)]
pub enum IndexError {
    /// The box tree refused.
    Tree(BvhError),
    /// A point could not be read.
    Point(PointError),
    /// A step of the arithmetic failed.
    Num(geom_num::NumError),
}

impl From<BvhError> for IndexError {
    fn from(e: BvhError) -> Self {
        IndexError::Tree(e)
    }
}

impl From<PointError> for IndexError {
    fn from(e: PointError) -> Self {
        IndexError::Point(e)
    }
}

impl From<geom_num::NumError> for IndexError {
    fn from(e: geom_num::NumError) -> Self {
        IndexError::Num(e)
    }
}

impl core::fmt::Display for IndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IndexError::Tree(e) => write!(f, "{e}"),
            IndexError::Point(e) => write!(f, "{e}"),
            IndexError::Num(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for IndexError {}

/// A snap found near a cursor.
#[derive(Clone, Debug)]
pub struct Found {
    /// The kind.
    pub kind: SnapKind,
    /// The entity, by name.
    pub name: NameId,
    /// The second entity, for a crossing.
    pub with: Option<NameId>,
    /// The part (a polyline's vertex or segment) and the pick (which quadrant, which crossing).
    pub sub: Option<u32>,
    /// Which of several answers.
    pub pick: u8,
    /// The exact point.
    pub point: Pt2,
    /// The squared distance from the cursor, enclosed.
    pub distance_squared: [f64; 2],
}

/// The box tree over a drawing's snappable entities.
pub struct SnapIndex<'d> {
    drawing: &'d Drawing,
    tree: BoxTree<2>,
}

/// The floor of a double, as an `i64` clamped to the frame.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "the value is clamped to the frame, below 2^40, which an i64 and a double hold exactly, before it is cast"
)]
fn floor(v: f64) -> i64 {
    let c = if v < -(FRAME as f64) {
        -(FRAME as f64)
    } else {
        v
    };
    let c = if c > FRAME as f64 { FRAME as f64 } else { c };
    let t = c as i64;
    if (t as f64) > c {
        t.saturating_sub(1)
    } else {
        t
    }
}

/// The ceiling of a double, clamped likewise.
fn ceil(v: f64) -> i64 {
    floor(v).saturating_add(1).min(FRAME)
}

/// The box holding the point `p`: exact for a grid or rational point, the outward-rounded
/// enclosure for one stored as a definition.
fn point_box(p: &Pt2, carriers: &Carriers) -> Result<([i64; 2], [i64; 2]), IndexError> {
    match p {
        Pt2::Grid(g) => Ok(([g.x.get(), g.y.get()], [g.x.get(), g.y.get()])),
        Pt2::Rational(r) => {
            let ([x, y], w) = r.coordinates();
            let down = |v: i128| i64::try_from(v.div_euclid(w)).unwrap_or(FRAME);
            let up = |v: i128| {
                i64::try_from(
                    v.div_euclid(w)
                        .saturating_add(i128::from(v.rem_euclid(w) != 0)),
                )
                .unwrap_or(FRAME)
            };
            Ok(([down(x), down(y)], [up(x), up(y)]))
        }
        Pt2::Crossing { .. } | Pt2::ConicRoot { .. } => {
            let [x, y] = match p.place(carriers)? {
                Place::OneRoot(z) => z.enclosure(),
                Place::Conic { .. } => return Ok(([-FRAME, -FRAME], [FRAME, FRAME])),
            };
            Ok(([floor(x.lo()), floor(y.lo())], [ceil(x.hi()), ceil(y.hi())]))
        }
    }
}

/// The union of two boxes.
fn join(a: ([i64; 2], [i64; 2]), b: ([i64; 2], [i64; 2])) -> ([i64; 2], [i64; 2]) {
    (
        [a.0[0].min(b.0[0]), a.0[1].min(b.0[1])],
        [a.1[0].max(b.1[0]), a.1[1].max(b.1[1])],
    )
}

/// The square root of `n` rounded up.
fn root_up(n: u128) -> i64 {
    let mut lo = 0_u64;
    let mut hi = 1_u64 << 62;
    while lo < hi {
        let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
        if u128::from(mid).saturating_mul(u128::from(mid)) >= n {
            hi = mid;
        } else {
            lo = mid.saturating_add(1);
        }
    }
    i64::try_from(lo).unwrap_or(FRAME).min(FRAME)
}

/// A box as its low and high corners, in whole steps.
pub type Corners = ([i64; 2], [i64; 2]);

/// The box of everything a snap of `entity` can be, or `None` if it has none.
///
/// # Errors
///
/// As for reading a point.
#[allow(
    clippy::too_many_lines,
    reason = "one match on the shape: each arm is the box of that entity's snaps"
)]
pub fn bounds(entity: &Entity, carriers: &Carriers) -> Result<Option<Corners>, IndexError> {
    let whole = ([-FRAME, -FRAME], [FRAME, FRAME]);
    let around = |cx: i64, cy: i64, r: i64| {
        (
            [cx.saturating_sub(r), cy.saturating_sub(r)],
            [cx.saturating_add(r), cy.saturating_add(r)],
        )
    };
    Ok(Some(match entity.shape() {
        Shape::Point(p) => ([p.x.get(), p.y.get()], [p.x.get(), p.y.get()]),
        Shape::Line { ends, .. } => match ends {
            Ends::Segment(a, b) => join(point_box(a, carriers)?, point_box(b, carriers)?),
            Ends::Ray(_) | Ends::Infinite => whole,
        },
        Shape::Circle(c) | Shape::Arc { circle: c, .. } => {
            let (num, den, [cx, cy]) = match c {
                crate::carrier::Circle2::Grid(g) => {
                    let r = g.radius();
                    (
                        r.saturating_mul(r).unsigned_abs(),
                        1_u128,
                        [
                            i128::from(g.centre().x.get()),
                            i128::from(g.centre().y.get()),
                        ],
                    )
                }
                crate::carrier::Circle2::Round(r) => {
                    let ([x, y], w) = r.centre().coordinates();
                    let on = r.on();
                    let dx = i128::from(on.x.get()).saturating_mul(w).saturating_sub(x);
                    let dy = i128::from(on.y.get()).saturating_mul(w).saturating_sub(y);
                    let num = dx
                        .saturating_mul(dx)
                        .saturating_add(dy.saturating_mul(dy))
                        .unsigned_abs();
                    let den = w.saturating_mul(w).unsigned_abs().max(1);
                    // The centre in whole steps, rounded to the nearest: the radius is grown by the
                    // distance it can be from there.
                    let half = w / 2;
                    (
                        num,
                        den,
                        [
                            x.saturating_add(half).div_euclid(w.max(1)),
                            y.saturating_add(half).div_euclid(w.max(1)),
                        ],
                    )
                }
            };
            // The radius squared rounded up to a whole number, then its root rounded up, and one
            // step more for a centre that is not on the grid.
            let rr = num.div_ceil(den.max(1));
            let r = root_up(rr).saturating_add(1);
            let cx = i64::try_from(cx).unwrap_or(0);
            let cy = i64::try_from(cy).unwrap_or(0);
            around(cx, cy, r)
        }
        Shape::Ellipse(e) | Shape::EllipticArc { ellipse: e, .. } => {
            let [mx, my] = e.major();
            let rr = i128::from(mx)
                .saturating_mul(i128::from(mx))
                .saturating_add(i128::from(my).saturating_mul(i128::from(my)))
                .unsigned_abs();
            around(e.centre().x.get(), e.centre().y.get(), root_up(rr))
        }
        Shape::Spline(s) => {
            let mut low = [i64::MAX, i64::MAX];
            let mut high = [i64::MIN, i64::MIN];
            for c in s.control() {
                low = [low[0].min(c.x.get()), low[1].min(c.y.get())];
                high = [high[0].max(c.x.get()), high[1].max(c.y.get())];
            }
            if low[0] > high[0] {
                return Ok(None);
            }
            (low, high)
        }
        Shape::Polyline(p) => {
            let vs = p.vertices();
            let mut low = [i64::MAX, i64::MAX];
            let mut high = [i64::MIN, i64::MIN];
            for v in vs {
                low = [low[0].min(v.x.get()), low[1].min(v.y.get())];
                high = [high[0].max(v.x.get()), high[1].max(v.y.get())];
            }
            // Each bulge bows out by at most |bulge| times half the chord, and a chord is at most
            // the sum of its two differences.
            let mut grow = 0_i64;
            for (i, b) in p.bulges().iter().enumerate() {
                if b.is_zero() {
                    continue;
                }
                let a = vs.get(i);
                let c = vs.get(i.saturating_add(1)).or_else(|| vs.first());
                if let (Some(a), Some(c)) = (a, c) {
                    let chord =
                        a.x.get()
                            .saturating_sub(c.x.get())
                            .abs()
                            .saturating_add(a.y.get().saturating_sub(c.y.get()).abs());
                    let top = i128::from(b.numerator())
                        .abs()
                        .saturating_mul(i128::from(chord));
                    let bottom = i128::from(b.denominator()).saturating_mul(2).max(1);
                    let sag = top
                        .saturating_add(bottom)
                        .saturating_sub(1)
                        .checked_div(bottom)
                        .unwrap_or(0);
                    grow = grow.max(i64::try_from(sag).unwrap_or(FRAME).saturating_add(1));
                }
            }
            (
                [low[0].saturating_sub(grow), low[1].saturating_sub(grow)],
                [high[0].saturating_add(grow), high[1].saturating_add(grow)],
            )
        }
        Shape::Text(t) => {
            let a = t.anchor();
            ([a.x.get(), a.y.get()], [a.x.get(), a.y.get()])
        }
        Shape::Insert(i) => {
            let image = i.at.apply_2d(P2::new(0, 0).map_err(IndexError::Num)?);
            match image {
                Ok(r) => {
                    let v = r.value();
                    ([v.x.get(), v.y.get()], [v.x.get(), v.y.get()])
                }
                Err(_) => whole,
            }
        }
        Shape::Region(_)
        | Shape::Hatch(_)
        | Shape::Dimension(_)
        | Shape::Leader(_)
        | Shape::Block(_)
        | Shape::Viewport(_) => return Ok(None),
    }))
}

impl<'d> SnapIndex<'d> {
    /// The index of `drawing`.
    ///
    /// # Errors
    ///
    /// [`IndexError::Tree`] if the box tree refuses or the meter stops; [`IndexError::Point`] for
    /// an end that cannot be read.
    pub fn build(
        drawing: &'d Drawing,
        exec: &Exec<'_>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<SnapIndex<'d>, IndexError> {
        let carriers = Carriers::new();
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(drawing.items().len())
            .map_err(|_| BvhError::OutOfMemory)?;
        for (key, (_, entity)) in (0_u64..).zip(drawing.items()) {
            if let Some((lo, hi)) = bounds(entity, &carriers)? {
                entries.push(Entry {
                    key,
                    bbox: Aabb::new(lo, hi)?,
                });
            }
        }
        Ok(SnapIndex {
            drawing,
            tree: BoxTree::build(&entries, exec, meter)?,
        })
    }

    /// The names of the entities whose boxes meet the square of `aperture` steps about `cursor`,
    /// in name order.
    ///
    /// # Errors
    ///
    /// As the tree's query.
    pub fn candidates(
        &self,
        cursor: P2,
        aperture: Steps,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<NameId>, IndexError> {
        let r = i64::try_from(aperture.get()).unwrap_or(FRAME).min(FRAME);
        let (x, y) = (cursor.x.get(), cursor.y.get());
        let query = Aabb::new(
            [
                x.saturating_sub(r).max(-FRAME),
                y.saturating_sub(r).max(-FRAME),
            ],
            [
                x.saturating_add(r).min(FRAME),
                y.saturating_add(r).min(FRAME),
            ],
        )?;
        let keys = self.tree.overlapping(&query, meter)?;
        Ok(keys
            .into_iter()
            .filter_map(|k| usize::try_from(k).ok())
            .filter_map(|k| self.drawing.items().get(k).map(|(n, _)| *n))
            .collect())
    }

    /// The closest snap of `kinds` within `aperture` steps of `cursor`, ties to the lower name.
    ///
    /// The kinds that need no other point are made on each entity near the cursor: node, end,
    /// start, mid, centre, quadrant, insertion and centroid; intersection (and apparent
    /// intersection) is made for each pair of entities near the cursor. A kind an entity does not
    /// have is passed over.
    ///
    /// # Errors
    ///
    /// As the tree's query; a snap that cannot be made is passed over, not an error.
    pub fn snap(
        &self,
        cursor: P2,
        aperture: Steps,
        kinds: &[SnapKind],
        carriers: &mut Carriers,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Found>, IndexError> {
        let near = self.candidates(cursor, aperture, meter)?;
        self.best_among(&near, cursor, aperture, kinds, carriers)
    }

    /// [`SnapIndex::snap`] over every entity of the drawing, with no tree: the reference the tests
    /// hold the tree to, and not for use on a large drawing.
    ///
    /// # Errors
    ///
    /// As for [`SnapIndex::snap`].
    pub fn scan(
        &self,
        cursor: P2,
        aperture: Steps,
        kinds: &[SnapKind],
        carriers: &mut Carriers,
    ) -> Result<Option<Found>, IndexError> {
        let all: Vec<NameId> = self.drawing.items().iter().map(|(n, _)| *n).collect();
        self.best_among(&all, cursor, aperture, kinds, carriers)
    }

    /// The closest snap among the entities `near`, which are in name order.
    fn best_among(
        &self,
        near: &[NameId],
        cursor: P2,
        aperture: Steps,
        kinds: &[SnapKind],
        carriers: &mut Carriers,
    ) -> Result<Option<Found>, IndexError> {
        let reach = Interval::point(rounded(aperture.get()))?;
        let limit = reach.square()?;
        let mut best: Option<Found> = None;
        for (i, name) in near.iter().enumerate() {
            let Some(entity) = self.drawing.entity(*name) else {
                continue;
            };
            for &kind in kinds {
                for (sub, pick) in picks(kind, entity) {
                    let mut snap = SnapRef::new(kind, Pt2::Grid(cursor));
                    snap.sub = sub;
                    snap.pick = pick;
                    let mark = carriers.len();
                    if let Ok(point) = snap.on_subject(&Subject::Entity(entity), carriers)
                        && let Some(found) = consider(
                            Found {
                                kind,
                                name: *name,
                                with: None,
                                sub,
                                pick,
                                point,
                                distance_squared: [0.0, 0.0],
                            },
                            cursor,
                            &limit,
                            carriers,
                        )?
                    {
                        best = Some(better(best, found));
                    }
                    carriers.truncate(mark);
                }
            }
            for &kind in kinds {
                if !matches!(
                    kind,
                    SnapKind::Intersection | SnapKind::ApparentIntersection
                ) {
                    continue;
                }
                for other in near.iter().skip(i.saturating_add(1)) {
                    let Some(second) = self.drawing.entity(*other) else {
                        continue;
                    };
                    for pick in 0..2 {
                        let mut snap = SnapRef::new(kind, Pt2::Grid(cursor));
                        snap.pick = pick;
                        let mark = carriers.len();
                        if let Ok(point) = snap.cross_subjects(
                            &Subject::Entity(entity),
                            &Subject::Entity(second),
                            carriers,
                        ) && let Some(found) = consider(
                            Found {
                                kind,
                                name: *name,
                                with: Some(*other),
                                sub: None,
                                pick,
                                point,
                                distance_squared: [0.0, 0.0],
                            },
                            cursor,
                            &limit,
                            carriers,
                        )? {
                            best = Some(better(best, found));
                        }
                        carriers.truncate(mark);
                    }
                }
            }
        }
        Ok(best.map(|mut found| {
            found.point = self.remake(&found, carriers).unwrap_or(found.point);
            found
        }))
    }
}

impl SnapIndex<'_> {
    /// The point of `found` made again, so that its carriers are in `carriers` now: the search
    /// makes each candidate's carriers for a moment and drops them, and only the winner keeps its.
    fn remake(&self, found: &Found, carriers: &mut Carriers) -> Option<Pt2> {
        let entity = self.drawing.entity(found.name)?;
        let mut snap = SnapRef::new(found.kind, found.point);
        snap.sub = found.sub;
        snap.pick = found.pick;
        match found.with {
            None => snap.on_subject(&Subject::Entity(entity), carriers).ok(),
            Some(other) => {
                let second = self.drawing.entity(other)?;
                snap.cross_subjects(&Subject::Entity(entity), &Subject::Entity(second), carriers)
                    .ok()
            }
        }
    }
}

/// `n` as a double, which a count of steps below 2^53 is exactly.
#[allow(
    clippy::cast_precision_loss,
    reason = "an aperture is a few thousand steps at most; the frame's width is below 2^53"
)]
fn rounded(n: u64) -> f64 {
    n as f64
}

/// The (part, pick) pairs of an entity's own snaps of `kind`.
fn picks(kind: SnapKind, entity: &Entity) -> Vec<(Option<u32>, u8)> {
    let mut out = Vec::new();
    match (kind, entity.shape()) {
        (SnapKind::Quadrant, _) => {
            for k in 0..4_u8 {
                out.push((None, k));
            }
        }
        (SnapKind::End | SnapKind::Start, Shape::Polyline(p)) => {
            for i in 0..p.vertices().len() {
                if let Ok(i) = u32::try_from(i) {
                    out.push((Some(i), 0));
                }
            }
        }
        (SnapKind::Mid, Shape::Polyline(p)) => {
            for i in 0..p.bulges().len() {
                if let Ok(i) = u32::try_from(i) {
                    out.push((Some(i), 0));
                }
            }
        }
        (
            SnapKind::Node
            | SnapKind::End
            | SnapKind::Start
            | SnapKind::Mid
            | SnapKind::Center
            | SnapKind::Insertion
            | SnapKind::Centroid,
            _,
        ) => out.push((None, 0)),
        _ => {}
    }
    out
}

/// The candidate with its distance enclosed, or `None` if it is certainly outside the aperture.
fn consider(
    mut found: Found,
    cursor: P2,
    limit: &Interval,
    carriers: &Carriers,
) -> Result<Option<Found>, IndexError> {
    let [x, y] = match found.point.place(carriers)? {
        Place::OneRoot(z) => z.enclosure(),
        Place::Conic { .. } => return Ok(None),
    };
    #[allow(
        clippy::cast_precision_loss,
        reason = "a grid coordinate is below 2^40, which a double holds exactly"
    )]
    let (cx, cy) = (cursor.x.get() as f64, cursor.y.get() as f64);
    let dx = x.sub(Interval::point(cx)?)?;
    let dy = y.sub(Interval::point(cy)?)?;
    let d = dx.square()?.add(dy.square()?)?;
    if d.lo() > limit.hi() {
        return Ok(None);
    }
    found.distance_squared = [d.lo(), d.hi()];
    Ok(Some(found))
}

/// The better of the best so far and `found`: closer if its whole interval is below, and otherwise
/// the lower name.
fn better(best: Option<Found>, found: Found) -> Found {
    match best {
        None => found,
        Some(held) => {
            let closer = found.distance_squared[1] < held.distance_squared[0];
            let farther = held.distance_squared[1] < found.distance_squared[0];
            let tied = !closer && !farther;
            if closer || (tied && found.name < held.name) {
                found
            } else {
                held
            }
        }
    }
}

/// A snap reference for what was found, for `on` and `with` given as the references the caller
/// holds for the two names.
#[must_use]
pub fn snap_of(
    found: &Found,
    on: crate::snap::Reference,
    with: Option<crate::snap::Reference>,
) -> SnapRef {
    let mut snap = SnapRef::new(found.kind, found.point);
    snap.on = Some(on);
    snap.with = with;
    snap.sub = found.sub;
    snap.pick = found.pick;
    snap
}

/// What an error of making a snap is, for callers that pass candidates over.
pub type Passed = SnapError;
