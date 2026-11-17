//! Fuzz target: `geom-bool`'s boolean pipeline on small solids agrees with counted cells, and
//! every result passes both checkers.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: two to [`MAX_BOXES`] boxes on
//! a lattice of [`SIDE`] cells a side, where shared faces, edges and corners are the rule, scaled
//! by one of [`SCALES`], with a level for a half-space, a row for a rod, a move for one tool and
//! two cells for seeds. [`check`] counts the cells the boxes cover, which is the truth here, and
//! holds to it each stage of the pipeline: the operands, selections and schema, half-spaces and
//! wires (G130), the n-ary entry (G131), the names and history (G132), the arrangement certificate
//! (G133), the canonical rebuild (G129's `rebuild.rs`), cells, rooms and the narrowest passage
//! (G137), batch cutting (G138), the arrangement cache and the re-cut after a move (G139), and the
//! arrangement step of local operations (G144). [`harness`] is `decode` then `check`, the function
//! every fuzzing engine drives (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **Valid by construction.** Every operand is a box `geom-make` builds, a half-space through
//!   three grid points not on a line, or a wire of two grid points; every seed is the middle of a
//!   cell of the room, on no face. So every refusal is a [`Finding`], except the one the fast path
//!   states (`InvalidInput` for operands its proof does not cover), which is held to its law.
//! - **A broken law is a [`Finding`]** naming the law and the case. A panic fails every leg.
//!
//! Method: chapter 11's laws (`projects/engines/geometry/architecture/11-booleans.md`) and 13.1's
//! fast path (`13-sweeps-blends-offsets.md`), by counting, the exact-volume pattern of
//! `geom-bool`'s own `tests/laws.rs`. Each box is a set of whole cells, so what covers a cell is a
//! matter of three comparisons, written here without the kernel's help. A selection has the volume
//! of the cells it keeps; a half-space keeps the cells below its level; a rod keeps the length of
//! the covered cells it runs through; a room's cells are the uncovered cells of the envelope, and
//! two seeds share a cell exactly when a walk through faces of uncovered cells joins them; a batch
//! cut has the cells of its host and no tool, each tool cuts exactly when it shares a cell with
//! the host, many hosts cut as each alone, and a re-cut from the last cut's cache is the fresh cut
//! bit for bit; a local step has its selection's cells, and its fast path is the full path or its
//! stated refusal. Every body any of them returns passes `geom-check`'s full check and the second
//! checker of 28.4 (`geom_test::agree::judge`), which agree and leave nothing unjudged, and every
//! solid passes the arrangement certificate.
//!
//! Cost: at most [`MAX_BOXES`] boxes and four more operands, in about twelve arrangements of at
//! most seven operands each, every result judged by both checkers.

use geom_bool::batch::{BatchRule, Cut, cut};
use geom_bool::certificate::certify;
use geom_bool::combine::{Combined, combine};
use geom_bool::half_space::half_space;
use geom_bool::local::{LocalInput, Plan, local_step};
use geom_bool::operand::{
    BooleanInput, CellSeed, Operand, OperandKey, OperandKind, Selection, Term,
};
use geom_bool::rebuild::wire;
use geom_make::{BoxInput, make_box};
use geom_num::plane::{Plane, Tpi};
use geom_num::wide::I256;
use geom_num::{Exactness, P3, Steps};
use geom_op::local::AllowSet;
use geom_op::{ErrorKind, OpContext, OpError, OpId, OpPath};
use geom_poly::merge::{Curves, Merge};
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::agree::judge;
use geom_test::{ByteSource, Entropy};
use geom_topo::query::volume;
use geom_topo::{Body, BodyKind, PointKind};

use crate::Finding;

/// The most boxes a case holds.
pub const MAX_BOXES: u8 = 3;

/// Cells to a side of the lattice the boxes and the room lie on.
pub const SIDE: u8 = 4;

/// What the lattice is scaled by: small and even, so that a cell has a middle on the grid, and one
/// that takes the lattice near the edge of the frame.
pub const SCALES: [i64; 3] = [2, 6, 1 << 36];

/// The most bytes [`decode`] reads from any input: two for the scale and the count, six for each
/// box, two for the half-space, three for the rod, one for the move and two for the seeds.
pub const MAX_INPUT_BYTES: usize = 28;

