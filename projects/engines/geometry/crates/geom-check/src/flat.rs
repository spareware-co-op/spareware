//! Chapter 8.3 on flat faces: the face's stored box holds every vertex (group 2), and each loop
//! runs the right way round its face (group 3) (issue #438's flat half, with G177 for curved
//! faces).
//!
//! What it does: [`boxes`] tests every vertex of every flat face's loops against its stored box
//! (`Box3::contains`). [`sense`] takes each loop's exact twice-area vector
//! (`geom_topo::query::loop_area`) against the plane's normal turned by the face's sense. Exactly
//! one loop must run counter-clockwise seen from outside the material, the face's outer loop;
//! every other must run clockwise, a hole; none may enclose no area. The sense of a loop means
//! something only for a loop that does not cross itself, so it runs after group 3's own tests,
//! on a body they passed. A face on another surface, and a flat face with a curved edge in a loop,
//! is left for G177 and counted in the report ([`crate::Report::deferred`]); a flat face's box is
//! still tested against the vertices it has.
//!
//! Guarantees:
//!
//! - **Exact on planes given by integers.** The area vector is below 2^114 in each component and an
//!   integer plane's normal below 2^83, so the dot product, below 2^199, fits 256 bits: its sign is
//!   exact, and the test holds at every coordinate at ±(2^40 − 1). A plane not given by integers
//!   has its normal and the dot product in certified intervals; a sign they cannot decide is
//!   [`FaultKind::Unchecked`], never a pass.
//! - **Runs on sound rings.** It runs only on a body group 1 passed, so every walk ends.
//! - **Charged.** One meter unit per face and fin in each pass.
//!
//! Method: chapter 8.3's rule, on the vector area of a polygon (Goldman, "Area of planar polygons
//! and volume of polyhedra", Graphics Gems II, 1991, doi:10.1016/B978-0-08-050754-5.50043-8) as
//! `geom_topo::query` computes it; the signs are the ones 8.1's sense gives a face
//! (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: each linear in faces and fins; nothing allocated. Recursion: none.

use geom_curve::tight;
use geom_num::Sign;
use geom_num::interval::Interval;
use geom_num::wide::I256;
use geom_run::Stopped;
use geom_topo::entity::{Face, LoopIx};
use geom_topo::ix::Arena;
use geom_topo::query::{QueryError, loop_area};
use geom_topo::{Arenas, Body, Ix, Sense, SurfaceKind};

use crate::combinatorial::{face_fins, face_loops, fin_ends, rows};
use crate::geometric::{plane, point};
use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// A plane's normal, as the data give it: exact integers, or the doubles of its two spanning
/// vectors.
enum Normal {
    Grid([i128; 3]),
    Spanned([f64; 3], [f64; 3]),
}

impl Normal {
    /// The sign of `area` · normal: `None` if the doubles cannot decide it.
    fn dot(&self, area: [i128; 3]) -> Option<Sign> {
        match self {
            Normal::Grid(n) => {
                let mut sum = I256::from_i64(0);
                for (&c, a) in n.iter().zip(area) {
                    let term = I256::from_i128(c).checked_mul(&I256::from_i128(a))?;
                    sum = sum.checked_add(&term)?;
                }
                Some(sum.signum())
            }
            Normal::Spanned(a, b) => spanned(*a, *b, area),
        }
    }
}

/// An interval around an `i128`: the doubles either side of its nearest double.
fn around(x: i128) -> Option<Interval> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the interval's two steps cover the rounding"
    )]
    let nearest = x as f64;
    Interval::new(nearest.next_down(), nearest.next_up()).ok()
}

/// `p q − r s`, in certified intervals exact where the doubles are.
fn minor(p: f64, q: f64, r: f64, s: f64) -> Option<Interval> {
    let product = |x: f64, y: f64| tight::mul(tight::point(x)?, tight::point(y)?);
    tight::sub(product(p, q).ok()?, product(r, s).ok()?).ok()
}

