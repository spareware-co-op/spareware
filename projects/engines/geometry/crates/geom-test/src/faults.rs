//! Checker fault injection (chapter 28.4, the first catalogue): valid bodies broken on purpose, one
//! fault at a time, and the checker must name each with the right kind; and it must pass every
//! valid body, so a checker that rejects too much is caught too.
//!
//! What it does: [`CATALOGUE`] lists every [`Injection`]: a name from 28.4's list or chapter 8.3's
//! groups, the [`FaultKind`]s that are the right answer, and a function that breaks a copy of a
//! valid body. It breaks the rows, not the checker's view of them: each goes through
//! `geom_topo::Builder::edit` and is published again, so only what the builder allows gets through,
//! as with a body from a damaged file or a broken operation. An injection applies to the bodies
//! that have what it breaks: a fin to flip, a line to move, a certified bound to lower. [`attack`]
//! runs the catalogue on one valid body and says, per injection, whether it applied and whether the
//! checker caught it, [`Outcome`]; [`missed`] reads the outcomes as a list of checker bugs.
//!
//! Chapter 28.4 asks for valid bodies from the corpus; the corpus is G114's, so piece G110's tests
//! break primitives and their transforms, and every closed-form construction `geom-make` builds.
//!
//! Guarantees:
//!
//! - **One fault at a time.** Each injection changes one thing a chapter 8.3 group promises to
//!   catch: the broken body differs from the valid one in that thing only, and an injection that
//!   finds nothing to break reports [`Applied::No`], never a pass.
//! - **The right kind.** A fault is caught only by a report that names one of the injection's
//!   kinds. Any other fault is a miss: a checker that reports the wrong thing has not understood
//!   what is broken.
//! - **Both ways.** [`attack`] first checks the body unbroken, so a body it is given that does not
//!   pass is an error, never a catalogue of false catches.
//! - **Just past the bound.** A fault of distance moves by the smallest whole number of steps past
//!   the body's bound (28.4, "by just more than its bound"): a looser check would miss it.
//! - **Deterministic.** Nothing here reads a clock or a random number; the same body gives the
//!   same outcomes on every target.
//!
//! Method: chapter 28.4's fault injection, both ways
//! (`projects/engines/geometry/architecture/28-testing.md`), on chapter 8.3's checker groups
//! (`projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: one full check of the body per injection, each linear in its rows but for group 3's
//! pairs of faces. Recursion: none.

use geom_check::{FaultKind, Level, Report, check};
use geom_curve::{Circle, CurveKind, Line, ParamCurve};
use geom_math::{Box3, GridTransform, TypedTransform};
use geom_num::plane::Plane;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_run::{Budget, Stopped, WorkMeter};
use geom_surface::{ParamSurface, SurfaceKind};
use geom_topo::ix::Arena;
use geom_topo::{
    Arenas, Body, Builder, Edge, Face, Fin, Ix, Loop, PlaneIx, PointIx, PointKind, RegionKind,
    Sense, Shell, TopoError, Vertex,
};

/// A broken body and the exactness record it is checked at.
#[derive(Clone, Debug)]
pub struct Broken {
    /// The body, with one fault.
    pub body: Body,
    /// What it is checked at: the valid body's record, or a lowered one.
    pub exactness: Exactness,
}

/// One fault the checker must catch.
#[derive(Clone, Copy, Debug)]
pub struct Injection {
    /// What it breaks, in chapter 28.4's words where the chapter names it.
    pub name: &'static str,
    /// The kinds that are the right answer.
    pub kinds: &'static [FaultKind],
    /// Breaks a copy of a valid body, recorded at the given exactness; `None` if the body has
    /// nothing this breaks.
    pub inject: fn(&Body, Exactness) -> Option<Broken>,
}

/// Whether an injection found something to break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The body has nothing this breaks.
    No,
    /// It broke the body, and the checker named it.
    Caught,
    /// It broke the body and the checker did not name it: a bug in the checker, or the injection.
    Missed,
}

/// What one injection did to one body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The injection's name.
    pub name: &'static str,
    /// Whether it applied, and whether the checker caught it.
    pub applied: Applied,
    /// What the checker reported instead, when it missed.
    pub reported: Vec<FaultKind>,
}

