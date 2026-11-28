//! A view's crop region: the part of the view that is drawn, as a rectangle or a closed outline.
//!
//! A crop is held in the view's own coordinates: for a plan, the project's x and y (the plan's
//! drawing keeps them, y up the sheet); for a section or an elevation, the distance along its line
//! from the first end and the height above the project's zero. It has an outline (the region), an
//! optional second outline for annotation (the annotation crop: what a view's notes and
//! dimensions are kept to, apart from the model), and two switches: **on** (the crop applies, so
//! the view's extent is the region's) and **shown** (the region's outline is drawn in the view).
//! The switches are the person's; the region stays when the crop is off or hidden.
//!
//! A region is exact. Every corner is a [`Length`], and a closed outline (a shaped crop) is checked
//! with exact integer arithmetic on the lengths' whole steps, never a float and never a tolerance:
//! at least three corners, no corner given twice in a row, no two edges that cross or touch except
//! the neighbours at their shared corner (which may not fold back along each other), and an area
//! that is not zero. A corner or a product too large for that arithmetic is refused as out of
//! range, not rounded. A rectangle is two opposite corners with the minimum below the maximum on
//! both axes. Which way round an outline runs is kept as given.
//!
//! What a crop does to the drawing (clipping lines and regions to the outline) is not here: this
//! module stores and serves the region. Only the kinds that draw the model hold a crop: a floor
//! plan, a ceiling plan, a section and an elevation.

use crate::cut::Point;
use crate::error::ViewError;
use crate::kind::ViewKind;
use construct_params::{Length, Value};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// The key a crop is stored under on a view record.
pub const CROP_KEY: &str = "@view.crop";
/// The version of the crop's stored text.
pub const CROP_FORMAT_VERSION: u32 = 1;

/// The shape of a crop: a rectangle or a simple closed outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Region {
    /// Two opposite corners: the lower left and the upper right, each axis strictly below.
    Rectangle {
        /// The corner with the least x and the least y.
        min: Point,
        /// The corner with the greatest x and the greatest y.
        max: Point,
    },
    /// A closed outline through its corners in order, the last joined to the first.
    Outline(Vec<Point>),
}

fn out_of_range() -> ViewError {
    ViewError::OutOfRange("a crop outline is too large to test exactly".to_owned())
}

fn at(p: Point) -> (i128, i128) {
    (i128::from(p.x.steps()), i128::from(p.y.steps()))
}

/// The difference of two whole-step values; both come from 64-bit lengths, so it cannot saturate.
fn gap(a: i128, b: i128) -> i128 {
    a.saturating_sub(b)
}

/// Where `c` lies about the line from `a` to `b`: `Greater` to its left, `Less` to its right,
/// `Equal` on it. Exact.
fn orient(a: Point, b: Point, c: Point) -> Result<Ordering, ViewError> {
    let (a, b, c) = (at(a), at(b), at(c));
    let (bx, by) = (gap(b.0, a.0), gap(b.1, a.1));
    let (cx, cy) = (gap(c.0, a.0), gap(c.1, a.1));
    let left = bx.checked_mul(cy).ok_or_else(out_of_range)?;
    let right = by.checked_mul(cx).ok_or_else(out_of_range)?;
    Ok(left.cmp(&right))
}

/// Whether `r`, known to lie on the line through `p` and `q`, lies between them.
fn between(p: Point, q: Point, r: Point) -> bool {
    let (p, q, r) = (at(p), at(q), at(r));
    r.0 >= p.0.min(q.0) && r.0 <= p.0.max(q.0) && r.1 >= p.1.min(q.1) && r.1 <= p.1.max(q.1)
}

/// Whether the closed segments `p1 p2` and `q1 q2` share a point.
fn meet(p1: Point, p2: Point, q1: Point, q2: Point) -> Result<bool, ViewError> {
    let o1 = orient(p1, p2, q1)?;
    let o2 = orient(p1, p2, q2)?;
    let o3 = orient(q1, q2, p1)?;
    let o4 = orient(q1, q2, p2)?;
    if o1 != o2 && o3 != o4 {
        return Ok(true);
    }
    Ok((o1 == Ordering::Equal && between(p1, p2, q1))
        || (o2 == Ordering::Equal && between(p1, p2, q2))
        || (o3 == Ordering::Equal && between(q1, q2, p1))
        || (o4 == Ordering::Equal && between(q1, q2, p2)))
}

/// Whether two edges that share the corner `s` fold back along each other: `p` and `q` are their
/// other ends.
fn fold(p: Point, s: Point, q: Point) -> Result<bool, ViewError> {
    if orient(p, s, q)? != Ordering::Equal {
        return Ok(false);
    }
    let (p, s, q) = (at(p), at(s), at(q));
    let across = gap(p.0, s.0)
        .checked_mul(gap(q.0, s.0))
        .ok_or_else(out_of_range)?;
    let down = gap(p.1, s.1)
        .checked_mul(gap(q.1, s.1))
        .ok_or_else(out_of_range)?;
    Ok(across.checked_add(down).ok_or_else(out_of_range)? > 0)
}

