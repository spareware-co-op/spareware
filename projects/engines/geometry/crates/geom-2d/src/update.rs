//! Arrangement updates and point location (chapter 14.4): zone insertion into a drawing kept in
//! shared chunks, overlay, and the exact question "where is this point".
//!
//! What it does: a [`Live`] is an arrangement of segments that can be changed. [`Live::insert`]
//! adds one segment and returns a new version, touching only the pieces the segment meets (its
//! zone); every chunk of pieces the segment does not touch is shared, by reference, with the
//! version before, which stays valid. [`Live::overlay`] puts one `Live` over another. [`Live::locate`]
//! says whether a point is a vertex, on an edge, or in a face, and names the face by a half-edge of
//! its boundary; [`Live::face_cycle`] walks that boundary. [`Live::batch`] builds the same thing
//! from a list at once, by [`crate::arrange::arrange`], and is the reference every update is held to.
//!
//! Guarantees:
//!
//! - **Incremental equals batch.** Inserting segments one by one, in any order, gives the pieces,
//!   with the same sources, that [`crate::arrange::arrange`] gives for the same segments; an overlay
//!   gives those of the union. Vertices are the end points and the exact crossings, touching splits,
//!   and pieces on one line share the edges of their common stretch. [`Live::same_as`] is the
//!   comparison.
//! - **Exact.** Every decision is a sign of one of `geom-num`'s exact predicates on carriers and
//!   rational points. A piece keeps the carrier of the segments along it, so a split never moves
//!   its line, and a crossing is the point where two original lines meet. A step past the exact
//!   budget is [`ArrangeError::Range`], never a wrong answer.
//! - **Old versions stay.** An insert does not change `self`; the chunks it does not touch are the
//!   same chunks, which [`Live::shared_chunks`] counts.
//! - **Located exactly.** A point on a vertex or an edge is reported as that vertex or edge; a
//!   point in a face is reported by the half-edge of the boundary under it, with the point on its
//!   left, found by shooting a ray straight down and, at a vertex, by the exact order of the edges
//!   around it. A point with nothing below it is in the unbounded face.
//! - **Cancellable.** Every insert and every location charges its [`WorkMeter`] for the pieces it
//!   examines.
//!
//! Method: the zone of a curve in an arrangement (Edelsbrunner, Guibas and Sharir 1990,
//! doi:10.1007/BF02187681) is the set of faces it meets; inserting walks it, splits what it crosses
//! and merges what it overlaps, as Fogel, Halperin and Wein, *CGAL Arrangements and Their
//! Applications*, Springer 2012, chapter 1 describe the operation, with the chunked version-sharing
//! of a persistent sorted sequence. Here the walk finds the zone by chunk boxes and not by walking
//! face to face: a chunk whose box misses the segment's box is not opened. The ray shot down is the
//! vertical ray shooting of de Berg, Cheong, van Kreveld and Overmars, *Computational Geometry*,
//! 3rd ed., chapter 6, done by the chunk boxes rather than a slab search tree.
//!
//! Cost: an insert opens the chunks whose boxes meet the segment's box (all chunk boxes are
//! compared, one comparison per chunk, c pieces per chunk) and splits the pieces the segment
//! meets; the new version copies only the chunks it changes and the list of chunk pointers. A
//! location opens the chunks over the point's column. The face cycle is linear in the face's
//! boundary. Batch building is the sweep's cost, O((n + k) log n).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "s, e, p, q, m, t, u, v are the geometry's own letters, and the insert and the location are each one procedure that reads best whole"
)]

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;

use geom_num::Sign;
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::P2;
use geom_run::WorkMeter;

use crate::arrange::{ArrangeError, arrange};

/// Pieces a chunk is split into halves at twice this many.
const CHUNK: usize = 256;

/// A box in whole steps, `[min x, min y, max x, max y]`, each the floor of the exact coordinate.
/// The floor never decreases, so two boxes whose exact ranges overlap still meet once floored: a
/// box skips only what cannot meet.
type Box4 = [i128; 4];

fn sign_order(s: Sign) -> Ordering {
    match s {
        Sign::Negative => Ordering::Less,
        Sign::Zero => Ordering::Equal,
        Sign::Positive => Ordering::Greater,
    }
}

/// Lexicographic order of (x, y).
fn cmp_pt(a: &Rational2, b: &Rational2) -> Result<Ordering, ArrangeError> {
    match sign_order(a.cmp_x(b)?) {
        Ordering::Equal => Ok(sign_order(a.cmp_y(b)?)),
        other => Ok(other),
    }
}

fn mul(a: i128, b: i128) -> Result<i128, ArrangeError> {
    a.checked_mul(b).ok_or(ArrangeError::Range)
}

