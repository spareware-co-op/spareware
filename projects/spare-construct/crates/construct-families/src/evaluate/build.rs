//! The solid steps, run in recipe order through the engine.
//!
//! - `extrude`: the profile less its holes, from the lower of its two heights to the higher, along
//!   the sketch plane's normal (the order the two are written in does not matter; equal heights
//!   are refused). The corners sit at the plane's offset plus the lower height.
//! - `subtract`: its target less its tool, both earlier steps. An extrusion used as a tool is the
//!   recipe's void: it cuts exactly the target its subtract step names, and nothing else.
//! - `unite`: its two earlier steps as one solid.
//!
//! A step a later step uses is that step's input and is not drawn; every other step is one of
//! the family's solids, in recipe order, with its subcategory, material and visibility.
//!
//! Each step is counted against the solid-step limit before it runs, and each extrusion's
//! corners against the result-size limit before the engine is asked (`limits.rs`).
//!
//! A family built turned ([`Turn`], piece A64) has every extrusion's corners and its vector
//! turned before the engine is asked; nothing else changes.
//!
//! A family seated in a host at any angle ([`Seat`], piece A83) has every extrusion's corners put
//! through its seat, each rounded once to the grid. A profile on a level (`xy`) plane stays on one
//! level plane however its corners round, so it is extruded straight up as it is. A profile on an
//! upright plane (`xz` or `yz`) does not: turned off the quarter turns, an upright plane holds grid
//! points only `L / gcd` apart along it (`L` the way in steps, `gcd` its two coordinates'), so
//! three rounded corners of one upright face are in general not on one plane, and the engine takes
//! no such profile. It is made of upright boxes instead, each a level rectangle extruded straight
//! up, which stays a flat-faced prism whatever its corners round to ([`upright`]): the box around
//! its outline, less one cutter for each rectangle of what the outline leaves out of that box and
//! for each rectangle of each hole, every cutter reaching [`PAST`] past the solid where it opens
//! onto the outside, so no face of a cutter lies on a face of the box. That holds for a profile
//! whose every edge runs along its sketch's two axes, as the library's frames, leaves, sashes and
//! glass do; one with a sloped edge is refused off the quarter turns (`UprightNotSquare`).
//! Interim: the kernel's `Engine` interface cannot turn a solid once it is made; an engine that
//! can (a later kernel piece, with the exact kernel) builds every profile turned as it is, and
//! this rule goes.

use super::error::{EvalError, Place, Reason};
use super::limits::{Limits, Meter};
use super::seat::Seat;
use super::sketch::{Drawn, resolve};
use super::turn::Turn;
use super::values::{TypeValues, Worked, work_out};
use super::{Built, Evaluated};
use crate::check::Sort;
use crate::model::{Base, Key, MaterialRef, Operation, Recipe, Step};
use construct_params::Length;
use engine_geometry::value::engine::{Engine, Profile};
use engine_geometry::value::grid::{GridStep, P3};
use kit_core::Id;
use std::collections::{BTreeMap, BTreeSet};

/// How far a cutter of an upright profile built at an angle reaches past the solid it cuts, where
/// it opens onto the outside: 10 mm, a thousand times the model's tolerance, in steps.
pub(crate) const PAST: i64 = 64_000;

/// How a family stands while it is built: turned by quarter turns, or seated at any angle.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Stand<'s> {
    /// Turned about its z axis by whole quarter turns: exact.
    Turned(Turn),
    /// Seated in a host at any angle: every corner rounded once.
    Seated(&'s Seat),
}

