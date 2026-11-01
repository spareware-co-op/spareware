//! Fuzz target: `geom-mesh`'s operations on small solids agree with counted cells and with each
//! other.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_BOXES`] boxes on a
//! lattice of [`SIDE`] cells a side, where shared faces, edges and corners are the rule, scaled by
//! one of [`SCALES`]. [`check`] counts the cells the boxes cover, which is the truth here, and holds
//! to it every operation that reaches: booleans (G148), point location (G147), slices, hulls and
//! exports (G149), Minkowski sums (G150), contact (G151) and the envelope (G152). [`harness`] is
//! `decode` then `check`, the function every fuzzing engine drives (the crate README sets out the
//! layout).
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes.
//! - **A panic or an error is a failure.** Every decoded case is valid, so an error is a [`Finding`].
//! - **A broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: chapter 19's laws (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`),
//! by counting. Each box is a set of whole cells, so what covers a cell is a
//! matter of three comparisons, written here without the code's help. A union, an intersection and
//! a difference have the volumes their cells say; a cell's middle is inside what covers it; a slice
//! has the area of the cells under it; a hull holds every corner; the sum of two boxes adds their
//! sides; a text file carries the mesh with nothing rounded; a corner of a box is inside its
//! envelope; and boxes with a cell between them are apart.
//!
//! Cost: at most [`MAX_BOXES`] boxes, one arrangement, and about a hundred more exact operations.

use geom_mesh::boolean::{Outcome, boolean};
use geom_mesh::classify::{Classifier, Located};
use geom_mesh::contact::{Contact, triangles};
use geom_mesh::envelope::Envelope;
use geom_mesh::exact::build::{Corner, Polygons, Source, build};
use geom_mesh::exact::mass::Volume;
use geom_mesh::exact::{ExactMesh, MeshKind, PropTable, Role};
use geom_mesh::export::{Format, export, triangles as split};
use geom_mesh::hull::Hull;
use geom_mesh::minkowski::minkowski;
use geom_mesh::slice::{point, slice};
use geom_num::grid::{GridStep, P3};
use geom_num::plane::Plane;
use geom_num::predicate::orient3d;
use geom_num::wide::I256;
use geom_num::{Exactness, Sign};
use geom_op::{NameId, OpId, OpPath};
use geom_poly::winding::WindingTest;
use geom_run::{Budget, Exec, WorkMeter};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The most boxes a case holds.
pub const MAX_BOXES: u8 = 3;

/// Cells to a side of the lattice.
pub const SIDE: u8 = 5;

/// What the lattice is scaled by: small and even, so that a cell has a middle on the grid, and one
/// that takes the lattice near the edge of the frame.
pub const SCALES: [i64; 3] = [2, 6, 1 << 36];

/// The most bytes [`decode`] reads from any input: two for the scale and the count, and six for
/// each box.
pub const MAX_INPUT_BYTES: usize = 20;

/// One decoded input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Each box as low and high cells, the high end past the last cell.
    pub boxes: Vec<([i64; 3], [i64; 3])>,
    /// Steps to a cell.
    pub scale: i64,
}

/// The case `data` holds, and how many bytes it read.
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "a byte modulo the table's length indexes [`SCALES`], and cells below [`SIDE`] + 1 add nothing near the range of `i64`"
)]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let scale = SCALES[usize::from(src.byte()) % SCALES.len()];
    let count = 1 + src.byte() % MAX_BOXES;
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
    let read = data.len().saturating_sub(src.remaining());
    (Case { boxes, scale }, read)
}

/// A meter no case reaches.
fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn at(x: i64, y: i64, z: i64) -> Result<P3, Finding> {
    P3::new(x, y, z).map_err(|e| Finding::new("a case fits the frame", || format!("{e:?}")))
}