fn sub(a: i128, b: i128) -> Result<i128, ArrangeError> {
    a.checked_sub(b).ok_or(ArrangeError::Range)
}

fn add(a: i128, b: i128) -> Result<i128, ArrangeError> {
    a.checked_add(b).ok_or(ArrangeError::Range)
}

/// The cross product of two direction vectors, as an order.
fn cross_sign(a: [i128; 2], b: [i128; 2]) -> Result<Ordering, ArrangeError> {
    Ok(mul(a[0], b[1])?.cmp(&mul(a[1], b[0])?))
}

/// Whether `t` lies between `lo` and `hi`, ends included, for three points on one line.
fn between(lo: &Rational2, t: &Rational2, hi: &Rational2) -> Result<bool, ArrangeError> {
    Ok(cmp_pt(lo, t)? != Ordering::Greater && cmp_pt(t, hi)? != Ordering::Greater)
}

/// The floor of `v / w`; `w` is a weight, so positive, and the division cannot fail.
fn floor(v: i128, w: i128) -> i128 {
    v.checked_div_euclid(w).unwrap_or(0)
}

/// The box of the segment `lo`..`hi` in whole steps (see [`Box4`]).
fn box_of(lo: &Rational2, hi: &Rational2) -> Box4 {
    let ([lx, ly], lw) = lo.coordinates();
    let ([hx, hy], hw) = hi.coordinates();
    let (x0, y0, x1, y1) = (floor(lx, lw), floor(ly, lw), floor(hx, hw), floor(hy, hw));
    [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]
}

fn meets(a: &Box4, b: &Box4) -> bool {
    let [ax0, ay0, ax1, ay1] = *a;
    let [bx0, by0, bx1, by1] = *b;
    ax0 <= bx1 && bx0 <= ax1 && ay0 <= by1 && by0 <= ay1
}

fn join(a: &Box4, b: &Box4) -> Box4 {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// A piece of the drawing: an edge between two vertices, lowest first, with the segments along it.
#[derive(Clone, Debug)]
pub struct Piece {
    lo: Rational2,
    hi: Rational2,
    line: Carrier,
    sources: Arc<[u32]>,
}

impl Piece {
    /// The lexicographically lower end.
    #[must_use]
    pub fn lo(&self) -> &Rational2 {
        &self.lo
    }

    /// The higher end.
    #[must_use]
    pub fn hi(&self) -> &Rational2 {
        &self.hi
    }

    /// The labels of the segments that cover the piece, ascending.
    #[must_use]
    pub fn sources(&self) -> &[u32] {
        &self.sources
    }

    /// The line the piece lies on, running from `lo` to `hi`.
    #[must_use]
    pub fn line(&self) -> &Carrier {
        &self.line
    }

    fn bounds(&self) -> Box4 {
        box_of(&self.lo, &self.hi)
    }
}

/// A run of pieces in key order, with a box round them all. A chunk is never empty: one that
/// loses its last piece is dropped.
#[derive(Clone, Debug)]
struct Chunk {
    pieces: Vec<Piece>,
    bounds: Box4,
}

impl Chunk {
    fn of(pieces: Vec<Piece>) -> Chunk {
        let bounds = pieces
            .iter()
            .map(Piece::bounds)
            .reduce(|a, b| join(&a, &b))
            .unwrap_or([0; 4]);
        Chunk { pieces, bounds }
    }
}

/// How much work a change did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    /// The chunks whose boxes met the segment, so that their pieces were looked at.
    pub chunks_opened: u64,
    /// The pieces looked at.
    pub examined: u64,
    /// The pieces that were split.
    pub split: u64,
    /// The pieces the segment added or whose sources it extended.
    pub written: u64,
    /// The chunks copied because the new version changed them, and the halves it split off chunks
    /// that grew past twice their size.
    pub chunks_copied: u64,
}

/// What lies at a point.
#[derive(Clone, Debug)]
pub enum Location {
    /// The point is this vertex.
    Vertex(Rational2),
    /// The point is inside the edge between these vertices, the lower first.
    Edge(Rational2, Rational2),
    /// The point is inside a face, which has this half-edge on its boundary, or is in the unbounded
    /// face when there is none.
    Face(Option<Half>),
}

/// A directed edge: the face on its left is the face meant.
#[derive(Clone, Debug)]
pub struct Half {
    /// Where it starts.
    pub from: Rational2,
    /// Where it ends.
    pub to: Rational2,
}

/// An arrangement of segments that can be changed, in chunks shared between versions.
#[derive(Clone, Debug, Default)]
pub struct Live {
    chunks: Vec<Arc<Chunk>>,
    len: usize,
}

/// A version being edited: the chunks, each copied the first time the edit changes it.
struct Edit {
    chunks: Vec<Arc<Chunk>>,
    len: usize,
    cost: Cost,
}