/// Evaluates `recipe` for the type whose values are `values`, asking `engine` for every solid,
/// within `limits`, the family standing as `stand` says.
pub(crate) fn run<E: Engine>(
    engine: &mut E,
    recipe: &Recipe,
    values: &TypeValues,
    limits: Limits,
    stand: Stand<'_>,
) -> Result<Evaluated<E::Solid>, EvalError> {
    let step = engine.edition().step;
    if step != GridStep::Q0 {
        return Err(EvalError::new(Place::Engine, Reason::WrongGrid { step }));
    }
    let mut meter = Meter::new(limits);
    let worked = work_out(recipe, values, &mut meter)?;
    let mut sketches: BTreeMap<Key, Drawn> = BTreeMap::new();
    let mut built: Vec<(Key, E::Solid)> = Vec::with_capacity(recipe.steps.len());
    let mut used: BTreeSet<Key> = BTreeSet::new();
    let mut materials: Vec<Id> = Vec::with_capacity(recipe.steps.len());
    for step in &recipe.steps {
        let at = |reason| EvalError::new(Place::Step(step.key.clone()), reason).in_step(&step.key);
        if built.iter().any(|(key, _)| *key == step.key) {
            return Err(at(Reason::Duplicate {
                key: step.key.as_str().to_owned(),
            }));
        }
        meter.solid_step().map_err(at)?;
        let solid = match &step.operation {
            Operation::Extrude {
                sketch,
                profile,
                holes,
                start,
                end,
            } => {
                if !sketches.contains_key(sketch) {
                    let drawn = resolve(recipe, sketch, &worked, &mut meter)
                        .map_err(|e| e.in_step(&step.key))?;
                    sketches.insert(sketch.clone(), drawn);
                }
                let drawn = sketches.get(sketch).ok_or_else(|| {
                    at(Reason::Missing {
                        sort: Sort::Sketch,
                        key: sketch.as_str().to_owned(),
                    })
                })?;
                let heights = (start.as_str(), end.as_str());
                extrude(
                    engine,
                    drawn,
                    &worked,
                    &mut meter,
                    (profile, holes),
                    heights,
                    stand,
                )
                .map_err(at)?
            }
            Operation::Subtract { target, tool } => {
                let target_solid = input(recipe, &built, step, target).map_err(at)?;
                let tool_solid = input(recipe, &built, step, tool).map_err(at)?;
                let solid = engine
                    .subtract(target_solid, tool_solid)
                    .map_err(|e| at(Reason::Engine(e)))?;
                used.extend([target.clone(), tool.clone()]);
                solid
            }
            Operation::Unite { a, b } => {
                let a_solid = input(recipe, &built, step, a).map_err(at)?;
                let b_solid = input(recipe, &built, step, b).map_err(at)?;
                let solid = engine
                    .unite(a_solid, b_solid)
                    .map_err(|e| at(Reason::Engine(e)))?;
                used.extend([a.clone(), b.clone()]);
                solid
            }
        };
        materials.push(material(&worked, step).map_err(at)?);
        built.push((step.key.clone(), solid));
    }
    let solids = recipe
        .steps
        .iter()
        .zip(built)
        .zip(materials)
        .filter(|((step, _), _)| !used.contains(&step.key))
        .map(|((step, (_, solid)), material)| Built {
            step: step.key.clone(),
            solid,
            subcategory: step.subcategory,
            material,
            visibility: step.visibility,
        })
        .collect();
    Ok(Evaluated {
        solids,
        values: worked.all,
        used: meter.used(),
    })
}

/// One extrusion step's solid.
fn extrude<E: Engine>(
    engine: &mut E,
    drawn: &Drawn,
    worked: &Worked,
    meter: &mut Meter,
    (profile, holes): (&[Key], &[Vec<Key>]),
    (start, end): (&str, &str),
    stand: Stand<'_>,
) -> Result<E::Solid, Reason> {
    let start = worked.length(start, meter)?;
    let end = worked.length(end, meter)?;
    let (low, high) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    if low == high {
        return Err(Reason::NoThickness);
    }
    let depth = high.checked_sub(low).ok_or(Reason::OutOfRange)?;
    let turn = match stand {
        Stand::Turned(turn) => turn,
        Stand::Seated(seat) => {
            return seated(engine, drawn, meter, (profile, holes), (low, depth), seat);
        }
    };
    let turned = |corners: Vec<P3>| {
        corners
            .into_iter()
            .map(|p| turn.point(p))
            .collect::<Result<Vec<P3>, Reason>>()
    };
    let outer = turned(drawn.corners(profile, low)?)?;
    let holes = holes
        .iter()
        .map(|hole| turned(drawn.corners(hole, low)?))
        .collect::<Result<Vec<_>, _>>()?;
    meter.corners(
        holes
            .iter()
            .fold(outer.len(), |n, h| n.saturating_add(h.len())),
    )?;
    engine
        .extrude(&Profile { outer, holes }, turn.vector(drawn.along(depth))?)
        .map_err(Reason::Engine)
}

