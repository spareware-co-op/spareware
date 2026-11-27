//! A floor's shape beyond its boundary (job 5.2, CON-A4-F4-P1 rest): one slope arrow, and slab
//! edges run along chosen edges of its boundary. [`Shape`] is a boundary with both, checked
//! together; what it promises is worked out here exactly, in grid steps, and what a builder takes
//! of it is [`Shape::tops`] and [`Shape::pieces`].
//!
//! **A slope** ([`SlopeArrow`]) is an arrow in plan from a tail to a head, with the top face's
//! height at the tail, and either its height at the head ([`Rise::Heights`]) or a rise over a run
//! along the arrow ([`Rise::Gradient`], construct-params' [`Slope`]). Heights are measured from
//! the floor's tied height (its level's elevation plus its offset), so a floor that falls 40 mm to
//! a drain has the tail at 0 and the head at -40 mm. The top face is the plane through the tail's
//! height rising along the arrow and level across it: at a plan point `p`,
//! `z(p) = at_tail + rise * ((p - tail) . (head - tail)) / den`, with `rise / den` the arrow's
//! height change over its length squared (heights) or the slope's rise over its run times the
//! arrow's length (gradient). Exact, as a ratio of whole numbers:
//!
//! - every corner of every loop must stand at a whole grid step on that plane, or the slope is
//!   refused naming the corner's edge ([`SlabError::SlopeOffGrid`]): the layers are built as the
//!   boundary lifted onto the plane and extruded straight down, so their corners are grid points
//!   and the faces exactly flat. Corners on whole millimetres with heights on whole millimetres
//!   along x or y always land (the grid has 6,400 steps to the millimetre);
//! - a gradient needs an arrow of a whole number of steps long ([`SlabError::SlopeArrowNotWhole`])
//!   (along x or y, or of a whole length such as 3, 4, 5), since along any other the length is a
//!   square root;
//! - layer thicknesses are plumb (measured straight down, as the type gives them), so every side
//!   face is upright, each layer's volume is the plan area times its thickness, exactly, and the
//!   floor's top and bottom faces are parallel;
//! - the sloped top face's true area is the plan area times `sqrt(1 + gradient^2)`: exact only
//!   where that root is whole in ratio (an arrow of 4 rising 3, for one), so it is held as the
//!   nearest whole square step, with [`TopArea::exact`] saying which (worked in num-bigint's
//!   integers, never a float);
//! - the highest and lowest points are corners of the outer loop (a plane's highest point over a
//!   polygon is a corner, and every hole lies inside the outer loop): the highest the top face's,
//!   the lowest the bottom face's, the type's thickness below the lowest top corner.
//!
//! **Slab edges** ([`SlabEdge`]): a profile run below the floor's bottom face along one of its
//! boundary's edges (outer loop or hole), flush with the floor's side face there:
//! [`EdgeProfile::Thickened`] (the slab made deeper at its edge, its inner face tapered from its
//! width at the slab's underside to its foot) or [`EdgeProfile::Downturn`] (an upright downstand of
//! one width). Each is a piece of its own: its plan at the underside, its width in from the edge,
//! and the same at its foot, with an exact volume (the prismatoid rule over its two flat ends,
//! twelve times which is a whole number) and length (the edge's). Where two edged sides with the
//! same profile meet, both pieces are mitred on the corner's bisector so they join; where an edged
//! side meets a plain one, the piece ends square; different profiles meeting are refused
//! ([`SlabError::EdgesMeet`]), and so is a square end at a corner sharper than a right angle
//! ([`SlabError::EdgeEndOutside`]). Every corner of a piece must be a grid point
//! ([`SlabError::EdgeOffGrid`]: a width across an edge that is not along x or y lands only where
//! the edge's length divides it); a piece whose inner face reaches another side, or whose ends
//! cross, is refused ([`SlabError::EdgeTooWide`]), and two pieces whose inner faces meet
//! ([`SlabError::EdgesOverlap`]). The builder unites the pieces with the floor's lowest layer of
//! any thickness, whose material they take.
//!
//! **Refused together** ([`SlabError::SlopeWithEdges`]): a slope and slab edges on one floor (the
//! pieces' tops would follow the sloped underside: not built yet).
//!
//! Depends on [`crate::boundary`], construct-params' lengths, areas, volumes and slope, and
//! num-bigint (MIT OR Apache-2.0) for the sloped area's square root. Nothing here is interim.

