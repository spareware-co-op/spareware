//! The arrangement of line segments (chapter 14.4): an exact plane sweep, overlap merging, the
//! order of edges at each vertex, the faces and the unbounded face, and where each edge came from.
//!
//! What it does: [`arrange`] takes segments between grid points and returns the [`Arrangement`]
//! they divide the plane into: the vertices (every end point and every crossing, as exact rational
//! points, in lexicographic order), the edges between consecutive vertices along each segment (two
//! segments that share a stretch share its edges, and an edge names every segment that covers it),
//! each vertex's edges in counter-clockwise order, and the faces: one unbounded face, and one
//! bounded face for each outer boundary, with the boundaries of the components inside it as holes.
//!
//! Guarantees:
//!
//! - **Exact.** Every decision is a sign of one of `geom-num`'s exact predicates on carriers and
//!   rational points ([`Carrier::side`], [`Carrier::order`], [`Carrier::meet`],
//!   [`Rational2::cmp_x`]); no coordinate is ever rounded, and a crossing is the exact rational
//!   point where two carriers meet. A step past the exact budget is [`ArrangeError::Range`],
//!   never a wrong answer.
//! - **Permutation-invariant.** Vertices are named by their rank in lexicographic order, edges by
//!   their end vertices, faces by the rank of their lowest-leftmost vertex; the only thing that
//!   depends on the input order is the label of each source, which is the segment's place in the
//!   input. Permuting the input gives the same arrangement with the labels permuted.
//! - **Overlaps merge, touching splits.** A segment through another's end point is split there; two
//!   segments on one line share the edges of their common stretch.
//! - **Cancellable.** The sweep charges its [`WorkMeter`] once for each event point.
//!
//! Method: Bentley and Ottmann's sweep ([DOI](https://doi.org/10.1109/TC.1979.1675432)) in the form
//! of de Berg, Cheong, van Kreveld and Overmars, *Computational Geometry*, 3rd ed., Springer 2008,
//! section 2.1 (doi:10.1007/978-3-540-77974-2), with the sweep line tilted so that a vertical segment
//! is the steepest upward one, and with every comparison an exact sign; the doubly-connected edge
//! list and the placing of holes by a leftward ray after the same book, section 2.2.
//!
//! Cost: O((n + k) log n) comparisons for the sweep, n segments and k crossings, plus the cost of
//! moving the entries of the status list on insertion, which is linear in the segments the sweep
//! line crosses at once; the faces are linear in the edges, and finding the face around each hole
//! is a second sweep over the finished edges.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "s, t, p, q, u, v, e, h are the geometry's own letters, and the sweep and the face walk are each one procedure of de Berg et al. that reads best whole"
)]

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use geom_curve::{CurveKind, Line};
use geom_math::bbox::Box3;
use geom_num::carrier::{Carrier, Rational2};
use geom_num::grid::{P2, P3};
use geom_num::{NumError, Sign};
use geom_run::{Stopped, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_topo::{
    Body, BodyKind, Builder, Edge as TopoEdge, Face as TopoFace, Fin, Ix, Loop, Region, RegionKind,
    Sense, Shell, TopoError, Vertex,
};

/// Why an arrangement was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrangeError {
    /// A step of the exact arithmetic did not fit its budget.
    Range,
    /// More segments, vertices or edges than a 32-bit index names.
    TooMany,
    /// The work meter stopped the sweep.
    Stopped(Stopped),
}

impl From<NumError> for ArrangeError {
    fn from(_: NumError) -> Self {
        ArrangeError::Range
    }
}

impl From<Stopped> for ArrangeError {
    fn from(s: Stopped) -> Self {
        ArrangeError::Stopped(s)
    }
}

impl fmt::Display for ArrangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArrangeError::Range => f.write_str("a step of the exact arithmetic did not fit"),
            ArrangeError::TooMany => f.write_str("more items than a 32-bit index names"),
            ArrangeError::Stopped(s) => write!(f, "stopped: {s:?}"),
        }
    }
}

impl core::error::Error for ArrangeError {}

/// An edge between two vertices, lowest first, with the segments that cover it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The vertex of the lexicographically lower end.
    pub from: u32,
    /// The vertex of the higher end.
    pub to: u32,
    /// The input places of the segments that cover the edge, ascending.
    pub sources: Vec<u32>,
}

/// A face: its outer boundary (none for the unbounded face) and the boundaries of the components
/// that lie inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// The index of the cycle that bounds the face, counter-clockwise; `None` for the unbounded face.
    pub outer: Option<u32>,
    /// The cycles of the components inside, each the outside boundary of its component, ascending.
    pub holes: Vec<u32>,
}

/// The arrangement of a set of segments.
#[derive(Clone, Debug)]
pub struct Arrangement {
    vertices: Vec<Rational2>,
    edges: Vec<Edge>,
    /// For each vertex, the half-edges that leave it, counter-clockwise from the negative-x
    /// direction. Half-edge `2 e` runs along edge `e` from `from` to `to`; `2 e + 1` runs back.
    around: Vec<Vec<u32>>,
    cycles: Vec<Vec<u32>>,
    faces: Vec<Face>,
}

