//! Operands and selections: what a boolean takes, and an operand's faces as polygons (11.1, 11.3).
//!
//! What it does: an [`Operand`] is a body with a stable key and a kind, and a [`Selection`] says
//! what to keep. [`prepare`] checks them, orders the operands by key, gives each solid an entry of
//! the arrangement, turns each selection into a [`WindingTest`], and converts every face into the
//! convex [`Polygon`]s the arrangement takes, each with the [`Carrier`] its pieces will return to.
//!
//! How a face enters: its plane is its surface, given by integers. Each side of a loop lies on the
//! plane of the face across its edge, turned to be positive on this face's inside, so the bounds of
//! its pieces are planes the body already has (11.3). Fins that run on along one line are one side.
//! A seam, where the face across lies on the same plane, takes a plane stood on the edge, or one of
//! its ends' own planes.
//!
//! Guarantees:
//!
//! - **The order of the operands does not count.** Entries follow the keys, and every later step
//!   follows the entries (11.1).
//! - **Refused, never guessed.** Two operands with one key are [`PrepareError::DuplicateOperand`],
//!   a target that is no operand is [`PrepareError::UnknownOperand`], and a face that is not flat
//!   on integers is [`PrepareError::NotPlaneBased`], which the curved path will take (11.4).
//! - **Nothing new.** Every bound of every polygon is a plane of the operand, or one stood on a seam
//!   between grid points, so 6.2's budget holds.
//!
//! Method: chapter 11.1 and 11.3's "How faces enter"
//! (`projects/engines/geometry/architecture/11-booleans.md`), after EMBER's section 3.2 (Trettner,
//! Nehring-Wirxel and Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: linear in the operands' fins, and what [`convex_pieces`] costs per face. Charges the meter
//! what that does.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_num::math::round_ties_even;
use geom_num::plane::Plane;
use geom_num::{Exactness, NumError, P3, Sign};
use geom_poly::convex::{ConvexError, Piece, Side, convex_pieces};
use geom_poly::merge::Merge;
use geom_poly::order::{Line, along, normals_det};
use geom_poly::plane::canonical;
use geom_poly::trace::Polygon;
use geom_poly::winding::{Entry, Term as Inner, WindingError, WindingTest};
use geom_run::{Stopped, WorkMeter};
use geom_topo::{Arenas, Body, Face, Fin, Ix, PointKind, Sense, SurfaceKind, Vertex};

use crate::rebuild::Carrier;

/// An operand's stable key: an element's permanent ID, or an `OpPath`'s bytes (11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperandKey(pub [u8; 16]);

/// What an operand does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    /// It counts towards inside and outside.
    Solid,
    /// It cuts faces and cells, and changes nothing about what is inside what.
    Cutter,
}

/// A body with a key and a kind.
#[derive(Clone, Copy, Debug)]
pub struct Operand<'b> {
    /// The stable key.
    pub key: OperandKey,
    /// The body.
    pub body: &'b Body,
    /// What it does.
    pub kind: OperandKind,
    /// How exact the body is.
    pub exactness: Exactness,
}

/// A term of a [`Selection::Where`] test, in postfix, over operand keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    /// Inside this operand.
    Inside(OperandKey),
    /// Inside at least this many solids.
    AtLeast(u32),
    /// Both answers below.
    And,
    /// Either answer below.
    Or,
    /// The opposite of the answer below.
    Not,
}

/// What to keep of the arrangement (11.1). General fuse and cells come with their regions (G137).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Inside any solid.
    Unite,
    /// Inside the target and outside every other solid.
    Subtract {
        /// The target.
        target: OperandKey,
    },
    /// Inside every solid.
    Intersect,
    /// Passing a test on winding numbers.
    Where(Vec<Term>),
    /// The target, its faces split where the others meet it, its shape unchanged.
    Imprint {
        /// The target.
        target: OperandKey,
    },
    /// The target, cut into separate bodies by the others.
    Split {
        /// The target.
        target: OperandKey,
    },
    /// The pieces of a sheet target that pass a test on the solids.
    Keep {
        /// The sheet.
        target: OperandKey,
        /// The test.
        test: Vec<Term>,
    },
    /// A sheet target cut by the others, keeping the pieces that hold the seeds.
    Trim {
        /// The sheet.
        target: OperandKey,
        /// Points on the pieces to keep.
        seeds: Vec<P3>,
    },
    /// Only the curves where operands meet, as a wire body.
    Section,
}

