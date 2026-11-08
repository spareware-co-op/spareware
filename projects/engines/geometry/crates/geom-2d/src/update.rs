//! Arrangement updates and point location (chapter 14.4): zone insertion into a drawing of segments
//! and circular arcs kept in shared chunks, overlay, and the exact question "where is this point".
//!
//! What it does: a [`Live`] is an arrangement of segments and arcs that can be changed.
//! [`Live::insert_curve`] adds one curve and returns a new version, touching only the pieces the
//! curve meets (its zone); every chunk of pieces the curve does not touch is shared, by reference,
//! with the version before, which stays valid. An arc goes in as the sweep takes it: cut where its
//! circle is leftmost or rightmost, so that each piece runs one way in x on one half of its circle,
//! and its vertices where it meets a line or another circle are one-root points ([`At`]).
//! [`Live::insert`] is the same for a segment given by its ends. [`Live::overlay`] puts one `Live`
//! over another. [`Live::locate`] says whether a point is a vertex, on an edge, or in a face, and
//! names the face by a half-edge of its boundary; [`Live::face_cycle`] walks that boundary.
//! [`Live::batch_curves`] builds the same thing from a list at once, by
//! [`crate::arrange::arrange_curves`], and is the reference every update is held to.
//!
//! Guarantees:
//!
//! - **Incremental equals batch.** Inserting curves one by one, in any order, gives the pieces,
//!   on the same lines and circles and with the same sources, that
//!   [`crate::arrange::arrange_curves`] gives for the same curves; an overlay gives those of the
//!   union. Vertices are the end points, the exact crossings, the points where an arc's circle is
//!   leftmost or rightmost, and touching splits; pieces on one line, or on one half of one circle,
//!   share the edges of their common stretch. [`Live::same_as`] is the comparison.
//! - **Exact.** Every decision is a sign of one of `geom-num`'s exact predicates on carriers,
//!   circles, rational points and one-root points, asked through the sweep's own questions (the
//!   arrangement's piece module): which side of a piece a point lies on, where two pieces meet, and
//!   the order of curves leaving a point by tangent, then turn, then curvature. A piece keeps the
//!   line or circle of the curves along it, so a split never moves it, and a crossing is the point
//!   where two original curves meet. A step past the exact budget is [`ArrangeError::Range`], never
//!   a wrong answer.
//! - **Old versions stay.** An insert does not change `self`; the chunks it does not touch are the
//!   same chunks, which [`Live::shared_chunks`] counts.
//! - **Located exactly.** A point on a vertex or an edge is reported as that vertex or edge; a
//!   point in a face is reported by the half-edge of the boundary under it, with the point on its
//!   left, found by shooting a ray straight down and, at a vertex, by the exact order of the curves
//!   leaving it. A point with nothing below it is in the unbounded face.
//! - **Cancellable.** Every insert and every location charges its [`WorkMeter`] for the pieces it
//!   examines.
//!
//! Method: the zone of a curve in an arrangement (Edelsbrunner, Guibas and Sharir 1990,
//! doi:10.1007/BF02187681) is the set of faces it meets; inserting walks it, splits what it crosses
//! and merges what it overlaps, as Fogel, Halperin and Wein, *CGAL Arrangements and Their
//! Applications*, Springer 2012, chapters 1 and 5 describe the operation for x-monotone curves, with
//! the chunked version-sharing of a persistent sorted sequence. Here the walk finds the zone by
//! chunk boxes and not by walking face to face: a chunk whose box misses the curve's box is not
//! opened. The ray shot down is the vertical ray shooting of de Berg, Cheong, van Kreveld and
//! Overmars, *Computational Geometry*, 3rd ed., chapter 6, done by the chunk boxes rather than a
//! slab search tree.
//!
//! Cost: an insert opens the chunks whose boxes meet the curve's box (all chunk boxes are compared,
//! one comparison per chunk, c pieces per chunk) and splits the pieces the curve meets; the new
//! version copies only the chunks it changes and the list of chunk pointers. An arc is at most three
//! pieces, each inserted so. A location opens the chunks over the point's column. The face cycle is
//! linear in the face's boundary. Every question on a one-root point is a filter of a few interval
//! operations, then at worst a few dozen exact products. Batch building is the sweep's cost,
//! O((n + k) log n).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "s, e, p, q, h, u, v, w are the geometry's own letters, and the insert and the location are each one procedure that reads best whole"
)]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::slice;