/// Why [`attack`] could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttackError {
    /// The body given does not pass the checker: the valid half of 28.4 failed.
    NotValid(Vec<FaultKind>),
    /// A check ran out of budget, which an unlimited check never does.
    Stopped(Stopped),
}

/// A full check of `body` at `exactness`.
///
/// # Errors
///
/// A stop of the meter; an unlimited one never stops.
pub fn full_check(body: &Body, exactness: Exactness) -> Result<Report, Stopped> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    check(body, exactness, Level::Full, &mut meter)
}

/// Runs the catalogue on `body`, a valid body recorded at `exactness`.
///
/// # Errors
///
/// [`AttackError::NotValid`] if the body does not pass; [`AttackError::Stopped`] if a check stops.
pub fn attack(body: &Body, exactness: Exactness) -> Result<Vec<Outcome>, AttackError> {
    attack_with(body, exactness, |b, e| {
        full_check(b, e).map(|r| r.faults().iter().map(|f| f.kind).collect())
    })
}

/// [`attack`] with `checker` in place of the full check: how a test shows that a blind checker, or
/// one that rejects too much, is caught. It returns the kinds a body is reported with.
///
/// # Errors
///
/// As for [`attack`].
pub fn attack_with<C>(
    body: &Body,
    exactness: Exactness,
    checker: C,
) -> Result<Vec<Outcome>, AttackError>
where
    C: Fn(&Body, Exactness) -> Result<Vec<FaultKind>, Stopped>,
{
    let valid = checker(body, exactness).map_err(AttackError::Stopped)?;
    if !valid.is_empty() {
        return Err(AttackError::NotValid(valid));
    }
    CATALOGUE
        .iter()
        .map(|injection| {
            let Some(broken) = (injection.inject)(body, exactness) else {
                return Ok(Outcome {
                    name: injection.name,
                    applied: Applied::No,
                    reported: Vec::new(),
                });
            };
            let reported = checker(&broken.body, broken.exactness).map_err(AttackError::Stopped)?;
            let caught = reported.iter().any(|k| injection.kinds.contains(k));
            Ok(Outcome {
                name: injection.name,
                applied: if caught {
                    Applied::Caught
                } else {
                    Applied::Missed
                },
                reported: if caught { Vec::new() } else { reported },
            })
        })
        .collect()
}

/// The names of the injections in `outcomes` the checker missed.
#[must_use]
pub fn missed(outcomes: &[Outcome]) -> Vec<&'static str> {
    outcomes
        .iter()
        .filter(|o| o.applied == Applied::Missed)
        .map(|o| o.name)
        .collect()
}

/// `body`'s rows after `edit`, published again; `None` if `edit` finds nothing to break or the
/// builder refuses.
fn edited(body: &Body, edit: impl FnOnce(&mut Arenas) -> Option<()>) -> Option<Body> {
    let mut builder = Builder::edit(body);
    edit(&mut builder.arenas)?;
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let republished: Result<Body, TopoError> =
        builder.publish(body.kind(), body.outside(), &mut meter);
    republished.ok()
}

/// A [`Broken`] at the body's own record.
fn same(
    body: &Body,
    exactness: Exactness,
    edit: impl FnOnce(&mut Arenas) -> Option<()>,
) -> Option<Broken> {
    edited(body, edit).map(|body| Broken { body, exactness })
}

/// The first row of an arena of `rows` rows that `pick` accepts.
fn first<K>(rows: u32, pick: impl FnMut(&Ix<K>) -> bool) -> Option<Ix<K>> {
    (0..rows).filter_map(Ix::new).find(pick)
}

/// The other sense.
const fn flipped(sense: Sense) -> Sense {
    match sense {
        Sense::Forward => Sense::Reversed,
        Sense::Reversed => Sense::Forward,
    }
}

/// A flipped fin: one fin runs the other way along its edge.
fn flipped_fin(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let f = first(a.rows(Arena::Fin), |_| true)?;
        let sense = *a.fins.sense.get(f)?;
        a.fins.sense.set(f, flipped(sense)).ok()
    })
}