fn cmp_key(p: &Piece, lo: &Rational2, hi: &Rational2) -> Result<Ordering, ArrangeError> {
    match cmp_pt(&p.lo, lo)? {
        Ordering::Equal => cmp_pt(&p.hi, hi),
        other => Ok(other),
    }
}

/// Where the key `lo`..`hi` is among `chunks`: the chunk, the slot in it, and whether it is there.
fn find(
    chunks: &[Arc<Chunk>],
    lo: &Rational2,
    hi: &Rational2,
) -> Result<(usize, usize, bool), ArrangeError> {
    // The last chunk whose first piece is not above the key.
    let (mut a, mut b) = (0_usize, chunks.len());
    while a < b {
        let mid = a.saturating_add(b.saturating_sub(a) / 2);
        let first = chunks
            .get(mid)
            .and_then(|c| c.pieces.first())
            .ok_or(ArrangeError::TooMany)?;
        if cmp_key(first, lo, hi)? == Ordering::Greater {
            b = mid;
        } else {
            a = mid.saturating_add(1);
        }
    }
    let c = a.saturating_sub(1);
    let Some(chunk) = chunks.get(c) else {
        return Ok((0, 0, false));
    };
    let (mut a, mut b) = (0_usize, chunk.pieces.len());
    while a < b {
        let mid = a.saturating_add(b.saturating_sub(a) / 2);
        let p = chunk.pieces.get(mid).ok_or(ArrangeError::TooMany)?;
        match cmp_key(p, lo, hi)? {
            Ordering::Less => a = mid.saturating_add(1),
            Ordering::Equal => return Ok((c, mid, true)),
            Ordering::Greater => b = mid,
        }
    }
    Ok((c, a, false))
}

impl Edit {
    fn of(live: &Live) -> Edit {
        Edit {
            chunks: live.chunks.clone(),
            len: live.len,
            cost: Cost::default(),
        }
    }

    /// The chunk at `c`, made this edit's own: copied if an older version still shares it.
    fn own(&mut self, c: usize) -> Result<&mut Chunk, ArrangeError> {
        let chunk = self.chunks.get_mut(c).ok_or(ArrangeError::TooMany)?;
        if Arc::get_mut(chunk).is_none() {
            self.cost.chunks_copied = self.cost.chunks_copied.saturating_add(1);
        }
        Ok(Arc::make_mut(chunk))
    }

    /// Takes out the piece whose key is `lo`..`hi`, if there is one.
    fn remove(&mut self, lo: &Rational2, hi: &Rational2) -> Result<(), ArrangeError> {
        let (c, i, found) = find(&self.chunks, lo, hi)?;
        if !found {
            return Ok(());
        }
        let chunk = self.own(c)?;
        // `find` found the key at slot i, so the slot is there.
        chunk.pieces.remove(i);
        *chunk = Chunk::of(core::mem::take(&mut chunk.pieces));
        if chunk.pieces.is_empty() {
            self.chunks.remove(c);
        }
        self.len = self.len.saturating_sub(1);
        Ok(())
    }

    /// Puts `piece` in at its key; if a piece is there already, it takes `piece`'s sources too.
    fn put(&mut self, piece: Piece) -> Result<(), ArrangeError> {
        let (c, i, found) = find(&self.chunks, &piece.lo, &piece.hi)?;
        if self.chunks.is_empty() {
            self.chunks.push(Arc::new(Chunk::of(alloc::vec![piece])));
            self.len = 1;
            return Ok(());
        }
        let half = {
            let chunk = self.own(c)?;
            if let (true, Some(there)) = (found, chunk.pieces.get_mut(i)) {
                there.sources = merge(&there.sources, &piece.sources);
                return Ok(());
            }
            // The chunk is not empty, so its box is a box of pieces, and joins the new one's.
            chunk.bounds = join(&chunk.bounds, &piece.bounds());
            chunk.pieces.insert(i.min(chunk.pieces.len()), piece);
            if chunk.pieces.len() > CHUNK.saturating_mul(2) {
                let tail = chunk.pieces.split_off(CHUNK);
                *chunk = Chunk::of(core::mem::take(&mut chunk.pieces));
                Some(Chunk::of(tail))
            } else {
                None
            }
        };
        self.len = self.len.saturating_add(1);
        if let Some(half) = half {
            // A chunk past twice its size is halved; its upper half is a chunk this edit made.
            self.chunks.insert(c.saturating_add(1), Arc::new(half));
            self.cost.chunks_copied = self.cost.chunks_copied.saturating_add(1);
        }
        Ok(())
    }

    fn done(self) -> (Live, Cost) {
        (
            Live {
                chunks: self.chunks,
                len: self.len,
            },
            self.cost,
        )
    }
}