use crate::boundary::{Boundary, Ring, twice_signed_area};
use crate::error::SlabError;
use crate::layers::Layers;
use crate::measure::area;
use crate::stored::{read_length, write_length};
use construct_params::{Area, Length, Slope, Value as Param, Volume};
use construct_solver::{COORD_LIMIT, Point};
use kit_core::Id;
use num_bigint::BigUint;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// How a slope arrow rises from its tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rise {
    /// The top face's height at the arrow's head.
    Heights {
        /// The height at the head.
        at_head: Length,
    },
    /// A rise over a run along the arrow (a fall where the rise is below zero).
    Gradient {
        /// The slope.
        slope: Slope,
    },
}

/// A floor's slope arrow, as the module header says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlopeArrow {
    tail: Point,
    head: Point,
    at_tail: Length,
    rise: Rise,
    /// The height change along the arrow, over [`SlopeArrow::den`].
    num: i128,
    /// Above zero.
    den: i128,
}

/// `b - a` along both axes, exact in 128 bits for points within the grid.
fn less(a: Point, b: Point) -> [i128; 2] {
    [
        i128::from(b.x).saturating_sub(i128::from(a.x)),
        i128::from(b.y).saturating_sub(i128::from(a.y)),
    ]
}

fn dot(u: [i128; 2], v: [i128; 2]) -> Option<i128> {
    u[0].checked_mul(v[0])?.checked_add(u[1].checked_mul(v[1])?)
}

fn cross(u: [i128; 2], v: [i128; 2]) -> Option<i128> {
    u[0].checked_mul(v[1])?.checked_sub(u[1].checked_mul(v[0])?)
}

/// The whole square root of `n`, if it has one.
fn whole_root(n: i128) -> Option<i128> {
    let n = u128::try_from(n).ok()?;
    let root = n.isqrt();
    (root.checked_mul(root)? == n).then(|| i128::try_from(root).ok())?
}

fn in_grid(p: Point) -> bool {
    (-COORD_LIMIT..=COORD_LIMIT).contains(&p.x) && (-COORD_LIMIT..=COORD_LIMIT).contains(&p.y)
}

impl SlopeArrow {
    /// A slope arrow from `tail` to `head`, the top face `at_tail` high at its tail, rising as
    /// `rise` says.
    ///
    /// # Errors
    /// [`SlabError::OutOfRange`] for a point beyond the grid; [`SlabError::SlopeNoDirection`]
    /// when the tail is the head; [`SlabError::SlopeArrowNotWhole`] for a gradient along an arrow
    /// that is not a whole number of steps long.
    pub fn new(tail: Point, head: Point, at_tail: Length, rise: Rise) -> Result<Self, SlabError> {
        if !in_grid(tail) || !in_grid(head) {
            return Err(SlabError::OutOfRange);
        }
        if tail == head {
            return Err(SlabError::SlopeNoDirection);
        }
        let d = less(tail, head);
        let dd = dot(d, d).ok_or(SlabError::OutOfRange)?;
        let (num, den) = match rise {
            Rise::Heights { at_head } => (
                i128::from(at_head.steps()).saturating_sub(i128::from(at_tail.steps())),
                dd,
            ),
            Rise::Gradient { slope } => {
                let length = whole_root(dd).ok_or(SlabError::SlopeArrowNotWhole)?;
                let run = i128::from(slope.run())
                    .checked_mul(length)
                    .ok_or(SlabError::OutOfRange)?;
                (i128::from(slope.rise()), run)
            }
        };
        // Above zero: a squared length of a vector that is not zero, or a run above zero times a
        // length above zero.
        if den <= 0 {
            return Err(SlabError::OutOfRange);
        }
        Ok(Self {
            tail,
            head,
            at_tail,
            rise,
            num,
            den,
        })
    }

    /// The arrow's tail.
    #[must_use]
    pub fn tail(&self) -> Point {
        self.tail
    }

    /// The arrow's head.
    #[must_use]
    pub fn head(&self) -> Point {
        self.head
    }

    /// The top face's height at the tail.
    #[must_use]
    pub fn at_tail(&self) -> Length {
        self.at_tail
    }

    /// How it rises.
    #[must_use]
    pub fn rise(&self) -> Rise {
        self.rise
    }

    /// The top face's height at `p` in grid steps, when it is a whole number of them.
    #[must_use]
    pub fn height_at(&self, p: Point) -> Option<i64> {
        let along = dot(less(self.tail, p), less(self.tail, self.head))?;
        let scaled = self.num.checked_mul(along)?;
        if scaled.checked_rem(self.den)? != 0 {
            return None;
        }
        let z = i128::from(self.at_tail.steps()).checked_add(scaled.checked_div(self.den)?)?;
        i64::try_from(z).ok()
    }