/// The lowest cell a moved tool can reach, and one past the highest: the lattice and one cell
/// round it.
const REACH: (i64, i64) = (-1, 5);

/// A box, as its low cell and one past its high cell on each axis.
pub type Block = ([i64; 3], [i64; 3]);

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The boxes, two or more.
    pub boxes: Vec<Block>,
    /// Steps to a cell.
    pub scale: i64,
    /// The half-space keeps every cell whose coordinate on `axis` is below `level`.
    pub cut: (usize, i64),
    /// The rod runs along `axis` through the middles of the cells at the other two coordinates.
    pub rod: (usize, [i64; 2]),
    /// The tool moved for the re-cut, as a place among the tools, the axis and the step (±1).
    pub moved: (usize, usize, i64),
    /// Two picks among the room's cells, for its seeds.
    pub seeds: [u8; 2],
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "cells below [`SIDE`] + 1 add nothing near the range of `i64`"
)]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let [first, ..] = SCALES;
    let scale = SCALES
        .get(usize::from(src.byte()) % SCALES.len())
        .copied()
        .unwrap_or(first);
    let count = 2 + src.byte() % (MAX_BOXES - 1);
    let mut boxes = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let mut low = [0; 3];
        let mut high = [0; 3];
        for (l, h) in low.iter_mut().zip(&mut high) {
            let (a, b) = (src.byte() % SIDE, src.byte() % SIDE);
            *l = i64::from(a.min(b));
            *h = i64::from(a.max(b)) + 1;
        }
        boxes.push((low, high));
    }
    let cut = (
        usize::from(src.byte() % 3),
        1 + i64::from(src.byte() % (SIDE - 1)),
    );
    let rod = (
        usize::from(src.byte() % 3),
        [i64::from(src.byte() % SIDE), i64::from(src.byte() % SIDE)],
    );
    let tools = usize::from(count - 1);
    let pick = src.byte();
    let moved = (
        usize::from(pick) % tools,
        usize::from(pick / 16 % 3),
        if pick & 8 == 0 { 1 } else { -1 },
    );
    let seeds = [src.byte(), src.byte()];
    let read = data.len().saturating_sub(src.remaining());
    let case = Case {
        boxes,
        scale,
        cut,
        rod,
        moved,
        seeds,
    };
    (case, read)
}

/// A meter no case reaches.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The context of operation `n`: each operand is made by its own operation, so each brings names
/// of its own.
fn ctx(n: u8) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([n; 16])), Budget::UNLIMITED)
}

/// The key of operand `n`.
const fn key(n: u8) -> OperandKey {
    OperandKey([n; 16])
}

/// The operation every boolean of a case runs as.
const STEP: u8 = 0x70;

/// The half-space's key, the rod's, the envelope's and the first seed's.
const HALF: u8 = 0x20;
const ROD: u8 = 0x30;
const ENVELOPE: u8 = 0x40;
const SEED: u8 = 0x50;

fn at(x: i64, y: i64, z: i64) -> Result<P3, Finding> {
    P3::new(x, y, z).map_err(|e| Finding::new("a case fits the frame", || format!("{e:?}")))
}

/// A refusal of a valid case, as a [`Finding`].
fn refused<'a>(law: &'static str, case: &'a Case) -> impl Fn(OpError) -> Finding + 'a {
    move |e| Finding::new(law, || format!("{case:?}: {e:?}"))
}

/// The box `block`, scaled, made by operation `maker`.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "cells are within [`REACH`] and the scale at most 2^36, so a corner is below 2^40"
)]
fn made(block: &Block, scale: i64, maker: u8, case: &Case) -> Result<Body, Finding> {
    let (low, high) = block;
    let [left, front, bottom] = low.map(|cell| cell * scale);
    let mut size = [Steps(0); 3];
    for ((side, from), to) in size.iter_mut().zip(low).zip(high) {
        *side = Steps(u64::try_from((to - from) * scale).unwrap_or(0));
    }
    let input = BoxInput {
        min: at(left, front, bottom)?,
        size,
    };
    make_box(&ctx(maker), &input)
        .map(|out| out.result.body)
        .map_err(refused("a box of whole cells is made", case))
}

/// A solid operand.
const fn solid(n: u8, body: &Body) -> Operand<'_> {
    Operand {
        key: key(n),
        body,
        kind: OperandKind::Solid,
        exactness: Exactness::Exact,
    }
}