/// A boolean's input, without the budget and exactness its caller's context holds.
#[derive(Clone, Debug)]
pub struct BooleanInput<'b> {
    /// The operands.
    pub operands: Vec<Operand<'b>>,
    /// One arrangement, several results.
    pub select: Vec<Selection>,
    /// What merges.
    pub merge: Merge,
    /// The least exact result taken; `None` takes anything within `ε_fit` (11.1).
    pub accept: Option<Exactness>,
}

/// Why a boolean's input was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    /// Two operands share a key.
    DuplicateOperand(OperandKey),
    /// A selection names a key that no solid operand has.
    UnknownOperand(OperandKey),
    /// A test does not leave one answer, or is too deep.
    BadTest(WindingError),
    /// A face of this operand is not flat on integers.
    NotPlaneBased(OperandKey),
    /// A face that cannot enter: an edge with no fins of its own, or a corner with no planes.
    Broken(OperandKey),
    /// The numbers layer refused.
    Num(NumError),
    /// The meter stopped the run.
    Stopped(Stopped),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareError::DuplicateOperand(_) => f.write_str("two operands share a key"),
            PrepareError::UnknownOperand(_) => f.write_str("a selection names no solid operand"),
            PrepareError::BadTest(e) => write!(f, "{e}"),
            PrepareError::NotPlaneBased(_) => f.write_str("an operand has a face not on integers"),
            PrepareError::Broken(_) => f.write_str("an operand has a face that cannot enter"),
            PrepareError::Num(e) => write!(f, "an operand that cannot enter: {e:?}"),
            PrepareError::Stopped(_) => f.write_str("the meter stopped the preparation"),
        }
    }
}

impl core::error::Error for PrepareError {}

impl From<NumError> for PrepareError {
    fn from(e: NumError) -> Self {
        PrepareError::Num(e)
    }
}

impl From<ConvexError> for PrepareError {
    fn from(e: ConvexError) -> Self {
        match e {
            ConvexError::Num(e) => PrepareError::Num(e),
            ConvexError::Stopped(e) => PrepareError::Stopped(e),
        }
    }
}

/// A boolean, ready for the arrangement.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// The operands' keys, in entry order, solids first by key, each with its entry; cutters have
    /// none.
    pub keys: Vec<(OperandKey, Option<Entry>)>,
    /// Every face's convex polygons, in operand order then face order.
    pub polygons: Vec<Polygon>,
    /// Each polygon's carrier.
    pub carriers: Vec<Carrier>,
    /// Each polygon's operand, as a place in `keys`.
    pub owners: Vec<usize>,
    /// Each polygon's face, counted over every operand.
    pub sources: Vec<usize>,
    /// One test per selection.
    pub tests: Vec<WindingTest>,
    /// One place in `keys` per selection that names a target.
    pub targets: Vec<Option<usize>>,
}

/// `plane`, facing the other way.
fn turned(plane: &Plane) -> Result<Plane, NumError> {
    let [x, y, z] = plane.n();
    let negate = |v: i128| v.checked_neg().ok_or(NumError::BudgetExceeded);
    Plane::new([negate(x)?, negate(y)?, negate(z)?], negate(plane.d())?)
}

/// The grid point at `v`, if every coordinate is a whole number of steps.
fn on_grid(v: [f64; 3]) -> Option<P3> {
    let whole = |x: f64| -> Option<i64> {
        let r = round_ties_even(x).ok()?.get();
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a double past the range saturates, and P3::new below refuses it"
        )]
        let n = r as i64;
        (r == x).then_some(n)
    };
    let [x, y, z] = v;
    P3::new(whole(x)?, whole(y)?, whole(z)?).ok()
}

