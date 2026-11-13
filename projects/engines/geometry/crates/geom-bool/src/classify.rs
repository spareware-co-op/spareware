//! Classification: one proven winding number for each group of split faces against each other
//! operand, by rays that qualify (11.4 step 6, G246).
//!
//! What it does: [`groups`] joins the split faces of [`crate::split`] that meet across a piece of
//! their own operand's boundary which touches no other operand; the faces of a group lie in one
//! cell of every other operand, so a group needs one answer. [`classify`] finds it. For a group
//! and an operand it takes grid points from a fixed sequence on the group's faces, keeps the first
//! one proven inside its split face ([`crate::split::within`]) whose surroundings are proven clear
//! of every face of the operand ([`Boundary::clear`]), and counts the operand's signed crossings
//! along a ray from it ([`Boundary::winding`]). The rays come from [`RAYS`], in order. A ray is
//! used only when it qualifies: it is proven further than the operand's bound from every edge of
//! the operand, every crossing of a flat face is decided off that face's boundary, and every
//! crossing of a curved face is a certified simple crossing whose normal is proven not square to
//! the ray. The first ray that qualifies decides, and the count is the winding number.
//!
//! Guarantees:
//!
//! - **The number is exact.** A [`Winding`] is an integer proven for the point it was counted
//!   from, never a rounded sum: every crossing counted is proven to exist, to lie ahead of the
//!   point and inside its face, and to pass through the face, and every face not counted is proven
//!   missed. The box round the point (the point, and the point of the group's face it stands for)
//!   is proven clear of the operand's faces, so the number is the group's.
//! - **Flat faces are crossed exactly.** Whether a ray meets a face's plane, and on which side of
//!   the point, is the sign of whole numbers ([`geom_num::plane::Lpi`]); the meeting is committed
//!   to the grid with whether it was exact, and placed in the face by [`FlatFace::place`], whose
//!   On refuses the ray.
//! - **No ray slips through a seam.** A ray proven further than the operand's bound from every
//!   edge cannot pass between two faces that meet only to within that bound.
//! - **Never a guess.** A ray with any undecided part is dropped whole and the next is tried. When
//!   none of [`RAYS`] qualifies the answer is [`BuildError::NoRay`], and when no point of a group
//!   is proven clear it is [`BuildError::NoClearBox`], each with a [`NearContact`] as evidence.
//! - **What this piece does not classify, it says.** A crossing of a curved carrier whose face
//!   has loops cannot be put in or out of that face until curved faces have their own picture
//!   (G244's analytic carriers, G245): a ray that meets such a carrier does not qualify, and a
//!   ray proven to miss it does. A group that lies in a face of another operand has no clear
//!   point; coincident faces are G248's.
//! - **Deterministic.** Groups in the order of their lowest face, operands in number order,
//!   points and rays in their fixed order, faces and edges in index order. Nothing is drawn at
//!   random.
//! - **Bounded.** At most [`GROUP_POINTS`] points for a group and an operand, the rays of
//!   [`RAYS`] for a point, and [`EDGE_PIECES`] boxes of an edge for a ray. No recursion: an edge
//!   is halved from a list that holds at most that many spans.
//!
//! Method: chapter 11.4, "Classification" (`projects/engines/geometry/architecture/11-booleans.md`):
//! a point in a certified clear box, a fixed sequence of rays, a ray kept only when it stays
//! further than the operand's bound from every edge and meets no face tangentially, crossings
//! exact on flat faces and certified on curved ones (chapter 12's curve with surface). The
//! crossing count is the ray test for a point in a solid (J. O'Rourke, *Computational Geometry in
//! C*, 2nd edition, Cambridge University Press (1998), section 7.5,
//! <https://doi.org/10.1017/CBO9780511804120>). **The picture lemma**, used wherever a point known
//! within ρ steps is placed in a flat face: if the point is within ρ of the face's plane and
//! [`FlatFace::place`] with a bound of 3ρ answers In or Out, every point within ρ of it has the
//! same answer. For if one had the other, the pictures of the stretch between them would cross
//! the boundary's picture; the point of the stretch there is within ρ of the plane, so within
//! √3·ρ of the boundary point with the same picture (the picture drops the coordinate the normal
//! is largest in), which puts the boundary within (1 + √3)·ρ < 3ρ of the point placed, and
//! `place` would have answered On.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/bb836b8183c6810b9f1c17f68059f4b3e1c1f94f/crates/solidcore/src/topology/shell.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//!   Changed for Spare Kit: `is_inner_shell_of` and `ray_fire` (`SmShell::IsInnerShellOf`,
//!   `SmShell::RayFire`) turned into [`Boundary::winding`], [`Boundary::clear`] and [`classify`]
//!   in safe `no_std` Rust. Kept: the order of the cases. Rays are tried one after another from
//!   a point of the shell being placed; a ray is used only when all it meets are single points
//!   inside faces, and any range, edge or vertex it meets sends the loop to the next ray; a ray
//!   that meets nothing puts the point outside; otherwise the crossings are read by the side of
//!   the face's normal the ray runs to; and no ray deciding is an answer of its own (`SM_UNSURE`,
//!   here a typed error). Changed: the ray directions are a fixed table, where the origin draws
//!   them from a random generator, seeded from the clock upstream; the start is a grid point in a
//!   box proven clear of the other operand, where the origin samples three parameter points of
//!   each face; whether a hit is a face's inside, its edge or behind the start is proven
//!   (whole-number signs, the contact rule, certified enclosures) where the origin compares
//!   doubles with `EFF_ZERO` and a zone tolerance; a normal whose side of the ray is not proven
//!   refuses the ray, where the origin reads the sign of a dot product in doubles; the stack of
//!   shells entered and left is a signed sum, since an operand here is one boundary; a failed
//!   step is never ignored, where upstream ignores the status of the box, the samples, the ray
//!   solve and the normal; the nearest-hit choice of `ray_fire`, which measures every candidate
//!   against the first one's parameter, is not taken, since every crossing is counted. Its voxel
//!   grid (`intersect/ray_tracer.rs`) is not taken either: candidates are found by each edge's
//!   hull and each face's box.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: [`Boundary::new`] converts each edge of the operand to its hull once. A ray costs an
//! interval test per edge, a few box tests of the edges it passes near, a few whole-number
//! products per flat face with one [`FlatFace::place`] where it meets the face's plane ahead near
//! the face, and one certified intersection per curved face; a clear test costs a distance per
//! face. All charged to the meter, a unit per ray and per point besides what the engines charge.
//! No recursion.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use geom_intersect::curve_surface::curve_surface;
use geom_intersect::{Coverage, IntersectError, Item, PointContact};
use geom_num::interval::Interval;
use geom_num::plane::{Lpi, Plane};
use geom_num::{NumError, P3, Sign, Steps};
use geom_run::WorkMeter;
use geom_topo::{CurveIx, CurveKind, Line, ParamCurve, ParamSurface, SurfaceIx};