/// Whether box `b` covers cell `c`.
fn covers(b: &Block, c: [i64; 3]) -> bool {
    let (low, high) = b;
    low.iter()
        .zip(high)
        .zip(c)
        .all(|((l, h), v)| *l <= v && v < *h)
}

/// Every cell a moved tool can reach.
fn every_cell() -> Vec<[i64; 3]> {
    let (low, high) = REACH;
    let mut out = Vec::new();
    for x in low..high {
        for y in low..high {
            for z in low..high {
                out.push([x, y, z]);
            }
        }
    }
    out
}

/// How many cells `keep` keeps, given which of `boxes` cover each.
fn count(boxes: &[Block], keep: impl Fn(&[bool], [i64; 3]) -> bool) -> usize {
    every_cell()
        .into_iter()
        .filter(|c| {
            let held: Vec<bool> = boxes.iter().map(|b| covers(b, *c)).collect();
            keep(&held, *c)
        })
        .count()
}

/// Six times `cells` cells' volume, as `volume` gives it.
fn six_times(cells: usize, scale: i64) -> Option<I256> {
    let side = I256::from_i128(i128::from(scale));
    let n = i128::try_from(cells).ok()?.checked_mul(6)?;
    I256::from_i128(n)
        .checked_mul(&side)?
        .checked_mul(&side)?
        .checked_mul(&side)
}

/// `body` passes both checkers, which agree and leave nothing unjudged, and a solid passes the
/// arrangement certificate.
fn judged(law: &str, body: &Body, case: &Case) -> Result<(), Finding> {
    let verdict = judge(body, Exactness::Exact)
        .map_err(|e| Finding::new(law, || format!("{case:?}: stopped: {e:?}")))?;
    if !verdict.passed() || !verdict.agree() || verdict.unchecked != 0 || verdict.unjudged != 0 {
        return Err(Finding::new(law, || format!("{case:?}: {verdict:?}")));
    }
    if body.kind() == BodyKind::Solid {
        certify(body, &mut meter())
            .map_err(|e| Finding::new("a solid result certifies", || format!("{case:?}: {e:?}")))?;
    }
    Ok(())
}

/// The bodies of a selection's results.
fn bodies_of(list: &[Combined]) -> Vec<&Body> {
    list.iter().map(|result| &result.body).collect()
}

/// Each of `list` judged, and, when every one is a solid or empty, `count` cells between them. A
/// general body with faces, whose volume the topology does not give, is held to the checkers
/// alone, as `geom-bool`'s laws hold it.
fn holds_cells(law: &str, list: &[&Body], count: usize, case: &Case) -> Result<(), Finding> {
    let mut sum = I256::from_i128(0);
    let mut measured = true;
    for body in list {
        judged("every result passes both checkers", body, case)?;
        match body.kind() {
            BodyKind::Solid => {
                let six = volume(body, &mut meter())
                    .map_err(|e| Finding::new(law, || format!("{case:?}: {e:?}")))?;
                sum = sum
                    .checked_add(&six)
                    .ok_or_else(|| Finding::new(law, || format!("{case:?}: overflow")))?;
            }
            _ if body.arenas().faces.is_empty() => {}
            _ => measured = false,
        }
    }
    if measured && six_times(count, case.scale) != Some(sum) {
        return Err(Finding::new(law, || {
            format!("{case:?}: six times the volume is {sum:?} for {count} cells")
        }));
    }
    Ok(())
}

