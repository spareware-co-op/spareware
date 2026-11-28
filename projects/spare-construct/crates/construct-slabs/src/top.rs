//! A floor's top face shape-edited (job 14.15, CON-A4-F4): points with heights and split lines
//! between them, and the exact surface they define.
//!
//! **What a person edits** ([`Edits`]): points ([`TopPoint`]) placed on the floor, each with a
//! height from the floor's top face as its tie puts it (its level's elevation plus its offset), so
//! a floor that falls 20 mm to a waste has a point at the waste 20 mm down; a point on a corner of
//! the floor's boundary sets that corner's height, one on an edge divides the edge there, and one
//! inside the floor is a point of its own. Split lines ([`SplitLine`]) join two points and are a
//! crease the top face must fold along (a ridge, a valley, a line held up by a wall below). A
//! line's height is its two ends' heights. Every corner without a point stays at the top face's
//! own height.
//!
//! **The surface** ([`Top::of`]): the top face is divided into triangles whose corners are the
//! boundary's corners and the points, whose edges include every edge of the boundary and every
//! split line, and which are otherwise as near equal-sided as those allow (the constrained
//! Delaunay triangulation, [`crate::triangles`]). Each triangle is flat, through its three corners
//! at their heights: every corner is a grid point at a whole grid step of height, so each triangle
//! is an exact plane and nothing is rounded. [`Face`] is one triangle; [`Top::height_at`] the
//! exact height at any point of the floor, a ratio of whole numbers, and [`Face::gradient`] its
//! fall, for a spot slope or a room's volume to read.
//!
//! **Refused, in words** ([`EditError`]): two points with one ID or at one place; a point off the
//! floor (outside it or in a hole) or more than 10 m from its top; a split line naming no point,
//! of no length, given twice, leaving the floor, running along its edge, or crossing another (add
//! a point where they cross); more than [`MOST`] points or split lines; a floor that both slopes
//! (A72's arrow) and is shape-edited, or has slab edges and is shape-edited (one or the other: an
//! arrow is one plane, edits are many); and, with the floor's type ([`Edits::fit_layers`]), a
//! point lowered so far that the variable layer would be no thickness there.
//!
//! The layers ([`crate::Layers`]): every layer keeps its thickness and follows the top face, so the
//! structure stays one thickness by default, unless the type has a variable layer, which takes up
//! the difference over the layers below it, flat where they are on a plain floor. Their volumes
//! ([`Top::sixfold_rise`]) are exact: a following layer's is its plan area times its thickness
//! (plumb, as a sloped floor's); the variable layer's that plus the integral of the top face's
//! height over the floor, one sixth of a whole number of cubic steps, rounded once.
//!
//! Depends on [`crate::boundary`] (the loops), [`crate::triangles`] (the division into triangles,
//! interim: its header says why and what replaces it), construct-params' lengths and
//! construct-solver's points.

#![allow(
    clippy::many_single_char_names,
    reason = "a triangle's corners and a segment's ends named as geometry names them: a, b, c, d"
)]

use crate::boundary::Boundary;
use crate::error::SlabError;
use crate::layers::{Layers, MOST_TOTAL};
use crate::stored::{read_length, write_length};
use crate::triangles::triangulate;
use construct_params::Length;
use construct_solver::Point;
use kit_core::Id;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// The most points, and the most split lines, one floor's top may hold.
pub const MOST: usize = 500;

/// A point on a floor's top face: its ID, where it is in plan (grid steps) and its height from the
/// floor's top face (above zero raised, below lowered).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopPoint {
    /// Its permanent ID, unique among the floor's points and split lines.
    pub id: Id,
    /// Where it is, in grid steps.
    pub at: Point,
    /// Its height from the floor's top face.
    pub height: Length,
}

/// A split line: a crease of the top face from one point to another, each named by its ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitLine {
    /// Its permanent ID, unique among the floor's points and split lines.
    pub id: Id,
    /// The point it starts at.
    pub from: Id,
    /// The point it ends at.
    pub to: Id,
}

