//! Fuzz target: `geom-topo`'s placed instances (G145), on any bytes, answer every query on an exact
//! placement as the same query on its materialisation, carry a point across a placement and back,
//! answer `General` for a general placement and give its re-validated body's answers through
//! `geom_make::placed::Library`, keep the record within 88 bytes and refuse an index past the
//! table.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a flat body (a box, or a prism
//! on a triangle, a slanted quadrilateral or a pentagon), at a corner on chapter 28.5's hard cases
//! (small, on a coarse sub-grid, at either edge of the grid) with sides of one step, a few or many;
//! one of the 48 signed axis permutations with a shift (small, any, or at the grid's edge) and a
//! packed permutation byte of any value; up to [`MAX_POINTS`] probe points, each coordinate drawn
//! about a box (one step outside it, on its faces, one step in, its middle, or anywhere across it)
//! with a shell that is the body's or past it; and a general transform (a rotation about an axis by
//! quarter turns, a Pythagorean direction or any other direction, the exact permutation itself, or
//! a scale), the class the caller accepts, and an index into a side table of one row or past it.
//! [`check`] places the body exactly and generally, materialises each placement with
//! `geom_make::placed::materialise`, asks every query of `geom_topo::placed` and of a [`Library`],
//! each probe point resolved about the box of the frame it is asked in, and holds each answer to
//! the laws below. [`harness`] is `decode` then `check`, the function every fuzzing engine drives
//! (the crate README sets out the layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, and [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case:
//!   - a record of more than 88 bytes, by its size or by its logical size;
//!   - a permutation that does not pack to one of the 48 and unpack to itself, a packed byte below
//!     48 that is refused, or one past 47 that is not refused as `Permutation`;
//!   - an exact placement refused while its materialisation is made, or made while its
//!     materialisation is refused, or refused other than for range; an exact box that is not the
//!     materialised body's box;
//!   - on an exact placement, faces, a face's area, six times the volume or the side of a shell at
//!     a probe point that is not the same query's answer on the materialised body, through
//!     `geom_topo::placed` or through a [`Library`], a refusal where the materialised body answers
//!     other than for a point the placement sends out of range, or a [`Library`] that makes a body
//!     for an exact placement;
//!   - a point the placement does not send where its transform does, or that does not come back
//!     where it started, either way across;
//!   - a general placement that answers anything but `General` in `geom_topo::placed`; a
//!     [`Library`] answer that is not the same query on the body `materialise` makes, or a refusal
//!     that is not `materialise`'s; a body made more than once, or kept when it is refused; a
//!     general box that does not hold its materialised body's box;
//!   - a scale that is not refused as `NotRigid`; an index past the table that is not refused as
//!     `NoSuchPlacement` with its index, or whose refusal keeps a body.
//!
//! Method: chapter 23.9's rule that an instance is defined as its materialisation and 19.12's
//! placements (`projects/engines/geometry/architecture/23-performance.md`,
//! `projects/engines/geometry/architecture/19-meshes-spatial-clash.md`), held by asking each query
//! on the body `geom_make::placed::materialise` makes and comparing; the transform's own moves of a
//! point are the oracle for crossing. On-face probe points are the hard case of 28.5 for the side
//! of a shell, so they are drawn often.
//!
//! Cost: one body of at most six corners built, two materialisations and one more in the
//! [`Library`], each checked in full; the four queries on each placement, the side of a shell at
//! [`MAX_POINTS`] points twice over on each; at most [`MAX_INPUT_BYTES`] bytes read.