/// The plane of a flat surface given by integers, facing as `sense` turns it.
fn face_plane(surface: &SurfaceKind, sense: Sense) -> Option<Result<Plane, NumError>> {
    let SurfaceKind::Plane(flat) = surface else {
        return None;
    };
    let (origin, along_u, along_v) = flat.vectors();
    let add = |a: [f64; 3], b: [f64; 3]| {
        let [ax, ay, az] = a;
        let [bx, by, bz] = b;
        [ax + bx, ay + by, az + bz]
    };
    let (p, q, r) = (
        on_grid(origin)?,
        on_grid(add(origin, along_u))?,
        on_grid(add(origin, along_v))?,
    );
    Some(Plane::through(p, q, r).and_then(|plane| match sense {
        Sense::Forward => Ok(plane),
        Sense::Reversed => turned(&plane),
    }))
}

/// The grid point or planes of a vertex.
fn point_of(a: &Arenas, v: Ix<Vertex>) -> Option<PointKind> {
    a.points.get(a.vertices.get(v)?.point).copied()
}

/// A plane through `v` that crosses the line where `p` and `q` meet.
#[allow(
    clippy::many_single_char_names,
    reason = "p and q are the planes, v the vertex, and g, x, y, z and n the grid point it stands on"
)]
fn crossing(a: &Arenas, v: Ix<Vertex>, p: &Plane, q: &Plane) -> Result<Option<Plane>, NumError> {
    let candidates: Vec<Plane> = match point_of(a, v) {
        Some(PointKind::Grid(g)) => {
            let [x, y, z] = [g.x.get(), g.y.get(), g.z.get()].map(i128::from);
            [([1, 0, 0], x), ([0, 1, 0], y), ([0, 0, 1], z)]
                .into_iter()
                .filter_map(|(n, at)| Plane::new(n, at.checked_neg()?).ok())
                .collect()
        }
        Some(PointKind::Meet(planes)) => planes
            .iter()
            .filter_map(|ix| a.planes.get(*ix).copied())
            .collect(),
        None => Vec::new(),
    };
    for plane in candidates {
        if normals_det(p, q, &plane)? != Sign::Zero {
            return Ok(Some(plane));
        }
    }
    Ok(None)
}