/// The box from `low` to `high` cells, run `name`, as a mesh.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "cells are at most [`SIDE`] and the scale at most 2^36, so a corner is below 2^40; a box has six faces of four corners"
)]
fn solid(b: &([i64; 3], [i64; 3]), scale: i64, name: u8) -> Result<ExactMesh, Finding> {
    let (low, high) = b;
    let [ax, ay, az] = low.map(|c| c * scale);
    let [bx, by, bz] = high.map(|c| c * scale);
    let mut vertices = Vec::with_capacity(8);
    for (x, y, z) in [
        (ax, ay, az),
        (bx, ay, az),
        (bx, by, az),
        (ax, by, az),
        (ax, ay, bz),
        (bx, ay, bz),
        (bx, by, bz),
        (ax, by, bz),
    ] {
        vertices.push(Corner::Grid(at(x, y, z)?));
    }
    let corners = vec![
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
    ];
    let input = Polygons {
        vertices,
        corners,
        loop_start: (0..=6).map(|f| f * 4).collect(),
        face_loops: (0..=6).collect(),
        face_run: vec![0; 6],
        runs: vec![Source {
            name: NameId::from_bytes([name; 16]),
            role: Role::Face,
            flags: 0,
        }],
        face_plane: Vec::new(),
        edge_plane: Vec::new(),
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: Exactness::Exact,
    };
    build(&input, &Exec::serial(), &mut meter())
        .map_err(|e| Finding::new("a box builds", || format!("{b:?}: {e:?}")))
}

/// Whether box `b` covers cell `c`.
fn covers(b: &([i64; 3], [i64; 3]), c: [i64; 3]) -> bool {
    let (low, high) = b;
    low.iter()
        .zip(high)
        .zip(c)
        .all(|((l, h), v)| *l <= v && v < *h)
}

/// How many of the boxes cover each cell.
fn cells(case: &Case) -> Vec<([i64; 3], usize)> {
    let span = i64::from(SIDE);
    let mut out = Vec::new();
    for x in 0..span {
        for y in 0..span {
            for z in 0..span {
                let c = [x, y, z];
                out.push((c, case.boxes.iter().filter(|b| covers(b, c)).count()));
            }
        }
    }
    out
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

/// `mesh` has the volume of `count` cells, and is closed.
fn holds_cells(law: &str, mesh: &ExactMesh, count: usize, case: &Case) -> Result<(), Finding> {
    let got = mesh.volume(&mut meter());
    let wanted = six_times(count, case.scale);
    let good = matches!((&got, &wanted), (Ok(Volume::Exact(v)), Some(w)) if v == w);
    if !good || mesh.kind() == MeshKind::Open {
        return Err(Finding::new(law, || {
            format!("{case:?}: {got:?} for {count} cells")
        }));
    }
    Ok(())
}

/// The mesh of what `test` keeps.
fn result(operands: &[ExactMesh], test: WindingTest, case: &Case) -> Result<Outcome, Finding> {
    let refs: Vec<&ExactMesh> = operands.iter().collect();
    let step = OpPath::new(OpId::from_bytes([7; 16]));
    boolean(&refs, &[test], &step, &Exec::serial(), &mut meter())
        .map_err(|e| {
            Finding::new("a boolean of closed boxes holds", || {
                format!("{case:?}: {e:?}")
            })
        })?
        .pop()
        .ok_or_else(|| Finding::new("one test gives one result", || format!("{case:?}")))
}

/// Booleans against counted cells, and point location and slices on the union.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "a cell's middle is below 2^40"
)]
fn check_union(case: &Case, operands: &[ExactMesh]) -> Result<ExactMesh, Finding> {
    let counted = cells(case);
    let at_least = |k: usize| counted.iter().filter(|(_, n)| *n >= k).count();
    let union = result(operands, WindingTest::unite(), case)?;
    holds_cells("a union has its cells", &union.mesh, at_least(1), case)?;
    let two = result(operands, WindingTest::intersect(2), case)?;
    holds_cells("what two cover has its cells", &two.mesh, at_least(2), case)?;
    let only = counted
        .iter()
        .filter(|(c, n)| *n == 1 && case.boxes.first().is_some_and(|b| covers(b, *c)))
        .count();
    let less = result(operands, WindingTest::subtract(0), case)?;
    holds_cells("a difference has its cells", &less.mesh, only, case)?;
    let classifier = Classifier::new(&union.mesh, &Exec::serial(), &mut meter());
    let classifier = classifier
        .map_err(|e| Finding::new("a union classifies", || format!("{case:?}: {e:?}")))?;
    let half = case.scale / 2;
    for (c, n) in &counted {
        let [x, y, z] = c.map(|v| v * case.scale + half);
        let found = classifier.locate(at(x, y, z)?, &mut meter());
        let inside = matches!(found, Ok(Located::Winding(w)) if w > 0);
        if inside != (*n > 0) {
            return Err(Finding::new(
                "a cell's middle is where its boxes are",
                || format!("{case:?}: {c:?} is {found:?}"),
            ));
        }
    }
    Ok(union.mesh)
}