use geom_make::placed::{Library, Shared, materialise};
use geom_make::{BoxInput, Made, Moved, PrismInput, make_box, prism};
use geom_math::{Axis, Box3, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::grid::COORD_MAX;
use geom_num::{Exactness, GridStep, NumError, P3, Steps};
use geom_op::{MakeDetail, OpContext, OpError, OpErrorDetail, OpId, OpOutput, OpPath};
use geom_run::{Budget, LogicalSize, WorkMeter};
use geom_test::{ByteSource, Entropy};
use geom_topo::placed::{self, Definition, Placed, PlacedError, Placement};
use geom_topo::query::{self, FaceRow};
use geom_topo::{Body, Ix, ShellIx};

use crate::Finding;

/// The most probe points a case asks the side of a shell at.
pub const MAX_POINTS: usize = 8;

/// The bytes one drawn value reads: a rule and a word.
const VALUE_BYTES: usize = 1 + 8;

/// The bytes the body reads: its kind, three sides and a corner.
const BODY_BYTES: usize = 1 + 3 * VALUE_BYTES + 3 * VALUE_BYTES;

/// The bytes the exact placement reads: its permutation, its shift and a packed byte.
const EXACT_BYTES: usize = 1 + 3 * VALUE_BYTES + 1;

/// The bytes one probe point reads: its shell and three coordinates.
const PROBE_BYTES: usize = 1 + 3 * VALUE_BYTES;

/// The bytes the general placement reads: its kind, its axis, two words, the class accepted and
/// the index.
const GENERAL_BYTES: usize = 1 + 1 + 2 * 8 + 1 + 1;

/// The most bytes [`decode`] reads: the body, the exact placement, a count and [`MAX_POINTS`]
/// probes, and the general placement.
pub const MAX_INPUT_BYTES: usize =
    BODY_BYTES + EXACT_BYTES + 1 + MAX_POINTS * PROBE_BYTES + GENERAL_BYTES;

const _: () = assert!(MAX_INPUT_BYTES == 329);

/// The prism outlines, strictly convex and counter-clockwise as `geom_make::prism` asks, in units
/// of the drawn scale: a triangle, a slanted quadrilateral and a pentagon, whose faces are slanted
/// to every axis but the prism's.
const OUTLINES: [&[(i64, i64)]; 3] = [
    &[(0, 0), (4, 1), (1, 4)],
    &[(0, 0), (3, 0), (4, 2), (1, 2)],
    &[(0, 0), (3, 0), (4, 2), (2, 4), (0, 3)],
];

/// The widest an outline is, in units.
const OUTLINE_UNITS: u64 = 4;

/// The six arrangements of the source axes, written here again.
const SOURCES: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

/// Pythagorean directions, cosine and sine: each rotation by one rounds to the grid.
const PYTHAGOREAN: [(i32, i32); 6] = [(3, 4), (4, 3), (5, 12), (8, 15), (-3, 4), (20, 21)];

/// A flat body of a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A box.
    Box(BoxInput),
    /// A prism.
    Prism(PrismInput),
}

/// A general transform of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum General {
    /// A rotation about an axis through the origin: quarter turns, or a direction.
    Rotate(Axis, Rotation),
    /// The case's exact permutation, held in the side table.
    Exact,
    /// A uniform scale, numerator and denominator, never one.
    Scale(u64, u64),
}

/// A probe point: its shell, and for each axis a rule and a word placing it about a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// The shell asked of: the body's one shell, or one past it.
    pub shell: u32,
    /// Each coordinate's rule and word.
    pub picks: [(u8, u64); 3],
}

/// One case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The definition's body.
    pub shape: Shape,
    /// The exact permutation: 8 × its arrangement + its signs, below 48.
    pub axes: u8,
    /// The exact placement's shift.
    pub shift: [i64; 3],
    /// A packed permutation byte of any value, unpacked with the shift.
    pub packed: u8,
    /// The probe points.
    pub probes: Vec<Probe>,
    /// The general transform.
    pub general: General,
    /// The largest bound the caller accepts for a rounded body, in steps; `None` accepts none.
    pub accept: Option<u64>,
    /// The general placement's index into a side table of one row.
    pub index: u32,
}