/// One extrusion step's solid, the family seated at `seat` (this module's header): the profile at
/// `low` along its plane's normal, `depth` deep.
fn seated<E: Engine>(
    engine: &mut E,
    drawn: &Drawn,
    meter: &mut Meter,
    (profile, holes): (&[Key], &[Vec<Key>]),
    (low, depth): (Length, Length),
    seat: &Seat,
) -> Result<E::Solid, Reason> {
    let outer = drawn.corners(profile, low)?;
    let holes = holes
        .iter()
        .map(|hole| drawn.corners(hole, low))
        .collect::<Result<Vec<_>, _>>()?;
    let base = drawn.base();
    if seat.is_square() || base == Base::Xy {
        let put = |corners: Vec<P3>| {
            corners
                .into_iter()
                .map(|p| seat.point(p))
                .collect::<Result<Vec<P3>, Reason>>()
        };
        let outer = put(outer)?;
        let holes = holes.into_iter().map(put).collect::<Result<Vec<_>, _>>()?;
        meter.corners(
            holes
                .iter()
                .fold(outer.len(), |n, h| n.saturating_add(h.len())),
        )?;
        let [x, y, z] = drawn.along(depth);
        let [x, y] = seat.vector([x, y]).ok_or(Reason::OutOfRange)?;
        return engine
            .extrude(&Profile { outer, holes }, [x, y, z])
            .map_err(Reason::Engine);
    }
    upright(
        engine,
        base == Base::Yz,
        (&outer, &holes),
        depth,
        meter,
        seat,
    )
}

/// A rectangle of an upright profile: from `u0` to `u1` along the sketch's first coordinate and
/// from `v0` to `v1` up, in steps, as `[u0, u1, v0, v1]`.
type Rectangle = [i64; 4];

/// The corner after corner `i` of a loop of `len` corners.
fn after(i: usize, len: usize) -> Option<usize> {
    i.checked_add(1).and_then(|n| n.checked_rem(len))
}

/// Whether every edge of `ring` (its corners in loop order) runs along one of its two axes.
fn square(ring: &[[i64; 2]]) -> bool {
    ring.iter().enumerate().all(|(i, a)| {
        after(i, ring.len())
            .and_then(|n| ring.get(n))
            .is_some_and(|b| a[0] == b[0] || a[1] == b[1])
    })
}

/// The rectangles whose union is the region inside `rings` (each a loop of corners, every edge
/// along an axis), a point being inside when an odd number of the rings hold it: the region cut
/// by a line across at every corner's height into slabs, each slab's stretches inside found from
/// the upright edges that span it, and each stretch made as tall as the region lets it be (while
/// the slabs above and below hold the whole stretch). Sorted, each once, so one region always
/// gives the same rectangles.
fn rectangles(rings: &[&[[i64; 2]]]) -> Vec<Rectangle> {
    let mut heights: Vec<i64> = rings.iter().flat_map(|r| r.iter().map(|p| p[1])).collect();
    heights.sort_unstable();
    heights.dedup();
    let slabs: Vec<(i64, i64)> = heights
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((*a, *b)),
            _ => None,
        })
        .collect();
    let stretches: Vec<Vec<(i64, i64)>> = slabs
        .iter()
        .map(|&(low, high)| {
            let mut crossings: Vec<i64> = Vec::new();
            for ring in rings {
                for (i, a) in ring.iter().enumerate() {
                    let Some(b) = after(i, ring.len()).and_then(|n| ring.get(n)) else {
                        continue;
                    };
                    if a[0] == b[0] && a[1].min(b[1]) <= low && a[1].max(b[1]) >= high {
                        crossings.push(a[0]);
                    }
                }
            }
            crossings.sort_unstable();
            let (pairs, _) = crossings.as_chunks::<2>();
            pairs
                .iter()
                .filter_map(|&[a, b]| (a < b).then_some((a, b)))
                .collect()
        })
        .collect();
    let holds = |slab: usize, (a, b): (i64, i64)| {
        stretches
            .get(slab)
            .is_some_and(|s| s.iter().any(|&(c, d)| c <= a && b <= d))
    };
    let mut found = BTreeSet::new();
    for (k, slab) in stretches.iter().enumerate() {
        for &stretch in slab {
            let (mut first, mut last) = (k, k);
            while first > 0 && holds(first.saturating_sub(1), stretch) {
                first = first.saturating_sub(1);
            }
            while holds(last.saturating_add(1), stretch) {
                last = last.saturating_add(1);
            }
            if let (Some(&(v0, _)), Some(&(_, v1))) = (slabs.get(first), slabs.get(last)) {
                found.insert([stretch.0, stretch.1, v0, v1]);
            }
        }
    }
    found.into_iter().collect()
}