/// `selections` of `operands` in one arrangement.
fn combined(
    operands: Vec<Operand<'_>>,
    select: Vec<Selection>,
    case: &Case,
) -> Result<(Vec<Vec<Combined>>, Vec<String>), Finding> {
    let input = BooleanInput {
        operands,
        select,
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let out = combine(&ctx(STEP), &Exec::serial(), &input)
        .map_err(refused("a boolean of valid operands holds", case))?;
    let notes = out.diagnostics.iter().map(|d| d.code.to_owned()).collect();
    Ok((out.result, notes))
}

/// One result list for each selection, or a [`Finding`].
fn lists<const N: usize>(
    got: Vec<Vec<Combined>>,
    case: &Case,
) -> Result<[Vec<Combined>; N], Finding> {
    got.try_into()
        .map_err(|_| Finding::new("one list per selection", || format!("{case:?}")))
}

/// The selections of the boxes against their cells: one arrangement, every selection's volume,
/// the history of named operands, and the same results in any order of the operands.
fn check_selections(case: &Case, bodies: &[Body]) -> Result<(), Finding> {
    let operands: Vec<Operand<'_>> = bodies.iter().zip(1..).map(|(b, n)| solid(n, b)).collect();
    let select = vec![
        Selection::Unite,
        Selection::Intersect,
        Selection::Subtract { target: key(1) },
        Selection::Where(vec![Term::AtLeast(2)]),
        Selection::GeneralFuse,
    ];
    let (got, _) = combined(operands.clone(), select.clone(), case)?;
    let mut reversed = operands;
    reversed.reverse();
    let (again, _) = combined(reversed, select, case)?;
    if again != got {
        return Err(Finding::new(
            "the order of the operands changes nothing",
            || format!("{case:?}"),
        ));
    }
    let [unite, meet, less, two, fused] = lists::<5>(got, case)?;
    let any = count(&case.boxes, |held, _| held.iter().any(|h| *h));
    holds_cells("a union has its cells", &bodies_of(&unite), any, case)?;
    let all = count(&case.boxes, |held, _| held.iter().all(|h| *h));
    holds_cells(
        "an intersection has its cells",
        &bodies_of(&meet),
        all,
        case,
    )?;
    let only = count(
        &case.boxes,
        |held, _| matches!(held, [true, rest @ ..] if !rest.iter().any(|h| *h)),
    );
    holds_cells("a difference has its cells", &bodies_of(&less), only, case)?;
    let twice = count(&case.boxes, |held, _| {
        held.iter().filter(|h| **h).count() >= 2
    });
    holds_cells(
        "what two cover has its cells",
        &bodies_of(&two),
        twice,
        case,
    )?;
    for result in &fused {
        judged("a general fuse passes both checkers", &result.body, case)?;
    }
    for result in unite.iter().chain(&meet).chain(&less).chain(&two) {
        if result.history.is_none() {
            return Err(Finding::new(
                "a result of named operands has its history",
                || format!("{case:?}"),
            ));
        }
    }
    Ok(())
}

/// A plane square to `axis` at `at`, its normal along +`axis`.
fn square(axis: usize, at: i64) -> Result<Plane, Finding> {
    let mut n = [0; 3];
    if let Some(v) = n.get_mut(axis) {
        *v = 1;
    }
    Plane::new(n, i128::from(at).saturating_neg())
        .map_err(|e| Finding::new("a square plane is a plane", || format!("{e:?}")))
}

/// The point `along` on `axis` and `across` on the other two axes in order.
fn placed(axis: usize, along: i64, across: [i64; 2]) -> [i64; 3] {
    let [u, v] = across;
    match axis {
        0 => [along, u, v],
        1 => [u, along, v],
        _ => [u, v, along],
    }
}

/// The rod of `case`: one straight edge along its axis through the middles of its row's cells,
/// from a cell before the lattice to a cell after it.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "cells are below [`SIDE`] + 1 and the scale at most 2^36"
)]
fn rod(case: &Case) -> Result<Body, Finding> {
    let (axis, row) = case.rod;
    let half = case.scale / 2;
    let across = row.map(|cell| cell * case.scale + half);
    let ends = [-case.scale, (i64::from(SIDE) + 1) * case.scale];
    let mut vertices = Vec::with_capacity(2);
    for along in ends {
        let [x, y, z] = placed(axis, along, across);
        let tpi = Tpi::new(square(0, x)?, square(1, y)?, square(2, z)?)
            .map_err(|e| Finding::new("three square planes meet", || format!("{e:?}")))?;
        vertices.push(tpi);
    }
    let curves = Curves {
        vertices,
        edges: vec![(0, 1)],
        meets: vec![Vec::new()],
    };
    wire(&curves, None, &mut meter())
        .map_err(|e| Finding::new("a rod is a wire", || format!("{case:?}: {e:?}")))
}