/// The points where the segment `s` and the piece `e` touch: one crossing, or for two on one line
/// the ends of each that lie on both. None when they miss.
fn contacts(s: &Piece, e: &Piece) -> Result<Vec<Rational2>, ArrangeError> {
    let (ds, de) = (s.line.direction(), e.line.direction());
    let mut out = Vec::new();
    if cross_sign(ds, de)? != Ordering::Equal {
        let m = s.line.meet(&e.line)?;
        if between(&s.lo, &m, &s.hi)? && between(&e.lo, &m, &e.hi)? {
            out.push(m);
        }
    } else if s.line.same(&e.line)? {
        for t in [&e.lo, &e.hi, &s.lo, &s.hi] {
            if between(&s.lo, t, &s.hi)? && between(&e.lo, t, &e.hi)? {
                out.push(*t);
            }
        }
    }
    Ok(out)
}

/// Sorts `items` by `order`, an order that can fail: the first failure is then the answer, and
/// the items are left in no promised order.
fn try_sort<T>(
    items: &mut [T],
    mut order: impl FnMut(&T, &T) -> Result<Ordering, ArrangeError>,
) -> Result<(), ArrangeError> {
    let mut fail: Option<ArrangeError> = None;
    items.sort_by(|a, b| match order(a, b) {
        Ok(o) => o,
        Err(e) => {
            fail = Some(e);
            Ordering::Equal
        }
    });
    fail.map_or(Ok(()), Err)
}

/// `points`, ascending and without repeats.
fn sorted(mut points: Vec<Rational2>) -> Result<Vec<Rational2>, ArrangeError> {
    try_sort(&mut points, cmp_pt)?;
    let mut out: Vec<Rational2> = Vec::with_capacity(points.len());
    for p in points {
        match out.last() {
            Some(last) if cmp_pt(last, &p)? == Ordering::Equal => {}
            _ => out.push(p),
        }
    }
    Ok(out)
}

/// How the direction `d` turns clockwise from `r`, as a vector whose counter-clockwise angle from
/// the x axis is that turn: (d · r, d × r).
fn turn(d: [i128; 2], r: [i128; 2]) -> Result<[i128; 2], ArrangeError> {
    Ok([
        add(mul(d[0], r[0])?, mul(d[1], r[1])?)?,
        sub(mul(d[0], r[1])?, mul(d[1], r[0])?)?,
    ])
}

/// The order of two vectors by counter-clockwise angle from the positive x axis, in [0, 2π).
fn angle_cmp(a: [i128; 2], b: [i128; 2]) -> Result<Ordering, ArrangeError> {
    let half = |v: [i128; 2]| u8::from(!(v[1] > 0 || (v[1] == 0 && v[0] > 0)));
    match half(a).cmp(&half(b)) {
        // With a cross product above zero `a` turns less than `b`.
        Ordering::Equal => Ok(cross_sign(a, b)?.reverse()),
        other => Ok(other),
    }
}

impl Live {
    /// An empty arrangement.
    #[must_use]
    pub fn new() -> Live {
        Live::default()
    }

    /// The number of pieces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no pieces.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of chunks.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// The pieces, in key order: by lower end, then by higher end.
    pub fn pieces(&self) -> impl Iterator<Item = &Piece> {
        self.chunks.iter().flat_map(|c| c.pieces.iter())
    }

    /// How many of this version's chunks are the very chunks of `other`.
    #[must_use]
    pub fn shared_chunks(&self, other: &Live) -> usize {
        self.chunks
            .iter()
            .filter(|c| other.chunks.iter().any(|d| Arc::ptr_eq(c, d)))
            .count()
    }

    /// The arrangement of `segments` built at once by the sweep, each labelled by its place in the
    /// list; segments of no length are ignored.
    ///
    /// # Errors
    ///
    /// As [`arrange`].
    pub fn batch(segments: &[(P2, P2)], meter: &mut WorkMeter<'_>) -> Result<Live, ArrangeError> {
        let arrangement = arrange(segments, meter)?;
        let vertices = arrangement.vertices();
        let mut pieces: Vec<Piece> = Vec::with_capacity(arrangement.edges().len());
        for edge in arrangement.edges() {
            // Segments only meet at rational points, so every vertex is one.
            let end = |ix: u32| -> Result<Rational2, ArrangeError> {
                vertices
                    .get(usize::try_from(ix).map_err(|_| ArrangeError::TooMany)?)
                    .and_then(|v| v.rational())
                    .copied()
                    .ok_or(ArrangeError::Range)
            };
            let (lo, hi) = (end(edge.from)?, end(edge.to)?);
            let first = edge.sources.first().copied().ok_or(ArrangeError::TooMany)?;
            let (a, b) = segments
                .get(usize::try_from(first).map_err(|_| ArrangeError::TooMany)?)
                .copied()
                .ok_or(ArrangeError::TooMany)?;
            let (p, q) = if cmp_pt(&Rational2::from(a), &Rational2::from(b))? == Ordering::Less {
                (a, b)
            } else {
                (b, a)
            };
            pieces.push(Piece {
                lo,
                hi,
                line: Carrier::through(p, q)?,
                sources: Arc::from(edge.sources.clone()),
            });
        }
        Live::from_pieces(pieces)
    }