/// An upright profile (on an `xz` plane, or a `yz` one when `side`), its corners `outer` and its
/// holes `holes` at the plane's level, `depth` deep, seated at `seat` off the quarter turns, made
/// of upright boxes (this module's header): the box around the outline less a cutter for each
/// rectangle of what the outline leaves out of it, then for each rectangle of each hole, each list
/// in its sorted order.
fn upright<E: Engine>(
    engine: &mut E,
    side: bool,
    (outer, holes): (&[P3], &[Vec<P3>]),
    depth: Length,
    meter: &mut Meter,
    seat: &Seat,
) -> Result<E::Solid, Reason> {
    // The sketch's two coordinates (along, up), and its level along the plane's normal.
    let flat = |p: &P3| {
        if side {
            [p.y.get(), p.z.get()]
        } else {
            [p.x.get(), p.z.get()]
        }
    };
    let level = outer
        .first()
        .map(|p| if side { p.x.get() } else { p.y.get() })
        .ok_or(Reason::NotALoop)?;
    let ring: Vec<[i64; 2]> = outer.iter().map(flat).collect();
    let hole_rings: Vec<Vec<[i64; 2]>> =
        holes.iter().map(|h| h.iter().map(flat).collect()).collect();
    if !square(&ring) || !hole_rings.iter().all(|h| square(h)) {
        return Err(Reason::UprightNotSquare);
    }
    let reach = |pick: usize| {
        let values = ring.iter().filter_map(|p| p.get(pick).copied());
        Some((values.clone().min()?, values.max()?))
    };
    let ((u_min, u_max), (v_min, v_max)) = reach(0).zip(reach(1)).ok_or(Reason::NotALoop)?;
    let around = [
        [u_min, v_min],
        [u_max, v_min],
        [u_max, v_max],
        [u_min, v_max],
    ];
    let moved = |at: i64, by: i64| at.checked_add(by).ok_or(Reason::OutOfRange);
    let mut cutters = Vec::new();
    for [u0, u1, v0, v1] in rectangles(&[&around, &ring]) {
        // Past the box where the rectangle meets its side.
        let past = |at: i64, edge: i64, by: i64| if at == edge { moved(at, by) } else { Ok(at) };
        cutters.push([
            past(u0, u_min, -PAST)?,
            past(u1, u_max, PAST)?,
            past(v0, v_min, -PAST)?,
            past(v1, v_max, PAST)?,
        ]);
    }
    for hole in &hole_rings {
        cutters.extend(rectangles(&[hole]));
    }
    meter.corners(cutters.len().saturating_add(1).saturating_mul(4))?;
    let far = moved(level, depth.steps())?;
    let reaching = (moved(level, -PAST)?, moved(far, PAST)?);
    let mut made = upright_box(
        engine,
        side,
        [u_min, u_max, v_min, v_max],
        (level, far),
        seat,
    )?;
    for cutter in cutters {
        let tool = upright_box(engine, side, cutter, reaching, seat)?;
        made = engine.subtract(&made, &tool).map_err(Reason::Engine)?;
    }
    Ok(made)
}

/// The upright box of the rectangle `[u0, u1, v0, v1]` of an upright profile (on a `yz` plane when
/// `side`) from `w0` to `w1` along the plane's normal, seated: its level rectangle at `v0`, each
/// corner put through the seat, extruded straight up to `v1`.
fn upright_box<E: Engine>(
    engine: &mut E,
    side: bool,
    [u0, u1, v0, v1]: Rectangle,
    (w0, w1): (i64, i64),
    seat: &Seat,
) -> Result<E::Solid, Reason> {
    // Anticlockwise from above.
    let plan = if side {
        [[w0, u0], [w1, u0], [w1, u1], [w0, u1]]
    } else {
        [[u0, w0], [u1, w0], [u1, w1], [u0, w1]]
    };
    let outer = plan
        .iter()
        .map(|&[x, y]| {
            P3::new(x, y, v0)
                .map_err(|_| Reason::OutOfRange)
                .and_then(|p| seat.point(p))
        })
        .collect::<Result<Vec<P3>, Reason>>()?;
    let up = v1.checked_sub(v0).ok_or(Reason::OutOfRange)?;
    engine
        .extrude(
            &Profile {
                outer,
                holes: Vec::new(),
            },
            [0, 0, up],
        )
        .map_err(Reason::Engine)
}