/// A plane through the edge from `start` to `end` that is not the face's: stood on the grid points
/// along an axis the face's normal has, or a plane of a meeting that holds both ends.
fn stood_on(
    a: &Arenas,
    face: &Plane,
    start: Ix<Vertex>,
    end: Ix<Vertex>,
) -> Result<Option<Plane>, NumError> {
    let same = |plane: &Plane| -> Result<bool, NumError> {
        Ok(canonical(plane)?.plane == canonical(face)?.plane)
    };
    match (point_of(a, start), point_of(a, end)) {
        (Some(PointKind::Grid(p)), Some(PointKind::Grid(q))) => {
            let [nx, ny, _] = face.n();
            let step = if nx != 0 {
                [1, 0, 0]
            } else if ny != 0 {
                [0, 1, 0]
            } else {
                [0, 0, 1]
            };
            let [sx, sy, sz] = step;
            let above = P3::new(
                p.x.get().saturating_add(sx),
                p.y.get().saturating_add(sy),
                p.z.get().saturating_add(sz),
            )?;
            Ok(Some(Plane::through(p, q, above)?))
        }
        (Some(PointKind::Meet(planes)), Some(other))
        | (Some(other), Some(PointKind::Meet(planes))) => {
            for ix in planes {
                let Some(plane) = a.planes.get(ix).copied() else {
                    continue;
                };
                if same(&plane)? {
                    continue;
                }
                let holds = match other {
                    PointKind::Grid(p) => plane.side(p)? == Sign::Zero,
                    PointKind::Meet(more) => more.contains(&ix),
                };
                if holds {
                    return Ok(Some(plane));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// The plane a fin's side lies on, positive on the inside of `face`.
fn side_of_fin(
    a: &Arenas,
    plane_of: &[Option<Plane>],
    face: &Plane,
    fin: &Fin,
    own: Ix<Fin>,
) -> Result<Option<Side>, NumError> {
    let Some(edge) = a.edges.get(fin.edge) else {
        return Ok(None);
    };
    let (Some(first), Some(last)) = (edge.start, edge.end) else {
        return Ok(None);
    };
    let (start, end) = match fin.sense {
        Sense::Forward => (first, last),
        Sense::Reversed => (last, first),
    };
    // The nearest fin around the edge on another plane.
    let mut across = None;
    let mut at = fin.radial;
    while at != own {
        let Some(other) = a.fins.get(at) else {
            break;
        };
        let held = a.loops.get(other.loop_).and_then(|l| {
            let row = usize::try_from(l.face.row()).ok()?;
            plane_of.get(row).copied().flatten()
        });
        if let Some(held) = held
            && canonical(&held)?.plane != canonical(face)?.plane
        {
            across = Some(held);
            break;
        }
        at = other.radial;
    }
    let across = match across {
        Some(plane) => plane,
        None => match stood_on(a, face, start, end)? {
            Some(plane) => plane,
            None => return Ok(None),
        },
    };
    let (Some(from), Some(to)) = (
        crossing(a, start, face, &across)?,
        crossing(a, end, face, &across)?,
    ) else {
        return Ok(None);
    };
    let line = Line::new(*face, across)?;
    // With the inside on its positive side, a side runs along n_side × n_face.
    Ok(Some(if along(&line, &from, &to)? == Ordering::Greater {
        across
    } else {
        turned(&across)?
    }))
}

/// `sides` with every run of sides on one line made one, around the ring.
fn joined(face: &Plane, sides: Vec<Side>) -> Result<Vec<Side>, NumError> {
    let mut ring: Vec<Side> = Vec::with_capacity(sides.len());
    for side in sides {
        let next = match ring.last() {
            Some(last) => normals_det(face, last, &side)? != Sign::Zero,
            None => true,
        };
        if next {
            ring.push(side);
        }
    }
    if let [first, .., last] = ring.as_slice()
        && normals_det(face, first, last)? == Sign::Zero
    {
        ring.pop();
    }
    Ok(ring)
}

/// `operand`'s polygons added to `ready`, with their carriers, its owner and its faces.
fn polygons_of(
    operand: &Operand<'_>,
    entry: Option<Entry>,
    owner: usize,
    ready: &mut Prepared,
    meter: &mut WorkMeter<'_>,
) -> Result<(), PrepareError> {
    let a = operand.body.arenas();
    let not_flat = PrepareError::NotPlaneBased(operand.key);
    let broken = PrepareError::Broken(operand.key);
    let faces = a.faces.len();
    let mut planes: Vec<Option<Plane>> = Vec::new();
    let mut rows: Vec<Face> = Vec::new();
    for row in 0..faces {
        let ix = Ix::<Face>::new(row).ok_or(broken)?;
        let face = a.faces.get(ix).ok_or(broken)?;
        let surface = a.surfaces.get(face.surface).ok_or(broken)?;
        let plane = face_plane(surface, face.sense).ok_or(not_flat)??;
        planes.push(Some(plane));
        rows.push(face);
    }
    for (face, plane) in rows.iter().zip(&planes) {
        let plane = plane.as_ref().ok_or(broken)?;
        let mut rings: Vec<Vec<Side>> = Vec::new();
        let mut at_loop = face.first_loop;
        while let Some(l) = at_loop {
            let held = a.loops.get(l).ok_or(broken)?;
            let start = held.first_fin.ok_or(broken)?;
            let mut sides = Vec::new();
            let mut at = start;
            loop {
                let fin = a.fins.get(at).ok_or(broken)?;
                sides.push(side_of_fin(a, &planes, plane, &fin, at)?.ok_or(broken)?);
                at = fin.next;
                if at == start {
                    break;
                }
            }
            rings.push(joined(plane, sides)?);
            at_loop = held.next;
        }
        let surface = a.surfaces.get(face.surface).ok_or(broken)?;
        let pieces: Vec<Piece> = convex_pieces(plane, &rings, meter)?;
        let source = ready
            .sources
            .last()
            .map_or(0, |last| last.saturating_add(1));
        for piece in pieces {
            ready.polygons.push(Polygon { piece, entry });
            ready.carriers.push(Carrier {
                surface: surface.clone(),
                sense: face.sense,
            });
            ready.owners.push(owner);
            ready.sources.push(source);
        }
    }
    Ok(())
}

/// The operand `selection` names as its target, as a place in `entries`.
fn target_of(
    selection: &Selection,
    entries: &[(OperandKey, Option<Entry>)],
) -> Result<Option<usize>, PrepareError> {
    let key = match selection {
        Selection::Subtract { target }
        | Selection::Imprint { target }
        | Selection::Split { target }
        | Selection::Keep { target, .. }
        | Selection::Trim { target, .. } => target,
        Selection::Unite | Selection::Intersect | Selection::Where(_) | Selection::Section => {
            return Ok(None);
        }
    };
    entries
        .iter()
        .position(|(k, _)| k == key)
        .map(Some)
        .ok_or(PrepareError::UnknownOperand(*key))
}

/// The test of `selection`, given each solid's entry.
fn test_of(
    selection: &Selection,
    entries: &[(OperandKey, Option<Entry>)],
    solids: u32,
) -> Result<WindingTest, PrepareError> {
    let entry_of = |key: &OperandKey| {
        entries
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, entry)| *entry)
            .ok_or(PrepareError::UnknownOperand(*key))
    };
    Ok(match selection {
        Selection::Unite | Selection::Trim { .. } | Selection::Section => WindingTest::unite(),
        Selection::Intersect => WindingTest::intersect(solids),
        Selection::Subtract { target } => WindingTest::subtract(entry_of(target)?),
        Selection::Imprint { target } | Selection::Split { target } => {
            WindingTest::new(alloc::vec![Inner::Inside(entry_of(target)?)])
                .map_err(PrepareError::BadTest)?
        }
        Selection::Where(terms) | Selection::Keep { test: terms, .. } => {
            let mut inner = Vec::with_capacity(terms.len());
            for term in terms {
                inner.push(match term {
                    Term::Inside(key) => Inner::Inside(entry_of(key)?),
                    Term::AtLeast(n) => Inner::AtLeast(*n),
                    Term::And => Inner::And,
                    Term::Or => Inner::Or,
                    Term::Not => Inner::Not,
                });
            }
            WindingTest::new(inner).map_err(PrepareError::BadTest)?
        }
    })
}

/// `input`, checked, in key order, its faces polygons and its selections tests.
///
/// # Errors
///
/// A [`PrepareError`].
pub fn prepare(
    input: &BooleanInput<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Prepared, PrepareError> {
    let mut order: Vec<&Operand<'_>> = input.operands.iter().collect();
    order.sort_by_key(|o| o.key);
    for pair in order.windows(2) {
        if let [a, b] = pair
            && a.key == b.key
        {
            return Err(PrepareError::DuplicateOperand(a.key));
        }
    }
    let mut keys = Vec::with_capacity(order.len());
    let mut solids = 0u32;
    for operand in &order {
        let entry = (operand.kind == OperandKind::Solid).then_some(solids);
        solids = solids.saturating_add(u32::from(entry.is_some()));
        keys.push((operand.key, entry));
    }
    let mut tests = Vec::with_capacity(input.select.len());
    let mut targets = Vec::with_capacity(input.select.len());
    for selection in &input.select {
        tests.push(test_of(selection, &keys, solids)?);
        targets.push(target_of(selection, &keys)?);
    }
    let entries: Vec<Option<Entry>> = keys.iter().map(|(_, entry)| *entry).collect();
    let mut ready = Prepared {
        keys,
        polygons: Vec::new(),
        carriers: Vec::new(),
        owners: Vec::new(),
        sources: Vec::new(),
        tests,
        targets,
    };
    for (owner, (operand, entry)) in order.iter().zip(entries).enumerate() {
        polygons_of(operand, entry, owner, &mut ready, meter)?;
    }
    Ok(ready)
}