impl Arrangement {
    /// The vertices, in lexicographic order of (x, y).
    #[must_use]
    pub fn vertices(&self) -> &[Rational2] {
        &self.vertices
    }

    /// The edges, in order of (`from`, `to`).
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The half-edges leaving `vertex`, counter-clockwise. Half-edge `2 e` runs from edge `e`'s
    /// `from` to its `to`, and `2 e + 1` back.
    #[must_use]
    pub fn around(&self, vertex: u32) -> &[u32] {
        usize::try_from(vertex)
            .ok()
            .and_then(|i| self.around.get(i))
            .map_or(&[], Vec::as_slice)
    }

    /// The boundary cycles, each a list of half-edges in order, the face on the left of each.
    #[must_use]
    pub fn cycles(&self) -> &[Vec<u32>] {
        &self.cycles
    }

    /// The faces: the unbounded face first, then one for each counter-clockwise cycle, by the rank
    /// of the cycle's lowest-leftmost vertex.
    #[must_use]
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }
}

/// A segment as the sweep sees it: lower end, higher end, and the line through them.
#[derive(Clone, Copy)]
struct Seg {
    lo: Rational2,
    hi: Rational2,
    line: Carrier,
    /// hi − lo, the direction of the carrier.
    dir: [i128; 2],
}

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

/// The cross product of two direction vectors, as a sign.
fn cross_sign(a: [i128; 2], b: [i128; 2]) -> Result<Ordering, ArrangeError> {
    let l = a[0].checked_mul(b[1]).ok_or(ArrangeError::Range)?;
    let r = a[1].checked_mul(b[0]).ok_or(ArrangeError::Range)?;
    Ok(l.cmp(&r))
}

/// One event: a point, and the segment that starts there if it is a start.
struct Event {
    at: Rational2,
    start: Option<u32>,
}

/// A binary min-heap of events by lexicographic point, with comparisons that can fail.
struct Heap {
    items: Vec<Event>,
}

