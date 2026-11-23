//! A floor's boundary: one closed outer loop and any number of closed holes, each a chain of
//! edges in plan, held as the sketch solver's geometry (construct-solver's [`Sketch`]: points,
//! lines and arcs in grid steps) so it can be constrained later (job 5.2), with the loops naming
//! the sketch's lines and arcs in order. Every edge has a permanent ID of its own, which names
//! the floor's side faces, so a boundary replaced with the same IDs keeps its faces' names.
//!
//! Checks ([`Boundary::new`]), each refusal an [`SlabError`] naming the loop ([`Ring`]) and the
//! edge or edges, in this order:
//! 1. the sketch holds no constraints yet;
//! 2. walking the loops, outer first: no edge is an arc (for now: the engine's extrusion takes a
//!    polygon, so an arc edge cannot be built exactly; the data model and the stored form hold
//!    one already), every edge names a line the sketch has, no two edges share an ID, and no two
//!    are one line;
//! 3. every line, curve and point of the sketch is on an edge;
//! 4. loop by loop: at least three edges, each ending where the next starts and the last where
//!    the first starts, none of no length, and no two crossing, touching or running over each
//!    other (exact orientation tests on the grid);
//! 5. each hole wholly inside the outer loop, touching nothing of it;
//! 6. no two holes overlapping or touching, and neither inside the other.
//!
//! A simple loop always encloses an area, so no loop that passes 4 encloses none.

use crate::error::SlabError;
use construct_solver::{CurveId, LineId, Point, PointId, Sketch, SketchError};
use kit_core::Id;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fmt;

/// The version of the stored boundary this crate writes.
pub const BOUNDARY_VERSION: u64 = 1;

/// A loop's corners in order, each with the ID of the edge that starts there.
type Corners = Vec<(Id, Point)>;

/// Which loop of a boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ring {
    /// The outer loop.
    Outer,
    /// A hole, by its index (0 first).
    Hole(usize),
}

impl fmt::Display for Ring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Outer => write!(f, "the outer loop"),
            Self::Hole(at) => write!(f, "hole {at}"),
        }
    }
}

/// What an edge is in the sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// A straight line, run from its start to its end.
    Line(LineId),
    /// An arc, run from its start to its end (anticlockwise about its centre), or the other way
    /// when `reversed`. Held, and refused by [`Boundary::new`] for now.
    Arc {
        /// The arc.
        curve: CurveId,
        /// Whether the loop runs it from its end to its start.
        reversed: bool,
    },
}

/// One edge of a loop: its permanent ID and what it is in the sketch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Its permanent ID, unique in the floor: its side faces are named by it.
    pub id: Id,
    /// The line or arc it is.
    pub edge: Edge,
}

/// A corner of a loop as a command gives it: where it is, and the ID of the edge that starts
/// there, or `None` for the default (`o<n>` on the outer loop, `h<k>-<n>` on hole `k`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corner {
    /// The ID of the edge from this corner to the next.
    pub id: Option<Id>,
    /// Where it is, in grid steps.
    pub at: Point,
}

/// A floor's boundary. Every value of this type has passed every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    sketch: Sketch,
    outer: Vec<Segment>,
    holes: Vec<Vec<Segment>>,
}

fn malformed(why: impl Into<String>) -> SlabError {
    SlabError::Malformed(why.into())
}

fn from_sketch(error: SketchError) -> SlabError {
    match error {
        SketchError::OutOfRange => SlabError::OutOfRange,
        other => malformed(format!("the sketch refused it: {other}")),
    }
}

/// `b - a` along both axes, exact: every coordinate is within the solver's range (2^40 steps), so
/// no difference or product of two differences comes near the range of an `i128`.
fn diff(a: Point, b: Point) -> (i128, i128) {
    (
        i128::from(b.x).saturating_sub(i128::from(a.x)),
        i128::from(b.y).saturating_sub(i128::from(a.y)),
    )
}

/// The cross product of `b - a` and `c - a`: above 0 when `c` is to the left of `a` to `b`.
fn orient(a: Point, b: Point, c: Point) -> i128 {
    let (ux, uy) = diff(a, b);
    let (vx, vy) = diff(a, c);
    ux.saturating_mul(vy).saturating_sub(uy.saturating_mul(vx))
}