/// An earlier step's solid, which `step` uses.
fn input<'b, S>(
    recipe: &Recipe,
    built: &'b [(Key, S)],
    step: &Step,
    key: &Key,
) -> Result<&'b S, Reason> {
    if let Some((_, solid)) = built.iter().find(|(k, _)| k == key) {
        return Ok(solid);
    }
    let named = key.as_str().to_owned();
    if *key == step.key || recipe.steps.iter().any(|s| s.key == *key) {
        Err(Reason::NotEarlier { key: named })
    } else {
        Err(Reason::Missing {
            sort: Sort::Step,
            key: named,
        })
    }
}

/// A step's material: fixed, or its material parameter's value for this type.
fn material(worked: &Worked, step: &Step) -> Result<Id, Reason> {
    match &step.material {
        MaterialRef::Fixed(id) => Ok(id.clone()),
        MaterialRef::Parameter(key) => worked.material(key),
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "tests; small whole numbers"
)]
mod tests {
    use super::*;

    /// A case: its name, its rings, and how many rectangles it is.
    type Case<'a> = (&'a str, Vec<&'a [[i64; 2]]>, usize);

    /// Whether the cell `[u, u + 1] x [v, v + 1]` is inside `rings`, by the even-odd rule.
    fn inside(rings: &[&[[i64; 2]]], u: i64, v: i64) -> bool {
        let (cu, cv) = (2 * u + 1, 2 * v + 1);
        let mut odd = false;
        for ring in rings {
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                // An upright edge to the right of the cell's centre, spanning its height.
                if a[0] == b[0] && 2 * a[0] > cu && (2 * a[1] < cv) != (2 * b[1] < cv) {
                    odd = !odd;
                }
            }
        }
        odd
    }

    /// The rectangles of a region cover exactly the region, cell by cell, and each lies inside it:
    /// a door's frame left out of its box (the notch under its head), a window's frame's hole, an
    /// L, a stair and an E (whose two notches stand one over the other and stay apart).
    #[test]
    fn the_rectangles_of_a_region_are_exactly_the_region() {
        let around = [[0, 0], [10, 0], [10, 9], [0, 9]];
        let u = [
            [0, 0],
            [2, 0],
            [2, 7],
            [8, 7],
            [8, 0],
            [10, 0],
            [10, 9],
            [0, 9],
        ];
        let hole = [[2, 2], [8, 2], [8, 7], [2, 7]];
        let l = [[0, 0], [10, 0], [10, 3], [4, 3], [4, 9], [0, 9]];
        let stair = [
            [0, 0],
            [10, 0],
            [10, 3],
            [7, 3],
            [7, 6],
            [4, 6],
            [4, 9],
            [0, 9],
        ];
        let e = [
            [0, 0],
            [10, 0],
            [10, 2],
            [3, 2],
            [3, 4],
            [10, 4],
            [10, 5],
            [3, 5],
            [3, 7],
            [10, 7],
            [10, 9],
            [0, 9],
        ];
        let cases: [Case<'_>; 5] = [
            ("door frame", vec![&around, &u], 1),
            ("window hole", vec![&hole], 1),
            ("l", vec![&around, &l], 1),
            ("stair", vec![&around, &stair], 2),
            ("e", vec![&around, &e], 2),
        ];
        for (name, rings, count) in cases {
            let found = rectangles(&rings);
            assert_eq!(found.len(), count, "{name}: {found:?}");
            for u in -1..11 {
                for v in -1..10 {
                    let covered = found
                        .iter()
                        .any(|r| r[0] <= u && u < r[1] && r[2] <= v && v < r[3]);
                    assert_eq!(covered, inside(&rings, u, v), "{name} at ({u}, {v})");
                }
            }
        }
        // The notch under the door's head runs from the foot to the head, between the jambs.
        assert_eq!(rectangles(&[&around, &u]), vec![[2, 8, 0, 7]]);
        // A stair's steps: the narrow one as tall as the region lets it be.
        assert_eq!(
            rectangles(&[&around, &stair]),
            vec![[4, 10, 6, 9], [7, 10, 3, 9]]
        );
    }

    /// A ring is square when every edge runs along one of its axes.
    #[test]
    fn a_ring_is_square_when_every_edge_runs_along_an_axis() {
        assert!(square(&[[0, 0], [5, 0], [5, 5], [0, 5]]));
        assert!(!square(&[[0, 0], [5, 0], [5, 5], [2, 7], [0, 5]]));
    }
}