/// A loop's fins taken out of order: each ring link is sound, the loop does not close.
fn open_loop(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        // The first loop of at least four fins on as many edges: a, b, c, d become a, c, b, d. A
        // loop that uses an edge twice, such as a torus's seams, can still close out of order.
        let l = first(a.rows(Arena::Loop), |&l| {
            ring(a, l).is_some_and(|fins| {
                let mut edges: Vec<_> = fins.iter().filter_map(|&f| a.fins.edge.get(f)).collect();
                edges.sort_unstable_by_key(|e| e.row());
                edges.dedup();
                fins.len() >= 4 && edges.len() == fins.len()
            })
        })?;
        let fins = ring(a, l)?;
        let [one, two, three, four] = [*fins.first()?, *fins.get(1)?, *fins.get(2)?, *fins.get(3)?];
        a.fins.next.set(one, three).ok()?;
        a.fins.prev.set(three, one).ok()?;
        a.fins.next.set(three, two).ok()?;
        a.fins.prev.set(two, three).ok()?;
        a.fins.next.set(two, four).ok()?;
        a.fins.prev.set(four, two).ok()
    })
}

/// The fins of loop `l`, in order; `None` for a loop with none or a ring that does not come back.
fn ring(a: &Arenas, l: Ix<Loop>) -> Option<Vec<Ix<Fin>>> {
    let start = (*a.loops.first_fin.get(l)?)?;
    let mut fins = vec![start];
    let mut at = *a.fins.next.get(start)?;
    while at != start {
        if fins.len() > a.rows(Arena::Fin) as usize {
            return None;
        }
        fins.push(at);
        at = *a.fins.next.get(at)?;
    }
    Some(fins)
}

/// A ring of fins cut short: one fin is its own next.
fn cut_ring(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let l = first(a.rows(Arena::Loop), |&l| {
            ring(a, l).is_some_and(|fins| fins.len() >= 2)
        })?;
        let f = *ring(a, l)?.get(1)?;
        a.fins.next.set(f, f).ok()
    })
}

/// A fin whose next fin does not name it back.
fn wrong_prev(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let l = first(a.rows(Arena::Loop), |&l| {
            ring(a, l).is_some_and(|fins| fins.len() >= 3)
        })?;
        let fins = ring(a, l)?;
        a.fins.prev.set(*fins.get(1)?, *fins.get(2)?).ok()
    })
}

/// An edge's ring of fins cut: a fin with a partner is its own radial neighbour.
fn cut_radial(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let f = first(a.rows(Arena::Fin), |&f| {
            a.fins.radial.get(f).is_some_and(|&r| r != f)
        })?;
        a.fins.radial.set(f, f).ok()
    })
}

/// An edge with one end: its end vertex gone, its start kept.
fn one_end(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let e = first(a.rows(Arena::Edge), |&e| {
            a.edges.end.get(e).is_some_and(Option::is_some)
        })?;
        a.edges.end.set(e, None).ok()
    })
}

/// A loop without fins.
fn empty_loop(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let l = first(a.rows(Arena::Loop), |_| true)?;
        a.loops.first_fin.set(l, None).ok()
    })
}

/// A list of faces that comes back on itself.
fn face_cycle(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let f = first(a.rows(Arena::Face), |_| true)?;
        a.faces.next.set(f, Some(f)).ok()
    })
}

/// A loop naming a face other than the one that lists it.
fn wrong_parent(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let (one, other) = (Ix::new(0)?, Ix::new(1)?);
        a.rows(Arena::Face).checked_sub(2)?;
        let l = (*a.faces.first_loop.get(one)?)?;
        a.loops.face.set(l, other).ok()
    })
}

/// A face its shell does not list: the list ends before it.
fn orphan_face(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let f = first(a.rows(Arena::Face), |&f| {
            a.faces.next.get(f).is_some_and(Option::is_some)
        })?;
        a.faces.next.set(f, None).ok()
    })
}

/// A shell in a void: the solid's region turned to void.
fn shell_of_void(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let r = first(a.rows(Arena::Region), |&r| {
            a.regions.kind.get(r) == Some(&RegionKind::Solid)
        })?;
        a.regions.kind.set(r, RegionKind::Void).ok()
    })
}