/// The dot product of `b - a` and `c - a`.
fn dot(a: Point, b: Point, c: Point) -> i128 {
    let (ux, uy) = diff(a, b);
    let (vx, vy) = diff(a, c);
    ux.saturating_mul(vx).saturating_add(uy.saturating_mul(vy))
}

/// Whether `p`, on the line through `a` and `b`, lies on the segment between them.
fn within(p: Point, a: Point, b: Point) -> bool {
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// Whether the segments `a`-`b` and `c`-`d` have any point in common.
fn touch((a, b): (Point, Point), (c, d): (Point, Point)) -> bool {
    let (d1, d2) = (orient(c, d, a), orient(c, d, b));
    let (d3, d4) = (orient(a, b, c), orient(a, b, d));
    if d1.signum().saturating_mul(d2.signum()) < 0 && d3.signum().saturating_mul(d4.signum()) < 0 {
        return true;
    }
    (d1 == 0 && within(a, c, d))
        || (d2 == 0 && within(b, c, d))
        || (d3 == 0 && within(c, a, b))
        || (d4 == 0 && within(d, a, b))
}

/// Whether two edges that meet end to start at `at` run over each other: `from` is the far end
/// of one, `to` the far end of the other.
fn fold(from: Point, at: Point, to: Point) -> bool {
    orient(from, at, to) == 0 && dot(at, from, to) > 0
}

/// Whether `p` is inside the loop through `corners` (winding number not 0). Only asked of a point
/// known to be on no edge of the loop.
fn inside(p: Point, corners: &[Point]) -> bool {
    let mut winding = 0_i64;
    for (at, a) in corners.iter().enumerate() {
        let b = corners
            .get(at.saturating_add(1))
            .or_else(|| corners.first())
            .copied()
            .unwrap_or(*a);
        if a.y <= p.y {
            if b.y > p.y && orient(*a, b, p) > 0 {
                winding = winding.saturating_add(1);
            }
        } else if b.y <= p.y && orient(*a, b, p) < 0 {
            winding = winding.saturating_sub(1);
        }
    }
    winding != 0
}

/// Twice the signed area of the loop through `corners`: above 0 when it runs anticlockwise.
pub(crate) fn twice_signed_area(corners: &[Point]) -> i128 {
    let origin = corners.first().copied().unwrap_or(Point { x: 0, y: 0 });
    let mut twice = 0_i128;
    for pair in corners.windows(2) {
        if let [a, b] = pair {
            twice = twice.saturating_add(orient(origin, *a, *b));
        }
    }
    twice
}

/// The edges of a loop as pairs of corners, each with its edge's ID.
fn edges(ring: &[(Id, Point)]) -> Vec<(&Id, (Point, Point))> {
    ring.iter()
        .enumerate()
        .map(|(at, (id, a))| {
            let b = ring
                .get(at.saturating_add(1))
                .or_else(|| ring.first())
                .map_or(*a, |(_, p)| *p);
            (id, (*a, b))
        })
        .collect()
}

impl Boundary {
    /// A boundary from a sketch and its loops, checked as the module header says.
    ///
    /// # Errors
    /// The first refusal, in the module header's order.
    pub fn new(
        sketch: Sketch,
        outer: Vec<Segment>,
        holes: Vec<Vec<Segment>>,
    ) -> Result<Self, SlabError> {
        if !sketch.constraints().is_empty() {
            return Err(SlabError::Constrained);
        }
        let boundary = Self {
            sketch,
            outer,
            holes,
        };
        boundary.check_edges()?;
        boundary.check_unused()?;
        let rings = boundary.rings()?;
        for (ring, corners) in &rings {
            check_ring(*ring, corners)?;
        }
        let points = |ring: &[(Id, Point)]| ring.iter().map(|(_, p)| *p).collect::<Vec<_>>();
        let Some(((_, outer), holes)) = rings.split_first() else {
            return Err(malformed("a boundary has an outer loop"));
        };
        let outer_points = points(outer);
        for (at, (_, hole)) in holes.iter().enumerate() {
            for (segment, h) in edges(hole) {
                if let Some((outer, _)) = edges(outer).into_iter().find(|(_, o)| touch(h, *o)) {
                    return Err(SlabError::HoleCrossesOuter {
                        hole: at,
                        segment: segment.clone(),
                        outer: outer.clone(),
                    });
                }
            }
            if !hole.first().is_some_and(|(_, p)| inside(*p, &outer_points)) {
                return Err(SlabError::HoleOutside { hole: at });
            }
        }
        for (first, (_, a)) in holes.iter().enumerate() {
            for (second, (_, b)) in holes.iter().enumerate().skip(first.saturating_add(1)) {
                for (x, ea) in edges(a) {
                    if let Some((y, _)) = edges(b).into_iter().find(|(_, eb)| touch(ea, *eb)) {
                        return Err(SlabError::HolesOverlap {
                            first,
                            second,
                            segment: Some((x.clone(), y.clone())),
                        });
                    }
                }
                let nested = b.first().is_some_and(|(_, p)| inside(*p, &points(a)))
                    || a.first().is_some_and(|(_, p)| inside(*p, &points(b)));
                if nested {
                    return Err(SlabError::HolesOverlap {
                        first,
                        second,
                        segment: None,
                    });
                }
            }
        }
        Ok(boundary)
    }

    /// A boundary drawn as corners: one line from each corner to the next, the last to the
    /// first, in a new sketch with the outer loop's points and lines first, then each hole's.
    ///
    /// # Errors
    /// [`SlabError::TooFewSides`] for a loop of fewer than three corners;
    /// [`SlabError::OutOfRange`] for a corner beyond the kernel's grid; any refusal of
    /// [`Boundary::new`].
    pub fn from_corners(outer: &[Corner], holes: &[Vec<Corner>]) -> Result<Self, SlabError> {
        let mut sketch = Sketch::new();
        let outer = drawn(&mut sketch, outer, Ring::Outer)?;
        let mut rings = Vec::with_capacity(holes.len());
        for (at, hole) in holes.iter().enumerate() {
            rings.push(drawn(&mut sketch, hole, Ring::Hole(at))?);
        }
        Self::new(sketch, outer, rings)
    }

    /// A boundary from a command's plain form: `{"outer":[corner, ...],"holes":[[corner, ...],
    /// ...]}`, each corner `{"id":"<edge ID>","at":[x,y]}` in grid steps, `id` optional and
    /// `holes` optional ([`Boundary::from_corners`]).
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for a value not of that form; any refusal of
    /// [`Boundary::from_corners`].
    pub fn from_plain(value: &Value) -> Result<Self, SlabError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Plain {
            outer: Vec<PlainCorner>,
            #[serde(default)]
            holes: Vec<Vec<PlainCorner>>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PlainCorner {
            #[serde(default)]
            id: Option<String>,
            at: [i64; 2],
        }
        let plain: Plain = serde_json::from_value(value.clone())
            .map_err(|e| malformed(format!("a boundary is outer and hole corners: {e}")))?;
        let corners = |list: Vec<PlainCorner>| -> Result<Vec<Corner>, SlabError> {
            list.into_iter()
                .map(|c| {
                    let id = match c.id {
                        Some(text) => Some(
                            Id::parse(&text)
                                .map_err(|_| malformed(format!("{text:?} is not an edge's ID")))?,
                        ),
                        None => None,
                    };
                    let [x, y] = c.at;
                    Ok(Corner {
                        id,
                        at: Point { x, y },
                    })
                })
                .collect()
        };
        let outer = corners(plain.outer)?;
        let holes = plain
            .holes
            .into_iter()
            .map(corners)
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_corners(&outer, &holes)
    }

    /// The sketch its loops are drawn in.
    #[must_use]
    pub fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    /// The outer loop's edges, in order.
    #[must_use]
    pub fn outer(&self) -> &[Segment] {
        &self.outer
    }

    /// Each hole's edges, in order.
    #[must_use]
    pub fn holes(&self) -> &[Vec<Segment>] {
        &self.holes
    }

    /// One loop's edges, if the boundary has it.
    #[must_use]
    pub fn ring(&self, ring: Ring) -> Option<&[Segment]> {
        match ring {
            Ring::Outer => Some(&self.outer),
            Ring::Hole(at) => self.holes.get(at).map(Vec::as_slice),
        }
    }

    /// One loop as its corners in order, each with the ID of the edge that starts there.
    #[must_use]
    pub fn corners(&self, ring: Ring) -> Option<Vec<(Id, Point)>> {
        let segments = self.ring(ring)?;
        segments
            .iter()
            .map(|s| self.ends(s).map(|(start, _)| (s.id.clone(), start)))
            .collect()
    }

    /// The loops as the builder takes them: the outer loop anticlockwise seen from above, then
    /// each hole clockwise, each corner with the ID of the edge from it to the next. A loop drawn
    /// the other way is run backwards from the same first corner, each edge keeping its own ID.
    #[must_use]
    pub fn profile(&self) -> Vec<Vec<(Id, Point)>> {
        let mut out = Vec::with_capacity(self.holes.len().saturating_add(1));
        let rings = std::iter::once(Ring::Outer).chain((0..self.holes.len()).map(Ring::Hole));
        for ring in rings {
            let Some(corners) = self.corners(ring) else {
                continue;
            };
            let points: Vec<Point> = corners.iter().map(|(_, p)| *p).collect();
            let anticlockwise = twice_signed_area(&points) > 0;
            if anticlockwise == (ring == Ring::Outer) {
                out.push(corners);
                continue;
            }
            // Backwards: edge k runs from corner k + 1 to corner k, so from the first corner the
            // last edge comes first.
            let mut backwards = Vec::with_capacity(corners.len());
            for (at, (id, _)) in corners.iter().enumerate().rev() {
                let end = corners
                    .get(at.saturating_add(1))
                    .or_else(|| corners.first())
                    .map_or(Point { x: 0, y: 0 }, |(_, p)| *p);
                backwards.push((id.clone(), end));
            }
            out.push(backwards);
        }
        out
    }

    /// Its stored form, as a floor's recipe holds it (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let segment = |s: &Segment| match s.edge {
            Edge::Line(line) => json!({ "id": s.id.as_str(), "line": line.0 }),
            Edge::Arc { curve, reversed } => {
                json!({ "id": s.id.as_str(), "arc": curve.0, "reversed": reversed })
            }
        };
        json!({
            "version": BOUNDARY_VERSION,
            "sketch": serde_json::to_value(&self.sketch).unwrap_or(Value::Null),
            "outer": self.outer.iter().map(segment).collect::<Vec<_>>(),
            "holes": self
                .holes
                .iter()
                .map(|hole| hole.iter().map(segment).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
        })
    }

    /// Reads the stored form [`Boundary::to_value`] writes. Anything else is refused, never
    /// repaired: a version this crate does not read, a sketch the solver would not make as it
    /// stands, an edge not of the two forms, a field added, or a boundary [`Boundary::new`]
    /// refuses.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for a value not in its one spelling; any refusal of
    /// [`Boundary::new`].
    pub fn from_value(value: &Value) -> Result<Self, SlabError> {
        let object = value
            .as_object()
            .ok_or_else(|| malformed("a stored boundary is an object"))?;
        if object.get("version").and_then(Value::as_u64) != Some(BOUNDARY_VERSION) {
            return Err(malformed("a stored boundary of a version this crate reads"));
        }
        let sketch = object
            .get("sketch")
            .ok_or_else(|| malformed("a stored boundary has a sketch"))?;
        let sketch = remade(sketch)?;
        let ring = |value: Option<&Value>| -> Result<Vec<Segment>, SlabError> {
            value
                .and_then(Value::as_array)
                .ok_or_else(|| malformed("a loop is a list of edges"))?
                .iter()
                .map(read_segment)
                .collect()
        };
        let outer = ring(object.get("outer"))?;
        let holes = object
            .get("holes")
            .and_then(Value::as_array)
            .ok_or_else(|| malformed("a stored boundary lists its holes"))?
            .iter()
            .map(|hole| ring(Some(hole)))
            .collect::<Result<Vec<_>, _>>()?;
        let read = Self::new(sketch, outer, holes)?;
        if read.to_value() != *value {
            return Err(malformed("the boundary is not in its one spelling"));
        }
        Ok(read)
    }

    /// Where an edge starts and ends. `None` for an arc or a line the sketch lacks, which
    /// [`Boundary::new`] refuses.
    fn ends(&self, segment: &Segment) -> Option<(Point, Point)> {
        let Edge::Line(line) = segment.edge else {
            return None;
        };
        let line = self.sketch.line(line)?;
        Some((self.sketch.point(line.start)?, self.sketch.point(line.end)?))
    }

    /// Every edge as a line of the sketch, its ID used once and its line once; arcs refused.
    fn check_edges(&self) -> Result<(), SlabError> {
        let mut ids = BTreeSet::new();
        let mut lines = BTreeSet::new();
        for (ring, segments) in self.all() {
            for segment in segments {
                let line = match segment.edge {
                    Edge::Arc { .. } => {
                        return Err(SlabError::Arc {
                            ring,
                            segment: segment.id.clone(),
                        });
                    }
                    Edge::Line(line) => line,
                };
                if self.sketch.line(line).is_none() {
                    return Err(SlabError::UnknownEntity {
                        ring,
                        segment: segment.id.clone(),
                    });
                }
                if !ids.insert(segment.id.clone()) {
                    return Err(SlabError::SameId {
                        segment: segment.id.clone(),
                    });
                }
                if !lines.insert(line) {
                    return Err(SlabError::UsedTwice {
                        ring,
                        segment: segment.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Every line, curve and point of the sketch on an edge: lines first, then curves, then
    /// points, each in the sketch's order.
    fn check_unused(&self) -> Result<(), SlabError> {
        let used: BTreeSet<LineId> = self
            .all()
            .flat_map(|(_, segments)| segments.iter())
            .filter_map(|s| match s.edge {
                Edge::Line(line) => Some(line),
                Edge::Arc { .. } => None,
            })
            .collect();
        for at in 0..self.sketch.lines().len() {
            let id = LineId(u32::try_from(at).map_err(|_| SlabError::OutOfRange)?);
            if !used.contains(&id) {
                return Err(SlabError::Unused {
                    what: format!("line {at}"),
                });
            }
        }
        if !self.sketch.curves().is_empty() {
            return Err(SlabError::Unused {
                what: "curve 0".to_owned(),
            });
        }
        let ends: BTreeSet<PointId> = self
            .sketch
            .lines()
            .iter()
            .flat_map(|line| [line.start, line.end])
            .collect();
        for at in 0..self.sketch.points().len() {
            let id = PointId(u32::try_from(at).map_err(|_| SlabError::OutOfRange)?);
            if !ends.contains(&id) {
                return Err(SlabError::Unused {
                    what: format!("point {at}"),
                });
            }
        }
        Ok(())
    }

    /// Every loop, outer first, with which it is.
    fn all(&self) -> impl Iterator<Item = (Ring, &[Segment])> {
        std::iter::once((Ring::Outer, self.outer.as_slice())).chain(
            self.holes
                .iter()
                .enumerate()
                .map(|(at, hole)| (Ring::Hole(at), hole.as_slice())),
        )
    }

    /// Every loop as its corners, each edge closing onto the next by point.
    fn rings(&self) -> Result<Vec<(Ring, Corners)>, SlabError> {
        let mut out = Vec::new();
        for (ring, segments) in self.all() {
            if segments.len() < 3 {
                return Err(SlabError::TooFewSides {
                    ring,
                    sides: segments.len(),
                });
            }
            let lines = segments
                .iter()
                .map(|s| match s.edge {
                    Edge::Line(line) => self.sketch.line(line),
                    Edge::Arc { .. } => None,
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| malformed("an edge that is not a line of the sketch"))?;
            for (at, line) in lines.iter().enumerate() {
                let next = lines
                    .get(at.saturating_add(1))
                    .or_else(|| lines.first())
                    .ok_or_else(|| malformed("a loop without edges"))?;
                if line.end != next.start {
                    let segment = segments.get(at).map(|s| s.id.clone());
                    return Err(SlabError::NotClosed {
                        ring,
                        segment: segment.ok_or_else(|| malformed("an edge out of its loop"))?,
                    });
                }
            }
            let mut corners = Vec::with_capacity(segments.len());
            for (segment, line) in segments.iter().zip(&lines) {
                let start = self
                    .sketch
                    .point(line.start)
                    .ok_or_else(|| malformed("a line without its point"))?;
                corners.push((segment.id.clone(), start));
            }
            out.push((ring, corners));
        }
        Ok(out)
    }
}

/// One loop's own checks: no edge of no length, and no two edges crossing, touching or running
/// over each other.
fn check_ring(ring: Ring, corners: &[(Id, Point)]) -> Result<(), SlabError> {
    let edges = edges(corners);
    for (id, (a, b)) in &edges {
        if a == b {
            return Err(SlabError::ZeroLength {
                ring,
                segment: (*id).clone(),
            });
        }
    }
    let last = edges.len().saturating_sub(1);
    for (i, (first, (a, b))) in edges.iter().enumerate() {
        for (j, (second, (c, d))) in edges.iter().enumerate().skip(i.saturating_add(1)) {
            let crossing = if j == i.saturating_add(1) {
                // This edge ends where the next starts.
                fold(*a, *b, *d)
            } else if i == 0 && j == last {
                // The last edge ends where this one starts.
                fold(*b, *a, *c)
            } else {
                touch((*a, *b), (*c, *d))
            };
            if crossing {
                return Err(SlabError::Crossing {
                    ring,
                    first: (*first).clone(),
                    second: (*second).clone(),
                });
            }
        }
    }
    Ok(())
}

/// One loop drawn as corners into `sketch`: a point at each corner and a line from each to the
/// next, the last to the first.
fn drawn(sketch: &mut Sketch, corners: &[Corner], which: Ring) -> Result<Vec<Segment>, SlabError> {
    if corners.len() < 3 {
        return Err(SlabError::TooFewSides {
            ring: which,
            sides: corners.len(),
        });
    }
    let mut points = Vec::with_capacity(corners.len());
    for corner in corners {
        points.push(
            sketch
                .add_point(corner.at.x, corner.at.y)
                .map_err(from_sketch)?,
        );
    }
    let mut segments = Vec::with_capacity(corners.len());
    for (at, corner) in corners.iter().enumerate() {
        let (Some(start), Some(end)) = (
            points.get(at),
            points.get(at.saturating_add(1)).or_else(|| points.first()),
        ) else {
            return Err(malformed("a corner without a point"));
        };
        let line = sketch.add_line(*start, *end).map_err(from_sketch)?;
        let id = match &corner.id {
            Some(id) => id.clone(),
            None => default_id(which, at)?,
        };
        segments.push(Segment {
            id,
            edge: Edge::Line(line),
        });
    }
    Ok(segments)
}

/// The default ID of edge `at` of a loop: `o<at>` on the outer loop, `h<k>-<at>` on hole `k`.
fn default_id(ring: Ring, at: usize) -> Result<Id, SlabError> {
    let text = match ring {
        Ring::Outer => format!("o{at}"),
        Ring::Hole(k) => format!("h{k}-{at}"),
    };
    Id::parse(&text).map_err(|_| malformed(format!("{text:?} is not an ID")))
}

/// One stored edge: `{"id":..,"line":n}` or `{"id":..,"arc":n,"reversed":b}`.
fn read_segment(value: &Value) -> Result<Segment, SlabError> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .and_then(|text| Id::parse(text).ok())
        .ok_or_else(|| malformed("an edge has an ID"))?;
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let edge = if let Some(line) = number("line") {
        Edge::Line(LineId(line))
    } else if let (Some(curve), Some(reversed)) = (
        number("arc"),
        value.get("reversed").and_then(Value::as_bool),
    ) {
        Edge::Arc {
            curve: CurveId(curve),
            reversed,
        }
    } else {
        return Err(malformed(format!("edge {id} is a line or an arc")));
    };
    Ok(Segment { id, edge })
}

/// A stored sketch, made again through the solver's own calls so that every point is in range,
/// every line joins two points it has and every arc's radius is the one it gives; one that is
/// not made again exactly is refused.
fn remade(value: &Value) -> Result<Sketch, SlabError> {
    let stored: Sketch = serde_json::from_value(value.clone())
        .map_err(|e| malformed(format!("the sketch is not a sketch: {e}")))?;
    if !stored.constraints().is_empty() {
        return Err(SlabError::Constrained);
    }
    let mut made = Sketch::new();
    for point in stored.points() {
        made.add_point(point.x, point.y).map_err(from_sketch)?;
    }
    for line in stored.lines() {
        made.add_line(line.start, line.end).map_err(from_sketch)?;
    }
    for curve in stored.curves() {
        let added = match *curve {
            construct_solver::Curve::Arc {
                centre, start, end, ..
            } => made.add_arc(centre, start, end),
            construct_solver::Curve::Circle { centre, radius } => made.add_circle(centre, radius),
        };
        added.map_err(from_sketch)?;
    }
    if made != stored {
        return Err(malformed("the sketch is not one the solver makes"));
    }
    Ok(made)
}
