//! Continuity across an edge (16.5; piece G346): a certified maximum of G0, G1, G2 or G3 along the
//! whole edge, refined until it is as tight as asked, with the parameter where it is reached.
//!
//! What it does: [`join`] measures a join of two faces along their pcurves, which share the
//! join's parameter t (`geom_surface::class_a`, which names and defines the measures); [`edge`]
//! reads that join from a body's edge with exactly two fins, each fin's face giving its surface
//! and sense and the fin its pcurve. The maximum is found by branch and bound on t: the range is
//! cut at every break of either pcurve, and each piece is measured at its middle (a value that
//! some t really has, the witness candidates) and over its whole span (an upper bound, the
//! centred form about the middle). A piece whose upper bound is no more than the best witness
//! cannot hold anything larger and is dropped; a piece whose bound is more than [`Ask::tight`]
//! above it is halved; the rest are kept. It stops when no piece needs halving, when every piece
//! that does is a single double wide, or when the next round would pass [`Ask::pieces`].
//!
//! Guarantees:
//!
//! - **Certified.** The largest value of the measure over the join lies in [`Maximum::lower`],
//!   [`Maximum::upper`]: the lower end is the enclosure's lower end at the witness parameter,
//!   the upper end the largest upper bound of any piece kept (or the witness's, if larger), each
//!   from `class_a`'s enclosures, so never a sample of points (16.5).
//! - **As tight as asked, or said not to be.** [`Maximum::as_asked`] says whether the two ends
//!   are within [`Ask::tight`]; a run that stopped at its cap of pieces still returns certified
//!   ends, only wider.
//! - **Undefined spans are reported.** Where the measure is not defined (a normal or tangent that
//!   may vanish: a pole, a degenerate point) and halving down to single doubles never bounds it,
//!   the upper end is [`Upper::Unresolved`], naming the span, never a guess.
//! - **Thresholds are inputs.** The tightness and the cap are the caller's; nothing is
//!   hard-coded but the default cap, [`Ask::PIECES`].
//! - **Deterministic.** Pieces are measured in increasing order of t, round by round, and the
//!   witness changes only to a strictly larger lower end, so the same inputs give the same bits.
//! - **Charged.** One unit of work for each piece measured, with the work `class_a`'s
//!   enclosures charge, and each piece's logical bytes as it is kept; a stop is
//!   [`ContinuityError::Stopped`].
//!
//! Method: interval branch and bound for a global maximum (R. E. Moore, R. B. Kearfott and
//! M. J. Cloud, *Introduction to Interval Analysis*, SIAM (2009), chapter 9,
//! <https://doi.org/10.1137/1.9780898717716>; E. Hansen and G. W. Walster, *Global Optimization
//! Using Interval Analysis*, 2nd ed., Marcel Dekker (2004), <https://doi.org/10.1201/9780203026922>),
//! the midpoint test giving the bound pieces are dropped against, on the measures of chapter
//! 16.5 (`projects/engines/geometry/architecture/16-free-form.md`).
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/db9f4ae1850b9bb9faabed46ca86390c242dedd3/crates/solidcore/src/query/continuity.rs>,
//!   for `continuity_along`, which measures an edge at even stations and keeps the stations that
//!   answer; replaced here by a certified bound over every piece. No code copied.
//!
//! Cost: each piece is one `class_a::at` and one `class_a::over` (O(1) each); a join of smooth
//! surfaces whose measure is not constant needs O(log(1/tight)) rounds of a few pieces near the
//! maximum, one whose measure is the same all along (an exact join) O(√(L/tight)) pieces, L the
//! length of t's range; never more than [`Ask::pieces`]. Memory: the pieces kept. No recursion.

use alloc::vec::Vec;
use core::fmt;

use geom_num::interval::Interval;
use geom_run::{LogicalSize, Stopped, WorkMeter};
use geom_surface::class_a::{self, CurveError, Limit, Measure, Parts, Side};
use geom_topo::Body;
use geom_topo::entity::{EdgeIx, FinIx, Sense};

/// What to measure, how tight, and how many pieces to measure at most.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ask {
    measure: Measure,
    tight: f64,
    pieces: usize,
}

impl Ask {
    /// The default cap on the pieces measured: 2¹⁴.
    pub const PIECES: usize = 16_384;