/// A side through its rule byte: one step, a few, or many.
fn side(src: &mut ByteSource<'_>) -> u64 {
    let rule = src.byte();
    let word = src.word();
    match rule % 3 {
        0 => 1,
        1 => (word % 16).saturating_add(1),
        _ => (word % 1_000_000).saturating_add(1),
    }
}

/// A corner coordinate through its rule byte for a body `extent` wide: small, on a coarse
/// sub-grid, or at either edge of the grid with the body inside it.
fn corner(src: &mut ByteSource<'_>, extent: u64) -> i64 {
    let rule = src.byte();
    let word = src.word();
    let any = i64::from_le_bytes(word.to_le_bytes());
    let wide = i64::try_from(extent).unwrap_or(COORD_MAX);
    let slack = (any % 8).saturating_abs();
    match rule % 5 {
        0 | 1 => any % 64,
        2 => (any % 1024).saturating_mul(10),
        3 => COORD_MAX.saturating_sub(wide).saturating_sub(slack),
        _ => slack.saturating_sub(COORD_MAX),
    }
}

/// A shift component through its rule byte: small, any (often past the grid), or at its edge.
fn shift(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let any = i64::from_le_bytes(src.word().to_le_bytes());
    let slack = (any % 8).saturating_abs();
    match rule % 4 {
        0 | 1 => any % 64,
        2 => any % (1 << 41),
        _ => {
            let edge = COORD_MAX.saturating_sub(slack);
            if rule & 0x80 == 0 {
                edge
            } else {
                edge.saturating_neg()
            }
        }
    }
}

/// The body.
fn shape(src: &mut ByteSource<'_>) -> Shape {
    let kind = src.byte();
    let sides = [side(src), side(src), side(src)];
    let prism = usize::from(kind % 4).checked_sub(1);
    let extent = if prism.is_none() {
        sides
    } else {
        let wide = sides[0].saturating_mul(OUTLINE_UNITS);
        [wide, wide, sides[2]]
    };
    let at = [
        corner(src, extent[0]),
        corner(src, extent[1]),
        corner(src, extent[2]),
    ];
    let min = P3::new(at[0], at[1], at[2]).unwrap_or(P3 {
        x: geom_num::Coord::ZERO,
        y: geom_num::Coord::ZERO,
        z: geom_num::Coord::ZERO,
    });
    match prism.and_then(|k| OUTLINES.get(k)) {
        None => Shape::Box(BoxInput {
            min,
            size: sides.map(Steps),
        }),
        Some(outline) => {
            let unit = i64::try_from(sides[0]).unwrap_or(1);
            let base = outline
                .iter()
                .filter_map(|&(u, v)| {
                    let x = min.x.get().saturating_add(u.saturating_mul(unit));
                    let y = min.y.get().saturating_add(v.saturating_mul(unit));
                    P3::new(x, y, min.z.get()).ok()
                })
                .collect();
            Shape::Prism(PrismInput {
                base,
                height: Steps(sides[2]),
            })
        }
    }
}

/// One probe point.
fn probe(src: &mut ByteSource<'_>) -> Probe {
    let shell = u32::from(src.byte() % 4 == 0);
    let mut picks = [(0, 0); 3];
    for pick in &mut picks {
        *pick = (src.byte(), src.word());
    }
    Probe { shell, picks }
}