use geom_num::Sign;
use geom_num::arc::Round;
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root, enclose};
use geom_num::wide::I256;
use geom_run::WorkMeter;

use crate::arrange::piece::{
    self, Circle, On, Way, cmp_pt, cut, meetings, order_at, same_circle, side, way,
};
use crate::arrange::{Along, ArrangeError, At, Curve, arrange_curves, segment};

/// Pieces a chunk is split into halves at twice this many.
const CHUNK: usize = 256;

/// A box in whole steps, `[min x, min y, max x, max y]`, each the floor of an exact coordinate or of
/// a certified bound beyond it. The floor never decreases, so two boxes whose exact ranges overlap
/// still meet once floored: a box skips only what cannot meet.
type Box4 = [i128; 4];

/// The floor of `v / w`; `w` is a weight, so positive, and the division cannot fail.
fn floor(v: i128, w: i128) -> i128 {
    v.checked_div_euclid(w).unwrap_or(0)
}

/// The floor of a finite double. A double of magnitude 2^53 or more is whole, and every bound here
/// is below 2^126, so the conversion is exact either way.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "a whole double below 2^126 converts exactly, and below 2^53 the comparison is exact"
)]
fn floor_of(v: f64) -> i128 {
    let t = v as i128;
    if (t as f64) > v {
        t.saturating_sub(1)
    } else {
        t
    }
}

