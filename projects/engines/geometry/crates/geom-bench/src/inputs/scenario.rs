//! Inputs for the flat boolean's scenarios (piece G140; chapter 23.13, "Scenarios at reduced
//! scale"): whole jobs a building model gives `geom-bool`, each at about a tenth of its full size.
//!
//! - A wall and its doors, cut in one call: [`FULL_OPENINGS`] at full size, the 300 openings of
//!   chapter 10's batch rule and 11.17's gate.
//! - The same wall with one door moved along it, cut again from the first cut's arrangement cache:
//!   the door move of Spare Construct's 16 ms target (28.14).
//! - A slab and its holes, in rows: [`FULL_HOLES`], 11.6's 10,000 holes, square here, as the flat
//!   boolean takes them (round holes take the curved path, a later piece).
//! - Overlapping boxes in a grid, united in one call: [`FULL_BOXES`], 23.13's 1,000-operand union.
//! - A storey: an envelope, walls on a square grid and one seed in each room, read as cells:
//!   [`FULL_ROOMS`] rooms a side (G137's storey).
//!
//! Class `typical` only, and each batch holds one scenario, not [`super::BATCH`] inputs: a scenario
//! is a whole job, whose cost is what the gate guards, and 23.13 runs it once at about a tenth of
//! its size. [`reduced`] gives the size the gates run, [`full`] the size of the job itself.
//!
//! Guarantees: every scenario is the same on every target, every body in it is a valid solid made
//! by `geom-make`, and every result of a scenario is `Ok` (`tests/inputs.rs`).
//!
//! Method: chapter 23.13's scenarios at reduced scale
//! (`projects/engines/geometry/architecture/23-performance.md`), on the shapes of `geom-bool`'s own
//! tests (`tests/batch.rs`'s wall of doors, `tests/cells.rs`'s rooms), grown to a size, with no
//! randomness: a scenario is a fixed job, as Spare Construct's targets are (28.14).
//!
//! Cost: one box built per body, outside every measured run.

use geom_bool::batch::{BatchRule, Cut, cut};
use geom_bool::operand::{CellSeed, Operand, OperandKey, OperandKind, Selection};
use geom_make::{BoxInput, make_box};
use geom_num::Exactness;
use geom_num::grid::{P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_poly::subdivide::Solved;
use geom_run::{Budget, Exec};
use geom_topo::Body;

use super::{Class, p3};

/// Openings in a wall at full size (10.1, 11.17).
pub const FULL_OPENINGS: usize = 300;

/// Holes in a slab at full size (11.6), in rows of [`HOLES_PER_ROW`].
pub const FULL_HOLES: usize = 10_000;

/// Holes in each row of a slab.
pub const HOLES_PER_ROW: usize = 40;

/// Boxes united at full size (23.13).
pub const FULL_BOXES: usize = 1_000;

/// Boxes in each row of a union.
pub const BOXES_PER_ROW: usize = 10;

/// Rooms a side of a storey at full size.
pub const FULL_ROOMS: usize = 10;

/// A host and the tools that cut it.
#[derive(Clone, Debug)]
pub struct Host {
    /// The host.
    pub host: Body,
    /// The tools, in key order.
    pub tools: Vec<Body>,
}

/// A host whose tools moved after a cut, with what that cut solved.
#[derive(Clone, Debug)]
pub struct Recut {
    /// The host and its tools after the move.
    pub host: Host,
    /// The first cut's arrangement cache.
    pub cache: Solved,
}

/// A host and its tools, and how many of the selections [`selections`] lists to read from their
/// one arrangement.
#[derive(Clone, Debug)]
pub struct Picks {
    /// The host and its tools.
    pub host: Host,
    /// How many selections.
    pub count: usize,
}

/// Solids to unite.
#[derive(Clone, Debug)]
pub struct Many {
    /// The solids, in key order.
    pub bodies: Vec<Body>,
}

/// A storey: its envelope, its walls, and a point in each room.
#[derive(Clone, Debug)]
pub struct Storey {
    /// The envelope.
    pub envelope: Body,
    /// The walls, in key order.
    pub walls: Vec<Body>,
    /// One point in each room, by rows.
    pub seeds: Vec<P3>,
}

/// The sizes of the scenarios: openings, holes, boxes and rooms a side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes {
    /// Openings in the wall.
    pub openings: usize,
    /// Holes in the slab.
    pub holes: usize,
    /// Boxes united.
    pub boxes: usize,
    /// Rooms a side of the storey.
    pub rooms: usize,
}