/// The general transform, the class accepted and the index.
fn general(src: &mut ByteSource<'_>) -> (General, Option<u64>, u32) {
    let kind = src.byte();
    let axis = match src.byte() % 3 {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    };
    let (first, second) = (src.word(), src.word());
    let small = |w: u64| i32::try_from(w % 2048).unwrap_or(0).saturating_sub(1024);
    let general = match kind % 5 {
        0 => General::Rotate(
            axis,
            Rotation::quarter_turns(i32::try_from(first % 4).unwrap_or(1)),
        ),
        1 => {
            let row = usize::try_from(first % 6).unwrap_or(0);
            let (cos, sin) = PYTHAGOREAN.get(row).copied().unwrap_or((3, 4));
            General::Rotate(
                axis,
                Rotation::direction(cos, sin).unwrap_or(Rotation::quarter_turns(0)),
            )
        }
        2 => {
            let (cos, sin) = (small(first), small(second));
            let (cos, sin) = if cos == 0 && sin == 0 {
                (1, 2)
            } else {
                (cos, sin)
            };
            General::Rotate(
                axis,
                Rotation::direction(cos, sin).unwrap_or(Rotation::quarter_turns(0)),
            )
        }
        3 => General::Exact,
        _ => {
            let down = (second % 5).saturating_add(1);
            General::Scale(down.saturating_add(first % 5).saturating_add(1), down)
        }
    };
    let accept = match src.byte() % 4 {
        0 => None,
        1 => Some(1),
        2 => Some(64),
        _ => Some(1 << 30),
    };
    let index = match src.byte() % 4 {
        0 | 1 => 0,
        2 => 1,
        _ => u32::MAX,
    };
    (general, accept, index)
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let shape = shape(&mut src);
    let axes = src.byte() % 48;
    let shift = [shift(&mut src), shift(&mut src), shift(&mut src)];
    let packed = src.byte();
    let count = usize::from(src.byte())
        .checked_rem(MAX_POINTS.saturating_add(1))
        .unwrap_or(0);
    let mut probes = Vec::with_capacity(MAX_POINTS);
    for i in 0..MAX_POINTS {
        let p = probe(&mut src);
        if i < count {
            probes.push(p);
        }
    }
    let (general, accept, index) = general(&mut src);
    let read = data.len().saturating_sub(src.remaining());
    let case = Case {
        shape,
        axes,
        shift,
        packed,
        probes,
        general,
        accept,
        index,
    };
    (case, read)
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// The context every operation of a case runs under.
fn context() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([0x45; 16])), Budget::UNLIMITED)
}

/// A meter with no limit.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The case's exact permutation, built from its parts here.
fn exact_transform(case: &Case) -> Result<GridTransform, NumError> {
    let source = SOURCES
        .get(usize::from(case.axes / 8))
        .copied()
        .ok_or(NumError::Degenerate)?;
    let bits = case.axes % 8;
    let negate = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
    GridTransform::from_parts(source, negate, case.shift)
}

/// The class a rounded body is accepted at, on the grid's finest step.
fn accepted(case: &Case) -> Option<Exactness> {
    case.accept.map(|bound| Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    })
}

/// The box of a body's vertices.
fn boxed(body: &Body) -> Option<Box3> {
    query::bounding_box(body, &mut meter())
        .ok()
        .flatten()
        .map(|(b, _)| b)
}

/// One coordinate about the interval `lo..=hi`: one step below, on it, one step in, the middle,
/// one step in from the top, on it, one step above, or anywhere across it.
fn about(lo: i64, hi: i64, (rule, word): (u8, u64)) -> i64 {
    match rule % 8 {
        0 => lo.saturating_sub(1),
        1 => lo,
        2 => lo.saturating_add(1),
        3 => lo.saturating_add(hi.saturating_sub(lo) / 2),
        4 => hi.saturating_sub(1),
        5 => hi,
        6 => hi.saturating_add(1),
        _ => {
            let span = hi.saturating_sub(lo).saturating_add(3).unsigned_abs();
            let off = i64::try_from(word.checked_rem(span).unwrap_or(0)).unwrap_or(0);
            lo.saturating_sub(1).saturating_add(off)
        }
    }
}

/// `probe` about box `b`, or `None` when it falls off the grid.
fn resolve(b: &Box3, probe: &Probe) -> Option<P3> {
    let (lo, hi) = (b.min(), b.max());
    let [px, py, pz] = probe.picks;
    P3::new(
        about(lo.x.get(), hi.x.get(), px),
        about(lo.y.get(), hi.y.get(), py),
        about(lo.z.get(), hi.z.get(), pz),
    )
    .ok()
}