    /// `1 + gradient^2` as a ratio `(p, q)` of whole numbers, `q` above zero: the sloped top
    /// face's area over its plan area, squared.
    fn stretch(&self) -> Option<(i128, i128)> {
        match self.rise {
            Rise::Heights { at_head } => {
                let dd = dot(less(self.tail, self.head), less(self.tail, self.head))?;
                let h =
                    i128::from(at_head.steps()).checked_sub(i128::from(self.at_tail.steps()))?;
                Some((dd.checked_add(h.checked_mul(h)?)?, dd))
            }
            Rise::Gradient { slope } => {
                let (rise, run) = (i128::from(slope.rise()), i128::from(slope.run()));
                let qq = run.checked_mul(run)?;
                Some((qq.checked_add(rise.checked_mul(rise)?)?, qq))
            }
        }
    }

    /// Its stored form: `{"at_tail":<length>,"head":[x,y],"tail":[x,y]}` with `"at_head":<length>`
    /// or `"slope":<slope>` (construct-params' stored spellings; FORMAT.md beside this crate).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("tail".to_owned(), json!([self.tail.x, self.tail.y]));
        out.insert("head".to_owned(), json!([self.head.x, self.head.y]));
        out.insert("at_tail".to_owned(), json!(write_length(self.at_tail)));
        match self.rise {
            Rise::Heights { at_head } => {
                out.insert("at_head".to_owned(), json!(write_length(at_head)));
            }
            Rise::Gradient { slope } => {
                out.insert("slope".to_owned(), json!(Param::Slope(slope).stored()));
            }
        }
        Value::Object(out)
    }

    /// Reads [`SlopeArrow::to_value`]'s form; anything else is refused, never repaired.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for a value not in its one spelling; any refusal of
    /// [`SlopeArrow::new`].
    pub fn from_value(value: &Value) -> Result<Self, SlabError> {
        let bad = || SlabError::Malformed("a slope arrow is its tail, head and heights".to_owned());
        let object = value.as_object().ok_or_else(bad)?;
        let point = |key: &str| -> Result<Point, SlabError> {
            let list = object.get(key).and_then(Value::as_array).ok_or_else(bad)?;
            match list.as_slice() {
                [x, y] => Ok(Point {
                    x: x.as_i64().ok_or_else(bad)?,
                    y: y.as_i64().ok_or_else(bad)?,
                }),
                _ => Err(bad()),
            }
        };
        let length = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_str)
                .and_then(read_length)
                .ok_or_else(bad)
        };
        let rise = if object.contains_key("at_head") {
            Rise::Heights {
                at_head: length("at_head")?,
            }
        } else {
            let slope = object
                .get("slope")
                .and_then(Value::as_str)
                .and_then(Param::from_stored);
            match slope {
                Some(Param::Slope(slope)) => Rise::Gradient { slope },
                _ => return Err(bad()),
            }
        };
        let read = Self::new(point("tail")?, point("head")?, length("at_tail")?, rise)?;
        if read.to_value() != *value {
            return Err(SlabError::Malformed(
                "the slope arrow is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}

/// A slab edge's profile, across its edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeProfile {
    /// The slab made deeper at its edge: `depth` below the floor's bottom face, `width` in from
    /// the edge at the underside, tapering to `foot` at the bottom (`foot` at most `width`).
    Thickened {
        /// Its width in from the edge at the floor's underside.
        width: Length,
        /// Its width at its bottom.
        foot: Length,
        /// How far below the floor's bottom face it reaches.
        depth: Length,
    },
    /// An upright downstand: `width` in from the edge, `depth` below the floor's bottom face.
    Downturn {
        /// Its width in from the edge.
        width: Length,
        /// How far below the floor's bottom face it reaches.
        depth: Length,
    },
}

impl EdgeProfile {
    /// Its width at the floor's underside.
    #[must_use]
    pub fn width(&self) -> Length {
        match *self {
            Self::Thickened { width, .. } | Self::Downturn { width, .. } => width,
        }
    }

    /// Its width at its bottom: its width for a downturn.
    #[must_use]
    pub fn foot(&self) -> Length {
        match *self {
            Self::Thickened { foot, .. } => foot,
            Self::Downturn { width, .. } => width,
        }
    }

    /// How far below the floor's bottom face it reaches.
    #[must_use]
    pub fn depth(&self) -> Length {
        match *self {
            Self::Thickened { depth, .. } | Self::Downturn { depth, .. } => depth,
        }
    }

    /// Its stored kind: `thickened` or `downturn`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Thickened { .. } => "thickened",
            Self::Downturn { .. } => "downturn",
        }
    }

    fn check(&self, edge: &Id) -> Result<(), SlabError> {
        let why = |why: &str| SlabError::EdgeProfile {
            edge: edge.clone(),
            why: why.to_owned(),
        };
        if self.width().steps() <= 0 {
            return Err(why("its width must be above zero"));
        }
        if self.depth().steps() <= 0 {
            return Err(why("its depth must be above zero"));
        }
        if self.foot().steps() <= 0 {
            return Err(why("its foot must be above zero"));
        }
        if self.foot().steps() > self.width().steps() {
            return Err(why("its foot must be no wider than its width"));
        }
        if self.depth().steps() > crate::layers::MOST_TOTAL.steps() {
            return Err(why("it reaches more than 10 m below the floor"));
        }
        Ok(())
    }
}