/// The length along `axis` of every edge of `list`'s wires, each between grid points.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "coordinates are below 2^40, and a wire's edges are at most a few"
)]
fn length(list: &[Combined], axis: usize, case: &Case) -> Result<i64, Finding> {
    let missing = || Finding::new("a kept rod's ends are grid points", || format!("{case:?}"));
    let mut sum = 0;
    for result in list {
        let arenas = result.body.arenas();
        let point = |vertex| -> Result<[i64; 3], Finding> {
            let place = arenas.vertices.point.get(vertex).ok_or_else(missing)?;
            match arenas.points.get(*place) {
                Some(PointKind::Grid(g)) => Ok([g.x.get(), g.y.get(), g.z.get()]),
                _ => Err(missing()),
            }
        };
        for (start, end) in arenas.edges.start.iter().zip(arenas.edges.end.iter()) {
            let (Some(s), Some(e)) = (*start, *end) else {
                return Err(missing());
            };
            let (s, e) = (point(s)?, point(e)?);
            let (Some(s), Some(e)) = (s.get(axis), e.get(axis)) else {
                return Err(missing());
            };
            sum += (e - s).abs();
        }
    }
    Ok(sum)
}

/// A half-space below a level, and a rod kept where the boxes are, in one arrangement.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_wrap,
    reason = "cells and lengths are below 2^40; a count of cells is at most 216"
)]
fn check_half_and_rod(case: &Case, bodies: &[Body]) -> Result<(), Finding> {
    let (axis, level) = case.cut;
    let height = level * case.scale;
    let corner = |along: i64, across: [i64; 2]| {
        let [x, y, z] = placed(axis, along, across);
        at(x, y, z)
    };
    // (b − a) × (c − a) is +axis for these three, so the material is below the level.
    let points = [
        corner(height, [0, 0])?,
        match axis {
            1 => corner(height, [0, 1])?,
            _ => corner(height, [1, 0])?,
        },
        match axis {
            1 => corner(height, [1, 0])?,
            _ => corner(height, [0, 1])?,
        },
    ];
    let space = half_space(
        &OpPath::new(OpId::from_bytes([HALF; 16])),
        points,
        &mut meter(),
    )
    .map_err(|e| Finding::new("a half-space is made", || format!("{case:?}: {e:?}")))?;
    let wire_body = rod(case)?;
    let mut operands: Vec<Operand<'_>> = bodies.iter().zip(1..).map(|(b, n)| solid(n, b)).collect();
    // Inside any box: the first, then each other or'd in, in postfix.
    let mut any = Vec::new();
    for (_, n) in bodies.iter().zip(1..) {
        any.push(Term::Inside(key(n)));
        if n > 1 {
            any.push(Term::Or);
        }
    }
    operands.push(solid(HALF, &space));
    operands.push(Operand {
        kind: OperandKind::Cutter,
        ..solid(ROD, &wire_body)
    });
    let mut below = any.clone();
    below.extend([Term::Inside(key(HALF)), Term::And]);
    let select = vec![
        Selection::Where(below),
        Selection::Keep {
            target: key(ROD),
            test: any,
        },
    ];
    let (got, _) = combined(operands, select, case)?;
    let [clipped, kept] = lists::<2>(got, case)?;
    let under = count(&case.boxes, |held, c| {
        held.iter().any(|h| *h) && c.get(axis).is_some_and(|v| *v < level)
    });
    holds_cells(
        "a half-space keeps the cells below it",
        &bodies_of(&clipped),
        under,
        case,
    )?;
    for result in &kept {
        judged("a kept rod passes both checkers", &result.body, case)?;
    }
    let (rod_axis, across) = case.rod;
    let covered = (0..i64::from(SIDE))
        .filter(|along| {
            let c = placed(rod_axis, *along, across);
            case.boxes.iter().any(|b| covers(b, c))
        })
        .count();
    let want = covered as i64 * case.scale;
    let got = length(&kept, rod_axis, case)?;
    if got != want {
        return Err(Finding::new(
            "a rod keeps the length of the covered cells it runs through",
            || format!("{case:?}: {got} for {want}"),
        ));
    }
    Ok(())
}