use crate::carrier::{Bound, CarrierIx};
use crate::crossing::{Enclosed, Hull};
use crate::distance::{Carriers, enclose_i128};
use crate::inside::{FlatFace, Place};
use crate::interfere::{
    Arrangement, BuildError, Entity, Interfered, committed, coordinates, nearest_point, widened,
};
use crate::rule::{Contact, ContactError, EPS_FIT, NearContact, REFINEMENTS, decide};
use crate::split::{Piece, SplitFace, Splitting, within};

/// The ray directions tried, in order: whole and primitive, no two along one line.
pub const RAYS: [[i32; 3]; 25] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 1, 0],
    [1, 0, 1],
    [0, 1, 1],
    [1, -1, 0],
    [1, 0, -1],
    [0, 1, -1],
    [1, 1, 1],
    [1, 1, -1],
    [1, -1, 1],
    [1, -1, -1],
    [2, 1, 0],
    [1, 2, 0],
    [2, 0, 1],
    [1, 0, 2],
    [0, 2, 1],
    [0, 1, 2],
    [2, 1, 1],
    [1, 2, 1],
    [1, 1, 2],
    [2, -1, 1],
    [1, 2, -1],
    [1, -1, 2],
];

/// The most boxes of one edge tested against one ray.
pub const EDGE_PIECES: u32 = 256;

/// The most points tried for one group against one operand.
pub const GROUP_POINTS: usize = 32;

/// The picture lemma's factor: a bound of this many times ρ in [`FlatFace::place`] decides for
/// every point within ρ (module documentation).
const PICTURE: u64 = 3;

/// A ray's length in its own parameter: past the frame from any grid point, 2^41 steps a unit.
const FAR: f64 = 2_199_023_255_552.0;

/// An operand's winding number at a point: its signed crossings along the ray that decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Winding {
    /// The number: crossings leaving the operand less crossings entering it, so 1 inside a solid
    /// whose faces face outward, 0 outside it. Exact: a proven integer.
    pub number: i32,
    /// How many crossings the ray made.
    pub crossings: u32,
    /// The ray that decided, by its index in [`RAYS`].
    pub ray: usize,
}

/// An edge of the operand, ready for the rays.
#[derive(Clone, Debug)]
struct Rim {
    /// The edge, by its index in the arrangement.
    edge: usize,
    /// Its curve.
    curve: CurveIx,
    /// Its hull.
    hull: Hull,
}

/// A face of the operand on a curved carrier.
#[derive(Clone, Copy, Debug)]
struct Round {
    /// The face, by its index in the arrangement.
    face: usize,
    /// Its carrier.
    surface: SurfaceIx,
    /// Whether loops bound it: then only part of its carrier is the face.
    trimmed: bool,
    /// Whether its outside is the side its carrier's normal points to.
    outward: bool,
}

/// One operand's boundary, ready to be classified against: its faces and edges.
#[derive(Clone, Debug)]
pub struct Boundary {
    /// The operand.
    operand: u32,
    /// The bound its faces meet within.
    bound: Bound,
    /// Its flat faces, each with whether its outside is the side its plane's normal points to.
    flats: Vec<(FlatFace, bool)>,
    /// Its faces on curved carriers.
    rounds: Vec<Round>,
    /// Its edges.
    rims: Vec<Rim>,
}

/// A ray from a grid point along a whole direction.
struct Ray {
    /// Its start.
    from: P3,
    /// The grid point one direction further.
    to: P3,
    /// Its direction.
    direction: [i32; 3],
    /// Two planes through its line, square to each other.
    planes: [Enclosed; 2],
    /// The least its direction's length can be.
    length: f64,
}

/// What a ray found at a face.
enum Met {
    /// The ray does not qualify.
    Refused,
    /// The face's signed crossings and their count.
    Crossed(i32, u32),
}

/// A count of steps as a double.
fn steps_f(bound: Bound) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "bounds are counts of steps far below 2^53"
    )]
    let steps = bound.steps() as f64;
    steps
}