/// The shell `probe` asks of.
fn shell(probe: &Probe) -> Option<ShellIx> {
    Ix::new(probe.shell)
}

/// The body of `case`'s shape.
fn build(ctx: &OpContext<'_>, shape: &Shape) -> Result<OpOutput<Made>, OpError> {
    match shape {
        Shape::Box(input) => make_box(ctx, input),
        Shape::Prism(input) => prism(ctx, input),
    }
}

/// The record is at most 88 bytes, by its size and its logical size.
fn small_record(case: &Case) -> Result<(), Finding> {
    let size = size_of::<Placed>();
    if size > 88 || <Placed as LogicalSize>::LOGICAL_BYTES != 88 {
        return Err(Finding::new("a placed record is at most 88 bytes", || {
            format!(
                "{size} bytes, logically {}; {case:?}",
                <Placed as LogicalSize>::LOGICAL_BYTES
            )
        }));
    }
    Ok(())
}

/// `t` packs to one of the 48 and back, and the case's packed byte unpacks or is refused.
fn packs(case: &Case, t: &GridTransform) -> Result<(), Finding> {
    let placement = Placement::exact(t);
    let fine = match placement {
        Placement::Exact { axes, shift } => {
            axes < 48 && shift == t.translation_part() && placement.grid() == Ok(*t)
        }
        Placement::General(_) => false,
    };
    if !fine {
        return Err(Finding::new(
            "a permutation packs to one of the 48 and unpacks to itself",
            || format!("{placement:?} of {t:?}; {case:?}"),
        ));
    }
    let raw = Placement::Exact {
        shift: t.translation_part(),
        axes: case.packed,
    };
    let fine = match raw.grid() {
        Ok(back) => case.packed < 48 && Placement::exact(&back) == raw,
        Err(e) => case.packed >= 48 && e == PlacedError::Permutation(case.packed),
    };
    if !fine {
        return Err(Finding::new(
            "a packed byte below 48 unpacks to itself, and one past 47 is refused as Permutation",
            || format!("{:?}; {case:?}", raw.grid()),
        ));
    }
    Ok(())
}

/// A query's answer through a placement, against the same query on the materialised body.
fn same<T: PartialEq + core::fmt::Debug>(
    case: &Case,
    what: &str,
    placed: &Result<T, PlacedError>,
    made: &Result<T, query::QueryError>,
) -> Result<(), Finding> {
    let fine = match (placed, made) {
        (Ok(a), Ok(b)) => a == b,
        (Err(PlacedError::Query(a)), Err(b)) => a == b,
        _ => false,
    };
    if fine {
        Ok(())
    } else {
        Err(Finding::new(
            "every query on an exact placement equals the same query on its materialisation",
            || format!("{what}: placed {placed:?}, materialised {made:?}; {case:?}"),
        ))
    }
}

/// A library's answer on an exact placement is `geom_topo::placed`'s.
fn same_through<T: PartialEq + core::fmt::Debug>(
    case: &Case,
    what: &str,
    library: &Result<T, OpError>,
    placed: &Result<T, PlacedError>,
) -> Result<(), Finding> {
    let fine = match (library, placed) {
        (Ok(a), Ok(b)) => a == b,
        (Err(_), Err(_)) => true,
        _ => false,
    };
    if fine {
        Ok(())
    } else {
        Err(Finding::new(
            "a library answers an exact placement as geom-topo does, making no body",
            || format!("{what}: library {library:?}, placed {placed:?}; {case:?}"),
        ))
    }
}