/// A shell that lists none of its faces.
fn unlisted_faces(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let s = first(a.rows(Arena::Shell), |_| true)?;
        a.shells.first_face.set(s, None).ok()
    })
}

/// A second shell of the solid, with no faces.
fn empty_shell(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let s = first(a.rows(Arena::Shell), |&s| {
            a.shells.next.get(s).is_some_and(Option::is_none)
        })?;
        let region = *a.shells.region.get(s)?;
        let added = a
            .shells
            .push(Shell {
                region,
                next: None,
                first_face: None,
            })
            .ok()?;
        a.shells.next.set(s, Some(added)).ok()
    })
}

/// A body recorded Exact with a curved face or edge: a lowered bound of chapter 28.4, the lowest.
fn exact_on_curved(body: &Body, exactness: Exactness) -> Option<Broken> {
    exactness.bound()?;
    let a = body.arenas();
    let curved = (0..a.rows(Arena::Face))
        .filter_map(Ix::new)
        .filter_map(|f| a.faces.surface.get(f).and_then(|s| a.surfaces.get(*s)))
        .any(|s| !matches!(s, SurfaceKind::Plane(_)));
    let round = (0..a.rows(Arena::Edge))
        .filter_map(Ix::new)
        .filter_map(|e| a.edges.curve.get(e).and_then(|c| a.curves.get(*c)))
        .any(|c| !matches!(c, CurveKind::Line(_)));
    (curved || round).then(|| Broken {
        body: body.clone(),
        exactness: Exactness::Exact,
    })
}

/// The steps a distance fault moves by: the smallest whole number past `exactness`'s bound, with
/// one more so that rounding the move to the grid cannot land it on the bound.
fn past(exactness: Exactness) -> Option<i64> {
    let bound = exactness.bound().map_or(0, |(b, _)| b.get());
    i64::try_from(bound.checked_add(2)?).ok()
}

/// `body`'s first vertex on a flat face, with that face's unit normal.
fn on_flat(a: &Arenas) -> Option<(PointIx, [f64; 3])> {
    for f in (0..a.rows(Arena::Face)).filter_map(Ix::<Face>::new) {
        let surface = a.surfaces.get(*a.faces.surface.get(f)?)?;
        let SurfaceKind::Plane(_) = surface else {
            continue;
        };
        let l = (*a.faces.first_loop.get(f)?)?;
        let fin = (*a.loops.first_fin.get(l)?)?;
        let edge = *a.fins.edge.get(fin)?;
        let v = (*a.edges.start.get(edge)?)?;
        let ((u0, _), (v0, _)) = surface.domain();
        let sides = Default::default();
        let [_, du, dv] = surface.partials(u0, v0, sides).ok()?;
        let [ux, uy, uz] = du;
        let [vx, vy, vz] = dv;
        let n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx];
        let [nx, ny, nz] = n;
        let length = geom_num::math::sqrt(nx * nx + ny * ny + nz * nz)
            .ok()?
            .get();
        return Some((*a.vertices.point.get(v)?, n.map(|c| c / length)));
    }
    None
}

/// `point` moved by `by` steps along the unit `normal`, rounded to the grid.
fn along(point: P3, normal: [f64; 3], by: i64) -> Option<P3> {
    let [nx, ny, nz] = normal;
    let step = |c: geom_num::Coord, n: f64| -> Option<i64> {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            reason = "by is a small count of steps, and the rounded product is a smaller one"
        )]
        let d = geom_num::math::round_ties_even(by as f64 * n).ok()?.get() as i64;
        c.get().checked_add(d)
    };
    P3::new(step(point.x, nx)?, step(point.y, ny)?, step(point.z, nz)?).ok()
}

/// A vertex off its face's plane by just more than the body's bound.
fn vertex_off_surface(body: &Body, exactness: Exactness) -> Option<Broken> {
    let by = past(exactness)?;
    same(body, exactness, |a| {
        let (row, normal) = on_flat(a)?;
        let moved = along(a.points.get(row)?.grid()?, normal, by)?;
        a.points.set(row, moved.into()).ok()
    })
}