/// The sign of `area` · (u × v) for a plane spanned by u and v, in certified intervals.
fn spanned(u: [f64; 3], v: [f64; 3], area: [i128; 3]) -> Option<Sign> {
    let ([ux, uy, uz], [vx, vy, vz]) = (u, v);
    let [ax, ay, az] = area.map(around);
    let along_x = tight::mul(ax?, minor(uy, vz, uz, vy)?).ok()?;
    let along_y = tight::mul(ay?, minor(uz, vx, ux, vz)?).ok()?;
    let along_z = tight::mul(az?, minor(ux, vy, uy, vx)?).ok()?;
    let dot = tight::add(tight::add(along_x, along_y).ok()?, along_z).ok()?;
    dot.certain_sign()
}

/// What one face's loops came to.
#[derive(Default)]
struct Loops {
    /// How many run counter-clockwise.
    outer: i64,
    /// The first loop that is wrong: a second counter-clockwise one, or one enclosing no area.
    wrong: Option<LoopIx>,
    /// Whether a loop has a curved edge.
    curved: bool,
    /// Whether a sign could not be decided.
    undecided: bool,
}

/// Each loop of `f` against `normal`, with the face's `sense`.
fn loops(
    body: &Body,
    f: Ix<Face>,
    normal: &Normal,
    sense: Sense,
    sink: &mut Sink<'_, '_>,
) -> Result<Loops, Stopped> {
    let mut found = Loops::default();
    for l in face_loops(body.arenas(), f) {
        let area = match loop_area(body, l, sink.meter()) {
            Ok(area) => area,
            Err(QueryError::Stopped(stop)) => return Err(stop),
            Err(_) => {
                found.curved = true;
                continue;
            }
        };
        match normal.dot(area).map(|s| match sense {
            Sense::Forward => s,
            Sense::Reversed => s.negate(),
        }) {
            Some(Sign::Positive) => {
                found.outer = found.outer.saturating_add(1);
                if found.outer > 1 {
                    found.wrong = found.wrong.or(Some(l));
                }
            }
            Some(Sign::Negative) => {}
            Some(Sign::Zero) => found.wrong = found.wrong.or(Some(l)),
            None => found.undecided = true,
        }
    }
    Ok(found)
}

/// The face's box against every vertex of its loops.
fn stored_box(a: &Arenas, f: Ix<Face>, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let bound = a.faces.bound.get(f).copied();
    for n in face_fins(a, f) {
        sink.charge()?;
        if let Some((v, _)) = fin_ends(a, n)
            && bound.is_some_and(|b| !b.contains(point(a, v)))
        {
            sink.push(Fault::at(FaultKind::FaceBound, Entity::Face(f)).with(Entity::Vertex(v)));
        }
    }
    Ok(())
}

/// Stored boxes on `a`'s flat faces.
pub(crate) fn boxes(a: &Arenas, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        let surface = a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s));
        if matches!(surface, Some(SurfaceKind::Plane(_))) {
            stored_box(a, f, sink)?;
        }
    }
    Ok(())
}

/// Loop sense on `body`'s flat faces.
pub(crate) fn sense(body: &Body, sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let a = body.arenas();
    for f in rows::<Face>(a.rows(Arena::Face)) {
        sink.charge()?;
        let surface = a.faces.surface.get(f).copied();
        let Some(kind @ SurfaceKind::Plane(flat)) = surface.and_then(|s| a.surfaces.get(s)) else {
            sink.defer();
            continue;
        };
        let (_, along, across) = flat.vectors();
        let normal = plane(kind).map_or(Normal::Spanned(along, across), |p| Normal::Grid(p.n()));
        let turned = a.faces.sense.get(f).copied().unwrap_or(Sense::Forward);
        let found = loops(body, f, &normal, turned, sink)?;
        if found.curved {
            sink.defer();
        } else if found.undecided {
            let at = surface.map_or(Entity::Face(f), Entity::Surface);
            sink.push(Fault::at(FaultKind::Unchecked, at).with(Entity::Face(f)));
        } else if found.outer != 1 || found.wrong.is_some() {
            let fault = Fault::at(FaultKind::LoopSense, Entity::Face(f)).value(found.outer);
            sink.push(found.wrong.map_or(fault, |l| fault.with(Entity::Loop(l))));
        }
    }
    Ok(())
}