/// Every query on the exact instance of `shared` against its materialisation `made`.
fn exact_answers(
    case: &Case,
    shared: &OpOutput<Made>,
    instance: &Placed,
    made: &Body,
) -> Result<(), Finding> {
    let body = &shared.result.body;
    if Some(*instance.bound()) != boxed(made) {
        return Err(Finding::new(
            "an exact box is its materialised body's box",
            || format!("{:?} against {:?}; {case:?}", instance.bound(), boxed(made)),
        ));
    }
    let table: [TypedTransform; 0] = [];
    let mut library = Library::new(&table);
    let key = Shared {
        key: 0_u8,
        body,
        exactness: shared.exactness,
        accept: None,
    };
    let ctx = context();
    let faces = placed::faces(body, instance, &mut meter());
    same(case, "faces", &faces, &query::faces(made, &mut meter()))?;
    same_through(case, "faces", &library.faces(&ctx, &key, instance), &faces)?;
    let rows: Vec<FaceRow> = faces.unwrap_or_default();
    for row in &rows {
        let area = placed::face_area(body, instance, row.face, &mut meter());
        same(
            case,
            "face area",
            &area,
            &query::face_area(made, row.face, &mut meter()),
        )?;
        same_through(
            case,
            "face area",
            &library.face_area(&ctx, &key, instance, row.face),
            &area,
        )?;
    }
    let six = placed::six_volume(body, instance, &mut meter());
    same(case, "six volume", &six, &query::volume(made, &mut meter()))?;
    same_through(
        case,
        "six volume",
        &library.six_volume(&ctx, &key, instance),
        &six,
    )?;
    for probe in &case.probes {
        let (Some(q), Some(s)) = (resolve(instance.bound(), probe), shell(probe)) else {
            continue;
        };
        let got = placed::side_of_shell(body, instance, s, q, &mut meter());
        let want = query::side_of_shell(made, s, q, &mut meter());
        let away = matches!(got, Err(PlacedError::Num(_)))
            && matches!(placed::to_shared(instance, q), Err(PlacedError::Num(_)));
        if !away {
            same(case, &format!("side of shell {s:?} at {q:?}"), &got, &want)?;
        }
        same_through(
            case,
            "side of shell",
            &library.side_of_shell(&ctx, &key, instance, s, q),
            &got,
        )?;
    }
    if !library.is_empty() {
        return Err(Finding::new(
            "a library answers an exact placement as geom-topo does, making no body",
            || format!("{} bodies made; {case:?}", library.len()),
        ));
    }
    Ok(())
}

/// Each probe point crosses the placement as `t` moves it and comes back, both ways.
fn crosses(
    case: &Case,
    t: &GridTransform,
    instance: &Placed,
    shared: &Box3,
) -> Result<(), Finding> {
    for probe in &case.probes {
        let wrong = |what: &str, q: P3| {
            let what = what.to_owned();
            Finding::new(
                "a point crosses a placement as its transform moves it, and comes back",
                move || format!("{what} at {q:?}; {case:?}"),
            )
        };
        if let Some(q) = resolve(shared, probe) {
            let there = placed::from_shared(instance, q);
            if there != t.apply(q).map_err(PlacedError::Num) {
                return Err(wrong("from the definition", q));
            }
            if let Ok(x) = there {
                if placed::to_shared(instance, x) != Ok(q) {
                    return Err(wrong("back to the definition", q));
                }
            }
        }
        if let Some(q) = resolve(instance.bound(), probe) {
            let here = placed::to_shared(instance, q);
            let want = t
                .inverse()
                .and_then(|b| b.apply(q))
                .map_err(PlacedError::Num);
            if here != want {
                return Err(wrong("into the definition", q));
            }
            if let Ok(y) = here {
                if placed::from_shared(instance, y) != Ok(q) {
                    return Err(wrong("back to the instance", q));
                }
            }
        }
    }
    Ok(())
}