/// The area of each level of the union, against its cells.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "coordinates are below 2^40, so twice a section's area is below 2^82 and fits `i128`"
)]
fn check_slices(case: &Case, union: &ExactMesh) -> Result<(), Finding> {
    let counted = cells(case);
    let scale = i128::from(case.scale);
    let mut planes = Vec::new();
    for z in 1..=i128::from(SIDE) {
        planes.push(
            Plane::new([0, 0, 1], -z * scale)
                .map_err(|e| Finding::new("a level is a plane", || format!("{e:?}")))?,
        );
    }
    let layers = slice(union, &planes, &Exec::serial(), &mut meter())
        .map_err(|e| Finding::new("a union slices", || format!("{case:?}: {e:?}")))?;
    for ((level, plane), z) in layers.iter().zip(&planes).zip(0_i64..) {
        let mut twice = 0_i128;
        for contour in level {
            let mut ring = Vec::with_capacity(contour.cuts().len());
            for cut in contour.cuts() {
                let place = point(union, plane, *cut)
                    .and_then(|p| Ok(p.to_grid()?))
                    .map_err(|e| {
                        Finding::new("a cut has a point", || format!("{case:?}: {e:?}"))
                    })?;
                ring.push((i128::from(place.x.get()), i128::from(place.y.get())));
            }
            let after = ring.iter().cycle().skip(1);
            for ((x, y), (u, v)) in ring.iter().zip(after) {
                twice += x * v - u * y;
            }
        }
        let under = counted
            .iter()
            .filter(|([_, _, c], n)| *c == z && *n > 0)
            .count();
        if twice != 2 * i128::try_from(under).unwrap_or(0) * scale * scale {
            return Err(Finding::new("a slice has the area of its cells", || {
                format!("{case:?}: level {z}: twice {twice}")
            }));
        }
    }
    Ok(())
}