/// A slab edge: a profile run along one edge of the floor's boundary, named by the edge's ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlabEdge {
    /// The edge it runs along (an outer loop's or a hole's).
    pub edge: Id,
    /// Its profile.
    pub profile: EdgeProfile,
}

impl SlabEdge {
    /// Its stored form: `{"depth":<length>,"edge":"<edge ID>","foot":<length>,"kind":"thickened",
    /// "width":<length>}`, a downturn without `foot`.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("edge".to_owned(), json!(self.edge.as_str()));
        out.insert("kind".to_owned(), json!(self.profile.kind()));
        out.insert(
            "width".to_owned(),
            json!(write_length(self.profile.width())),
        );
        if let EdgeProfile::Thickened { foot, .. } = self.profile {
            out.insert("foot".to_owned(), json!(write_length(foot)));
        }
        out.insert(
            "depth".to_owned(),
            json!(write_length(self.profile.depth())),
        );
        Value::Object(out)
    }

    /// Reads [`SlabEdge::to_value`]'s form; anything else is refused, never repaired.
    ///
    /// # Errors
    /// [`SlabError::Malformed`] for a value not in its one spelling.
    pub fn from_value(value: &Value) -> Result<Self, SlabError> {
        let bad = || SlabError::Malformed("a slab edge is its edge, kind and sizes".to_owned());
        let object = value.as_object().ok_or_else(bad)?;
        let length = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_str)
                .and_then(read_length)
                .ok_or_else(bad)
        };
        let edge = object
            .get("edge")
            .and_then(Value::as_str)
            .and_then(|text| Id::parse(text).ok())
            .ok_or_else(bad)?;
        let profile = match object.get("kind").and_then(Value::as_str) {
            Some("thickened") => EdgeProfile::Thickened {
                width: length("width")?,
                foot: length("foot")?,
                depth: length("depth")?,
            },
            Some("downturn") => EdgeProfile::Downturn {
                width: length("width")?,
                depth: length("depth")?,
            },
            _ => return Err(bad()),
        };
        let read = Self { edge, profile };
        if read.to_value() != *value {
            return Err(SlabError::Malformed(
                "the slab edge is not in its one spelling".to_owned(),
            ));
        }
        Ok(read)
    }
}

/// A slab edge as the builder takes it: its plan at the floor's underside and at its foot, each
/// four corners in grid steps anticlockwise seen from above (along the edge, then back along its
/// inner face), with its exact length and volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgePiece {
    /// The edge it runs along.
    pub edge: Id,
    /// Its profile.
    pub profile: EdgeProfile,
    /// Its plan at the floor's underside.
    pub top: [Point; 4],
    /// Its plan at its foot: the same as `top` for a downturn.
    pub foot: [Point; 4],
    /// Its length: the edge's.
    pub length: Length,
    /// Its volume.
    pub volume: Volume,
}

/// A floor's shape: its boundary with its slope and slab edges, checked together. Every value of
/// this type has passed every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    boundary: Boundary,
    slope: Option<SlopeArrow>,
    edges: Vec<SlabEdge>,
    tops: Vec<Vec<i64>>,
    pieces: Vec<EdgePiece>,
}

/// The sloped top face's area: the nearest whole square step, and whether that is exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopArea {
    /// The area, to the nearest square step.
    pub area: Area,
    /// Whether it is the area exactly.
    pub exact: bool,
}

/// A corner of the profile as the edge that starts there and its point.
type Corner = (Id, Point);

/// The corner after `at` in a loop, round to the first.
fn next(ring: &[Corner], at: usize) -> Option<&Corner> {
    ring.get(at.saturating_add(1)).or_else(|| ring.first())
}

/// The corner before `at` in a loop, round to the last.
fn prev(ring: &[Corner], at: usize) -> Option<&Corner> {
    if at == 0 {
        ring.last()
    } else {
        ring.get(at.saturating_sub(1))
    }
}

/// `i` round a loop of `n` corners.
fn wrap(i: usize, n: usize) -> usize {
    i.checked_rem(n).unwrap_or(0)
}