    /// The pieces `pieces` as a `Live`; they are sorted here.
    fn from_pieces(mut pieces: Vec<Piece>) -> Result<Live, ArrangeError> {
        try_sort(&mut pieces, |a, b| cmp_key(a, &b.lo, &b.hi))?;
        let len = pieces.len();
        // Each chunk is collected into a vector of its own size. (Splitting the front off a long
        // vector keeps the long vector's buffer in the chunk, and copies the rest each time: a
        // million pieces was tens of gigabytes.)
        let mut chunks = Vec::new();
        let mut rest = pieces.into_iter();
        loop {
            let run: Vec<Piece> = rest.by_ref().take(CHUNK).collect();
            if run.is_empty() {
                break;
            }
            chunks.push(Arc::new(Chunk::of(run)));
        }
        Ok(Live { chunks, len })
    }

    /// A drawing from segments the caller says are already noded: no two meet except at an end
    /// point of both, and none repeats. It is the way to a large drawing without the sweep; with
    /// segments that cross it is not an arrangement, and nothing here checks that.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget.
    pub fn from_noded(segments: &[(P2, P2, u32)]) -> Result<Live, ArrangeError> {
        let mut pieces = Vec::with_capacity(segments.len());
        for &(a, b, label) in segments {
            let (ra, rb) = (Rational2::from(a), Rational2::from(b));
            let (lo, hi, p, q) = match cmp_pt(&ra, &rb)? {
                Ordering::Equal => continue,
                Ordering::Less => (ra, rb, a, b),
                Ordering::Greater => (rb, ra, b, a),
            };
            pieces.push(Piece {
                lo,
                hi,
                line: Carrier::through(p, q)?,
                sources: Arc::from([label]),
            });
        }
        Live::from_pieces(pieces)
    }