/// A stated bound lowered below the real error: a vertex off its plane by `d`, at a record of
/// fewer steps than `d`.
fn lowered_bound(body: &Body, _exactness: Exactness) -> Option<Broken> {
    let broken = edited(body, |a| {
        let (row, normal) = on_flat(a)?;
        let moved = along(a.points.get(row)?.grid()?, normal, 8)?;
        a.points.set(row, moved.into()).ok()
    })?;
    Some(Broken {
        body: broken,
        exactness: Exactness::Certified {
            bound: Steps(6),
            step: GridStep::Q0,
        },
    })
}

/// A line edge moved off its end vertices by just more than the bound.
fn vertex_off_curve(body: &Body, exactness: Exactness) -> Option<Broken> {
    let by = past(exactness)?;
    same(body, exactness, |a| {
        let row = first(a.rows(Arena::Curve), |&c| {
            matches!(a.curves.get(c), Some(CurveKind::Line(_)))
        })?;
        let Some(CurveKind::Line(line)) = a.curves.get(row) else {
            return None;
        };
        let ((t0, t1), [ox, oy, oz], direction) = (line.domain(), line.origin(), line.direction());
        #[allow(clippy::cast_precision_loss, reason = "by is a small count of steps")]
        let moved = Line::new([ox + by as f64, oy, oz], direction, (t0, t1)).ok()?;
        a.curves.set(row, CurveKind::Line(moved)).ok()
    })
}

/// A full circle grown and lifted off its plane by just more than the bound, so it leaves every
/// surface it joins: a cone's cap at once, however the cone slopes.
fn edge_off_surface(body: &Body, exactness: Exactness) -> Option<Broken> {
    let by = past(exactness)?;
    same(body, exactness, |a| {
        let row = first(a.rows(Arena::Curve), |&c| {
            matches!(a.curves.get(c), Some(CurveKind::Circle(_)))
        })?;
        let Some(CurveKind::Circle(circle)) = a.curves.get(row) else {
            return None;
        };
        let (doubles, _) = circle.frame().unit_vectors().ok()?;
        let [_, _, [nx, ny, nz]] = doubles;
        let [cx, cy, cz] = circle.center();
        #[allow(clippy::cast_precision_loss, reason = "by is a small count of steps")]
        let by = by as f64;
        let lifted = [cx + nx * by, cy + ny * by, cz + nz * by];
        let moved = Circle::full(lifted, circle.radius() + by, circle.frame()).ok()?;
        a.curves.set(row, CurveKind::Circle(moved)).ok()
    })
}

/// `ix` moved past `by` rows.
fn past_rows<K>(ix: Ix<K>, by: u32) -> Option<Ix<K>> {
    Ix::new(ix.row().checked_add(by)?)
}

/// As [`past_rows`], for a link that may be absent.
#[allow(
    clippy::option_option,
    reason = "the outer option is a row past the arena's end, the inner one a link that is absent"
)]
fn past_link<K>(ix: Option<Ix<K>>, by: u32) -> Option<Option<Ix<K>>> {
    match ix {
        Some(ix) => past_rows(ix, by).map(Some),
        None => Some(None),
    }
}

/// A quarter of the size of the box round `body`'s faces, along each axis.
fn quarter_size(a: &Arenas) -> Option<[i64; 3]> {
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for f in (0..a.rows(Arena::Face)).filter_map(Ix::<Face>::new) {
        let b = a.faces.bound.get(f)?;
        let (lo, hi) = (b.min(), b.max());
        for (((l, h), x), y) in low
            .iter_mut()
            .zip(&mut high)
            .zip([lo.x, lo.y, lo.z])
            .zip([hi.x, hi.y, hi.z])
        {
            *l = (*l).min(x.get());
            *h = (*h).max(y.get());
        }
    }
    let [lx, ly, lz] = low;
    let [hx, hy, hz] = high;
    Some([
        hx.checked_sub(lx)?.checked_div(4)?,
        hy.checked_sub(ly)?.checked_div(4)?,
        hz.checked_sub(lz)?.checked_div(4)?,
    ])
}

