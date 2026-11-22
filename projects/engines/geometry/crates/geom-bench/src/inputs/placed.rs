//! Inputs for placed instances (piece G145; chapters 19.12 and 23.9): definitions placed exactly,
//! for [`areas`] (`geom_topo::placed::face_area`) and [`sides`] (`geom_topo::placed::side_of_shell`),
//! and definitions turned in plan, for [`turns`] (`geom_make::placed::materialise`).
//!
//! What it does: each definition is a primitive within ±2^20 grid steps of the origin, its sizes
//! from 4 to 2^16 steps, half of them boxes and half prisms on a right triangle, as the core
//! queries' inputs are (`inputs/query.rs`). An exact placement is one of the 48 signed axis
//! permutations, drawn by its packed number, with a shift within ±2^20 steps; an area input asks
//! for one face of its instance, and a side input for one point, half of them inside the instance
//! and half outside it, never on its boundary. A turn is the definition with one of eight rotations
//! about z by a Pythagorean direction, a placed copy turned in plan as a building places one, so the
//! sides of the turned body stay upright and its rounded corners keep every face flat.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`), every
//! answer on an exact placement equals the same query on its materialisation, every point's side
//! is decided, and every turned body is certified within [`ACCEPT`]'s bound. Only the typical class
//! exists: the refusals are tested in `geom-topo` and `geom-make` themselves.
//!
//! Method: the cases of `geom-make`'s own tests (`tests/placed.rs`: a box and a prism under each
//! of the 48 signed permutations with a shift, and a rotation about z by a Pythagorean direction
//! accepted as certified within 64 steps), drawn from a stream; each is measured as chapter 28.14
//! sets benchmarks out, `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: a few draws of [`SplitMix64`] and one body built per input, once per batch, outside every
//! measured run.

use geom_make::{BoxInput, Made, PrismInput, make_box, prism};
use geom_math::{Axis, Box3, Rotation, TypedTransform};
use geom_num::grid::{P3, Steps};
use geom_num::{Exactness, GridStep};
use geom_op::{OpContext, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::placed::{Definition, Placed, Placement};
use geom_topo::{Arena, query};

use super::{Class, batch_of, p3, rng, within};
use crate::rng::SplitMix64;

/// The reach of a definition's corner and of a placement's shift, in grid steps.
const REACH: i64 = 1 << 20;

/// The class and bound a turned body is accepted at: certified within 64 steps.
pub const ACCEPT: Exactness = Exactness::Certified {
    bound: Steps(64),
    step: GridStep::Q0,
};

/// The directions of the turns, each a cosine and sine over a whole hypotenuse.
const TURNS: [(i32, i32); 8] = [
    (3, 4),
    (4, 3),
    (5, 12),
    (12, 5),
    (8, 15),
    (-4, 3),
    (7, 24),
    (-20, 21),
];

/// A definition placed exactly.
#[derive(Clone, Debug)]
pub struct Instance {
    /// The definition, in its own frame.
    pub shared: OpOutput<Made>,
    /// Its placement.
    pub placed: Placed,
}

/// A definition and the turn that places it.
#[derive(Clone, Debug)]
pub struct Turn {
    /// The definition, in its own frame.
    pub shared: OpOutput<Made>,
    /// The rotation about z.
    pub turn: TypedTransform,
}

/// A size in steps, at least 4, so a box and a prism have points strictly inside.
fn size(g: &mut SplitMix64) -> i64 {
    g.range(4, 1 << 16)
}

/// The `n`-th definition: a box for even `n`, a prism on a right triangle for odd, and a point
/// strictly inside it.
fn definition(g: &mut SplitMix64, n: usize) -> Option<(OpOutput<Made>, P3)> {
    let ctx = OpContext::new(OpPath::new(OpId::from_bytes([1; 16])), Budget::UNLIMITED);
    let (x, y, z) = (within(g, REACH), within(g, REACH), within(g, REACH));
    let (a, b, h) = (size(g), size(g), size(g));
    let steps = |v: i64| Steps(v.unsigned_abs());
    if n.is_multiple_of(2) {
        let input = BoxInput {
            min: p3(x, y, z),
            size: [steps(a), steps(b), steps(h)],
        };
        let inside = p3(x + a / 2, y + b / 2, z + h / 2);
        return Some((make_box(&ctx, &input).ok()?, inside));
    }
    let input = PrismInput {
        base: vec![p3(x, y, z), p3(x + a, y, z), p3(x, y + b, z)],
        height: steps(h),
    };
    // A quarter of each leg in: a / 4 · b + b / 4 · a is at most half of a · b, inside the
    // hypotenuse.
    let inside = p3(x + a / 4, y + b / 4, z + h / 2);
    Some((prism(&ctx, &input).ok()?, inside))
}

/// The box of a body's vertices.
fn bound(made: &OpOutput<Made>) -> Option<Box3> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    Some(query::bounding_box(&made.result.body, &mut meter).ok()??.0)
}

