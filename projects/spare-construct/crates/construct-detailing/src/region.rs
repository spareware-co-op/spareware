//! Filled regions and masking regions: a closed outline in one view, with its area readable.
//!
//! A **filled region** carries a fill pattern (by its name in the project's fill patterns) and a
//! colour for the pattern's lines, and a boundary line style (or none, so the region has no
//! outline drawn). A **masking region** is opaque and has no pattern: it hides what is drawn under
//! it (the model, and detail items before it), in the paper's white; it too may have a boundary
//! line style.
//!
//! The outline is an exactly simple loop, checked as a view's crop outline is: at least three
//! corners, no corner given twice in a row, no two edges that cross, touch or overlap (neighbours
//! share only their one corner and may not fold back along each other), and an area that is not
//! zero, all by exact integer arithmetic on whole grid steps, never a float and never a tolerance.
//! The predicates are written here, not taken from a library, because they must be exact on
//! 128-bit whole numbers, which no floating-point geometry library gives; they are the crop
//! outline's predicates, in this crate's own types.

use crate::error::DetailError;
use crate::num::{add, at, mul, point_in_grid, sub};
use crate::record::{Props, Reader};
use crate::style::{StyleRef, colour_of, colour_spelled};
use construct_categories::{FILLED_REGIONS, MASKING_REGIONS};
use construct_grids::Point;
use construct_model::element::Element;
use construct_params::Area;
use construct_styles::Colour;
use kit_core::Id;
use std::cmp::Ordering;

/// The `kind` of a filled region record.
pub const REGION_KIND: &str = "detail.region";
/// The `kind` of a masking region record.
pub const MASKING_KIND: &str = "detail.masking";
/// The name a filled region record carries.
pub const REGION_NAME: &str = "Filled region";
/// The name a masking region record carries.
pub const MASKING_NAME: &str = "Masking region";
/// The fill pattern's name.
pub const PATTERN_KEY: &str = "@detail.region.pattern";
/// The colour of the pattern's lines, `#rrggbb`.
pub const COLOUR_KEY: &str = "@detail.region.colour";
/// The boundary's line style, or `none`.
pub const BOUNDARY_KEY: &str = "@detail.region.boundary";
/// The corners, each `@detail.region.corner.<nnnn>` with `.x` and `.y`.
pub const CORNER_KEY: &str = "@detail.region.corner";
/// The most corners an outline holds.
pub const MAX_CORNERS: usize = 1024;

const NONE: &str = "none";

/// What fills a region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fill {
    /// A pattern of the project's fill patterns, by name, in a colour.
    Pattern {
        /// The pattern's name.
        pattern: String,
        /// The colour of its lines.
        colour: Colour,
    },
    /// Opaque paper: no pattern; it hides what is under it.
    Masking,
}

/// A region's area, exact, in square grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionArea {
    /// Twice the area, exactly, in square steps (a whole number for any outline).
    pub twice: i128,
    /// The area to the nearest whole square step (a half rounds up).
    pub area: Area,
    /// Whether `area` is the true one: false only when twice the area is odd.
    pub exact: bool,
}

/// A filled or masking region: its ID, its view, its fill, its boundary and its outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    id: Id,
    view: Id,
    fill: Fill,
    boundary: Option<StyleRef>,
    outline: Vec<Point>,
}

fn orient(a: [i128; 2], b: [i128; 2], c: [i128; 2]) -> Result<Ordering, DetailError> {
    let left = mul(sub(b[0], a[0])?, sub(c[1], a[1])?)?;
    let right = mul(sub(b[1], a[1])?, sub(c[0], a[0])?)?;
    Ok(left.cmp(&right))
}

fn between(p: [i128; 2], q: [i128; 2], r: [i128; 2]) -> bool {
    r[0] >= p[0].min(q[0])
        && r[0] <= p[0].max(q[0])
        && r[1] >= p[1].min(q[1])
        && r[1] <= p[1].max(q[1])
}