/// Why a floor's shape edits were refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// Two points or split lines share one ID.
    IdTwice {
        /// The ID.
        id: Id,
    },
    /// Two points stand at one place.
    PointsMeet {
        /// The earlier point.
        first: Id,
        /// The later point.
        second: Id,
    },
    /// A point is not on the floor: outside it, or in a hole.
    PointOff {
        /// The point.
        point: Id,
    },
    /// A point is more than 10 m above or below the floor's top face.
    PointTooFar {
        /// The point.
        point: Id,
    },
    /// A split line names a point the floor's top has not.
    SplitEnd {
        /// The split line.
        split: Id,
        /// The point it names.
        end: Id,
    },
    /// A split line starts and ends at one point.
    SplitNoLength {
        /// The split line.
        split: Id,
    },
    /// Two split lines join the same two points.
    SplitTwice {
        /// The earlier line.
        first: Id,
        /// The later line.
        second: Id,
    },
    /// A split line leaves the floor: it crosses an edge of the boundary or runs over a hole.
    SplitOff {
        /// The split line.
        split: Id,
    },
    /// A split line runs along an edge of the floor, where the top face already folds.
    SplitAlongEdge {
        /// The split line.
        split: Id,
    },
    /// Two split lines cross.
    SplitsCross {
        /// The earlier line.
        first: Id,
        /// The later line.
        second: Id,
    },
    /// More than [`MOST`] points or split lines.
    TooMany,
    /// The top face could not be divided into triangles within the work allowed.
    NotDivided,
    /// The floor slopes by an arrow and is shape-edited.
    WithSlope,
    /// The floor has slab edges and is shape-edited.
    WithEdges,
    /// A point is lowered so far that the variable layer would be no thickness there.
    VariableTooThin {
        /// The point.
        point: Id,
        /// The variable layer's index from the top.
        layer: usize,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdTwice { id } => write!(f, "two points or split lines are both {id}"),
            Self::PointsMeet { first, second } => {
                write!(f, "points {first} and {second} are at one place")
            }
            Self::PointOff { point } => write!(
                f,
                "point {point} is not on the floor: it is outside it or in a hole"
            ),
            Self::PointTooFar { point } => write!(
                f,
                "point {point} is more than 10 m from the floor's top face"
            ),
            Self::SplitEnd { split, end } => {
                write!(
                    f,
                    "split line {split} ends at {end}, which is no point of the floor"
                )
            }
            Self::SplitNoLength { split } => {
                write!(f, "split line {split} starts and ends at one point")
            }
            Self::SplitTwice { first, second } => {
                write!(
                    f,
                    "split lines {first} and {second} join the same two points"
                )
            }
            Self::SplitOff { split } => write!(
                f,
                "split line {split} leaves the floor: it crosses its edge or runs over a hole"
            ),
            Self::SplitAlongEdge { split } => write!(
                f,
                "split line {split} runs along an edge of the floor, where the top already folds"
            ),
            Self::SplitsCross { first, second } => write!(
                f,
                "split lines {first} and {second} cross: add a point where they cross and draw \
                 each to it"
            ),
            Self::TooMany => write!(
                f,
                "a floor's top holds at most {MOST} points and {MOST} split lines"
            ),
            Self::NotDivided => write!(
                f,
                "the floor's top could not be divided into triangles: take a point or a split line \
                 away"
            ),
            Self::WithSlope => write!(
                f,
                "a floor either slopes by an arrow or has its shape edited, not both: clear the \
                 slope, or reset the shape"
            ),
            Self::WithEdges => write!(
                f,
                "a floor with slab edges cannot have its shape edited yet: take the slab edges off, \
                 or reset the shape"
            ),
            Self::VariableTooThin { point, layer } => write!(
                f,
                "point {point} is lowered so far that layer {layer}, the variable one, would be no \
                 thickness there: raise it, or give the type a thicker variable layer"
            ),
        }
    }
}

/// A floor's shape edits: its points and split lines, in the order they were made. Every value of
/// this type has passed the checks that need no boundary; [`Top::of`] makes the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edits {
    points: Vec<TopPoint>,
    splits: Vec<SplitLine>,
}

fn edit(error: EditError) -> SlabError {
    SlabError::Edit(error)
}