/// The face-connected and the touching groups of the room's cells, as a group number per cell.
fn groups(rooms: &[[i64; 3]], touching: bool) -> Vec<usize> {
    let near = |a: [i64; 3], b: [i64; 3]| {
        let gaps = a.iter().zip(b).map(|(x, y)| x.abs_diff(y));
        let (most, sum) = gaps.fold((0, 0_u64), |(m, s), g| (m.max(g), s.saturating_add(g)));
        if touching { most == 1 } else { sum == 1 }
    };
    let mut group: Vec<Option<usize>> = vec![None; rooms.len()];
    let mut next = 0;
    for start in 0..rooms.len() {
        if group.get(start).copied().flatten().is_some() {
            continue;
        }
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            match group.get_mut(i) {
                Some(slot @ None) => *slot = Some(next),
                _ => continue,
            }
            let Some(here) = rooms.get(i).copied() else {
                continue;
            };
            for (j, there) in rooms.iter().enumerate() {
                if group.get(j).copied().flatten().is_none() && near(here, *there) {
                    stack.push(j);
                }
            }
        }
        next = next.saturating_add(1);
    }
    group.into_iter().map(|g| g.unwrap_or(usize::MAX)).collect()
}

/// The rooms of the envelope walled by the boxes: every cell of the room in all, and the cells
/// that hold the seeds, one when a walk through faces joins them, with the leak reported.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "cells are below [`SIDE`] and the scale at most 2^36"
)]
fn check_rooms(case: &Case, bodies: &[Body]) -> Result<(), Finding> {
    let span = i64::from(SIDE);
    let whole = ([0, 0, 0], [span, span, span]);
    let rooms: Vec<[i64; 3]> = every_cell()
        .into_iter()
        .filter(|c| covers(&whole, *c) && !case.boxes.iter().any(|b| covers(b, *c)))
        .collect();
    let picks: Vec<usize> = case.seeds.iter().map(|s| usize::from(*s)).collect();
    let (Some(&one), Some(&two)) = (picks.first(), picks.get(1)) else {
        return Ok(());
    };
    let total = rooms.len();
    if total == 0 {
        return Ok(());
    }
    let (first, second) = (one % total, two % total);
    let envelope = made(&whole, case.scale, ENVELOPE, case)?;
    let mut operands = vec![solid(ENVELOPE, &envelope)];
    let mut test = vec![Term::Inside(key(ENVELOPE))];
    for (body, wall) in bodies.iter().zip(1..) {
        operands.push(solid(wall, body));
        test.extend([Term::Inside(key(wall)), Term::Not, Term::And]);
    }
    let half = case.scale / 2;
    let seed = |place: usize, name: u8| -> Result<CellSeed, Finding> {
        let cell = rooms.get(place).copied().unwrap_or_default();
        let [x, y, z] = cell.map(|v| v * case.scale + half);
        Ok(CellSeed {
            point: at(x, y, z)?,
            key: key(name),
        })
    };
    let mut seeds = vec![seed(first, SEED)?];
    if second != first {
        seeds.push(seed(second, SEED + 1)?);
    }
    let select = vec![
        Selection::Cells {
            test: test.clone(),
            seeds: Vec::new(),
        },
        Selection::Cells {
            test,
            seeds: seeds.clone(),
        },
    ];
    let (got, notes) = combined(operands, select, case)?;
    let [every, held] = lists::<2>(got, case)?;
    holds_cells(
        "the rooms have the room's cells",
        &bodies_of(&every),
        total,
        case,
    )?;
    for room in &every {
        let inside = room.cell.as_ref().map(|r| r.elements.clone());
        if inside != Some(vec![key(ENVELOPE)]) {
            return Err(Finding::new(
                "a room lies inside the envelope alone",
                || format!("{case:?}: {inside:?}"),
            ));
        }
    }
    check_seeds(case, &rooms, (first, second), &seeds, &held, &notes)
}