/// Two crossing faces: a flat body's solid gains a second shell, a copy of its own moved a quarter
/// of its size along each axis, so faces of the two cross away from any shared edge. Half its size
/// would seat an L's copy in the L's notch, touching and not crossing.
#[allow(
    clippy::too_many_lines,
    reason = "one pass copies every arena's rows; split, each piece would carry the same counts"
)]
fn crossing_faces(body: &Body, exactness: Exactness) -> Option<Broken> {
    let a = body.arenas();
    if exactness != Exactness::Exact || a.rows(Arena::Shell) != 1 {
        return None;
    }
    let offset = quarter_size(a)?;
    if offset.contains(&0) {
        return None;
    }
    let t = TypedTransform::Exact(GridTransform::translation(offset).ok()?);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let mut builder = Builder::edit(body);
    let out = &mut builder.arenas;
    let (faces, loops, fins, edges, vertices, surfaces, curves, points) = (
        a.rows(Arena::Face),
        a.rows(Arena::Loop),
        a.rows(Arena::Fin),
        a.rows(Arena::Edge),
        a.rows(Arena::Vertex),
        a.rows(Arena::Surface),
        a.rows(Arena::Curve),
        a.rows(Arena::Point),
    );
    let first_shell = Ix::<Shell>::new(0)?;
    let copied = Ix::<Shell>::new(1)?;
    out.shells
        .push(Shell {
            region: *a.shells.region.get(first_shell)?,
            next: None,
            first_face: past_link(*a.shells.first_face.get(first_shell)?, faces)?,
        })
        .ok()?;
    out.shells.next.set(first_shell, Some(copied)).ok()?;
    for row in (0..faces).filter_map(Ix::<Face>::new) {
        let bound = *a.faces.bound.get(row)?;
        out.faces
            .push(Face {
                shell: copied,
                next: past_link(*a.faces.next.get(row)?, faces)?,
                surface: past_rows(*a.faces.surface.get(row)?, surfaces)?,
                sense: *a.faces.sense.get(row)?,
                first_loop: past_link(*a.faces.first_loop.get(row)?, loops)?,
                bound: Box3::new(
                    t.apply(bound.min()).ok()?.value(),
                    t.apply(bound.max()).ok()?.value(),
                )
                .ok()?,
            })
            .ok()?;
    }
    for row in (0..loops).filter_map(Ix::<Loop>::new) {
        out.loops
            .push(Loop {
                face: past_rows(*a.loops.face.get(row)?, faces)?,
                next: past_link(*a.loops.next.get(row)?, loops)?,
                first_fin: past_link(*a.loops.first_fin.get(row)?, fins)?,
            })
            .ok()?;
    }
    for row in (0..fins).filter_map(Ix::<Fin>::new) {
        out.fins
            .push(Fin {
                loop_: past_rows(*a.fins.loop_.get(row)?, loops)?,
                next: past_rows(*a.fins.next.get(row)?, fins)?,
                prev: past_rows(*a.fins.prev.get(row)?, fins)?,
                edge: past_rows(*a.fins.edge.get(row)?, edges)?,
                sense: *a.fins.sense.get(row)?,
                radial: past_rows(*a.fins.radial.get(row)?, fins)?,
                pcurve: None,
            })
            .ok()?;
    }
    for row in (0..edges).filter_map(Ix::<Edge>::new) {
        out.edges
            .push(Edge {
                first_fin: past_link(*a.edges.first_fin.get(row)?, fins)?,
                start: past_link(*a.edges.start.get(row)?, vertices)?,
                end: past_link(*a.edges.end.get(row)?, vertices)?,
                curve: past_rows(*a.edges.curve.get(row)?, curves)?,
            })
            .ok()?;
    }
    for row in (0..vertices).filter_map(Ix::<Vertex>::new) {
        out.vertices
            .push(Vertex {
                point: past_rows(*a.vertices.point.get(row)?, points)?,
            })
            .ok()?;
    }
    for row in (0..surfaces).filter_map(Ix::new) {
        let moved = a.surfaces.get(row)?.transformed(&t, &mut meter).ok()?;
        out.surfaces.push(moved.value).ok()?;
    }
    for row in (0..curves).filter_map(Ix::new) {
        let moved = a.curves.get(row)?.transformed(&t, &mut meter).ok()?;
        out.curves.push(moved.value).ok()?;
    }
    for row in (0..points).filter_map(Ix::new) {
        out.points
            .push(t.apply(a.points.get(row)?.grid()?).ok()?.value().into())
            .ok()?;
    }
    let body = builder
        .publish(body.kind(), body.outside(), &mut meter)
        .ok()?;
    Some(Broken { body, exactness })
}

