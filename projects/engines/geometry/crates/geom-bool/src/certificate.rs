//! The arrangement certificate: what a result must show before it is returned (11.10).
//!
//! What it does: [`certify`] holds a rebuilt body to chapter 11.10's closure checks, cheap enough
//! to run on every result: every edge's fins pair up, in radial order alternating which way they
//! run, so each wedge around an edge is one cell; no edge has zero length; and every face has an
//! area. [`labelled`] holds the pieces of a winding selection to the last: each separates two cells
//! the test tells apart, the result's material behind it. Planes within budget need no check: a
//! plane outside its budget cannot be made. A sheet's edge may have one fin, and a wire has no
//! fins at all, so only bodies that bound something pair up.
//!
//! A transaction needs no code: operands are shared borrows, a [`Body`] is immutable, and a failure
//! returns no body, so a failed or cancelled boolean leaves the model exactly as it was.
//!
//! Guarantees:
//!
//! - **Reports, never repairs.** A fault names what broke and the row it broke at; the body is not
//!   changed.
//! - **The first fault in a fixed order.** Fins by edge, then edges, then faces, then pieces, each
//!   ascending, the same on every machine.
//!
//! Method: chapter 11.10 (`projects/engines/geometry/architecture/11-booleans.md`), after the
//! closure certificate hypermesh runs before publishing a mesh
//! (<https://github.com/timschmidt/csgrs/blob/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586/crates/hypermesh/src/surface_arrangement.rs#L4336-L4456>).
//!
//! Cost: linear in the body's fins, and in the pieces. Charges the meter one unit per edge, face
//! and piece.

use core::fmt;

use geom_poly::winding::WindingTest;
use geom_run::{Stopped, WorkMeter};
use geom_topo::{Arenas, Body, BodyKind, Ix, Sense};

use geom_poly::merge::Outward;

/// What a result failed to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// An edge's fins do not pair up: an odd number, or two neighbours running the same way.
    Fins {
        /// The edge's row.
        edge: u32,
    },
    /// An edge starts where it ends, or has no ends.
    ZeroLength {
        /// The edge's row.
        edge: u32,
    },
    /// A face has a loop of fewer than three fins, or no loop.
    ZeroArea {
        /// The face's row.
        face: u32,
    },
    /// A kept piece does not separate two cells the test tells apart.
    Label {
        /// The piece's place.
        piece: usize,
    },
    /// The meter stopped the run.
    Stopped(Stopped),
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Fins { edge } => write!(f, "edge {edge}: fins that do not pair up"),
            Fault::ZeroLength { edge } => write!(f, "edge {edge} has no length"),
            Fault::ZeroArea { face } => write!(f, "face {face} has no area"),
            Fault::Label { piece } => write!(f, "piece {piece} separates nothing"),
            Fault::Stopped(_) => f.write_str("the meter stopped the certificate"),
        }
    }
}

impl core::error::Error for Fault {}

impl From<Stopped> for Fault {
    fn from(e: Stopped) -> Self {
        Fault::Stopped(e)
    }
}

/// Whether `edge`'s fins alternate around it, an even number of them.
fn pairs_up(a: &Arenas, edge: u32) -> Option<bool> {
    let row = a.edges.get(Ix::new(edge)?)?;
    let first = row.first_fin?;
    let mut at = first;
    let mut count = 0u32;
    loop {
        let fin = a.fins.get(at)?;
        let next = a.fins.get(fin.radial)?;
        count = count.saturating_add(1);
        if fin.radial != at && same_way(fin.sense, next.sense) {
            return Some(false);
        }
        at = fin.radial;
        if at == first {
            return Some(count.checked_rem(2) == Some(0));
        }
    }
}

/// Whether two fins run the same way along their edge.
const fn same_way(a: Sense, b: Sense) -> bool {
    matches!(
        (a, b),
        (Sense::Forward, Sense::Forward) | (Sense::Reversed, Sense::Reversed)
    )
}

/// Whether `face` has a loop and every loop of it at least three fins.
fn has_area(a: &Arenas, face: u32) -> Option<bool> {
    let mut at_loop = a.faces.get(Ix::new(face)?)?.first_loop;
    if at_loop.is_none() {
        return Some(false);
    }
    while let Some(ix) = at_loop {
        let held = a.loops.get(ix)?;
        let first = held.first_fin?;
        let mut at = a.fins.get(first)?.next;
        let mut count = 1u32;
        while at != first {
            count = count.saturating_add(1);
            at = a.fins.get(at)?.next;
        }
        if count < 3 {
            return Some(false);
        }
        at_loop = held.next;
    }
    Some(true)
}

/// Holds `body` to chapter 11.10's closure checks.
///
/// # Errors
///
/// The first [`Fault`].
pub fn certify(body: &Body, meter: &mut WorkMeter<'_>) -> Result<(), Fault> {
    let a = body.arenas();
    let bounds = matches!(body.kind(), BodyKind::Solid | BodyKind::General);
    for edge in 0..a.edges.len() {
        meter.charge(1)?;
        if bounds && pairs_up(a, edge) != Some(true) {
            return Err(Fault::Fins { edge });
        }
        let row = Ix::new(edge).and_then(|ix| a.edges.get(ix));
        let ends = row.and_then(|e| e.start.zip(e.end));
        if ends.is_none_or(|(start, end)| start == end) {
            return Err(Fault::ZeroLength { edge });
        }
    }
    for face in 0..a.faces.len() {
        meter.charge(1)?;
        if has_area(a, face) != Some(true) {
            return Err(Fault::ZeroArea { face });
        }
    }
    Ok(())
}

/// Holds the `kept` pieces of a winding selection to `test`: outside in front, inside behind.
///
/// # Errors
///
/// The first [`Fault`].
pub fn labelled(
    kept: &[Outward],
    test: &WindingTest,
    meter: &mut WorkMeter<'_>,
) -> Result<(), Fault> {
    for (piece, held) in kept.iter().enumerate() {
        meter.charge(1)?;
        if test.holds(&held.front) || !test.holds(&held.back) {
            return Err(Fault::Label { piece });
        }
    }
    Ok(())
}