/// The rooms that hold `seeds`, at `first` and `second` among `rooms`: one when a walk through faces
/// joins them, with the leak reported, two when their groups do not even touch.
fn check_seeds(
    case: &Case,
    rooms: &[[i64; 3]],
    (first, second): (usize, usize),
    seeds: &[CellSeed],
    held: &[Combined],
    notes: &[String],
) -> Result<(), Finding> {
    let total = rooms.len();
    let faces = groups(rooms, false);
    let touch = groups(rooms, true);
    let group = |of: &[usize], place: usize| of.get(place).copied().unwrap_or(usize::MAX);
    let leak = notes.iter().any(|c| c == "bool.seeds_share_cell");
    let cells_of = |place: usize| {
        let by_face = group(&faces, place);
        let by_touch = group(&touch, place);
        let clean = (0..total)
            .all(|other| (group(&faces, other) == by_face) == (group(&touch, other) == by_touch));
        clean.then(|| {
            (0..total)
                .filter(|other| group(&faces, *other) == by_face)
                .count()
        })
    };
    // Seeds a walk through faces joins lie in one cell; seeds in groups that do not even touch lie
    // in two. Groups that touch only at an edge or a corner are held to the checkers alone.
    let one_cell = second == first || group(&faces, first) == group(&faces, second);
    let apart = group(&touch, first) != group(&touch, second);
    let wrong = |what: &'static str| Err(Finding::new(what, || format!("{case:?}: {notes:?}")));
    if one_cell {
        let both: Vec<OperandKey> = seeds.iter().map(|s| s.key).collect();
        let [room] = held else {
            return wrong("seeds a walk joins share one cell");
        };
        if room.cell.as_ref().map(|r| r.keys.clone()) != Some(both) {
            return wrong("a cell holds the keys of its seeds");
        }
        if leak != (second != first) {
            return wrong("two seeds in one cell are a leak, reported");
        }
        if let Some(cells) = cells_of(first) {
            holds_cells("a seeded room has its cells", &[&room.body], cells, case)?;
        }
    } else if apart {
        if leak || held.len() != 2 {
            return wrong("seeds no walk joins lie in two cells");
        }
        for room in held {
            let keys = room.cell.as_ref().map(|r| r.keys.clone());
            let place = if keys == Some(vec![key(SEED)]) {
                first
            } else if keys == Some(vec![key(SEED.saturating_add(1))]) {
                second
            } else {
                return wrong("a cell holds the key of its seed");
            };
            if let Some(cells) = cells_of(place) {
                holds_cells("a seeded room has its cells", &[&room.body], cells, case)?;
            }
        }
    }
    for room in held {
        judged("a room passes both checkers", &room.body, case)?;
    }
    Ok(())
}

/// The host's cut, for `tools` given by their places among the boxes.
fn host_cut<'b>(
    host: (u8, &'b Body),
    tools: &[(u8, &'b Body)],
    reuse: Option<&'b geom_bool::batch::Recut>,
) -> Cut<'b> {
    Cut {
        host: solid(host.0, host.1),
        tools: tools.iter().map(|(n, b)| solid(*n, b)).collect(),
        reuse,
    }
}

/// Batch cuts: the host's cells less the tools', each tool's report, many hosts as each alone, and
/// a re-cut after a move from the last cut's cache as the fresh cut.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "a moved cell is within [`REACH`]"
)]
fn check_batch(case: &Case, bodies: &[Body]) -> Result<(), Finding> {
    let named: Vec<(u8, &Body)> = (1..).zip(bodies).collect();
    let [host, tools @ ..] = named.as_slice() else {
        return Ok(());
    };
    let run = |cuts: &[Cut<'_>]| {
        cut(&ctx(STEP), &Exec::serial(), cuts, BatchRule::Atomic)
            .map_err(refused("a batch cut of valid operands holds", case))
    };
    let first = run(&[host_cut(*host, tools, None)])?;
    let [one] = first.result.as_slice() else {
        return Err(Finding::new("one host gives one result", || {
            format!("{case:?}")
        }));
    };
    let only = count(
        &case.boxes,
        |held, _| matches!(held, [true, rest @ ..] if !rest.iter().any(|h| *h)),
    );
    holds_cells(
        "a cut host has its cells less its tools'",
        &[&one.result.body],
        only,
        case,
    )?;
    let [hosted, rest @ ..] = case.boxes.as_slice() else {
        return Ok(());
    };
    for (report, (tool, (n, _))) in one.tools.iter().zip(rest.iter().zip(tools)) {
        let shared = every_cell()
            .into_iter()
            .any(|c| covers(hosted, c) && covers(tool, c));
        if report.key != key(*n) || report.cut != shared {
            return Err(Finding::new(
                "a tool cuts exactly when it shares a cell with the host",
                || format!("{case:?}: {:?}", one.tools),
            ));
        }
    }
    if one.tools.len() != tools.len() || !one.skipped.is_empty() {
        return Err(Finding::new("every tool enters and is reported", || {
            format!("{case:?}: {:?}", one.tools)
        }));
    }
    // Two hosts in one call: each as it is alone.
    if let [second, others @ ..] = tools {
        let mut theirs = vec![*host];
        theirs.extend(others.iter().copied());
        let alone = run(&[host_cut(*second, &theirs, None)])?;
        let both = run(&[
            host_cut(*host, tools, None),
            host_cut(*second, &theirs, None),
        ])?;
        let mut singles = first.result.clone();
        singles.extend(alone.result);
        if both.result != singles {
            return Err(Finding::new("many hosts cut as each alone", || {
                format!("{case:?}")
            }));
        }
    }
    // One tool moved a cell: from the last cut's cache, the fresh cut, bit for bit.
    let (which, axis, step) = case.moved;
    let mut boxes = case.boxes.clone();
    let place = which + 1;
    if let Some((low, high)) = boxes.get_mut(place) {
        if let (Some(l), Some(h)) = (low.get_mut(axis), high.get_mut(axis)) {
            *l += step;
            *h += step;
        }
    }
    let mut moved_bodies = Vec::with_capacity(boxes.len());
    for (b, n) in boxes.iter().zip(1..) {
        moved_bodies.push(made(b, case.scale, n, case)?);
    }
    let named: Vec<(u8, &Body)> = (1..).zip(&moved_bodies).collect();
    let [host, tools @ ..] = named.as_slice() else {
        return Ok(());
    };
    let fresh = run(&[host_cut(*host, tools, None)])?;
    let again = run(&[host_cut(*host, tools, Some(&one.cache))])?;
    if again.result != fresh.result || again.diagnostics != fresh.diagnostics {
        return Err(Finding::new(
            "a re-cut from the last cut's cache is the fresh cut",
            || format!("{case:?}"),
        ));
    }
    let only = count(
        &boxes,
        |held, _| matches!(held, [true, rest @ ..] if !rest.iter().any(|h| *h)),
    );
    let cuts: Vec<&Body> = fresh.result.iter().map(|r| &r.result.body).collect();
    holds_cells("a moved cut has its cells", &cuts, only, case)
}