/// An exact placement of a definition drawn from `g`, as the `n`-th input, and a point strictly
/// inside it, where the placement takes it.
fn placed(g: &mut SplitMix64, n: usize) -> Option<(Instance, P3)> {
    let (shared, inside) = definition(g, n)?;
    let shift = [within(g, REACH), within(g, REACH), within(g, REACH)];
    let axes = u8::try_from(g.below(48)).ok()?;
    let t = Placement::Exact { shift, axes }.grid().ok()?;
    let index = u32::try_from(n).ok()?;
    let placed = Placed::exact(Definition(index), &t, &bound(&shared)?).ok()?;
    let inside = t.apply(inside).ok()?;
    Some((Instance { shared, placed }, inside))
}

/// Instances and a face of each, for `query.placed_area`.
#[must_use]
pub fn areas(class: Class) -> Vec<(Instance, u32)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("placed::areas", class);
    batch_of(|n| {
        let (instance, _) = placed(&mut g, n)?;
        let rows = u64::from(instance.shared.result.body.arenas().rows(Arena::Face));
        let face = u32::try_from(g.below(rows)).ok()?;
        Some((instance, face))
    })
}

/// Instances and a point of each, for `query.placed_side`: inside on half of them, and on the
/// other half past the instance's box by 1 to 1,000 steps along one axis, so never on its boundary.
#[must_use]
pub fn sides(class: Class) -> Vec<(Instance, P3)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("placed::sides", class);
    batch_of(|n| {
        let (instance, inside) = placed(&mut g, n)?;
        if g.below(2) == 0 {
            return Some((instance, inside));
        }
        let b = *instance.placed.bound();
        let gap = g.range(1, 1000);
        let (lo, hi) = (b.min(), b.max());
        let outside = match g.below(6) {
            0 => p3(lo.x.get() - gap, inside.y.get(), inside.z.get()),
            1 => p3(hi.x.get() + gap, inside.y.get(), inside.z.get()),
            2 => p3(inside.x.get(), lo.y.get() - gap, inside.z.get()),
            3 => p3(inside.x.get(), hi.y.get() + gap, inside.z.get()),
            4 => p3(inside.x.get(), inside.y.get(), lo.z.get() - gap),
            _ => p3(inside.x.get(), inside.y.get(), hi.z.get() + gap),
        };
        Some((instance, outside))
    })
}

/// Definitions and a turn of each, for `make.materialise`.
#[must_use]
pub fn turns(class: Class) -> Vec<Turn> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("placed::turns", class);
    batch_of(|n| {
        let (shared, _) = definition(&mut g, n)?;
        let pick = usize::try_from(g.below(8)).ok()?;
        let (c, s) = *TURNS.get(pick)?;
        let turn = TypedTransform::rotation(Axis::Z, Rotation::direction(c, s).ok()?).ok()?;
        Some(Turn { shared, turn })
    })
}