/// The least magnitude in an enclosure.
fn least(x: Interval) -> f64 {
    if x.lo() > 0.0 {
        x.lo()
    } else if x.hi() < 0.0 {
        -x.hi()
    } else {
        0.0
    }
}

/// The enclosure of a box given by its corners.
fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [lx, ly, lz] = lo;
    let [hx, hy, hz] = hi;
    Ok([
        Interval::new(lx, hx)?,
        Interval::new(ly, hy)?,
        Interval::new(lz, hz)?,
    ])
}

/// n · p + d for a grid point, exactly.
fn plane_value(plane: &Plane, p: P3) -> Result<i128, NumError> {
    let [n0, n1, n2] = plane.n();
    let [x, y, z] = [p.x.get(), p.y.get(), p.z.get()].map(i128::from);
    n0.checked_mul(x)
        .and_then(|s| s.checked_add(n1.checked_mul(y)?))
        .and_then(|s| s.checked_add(n2.checked_mul(z)?))
        .and_then(|s| s.checked_add(plane.d()))
        .ok_or(NumError::BudgetExceeded)
}

/// a × b in whole numbers.
fn cross(a: [i128; 3], b: [i128; 3]) -> Option<[i128; 3]> {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    Some([
        a1.checked_mul(b2)?.checked_sub(a2.checked_mul(b1)?)?,
        a2.checked_mul(b0)?.checked_sub(a0.checked_mul(b2)?)?,
        a0.checked_mul(b1)?.checked_sub(a1.checked_mul(b0)?)?,
    ])
}

/// The evidence of a refusal: something of the operand lies within `reach` steps, or meets
/// tangentially, wherever this piece looked.
fn evidence(reach: f64) -> Result<NearContact, NumError> {
    Ok(NearContact {
        enclosure: Interval::new(0.0, reach)?,
        refinement: REFINEMENTS,
    })
}

impl Ray {
    /// The ray from `from` along `direction`, or `None` when the grid point one direction further
    /// lies outside the frame.
    fn new(from: P3, direction: [i32; 3]) -> Result<Option<Ray>, NumError> {
        let [dx, dy, dz] = direction;
        let step = |at: i64, by: i32| at.checked_add(i64::from(by));
        let (Some(x), Some(y), Some(z)) = (
            step(from.x.get(), dx),
            step(from.y.get(), dy),
            step(from.z.get(), dz),
        ) else {
            return Ok(None);
        };
        let Ok(to) = P3::new(x, y, z) else {
            return Ok(None);
        };
        let d = direction.map(i128::from);
        // The axis the direction leans along least, the first of equals: never along it.
        let mut axis = 0_usize;
        let mut least_lean = u128::MAX;
        for (k, part) in d.iter().enumerate() {
            let lean: u128 = part.unsigned_abs();
            if lean < least_lean {
                least_lean = lean;
                axis = k;
            }
        }
        let mut unit = [0_i128; 3];
        if let Some(slot) = unit.get_mut(axis) {
            *slot = 1;
        }
        // Two normals square to the direction and to each other: d × e, and d × (d × e).
        let first = cross(d, unit).ok_or(NumError::BudgetExceeded)?;
        let second = cross(d, first).ok_or(NumError::BudgetExceeded)?;
        let at = [from.x.get(), from.y.get(), from.z.get()].map(i128::from);
        let through = |n: [i128; 3]| -> Result<Enclosed, NumError> {
            let [n0, n1, n2] = n;
            let [x, y, z] = at;
            let offset = n0
                .checked_mul(x)
                .and_then(|s| s.checked_add(n1.checked_mul(y)?))
                .and_then(|s| s.checked_add(n2.checked_mul(z)?))
                .and_then(i128::checked_neg)
                .ok_or(NumError::BudgetExceeded)?;
            Enclosed::of(&Plane::new(n, offset)?)
        };
        let [fx, fy, fz] = direction.map(f64::from);
        let length = Interval::norm(&[
            Interval::point(fx)?,
            Interval::point(fy)?,
            Interval::point(fz)?,
        ])?;
        Ok(Some(Ray {
            from,
            to,
            direction,
            planes: [through(first)?, through(second)?],
            length: length.lo(),
        }))
    }

    /// d · (x − p) over the box `x`: the position along the ray, times the direction's length
    /// squared.
    fn along(&self, x: &[Interval; 3]) -> Result<Interval, NumError> {
        let at = coordinates(self.from);
        let mut sum = Interval::point(0.0)?;
        for ((span, origin), by) in x.iter().zip(at).zip(self.direction) {
            let part = Interval::point(f64::from(by))?.mul(span.sub(Interval::point(origin)?)?)?;
            sum = sum.add(part)?;
        }
        Ok(sum)
    }

    /// Whether every point of the box `x` is proven further than `reach` from the ray: off one of
    /// the two planes through its line by more than that, or behind its start by more than that.
    fn clears(&self, x: &[Interval; 3], reach: f64) -> Result<bool, NumError> {
        for plane in &self.planes {
            if least(plane.value(x)?.div(plane.norm)?) > reach {
                return Ok(true);
            }
        }
        // Behind the start by more than the reach, measured along the ray.
        Ok(self.along(x)?.hi() < -(reach * self.length) && self.length > 0.0)
    }
}