/// The full sizes.
#[must_use]
pub const fn full() -> Sizes {
    Sizes {
        openings: FULL_OPENINGS,
        holes: FULL_HOLES,
        boxes: FULL_BOXES,
        rooms: FULL_ROOMS,
    }
}

/// The sizes the count gates run (23.13): a tenth of the openings, and a third of the rooms a
/// side, a ninth of the rooms; a hundredth of the holes and a fortieth of the boxes, which each
/// cost about what the wall's 30 openings do, so no scenario takes more than a few minutes under
/// Callgrind. At a tenth the slab took 3.6 s and the union 0.9 s natively (G140, on the owner's
/// workstation); they rise towards a tenth as batch cutting and the rebuild meet their targets.
#[must_use]
pub const fn reduced() -> Sizes {
    Sizes {
        openings: FULL_OPENINGS / 10,
        holes: FULL_HOLES / 100,
        boxes: FULL_BOXES / 40,
        rooms: FULL_ROOMS / 3,
    }
}

/// A context of one operation, under no limit.
fn context() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([3; 16])), Budget::UNLIMITED)
}

/// A whole number of steps, from a size this module chose.
fn steps(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The box from `low` of `size`, built by `geom-make`.
fn cuboid(low: [i64; 3], size: [i64; 3]) -> Option<Body> {
    let [x, y, z] = low;
    let input = BoxInput {
        min: p3(x, y, z),
        size: size.map(|s| Steps(s.unsigned_abs())),
    };
    Some(make_box(&context(), &input).ok()?.result.body)
}

/// A wall of `openings` doors, each 20 wide and 30 high, 10 apart.
fn wall_of(openings: usize) -> Option<Host> {
    let n = steps(openings);
    let host = cuboid([0, 0, 0], [10 + 30 * n, 10, 60])?;
    let tools = (0..n)
        .map(|i| cuboid([10 + 30 * i, -5, 10], [20, 20, 30]))
        .collect::<Option<Vec<Body>>>()?;
    Some(Host { host, tools })
}

/// The operand of `body`, keyed `n`.
#[must_use]
pub fn solid(n: usize, body: &Body) -> Operand<'_> {
    let mut key = [0u8; 16];
    for (k, byte) in key.iter_mut().rev().zip(n.to_be_bytes().iter().rev()) {
        *k = *byte;
    }
    Operand {
        key: OperandKey(key),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// The host keyed 0 and its tools keyed 1 and on, cut afresh or from `reuse`.
#[must_use]
pub fn cut_of<'h>(host: &'h Host, reuse: Option<&'h Solved>) -> Cut<'h> {
    Cut {
        host: solid(0, &host.host),
        tools: host
            .tools
            .iter()
            .enumerate()
            .map(|(i, tool)| solid(i + 1, tool))
            .collect(),
        reuse,
    }
}

/// A wall and its doors, `sizes.openings` of them, for `cut`.
#[must_use]
pub fn wall(class: Class, sizes: Sizes) -> Vec<Host> {
    if class != Class::Typical {
        return Vec::new();
    }
    wall_of(sizes.openings).into_iter().collect()
}

/// The wall and its doors, `sizes.openings` of them, read for `count` selections at once, for
/// `combine` (11.17's gate compares three with one).
#[must_use]
pub fn picks(class: Class, sizes: Sizes, count: usize) -> Vec<Picks> {
    wall(class, sizes)
        .into_iter()
        .map(|host| Picks { host, count })
        .collect()
}

/// The first `picks.count` of: the host less its tools, everything, and where all meet.
#[must_use]
pub fn selections(picks: &Picks) -> Vec<Selection> {
    let target = solid(0, &picks.host.host).key;
    [
        Selection::Subtract { target },
        Selection::Unite,
        Selection::Intersect,
    ]
    .into_iter()
    .take(picks.count)
    .collect()
}

/// The same wall cut once, then its middle door moved 5 steps along it, with the first cut's
/// cache, for `cut` with `reuse`.
#[must_use]
pub fn moved_door(class: Class, sizes: Sizes) -> Vec<Recut> {
    if class != Class::Typical {
        return Vec::new();
    }
    let recut = || {
        let mut host = wall_of(sizes.openings)?;
        let first = cut(
            &context(),
            &Exec::serial(),
            &[cut_of(&host, None)],
            BatchRule::Atomic,
        )
        .ok()?;
        let cache = first.result.into_iter().next()?.cache;
        let middle = sizes.openings / 2;
        let at = steps(middle);
        *host.tools.get_mut(middle)? = cuboid([10 + 30 * at + 5, -5, 10], [20, 20, 30])?;
        Some(Recut { host, cache })
    };
    recut().into_iter().collect()
}

/// A slab 20 thick and its square holes, 10 a side and 20 apart, `sizes.holes` of them in rows of
/// [`HOLES_PER_ROW`], for `cut`.
#[must_use]
pub fn slab(class: Class, sizes: Sizes) -> Vec<Host> {
    if class != Class::Typical {
        return Vec::new();
    }
    let make = || {
        let rows = steps(sizes.holes.div_ceil(HOLES_PER_ROW));
        let across = steps(HOLES_PER_ROW);
        let host = cuboid([0, 0, 0], [10 + 20 * across, 10 + 20 * rows, 20])?;
        let tools = (0..sizes.holes)
            .map(|k| {
                let (row, column) = (steps(k / HOLES_PER_ROW), steps(k % HOLES_PER_ROW));
                cuboid([10 + 20 * column, 10 + 20 * row, -5], [10, 10, 30])
            })
            .collect::<Option<Vec<Body>>>()?;
        Some(Host { host, tools })
    };
    make().into_iter().collect()
}

/// `sizes.boxes` boxes 30 a side, 20 apart in rows of [`BOXES_PER_ROW`], each overlapping its
/// neighbours, and every other one raised 5, for `unite`.
#[must_use]
pub fn boxes(class: Class, sizes: Sizes) -> Vec<Many> {
    if class != Class::Typical {
        return Vec::new();
    }
    let bodies = (0..sizes.boxes)
        .map(|k| {
            let (row, column) = (steps(k / BOXES_PER_ROW), steps(k % BOXES_PER_ROW));
            let up = 5 * ((row + column) % 2);
            cuboid([20 * column, 20 * row, up], [30, 30, 30])
        })
        .collect::<Option<Vec<Body>>>();
    bodies.map(|bodies| Many { bodies }).into_iter().collect()
}

/// A storey of `sizes.rooms` by `sizes.rooms` rooms, each 100 square inside walls 4 thick and 30
/// high, with a seed at each room's middle, for `combine`'s cells.
#[must_use]
pub fn storey(class: Class, sizes: Sizes) -> Vec<Storey> {
    if class != Class::Typical {
        return Vec::new();
    }
    let make = || {
        let n = steps(sizes.rooms);
        let side = 4 + 104 * n;
        let envelope = cuboid([0, 0, 0], [side, side, 30])?;
        let mut walls = Vec::new();
        for k in 0..=n {
            walls.push(cuboid([104 * k, 0, 0], [4, side, 30])?);
            walls.push(cuboid([0, 104 * k, 0], [side, 4, 30])?);
        }
        let seeds = (0..n)
            .flat_map(|row| (0..n).map(move |column| p3(54 + 104 * column, 54 + 104 * row, 15)))
            .collect();
        Some(Storey {
            envelope,
            walls,
            seeds,
        })
    };
    make().into_iter().collect()
}

/// The seeds of `storey`, each keyed by its room.
#[must_use]
pub fn seeds(storey: &Storey) -> Vec<CellSeed> {
    storey
        .seeds
        .iter()
        .enumerate()
        .map(|(k, point)| CellSeed {
            point: *point,
            key: solid(1_000_000 + k, &storey.envelope).key,
        })
        .collect()
}