/// The first point of `a` that is a meeting of planes, with its planes.
fn first_meeting(a: &Arenas) -> Option<(PointIx, [PlaneIx; 3])> {
    (0..a.rows(Arena::Point))
        .filter_map(Ix::new)
        .find_map(|row| match a.points.get(row)? {
            PointKind::Meet(planes) => Some((row, *planes)),
            PointKind::Grid(_) => None,
        })
}

/// A vertex whose three planes are not three: one given twice.
fn singular_meeting(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let (row, [p, q, _]) = first_meeting(a)?;
        a.points.set(row, PointKind::Meet([p, p, q])).ok()
    })
}

/// A vertex given another vertex's planes, none of its faces' but those they share.
fn meeting_elsewhere(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let (row, planes) = first_meeting(a)?;
        let other = (0..a.rows(Arena::Point))
            .filter_map(Ix::new)
            .find_map(|r| match a.points.get(r)? {
                PointKind::Meet(found) if found.iter().all(|p| !planes.contains(p)) => Some(*found),
                _ => None,
            })?;
        a.points.set(row, PointKind::Meet(other)).ok()
    })
}

/// A plane of the pool moved a step of its offset, so what meets on it leaves the other faces.
fn plane_moved(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let (_, [row, _, _]) = first_meeting(a)?;
        let plane = a.planes.get(row)?;
        let moved = Plane::new(plane.n(), plane.d().checked_add(1)?).ok()?;
        a.planes.set(row, moved).ok()
    })
}

/// The first edge of `a` that starts at a meeting of planes.
fn edge_from_meeting(a: &Arenas) -> Option<(Ix<Edge>, Ix<Vertex>)> {
    (0..a.rows(Arena::Edge))
        .filter_map(Ix::<Edge>::new)
        .find_map(|e| {
            let v = (*a.edges.start.get(e)?)?;
            let place = a.points.get(*a.vertices.point.get(v)?)?;
            matches!(place, PointKind::Meet(_)).then_some((e, v))
        })
}

/// A shadow line that ends far from the vertex it shadows.
fn shadow_far(body: &Body, exactness: Exactness) -> Option<Broken> {
    let by = f64::from(i32::try_from(past(exactness)?.checked_add(4)?).ok()?);
    same(body, exactness, |a| {
        let (e, _) = edge_from_meeting(a)?;
        let curve = *a.edges.curve.get(e)?;
        let CurveKind::Line(line) = a.curves.get(curve)? else {
            return None;
        };
        let [x, y, z] = line.origin();
        let moved = Line::new([x, y, z + by], line.direction(), line.domain()).ok()?;
        a.curves.set(curve, CurveKind::Line(moved)).ok()
    })
}

/// A face's stored box shrunk to a point of its first meeting's far side, so it holds no corner.
fn box_off_meeting(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        let f = (0..a.rows(Arena::Face))
            .filter_map(Ix::<Face>::new)
            .find(|f| {
                a.faces
                    .surface
                    .get(*f)
                    .and_then(|s| a.surfaces.get(*s))
                    .is_some_and(|s| matches!(s, SurfaceKind::Plane(_)))
            })?;
        first_meeting(a)?;
        let high = a.faces.bound.get(f)?.max();
        let far = P3::new(
            high.x.get().checked_add(9)?,
            high.y.get().checked_add(9)?,
            high.z.get().checked_add(9)?,
        )
        .ok()?;
        a.faces.bound.set(f, Box3::new(far, far).ok()?).ok()
    })
}