impl Boundary {
    /// The boundary of `operand` in `arr`: its faces and edges. `outward` says, for each face of
    /// the arrangement, whether the face's outside is the side its carrier's normal points to (a
    /// plane's normal as the table holds it, a surface's `S_u` × `S_v`), and `bound` is the
    /// distance its faces meet within, zero for an exact body.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face without its `outward` entry or a carrier or curve that
    /// is not there; a curve's conversion's errors.
    pub fn new(
        arr: &Arrangement,
        operand: u32,
        outward: &[bool],
        bound: Bound,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Boundary, BuildError> {
        let mut made = Boundary {
            operand,
            bound,
            flats: Vec::new(),
            rounds: Vec::new(),
            rims: Vec::new(),
        };
        for (f, face) in arr.faces.iter().enumerate() {
            if face.operand != operand {
                continue;
            }
            let missing = BuildError::Missing(Entity::Face(f));
            let out = *outward.get(f).ok_or(missing)?;
            match (FlatFace::new(arr, f, meter)?, face.surface) {
                (Some(flat), _) => made.flats.push((flat, out)),
                (None, CarrierIx::Surface(surface)) => made.rounds.push(Round {
                    face: f,
                    surface,
                    trimmed: !face.loops.is_empty(),
                    outward: out,
                }),
                (None, _) => return Err(missing),
            }
        }
        for (e, edge) in arr.edges.iter().enumerate() {
            if edge.operand != operand {
                continue;
            }
            let curve = arr
                .carriers
                .curve(edge.curve)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            made.rims.push(Rim {
                edge: e,
                curve: edge.curve,
                hull: Hull::of(curve, meter)?,
            });
        }
        Ok(made)
    }

    /// The operand.
    #[must_use]
    pub const fn operand(&self) -> u32 {
        self.operand
    }

    /// Whether every point within `reach` of the grid point `point` is proven off every face of
    /// the operand, by more than the operand's bound: the box round the point is clear, and the
    /// winding number is one integer all over it. `false` is not a proof of contact: it says only
    /// that this point is not proven clear.
    ///
    /// # Errors
    ///
    /// A stopped meter, or a carrier that is not there.
    pub fn clear(
        &self,
        carriers: &Carriers,
        point: P3,
        reach: Bound,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, BuildError> {
        meter.charge(1).map_err(IntersectError::from)?;
        // ρ: how far a point of the box, moved by the operand's bound, can be from `point`.
        let rho = reach.steps().saturating_add(self.bound.steps()).max(1);
        let rho_bound = Bound(Steps(rho));
        for (flat, _) in &self.flats {
            let plane = Enclosed::of(flat.plane())?;
            let gap = enclose_i128(plane_value(flat.plane(), point)?)?.div(plane.norm)?;
            if least(gap) > steps_f(rho_bound) {
                continue;
            }
            // Within ρ of the plane: by the picture lemma, Out with a bound of 3ρ puts every
            // point of the box out of the face.
            let wide = Bound(Steps(rho.saturating_mul(PICTURE)));
            match flat.place(carriers, point, wide, meter) {
                Ok(Place::Out) => {}
                Ok(Place::In | Place::On(_))
                | Err(
                    BuildError::Unclassified { .. }
                    | BuildError::Contact(ContactError::NearContact(_)),
                ) => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        for round in &self.rounds {
            // The whole carrier, which holds the face.
            let carrier = CarrierIx::Surface(round.surface);
            let contact = decide(|level: u32| {
                let distance = carriers.distance(point, carrier, level, meter)?;
                Ok(widened(distance, rho_bound)?)
            });
            match contact {
                Ok(Contact::Apart(_)) => {}
                Ok(Contact::Touch(_)) | Err(ContactError::NearContact(_)) => return Ok(false),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(true)
    }

    /// The operand's winding number at the grid point `point`, by the first ray of [`RAYS`] that
    /// qualifies (module documentation). The point should be clear of the operand's faces
    /// ([`Boundary::clear`]); one on a face has no ray that qualifies.
    ///
    /// # Errors
    ///
    /// [`BuildError::NoRay`] when no ray qualifies; a stopped meter; an intersection's stop.
    pub fn winding(
        &self,
        carriers: &Carriers,
        point: P3,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Winding, BuildError> {
        for (index, direction) in RAYS.iter().enumerate() {
            meter.charge(1).map_err(IntersectError::from)?;
            let Some(ray) = Ray::new(point, *direction)? else {
                continue;
            };
            if let Met::Crossed(number, crossings) = self.cast(carriers, &ray, meter)? {
                return Ok(Winding {
                    number,
                    crossings,
                    ray: index,
                });
            }
        }
        let reach = steps_f(self.bound) + steps_f(Bound(EPS_FIT)) + steps_f(Bound(Steps(PICTURE)));
        Err(BuildError::NoRay {
            operand: self.operand,
            at: point,
            near: evidence(reach)?,
        })
    }

    /// One ray against the whole boundary.
    fn cast(
        &self,
        carriers: &Carriers,
        ray: &Ray,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Met, BuildError> {
        // Every edge first: a ray that passes within the bound of one could slip through a seam.
        for rim in &self.rims {
            if !self.past(carriers, ray, rim, meter)? {
                return Ok(Met::Refused);
            }
        }
        let (mut number, mut crossings) = (0_i32, 0_u32);
        let mut add = |by: i32, count: u32| -> Result<(), NumError> {
            number = number.checked_add(by).ok_or(NumError::BudgetExceeded)?;
            crossings = crossings
                .checked_add(count)
                .ok_or(NumError::BudgetExceeded)?;
            Ok(())
        };
        for (flat, outward) in &self.flats {
            match self.through_flat(carriers, ray, flat, *outward, meter)? {
                Met::Refused => return Ok(Met::Refused),
                Met::Crossed(by, count) => add(by, count)?,
            }
        }
        for round in &self.rounds {
            match Self::through_round(carriers, ray, round, meter)? {
                Met::Refused => return Ok(Met::Refused),
                Met::Crossed(by, count) => add(by, count)?,
            }
        }
        Ok(Met::Crossed(number, crossings))
    }

    /// Whether the ray is proven further than the operand's bound from the edge `rim`: by its
    /// hull, or by the boxes of its halves, at most [`EDGE_PIECES`] of them.
    fn past(
        &self,
        carriers: &Carriers,
        ray: &Ray,
        rim: &Rim,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, BuildError> {
        let reach = steps_f(self.bound);
        for plane in &ray.planes {
            if least(rim.hull.spread(plane)?) > reach {
                return Ok(true);
            }
        }
        let curve = carriers
            .curve(rim.curve)
            .ok_or(BuildError::Missing(Entity::Edge(rim.edge)))?;
        let (start, end) = curve.domain();
        if !(start.is_finite() && end.is_finite()) {
            return Ok(false);
        }
        let mut spans = alloc::vec![(start, end)];
        let mut tested = 0_u32;
        // Each turn tests one span and adds at most two, so the list never passes the cap.
        while let Some((from, to)) = spans.pop() {
            tested = tested.saturating_add(1);
            if tested > EDGE_PIECES {
                return Ok(false);
            }
            let at = curve
                .bound((from, to), meter)
                .map_err(IntersectError::from)?;
            if ray.clears(&boxed(at.lo, at.hi)?, reach)? {
                continue;
            }
            let middle = from / 2.0 + to / 2.0;
            if !(from < middle && middle < to) {
                return Ok(false);
            }
            spans.push((middle, to));
            spans.push((from, middle));
        }
        Ok(true)
    }

    /// The ray against a flat face: where it meets the face's plane, exactly, and where that lies
    /// in the face.
    fn through_flat(
        &self,
        carriers: &Carriers,
        ray: &Ray,
        flat: &FlatFace,
        outward: bool,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Met, BuildError> {
        let plane = flat.plane();
        let (here, next) = (plane_value(plane, ray.from)?, plane_value(plane, ray.to)?);
        let weight = next.checked_sub(here).ok_or(NumError::BudgetExceeded)?;
        // Along the plane: beside it the ray never meets it, and in it the ray would have to
        // cross the face's boundary to meet the face, which it is proven clear of.
        if weight == 0 {
            return Ok(Met::Crossed(0, 0));
        }
        // The meeting is at −here / weight along the ray: behind the start when they agree in
        // sign.
        if here != 0 && (here > 0) == (weight > 0) {
            return Ok(Met::Crossed(0, 0));
        }
        let hit = match Lpi::new(ray.from, ray.to, *plane).and_then(|meeting| meeting.to_grid()) {
            Ok(hit) => hit,
            // Outside the frame, where no face is.
            Err(NumError::OutOfRange { .. }) => return Ok(Met::Crossed(0, 0)),
            Err(e) => return Err(e.into()),
        };
        // The meeting is within a step of its grid point when rounded, and the face's boundary
        // within the operand's bound of its edges: the picture lemma's ρ.
        let rho = self
            .bound
            .steps()
            .saturating_add(u64::from(!hit.is_exact()));
        let wide = Bound(Steps(rho.saturating_mul(PICTURE)));
        match flat.place(carriers, hit.value(), wide, meter) {
            Ok(Place::Out) => Ok(Met::Crossed(0, 0)),
            // A start on the face itself has no winding number.
            Ok(Place::In) if here == 0 => Ok(Met::Refused),
            Ok(Place::In) => Ok(Met::Crossed(
                if (weight > 0) == outward { 1 } else { -1 },
                1,
            )),
            Ok(Place::On(_))
            | Err(
                BuildError::Unclassified { .. } | BuildError::Contact(ContactError::NearContact(_)),
            ) => Ok(Met::Refused),
            Err(e) => Err(e),
        }
    }

    /// The ray against a face on a curved carrier: the certified meetings of its line with the
    /// carrier, each a simple crossing ahead of the start whose normal is proven to one side.
    fn through_round(
        carriers: &Carriers,
        ray: &Ray,
        round: &Round,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Met, BuildError> {
        let surface = carriers
            .surface(round.surface)
            .ok_or(BuildError::Missing(Entity::Face(round.face)))?;
        let direction = ray.direction.map(f64::from);
        let line = Line::new(coordinates(ray.from), direction, (0.0, FAR))
            .map_err(IntersectError::from)?;
        let met = curve_surface(&CurveKind::Line(line), surface, meter)?;
        if met.items().is_empty() {
            return Ok(match met.coverage() {
                Coverage::TopologyCertified => Met::Crossed(0, 0),
                Coverage::CompleteOutsideFlagged => Met::Refused,
            });
        }
        // Which meetings lie in a face with loops waits for the curved faces' own picture.
        if round.trimmed {
            return Ok(Met::Refused);
        }
        let (mut number, mut crossings) = (0_i32, 0_u32);
        for item in met.items() {
            let Item::Point {
                at,
                contact: PointContact::Crossing,
                ..
            } = item
            else {
                return Ok(Met::Refused);
            };
            let [t, u, v] = at.spans() else {
                return Err(IntersectError::Internal.into());
            };
            // Not proven ahead of the start.
            if t.lo() <= 0.0 {
                return Ok(Met::Refused);
            }
            let normal = surface
                .normal_bound(((u.lo(), u.hi()), (v.lo(), v.hi())), meter)
                .map_err(IntersectError::from)?;
            let mut facing = Interval::point(0.0)?;
            for (span, by) in boxed(normal.lo, normal.hi)?.iter().zip(direction) {
                facing = facing.add(span.mul(Interval::point(by)?)?)?;
            }
            let leaving = match facing.certain_sign() {
                Some(Sign::Positive) => true,
                Some(Sign::Negative) => false,
                // Square to the normal, as far as is proven: tangent.
                Some(Sign::Zero) | None => return Ok(Met::Refused),
            };
            number = number
                .checked_add(if leaving == round.outward { 1 } else { -1 })
                .ok_or(NumError::BudgetExceeded)?;
            crossings = crossings.checked_add(1).ok_or(NumError::BudgetExceeded)?;
        }
        Ok(Met::Crossed(number, crossings))
    }
}

/// The split edges that touch another operand: members of a common block with an edge of another
/// operand, and those lying inside a face of another operand.
fn foreign(arr: &Arrangement, out: &Interfered) -> BTreeSet<usize> {
    let owner = |split: usize| -> Option<u32> {
        let source = out.splits.get(split)?.source;
        arr.edges.get(source).map(|edge| edge.operand)
    };
    let mut touched = BTreeSet::new();
    for block in &out.common {
        let mut owners = block.members.iter().map(|&m| owner(m));
        let first = owners.next().flatten();
        if owners.any(|other| other != first) {
            touched.extend(block.members.iter().copied());
        }
    }
    for info in &out.faces {
        touched.extend(info.in_blocks.iter().copied());
    }
    touched
}

/// The groups of split faces: faces of one operand that meet across a split edge of that operand
/// which touches no other operand. Each group lists its faces by index into
/// [`Splitting::faces`], in order, and the groups come in the order of their lowest face.
#[must_use]
pub fn groups(arr: &Arrangement, out: &Interfered, splitting: &Splitting) -> Vec<Vec<usize>> {
    let touched = foreign(arr, out);
    let operand_of = |one: &SplitFace| arr.faces.get(one.face).map(|face| face.operand);
    let mut lead: Vec<usize> = (0..splitting.faces.len()).collect();
    let find = |lead: &[usize], start: usize| -> usize {
        let mut i = start;
        // Each step moves to a lower index, so the walk ends within the faces' count.
        for _ in 0..lead.len() {
            let up = lead.get(i).copied().unwrap_or(i);
            if up == i {
                break;
            }
            i = up;
        }
        i
    };
    let mut first_at: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, one) in splitting.faces.iter().enumerate() {
        let uses = one.outer.iter().chain(one.holes.iter().flatten());
        for used in uses {
            let Piece::Split(split) = used.piece else {
                continue;
            };
            let own = out
                .splits
                .get(split)
                .and_then(|s| arr.edges.get(s.source))
                .map(|edge| edge.operand);
            if touched.contains(&split) || own.is_none() || own != operand_of(one) {
                continue;
            }
            match first_at.get(&split) {
                Some(&other) => {
                    let (a, b) = (find(&lead, i), find(&lead, other));
                    let (low, high) = if a < b { (a, b) } else { (b, a) };
                    if let Some(slot) = lead.get_mut(high) {
                        *slot = low;
                    }
                }
                None => {
                    first_at.insert(split, i);
                }
            }
        }
    }
    let mut by_lead: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..splitting.faces.len() {
        by_lead.entry(find(&lead, i)).or_default().push(i);
    }
    by_lead.into_values().collect()
}

/// A group's classification against one other operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Against {
    /// The other operand.
    pub operand: u32,
    /// Its winding number all over the group.
    pub winding: Winding,
    /// The grid point it was counted from.
    pub at: P3,
    /// The split face the point stands for, by index into [`Splitting::faces`].
    pub face: usize,
}

/// A group of split faces and its classification against every other operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The operand its faces belong to.
    pub operand: u32,
    /// Its split faces, by index into [`Splitting::faces`], in order.
    pub faces: Vec<usize>,
    /// Its classification against each other operand, in the operands' order.
    pub against: Vec<Against>,
}

/// The corners of a split face's outer loop, in doubles: the ends of its pieces, and for a closed
/// curve with no pave three points of it.
fn corners(
    arr: &Arrangement,
    out: &Interfered,
    one: &SplitFace,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<[f64; 3]>, BuildError> {
    let mut found: Vec<[f64; 3]> = Vec::new();
    let mut push = |at: [f64; 3]| {
        if found.last() != Some(&at) && found.first() != Some(&at) {
            found.push(at);
        }
    };
    let vertex = |v: usize| -> Result<[f64; 3], BuildError> {
        Ok(coordinates(committed(&arr.carriers, &out.vertices, v)?.0))
    };
    for used in &one.outer {
        let ends = match used.piece {
            Piece::Split(i) => {
                let split = out
                    .splits
                    .get(i)
                    .ok_or(BuildError::Missing(Entity::Edge(i)))?;
                if let (Some(from), Some(to)) = (split.block.from, split.block.to) {
                    Some([from.vertex, to.vertex])
                } else {
                    let missing = BuildError::Missing(Entity::Edge(split.block.edge));
                    let curve = arr
                        .edges
                        .get(split.block.edge)
                        .and_then(|edge| arr.carriers.curve(edge.curve))
                        .ok_or(missing)?;
                    let (start, end) = curve.domain();
                    for third in [0.0, 1.0, 2.0] {
                        let t = start + (end - start) * third / 3.0;
                        let at = curve.bound((t, t), meter).map_err(IntersectError::from)?;
                        let [lx, ly, lz] = at.lo;
                        let [hx, hy, hz] = at.hi;
                        push([
                            lx / 2.0 + hx / 2.0,
                            ly / 2.0 + hy / 2.0,
                            lz / 2.0 + hz / 2.0,
                        ]);
                    }
                    None
                }
            }
            Piece::Section(j) => {
                let section = out
                    .sections
                    .get(j)
                    .ok_or(BuildError::Missing(Entity::Face(one.face)))?;
                Some([section.from.vertex, section.to.vertex])
            }
        };
        if let Some([a, b]) = ends {
            let (first, second) = if used.forward { (a, b) } else { (b, a) };
            push(vertex(first)?);
            push(vertex(second)?);
        }
    }
    Ok(found)
}

/// The mean of some points.
fn mean(points: &[[f64; 3]]) -> [f64; 3] {
    let mut sum = [0.0_f64; 3];
    let mut count = 0.0_f64;
    for point in points {
        for (slot, x) in sum.iter_mut().zip(point) {
            *slot += x;
        }
        count += 1.0;
    }
    if count > 0.0 {
        sum.map(|s| s / count)
    } else {
        sum
    }
}

/// The grid points tried on a split face, in order: the mean of its corners, the mean of each
/// three corners in a row, then the points halfway from the first mean to each corner.
fn candidates(
    arr: &Arrangement,
    out: &Interfered,
    one: &SplitFace,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<P3>, BuildError> {
    let found = corners(arr, out, one, meter)?;
    let centre = mean(&found);
    let mut tried: Vec<[f64; 3]> = alloc::vec![centre];
    if found.len() > 3 {
        for i in 0..found.len() {
            let three: Vec<[f64; 3]> = found.iter().cycle().skip(i).take(3).copied().collect();
            tried.push(mean(&three));
        }
    }
    for corner in &found {
        tried.push(mean(&[centre, *corner]));
    }
    let mut points = Vec::new();
    for at in tried {
        // A point outside the frame is no candidate.
        if let Ok(point) = nearest_point(at)
            && !points.contains(&point)
        {
            points.push(point);
        }
    }
    Ok(points)
}

/// How far the grid point `point` is from the point of the face's plane with the same picture:
/// its offset along the coordinate the plane's normal is largest in, in whole steps, rounded up.
fn lift(plane: &Plane, point: P3) -> Result<Bound, NumError> {
    let value = plane_value(plane, point)?.unsigned_abs();
    let largest = plane
        .n()
        .iter()
        .map(|n| n.unsigned_abs())
        .max()
        .unwrap_or(0);
    if largest == 0 {
        return Err(NumError::Degenerate);
    }
    let steps = value.div_ceil(largest);
    Ok(Bound(Steps(
        u64::try_from(steps).map_err(|_| NumError::BudgetExceeded)?,
    )))
}

/// One group against one operand: the first point of the group proven inside its split face and
/// clear of the operand that has a ray.
fn against(
    arr: &Arrangement,
    out: &Interfered,
    (splitting, faces): (&Splitting, &[usize]),
    boundary: &Boundary,
    meter: &mut WorkMeter<'_>,
) -> Result<Against, BuildError> {
    let mut tried = 0_usize;
    let mut refused: Option<BuildError> = None;
    let mut first_face = 0_usize;
    for (nth, &index) in faces.iter().enumerate() {
        let one = splitting
            .faces
            .get(index)
            .ok_or(BuildError::Missing(Entity::Face(index)))?;
        if nth == 0 {
            first_face = one.face;
        }
        let missing = BuildError::Missing(Entity::Face(one.face));
        let plane = match arr.faces.get(one.face).map(|face| face.surface) {
            Some(CarrierIx::Plane(ix)) => arr.carriers.plane(ix).ok_or(missing)?,
            _ => return Err(missing),
        };
        for point in candidates(arr, out, one, meter)? {
            if tried >= GROUP_POINTS {
                break;
            }
            tried = tried.saturating_add(1);
            match within(arr, out, one, point, meter) {
                Ok(true) => {}
                Ok(false) | Err(BuildError::Unclassified { .. }) => continue,
                Err(e) => return Err(e),
            }
            if !boundary.clear(&arr.carriers, point, lift(&plane, point)?, meter)? {
                continue;
            }
            match boundary.winding(&arr.carriers, point, meter) {
                Ok(winding) => {
                    return Ok(Against {
                        operand: boundary.operand,
                        winding,
                        at: point,
                        face: index,
                    });
                }
                Err(e @ BuildError::NoRay { .. }) => refused = Some(e),
                Err(e) => return Err(e),
            }
        }
    }
    // A clear point with no ray says more than no clear point.
    match refused {
        Some(e) => Err(e),
        None => Err(BuildError::NoClearBox {
            face: first_face,
            operand: boundary.operand,
            near: evidence(steps_f(boundary.bound) + steps_f(Bound(EPS_FIT)))?,
        }),
    }
}

/// Every group of split faces classified against every other operand (module documentation).
/// `outward` says, for each face of the arrangement, whether its outside is the side its
/// carrier's normal points to, and `bounds` gives each operand's bound, zero for an exact body.
///
/// # Errors
///
/// [`BuildError::NoClearBox`] when no point of a group is proven clear of an operand, and
/// [`BuildError::NoRay`] when a clear point has no ray that qualifies, each with its evidence;
/// [`BuildError::Missing`] for an operand without its bound or a face without its entry; a
/// stopped meter.
pub fn classify(
    arr: &Arrangement,
    out: &Interfered,
    splitting: &Splitting,
    outward: &[bool],
    bounds: &BTreeMap<u32, Bound>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Group>, BuildError> {
    let mut boundaries = Vec::with_capacity(bounds.len());
    for (&operand, &bound) in bounds {
        boundaries.push(Boundary::new(arr, operand, outward, bound, meter)?);
    }
    let mut classified = Vec::new();
    for faces in groups(arr, out, splitting) {
        let Some(&lowest) = faces.first() else {
            continue;
        };
        let face = splitting
            .faces
            .get(lowest)
            .ok_or(BuildError::Missing(Entity::Face(lowest)))?
            .face;
        let operand = arr
            .faces
            .get(face)
            .ok_or(BuildError::Missing(Entity::Face(face)))?
            .operand;
        if !bounds.contains_key(&operand) {
            return Err(BuildError::Missing(Entity::Face(face)));
        }
        let mut found = Vec::new();
        for boundary in &boundaries {
            if boundary.operand == operand {
                continue;
            }
            found.push(against(arr, out, (splitting, &faces), boundary, meter)?);
        }
        classified.push(Group {
            operand,
            faces,
            against: found,
        });
    }
    Ok(classified)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]
mod tests {
    use super::*;

    #[test]
    fn no_two_rays_run_along_one_line() {
        for (i, a) in RAYS.iter().enumerate() {
            assert!(a.iter().any(|&x| x != 0));
            for b in &RAYS[..i] {
                let c = cross(a.map(i128::from), b.map(i128::from)).unwrap();
                assert_ne!(c, [0, 0, 0], "{a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn a_rays_planes_hold_its_line_and_are_square() {
        for direction in RAYS {
            let from = P3::new(7, -3, 11).unwrap();
            let ray = Ray::new(from, direction).unwrap().unwrap();
            let on = |k: f64| {
                let c = coordinates(from);
                let d = direction.map(f64::from);
                [
                    Interval::point(c[0] + k * d[0]).unwrap(),
                    Interval::point(c[1] + k * d[1]).unwrap(),
                    Interval::point(c[2] + k * d[2]).unwrap(),
                ]
            };
            for plane in &ray.planes {
                assert!(plane.value(&on(0.0)).unwrap().contains(0.0));
                assert!(plane.value(&on(5.0)).unwrap().contains(0.0));
                assert!(plane.norm.lo() > 0.0);
            }
            let [a, b] = ray.planes;
            let dot = a.n[0]
                .mul(b.n[0])
                .unwrap()
                .add(a.n[1].mul(b.n[1]).unwrap())
                .unwrap()
                .add(a.n[2].mul(b.n[2]).unwrap())
                .unwrap();
            assert!(dot.contains(0.0));
        }
    }

    #[test]
    fn a_box_is_clear_of_a_ray_only_when_proven() {
        let ray = Ray::new(P3::new(0, 0, 0).unwrap(), [1, 0, 0])
            .unwrap()
            .unwrap();
        let cube = |lo: [f64; 3], hi: [f64; 3]| boxed(lo, hi).unwrap();
        // Beside the line by more than the reach.
        assert!(
            ray.clears(&cube([0.0, 3.0, 0.0], [10.0, 4.0, 0.0]), 2.0)
                .unwrap()
        );
        // Beside it by exactly the reach: not proven.
        assert!(
            !ray.clears(&cube([0.0, 2.0, 0.0], [10.0, 4.0, 0.0]), 2.0)
                .unwrap()
        );
        // Behind the start by more than the reach, on the line.
        assert!(
            ray.clears(&cube([-9.0, 0.0, 0.0], [-3.0, 0.0, 0.0]), 2.0)
                .unwrap()
        );
        assert!(
            !ray.clears(&cube([-9.0, 0.0, 0.0], [-2.0, 0.0, 0.0]), 2.0)
                .unwrap()
        );
        // On the line ahead: never clear, even at reach zero.
        assert!(
            !ray.clears(&cube([4.0, 0.0, 0.0], [5.0, 0.0, 0.0]), 0.0)
                .unwrap()
        );
        // Behind the start at reach zero: clear only strictly behind.
        assert!(
            !ray.clears(&cube([-1.0, 0.0, 0.0], [0.0, 0.0, 0.0]), 0.0)
                .unwrap()
        );
        assert!(
            ray.clears(&cube([-2.0, 0.0, 0.0], [-1.0, 0.0, 0.0]), 0.0)
                .unwrap()
        );
    }

    #[test]
    fn the_lift_is_rounded_up() {
        let plane = Plane::new([0, 0, 2], -3).unwrap();
        // 2z − 3 at z = 0: 3 over 2, rounded up to 2 steps.
        assert_eq!(
            lift(&plane, P3::new(5, 5, 0).unwrap()).unwrap(),
            Bound(Steps(2))
        );
        let level = Plane::new([0, 0, 1], -4).unwrap();
        assert_eq!(
            lift(&level, P3::new(1, 2, 4).unwrap()).unwrap(),
            Bound::ZERO
        );
    }
}