impl Edits {
    /// Shape edits of these points and split lines.
    ///
    /// # Errors
    /// [`EditError::TooMany`], [`EditError::IdTwice`], [`EditError::PointsMeet`],
    /// [`EditError::PointTooFar`], [`EditError::SplitEnd`], [`EditError::SplitNoLength`] and
    /// [`EditError::SplitTwice`], each as [`SlabError::Edit`].
    pub fn new(points: Vec<TopPoint>, splits: Vec<SplitLine>) -> Result<Self, SlabError> {
        if points.len() > MOST || splits.len() > MOST {
            return Err(edit(EditError::TooMany));
        }
        let mut ids = BTreeSet::new();
        let mut places: BTreeMap<(i64, i64), &Id> = BTreeMap::new();
        for point in &points {
            if !ids.insert(point.id.clone()) {
                return Err(edit(EditError::IdTwice {
                    id: point.id.clone(),
                }));
            }
            if let Some(first) = places.insert((point.at.x, point.at.y), &point.id) {
                return Err(edit(EditError::PointsMeet {
                    first: first.clone(),
                    second: point.id.clone(),
                }));
            }
            if point.height.steps().unsigned_abs() > MOST_TOTAL.steps().unsigned_abs() {
                return Err(edit(EditError::PointTooFar {
                    point: point.id.clone(),
                }));
            }
        }
        let named: BTreeSet<&Id> = points.iter().map(|p| &p.id).collect();
        let mut joined: BTreeMap<(Id, Id), &Id> = BTreeMap::new();
        for split in &splits {
            if !ids.insert(split.id.clone()) {
                return Err(edit(EditError::IdTwice {
                    id: split.id.clone(),
                }));
            }
            for end in [&split.from, &split.to] {
                if !named.contains(end) {
                    return Err(edit(EditError::SplitEnd {
                        split: split.id.clone(),
                        end: end.clone(),
                    }));
                }
            }
            if split.from == split.to {
                return Err(edit(EditError::SplitNoLength {
                    split: split.id.clone(),
                }));
            }
            let pair = if split.from <= split.to {
                (split.from.clone(), split.to.clone())
            } else {
                (split.to.clone(), split.from.clone())
            };
            if let Some(first) = joined.insert(pair, &split.id) {
                return Err(edit(EditError::SplitTwice {
                    first: first.clone(),
                    second: split.id.clone(),
                }));
            }
        }
        Ok(Self { points, splits })
    }

    /// The points, in the order they were made.
    #[must_use]
    pub fn points(&self) -> &[TopPoint] {
        &self.points
    }

    /// The split lines, in the order they were made.
    #[must_use]
    pub fn splits(&self) -> &[SplitLine] {
        &self.splits
    }

    /// The point of this ID, if there is one.
    #[must_use]
    pub fn point(&self, id: &Id) -> Option<&TopPoint> {
        self.points.iter().find(|p| p.id == *id)
    }