/// Whether the closed segments `p1 p2` and `q1 q2` share a point.
fn meet(p1: [i128; 2], p2: [i128; 2], q1: [i128; 2], q2: [i128; 2]) -> Result<bool, DetailError> {
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
fn fold(p: [i128; 2], s: [i128; 2], q: [i128; 2]) -> Result<bool, DetailError> {
    if orient(p, s, q)? != Ordering::Equal {
        return Ok(false);
    }
    let across = mul(sub(p[0], s[0])?, sub(q[0], s[0])?)?;
    let down = mul(sub(p[1], s[1])?, sub(q[1], s[1])?)?;
    Ok(add(across, down)? > 0)
}

/// Twice the signed area of the outline: positive when it runs anticlockwise.
fn double_area(corners: &[Point]) -> Result<i128, DetailError> {
    let mut sum: i128 = 0;
    for (index, from) in corners.iter().enumerate() {
        let to = corners
            .get(index.saturating_add(1))
            .or_else(|| corners.first())
            .unwrap_or(from);
        let (a, b) = (at(*from), at(*to));
        sum = add(sum, sub(mul(a[0], b[1])?, mul(a[1], b[0])?)?)?;
    }
    Ok(sum)
}

/// Checks an outline: the rules in the module header.
///
/// # Errors
/// [`DetailError::FewCorners`], [`DetailError::TooManyCorners`], [`DetailError::RepeatedCorner`],
/// [`DetailError::SelfCrossing`], [`DetailError::NoArea`] or [`DetailError::OutOfRange`].
pub fn check_outline(corners: &[Point]) -> Result<(), DetailError> {
    let count = corners.len();
    if count < 3 {
        return Err(DetailError::FewCorners(count));
    }
    if count > MAX_CORNERS {
        return Err(DetailError::TooManyCorners(count));
    }
    for p in corners {
        point_in_grid(*p)?;
    }
    let corner = |index: usize| -> [i128; 2] {
        corners
            .get(index.checked_rem(count).unwrap_or(0))
            .map_or([0, 0], |p| at(*p))
    };
    for index in 0..count {
        if corner(index) == corner(index.saturating_add(1)) {
            return Err(DetailError::RepeatedCorner(index.saturating_add(1)));
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
                return Err(DetailError::SelfCrossing {
                    first: first.saturating_add(1),
                    second: second.saturating_add(1),
                });
            }
        }
    }
    if double_area(corners)? == 0 {
        return Err(DetailError::NoArea);
    }
    Ok(())
}

impl Region {
    /// A region.
    ///
    /// # Errors
    /// What [`check_outline`] refuses.
    pub fn new(
        id: Id,
        view: Id,
        fill: Fill,
        boundary: Option<StyleRef>,
        outline: Vec<Point>,
    ) -> Result<Self, DetailError> {
        check_outline(&outline)?;
        Ok(Self {
            id,
            view,
            fill,
            boundary,
            outline,
        })
    }

    /// Its permanent ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The view it is in.
    #[must_use]
    pub fn view(&self) -> &Id {
        &self.view
    }

    /// What fills it.
    #[must_use]
    pub fn fill(&self) -> &Fill {
        &self.fill
    }

    /// Whether it is a masking region.
    #[must_use]
    pub fn is_masking(&self) -> bool {
        matches!(self.fill, Fill::Masking)
    }

    /// The line style of its boundary, or `None` when no outline is drawn.
    #[must_use]
    pub fn boundary(&self) -> Option<&StyleRef> {
        self.boundary.as_ref()
    }

    /// Its corners, in order as given, the last joined to the first.
    #[must_use]
    pub fn outline(&self) -> &[Point] {
        &self.outline
    }

    /// The region with its corners edited.
    ///
    /// # Errors
    /// What [`check_outline`] refuses.
    pub fn with_outline(mut self, outline: Vec<Point>) -> Result<Self, DetailError> {
        check_outline(&outline)?;
        self.outline = outline;
        Ok(self)
    }