/// The exact placement: packed, placed, materialised, asked and crossed.
fn exact_leg(case: &Case, shared: &OpOutput<Made>, bound: &Box3) -> Result<(), Finding> {
    let Ok(t) = exact_transform(case) else {
        // A shift past the grid transform's own limit: no placement to make.
        return Ok(());
    };
    packs(case, &t)?;
    let instance = Placed::exact(Definition(u32::from(case.axes)), &t, bound);
    let moved = Moved {
        body: &shared.result.body,
        exactness: shared.exactness,
        accept: None,
    };
    let made = materialise(&context(), moved, &TypedTransform::Exact(t));
    match (instance, made) {
        (Ok(instance), Ok(made)) => {
            exact_answers(case, shared, &instance, &made.result.body)?;
            crosses(case, &t, &instance, bound)
        }
        (Err(PlacedError::Num(_)), Err(_)) => Ok(()),
        (instance, made) => Err(Finding::new(
            "an exact placement is refused for range exactly when its materialisation is refused",
            || {
                format!(
                    "placed {instance:?}, materialised {:?}; {case:?}",
                    made.map(|m| m.exactness)
                )
            },
        )),
    }
}

/// The general transform of `case`, `None` for a rotation the kernel refuses to make.
fn general_transform(case: &Case) -> Option<TypedTransform> {
    match case.general {
        General::Rotate(axis, rotation) => TypedTransform::rotation(axis, rotation).ok(),
        General::Exact => exact_transform(case).ok().map(TypedTransform::Exact),
        General::Scale(n, d) => RationalTransform::uniform_scale(Scale::new(n, d).ok()?)
            .ok()
            .map(TypedTransform::from),
    }
}

/// A general placement answers `General` in `geom_topo::placed`.
fn answers_general(case: &Case, body: &Body, instance: &Placed) -> Result<(), Finding> {
    let origin = P3 {
        x: geom_num::Coord::ZERO,
        y: geom_num::Coord::ZERO,
        z: geom_num::Coord::ZERO,
    };
    let s = Ix::new(0).ok_or_else(|| Finding::new("shell 0 is an index", String::new))?;
    let f = Ix::new(0).ok_or_else(|| Finding::new("face 0 is an index", String::new))?;
    let answers = [
        placed::to_shared(instance, origin).map(|_| ()),
        placed::from_shared(instance, origin).map(|_| ()),
        placed::faces(body, instance, &mut meter()).map(|_| ()),
        placed::face_area(body, instance, f, &mut meter()).map(|_| ()),
        placed::six_volume(body, instance, &mut meter()).map(|_| ()),
        placed::side_of_shell(body, instance, s, origin, &mut meter()).map(|_| ()),
    ];
    if answers.iter().any(|a| *a != Err(PlacedError::General)) {
        return Err(Finding::new(
            "a general placement answers General in geom-topo",
            || format!("{answers:?}; {case:?}"),
        ));
    }
    Ok(())
}

/// A library's answer on a general placement against the same query on `made`.
fn same_made<T: PartialEq + core::fmt::Debug>(
    case: &Case,
    what: &str,
    library: &Result<T, OpError>,
    made: &Result<T, query::QueryError>,
) -> Result<(), Finding> {
    let fine = match (library, made) {
        (Ok(a), Ok(b)) => a == b,
        (Err(_), Err(_)) => true,
        _ => false,
    };
    if fine {
        Ok(())
    } else {
        Err(Finding::new(
            "a library answers a general placement as its materialised body does",
            || format!("{what}: library {library:?}, materialised {made:?}; {case:?}"),
        ))
    }
}