/// `p + r`, as a grid point.
fn moved(p: Point, r: [i128; 2]) -> Option<Point> {
    let x = i64::try_from(i128::from(p.x).checked_add(r[0])?).ok()?;
    let y = i64::try_from(i128::from(p.y).checked_add(r[1])?).ok()?;
    let out = Point { x, y };
    in_grid(out).then_some(out)
}

/// `n / d` when it is whole.
fn exactly(n: i128, d: i128) -> Option<i128> {
    (d != 0 && n.checked_rem(d)? == 0).then(|| n.checked_div(d))?
}

/// The point `w` in from the corner `c`, square across the edge of direction `d` and length
/// `len`, material on its left: `c + w * (-dy, dx) / len`, when that is a grid point.
fn square(c: Point, d: [i128; 2], len: i128, w: i128) -> Option<Point> {
    let rx = exactly(w.checked_mul(d[1])?.checked_neg()?, len)?;
    let ry = exactly(w.checked_mul(d[0])?, len)?;
    moved(c, [rx, ry])
}

/// Where the lines `w` in from the edge into the corner `c` (direction `dp`, length `lp`) and the
/// edge out of it (`dq`, `lq`) meet: the mitre's inner corner, when it is a grid point. Two edges
/// running on in one line meet square across them.
fn mitre(
    c: Point,
    (dp, lp): ([i128; 2], i128),
    (dq, lq): ([i128; 2], i128),
    w: i128,
) -> Option<Point> {
    // The inward normals, unscaled: (-dy, dx). A point r from c is w in from each when
    // r . n = w * len for both.
    let (np, nq) = ([dp[1].checked_neg()?, dp[0]], [dq[1].checked_neg()?, dq[0]]);
    let det = cross(np, nq)?;
    if det == 0 {
        return square(c, dq, lq, w);
    }
    let (sp, sq) = (w.checked_mul(lp)?, w.checked_mul(lq)?);
    let rx = exactly(
        sp.checked_mul(nq[1])?.checked_sub(sq.checked_mul(np[1])?)?,
        det,
    )?;
    let ry = exactly(
        np[0].checked_mul(sq)?.checked_sub(nq[0].checked_mul(sp)?)?,
        det,
    )?;
    moved(c, [rx, ry])
}

/// The cross product of `b - a` and `c - a`.
fn orient(a: Point, b: Point, c: Point) -> i128 {
    cross(less(a, b), less(a, c)).unwrap_or(0)
}

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

/// Whether `p` is inside the loop through `corners` (winding number not 0); asked only of a point
/// on no edge of it.
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

/// A point with both coordinates doubled.
fn doubled(p: Point) -> Point {
    Point {
        x: p.x.saturating_mul(2),
        y: p.y.saturating_mul(2),
    }
}