/// Twice the signed area of the outline (positive when it runs counter-clockwise).
fn double_area(corners: &[Point]) -> Result<i128, ViewError> {
    let mut sum: i128 = 0;
    for (index, from) in corners.iter().enumerate() {
        let next = index.checked_add(1).unwrap_or(0);
        let to = corners
            .get(next)
            .or_else(|| corners.first())
            .unwrap_or(from);
        let (a, b) = (at(*from), at(*to));
        let term =
            a.0.checked_mul(b.1)
                .zip(a.1.checked_mul(b.0))
                .and_then(|(left, right)| left.checked_sub(right))
                .ok_or_else(out_of_range)?;
        sum = sum.checked_add(term).ok_or_else(out_of_range)?;
    }
    Ok(sum)
}

impl Region {
    /// A rectangle from its lower left and upper right corners.
    ///
    /// # Errors
    /// [`ViewError::CropEmptyRectangle`] unless `min` is below `max` on both axes.
    pub fn rectangle(min: Point, max: Point) -> Result<Self, ViewError> {
        if min.x >= max.x || min.y >= max.y {
            return Err(ViewError::CropEmptyRectangle);
        }
        Ok(Self::Rectangle { min, max })
    }

    /// A closed outline through `corners`, checked exactly to be simple.
    ///
    /// # Errors
    /// [`ViewError::CropFewCorners`] for fewer than three corners, [`ViewError::CropRepeatedCorner`]
    /// for a corner that equals the next, [`ViewError::CropSelfCrossing`] for two edges that
    /// cross, touch or overlap (neighbours may share only their one corner), [`ViewError::CropNoArea`]
    /// for an outline of no area, [`ViewError::OutOfRange`] for coordinates too large to test.
    pub fn outline(corners: Vec<Point>) -> Result<Self, ViewError> {
        let count = corners.len();
        if count < 3 {
            return Err(ViewError::CropFewCorners { count });
        }
        let corner = |index: usize| -> Point {
            // `index` is reduced modulo the corner count, so it is in range.
            corners
                .get(index.checked_rem(count).unwrap_or(0))
                .copied()
                .unwrap_or(Point {
                    x: Length::from_steps(0),
                    y: Length::from_steps(0),
                })
        };
        for index in 0..count {
            if corner(index) == corner(index.saturating_add(1)) {
                return Err(ViewError::CropRepeatedCorner {
                    corner: index.saturating_add(1),
                });
            }
        }
        for first in 0..count {
            for second in first.saturating_add(1)..count {
                let next = second == first.saturating_add(1);
                let wraps = first == 0 && second == count.saturating_sub(1);
                let crossing = if next {
                    fold(
                        corner(first),
                        corner(second),
                        corner(second.saturating_add(1)),
                    )?
                } else if wraps {
                    fold(
                        corner(second),
                        corner(first),
                        corner(first.saturating_add(1)),
                    )?
                } else {
                    meet(
                        corner(first),
                        corner(first.saturating_add(1)),
                        corner(second),
                        corner(second.saturating_add(1)),
                    )?
                };
                if crossing {
                    return Err(ViewError::CropSelfCrossing {
                        first: first.saturating_add(1),
                        second: second.saturating_add(1),
                    });
                }
            }
        }
        if double_area(&corners)? == 0 {
            return Err(ViewError::CropNoArea);
        }
        Ok(Self::Outline(corners))
    }

    /// The corners in order: a rectangle's four run counter-clockwise from its lower left.
    #[must_use]
    pub fn corners(&self) -> Vec<Point> {
        match self {
            Self::Rectangle { min, max } => vec![
                *min,
                Point { x: max.x, y: min.y },
                *max,
                Point { x: min.x, y: max.y },
            ],
            Self::Outline(corners) => corners.clone(),
        }
    }

    /// The least and greatest corner of the box around the region.
    #[must_use]
    pub fn bounds(&self) -> (Point, Point) {
        match self {
            Self::Rectangle { min, max } => (*min, *max),
            Self::Outline(corners) => {
                let mut low = corners.first().copied().unwrap_or(Point {
                    x: Length::from_steps(0),
                    y: Length::from_steps(0),
                });
                let mut high = low;
                for p in corners {
                    low = Point {
                        x: low.x.min(p.x),
                        y: low.y.min(p.y),
                    };
                    high = Point {
                        x: high.x.max(p.x),
                        y: high.y.max(p.y),
                    };
                }
                (low, high)
            }
        }
    }

    /// How wide and how high the box around the region is, or `None` when that does not fit a
    /// length.
    #[must_use]
    pub fn size(&self) -> Option<(Length, Length)> {
        let (low, high) = self.bounds();
        Some((high.x.checked_sub(low.x)?, high.y.checked_sub(low.y)?))
    }

    /// The shape's key: `rectangle` or `outline`.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Rectangle { .. } => "rectangle",
            Self::Outline(_) => "outline",
        }
    }
}