/// Every query of a library on the general instance against the body `materialise` makes.
fn general_answers(
    case: &Case,
    shared: &OpOutput<Made>,
    instance: &Placed,
    t: &TypedTransform,
) -> Result<(), Finding> {
    let table = [*t];
    let mut library = Library::new(&table);
    let key = Shared {
        key: 0_u8,
        body: &shared.result.body,
        exactness: shared.exactness,
        accept: accepted(case),
    };
    let ctx = context();
    let got = library.six_volume(&ctx, &key, instance);
    if case.index != 0 {
        let fine = got.as_ref().is_err_and(|e| {
            *e.detail() == OpErrorDetail::Make(MakeDetail::NoSuchPlacement { index: case.index })
        }) && library.is_empty();
        if !fine {
            return Err(Finding::new(
                "an index past the table is refused as NoSuchPlacement, and nothing is kept",
                || format!("{got:?}, {} kept; {case:?}", library.len()),
            ));
        }
        return Ok(());
    }
    let moved = Moved {
        body: &shared.result.body,
        exactness: shared.exactness,
        accept: accepted(case),
    };
    let made = match materialise(&ctx, moved, t) {
        Ok(made) => made,
        Err(want) => {
            let fine = got
                .as_ref()
                .is_err_and(|e| e.detail() == want.detail() && e.kind() == want.kind())
                && library.is_empty();
            if !fine {
                return Err(Finding::new(
                    "a general placement whose body is refused is refused as materialise refuses \
                     it, and nothing is kept",
                    || format!("{got:?} against {want:?}, {} kept; {case:?}", library.len()),
                ));
            }
            return Ok(());
        }
    };
    let body = &made.result.body;
    same_made(case, "six volume", &got, &query::volume(body, &mut meter()))?;
    let faces = library.faces(&ctx, &key, instance);
    same_made(case, "faces", &faces, &query::faces(body, &mut meter()))?;
    for row in faces.unwrap_or_default() {
        same_made(
            case,
            "face area",
            &library.face_area(&ctx, &key, instance, row.face),
            &query::face_area(body, row.face, &mut meter()),
        )?;
    }
    for probe in &case.probes {
        let (Some(q), Some(s)) = (resolve(instance.bound(), probe), shell(probe)) else {
            continue;
        };
        same_made(
            case,
            &format!("side of shell {s:?} at {q:?}"),
            &library.side_of_shell(&ctx, &key, instance, s, q),
            &query::side_of_shell(body, s, q, &mut meter()),
        )?;
    }
    if library.len() != 1 {
        return Err(Finding::new(
            "a general placement's body is made once, when first asked",
            || format!("{} bodies; {case:?}", library.len()),
        ));
    }
    let held = boxed(body)
        .is_some_and(|b| instance.bound().contains(b.min()) && instance.bound().contains(b.max()));
    if !held {
        return Err(Finding::new(
            "a general box holds its materialised body's box",
            || format!("{:?} against {:?}; {case:?}", instance.bound(), boxed(body)),
        ));
    }
    Ok(())
}

/// The general placement: refused if it scales, else `General` in geom-topo and the materialised
/// body's answers through a library.
fn general_leg(case: &Case, shared: &OpOutput<Made>, bound: &Box3) -> Result<(), Finding> {
    let Some(t) = general_transform(case) else {
        return Ok(());
    };
    let instance = Placed::general(Definition(1), case.index, &t, bound);
    let scale = matches!(case.general, General::Scale(..));
    match instance {
        Err(PlacedError::NotRigid) if scale => Ok(()),
        Err(PlacedError::Num(_)) if !scale => Ok(()),
        Ok(instance) if !scale => {
            answers_general(case, &shared.result.body, &instance)?;
            general_answers(case, shared, &instance, &t)
        }
        other => Err(Finding::new(
            "a scale is refused as NotRigid, and a rotation is placed or refused for range",
            || format!("{other:?}; {case:?}"),
        )),
    }
}

/// Holds the case to the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    small_record(case)?;
    let shared = match build(&context(), &case.shape) {
        Ok(shared) => shared,
        Err(e) => {
            return Err(Finding::new(
                "a box or prism of positive sides inside the grid is made",
                || format!("{e:?}; {case:?}"),
            ));
        }
    };
    let Some(bound) = boxed(&shared.result.body) else {
        return Err(Finding::new("a made body has a box", || {
            format!("{case:?}")
        }));
    };
    exact_leg(case, &shared, &bound)?;
    general_leg(case, &shared, &bound)
}