/// Every corner on or under every face of the hull of the boxes.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "corners are below 2^40, and a hull's rings index its own points; a and d are a box's low and high x, b and e its y, c and f its z"
)]
fn check_hull(case: &Case) -> Result<(), Finding> {
    let mut points = Vec::new();
    for (low, high) in &case.boxes {
        for mask in 0..8_u8 {
            let pick = |k: u8, l: i64, h: i64| if mask >> k & 1 == 0 { l } else { h };
            let [a, b, c] = *low;
            let [d, e, f] = *high;
            let (x, y, z) = (pick(0, a, d), pick(1, b, e), pick(2, c, f));
            points.push(at(x * case.scale, y * case.scale, z * case.scale)?);
        }
    }
    let hull = Hull::of(&points, &mut meter())
        .map_err(|e| Finding::new("corners have a hull", || format!("{case:?}: {e:?}")))?;
    for ring in hull.faces() {
        let corner = |k: usize| hull.points()[ring[k] as usize];
        for q in &points {
            let side = orient3d(corner(0), corner(1), corner(2), *q);
            if side.is_err() || side == Ok(Sign::Positive) {
                return Err(Finding::new(
                    "every point is on or under every face",
                    || format!("{case:?}: {q:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// A box's corner is inside its envelope; boxes a cell apart do not touch; text carries the union
/// with nothing rounded; and two boxes sum to the box of their sides.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "triangles index their own vertices, and a box's sides are below twice [`SIDE`] cells"
)]
fn check_rest(case: &Case, operands: &[ExactMesh], union: &ExactMesh) -> Result<(), Finding> {
    let written = export(
        union,
        Format::Obj,
        GridStep::Q0,
        false,
        &Exec::serial(),
        &mut meter(),
    )
    .map_err(|e| Finding::new("a union writes", || format!("{case:?}: {e:?}")))?;
    if written.class != Exactness::Exact || written.bound != 0 {
        return Err(Finding::new("text rounds nothing", || format!("{case:?}")));
    }
    let [first, rest @ ..] = operands else {
        return Ok(());
    };
    let pieces = split(first, &mut meter())
        .map_err(|e| Finding::new("a box splits", || format!("{case:?}: {e:?}")))?;
    let triangle = |c: [u32; 3]| c.map(|v| pieces.vertices[v as usize]);
    let list: Vec<[P3; 3]> = pieces.corners.iter().map(|c| triangle(*c)).collect();
    let envelope = Envelope::new(&list, 8, &Exec::serial(), &mut meter())
        .map_err(|e| Finding::new("a box has an envelope", || format!("{case:?}: {e:?}")))?;
    for p in &pieces.vertices {
        if envelope.contains(*p, &mut meter()) != Ok(true) {
            return Err(Finding::new("a corner is inside its envelope", || {
                format!("{case:?}: {p:?}")
            }));
        }
    }
    if let ([a, b, ..], [second, ..]) = (case.boxes.as_slice(), rest) {
        let [low_a, high_a] = [a.0, a.1];
        let [low_b, high_b] = [b.0, b.1];
        let apart = low_a.iter().zip(high_b).any(|(l, h)| *l > h)
            || low_b.iter().zip(high_a).any(|(l, h)| *l > h);
        let others = split(second, &mut meter())
            .map_err(|e| Finding::new("a box splits", || format!("{case:?}: {e:?}")))?;
        for x in &list {
            for corner in &others.corners {
                let y = corner.map(|v| others.vertices[v as usize]);
                if apart && triangles(*x, y) != Ok(Contact::Apart) {
                    return Err(Finding::new("boxes a cell apart do not touch", || {
                        format!("{case:?}")
                    }));
                }
            }
        }
        if case.scale < 1 << 30 {
            let source = Source {
                name: NameId::from_bytes([9; 16]),
                role: Role::Face,
                flags: 0,
            };
            let sum = minkowski(
                first,
                second,
                (source, GridStep::Q0),
                &Exec::serial(),
                &mut meter(),
            )
            .map_err(|e| Finding::new("two boxes sum", || format!("{case:?}: {e:?}")))?;
            let side = |k: usize| (high_a[k] - low_a[k]) + (high_b[k] - low_b[k]);
            let count = side(0) * side(1) * side(2);
            holds_cells(
                "a sum of boxes adds their sides",
                &sum.mesh,
                usize::try_from(count).unwrap_or(0),
                case,
            )?;
        }
    }
    Ok(())
}

/// Holds the operations to the case's cells.
///
/// # Errors
///
/// A [`Finding`] naming the first law broken, or an error from a valid case.
pub fn check(case: &Case) -> Result<(), Finding> {
    let mut operands = Vec::with_capacity(case.boxes.len());
    for (b, name) in case.boxes.iter().zip(1_u8..) {
        operands.push(solid(b, case.scale, name)?);
    }
    let union = check_union(case, &operands)?;
    check_slices(case, &union)?;
    check_hull(case)?;
    check_rest(case, &operands, &union)
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As for [`check`].
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