/// The arrangement step of a local operation: the first box with the second, full and fast.
fn check_local(case: &Case, bodies: &[Body]) -> Result<(), Finding> {
    let ([a, b, ..], [first, second, ..]) = (case.boxes.as_slice(), bodies) else {
        return Ok(());
    };
    let step = |plan: Plan, select: Selection| {
        let input = LocalInput {
            target: solid(1, first),
            tools: vec![solid(2, second)],
            select,
            merge: Merge::RemoveRedundant,
            accept: None,
            may: AllowSet::ALL,
            plan,
        };
        local_step(&ctx(STEP), &Exec::serial(), &input)
    };
    let full = step(Plan::Arrange, Selection::Unite)
        .map_err(refused("a local step of valid operands holds", case))?;
    let pair = [*a, *b];
    let any = count(&pair, |held, _| held.iter().any(|h| *h));
    holds_cells(
        "a local step has its cells",
        &[&full.result.body],
        any,
        case,
    )?;
    let less = step(Plan::Arrange, Selection::Subtract { target: key(1) })
        .map_err(refused("a local cut of valid operands holds", case))?;
    let only = count(&pair, |held, _| matches!(held, [true, false]));
    holds_cells(
        "a local cut has its cells",
        &[&less.result.body],
        only,
        case,
    )?;
    let apart = !every_cell()
        .into_iter()
        .any(|c| covers(a, c) && covers(b, c));
    if apart {
        match step(Plan::Glued, Selection::Unite) {
            Ok(fast) => {
                if fast.result.body != full.result.body
                    || fast.result.history != full.result.history
                {
                    return Err(Finding::new(
                        "the fast path gives the full path's body and history",
                        || format!("{case:?}"),
                    ));
                }
            }
            Err(e) if e.kind() == ErrorKind::InvalidInput => {}
            Err(e) => {
                return Err(Finding::new(
                    "the fast path gives the full path's body or refuses as stated",
                    || format!("{case:?}: {e:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// Holds the pipeline to the case's cells.
///
/// # Errors
///
/// A [`Finding`] naming the first law broken, or an error from a valid case.
pub fn check(case: &Case) -> Result<(), Finding> {
    let mut bodies = Vec::with_capacity(case.boxes.len());
    for (b, n) in case.boxes.iter().zip(1..) {
        bodies.push(made(b, case.scale, n, case)?);
    }
    check_selections(case, &bodies)?;
    check_half_and_rod(case, &bodies)?;
    check_rooms(case, &bodies)?;
    check_batch(case, &bodies)?;
    check_local(case, &bodies)
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