    /// Measure `measure` until the maximum's two ends are within `tight` of each other (in the
    /// measure's own unit), measuring at most [`Ask::PIECES`] pieces.
    ///
    /// # Errors
    ///
    /// [`ContinuityError::Tight`] unless `tight` is finite and not negative.
    pub fn new(measure: Measure, tight: f64) -> Result<Ask, ContinuityError> {
        if !(tight.is_finite() && tight >= 0.0) {
            return Err(ContinuityError::Tight);
        }
        Ok(Ask {
            measure,
            tight,
            pieces: Ask::PIECES,
        })
    }

    /// The same ask, measuring at most `pieces` pieces.
    #[must_use]
    pub const fn with_pieces(self, pieces: usize) -> Ask {
        Ask { pieces, ..self }
    }

    /// The measure.
    #[must_use]
    pub const fn measure(&self) -> Measure {
        self.measure
    }

    /// How close the maximum's two ends are asked to be.
    #[must_use]
    pub const fn tight(&self) -> f64 {
        self.tight
    }

    /// The cap on the pieces measured.
    #[must_use]
    pub const fn pieces(&self) -> usize {
        self.pieces
    }
}

/// A parameter of the join where the measure is enclosed: the maximum is at least its lower end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Witness {
    /// The join's parameter.
    pub at: f64,
    /// An enclosure of the measure there.
    pub value: Interval,
}

/// The upper end of a maximum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Upper {
    /// The measure is at most this everywhere on the join.
    Bounded(f64),
    /// The measure could not be bounded over this span of the parameter (it may not be defined
    /// there), so the join has no upper bound.
    Unresolved {
        /// Where the span starts.
        from: f64,
        /// Where it ends.
        to: f64,
    },
}

/// The certified maximum of a measure along a join.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Maximum {
    /// What was measured.
    pub measure: Measure,
    /// The parameter where the largest lower end was found, with the enclosure there; `None` if
    /// the measure was defined at no parameter measured.
    pub witness: Option<Witness>,
    /// The upper end.
    pub upper: Upper,
    /// Whether the two ends are within the tightness asked.
    pub as_asked: bool,
    /// How many pieces were measured.
    pub pieces: usize,
}

impl Maximum {
    /// The lower end: the witness's lower end, or 0 (every measure is at least 0) without one.
    #[must_use]
    pub fn lower(&self) -> f64 {
        self.witness.map_or(0.0, |w| w.value.lo())
    }
}

/// Why a join could not be measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuityError {
    /// A tightness that is not finite, or is negative.
    Tight,
    /// The two pcurves run over different parameter ranges, so they do not share the join's
    /// parameter.
    Mismatch,
    /// The edge has not exactly two fins (a boundary edge or a non-manifold one).
    NotAJoin(EdgeIx),
    /// A fin of the edge has no pcurve.
    NoPcurve(FinIx),
    /// A link into a row its arena does not have; a published body never holds one.
    NoSuchRow,
    /// A surface or pcurve could not be evaluated (a pcurve outside its surface's domain, a
    /// value past the doubles).
    Geometry(CurveError),
    /// The budget ran out or the run was cancelled.
    Stopped(Stopped),
}

impl fmt::Display for ContinuityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContinuityError::Tight => f.write_str("a tightness that is not finite or is negative"),
            ContinuityError::Mismatch => {
                f.write_str("the two pcurves run over different parameter ranges")
            }
            ContinuityError::NotAJoin(e) => write!(f, "edge {} has not exactly two fins", e.row()),
            ContinuityError::NoPcurve(fin) => write!(f, "fin {} has no pcurve", fin.row()),
            ContinuityError::NoSuchRow => f.write_str("a link into a row its arena does not have"),
            ContinuityError::Geometry(e) => write!(f, "the geometry could not be evaluated: {e}"),
            ContinuityError::Stopped(s) => write!(f, "{s}"),
        }
    }
}

impl core::error::Error for ContinuityError {}

impl From<Stopped> for ContinuityError {
    fn from(s: Stopped) -> Self {
        ContinuityError::Stopped(s)
    }
}

impl From<CurveError> for ContinuityError {
    fn from(e: CurveError) -> Self {
        match e {
            CurveError::Stopped(s) => ContinuityError::Stopped(s),
            other => ContinuityError::Geometry(other),
        }
    }
}