impl Shape {
    /// A floor's shape: `boundary` with `slope` and `edges`, checked as the module header says.
    ///
    /// # Errors
    /// [`SlabError::SlopeWithEdges`]; [`SlabError::SlopeOffGrid`]; [`SlabError::EdgeUnknown`],
    /// [`SlabError::EdgeTwice`], [`SlabError::EdgeProfile`], [`SlabError::EdgeOffGrid`],
    /// [`SlabError::EdgesMeet`], [`SlabError::EdgeEndOutside`], [`SlabError::EdgeTooWide`],
    /// [`SlabError::EdgesOverlap`], [`SlabError::EdgeVolumeNotExact`]; [`SlabError::OutOfRange`].
    pub fn new(
        boundary: Boundary,
        slope: Option<SlopeArrow>,
        edges: Vec<SlabEdge>,
    ) -> Result<Self, SlabError> {
        if slope.is_some() && !edges.is_empty() {
            return Err(SlabError::SlopeWithEdges);
        }
        let profile = boundary.profile();
        let mut tops = Vec::new();
        if let Some(slope) = &slope {
            for (at, ring) in profile.iter().enumerate() {
                let which = if at == 0 {
                    Ring::Outer
                } else {
                    Ring::Hole(at.saturating_sub(1))
                };
                let heights = ring
                    .iter()
                    .map(|(id, p)| {
                        slope.height_at(*p).ok_or_else(|| SlabError::SlopeOffGrid {
                            ring: which,
                            segment: id.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                tops.push(heights);
            }
        }
        let pieces = pieces(&profile, &edges)?;
        Ok(Self {
            boundary,
            slope,
            edges,
            tops,
            pieces,
        })
    }

    /// A flat floor of `boundary`, with no slope and no slab edges.
    #[must_use]
    pub fn flat(boundary: Boundary) -> Self {
        Self {
            boundary,
            slope: None,
            edges: Vec::new(),
            tops: Vec::new(),
            pieces: Vec::new(),
        }
    }

    /// Its boundary.
    #[must_use]
    pub fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    /// Its slope arrow, if it slopes.
    #[must_use]
    pub fn slope(&self) -> Option<&SlopeArrow> {
        self.slope.as_ref()
    }

    /// Its slab edges, as given.
    #[must_use]
    pub fn edges(&self) -> &[SlabEdge] {
        &self.edges
    }

    /// Whether it is a flat floor with no slab edges, as every floor was before job 5.2.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.slope.is_none() && self.edges.is_empty()
    }

    /// For a sloped floor, the top face's height at each corner of each loop of
    /// [`Boundary::profile`], in its order, in grid steps; empty for a flat floor (every corner
    /// at 0).
    #[must_use]
    pub fn tops(&self) -> &[Vec<i64>] {
        &self.tops
    }

    /// Its slab edges as the builder takes them, in the order given.
    #[must_use]
    pub fn pieces(&self) -> &[EdgePiece] {
        &self.pieces
    }

    /// The top face's highest and lowest points, in grid steps: both 0 on a flat floor.
    #[must_use]
    pub fn top_range(&self) -> (i64, i64) {
        match self.tops.first() {
            Some(outer) => (
                outer.iter().copied().min().unwrap_or(0),
                outer.iter().copied().max().unwrap_or(0),
            ),
            None => (0, 0),
        }
    }

    /// The floor's lowest and highest points for a type of `layers`, from its tied height: the
    /// highest is the top face's highest corner; the lowest the bottom face's lowest, or the
    /// deepest slab edge's foot.
    ///
    /// # Errors
    /// [`SlabError::OutOfRange`] when a height does not fit a length.
    pub fn lowest_and_highest(&self, layers: &Layers) -> Result<(Length, Length), SlabError> {
        let (low, high) = self.top_range();
        let deepest = self
            .pieces
            .iter()
            .map(|p| p.profile.depth().steps())
            .max()
            .unwrap_or(0);
        let lowest = low
            .checked_sub(layers.total().steps())
            .and_then(|z| z.checked_sub(deepest))
            .ok_or(SlabError::OutOfRange)?;
        Ok((Length::from_steps(lowest), Length::from_steps(high)))
    }

    /// The top face's area: the plan area on a flat floor, exactly; on a sloped one, the plan
    /// area times `sqrt(1 + gradient^2)` to the nearest square step.
    ///
    /// # Errors
    /// [`SlabError::AreaNotExact`] for a plan area that is not whole; [`SlabError::OutOfRange`].
    pub fn top_area(&self) -> Result<TopArea, SlabError> {
        let plan = area(&self.boundary)?;
        let Some((over, under)) = self.slope.and_then(|s| s.stretch()) else {
            return Ok(TopArea {
                area: plan,
                exact: true,
            });
        };
        let big = |n: i128| {
            u128::try_from(n)
                .map(BigUint::from)
                .map_err(|_| SlabError::OutOfRange)
        };
        let (root, exact) = stretched_root(&big(plan.square_steps())?, &big(over)?, &big(under)?);
        let nearest = i128::try_from(root).map_err(|_| SlabError::OutOfRange)?;
        Ok(TopArea {
            area: Area::from_square_steps(nearest),
            exact,
        })
    }
}

/// The whole number nearest `plan * sqrt(over / under)`, and whether it is that exactly:
/// `under` is above zero ([`SlopeArrow::stretch`]).
#[allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision integers cannot overflow; the one division's divisor is above \
              zero, as the caller's stretch promises"
)]
fn stretched_root(plan: &BigUint, over: &BigUint, under: &BigUint) -> (BigUint, bool) {
    // The value squared is `squared / under`; `root` is the whole root of its floor, and the
    // nearest whole root is `root + 1` when (root + 1/2)^2 <= squared / under, that is when
    // (2 root + 1)^2 under <= 4 squared.
    let squared = plan * plan * over;
    let root = (&squared / under).sqrt();
    let exact = &root * &root * under == squared;
    let odd: BigUint = &root * 2_u32 + 1_u32;
    if &odd * &odd * under <= squared * 4_u32 {
        (root + 1_u32, exact)
    } else {
        (root, exact)
    }
}

/// The pieces of the slab edges along `profile`'s loops (material on the left of every edge),
/// checked as the module header says.
fn pieces(profile: &[Vec<Corner>], edges: &[SlabEdge]) -> Result<Vec<EdgePiece>, SlabError> {
    // Where each edge is: its loop and its index there.
    let mut place = BTreeMap::new();
    for (r, ring) in profile.iter().enumerate() {
        for (k, (id, _)) in ring.iter().enumerate() {
            place.insert(id.clone(), (r, k));
        }
    }
    let mut seen = BTreeSet::new();
    let mut profile_of = BTreeMap::new();
    for edge in edges {
        if !place.contains_key(&edge.edge) {
            return Err(SlabError::EdgeUnknown {
                edge: edge.edge.clone(),
            });
        }
        if !seen.insert(edge.edge.clone()) {
            return Err(SlabError::EdgeTwice {
                edge: edge.edge.clone(),
            });
        }
        edge.profile.check(&edge.edge)?;
        profile_of.insert(edge.edge.clone(), edge.profile);
    }
    let mut out = Vec::with_capacity(edges.len());
    for edge in edges {
        let &(r, k) = place.get(&edge.edge).ok_or(SlabError::OutOfRange)?;
        let ring = profile.get(r).ok_or(SlabError::OutOfRange)?;
        let piece = piece(ring, k, edge, &profile_of)?;
        check_inside(profile, (r, k), &edge.edge, piece.top)?;
        out.push(piece);
    }
    // No two pieces overlap; two mitred on one corner only share their mitre face.
    for (i, first) in out.iter().enumerate() {
        for second in out.iter().skip(i.saturating_add(1)) {
            if overlap(&first.top, &second.top) {
                return Err(SlabError::EdgesOverlap {
                    first: first.edge.clone(),
                    second: second.edge.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// Whether two convex plans, each anticlockwise, share any area: they do unless an edge of one
/// has every corner of the other on or to its right (a separating line, exactly on the grid).
/// A piece's plan is convex: two sides along its edge and its inner face, which run the same way,
/// and two ends, each square or on a mitre.
fn overlap(p: &[Point; 4], q: &[Point; 4]) -> bool {
    let apart = |a: &[Point; 4], b: &[Point; 4]| {
        (0..4).any(|k| {
            let (Some(from), Some(to)) = (a.get(k), a.get(wrap(k.saturating_add(1), 4))) else {
                return false;
            };
            b.iter().all(|c| orient(*from, *to, *c) <= 0)
        })
    };
    !(apart(p, q) || apart(q, p))
}

/// One edge of a loop as a slab edge sees it: where it runs and the edges on either side.
struct Run<'a> {
    edge: &'a SlabEdge,
    /// Its start and end.
    from: Point,
    to: Point,
    /// Its direction and its whole length.
    along: [i128; 2],
    length: i128,
    /// The edge into its start and the edge out of its end: each one's ID and direction.
    before: (&'a Id, [i128; 2]),
    after: (&'a Id, [i128; 2]),
}

/// Which end of a run.
#[derive(Clone, Copy)]
enum End {
    Start,
    Finish,
}

impl Run<'_> {
    fn off(&self) -> SlabError {
        SlabError::EdgeOffGrid {
            edge: self.edge.edge.clone(),
        }
    }

    /// The inner corner `w` in from the run at one end: on the mitre with the neighbour there
    /// when it carries the same profile, square across the run when it carries none.
    fn inner(
        &self,
        end: End,
        w: i128,
        profile_of: &BTreeMap<Id, EdgeProfile>,
    ) -> Result<Point, SlabError> {
        let (corner, (neighbour, dn)) = match end {
            End::Start => (self.from, self.before),
            End::Finish => (self.to, self.after),
        };
        let mine = self.edge.profile;
        match profile_of.get(neighbour) {
            Some(theirs) if *theirs == mine => {
                let ln = whole_root(dot(dn, dn).ok_or(SlabError::OutOfRange)?)
                    .ok_or_else(|| self.off())?;
                let (here, there) = ((self.along, self.length), (dn, ln));
                let point = match end {
                    End::Start => mitre(corner, there, here, w),
                    End::Finish => mitre(corner, here, there, w),
                };
                point.ok_or_else(|| self.off())
            }
            Some(_) => Err(match end {
                End::Start => SlabError::EdgesMeet {
                    first: neighbour.clone(),
                    second: self.edge.edge.clone(),
                },
                End::Finish => SlabError::EdgesMeet {
                    first: self.edge.edge.clone(),
                    second: neighbour.clone(),
                },
            }),
            None => {
                // A square end at a corner turning left by more than a right angle would stand
                // outside the floor.
                let (into, out_of) = match end {
                    End::Start => (dn, self.along),
                    End::Finish => (self.along, dn),
                };
                let turn = cross(into, out_of).ok_or(SlabError::OutOfRange)?;
                let ahead = dot(into, out_of).ok_or(SlabError::OutOfRange)?;
                if turn > 0 && ahead < 0 {
                    return Err(SlabError::EdgeEndOutside {
                        edge: self.edge.edge.clone(),
                    });
                }
                square(corner, self.along, self.length, w).ok_or_else(|| self.off())
            }
        }
    }
}

/// The piece of `edge`, which runs along edge `k` of `ring`.
fn piece(
    ring: &[Corner],
    k: usize,
    edge: &SlabEdge,
    profile_of: &BTreeMap<Id, EdgeProfile>,
) -> Result<EdgePiece, SlabError> {
    let (_, from) = ring.get(k).ok_or(SlabError::OutOfRange)?;
    let (after_id, to) = next(ring, k).ok_or(SlabError::OutOfRange)?;
    let (before_id, before) = prev(ring, k).ok_or(SlabError::OutOfRange)?;
    let (_, beyond) =
        next(ring, wrap(k.saturating_add(1), ring.len())).ok_or(SlabError::OutOfRange)?;
    let along = less(*from, *to);
    let mut run = Run {
        edge,
        from: *from,
        to: *to,
        along,
        length: 0,
        before: (before_id, less(*before, *from)),
        after: (after_id, less(*to, *beyond)),
    };
    run.length =
        whole_root(dot(along, along).ok_or(SlabError::OutOfRange)?).ok_or_else(|| run.off())?;
    let mut plans = Vec::with_capacity(2);
    for w in [edge.profile.width(), edge.profile.foot()] {
        let w = i128::from(w.steps());
        let start = run.inner(End::Start, w, profile_of)?;
        let finish = run.inner(End::Finish, w, profile_of)?;
        // The inner face runs the edge's way, or the piece is too wide for its edge.
        if dot(less(start, finish), along).is_none_or(|ahead| ahead <= 0) {
            return Err(SlabError::EdgeTooWide {
                edge: edge.edge.clone(),
            });
        }
        plans.push([run.from, run.to, finish, start]);
    }
    let (Some(top), Some(foot)) = (plans.first().copied(), plans.get(1).copied()) else {
        return Err(SlabError::OutOfRange);
    };
    let volume = prismatoid(top, foot, edge.profile.depth()).ok_or_else(|| {
        SlabError::EdgeVolumeNotExact {
            edge: edge.edge.clone(),
        }
    })?;
    Ok(EdgePiece {
        edge: edge.edge.clone(),
        profile: edge.profile,
        top,
        foot,
        length: Length::from_steps(i64::try_from(run.length).map_err(|_| SlabError::OutOfRange)?),
        volume,
    })
}

/// A piece's inner face, `top[3]` to `top[2]`, touches no edge of the floor but its own edge's
/// neighbours, and its middle is inside the floor (in the outer loop and in no hole).
fn check_inside(
    profile: &[Vec<Corner>],
    (r, k): (usize, usize),
    edge: &Id,
    top: [Point; 4],
) -> Result<(), SlabError> {
    let wide = || SlabError::EdgeTooWide { edge: edge.clone() };
    let [_, _, finish, start] = top;
    let inner = (start, finish);
    for (s, ring) in profile.iter().enumerate() {
        let n = ring.len();
        for (j, (_, c)) in ring.iter().enumerate() {
            let near = s == r
                && (j == k
                    || j == wrap(k.saturating_add(1), n)
                    || wrap(j.saturating_add(1), n) == k);
            if near {
                continue;
            }
            let d = next(ring, j).map_or(*c, |(_, p)| *p);
            if touch(inner, (*c, d)) {
                return Err(wide());
            }
        }
    }
    let middle = Point {
        x: start.x.saturating_add(finish.x),
        y: start.y.saturating_add(finish.y),
    };
    for (s, ring) in profile.iter().enumerate() {
        let twice: Vec<Point> = ring.iter().map(|(_, p)| doubled(*p)).collect();
        let held = inside(middle, &twice);
        if (s == 0) != held {
            return Err(wide());
        }
    }
    Ok(())
}

/// The volume between a plan `top` at one height and `foot` at `depth` below it, corner joined
/// to corner by flat faces (a prismatoid): `depth (A_top + A_foot + 4 A_mid) / 6`, twelve times
/// which is `depth` times the three twice-areas (the middle section's, doubled, is the polygon of
/// the corners' sums); `None` when it is not a whole number of cubic steps.
fn prismatoid(top: [Point; 4], foot: [Point; 4], depth: Length) -> Option<Volume> {
    let sums: Vec<Point> = top
        .iter()
        .zip(&foot)
        .map(|(a, b)| Point {
            x: a.x.saturating_add(b.x),
            y: a.y.saturating_add(b.y),
        })
        .collect();
    let twice = twice_signed_area(&top)
        .checked_add(twice_signed_area(&foot))?
        .checked_add(twice_signed_area(&sums))?;
    let twelve = twice.checked_mul(i128::from(depth.steps()))?;
    exactly(twelve, 12).map(Volume::from_cubic_steps)
}