    /// Whether `other` holds the same pieces with the same sources, wherever the chunks fall.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget.
    pub fn same_as(&self, other: &Live) -> Result<bool, ArrangeError> {
        if self.len != other.len {
            return Ok(false);
        }
        for (a, b) in self.pieces().zip(other.pieces()) {
            if !a.lo.same(&b.lo)? || !a.hi.same(&b.hi)? || a.sources != b.sources {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// This drawing with the segment `a`..`b` added, labelled `label`; a segment of no length adds
    /// nothing, and a label already along the stretch stays once.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget, [`ArrangeError::Stopped`] if the meter stops.
    pub fn insert(
        &self,
        a: P2,
        b: P2,
        label: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Live, ArrangeError> {
        self.insert_costed(a, b, label, meter).map(|(live, _)| live)
    }

    /// As [`Live::insert`], with what the insert cost.
    ///
    /// # Errors
    ///
    /// As [`Live::insert`].
    pub fn insert_costed(
        &self,
        a: P2,
        b: P2,
        label: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Live, Cost), ArrangeError> {
        let (ra, rb) = (Rational2::from(a), Rational2::from(b));
        let (lo, hi, p, q) = match cmp_pt(&ra, &rb)? {
            Ordering::Equal => return Ok((self.clone(), Cost::default())),
            Ordering::Less => (ra, rb, a, b),
            Ordering::Greater => (rb, ra, b, a),
        };
        let s = Piece {
            lo,
            hi,
            line: Carrier::through(p, q)?,
            sources: Arc::from([label]),
        };
        self.insert_piece(&s, meter)
    }

    /// Puts `s`, a piece with its sources, into this drawing: the zone insertion.
    fn insert_piece(
        &self,
        s: &Piece,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Live, Cost), ArrangeError> {
        let mut edit = Edit::of(self);
        let sbox = s.bounds();
        // The zone: every piece whose box meets the segment's.
        let mut near: Vec<Piece> = Vec::new();
        for chunk in &self.chunks {
            if !meets(&chunk.bounds, &sbox) {
                continue;
            }
            edit.cost.chunks_opened = edit.cost.chunks_opened.saturating_add(1);
            for piece in &chunk.pieces {
                if meets(&piece.bounds(), &sbox) {
                    near.push(piece.clone());
                }
            }
        }
        let looked = u64::try_from(near.len()).unwrap_or(u64::MAX);
        edit.cost.examined = looked;
        meter.charge(looked.saturating_add(1))?;
        // Where the segment is cut, and where each piece it meets is.
        let mut cuts: Vec<Rational2> = alloc::vec![s.lo, s.hi];
        let mut splits: Vec<(Piece, Vec<Rational2>)> = Vec::new();
        for e in &near {
            let touch = contacts(s, e)?;
            let mut inside: Vec<Rational2> = Vec::new();
            for t in &touch {
                cuts.push(*t);
                if cmp_pt(&e.lo, t)? == Ordering::Less && cmp_pt(t, &e.hi)? == Ordering::Less {
                    inside.push(*t);
                }
            }
            if !inside.is_empty() {
                splits.push((e.clone(), inside));
            }
        }
        for (e, inside) in splits {
            edit.cost.split = edit.cost.split.saturating_add(1);
            edit.remove(&e.lo, &e.hi)?;
            let mut chain: Vec<Rational2> = alloc::vec![e.lo, e.hi];
            chain.extend(inside);
            let chain = sorted(chain)?;
            for (u, v) in chain.iter().zip(chain.iter().skip(1)) {
                edit.put(Piece {
                    lo: *u,
                    hi: *v,
                    line: e.line,
                    sources: e.sources.clone(),
                })?;
            }
        }
        // The segment itself, piece by piece between its cuts; a piece already along a stretch
        // takes the segment's sources.
        let cuts = sorted(cuts)?;
        for (u, v) in cuts.iter().zip(cuts.iter().skip(1)) {
            edit.cost.written = edit.cost.written.saturating_add(1);
            edit.put(Piece {
                lo: *u,
                hi: *v,
                line: s.line,
                sources: s.sources.clone(),
            })?;
        }
        Ok(edit.done())
    }

    /// This drawing with everything of `other` laid over it: the arrangement of the union of the
    /// segments of both, with the labels of both along each piece.
    ///
    /// # Errors
    ///
    /// As [`Live::insert`].
    pub fn overlay(&self, other: &Live, meter: &mut WorkMeter<'_>) -> Result<Live, ArrangeError> {
        let mut out = self.clone();
        for piece in other.pieces() {
            out = out.insert_piece(piece, meter)?.0;
        }
        Ok(out)
    }

    /// The pieces that have `v` at an end.
    fn at(&self, v: &Rational2) -> Result<Vec<Piece>, ArrangeError> {
        let vbox = box_of(v, v);
        let mut out = Vec::new();
        for chunk in &self.chunks {
            if !meets(&chunk.bounds, &vbox) {
                continue;
            }
            for piece in &chunk.pieces {
                if meets(&piece.bounds(), &vbox) && (piece.lo.same(v)? || piece.hi.same(v)?) {
                    out.push(piece.clone());
                }
            }
        }
        Ok(out)
    }

    /// The direction a piece leaves `v` by, and the other end.
    fn leaving(piece: &Piece, v: &Rational2) -> Result<([i128; 2], Rational2), ArrangeError> {
        let d = piece.line.direction();
        if piece.lo.same(v)? {
            Ok((d, piece.hi))
        } else {
            Ok((
                [0_i128.saturating_sub(d[0]), 0_i128.saturating_sub(d[1])],
                piece.lo,
            ))
        }
    }

    /// What lies at `p`.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget, [`ArrangeError::Stopped`] if the meter stops.
    pub fn locate(
        &self,
        p: &Rational2,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Location, ArrangeError> {
        let ([px, py], pw) = p.coordinates();
        let x = floor(px, pw);
        let column: Box4 = [x, i128::MIN, x, floor(py, pw)];
        let vertical = Carrier::new(pw, 0, px)?;
        // The closest hit below p: its point, the first piece found that reaches it, and whether
        // that piece ends there. Pieces meet only at their ends, so a point that is the end of one
        // piece reaching it is the end of every piece reaching it, and any one piece decides.
        let mut best: Option<(Rational2, Half, bool)> = None;
        let mut looked = 0_u64;
        for chunk in &self.chunks {
            if !meets(&chunk.bounds, &column) {
                continue;
            }
            for e in &chunk.pieces {
                if !meets(&e.bounds(), &column) {
                    continue;
                }
                looked = looked.saturating_add(1);
                if e.lo.same(p)? || e.hi.same(p)? {
                    return Ok(Location::Vertex(*p));
                }
                if e.line.side(p)? == Sign::Zero && between(&e.lo, p, &e.hi)? {
                    return Ok(Location::Edge(e.lo, e.hi));
                }
                if sign_order(e.lo.cmp_x(p)?) == Ordering::Greater
                    || sign_order(e.hi.cmp_x(p)?) == Ordering::Less
                {
                    continue;
                }
                let h = if e.lo.cmp_x(&e.hi)? == Sign::Zero {
                    // A vertical edge in the column: the ray meets its top end.
                    e.hi
                } else {
                    vertical.meet(&e.line)?
                };
                if sign_order(h.cmp_y(p)?) != Ordering::Less {
                    continue;
                }
                let higher = match &best {
                    Some((at, _, _)) => sign_order(h.cmp_y(at)?) == Ordering::Greater,
                    None => true,
                };
                if higher {
                    let ends = e.lo.same(&h)? || e.hi.same(&h)?;
                    let under = Half {
                        from: e.lo,
                        to: e.hi,
                    };
                    best = Some((h, under, ends));
                }
            }
        }
        meter.charge(looked.saturating_add(1))?;
        let Some((h, under, at_end)) = best else {
            return Ok(Location::Face(None));
        };
        if !at_end {
            // Under an edge's inside: the face above it is on the left of the edge as it runs.
            return Ok(Location::Face(Some(under)));
        }
        // At a vertex: the face is the one the first edge clockwise from straight up bounds.
        let ring = self.at(&h)?;
        let up = [0_i128, 1];
        let mut first: Option<([i128; 2], Rational2)> = None;
        for e in &ring {
            let (d, other) = Live::leaving(e, &h)?;
            let key = turn(d, up)?;
            let better = match &first {
                None => true,
                Some((fd, _)) => angle_cmp(key, turn(*fd, up)?)? == Ordering::Less,
            };
            if better {
                first = Some((d, other));
            }
        }
        Ok(Location::Face(first.map(|(_, to)| Half { from: h, to })))
    }

    /// The vertices round the boundary the half-edge `half` starts, with the face on its left, in
    /// the order the boundary runs, starting at `half.from`.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget, [`ArrangeError::Stopped`] if the meter stops,
    /// and [`ArrangeError::TooMany`] if `half` is not an edge of the drawing, or if the walk does
    /// not come back to `half` within twice as many steps as there are pieces (which only a drawing
    /// from [`Live::from_noded`] whose segments were not noded can do).
    pub fn face_cycle(
        &self,
        half: &Half,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Rational2>, ArrangeError> {
        let mut out = alloc::vec![half.from];
        let (mut u, mut v) = (half.from, half.to);
        let limit = self.len.saturating_mul(2).saturating_add(2);
        for _ in 0..limit {
            meter.charge(1)?;
            let ring = self.at(&v)?;
            let mut back: Option<[i128; 2]> = None;
            let mut dirs: Vec<([i128; 2], Rational2)> = Vec::new();
            for e in &ring {
                let (d, other) = Live::leaving(e, &v)?;
                if other.same(&u)? {
                    back = Some(d);
                }
                dirs.push((d, other));
            }
            let back = back.ok_or(ArrangeError::TooMany)?;
            // The next edge is the first clockwise from the way back; with only the way back, that.
            let mut next: Option<([i128; 2], Rational2)> = None;
            for (d, other) in &dirs {
                if other.same(&u)? && dirs.len() > 1 {
                    continue;
                }
                let key = turn(*d, back)?;
                let better = match &next {
                    None => true,
                    Some((nd, _)) => angle_cmp(key, turn(*nd, back)?)? == Ordering::Less,
                };
                if better {
                    next = Some((*d, *other));
                }
            }
            let (_, w) = next.ok_or(ArrangeError::TooMany)?;
            if v.same(&half.from)? && w.same(&half.to)? {
                return Ok(out);
            }
            out.push(v);
            u = v;
            v = w;
        }
        Err(ArrangeError::TooMany)
    }
}

/// The sources `a` and `b` together, ascending and without repeats.
fn merge(a: &[u32], b: &[u32]) -> Arc<[u32]> {
    let mut all: Vec<u32> = a.to_vec();
    for &l in b {
        if let Err(at) = all.binary_search(&l) {
            all.insert(at, l);
        }
    }
    Arc::from(all)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    reason = "a test generator; the rules are for kernel code"
)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> P2 {
        P2::new(x, y).unwrap()
    }

    /// A lattice of unit segments, noded by construction.
    fn lattice(side: i64) -> Vec<(P2, P2, u32)> {
        let mut out = Vec::new();
        for y in 0..=side {
            for x in 0..side {
                out.push((p(x, y), p(x + 1, y), u32::try_from(out.len()).unwrap()));
                out.push((p(y, x), p(y, x + 1), u32::try_from(out.len()).unwrap()));
            }
        }
        out
    }

    /// A chunk holds a buffer about its own size, never the buffer of the list it was cut from.
    #[test]
    fn from_noded_chunks_hold_buffers_of_their_own_size() {
        let live = Live::from_noded(&lattice(60)).unwrap();
        assert_eq!(live.chunk_count(), live.len().div_ceil(CHUNK));
        for chunk in &live.chunks {
            assert!(chunk.pieces.len() <= CHUNK);
            assert!(chunk.pieces.capacity() <= CHUNK.saturating_mul(2));
        }
    }

    /// The angle of `v` from the positive x axis, as a "diamond angle": a quarter turn number and a
    /// fraction (numerator, denominator) along that quarter. It grows with the true angle, and is
    /// worked out here without the half planes and cross products `angle_cmp` uses.
    fn diamond(v: [i128; 2]) -> (i128, i128, i128) {
        let [x, y] = v;
        if x > 0 && y >= 0 {
            (0, y, x + y)
        } else if x <= 0 && y > 0 {
            (1, -x, -x + y)
        } else if x < 0 && y <= 0 {
            (2, -y, -x - y)
        } else {
            (3, x, x - y)
        }
    }

    fn diamond_cmp(a: [i128; 2], b: [i128; 2]) -> Ordering {
        let ((qa, na, da), (qb, nb, db)) = (diamond(a), diamond(b));
        qa.cmp(&qb).then((na * db).cmp(&(nb * da)))
    }

    /// Every pair of directions with small coordinates is ordered by angle as the diamond angle
    /// orders them, including the axes and directions that are multiples of each other.
    #[test]
    fn angles_are_ordered_from_the_positive_x_axis_round() {
        let mut all = Vec::new();
        for x in -3..=3_i128 {
            for y in -3..=3_i128 {
                if (x, y) != (0, 0) {
                    all.push([x, y]);
                }
            }
        }
        for &a in &all {
            for &b in &all {
                assert_eq!(angle_cmp(a, b).unwrap(), diamond_cmp(a, b), "{a:?} {b:?}");
            }
        }
        // And a turn is measured clockwise from the reference: from straight up, the positive x axis
        // is a quarter turn, straight down a half and the negative x axis three quarters.
        let quarter = |d: [i128; 2]| diamond(turn(d, [0, 1]).unwrap());
        assert_eq!(
            [
                quarter([0, 1]),
                quarter([1, 0]),
                quarter([0, -1]),
                quarter([-1, 0])
            ],
            [(0, 0, 1), (1, 0, 1), (2, 0, 1), (3, 0, 1)]
        );
    }

    /// A step past i128 is refused, never wrapped.
    #[test]
    fn a_turn_past_the_integers_is_refused() {
        let big = [i128::MAX, 1];
        assert!(matches!(turn(big, big), Err(ArrangeError::Range)));
        assert!(matches!(
            turn([1, i128::MAX], [1, i128::MAX]),
            Err(ArrangeError::Range)
        ));
        assert!(matches!(
            turn([1, 2], [i128::MAX, 1]),
            Err(ArrangeError::Range)
        ));
        assert!(matches!(
            turn([1, -1], [1, i128::MAX]),
            Err(ArrangeError::Range)
        ));
        assert!(matches!(
            angle_cmp(big, [1, i128::MAX]),
            Err(ArrangeError::Range)
        ));
    }

    /// An order that fails makes the sort fail, with the order's own error.
    #[test]
    fn a_sort_whose_order_fails_fails() {
        let mut items = [3, 1, 2];
        assert!(matches!(
            try_sort(&mut items, |_, _| Err(ArrangeError::TooMany)),
            Err(ArrangeError::TooMany)
        ));
        try_sort(&mut items, |a, b| Ok(a.cmp(b))).unwrap();
        assert_eq!(items, [1, 2, 3]);
    }

    /// Taking out a key that is not there changes nothing, in an empty drawing or not.
    #[test]
    fn removing_a_key_that_is_not_there_changes_nothing() {
        let (a, b) = (Rational2::from(p(0, 0)), Rational2::from(p(1, 0)));
        let mut empty = Edit::of(&Live::new());
        empty.remove(&a, &b).unwrap();
        let (live, cost) = empty.done();
        assert!(live.is_empty());
        assert_eq!(cost, Cost::default());
        let drawing = Live::from_noded(&lattice(3)).unwrap();
        let mut edit = Edit::of(&drawing);
        edit.remove(&a, &Rational2::from(p(2, 0))).unwrap();
        let (after, cost) = edit.done();
        assert!(after.same_as(&drawing).unwrap());
        assert_eq!(cost, Cost::default());
        assert_eq!(after.shared_chunks(&drawing), drawing.chunk_count());
    }

    /// A chunk index that is not there is refused.
    #[test]
    fn owning_a_chunk_that_is_not_there_is_refused() {
        let mut edit = Edit::of(&Live::new());
        assert!(matches!(edit.own(0), Err(ArrangeError::TooMany)));
    }
}