/// A view's crop: a region, an optional annotation region and two switches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crop {
    region: Region,
    annotation: Option<Region>,
    on: bool,
    shown: bool,
}

impl Crop {
    /// A crop of `region`, on and shown, with no annotation crop.
    #[must_use]
    pub fn new(region: Region) -> Self {
        Self {
            region,
            annotation: None,
            on: true,
            shown: true,
        }
    }

    /// The region the model is cropped to.
    #[must_use]
    pub fn region(&self) -> &Region {
        &self.region
    }

    /// The region the view's annotation is cropped to, if it has its own.
    #[must_use]
    pub fn annotation(&self) -> Option<&Region> {
        self.annotation.as_ref()
    }

    /// Whether the crop applies: the view's extent is the region's.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Whether the region's outline is shown in the view.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// The crop with another region.
    #[must_use]
    pub fn with_region(mut self, region: Region) -> Self {
        self.region = region;
        self
    }

    /// The crop with an annotation region, or with none.
    #[must_use]
    pub fn with_annotation(mut self, annotation: Option<Region>) -> Self {
        self.annotation = annotation;
        self
    }

    /// The crop switched on or off.
    #[must_use]
    pub fn with_on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// The crop's outline shown or hidden.
    #[must_use]
    pub fn with_shown(mut self, shown: bool) -> Self {
        self.shown = shown;
        self
    }

    /// Asks whether a view of `kind` can hold a crop.
    ///
    /// # Errors
    /// [`ViewError::NoCropOnKind`] for a three-dimensional view, a drafting view or a legend.
    pub fn check_kind(kind: &ViewKind) -> Result<(), ViewError> {
        match kind {
            ViewKind::FloorPlan(_)
            | ViewKind::CeilingPlan(_)
            | ViewKind::Section(_)
            | ViewKind::Elevation(_) => Ok(()),
            other => Err(ViewError::NoCropOnKind {
                kind: other.key().replace('_', " "),
            }),
        }
    }

    /// The stored text.
    pub(crate) fn to_text(&self) -> String {
        let stored = Stored {
            format: CROP_FORMAT_VERSION,
            on: self.on,
            shown: self.shown,
            region: StoredRegion::of(&self.region),
            annotation: self.annotation.as_ref().map(StoredRegion::of),
        };
        // Plain structs of strings and flags: serialising one cannot fail, and a record that held
        // nothing here would be refused when read.
        serde_json::to_string(&stored).unwrap_or_default()
    }

    /// Reads the stored text. The caller compares the record it makes of the result with the one
    /// it was given, so any other spelling of the same crop is refused there.
    pub(crate) fn from_text(text: &str) -> Result<Self, String> {
        let stored: Stored = serde_json::from_str(text)
            .map_err(|why| format!("holds a crop that does not read: {why}"))?;
        if stored.format != CROP_FORMAT_VERSION {
            return Err(format!(
                "holds a crop of version {}, which is not known",
                stored.format
            ));
        }
        let region = stored.region.read()?;
        let annotation = match stored.annotation {
            Some(one) => Some(one.read()?),
            None => None,
        };
        Ok(Self {
            region,
            annotation,
            on: stored.on,
            shown: stored.shown,
        })
    }
}

/// The stored crop, in this order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    on: bool,
    shown: bool,
    region: StoredRegion,
    annotation: Option<StoredRegion>,
}

/// A stored region: lengths in construct-params' stored text.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum StoredRegion {
    /// The lower left x and y, then the upper right x and y.
    Rectangle([String; 4]),
    /// Each corner's x and y.
    Outline(Vec<[String; 2]>),
}

fn text(length: Length) -> String {
    Value::Length(length).stored()
}

fn read_length(text: &str) -> Result<Length, String> {
    match Value::from_stored(text) {
        Some(Value::Length(length)) => Ok(length),
        _ => Err(format!(
            "holds a crop length {text:?} that is not a stored length"
        )),
    }
}

fn read_point(pair: &[String; 2]) -> Result<Point, String> {
    let [x, y] = pair;
    Ok(Point {
        x: read_length(x)?,
        y: read_length(y)?,
    })
}

impl StoredRegion {
    fn of(region: &Region) -> Self {
        match region {
            Region::Rectangle { min, max } => {
                Self::Rectangle([text(min.x), text(min.y), text(max.x), text(max.y)])
            }
            Region::Outline(corners) => {
                Self::Outline(corners.iter().map(|p| [text(p.x), text(p.y)]).collect())
            }
        }
    }

    fn read(&self) -> Result<Region, String> {
        match self {
            Self::Rectangle([x0, y0, x1, y1]) => Region::rectangle(
                read_point(&[x0.clone(), y0.clone()])?,
                read_point(&[x1.clone(), y1.clone()])?,
            )
            .map_err(|e| format!("holds a crop that is not allowed: {e}")),
            Self::Outline(pairs) => Region::outline(
                pairs
                    .iter()
                    .map(read_point)
                    .collect::<Result<Vec<_>, String>>()?,
            )
            .map_err(|e| format!("holds a crop that is not allowed: {e}")),
        }
    }
}