    /// The region with another boundary line style (`None`: no outline).
    #[must_use]
    pub fn with_boundary(mut self, boundary: Option<StyleRef>) -> Self {
        self.boundary = boundary;
        self
    }

    /// The region with another fill; a filled region cannot become a masking one nor the other way
    /// (it is another kind of record): the fill must be of the region's own kind.
    ///
    /// # Errors
    /// [`DetailError::BadStyle`] when the kinds differ.
    pub fn with_fill(mut self, fill: Fill) -> Result<Self, DetailError> {
        if self.is_masking() != matches!(fill, Fill::Masking) {
            return Err(DetailError::BadStyle(
                "A masking region has no pattern, and a filled region has one: delete it and make the other kind.".to_owned(),
            ));
        }
        self.fill = fill;
        Ok(self)
    }

    /// Its area, exactly: the outline's, which is simple, so the area it encloses.
    ///
    /// # Errors
    /// [`DetailError::OutOfRange`] for an outline too large to work out (never one the grid holds
    /// at fewer than 1,024 corners, but the check is made).
    pub fn area(&self) -> Result<RegionArea, DetailError> {
        let twice = double_area(&self.outline)?
            .checked_abs()
            .ok_or_else(crate::num::too_big)?;
        let half = twice.checked_div(2).unwrap_or(0);
        let odd = twice.checked_rem(2).unwrap_or(0) != 0;
        let area = if odd { add(half, 1)? } else { half };
        Ok(RegionArea {
            twice,
            area: Area::from_square_steps(area),
            exact: !odd,
        })
    }

    /// The outline as anticlockwise corners, in steps, ready to hatch or fill.
    #[must_use]
    pub fn anticlockwise(&self) -> Vec<[i128; 2]> {
        let mut corners: Vec<[i128; 2]> = self.outline.iter().map(|p| at(*p)).collect();
        if double_area(&self.outline).is_ok_and(|a| a < 0) {
            corners.reverse();
        }
        corners
    }

    /// The region as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let (kind, name, category) = if self.is_masking() {
            (MASKING_KIND, MASKING_NAME, MASKING_REGIONS)
        } else {
            (REGION_KIND, REGION_NAME, FILLED_REGIONS)
        };
        let mut props = Props::new(category.0, &self.view);
        if let Fill::Pattern { pattern, colour } = &self.fill {
            props.text(PATTERN_KEY, pattern);
            props.text(COLOUR_KEY, &colour_spelled(*colour));
        }
        props.text(
            BOUNDARY_KEY,
            &self
                .boundary
                .as_ref()
                .map_or(NONE.to_owned(), StyleRef::stored),
        );
        for (n, corner) in self.outline.iter().enumerate() {
            props.point(&format!("{CORNER_KEY}.{n:04}"), *corner);
        }
        props.finish(&self.id, document, kind, name, Vec::new())
    }

    /// Reads a region from its record; anything [`Region::to_record`] would not write is refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, DetailError> {
        let masking = record.kind == MASKING_KIND;
        let kind = if masking { MASKING_KIND } else { REGION_KIND };
        let read = Reader::new(record, kind, "region")?;
        let wrap = |e: DetailError| read.bad(&e.to_string());
        let fill = if masking {
            Fill::Masking
        } else {
            Fill::Pattern {
                pattern: read.text(PATTERN_KEY)?.to_owned(),
                colour: colour_of(read.text(COLOUR_KEY)?).map_err(wrap)?,
            }
        };
        let boundary = match read.text(BOUNDARY_KEY)? {
            NONE => None,
            text => Some(StyleRef::from_stored(text).map_err(wrap)?),
        };
        let mut outline = Vec::new();
        loop {
            let key = format!("{CORNER_KEY}.{:04}", outline.len());
            if !record.props.contains_key(&format!("{key}.x")) {
                break;
            }
            outline.push(read.point(&key)?);
        }
        let region =
            Self::new(record.id.clone(), read.view()?, fill, boundary, outline).map_err(wrap)?;
        read.same_as(&region.to_record(&record.document))?;
        Ok(region)
    }
}