impl Heap {
    fn push(&mut self, e: Event) -> Result<(), ArrangeError> {
        self.items.push(e);
        let mut i = self.items.len().saturating_sub(1);
        while i > 0 {
            let parent = i.saturating_sub(1) / 2;
            let (Some(a), Some(b)) = (self.items.get(i), self.items.get(parent)) else {
                break;
            };
            if cmp_pt(&a.at, &b.at)? == Ordering::Less {
                self.items.swap(i, parent);
                i = parent;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn peek(&self) -> Option<&Event> {
        self.items.first()
    }

    fn pop(&mut self) -> Result<Option<Event>, ArrangeError> {
        let Some(last) = self.items.len().checked_sub(1) else {
            return Ok(None);
        };
        self.items.swap(0, last);
        let out = self.items.pop();
        let n = self.items.len();
        let mut i = 0_usize;
        loop {
            let l = i.saturating_mul(2).saturating_add(1);
            let r = l.saturating_add(1);
            let mut least = i;
            for c in [l, r] {
                if c < n {
                    let (Some(a), Some(b)) = (self.items.get(c), self.items.get(least)) else {
                        continue;
                    };
                    if cmp_pt(&a.at, &b.at)? == Ordering::Less {
                        least = c;
                    }
                }
            }
            if least == i {
                break;
            }
            self.items.swap(i, least);
            i = least;
        }
        Ok(out)
    }
}

fn index(i: usize) -> Result<u32, ArrangeError> {
    u32::try_from(i).map_err(|_| ArrangeError::TooMany)
}

fn seg_of(segs: &[Seg], id: u32) -> Result<&Seg, ArrangeError> {
    usize::try_from(id)
        .ok()
        .and_then(|i| segs.get(i))
        .ok_or(ArrangeError::TooMany)
}

/// Whether the crossing of `s` and `t`, if they cross at one point, is a point after `p`: pushes
/// that point as an event.
fn new_event(
    heap: &mut Heap,
    segs: &[Seg],
    s: u32,
    t: u32,
    p: &Rational2,
) -> Result<(), ArrangeError> {
    let (a, b) = (seg_of(segs, s)?, seg_of(segs, t)?);
    if cross_sign(a.dir, b.dir)? == Ordering::Equal {
        return Ok(());
    }
    let q = a.line.meet(&b.line)?;
    if cmp_pt(&q, p)? != Ordering::Greater {
        return Ok(());
    }
    for z in [a, b] {
        if cmp_pt(&q, &z.lo)? == Ordering::Less || cmp_pt(&q, &z.hi)? == Ordering::Greater {
            return Ok(());
        }
    }
    heap.push(Event { at: q, start: None })
}

/// The arrangement of `segments`, each between two grid points; segments of no length are ignored.
///
/// # Errors
///
/// [`ArrangeError::Range`] past the exact budget, [`ArrangeError::TooMany`] past 2^32 items and
/// [`ArrangeError::Stopped`] if the meter stops.
pub fn arrange(
    segments: &[(P2, P2)],
    meter: &mut WorkMeter<'_>,
) -> Result<Arrangement, ArrangeError> {
    // The segments, lower end first.
    let mut segs: Vec<Seg> = Vec::with_capacity(segments.len());
    let mut heap = Heap { items: Vec::new() };
    let mut ids: Vec<u32> = Vec::new();
    for (place, (a, b)) in segments.iter().enumerate() {
        let (ra, rb) = (Rational2::from(*a), Rational2::from(*b));
        let (lo, hi, p, q) = match cmp_pt(&ra, &rb)? {
            Ordering::Equal => continue,
            Ordering::Less => (ra, rb, *a, *b),
            Ordering::Greater => (rb, ra, *b, *a),
        };
        let line = Carrier::through(p, q)?;
        let [dx, dy] = line.direction();
        let id = index(segs.len())?;
        segs.push(Seg {
            lo,
            hi,
            line,
            dir: [dx, dy],
        });
        ids.push(index(place)?);
        heap.push(Event {
            at: lo,
            start: Some(id),
        })?;
        heap.push(Event {
            at: hi,
            start: None,
        })?;
    }
    // The status: the segments the sweep line crosses, lowest first.
    let mut status: Vec<u32> = Vec::new();
    // The vertices, and for each segment the vertices on it, in order.
    let mut vertices: Vec<Rational2> = Vec::new();
    let mut on: Vec<Vec<u32>> = alloc::vec![Vec::new(); segs.len()];
    while let Some(first) = heap.pop()? {
        meter.charge(1)?;
        let p = first.at;
        let mut starting: Vec<u32> = first.start.into_iter().collect();
        while let Some(next) = heap.peek() {
            if cmp_pt(&next.at, &p)? != Ordering::Equal {
                break;
            }
            if let Some(e) = heap.pop()?
                && let Some(s) = e.start
            {
                starting.push(s);
            }
        }
        // The status segments through p are contiguous: those with p above them come first.
        let mut lo = 0_usize;
        let mut hi = status.len();
        // lo: first entry with p not above; hi: first entry with p below.
        let (mut a, mut b) = (0_usize, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let s = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if seg_of(&segs, s)?.line.side(&p)? == Sign::Positive {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        lo = lo.max(a);
        let (mut a, mut b) = (lo, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let s = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if seg_of(&segs, s)?.line.side(&p)? == Sign::Zero {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        hi = hi.min(a);
        let through: Vec<u32> = status.get(lo..hi).map(<[u32]>::to_vec).unwrap_or_default();
        if through.is_empty() && starting.is_empty() {
            continue;
        }
        let v = index(vertices.len())?;
        vertices.push(p);
        let mut keep: Vec<u32> = Vec::new();
        for &s in &through {
            if let Some(list) = usize::try_from(s).ok().and_then(|i| on.get_mut(i)) {
                list.push(v);
            }
            if cmp_pt(&seg_of(&segs, s)?.hi, &p)? != Ordering::Equal {
                keep.push(s);
            }
        }
        for &s in &starting {
            if let Some(list) = usize::try_from(s).ok().and_then(|i| on.get_mut(i)) {
                list.push(v);
            }
        }
        // Those that go on, in the order just after p: by direction, lowest first.
        keep.extend(starting.iter().copied());
        let mut fail: Option<ArrangeError> = None;
        keep.sort_by(|&x, &y| {
            let (Ok(sx), Ok(sy)) = (seg_of(&segs, x), seg_of(&segs, y)) else {
                fail = Some(ArrangeError::TooMany);
                return Ordering::Equal;
            };
            match cross_sign(sy.dir, sx.dir) {
                Ok(Ordering::Equal) => x.cmp(&y),
                Ok(o) => o,
                Err(e) => {
                    fail = Some(e);
                    Ordering::Equal
                }
            }
        });
        if let Some(e) = fail {
            return Err(e);
        }
        let count = keep.len();
        let tail = status.split_off(hi);
        status.truncate(lo);
        status.extend(keep);
        status.extend(tail);
        let end = lo.saturating_add(count);
        if count == 0 {
            if let (Some(l), Some(r)) = (
                lo.checked_sub(1).and_then(|i| status.get(i)).copied(),
                status.get(lo).copied(),
            ) {
                new_event(&mut heap, &segs, l, r, &p)?;
            }
        } else {
            if let (Some(l), Some(r)) = (
                lo.checked_sub(1).and_then(|i| status.get(i)).copied(),
                status.get(lo).copied(),
            ) {
                new_event(&mut heap, &segs, l, r, &p)?;
            }
            if let (Some(l), Some(r)) = (
                end.checked_sub(1).and_then(|i| status.get(i)).copied(),
                status.get(end).copied(),
            ) {
                new_event(&mut heap, &segs, l, r, &p)?;
            }
        }
    }
    build(&segs, &ids, vertices, &on)
}

/// The edges, the order around each vertex and the faces, from the vertices on each segment.
fn build(
    segs: &[Seg],
    ids: &[u32],
    vertices: Vec<Rational2>,
    on: &[Vec<u32>],
) -> Result<Arrangement, ArrangeError> {
    // Edges: consecutive vertices along each segment, with the segment as a source.
    let mut raw: Vec<(u32, u32, u32, u32)> = Vec::new();
    for (si, list) in on.iter().enumerate() {
        let place = ids.get(si).copied().ok_or(ArrangeError::TooMany)?;
        let id = index(si)?;
        for pair in list.windows(2) {
            if let [u, v] = *pair
                && u != v
            {
                raw.push((u, v, place, id));
            }
        }
    }
    raw.sort_unstable();
    let mut edges: Vec<Edge> = Vec::new();
    // The segment that stands for each edge's direction.
    let mut rep: Vec<u32> = Vec::new();
    for &(u, v, place, id) in &raw {
        match edges.last_mut() {
            Some(e) if e.from == u && e.to == v => e.sources.push(place),
            _ => {
                edges.push(Edge {
                    from: u,
                    to: v,
                    sources: alloc::vec![place],
                });
                rep.push(id);
            }
        }
    }
    let dir_of = |h: u32| -> Result<[i128; 2], ArrangeError> {
        let e = usize::try_from(h / 2).map_err(|_| ArrangeError::TooMany)?;
        let s = rep.get(e).copied().ok_or(ArrangeError::TooMany)?;
        let [dx, dy] = seg_of(segs, s)?.dir;
        if h.is_multiple_of(2) {
            Ok([dx, dy])
        } else {
            Ok([
                dx.checked_neg().ok_or(ArrangeError::Range)?,
                dy.checked_neg().ok_or(ArrangeError::Range)?,
            ])
        }
    };
    // Around each vertex: the half-edges that leave it, counter-clockwise from the negative x axis.
    let mut around: Vec<Vec<u32>> = alloc::vec![Vec::new(); vertices.len()];
    for (ei, e) in edges.iter().enumerate() {
        let h = index(ei.checked_mul(2).ok_or(ArrangeError::TooMany)?)?;
        if let Some(list) = usize::try_from(e.from).ok().and_then(|i| around.get_mut(i)) {
            list.push(h);
        }
        if let Some(list) = usize::try_from(e.to).ok().and_then(|i| around.get_mut(i)) {
            list.push(h.saturating_add(1));
        }
    }
    let mut fail: Option<ArrangeError> = None;
    for list in &mut around {
        list.sort_by(|&x, &y| {
            let (Ok(dx), Ok(dy)) = (dir_of(x), dir_of(y)) else {
                fail = Some(ArrangeError::Range);
                return Ordering::Equal;
            };
            // Upper half-plane (y > 0, or y = 0 and x < 0) first would start at the negative x
            // axis; the half is the first key, the cross product the second.
            let half = |d: [i128; 2]| u8::from(d[1] > 0 || (d[1] == 0 && d[0] > 0));
            match half(dx).cmp(&half(dy)) {
                Ordering::Equal => match cross_sign(dy, dx) {
                    Ok(o) => o,
                    Err(e) => {
                        fail = Some(e);
                        Ordering::Equal
                    }
                },
                other => other,
            }
        });
    }
    if let Some(e) = fail {
        return Err(e);
    }
    // The place of each half-edge in the list around its origin.
    let origin = |h: u32| -> Result<usize, ArrangeError> {
        let e = usize::try_from(h / 2).map_err(|_| ArrangeError::TooMany)?;
        let edge = edges.get(e).ok_or(ArrangeError::TooMany)?;
        usize::try_from(if h.is_multiple_of(2) {
            edge.from
        } else {
            edge.to
        })
        .map_err(|_| ArrangeError::TooMany)
    };
    let mut place_in: Vec<usize> = alloc::vec![0; edges.len().saturating_mul(2)];
    for list in &around {
        for (i, &h) in list.iter().enumerate() {
            if let Some(slot) = usize::try_from(h).ok().and_then(|k| place_in.get_mut(k)) {
                *slot = i;
            }
        }
    }
    // next(h): at the end of h, the half-edge clockwise from its twin.
    let next = |h: u32| -> Result<u32, ArrangeError> {
        let twin = h ^ 1;
        let at = origin(twin)?;
        let list = around.get(at).ok_or(ArrangeError::TooMany)?;
        let i = place_in
            .get(usize::try_from(twin).map_err(|_| ArrangeError::TooMany)?)
            .copied()
            .ok_or(ArrangeError::TooMany)?;
        let k = if i == 0 {
            list.len().saturating_sub(1)
        } else {
            i.saturating_sub(1)
        };
        list.get(k).copied().ok_or(ArrangeError::TooMany)
    };
    let total = edges.len().saturating_mul(2);
    let mut seen: Vec<bool> = alloc::vec![false; total];
    let mut cycles: Vec<Vec<u32>> = Vec::new();
    for start in 0..total {
        if seen.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut cycle: Vec<u32> = Vec::new();
        let mut h = index(start)?;
        loop {
            let k = usize::try_from(h).map_err(|_| ArrangeError::TooMany)?;
            if seen.get(k).copied().unwrap_or(true) {
                break;
            }
            if let Some(s) = seen.get_mut(k) {
                *s = true;
            }
            cycle.push(h);
            h = next(h)?;
        }
        cycles.push(cycle);
    }
    // Orientation. A component's outside boundary is the cycle that has the wedge toward the
    // negative x direction at the component's lowest-leftmost vertex; with the sweep line tilted,
    // that direction lies just after an edge that runs exactly west. Every other cycle of the
    // component bounds a face from inside. At a cycle's own lowest-leftmost vertex, the cycle is the
    // outside kind exactly when it holds the half-edge that leaves that vertex just before the
    // wedge, going counter-clockwise.
    let mut cycle_of_half: Vec<usize> = alloc::vec![0; total];
    for (c, cycle) in cycles.iter().enumerate() {
        for &h in cycle {
            if let Some(slot) = usize::try_from(h)
                .ok()
                .and_then(|k| cycle_of_half.get_mut(k))
            {
                *slot = c;
            }
        }
    }
    let mut counter: Vec<bool> = Vec::new();
    let mut low: Vec<usize> = Vec::new();
    for (c, cycle) in cycles.iter().enumerate() {
        let mut best: Option<usize> = None;
        for &h in cycle {
            let o = origin(h)?;
            if best.is_none_or(|b| o < b) {
                best = Some(o);
            }
        }
        let m = best.ok_or(ArrangeError::TooMany)?;
        let list = around.get(m).ok_or(ArrangeError::TooMany)?;
        let first = list.first().copied().ok_or(ArrangeError::TooMany)?;
        let [fx, fy] = dir_of(first)?;
        let skip = usize::from(fy == 0 && fx < 0);
        let n = list.len();
        let before = skip
            .checked_add(n)
            .and_then(|v| v.checked_sub(1))
            .and_then(|v| v.checked_rem(n))
            .ok_or(ArrangeError::TooMany)?;
        let out = list.get(before).copied().ok_or(ArrangeError::TooMany)?;
        let outside = usize::try_from(out)
            .ok()
            .and_then(|k| cycle_of_half.get(k))
            .copied()
            == Some(c);
        counter.push(!outside);
        low.push(m);
    }
    let (faces, _) = faces_of(&edges, &vertices, segs, &rep, &cycles, &counter, &low)?;
    Ok(Arrangement {
        vertices,
        edges,
        around,
        cycles,
        faces,
    })
}

/// The faces: the unbounded one, then one for each counter-clockwise cycle, with the holes placed.
#[allow(
    clippy::too_many_arguments,
    reason = "the pieces of one arrangement, passed as they are built"
)]
fn faces_of(
    edges: &[Edge],
    vertices: &[Rational2],
    segs: &[Seg],
    rep: &[u32],
    cycles: &[Vec<u32>],
    counter: &[bool],
    low: &[usize],
) -> Result<(Vec<Face>, Vec<u32>), ArrangeError> {
    // Face numbers: by the rank of the lowest vertex of each counter-clockwise cycle.
    let mut order: Vec<usize> = (0..cycles.len())
        .filter(|&c| counter.get(c).copied().unwrap_or(false))
        .collect();
    order.sort_by_key(|&c| (low.get(c).copied().unwrap_or(0), c));
    let mut face_of_cycle: Vec<Option<u32>> = alloc::vec![None; cycles.len()];
    let mut faces = alloc::vec![Face {
        outer: None,
        holes: Vec::new(),
    }];
    for &c in &order {
        let fi = index(faces.len())?;
        faces.push(Face {
            outer: Some(index(c)?),
            holes: Vec::new(),
        });
        if let Some(slot) = face_of_cycle.get_mut(c) {
            *slot = Some(fi);
        }
    }
    // The cycle each half-edge is on.
    let mut cycle_of: Vec<u32> = alloc::vec![0; edges.len().saturating_mul(2)];
    for (c, cycle) in cycles.iter().enumerate() {
        for &h in cycle {
            if let Some(slot) = usize::try_from(h).ok().and_then(|k| cycle_of.get_mut(k)) {
                *slot = index(c)?;
            }
        }
    }
    // The parent cycle of each clockwise cycle: the cycle of the edge nearest below its lowest-
    // leftmost vertex. A second sweep over the finished edges finds it: the vertices are in
    // lexicographic order already, the status holds the edges the sweep line crosses, lowest first,
    // and no two cross, so a vertex is placed among them by exact side tests. At a hole's vertex
    // the edge just below it in the status is the edge below the hole; the face above that edge
    // is on the left of its half-edge that runs rightwards.
    let seg_of_edge = |e: usize| -> Result<&Seg, ArrangeError> {
        let s = rep.get(e).copied().ok_or(ArrangeError::TooMany)?;
        seg_of(segs, s)
    };
    let mut hole_at: Vec<Option<usize>> = alloc::vec![None; vertices.len()];
    for (c, m) in low.iter().enumerate() {
        if !counter.get(c).copied().unwrap_or(true)
            && let Some(slot) = hole_at.get_mut(*m)
        {
            *slot = Some(c);
        }
    }
    let mut parent: Vec<Option<u32>> = alloc::vec![None; cycles.len()];
    let mut status: Vec<usize> = Vec::new();
    let mut next_edge = 0_usize;
    for (v, at) in vertices.iter().enumerate() {
        // The edges that end here are those the vertex lies on.
        let (mut a, mut b) = (0_usize, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let e = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if seg_of_edge(e)?.line.side(at)? == Sign::Positive {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let lo = a;
        let (mut a, mut b) = (lo, status.len());
        while a < b {
            let mid = a.saturating_add(b.saturating_sub(a) / 2);
            let e = status.get(mid).copied().ok_or(ArrangeError::TooMany)?;
            if seg_of_edge(e)?.line.side(at)? == Sign::Zero {
                a = mid.saturating_add(1);
            } else {
                b = mid;
            }
        }
        let hi = a;
        let tail = status.split_off(hi);
        status.truncate(lo);
        status.extend(tail);
        if let Some(Some(c)) = hole_at.get(v).copied()
            && let Some(below) = lo.checked_sub(1).and_then(|k| status.get(k)).copied()
        {
            let h = index(below.checked_mul(2).ok_or(ArrangeError::TooMany)?)?;
            let on_cycle = cycle_of
                .get(usize::try_from(h).map_err(|_| ArrangeError::TooMany)?)
                .copied()
                .ok_or(ArrangeError::TooMany)?;
            if let Some(slot) = parent.get_mut(c) {
                *slot = Some(on_cycle);
            }
        }
        // The edges that start here, in the order just after the vertex: by direction.
        let mut starting: Vec<usize> = Vec::new();
        while let Some(e) = edges.get(next_edge) {
            if usize::try_from(e.from).ok() != Some(v) {
                break;
            }
            starting.push(next_edge);
            next_edge = next_edge.saturating_add(1);
        }
        let mut fail: Option<ArrangeError> = None;
        starting.sort_by(|&x, &y| {
            let (Ok(sx), Ok(sy)) = (seg_of_edge(x), seg_of_edge(y)) else {
                fail = Some(ArrangeError::TooMany);
                return Ordering::Equal;
            };
            match cross_sign(sy.dir, sx.dir) {
                Ok(o) => o,
                Err(e) => {
                    fail = Some(e);
                    Ordering::Equal
                }
            }
        });
        if let Some(e) = fail {
            return Err(e);
        }
        let tail = status.split_off(lo);
        status.extend(starting);
        status.extend(tail);
    }
    // Resolve each cycle to a face: a counter-clockwise cycle is its own; a clockwise one is in the
    // face of its parent, or the unbounded face.
    let resolve = |start: usize| -> Result<u32, ArrangeError> {
        let mut c = start;
        for _ in 0..=cycles.len() {
            if let Some(Some(f)) = face_of_cycle.get(c) {
                return Ok(*f);
            }
            match parent.get(c).copied().flatten() {
                Some(p) => c = usize::try_from(p).map_err(|_| ArrangeError::TooMany)?,
                None => return Ok(0),
            }
        }
        Ok(0)
    };
    for c in 0..cycles.len() {
        if counter.get(c).copied().unwrap_or(true) {
            continue;
        }
        let f = resolve(c)?;
        let hole = index(c)?;
        if let Some(face) = usize::try_from(f).ok().and_then(|i| faces.get_mut(i)) {
            face.holes.push(hole);
        }
    }
    Ok((faces, cycle_of))
}

/// Why an arrangement could not be stored as a body.
#[derive(Clone, Debug)]
pub enum BodyError {
    /// A vertex is not a grid point, and the body's points are: the first such vertex, by rank
    /// (the vertices are in lexicographic order), and where it is. The exact [`Arrangement`] still
    /// holds it.
    OffGrid {
        /// The vertex's rank.
        vertex: u32,
        /// Where it is.
        at: Rational2,
    },
    /// More items than a 32-bit index names.
    TooMany,
    /// The geometry of a plane or a line could not be made.
    Geometry,
    /// The topology refused a row, or the meter stopped publishing.
    Topo(TopoError),
}

impl From<TopoError> for BodyError {
    fn from(e: TopoError) -> Self {
        BodyError::Topo(e)
    }
}

/// The index of row `n`.
fn row<K: geom_topo::Kind>(n: usize) -> Result<Ix<K>, BodyError> {
    Ix::new(u32::try_from(n).map_err(|_| BodyError::TooMany)?).ok_or(BodyError::TooMany)
}

impl Arrangement {
    /// The arrangement as a `General` body, in the plane z = 0: a vertex for each vertex, an edge on
    /// a line segment for each edge, one face on the plane for each bounded face, its outer loop
    /// first and the loops of the components inside it after, one fin for each side of an edge that
    /// a bounded face lies on (the unbounded face has none), and one shell in the outside region.
    ///
    /// The body's points are grid points, so the arrangement must have no crossing off the grid
    /// (the options for storing exact points are in the pull request that added this); the first
    /// that is off the grid is the error.
    ///
    /// # Errors
    ///
    /// [`BodyError::OffGrid`], [`BodyError::TooMany`], [`BodyError::Geometry`] and
    /// [`BodyError::Topo`].
    pub fn to_body(&self, meter: &mut WorkMeter<'_>) -> Result<Body, BodyError> {
        // The vertices' points.
        let mut points: Vec<P3> = Vec::with_capacity(self.vertices.len());
        for (rank, v) in self.vertices.iter().enumerate() {
            let ([x, y], w) = v.coordinates();
            let grid = x
                .checked_rem(w)
                .zip(y.checked_rem(w))
                .is_some_and(|(a, b)| a == 0 && b == 0);
            let at = x.checked_div(w).zip(y.checked_div(w));
            let point = at.filter(|_| grid).and_then(|(gx, gy)| {
                P3::new(i64::try_from(gx).ok()?, i64::try_from(gy).ok()?, 0).ok()
            });
            match point {
                Some(p) => points.push(p),
                None => {
                    return Err(BodyError::OffGrid {
                        vertex: index(rank).map_err(|_| BodyError::TooMany)?,
                        at: *v,
                    });
                }
            }
        }
        // The cycle each half-edge is on.
        let total = self.edges.len().saturating_mul(2);
        let mut cycle_of_half: Vec<usize> = alloc::vec![usize::MAX; total];
        for (c, cycle) in self.cycles.iter().enumerate() {
            for &h in cycle {
                if let Some(slot) = usize::try_from(h)
                    .ok()
                    .and_then(|k| cycle_of_half.get_mut(k))
                {
                    *slot = c;
                }
            }
        }
        // Which bounded face (numbered from 0, the unbounded face left out) each cycle lies in, and
        // the order of the loops: for each bounded face its outer cycle, then its holes.
        let mut face_of_cycle: Vec<Option<usize>> = alloc::vec![None; self.cycles.len()];
        let mut loop_of_cycle: Vec<Option<usize>> = alloc::vec![None; self.cycles.len()];
        let mut loops: Vec<(usize, usize)> = Vec::new();
        for (fi, face) in self.faces.iter().enumerate().skip(1) {
            for c in face.outer.iter().chain(face.holes.iter()) {
                let k = usize::try_from(*c).map_err(|_| BodyError::TooMany)?;
                if let Some(slot) = face_of_cycle.get_mut(k) {
                    *slot = Some(fi.saturating_sub(1));
                }
                if let Some(slot) = loop_of_cycle.get_mut(k) {
                    *slot = Some(loops.len());
                }
                loops.push((fi.saturating_sub(1), k));
            }
        }
        // One fin for each half-edge on a cycle of a bounded face, in half-edge order.
        let mut fin_of_half: Vec<Option<usize>> = alloc::vec![None; total];
        let mut fins: Vec<usize> = Vec::new();
        for (h, slot) in fin_of_half.iter_mut().enumerate() {
            let c = cycle_of_half.get(h).copied().unwrap_or(usize::MAX);
            if face_of_cycle.get(c).copied().flatten().is_some() {
                *slot = Some(fins.len());
                fins.push(h);
            }
        }
        let mut b = Builder::new();
        let a = &mut b.arenas;
        // The outside region, holding the one shell.
        a.regions.push(Region {
            kind: RegionKind::Void,
            first_shell: Some(row(0)?),
        })?;
        // The plane z = 0.
        let plane = Plane::new(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            (
                (f64::NEG_INFINITY, f64::INFINITY),
                (f64::NEG_INFINITY, f64::INFINITY),
            ),
        )
        .map_err(|_| BodyError::Geometry)?;
        let surface = a.surfaces.push(SurfaceKind::Plane(plane))?;
        // Vertices.
        for p in &points {
            let at = a.points.push(*p)?;
            a.vertices.push(Vertex { point: at })?;
        }
        // Edges, each on the segment between its end points.
        let flat = |v: u32| -> Result<[f64; 3], BodyError> {
            let p = usize::try_from(v)
                .ok()
                .and_then(|k| points.get(k))
                .ok_or(BodyError::TooMany)?;
            #[allow(
                clippy::cast_precision_loss,
                reason = "grid coordinates are below 2^40"
            )]
            Ok([p.x.get() as f64, p.y.get() as f64, 0.0])
        };
        for (e, edge) in self.edges.iter().enumerate() {
            let line = Line::<3>::segment(flat(edge.from)?, flat(edge.to)?)
                .map_err(|_| BodyError::Geometry)?
                .value;
            let curve = a.curves.push(CurveKind::Line(line))?;
            let own = e.checked_mul(2).ok_or(BodyError::TooMany)?;
            let first_fin = [own, own.saturating_add(1)]
                .iter()
                .find_map(|h| fin_of_half.get(*h).copied().flatten());
            a.edges.push(TopoEdge {
                first_fin: first_fin.map(row).transpose()?,
                start: Some(row(
                    usize::try_from(edge.from).map_err(|_| BodyError::TooMany)?
                )?),
                end: Some(row(
                    usize::try_from(edge.to).map_err(|_| BodyError::TooMany)?
                )?),
                curve,
            })?;
        }
        // Fins, each linked to the next and previous of its cycle and to the other side.
        for &h in &fins {
            let c = cycle_of_half.get(h).copied().unwrap_or(usize::MAX);
            let cycle = self.cycles.get(c).ok_or(BodyError::TooMany)?;
            let at = cycle
                .iter()
                .position(|x| usize::try_from(*x).ok() == Some(h))
                .ok_or(BodyError::TooMany)?;
            let n = cycle.len();
            let neighbour = |k: usize| -> Result<Ix<geom_topo::Fin>, BodyError> {
                let half = cycle
                    .get(k)
                    .and_then(|x| usize::try_from(*x).ok())
                    .ok_or(BodyError::TooMany)?;
                let fin = fin_of_half
                    .get(half)
                    .copied()
                    .flatten()
                    .ok_or(BodyError::TooMany)?;
                row(fin)
            };
            let next = neighbour(at.saturating_add(1).checked_rem(n).unwrap_or(0))?;
            let prev = neighbour(at.checked_sub(1).unwrap_or(n.saturating_sub(1)))?;
            let own = row(fin_of_half
                .get(h)
                .copied()
                .flatten()
                .ok_or(BodyError::TooMany)?)?;
            let twin = fin_of_half.get(h ^ 1).copied().flatten();
            a.fins.push(Fin {
                loop_: row(loop_of_cycle
                    .get(c)
                    .copied()
                    .flatten()
                    .ok_or(BodyError::TooMany)?)?,
                next,
                prev,
                edge: row(h / 2)?,
                sense: if h % 2 == 0 {
                    Sense::Forward
                } else {
                    Sense::Reversed
                },
                radial: twin.map(row).transpose()?.unwrap_or(own),
                pcurve: None,
            })?;
        }
        // Loops, in the order of their faces.
        for (k, &(face, c)) in loops.iter().enumerate() {
            let cycle = self.cycles.get(c).ok_or(BodyError::TooMany)?;
            let first = cycle
                .first()
                .and_then(|x| usize::try_from(*x).ok())
                .and_then(|h| fin_of_half.get(h).copied().flatten())
                .ok_or(BodyError::TooMany)?;
            let following = k.saturating_add(1);
            let next = loops
                .get(following)
                .filter(|(f, _)| *f == face)
                .map(|_| row(following))
                .transpose()?;
            a.loops.push(Loop {
                face: row(face)?,
                next,
                first_fin: Some(row(first)?),
            })?;
        }
        // Faces: on the plane, bounded by the box of their outer cycle's vertices.
        let bounded = self.faces.len().saturating_sub(1);
        for (fi, face) in self.faces.iter().enumerate().skip(1) {
            let outer = face
                .outer
                .and_then(|c| usize::try_from(c).ok())
                .ok_or(BodyError::TooMany)?;
            let cycle = self.cycles.get(outer).ok_or(BodyError::TooMany)?;
            let corners = cycle.iter().filter_map(|&h| {
                let e = self.edges.get(usize::try_from(h / 2).ok()?)?;
                let v = if h % 2 == 0 { e.from } else { e.to };
                points.get(usize::try_from(v).ok()?).copied()
            });
            let bound = Box3::from_points(corners).ok_or(BodyError::TooMany)?;
            let first_loop = loop_of_cycle.get(outer).copied().flatten();
            a.faces.push(TopoFace {
                shell: row(0)?,
                next: if fi < bounded { Some(row(fi)?) } else { None },
                surface,
                sense: Sense::Forward,
                first_loop: first_loop.map(row).transpose()?,
                bound,
            })?;
        }
        a.shells.push(Shell {
            region: row(0)?,
            next: None,
            first_face: if bounded > 0 { Some(row(0)?) } else { None },
        })?;
        Ok(b.publish(BodyKind::General, row(0)?, meter)?)
    }
}