/// A span of the parameter and its upper bound (`None`: not bounded).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    lo: f64,
    hi: f64,
    upper: Option<f64>,
}

impl LogicalSize for Piece {
    const LOGICAL_BYTES: u64 = 24;
}

/// Pushes `piece`, its logical bytes charged first.
fn keep(list: &mut Vec<Piece>, piece: Piece, meter: &mut WorkMeter<'_>) -> Result<(), Stopped> {
    meter.reserve_for::<Piece>(1)?;
    if list.try_reserve(1).is_err() {
        return Err(meter.out_of_memory());
    }
    list.push(piece);
    Ok(())
}

/// The certified maximum of `ask`'s measure along the join `sides`, whose pcurves share one
/// parameter range.
///
/// # Errors
///
/// [`ContinuityError::Mismatch`] for pcurves over different ranges; [`ContinuityError::Geometry`]
/// for a pcurve outside its surface's domain or a value past the doubles;
/// [`ContinuityError::Stopped`].
pub fn join(
    sides: [Side<'_>; 2],
    ask: Ask,
    meter: &mut WorkMeter<'_>,
) -> Result<Maximum, ContinuityError> {
    let [a, b] = sides;
    let (lo, hi) = a.domain();
    if (lo, hi) != b.domain() {
        return Err(ContinuityError::Mismatch);
    }
    let measure = ask.measure;
    let mut best: Option<Witness> = None;
    for (t, limit) in [(lo, Limit::FromRight), (hi, Limit::FromLeft)] {
        if let Some(p) = point(measure, sides, t, limit, meter)? {
            best = Some(better(best, t, p.value()?));
        }
    }
    let mut queue = first_pieces(sides, (lo, hi), meter)?;
    let mut live: Vec<Piece> = Vec::new();
    let mut measured = 0_usize;
    loop {
        for piece in queue.drain(..) {
            measured = measured.saturating_add(1);
            meter.charge(1)?;
            let (lo, hi) = (piece.lo, piece.hi);
            let c = f64::midpoint(lo, hi);
            let centre = point(measure, sides, c, Limit::FromRight, meter)?;
            if let Some(p) = &centre {
                best = Some(better(best, c, p.value()?));
            }
            let upper = match class_a::over(
                measure,
                sides,
                (lo, hi),
                centre.as_ref().map(|p| (c, p)),
                meter,
            ) {
                Ok(p) => Some(p.value()?.hi()),
                Err(CurveError::Unbounded) => None,
                Err(e) => return Err(e.into()),
            };
            keep(&mut live, Piece { lo, hi, upper }, meter)?;
        }
        let floor = best.map_or(0.0, |w| w.value.lo());
        live.retain(|p| p.upper.is_none_or(|u| u > floor));
        let wide = |p: &Piece| p.upper.is_none_or(|u| u - floor > ask.tight);
        let halves = live
            .iter()
            .filter(|p| wide(p) && splits(p))
            .count()
            .saturating_mul(2);
        if halves == 0 || measured.saturating_add(halves) > ask.pieces {
            break;
        }
        let mut rest: Vec<Piece> = Vec::new();
        for p in live.drain(..) {
            if wide(&p) && splits(&p) {
                let c = f64::midpoint(p.lo, p.hi);
                keep(&mut queue, Piece { hi: c, ..p }, meter)?;
                keep(&mut queue, Piece { lo: c, ..p }, meter)?;
            } else {
                keep(&mut rest, p, meter)?;
            }
        }
        live = rest;
    }
    Ok(finish(measure, best, &live, ask.tight, measured))
}

/// The join's range cut at each break of either pcurve strictly inside it, as pieces to measure.
fn first_pieces(
    [a, b]: [Side<'_>; 2],
    (lo, hi): (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Piece>, ContinuityError> {
    let mut cuts: Vec<f64> = Vec::new();
    for t in a.breaks().into_iter().chain(b.breaks()) {
        if lo < t && t < hi {
            meter.reserve_for::<Piece>(1)?;
            cuts.try_reserve(1).map_err(|_| meter.out_of_memory())?;
            cuts.push(t);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut queue: Vec<Piece> = Vec::new();
    let mut from = lo;
    for t in cuts.into_iter().chain(core::iter::once(hi)) {
        keep(
            &mut queue,
            Piece {
                lo: from,
                hi: t,
                upper: None,
            },
            meter,
        )?;
        from = t;
    }
    Ok(queue)
}

/// The maximum from the best witness and the pieces kept: the upper end is the largest of their
/// upper bounds and the witness's, or the first unbounded piece's span.
fn finish(
    measure: Measure,
    best: Option<Witness>,
    live: &[Piece],
    tight: f64,
    measured: usize,
) -> Maximum {
    let floor = best.map_or(0.0, |w| w.value.lo());
    let mut upper = Upper::Bounded(best.map_or(0.0, |w| w.value.hi()));
    for p in live {
        upper = match (upper, p.upper) {
            (Upper::Unresolved { .. }, _) => upper,
            (_, None) => Upper::Unresolved {
                from: p.lo,
                to: p.hi,
            },
            (Upper::Bounded(u), Some(v)) => Upper::Bounded(if v > u { v } else { u }),
        };
    }
    let as_asked = match upper {
        Upper::Bounded(u) => u - floor <= tight,
        Upper::Unresolved { .. } => false,
    };
    Maximum {
        measure,
        witness: best,
        upper,
        as_asked,
        pieces: measured,
    }
}

/// Whether a piece can be halved: its middle lies strictly between its ends.
fn splits(p: &Piece) -> bool {
    let c = f64::midpoint(p.lo, p.hi);
    p.lo < c && c < p.hi
}

/// The measure's parts at `t`, `None` where it is not defined there.
fn point(
    measure: Measure,
    sides: [Side<'_>; 2],
    t: f64,
    limit: Limit,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Parts>, ContinuityError> {
    match class_a::at(measure, sides, t, limit, meter) {
        Ok(p) => Ok(Some(p)),
        Err(CurveError::Unbounded) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The witness with the larger lower end, the first kept on a tie.
fn better(best: Option<Witness>, at: f64, value: Interval) -> Witness {
    match best {
        Some(w) if w.value.lo() >= value.lo() => w,
        _ => Witness { at, value },
    }
}

/// The certified maximum of `ask`'s measure across edge `edge` of `body`: the join of its two
/// fins' faces, each face's normal pointing out of its material (turned for a face of reversed
/// sense), along the fins' pcurves, which run on the edge's parameter.
///
/// # Errors
///
/// [`ContinuityError::NotAJoin`] for an edge with one fin or more than two;
/// [`ContinuityError::NoPcurve`] for a fin without a pcurve; [`ContinuityError::NoSuchRow`]; and
/// [`join`]'s errors.
pub fn edge(
    body: &Body,
    edge: EdgeIx,
    ask: Ask,
    meter: &mut WorkMeter<'_>,
) -> Result<Maximum, ContinuityError> {
    let a = body.arenas();
    let first = a
        .edges
        .get(edge)
        .ok_or(ContinuityError::NoSuchRow)?
        .first_fin
        .ok_or(ContinuityError::NotAJoin(edge))?;
    let second = a.fins.get(first).ok_or(ContinuityError::NoSuchRow)?.radial;
    let back = a.fins.get(second).ok_or(ContinuityError::NoSuchRow)?.radial;
    if second == first || back != first {
        return Err(ContinuityError::NotAJoin(edge));
    }
    let side = |fin: FinIx| -> Result<Side<'_>, ContinuityError> {
        let f = a.fins.get(fin).ok_or(ContinuityError::NoSuchRow)?;
        let face = a.loops.get(f.loop_).ok_or(ContinuityError::NoSuchRow)?.face;
        let face = a.faces.get(face).ok_or(ContinuityError::NoSuchRow)?;
        let surface = a
            .surfaces
            .get(face.surface)
            .ok_or(ContinuityError::NoSuchRow)?;
        let pcurve = f
            .pcurve
            .ok_or(ContinuityError::NoPcurve(fin))
            .and_then(|p| a.pcurves.get(p).ok_or(ContinuityError::NoSuchRow))?;
        Ok(Side {
            surface,
            pcurve,
            turned: face.sense == Sense::Reversed,
        })
    };
    join([side(first)?, side(second)?], ask, meter)
}