    /// Whether there is nothing to edit: no point and no split line.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.splits.is_empty()
    }

    /// Whether a floor of `layers` can be built with these edits: the variable layer, if the type
    /// has one, keeps a thickness above zero under every point.
    ///
    /// # Errors
    /// [`EditError::VariableTooThin`] as [`SlabError::Edit`], naming the first such point.
    pub fn fit_layers(&self, layers: &Layers) -> Result<(), SlabError> {
        let Some(v) = layers.variable() else {
            return Ok(());
        };
        let thickness = layers.layer(v).map_or(0, |layer| layer.thickness.steps());
        for point in &self.points {
            if thickness.saturating_add(point.height.steps()) <= 0 {
                return Err(edit(EditError::VariableTooThin {
                    point: point.id.clone(),
                    layer: v,
                }));
            }
        }
        Ok(())
    }

    /// Its stored form: `{"points":[{"at":[x,y],"height":<length>,"id":<ID>}, ...],
    /// "splits":[{"from":<ID>,"id":<ID>,"to":<ID>}, ...]}` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let points: Vec<Value> = self
            .points
            .iter()
            .map(|p| {
                json!({
                    "id": p.id.as_str(),
                    "at": [p.at.x, p.at.y],
                    "height": write_length(p.height),
                })
            })
            .collect();
        let splits: Vec<Value> = self
            .splits
            .iter()
            .map(|s| json!({ "id": s.id.as_str(), "from": s.from.as_str(), "to": s.to.as_str() }))
            .collect();
        let mut out = Map::new();
        out.insert("points".to_owned(), Value::Array(points));
        out.insert("splits".to_owned(), Value::Array(splits));
        Value::Object(out)
    }

    /// Reads [`Edits::to_value`]'s form; anything else is refused, never repaired.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for a value not in its one spelling; any refusal of
    /// [`Edits::new`].
    pub fn from_value(value: &Value) -> Result<Self, SlabError> {
        let bad =
            || SlabError::Malformed("a floor's top is its points and its split lines".to_owned());
        let object = value.as_object().ok_or_else(bad)?;
        let list = |key: &str| object.get(key).and_then(Value::as_array).ok_or_else(bad);
        let id = |entry: &Value, key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .and_then(|text| Id::parse(text).ok())
                .ok_or_else(bad)
        };
        let mut points = Vec::new();
        for entry in list("points")? {
            let at = entry.get("at").and_then(Value::as_array).ok_or_else(bad)?;
            let (Some(x), Some(y), 2) = (
                at.first().and_then(Value::as_i64),
                at.get(1).and_then(Value::as_i64),
                at.len(),
            ) else {
                return Err(bad());
            };
            let height = entry
                .get("height")
                .and_then(Value::as_str)
                .and_then(read_length)
                .ok_or_else(bad)?;
            points.push(TopPoint {
                id: id(entry, "id")?,
                at: Point { x, y },
                height,
            });
        }
        let mut splits = Vec::new();
        for entry in list("splits")? {
            splits.push(SplitLine {
                id: id(entry, "id")?,
                from: id(entry, "from")?,
                to: id(entry, "to")?,
            });
        }
        let read = Self::new(points, splits)?;
        if read.to_value() != *value {
            return Err(SlabError::Malformed(
                "the floor's top is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}

/// One triangle of a shape-edited top face: its three corners `[x, y, z]` in grid steps,
/// anticlockwise seen from above, `z` the height from the floor's top face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    /// Its corners.
    pub corners: [[i64; 3]; 3],
}

impl Face {
    /// Twice its area in plan, in square steps: above zero.
    #[must_use]
    pub fn twice_area(&self) -> i128 {
        let [a, b, c] = self.corners.map(|p| [i128::from(p[0]), i128::from(p[1])]);
        let (ux, uy) = (b[0].saturating_sub(a[0]), b[1].saturating_sub(a[1]));
        let (vx, vy) = (c[0].saturating_sub(a[0]), c[1].saturating_sub(a[1]));
        ux.saturating_mul(vy).saturating_sub(uy.saturating_mul(vx))
    }

    /// Its fall in plan: the height's change per step along x and along y, as `([gx, gy], den)`,
    /// each `g / den`, `den` above zero (twice its area). A spot slope reads its steepest fall from
    /// it.
    #[must_use]
    pub fn gradient(&self) -> ([i128; 2], i128) {
        let [a, b, c] = self.corners.map(|p| p.map(i128::from));
        let u = [
            b[0].saturating_sub(a[0]),
            b[1].saturating_sub(a[1]),
            b[2].saturating_sub(a[2]),
        ];
        let v = [
            c[0].saturating_sub(a[0]),
            c[1].saturating_sub(a[1]),
            c[2].saturating_sub(a[2]),
        ];
        // The normal is u x v; the plane's gradient is (-nx / nz, -ny / nz), nz twice the area.
        let nx = u[1]
            .saturating_mul(v[2])
            .saturating_sub(u[2].saturating_mul(v[1]));
        let ny = u[2]
            .saturating_mul(v[0])
            .saturating_sub(u[0].saturating_mul(v[2]));
        (
            [nx.saturating_neg(), ny.saturating_neg()],
            self.twice_area(),
        )
    }

    /// Whether plan point `p` is on it (inside or on its edges), and if so the parts of twice its
    /// area that `p` makes with each edge, opposite each corner.
    fn weights(&self, p: Point) -> Option<[i128; 3]> {
        let [a, b, c] = self.corners.map(|q| Point { x: q[0], y: q[1] });
        let part = |u: Point, v: Point| {
            let (ux, uy) = (
                i128::from(v.x).saturating_sub(i128::from(u.x)),
                i128::from(v.y).saturating_sub(i128::from(u.y)),
            );
            let (px, py) = (
                i128::from(p.x).saturating_sub(i128::from(u.x)),
                i128::from(p.y).saturating_sub(i128::from(u.y)),
            );
            ux.saturating_mul(py).saturating_sub(uy.saturating_mul(px))
        };
        let w = [part(b, c), part(c, a), part(a, b)];
        w.iter().all(|w| *w >= 0).then_some(w)
    }
}

/// A shape-edited floor's top face: its triangles, each sharing an edge with one before it, so a
/// builder that joins them in order always joins a piece to what it already has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Top {
    faces: Vec<Face>,
}

/// Twice the area of the triangle `a`, `b`, `c`, signed: above zero anticlockwise.
fn orient(a: Point, b: Point, c: Point) -> i128 {
    let ux = i128::from(b.x).saturating_sub(i128::from(a.x));
    let uy = i128::from(b.y).saturating_sub(i128::from(a.y));
    let vx = i128::from(c.x).saturating_sub(i128::from(a.x));
    let vy = i128::from(c.y).saturating_sub(i128::from(a.y));
    ux.saturating_mul(vy).saturating_sub(uy.saturating_mul(vx))
}

/// Whether `p` lies on the segment `a`-`b`, its ends left out.
fn strictly_on(p: Point, a: Point, b: Point) -> bool {
    if orient(a, b, p) != 0 || p == a || p == b {
        return false;
    }
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// Whether the segments `a`-`b` and `c`-`d` have any point in common.
fn touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (orient(c, d, a).signum(), orient(c, d, b).signum());
    let (d3, d4) = (orient(a, b, c).signum(), orient(a, b, d).signum());
    if d1.saturating_mul(d2) < 0 && d3.saturating_mul(d4) < 0 {
        return true;
    }
    let within = |p: Point, u: Point, v: Point| {
        p.x >= u.x.min(v.x) && p.x <= u.x.max(v.x) && p.y >= u.y.min(v.y) && p.y <= u.y.max(v.y)
    };
    (d1 == 0 && within(a, c, d))
        || (d2 == 0 && within(b, c, d))
        || (d3 == 0 && within(c, a, b))
        || (d4 == 0 && within(d, a, b))
}

/// Whether `p` is inside the loop through `corners` (winding number not 0); asked only of a point
/// on no edge of it.
fn inside_loop(p: Point, corners: &[Point]) -> bool {
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

/// `p` with both coordinates times `k`.
fn scaled(p: Point, k: i64) -> Point {
    Point {
        x: p.x.saturating_mul(k),
        y: p.y.saturating_mul(k),
    }
}

/// Whether `p`, given at `k` times its coordinates, is strictly inside the floor of `loops` (in
/// the outer loop, in no hole), asked only of a point on no edge.
fn on_floor(p: Point, loops: &[Vec<Point>], k: i64) -> bool {
    loops.iter().enumerate().all(|(at, ring)| {
        let ring: Vec<Point> = ring.iter().map(|q| scaled(*q, k)).collect();
        inside_loop(p, &ring) == (at == 0)
    })
}

/// Where a point of the edits lands among the floor's vertices.
enum Landing {
    /// At the corner of this vertex.
    Corner(usize),
    /// On edge `k` of loop `r`, inside it.
    Edge(usize, usize),
    /// Inside the floor.
    Inside,
}

/// `p`'s place along the line from `a` towards `b`: the dot product of `p - a` with `b - a`, which
/// orders the points on a segment.
fn along(a: Point, b: Point, p: Point) -> i128 {
    let d = |u: i64, v: i64| i128::from(v).saturating_sub(i128::from(u));
    d(a.x, p.x)
        .saturating_mul(d(a.x, b.x))
        .saturating_add(d(a.y, p.y).saturating_mul(d(a.y, b.y)))
}

/// The floor's vertices as [`Top::of`] gathers them: every corner of every loop at height 0, then
/// each point of the edits not on a corner, with where each point landed.
struct Vertices<'a> {
    at: Vec<Point>,
    heights: Vec<i64>,
    /// The first vertex of each loop.
    starts: Vec<usize>,
    /// The points on each edge of each loop, by loop and edge.
    on_edges: BTreeMap<(usize, usize), Vec<usize>>,
    /// Each point's vertex.
    of_point: BTreeMap<&'a Id, usize>,
}

impl<'a> Vertices<'a> {
    fn of(loops: &[Vec<Point>], edits: &'a Edits) -> Result<Self, SlabError> {
        let mut at: Vec<Point> = loops.iter().flatten().copied().collect();
        let mut heights: Vec<i64> = vec![0; at.len()];
        let at_corner: BTreeMap<(i64, i64), usize> = at
            .iter()
            .enumerate()
            .map(|(i, p)| ((p.x, p.y), i))
            .collect();
        let mut starts = Vec::with_capacity(loops.len());
        let mut sum = 0_usize;
        for ring in loops {
            starts.push(sum);
            sum = sum.saturating_add(ring.len());
        }
        let mut on_edges: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        let mut of_point = BTreeMap::new();
        for point in edits.points() {
            let index = match Top::land(point, loops, &at_corner)? {
                Landing::Corner(i) => i,
                landing => {
                    at.push(point.at);
                    heights.push(0);
                    let i = at.len().saturating_sub(1);
                    if let Landing::Edge(r, k) = landing {
                        on_edges.entry((r, k)).or_default().push(i);
                    }
                    i
                }
            };
            if let Some(h) = heights.get_mut(index) {
                *h = point.height.steps();
            }
            of_point.insert(&point.id, index);
        }
        Ok(Self {
            at,
            heights,
            starts,
            on_edges,
            of_point,
        })
    }

    fn point(&self, i: usize) -> Result<Point, SlabError> {
        self.at.get(i).copied().ok_or(SlabError::OutOfRange)
    }

    /// The edges of the boundary, each cut at the points on it, in order along it.
    fn boundary_pieces(&self, loops: &[Vec<Point>]) -> Result<Vec<(usize, usize)>, SlabError> {
        let mut pieces = Vec::new();
        for (r, ring) in loops.iter().enumerate() {
            let start = self.starts.get(r).copied().unwrap_or(0);
            let n = ring.len();
            for k in 0..n {
                let a = start.saturating_add(k);
                let b = start.saturating_add(k.saturating_add(1).checked_rem(n).unwrap_or(0));
                let (pa, pb) = (self.point(a)?, self.point(b)?);
                let mut chain = vec![a];
                if let Some(between) = self.on_edges.get(&(r, k)) {
                    let mut between = between.clone();
                    between.sort_by_key(|i| self.at.get(*i).map_or(0, |p| along(pa, pb, *p)));
                    chain.extend(between);
                }
                chain.push(b);
                pieces.extend(chain.windows(2).filter_map(|pair| match pair {
                    [u, v] => Some((*u, *v)),
                    _ => None,
                }));
            }
        }
        Ok(pieces)
    }

    /// One split line's pieces, cut at every vertex on it, each checked against the boundary.
    fn split_pieces(
        &self,
        split: &SplitLine,
        loops: &[Vec<Point>],
        boundary: &[(usize, usize)],
    ) -> Result<Vec<(usize, usize)>, SlabError> {
        let (Some(&a), Some(&b)) = (self.of_point.get(&split.from), self.of_point.get(&split.to))
        else {
            return Err(edit(EditError::SplitEnd {
                split: split.id.clone(),
                end: split.from.clone(),
            }));
        };
        let (pa, pb) = (self.point(a)?, self.point(b)?);
        let mut chain: Vec<usize> = self
            .at
            .iter()
            .enumerate()
            .filter(|(_, p)| strictly_on(**p, pa, pb))
            .map(|(i, _)| i)
            .collect();
        chain.sort_by_key(|i| self.at.get(*i).map_or(0, |p| along(pa, pb, *p)));
        chain.insert(0, a);
        chain.push(b);
        let along_edge = || {
            edit(EditError::SplitAlongEdge {
                split: split.id.clone(),
            })
        };
        let off = || {
            edit(EditError::SplitOff {
                split: split.id.clone(),
            })
        };
        let mut pieces = Vec::new();
        for pair in chain.windows(2) {
            let [u, v] = pair else { continue };
            let (pu, pv) = (self.point(*u)?, self.point(*v)?);
            if boundary.iter().any(|&(x, y)| key(x, y) == key(*u, *v)) {
                return Err(along_edge());
            }
            let middle = Point {
                x: pu.x.saturating_add(pv.x),
                y: pu.y.saturating_add(pv.y),
            };
            for &(x, y) in boundary {
                let (px, py) = (self.point(x)?, self.point(y)?);
                let shares = x == *u || x == *v || y == *u || y == *v;
                if !shares && touch(pu, pv, px, py) {
                    return Err(off());
                }
                let (dx, dy) = (scaled(px, 2), scaled(py, 2));
                if orient(dx, dy, middle) == 0 && touch(middle, middle, dx, dy) {
                    return Err(along_edge());
                }
            }
            if !on_floor(middle, loops, 2) {
                return Err(off());
            }
            pieces.push((*u, *v));
        }
        Ok(pieces)
    }
}

impl Top {
    /// The top face of a floor of `boundary` with `edits`, as the module header says.
    ///
    /// # Errors
    /// [`EditError::PointOff`], [`EditError::SplitOff`], [`EditError::SplitAlongEdge`],
    /// [`EditError::SplitsCross`] and [`EditError::NotDivided`], each as [`SlabError::Edit`].
    pub fn of(boundary: &Boundary, edits: &Edits) -> Result<Self, SlabError> {
        let loops: Vec<Vec<Point>> = boundary
            .profile()
            .into_iter()
            .map(|ring| ring.into_iter().map(|(_, p)| p).collect())
            .collect();
        let vertices = Vertices::of(&loops, edits)?;
        let boundary_pieces = vertices.boundary_pieces(&loops)?;
        let mut segments: BTreeSet<(usize, usize)> =
            boundary_pieces.iter().map(|&(u, v)| key(u, v)).collect();
        let mut split_pieces: Vec<(&Id, usize, usize)> = Vec::new();
        for split in edits.splits() {
            for (u, v) in vertices.split_pieces(split, &loops, &boundary_pieces)? {
                split_pieces.push((&split.id, u, v));
            }
        }
        for (i, &(first, a, b)) in split_pieces.iter().enumerate() {
            for &(second, c, d) in split_pieces.iter().skip(i.saturating_add(1)) {
                let shares = a == c || a == d || b == c || b == d;
                if first == second || shares {
                    continue;
                }
                let (pa, pb) = (vertices.point(a)?, vertices.point(b)?);
                let (pc, pd) = (vertices.point(c)?, vertices.point(d)?);
                if touch(pa, pb, pc, pd) {
                    return Err(edit(EditError::SplitsCross {
                        first: first.clone(),
                        second: second.clone(),
                    }));
                }
            }
            segments.insert(key(a, b));
        }
        let segments: Vec<(usize, usize)> = segments.into_iter().collect();
        let triangles = triangulate(&vertices.at, &segments, |c| on_floor(c, &loops, 3))
            .ok_or(edit(EditError::NotDivided))?;
        let mut faces = Vec::with_capacity(triangles.len());
        for t in &triangles {
            let mut corners = [[0_i64; 3]; 3];
            for (slot, &i) in corners.iter_mut().zip(t) {
                let (p, z) = (vertices.point(i)?, vertices.heights.get(i).copied());
                *slot = [p.x, p.y, z.ok_or(SlabError::OutOfRange)?];
            }
            faces.push(Face { corners });
        }
        Ok(Self {
            faces: joined_order(&triangles, &faces),
        })
    }

    /// Where `point` lands: on a corner, on an edge, or inside the floor; refused off it.
    fn land(
        point: &TopPoint,
        loops: &[Vec<Point>],
        at_corner: &BTreeMap<(i64, i64), usize>,
    ) -> Result<Landing, SlabError> {
        if let Some(&i) = at_corner.get(&(point.at.x, point.at.y)) {
            return Ok(Landing::Corner(i));
        }
        for (r, ring) in loops.iter().enumerate() {
            for (k, a) in ring.iter().enumerate() {
                let b = ring
                    .get(k.saturating_add(1))
                    .or_else(|| ring.first())
                    .copied()
                    .unwrap_or(*a);
                if strictly_on(point.at, *a, b) {
                    return Ok(Landing::Edge(r, k));
                }
            }
        }
        if on_floor(point.at, loops, 1) {
            Ok(Landing::Inside)
        } else {
            Err(edit(EditError::PointOff {
                point: point.id.clone(),
            }))
        }
    }

    /// Its triangles, each after one it shares an edge with.
    #[must_use]
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// The top face's lowest and highest heights from the floor's top face, in grid steps.
    #[must_use]
    pub fn range(&self) -> (i64, i64) {
        let all = self
            .faces
            .iter()
            .flat_map(|f| f.corners.iter().map(|c| c[2]));
        let (low, high) = all.fold((i64::MAX, i64::MIN), |(l, h), z| (l.min(z), h.max(z)));
        if self.faces.is_empty() {
            (0, 0)
        } else {
            (low, high)
        }
    }

    /// The top face's height at plan point `p` from the floor's top face, exactly, as
    /// `(num, den)`, `den` above zero; `None` off the floor.
    #[must_use]
    pub fn height_at(&self, p: Point) -> Option<(i128, i128)> {
        for face in &self.faces {
            if let Some(w) = face.weights(p) {
                let mut num = 0_i128;
                for (wk, corner) in w.iter().zip(face.corners) {
                    num = num.checked_add(wk.checked_mul(i128::from(corner[2]))?)?;
                }
                return Some((num, face.twice_area()));
            }
        }
        None
    }

    /// Six times the integral of the top face's height over the floor, in cubic steps: the sum over
    /// its triangles of twice the area times the three corners' heights summed. A variable layer's
    /// volume is its plan area times its thickness plus a sixth of this.
    ///
    /// # Errors
    /// [`SlabError::OutOfRange`] when the sum does not fit 128 bits.
    pub fn sixfold_rise(&self) -> Result<i128, SlabError> {
        let mut sum = 0_i128;
        for face in &self.faces {
            let heights = face
                .corners
                .iter()
                .try_fold(0_i128, |s, c| s.checked_add(i128::from(c[2])))
                .ok_or(SlabError::OutOfRange)?;
            sum = face
                .twice_area()
                .checked_mul(heights)
                .and_then(|v| sum.checked_add(v))
                .ok_or(SlabError::OutOfRange)?;
        }
        Ok(sum)
    }
}

/// An undirected edge, its lower index first.
fn key(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

/// `faces` (one per triangle of `triangles`) reordered so each after the first shares an edge
/// with one before it: from the first, each next the least-ordered triangle touching those placed.
fn joined_order(triangles: &[[usize; 3]], faces: &[Face]) -> Vec<Face> {
    let mut by_edge: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (t, [a, b, c]) in triangles.iter().enumerate() {
        for edge in [key(*a, *b), key(*b, *c), key(*c, *a)] {
            by_edge.entry(edge).or_default().push(t);
        }
    }
    let mut placed = vec![false; triangles.len()];
    let mut order = Vec::with_capacity(triangles.len());
    for seed in 0..triangles.len() {
        if placed.get(seed).copied().unwrap_or(true) {
            continue;
        }
        let mut queue = VecDeque::from([seed]);
        if let Some(slot) = placed.get_mut(seed) {
            *slot = true;
        }
        while let Some(t) = queue.pop_front() {
            order.push(t);
            let Some([a, b, c]) = triangles.get(t) else {
                continue;
            };
            let mut next: Vec<usize> = [key(*a, *b), key(*b, *c), key(*c, *a)]
                .iter()
                .filter_map(|edge| by_edge.get(edge))
                .flatten()
                .copied()
                .filter(|n| !placed.get(*n).copied().unwrap_or(true))
                .collect();
            next.sort_unstable();
            next.dedup();
            for n in next {
                if let Some(slot) = placed.get_mut(n) {
                    *slot = true;
                }
                queue.push_back(n);
            }
        }
    }
    order
        .into_iter()
        .filter_map(|t| faces.get(t).copied())
        .collect()
}