/// A face turned the wrong way on a body whose vertices meet on planes: its loops read backwards.
fn turned_face(body: &Body, exactness: Exactness) -> Option<Broken> {
    same(body, exactness, |a| {
        first_meeting(a)?;
        let f: Ix<Face> = first(a.rows(Arena::Face), |_| true)?;
        let turned = flipped(*a.faces.sense.get(f)?);
        a.faces.sense.set(f, turned).ok()
    })
}

/// The catalogue, in chapter 8.3's group order.
pub const CATALOGUE: &[Injection] = &[
    Injection {
        name: "a flipped fin",
        kinds: &[FaultKind::LoopOpen, FaultKind::PartnerSense],
        inject: flipped_fin,
    },
    Injection {
        name: "an open loop",
        kinds: &[FaultKind::LoopOpen],
        inject: open_loop,
    },
    Injection {
        name: "a ring of fins cut short",
        kinds: &[FaultKind::FinRing],
        inject: cut_ring,
    },
    Injection {
        name: "a fin its next fin does not name",
        kinds: &[FaultKind::FinPrev],
        inject: wrong_prev,
    },
    Injection {
        name: "a radial ring cut",
        kinds: &[FaultKind::RadialRing, FaultKind::OffRadialRing],
        inject: cut_radial,
    },
    Injection {
        name: "an edge with one end",
        kinds: &[FaultKind::EdgeEnds],
        inject: one_end,
    },
    Injection {
        name: "a loop without fins",
        kinds: &[FaultKind::EmptyLoop],
        inject: empty_loop,
    },
    Injection {
        name: "a list that comes back on itself",
        kinds: &[FaultKind::ChainCycle],
        inject: face_cycle,
    },
    Injection {
        name: "a loop naming the wrong face",
        kinds: &[FaultKind::WrongParent],
        inject: wrong_parent,
    },
    Injection {
        name: "a face its shell does not list",
        kinds: &[FaultKind::Orphan],
        inject: orphan_face,
    },
    Injection {
        name: "a shell of a void region",
        kinds: &[FaultKind::ShellOfVoid],
        inject: shell_of_void,
    },
    Injection {
        name: "a shell that lists none of its faces",
        kinds: &[FaultKind::Orphan],
        inject: unlisted_faces,
    },
    Injection {
        name: "an empty shell",
        kinds: &[FaultKind::EmptyShell],
        inject: empty_shell,
    },
    Injection {
        name: "an exact record on curved geometry",
        kinds: &[FaultKind::NotFlat, FaultKind::NotStraight],
        inject: exact_on_curved,
    },
    Injection {
        name: "a vertex off its surface by just more than its bound",
        kinds: &[FaultKind::VertexOffSurface],
        inject: vertex_off_surface,
    },
    Injection {
        name: "a stated bound lowered below the real error",
        kinds: &[FaultKind::VertexOffSurface],
        inject: lowered_bound,
    },
    Injection {
        name: "a vertex off its curve by just more than its bound",
        kinds: &[FaultKind::VertexOffCurve],
        inject: vertex_off_curve,
    },
    Injection {
        name: "an edge off its surface by just more than its bound",
        kinds: &[FaultKind::EdgeOffSurface],
        inject: edge_off_surface,
    },
    Injection {
        name: "two crossing faces",
        kinds: &[FaultKind::FacesCross],
        inject: crossing_faces,
    },
    Injection {
        name: "three planes that do not meet",
        kinds: &[FaultKind::MeetSingular],
        inject: singular_meeting,
    },
    Injection {
        name: "a vertex given another's planes",
        kinds: &[FaultKind::VertexOffSurface],
        inject: meeting_elsewhere,
    },
    Injection {
        name: "a plane of the pool moved",
        kinds: &[FaultKind::VertexOffSurface],
        inject: plane_moved,
    },
    Injection {
        name: "a shadow line far from its vertex",
        kinds: &[FaultKind::VertexOffCurve],
        inject: shadow_far,
    },
    Injection {
        name: "a face's box off its corners",
        kinds: &[FaultKind::FaceBound],
        inject: box_off_meeting,
    },
    Injection {
        name: "a face turned the wrong way",
        kinds: &[FaultKind::LoopSense],
        inject: turned_face,
    },
];