/// The box of a point: its floors, or the floors of a one-root point's certified enclosure.
fn point_box(p: &At) -> Box4 {
    match p {
        At::Rational(r) => {
            let ([x, y], w) = r.coordinates();
            let (x, y) = (floor(x, w), floor(y, w));
            [x, y, x, y]
        }
        At::Root(z) => {
            let [ix, iy] = z.enclosure();
            [
                floor_of(ix.lo()),
                floor_of(iy.lo()),
                floor_of(ix.hi()),
                floor_of(iy.hi()),
            ]
        }
    }
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

/// A circle the drawing's arcs lie on, with what an arc piece's box needs: the floor of its
/// centre's x, and floors at or below its lowest y and at or above its highest.
#[derive(Debug)]
struct Ring {
    circle: Circle,
    centre_x: i128,
    bottom: i128,
    top: i128,
}

impl Ring {
    /// The circle `round`, with its centre's column and its extent in y, certified.
    fn of(round: Round) -> Result<Ring, ArrangeError> {
        let circle = Circle::of(round)?;
        let ([x, y], w) = circle.centre.coordinates();
        let whole = |v: i128| enclose(&I256::from_i128(v));
        // The radius is √rr / W, rr / W² the squared radius.
        let cy = whole(y)?.div(whole(w)?)?;
        let r = enclose(&circle.disc.squared_radius())?
            .sqrt()?
            .div(whole(w)?)?;
        Ok(Ring {
            centre_x: floor(x, w),
            bottom: floor_of(cy.sub(r)?.lo()),
            top: floor_of(cy.add(r)?.hi()),
            circle,
        })
    }
}

/// What a piece lies on.
#[derive(Clone, Copy, Debug)]
pub enum Track {
    /// A line: its carrier, running from the piece's lower end to its higher.
    Line(Carrier),
    /// One half of a circle: the upper runs from the circle's leftmost point over the top to its
    /// rightmost, the lower from the leftmost under the bottom.
    Arc {
        /// The circle.
        circle: Round,
        /// Whether the piece is on the upper half.
        upper: bool,
    },
}

/// A piece of the drawing: an edge between two vertices, lowest first, on a line or on one half of
/// a circle, with the curves along it.
#[derive(Clone, Debug)]
pub struct Piece {
    /// The ends and what the piece lies on, as the sweep holds its pieces; an arc's circle is the
    /// one of `ring`, at place 0 of [`Piece::circles`].
    run: piece::Piece,
    /// The circle of an arc piece; none for a line.
    ring: Option<Arc<Ring>>,
    sources: Arc<[u32]>,
}

impl Piece {
    /// The lexicographically lower end.
    #[must_use]
    pub fn lo(&self) -> &At {
        &self.run.lo
    }

    /// The higher end.
    #[must_use]
    pub fn hi(&self) -> &At {
        &self.run.hi
    }

    /// The labels of the curves that cover the piece, ascending.
    #[must_use]
    pub fn sources(&self) -> &[u32] {
        &self.sources
    }

    /// What the piece lies on.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::TooMany`] for an arc piece without its circle, which no `Live` holds.
    pub fn track(&self) -> Result<Track, ArrangeError> {
        match (self.run.on, &self.ring) {
            (On::Line { line, .. }, _) => Ok(Track::Line(line)),
            (On::Arc { upper, .. }, Some(ring)) => Ok(Track::Arc {
                circle: ring.circle.round,
                upper,
            }),
            (On::Arc { .. }, None) => Err(ArrangeError::TooMany),
        }
    }

    /// The circles the piece's questions read: its own circle at place 0, or none for a line.
    fn circles(&self) -> &[Circle] {
        self.ring
            .as_deref()
            .map_or(&[], |ring| slice::from_ref(&ring.circle))
    }

    /// The piece's box: its ends', and for an arc that may pass the top or the bottom of its circle,
    /// that extreme too.
    fn bounds(&self) -> Box4 {
        let b = join(&point_box(&self.run.lo), &point_box(&self.run.hi));
        match (&self.ring, self.run.on) {
            // The piece spans the centre's x exactly when its ends' x lie either side of it; the
            // floors keep that order, so this test may take a piece that does not span it, never
            // miss one that does.
            (Some(ring), On::Arc { upper, .. })
                if b[0] <= ring.centre_x && ring.centre_x <= b[2] =>
            {
                if upper {
                    [b[0], b[1], b[2], b[3].max(ring.top)]
                } else {
                    [b[0], b[1].min(ring.bottom), b[2], b[3]]
                }
            }
            _ => b,
        }
    }

    /// The way the piece leaves `v`, one of its ends, and its other end.
    fn leaving(&self, v: &At) -> Result<(Way<'_>, At), ArrangeError> {
        if self.run.lo.same(v)? {
            Ok((way(&self.run, self.circles(), false)?, self.run.hi.clone()))
        } else {
            Ok((way(&self.run, self.circles(), true)?, self.run.lo.clone()))
        }
    }

    /// The piece as a half-edge from `from`, one of its ends, to the other.
    fn half_from(&self, from: &At) -> Result<Half, ArrangeError> {
        let to = if self.run.lo.same(from)? {
            self.run.hi.clone()
        } else {
            self.run.lo.clone()
        };
        Ok(Half {
            from: from.clone(),
            to,
            on: self.track()?,
        })
    }

    /// The same stretch of the same curve, from `lo` to `hi`.
    fn between(&self, lo: &At, hi: &At) -> Piece {
        Piece {
            run: piece::Piece {
                lo: lo.clone(),
                hi: hi.clone(),
                on: self.run.on,
            },
            ring: self.ring.clone(),
            sources: self.sources.clone(),
        }
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
    /// The chunks whose boxes met a piece of the curve, so that their pieces were looked at.
    pub chunks_opened: u64,
    /// The pieces looked at.
    pub examined: u64,
    /// The pieces that were split.
    pub split: u64,
    /// The pieces the curve added or whose sources it extended.
    pub written: u64,
    /// The chunks copied because the new version changed them, and the halves it split off chunks
    /// that grew past twice their size.
    pub chunks_copied: u64,
}

/// What lies at a point.
#[derive(Clone, Debug)]
pub enum Location {
    /// The point is this vertex.
    Vertex(At),
    /// The point is inside this edge, given as a half-edge from its lower end to its higher.
    Edge(Half),
    /// The point is inside a face, which has this half-edge on its boundary, or is in the unbounded
    /// face when there is none.
    Face(Option<Half>),
}

/// A directed edge: the face on its left is the face meant.
#[derive(Clone, Debug)]
pub struct Half {
    /// Where it starts.
    pub from: At,
    /// Where it ends.
    pub to: At,
    /// What it runs along: two edges between the same two vertices (a chord and its arc, or two
    /// arcs) differ here.
    pub on: Track,
}

/// An arrangement of segments and arcs that can be changed, in chunks shared between versions.
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

/// The order of pieces: by lower end, then higher end, then the order they leave the lower end in
/// (a chord before or after its arc, counter-clockwise). Equal only for one stretch of one curve.
fn cmp_key(p: &Piece, q: &Piece) -> Result<Ordering, ArrangeError> {
    match cmp_pt(&p.run.lo, &q.run.lo)? {
        Ordering::Equal => {}
        other => return Ok(other),
    }
    match cmp_pt(&p.run.hi, &q.run.hi)? {
        Ordering::Equal => {}
        other => return Ok(other),
    }
    order_at(
        &p.run.lo,
        &way(&p.run, p.circles(), false)?,
        &way(&q.run, q.circles(), false)?,
    )
}

/// Where the key of `key` is among `chunks`: the chunk, the slot in it, and whether it is there.
fn find(chunks: &[Arc<Chunk>], key: &Piece) -> Result<(usize, usize, bool), ArrangeError> {
    // The last chunk whose first piece is not above the key.
    let (mut a, mut b) = (0_usize, chunks.len());
    while a < b {
        let mid = a.saturating_add(b.saturating_sub(a) / 2);
        let first = chunks
            .get(mid)
            .and_then(|c| c.pieces.first())
            .ok_or(ArrangeError::TooMany)?;
        if cmp_key(first, key)? == Ordering::Greater {
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
        match cmp_key(p, key)? {
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

    /// Takes out the piece with the key of `key`, if there is one.
    fn remove(&mut self, key: &Piece) -> Result<(), ArrangeError> {
        let (c, i, found) = find(&self.chunks, key)?;
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
        let (c, i, found) = find(&self.chunks, &piece)?;
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

    /// Puts `s`, a piece with its sources, into the drawing being edited: the zone insertion.
    fn insert(&mut self, s: &Piece, meter: &mut WorkMeter<'_>) -> Result<(), ArrangeError> {
        let sbox = s.bounds();
        // The zone: every piece whose box meets the piece's.
        let mut near: Vec<Piece> = Vec::new();
        for chunk in &self.chunks {
            if !meets(&chunk.bounds, &sbox) {
                continue;
            }
            self.cost.chunks_opened = self.cost.chunks_opened.saturating_add(1);
            for piece in &chunk.pieces {
                if meets(&piece.bounds(), &sbox) {
                    near.push(piece.clone());
                }
            }
        }
        let looked = u64::try_from(near.len()).unwrap_or(u64::MAX);
        self.cost.examined = self.cost.examined.saturating_add(looked);
        meter.charge(looked.saturating_add(1))?;
        // Where the piece is cut, and where each piece it meets is.
        let mut cuts: Vec<At> = alloc::vec![s.run.lo.clone(), s.run.hi.clone()];
        let mut splits: Vec<(Piece, Vec<At>)> = Vec::new();
        for e in &near {
            let touch = contacts(s, e)?;
            let mut inside: Vec<At> = Vec::new();
            for t in touch {
                if cmp_pt(&e.run.lo, &t)? == Ordering::Less
                    && cmp_pt(&t, &e.run.hi)? == Ordering::Less
                {
                    inside.push(t.clone());
                }
                cuts.push(t);
            }
            if !inside.is_empty() {
                splits.push((e.clone(), inside));
            }
        }
        for (e, inside) in splits {
            self.cost.split = self.cost.split.saturating_add(1);
            self.remove(&e)?;
            let mut chain: Vec<At> = alloc::vec![e.run.lo.clone(), e.run.hi.clone()];
            chain.extend(inside);
            let chain = sorted(chain)?;
            for (u, v) in chain.iter().zip(chain.iter().skip(1)) {
                self.put(e.between(u, v))?;
            }
        }
        // The piece itself, stretch by stretch between its cuts; a piece already along a stretch
        // takes the new piece's sources.
        let cuts = sorted(cuts)?;
        for (u, v) in cuts.iter().zip(cuts.iter().skip(1)) {
            self.cost.written = self.cost.written.saturating_add(1);
            self.put(s.between(u, v))?;
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

/// Whether `t` lies on `e`: not before its lower end, not past its higher, and on its line or its
/// half of its circle.
fn holds(e: &Piece, t: &At) -> Result<bool, ArrangeError> {
    Ok(cmp_pt(&e.run.lo, t)? != Ordering::Greater
        && cmp_pt(t, &e.run.hi)? != Ordering::Greater
        && side(&e.run, e.circles(), t)? == Sign::Zero)
}

/// Whether two pieces lie on one line, or on one circle (either half).
fn one_carrier(s: &Piece, e: &Piece) -> Result<bool, ArrangeError> {
    match (s.run.on, e.run.on, &s.ring, &e.ring) {
        (On::Line { line: l, .. }, On::Line { line: m, .. }, _, _) => Ok(l.same(&m)?),
        (On::Arc { .. }, On::Arc { .. }, Some(a), Some(b)) => {
            Ok(same_circle(&a.circle, &b.circle)? == Ordering::Equal)
        }
        (On::Arc { .. }, _, None, _) | (_, On::Arc { .. }, _, None) => Err(ArrangeError::TooMany),
        _ => Ok(false),
    }
}

/// Where two pieces on different lines or circles cross after `after`, the later of their lower
/// ends, as the sweep finds it: each piece's circle placed in one list.
fn crossings(s: &Piece, e: &Piece, after: &At) -> Result<[Option<At>; 2], ArrangeError> {
    match (&s.ring, &e.ring) {
        (Some(a), Some(b)) => {
            let mut t = e.run.clone();
            if let On::Arc { upper, .. } = t.on {
                t.on = On::Arc { circle: 1, upper };
            }
            let circles = [a.circle.clone(), b.circle.clone()];
            meetings(&s.run, &t, &circles, after)
        }
        (Some(a), None) => meetings(&s.run, &e.run, slice::from_ref(&a.circle), after),
        (None, Some(b)) => meetings(&s.run, &e.run, slice::from_ref(&b.circle), after),
        (None, None) => meetings(&s.run, &e.run, &[], after),
    }
}

/// The points where the piece `s` and the piece `e` touch: the ends of each that lie on the other,
/// and, on different lines or circles, where they cross.
fn contacts(s: &Piece, e: &Piece) -> Result<Vec<At>, ArrangeError> {
    let mut out = Vec::new();
    for (a, b) in [(s, e), (e, s)] {
        for t in [&a.run.lo, &a.run.hi] {
            if holds(b, t)? {
                out.push(t.clone());
            }
        }
    }
    if !one_carrier(s, e)? {
        // A crossing at the later lower end is that end, on the other piece, and found above.
        let after = if cmp_pt(&s.run.lo, &e.run.lo)? == Ordering::Less {
            &e.run.lo
        } else {
            &s.run.lo
        };
        out.extend(crossings(s, e, after)?.into_iter().flatten());
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
fn sorted(mut points: Vec<At>) -> Result<Vec<At>, ArrangeError> {
    try_sort(&mut points, cmp_pt)?;
    let mut out: Vec<At> = Vec::with_capacity(points.len());
    for p in points {
        match out.last() {
            Some(last) if cmp_pt(last, &p)? == Ordering::Equal => {}
            _ => out.push(p),
        }
    }
    Ok(out)
}

/// Of the pieces of `ring`, all with `v` at an end, the first met turning clockwise from the way
/// `from` out of `v`, with its other end; `skip`, the piece `from` runs along, is passed over unless
/// it is the only one. In the counter-clockwise order of the ways out of `v` ([`order_at`]), that is
/// the last one before `from`, or else the last of all.
fn clockwise_from(
    v: &At,
    from: &Way<'_>,
    ring: &[Piece],
    skip: Option<&Piece>,
) -> Result<Option<Piece>, ArrangeError> {
    let mut before: Option<(Way<'_>, &Piece)> = None;
    let mut after: Option<(Way<'_>, &Piece)> = None;
    for e in ring {
        if let Some(s) = skip
            && ring.len() > 1
            && cmp_key(e, s)? == Ordering::Equal
        {
            continue;
        }
        let (w, _) = e.leaving(v)?;
        let group = if order_at(v, &w, from)? == Ordering::Greater {
            &mut after
        } else {
            &mut before
        };
        let better = match group {
            Some((g, _)) => order_at(v, &w, g)? == Ordering::Greater,
            None => true,
        };
        if better {
            *group = Some((w, e));
        }
    }
    Ok(before.or(after).map(|(_, e)| e.clone()))
}

/// Whether a piece lies on what `track` names.
fn on_track(p: &Piece, track: &Track) -> Result<bool, ArrangeError> {
    match (p.run.on, track, &p.ring) {
        (On::Line { line, .. }, Track::Line(other), _) => Ok(line.same(other)?),
        (On::Arc { upper, .. }, Track::Arc { circle, upper: u }, Some(ring)) => {
            Ok(upper == *u && same_circle(&ring.circle, &Circle::of(*circle)?)? == Ordering::Equal)
        }
        _ => Ok(false),
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

    /// The pieces, in key order: by lower end, then by higher end, then by the order they leave the
    /// lower end in.
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
    /// As [`arrange_curves`].
    pub fn batch(segments: &[(P2, P2)], meter: &mut WorkMeter<'_>) -> Result<Live, ArrangeError> {
        let curves: Vec<Curve> = segments
            .iter()
            .map(|&(a, b)| Curve::Segment(a, b))
            .collect();
        Live::batch_curves(&curves, meter)
    }

    /// The arrangement of `curves`, segments and arcs, built at once by the sweep, each labelled by
    /// its place in the list; segments of no length are ignored.
    ///
    /// # Errors
    ///
    /// As [`arrange_curves`].
    pub fn batch_curves(curves: &[Curve], meter: &mut WorkMeter<'_>) -> Result<Live, ArrangeError> {
        let arrangement = arrange_curves(curves, meter)?;
        let vertices = arrangement.vertices();
        let mut rings: Vec<Arc<Ring>> = Vec::with_capacity(arrangement.circles().len());
        for round in arrangement.circles() {
            rings.push(Arc::new(Ring::of(*round)?));
        }
        let mut pieces: Vec<Piece> = Vec::with_capacity(arrangement.edges().len());
        for (e, edge) in arrangement.edges().iter().enumerate() {
            let end = |ix: u32| -> Result<At, ArrangeError> {
                vertices
                    .get(usize::try_from(ix).map_err(|_| ArrangeError::TooMany)?)
                    .cloned()
                    .ok_or(ArrangeError::TooMany)
            };
            let along = arrangement
                .along(u32::try_from(e).map_err(|_| ArrangeError::TooMany)?)
                .ok_or(ArrangeError::TooMany)?;
            // The sweep's line is the smallest carrier, as a segment's piece holds it.
            let (on, ring) = match along {
                Along::Line(line) => (
                    On::Line {
                        line,
                        dir: line.direction(),
                    },
                    None,
                ),
                Along::Arc { circle, upper } => {
                    let ring = usize::try_from(circle)
                        .ok()
                        .and_then(|i| rings.get(i))
                        .cloned()
                        .ok_or(ArrangeError::TooMany)?;
                    (On::Arc { circle: 0, upper }, Some(ring))
                }
            };
            pieces.push(Piece {
                run: piece::Piece {
                    lo: end(edge.from)?,
                    hi: end(edge.to)?,
                    on,
                },
                ring,
                sources: Arc::from(edge.sources.clone()),
            });
        }
        Live::from_pieces(pieces)
    }

    /// The pieces `pieces` as a `Live`; they are sorted here.
    fn from_pieces(mut pieces: Vec<Piece>) -> Result<Live, ArrangeError> {
        try_sort(&mut pieces, cmp_key)?;
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
            if let Some(run) = segment(a, b)? {
                pieces.push(Piece {
                    run,
                    ring: None,
                    sources: Arc::from([label]),
                });
            }
        }
        Live::from_pieces(pieces)
    }

    /// Whether `other` holds the same pieces, on the same lines and circles, with the same sources,
    /// wherever the chunks fall.
    ///
    /// # Errors
    ///
    /// [`ArrangeError::Range`] past the exact budget.
    pub fn same_as(&self, other: &Live) -> Result<bool, ArrangeError> {
        if self.len != other.len {
            return Ok(false);
        }
        for (a, b) in self.pieces().zip(other.pieces()) {
            if !a.run.lo.same(&b.run.lo)?
                || !a.run.hi.same(&b.run.hi)?
                || a.sources != b.sources
                || !on_track(a, &b.track()?)?
            {
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
        self.insert_curve(&Curve::Segment(a, b), label, meter)
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
        self.insert_curve_costed(&Curve::Segment(a, b), label, meter)
    }

    /// This drawing with `curve` added, labelled `label`: a segment, or an arc cut where its circle
    /// is leftmost or rightmost. A segment of no length adds nothing, and a label already along a
    /// stretch stays once.
    ///
    /// # Errors
    ///
    /// As [`Live::insert`].
    pub fn insert_curve(
        &self,
        curve: &Curve,
        label: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Live, ArrangeError> {
        self.insert_curve_costed(curve, label, meter)
            .map(|(live, _)| live)
    }

    /// As [`Live::insert_curve`], with what the insert cost, summed over the curve's pieces.
    ///
    /// # Errors
    ///
    /// As [`Live::insert`].
    pub fn insert_curve_costed(
        &self,
        curve: &Curve,
        label: u32,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Live, Cost), ArrangeError> {
        let mut runs: Vec<piece::Piece> = Vec::new();
        let ring = match curve {
            Curve::Segment(a, b) => {
                runs.extend(segment(*a, *b)?);
                None
            }
            Curve::Arc(arc) => {
                let ring = Arc::new(Ring::of(arc.round())?);
                cut(arc, 0, slice::from_ref(&ring.circle), &mut runs)?;
                Some(ring)
            }
        };
        if runs.is_empty() {
            return Ok((self.clone(), Cost::default()));
        }
        let sources: Arc<[u32]> = Arc::from([label]);
        let mut edit = Edit::of(self);
        for run in runs {
            let s = Piece {
                run,
                ring: ring.clone(),
                sources: sources.clone(),
            };
            edit.insert(&s, meter)?;
        }
        Ok(edit.done())
    }

    /// This drawing with everything of `other` laid over it: the arrangement of the union of the
    /// curves of both, with the labels of both along each piece.
    ///
    /// # Errors
    ///
    /// As [`Live::insert`].
    pub fn overlay(&self, other: &Live, meter: &mut WorkMeter<'_>) -> Result<Live, ArrangeError> {
        let mut edit = Edit::of(self);
        for piece in other.pieces() {
            edit.insert(piece, meter)?;
        }
        Ok(edit.done().0)
    }

    /// The pieces that have `v` at an end.
    fn at(&self, v: &At) -> Result<Vec<Piece>, ArrangeError> {
        let vbox = point_box(v);
        let mut out = Vec::new();
        for chunk in &self.chunks {
            if !meets(&chunk.bounds, &vbox) {
                continue;
            }
            for piece in &chunk.pieces {
                if meets(&piece.bounds(), &vbox)
                    && (piece.run.lo.same(v)? || piece.run.hi.same(v)?)
                {
                    out.push(piece.clone());
                }
            }
        }
        Ok(out)
    }

    /// Where the ray straight down along `vertical` meets the piece `e`, whose x range holds the
    /// ray's: the top end of a vertical piece, or the point of `e`'s line or half circle there.
    fn hit(e: &Piece, vertical: &Carrier) -> Result<At, ArrangeError> {
        if e.run.lo.cmp_x(&e.run.hi)? == Sign::Zero {
            return Ok(e.run.hi.clone());
        }
        match (e.run.on, &e.ring) {
            (On::Line { line, .. }, _) => Ok(At::Rational(vertical.meet(&line)?)),
            (On::Arc { upper, .. }, Some(ring)) => {
                // The vertical meets the circle twice (once where it touches it); the point on the
                // piece's half is the one not below the centre for the upper half, not above it for
                // the lower.
                let centre = At::Rational(ring.circle.centre);
                for root in [Root::First, Root::Second] {
                    let z = At::Root(Box::new(Crossing2::of_round(
                        vertical,
                        &ring.circle.round,
                        root,
                    )?));
                    let level = z.cmp_y(&centre)?;
                    if (upper && level != Sign::Negative) || (!upper && level != Sign::Positive) {
                        return Ok(z);
                    }
                }
                // A point of the piece is on the vertical, so a root is on its half; a step that
                // found none is past the budget.
                Err(ArrangeError::Range)
            }
            (On::Arc { .. }, None) => Err(ArrangeError::TooMany),
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
        let at = At::Rational(*p);
        let ([px, py], pw) = p.coordinates();
        let x = floor(px, pw);
        let column: Box4 = [x, i128::MIN, x, floor(py, pw)];
        let vertical = Carrier::new(pw, 0, px)?;
        // The closest hit below p: its point, the first piece found that reaches it, and whether
        // that piece ends there. Pieces meet only at their ends, so a point that is the end of one
        // piece reaching it is the end of every piece reaching it, and any one piece decides.
        let mut best: Option<(At, Piece, bool)> = None;
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
                if e.run.lo.same(&at)? || e.run.hi.same(&at)? {
                    return Ok(Location::Vertex(at));
                }
                if holds(e, &at)? {
                    return Ok(Location::Edge(e.half_from(&e.run.lo)?));
                }
                if e.run.lo.cmp_x(&at)? == Sign::Positive || e.run.hi.cmp_x(&at)? == Sign::Negative
                {
                    continue;
                }
                let h = Live::hit(e, &vertical)?;
                if h.cmp_y(&at)? != Sign::Negative {
                    continue;
                }
                let higher = match &best {
                    Some((b, _, _)) => h.cmp_y(b)? == Sign::Positive,
                    None => true,
                };
                if higher {
                    let ends = e.run.lo.same(&h)? || e.run.hi.same(&h)?;
                    best = Some((h, e.clone(), ends));
                }
            }
        }
        meter.charge(looked.saturating_add(1))?;
        let Some((h, under, at_end)) = best else {
            return Ok(Location::Face(None));
        };
        if !at_end {
            // Under an edge's inside: the face above it is on the left of the edge as it runs.
            return Ok(Location::Face(Some(under.half_from(&under.run.lo)?)));
        }
        // At a vertex: the face is the one the first edge clockwise from straight up bounds.
        let ring = self.at(&h)?;
        let up = Way::Line([0, 1]);
        match clockwise_from(&h, &up, &ring, None)? {
            Some(e) => Ok(Location::Face(Some(e.half_from(&h)?))),
            None => Ok(Location::Face(None)),
        }
    }

    /// The half-edges round the boundary the half-edge `half` starts, with the face on its left, in
    /// the order the boundary runs, starting with `half`.
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
    ) -> Result<Vec<Half>, ArrangeError> {
        let mut first: Option<Piece> = None;
        for e in self.at(&half.from)? {
            let (_, other) = e.leaving(&half.from)?;
            if other.same(&half.to)? && on_track(&e, &half.on)? {
                first = Some(e);
                break;
            }
        }
        let first = first.ok_or(ArrangeError::TooMany)?;
        let mut out: Vec<Half> = Vec::new();
        let (mut e, mut u) = (first.clone(), half.from.clone());
        let limit = self.len.saturating_mul(2).saturating_add(2);
        for _ in 0..limit {
            meter.charge(1)?;
            let step = e.half_from(&u)?;
            let v = step.to.clone();
            out.push(step);
            // The next edge is the first clockwise from the way back; with only the way back, that.
            let ring = self.at(&v)?;
            let (back, _) = e.leaving(&v)?;
            let next = clockwise_from(&v, &back, &ring, Some(&e))?.ok_or(ArrangeError::TooMany)?;
            if v.same(&half.from)? && cmp_key(&next, &first)? == Ordering::Equal {
                let (_, to) = next.leaving(&v)?;
                if to.same(&half.to)? {
                    return Ok(out);
                }
            }
            e = next;
            u = v;
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
        let key = |a: P2, b: P2| Piece {
            run: segment(a, b).unwrap().unwrap(),
            ring: None,
            sources: Arc::from([0]),
        };
        let mut empty = Edit::of(&Live::new());
        empty.remove(&key(p(0, 0), p(1, 0))).unwrap();
        let (live, cost) = empty.done();
        assert!(live.is_empty());
        assert_eq!(cost, Cost::default());
        let drawing = Live::from_noded(&lattice(3)).unwrap();
        let mut edit = Edit::of(&drawing);
        edit.remove(&key(p(0, 0), p(2, 0))).unwrap();
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

    /// The floor of a double is the whole number at or below it, for whole and fractional values
    /// either side of zero and past 2^53.
    #[test]
    fn floors_of_doubles_are_at_or_below() {
        for (v, want) in [
            (0.0, 0),
            (-0.0, 0),
            (2.5, 2),
            (-2.5, -3),
            (-3.0, -3),
            (9_007_199_254_740_993.0, 9_007_199_254_740_992),
            (-1e30, -1_000_000_000_000_000_019_884_624_838_656),
        ] {
            assert_eq!(floor_of(v), want, "{v}");
        }
    }
}

