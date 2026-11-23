//! A drawing as plain values: lines and regions in view coordinates, each knowing its source, in
//! one canonical order, and its stored form.
//!
//! What it holds:
//!
//! - [`ViewPoint`]: an exact point of the view, `x / den` and `y / den` grid steps, in lowest
//!   terms over a positive denominator (a grid point has `den` 1).
//! - [`Line`]: one piece of line work. Its [`Shape`] is a segment or an arc in view coordinates;
//!   its [`LineKind`] says whether the view's plane cuts it ([`LineKind::Cut`]) or it is an edge
//!   seen in projection ([`LineKind::Projected`]); its [`Source`] names the element and the face
//!   or edge it came from, by persistent name and role; its category (a subcategory where the
//!   element gives one) is what pens and patterns are looked up by ([`crate::pens`]).
//! - [`Region`]: a closed cut region of one element, its loops in view coordinates (the outline
//!   turning counter-clockwise, holes clockwise) and its material, for the fill to come.
//! - [`Drawing`]: the view, its lines and its regions.
//!
//! The canonical order: lines by kind (cut first), then source (element, face or edge, name,
//! role), then category, then shape; regions by element, then category, material and loops.
//! [`Drawing::new`] checks every value and puts them in that order, so two drawings of the same
//! lines given in any order are equal and store the same bytes.
//!
//! The stored form ([`Drawing::to_bytes`]): JSON, `{"format":1,"view":...,"lines":[...],
//! "regions":[...]}`, every number a whole number, the view's cut a stored length
//! (`length:q0:<steps>`, construct-params' `Value::stored`). [`Drawing::from_bytes`] reads only
//! bytes in that one form: anything else, a reordered list included, is refused.

use construct_categories::CategoryId;
use construct_materials::MaterialId;
use construct_params::Value;
use kit_core::Id;
use serde::{Deserialize, Serialize};

use crate::DrawingError;
use crate::view::{Looking, View};

/// The stored form's version.
pub const FORMAT_VERSION: u32 = 1;

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a.checked_rem(b).unwrap_or(0));
    }
    a
}

/// An exact point of a view: `x / den` and `y / den` grid steps.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct ViewPoint {
    /// Across the sheet, times `den`.
    pub x: i128,
    /// Up the sheet, times `den`.
    pub y: i128,
    /// The denominator: positive, sharing no factor with both `x` and `y`.
    pub den: i128,
}

impl ViewPoint {
    /// The point `(x / den, y / den)`, in lowest terms over a positive denominator.
    ///
    /// # Errors
    ///
    /// [`DrawingError::BadPoint`] for a zero denominator, [`DrawingError::TooBig`] for a value
    /// past 128 bits once its sign is moved.
    pub fn new(x: i128, y: i128, den: i128) -> Result<Self, DrawingError> {
        if den == 0 {
            return Err(DrawingError::BadPoint);
        }
        let g = gcd(gcd(x.unsigned_abs(), y.unsigned_abs()), den.unsigned_abs());
        let g = i128::try_from(g).map_err(|_| DrawingError::TooBig)?;
        let g = if den < 0 {
            g.checked_neg().ok_or(DrawingError::TooBig)?
        } else {
            g
        };
        let part = |v: i128| v.checked_div(g).ok_or(DrawingError::TooBig);
        Ok(Self {
            x: part(x)?,
            y: part(y)?,
            den: part(den)?,
        })
    }

    /// The grid point `(x, y)` steps.
    #[must_use]
    pub const fn grid(x: i128, y: i128) -> Self {
        Self { x, y, den: 1 }
    }

    /// `self` against `other` by x, then y, exactly.
    ///
    /// # Errors
    ///
    /// [`DrawingError::TooBig`] when a cross product passes 128 bits.
    pub fn order(self, other: Self) -> Result<std::cmp::Ordering, DrawingError> {
        let by = |a: i128, b: i128| {
            a.checked_mul(other.den)
                .zip(b.checked_mul(self.den))
                .map(|(l, r)| l.cmp(&r))
                .ok_or(DrawingError::TooBig)
        };
        Ok(by(self.x, other.x)?.then(by(self.y, other.y)?))
    }

    /// Whether it is in its one form: in lowest terms over a positive denominator.
    fn checked(self) -> Result<Self, DrawingError> {
        if Self::new(self.x, self.y, self.den)? == self {
            Ok(self)
        } else {
            Err(DrawingError::BadPoint)
        }
    }
}

/// Which way an arc turns from its start to its end, seen in the drawing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Turn {
    /// Counter-clockwise.
    Counterclockwise,
    /// Clockwise.
    Clockwise,
}

/// The shape of a line, in view coordinates.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Shape {
    /// The straight segment from `start` to `end`.
    Segment {
        /// Where it starts.
        start: ViewPoint,
        /// Where it ends.
        end: ViewPoint,
    },
    /// The arc of the circle about `centre` from `start` to `end`, turning `turn`; the two ends
    /// are exactly as far from the centre, and an arc whose ends meet is the whole circle.
    Arc {
        /// The circle's centre.
        centre: ViewPoint,
        /// Where it starts.
        start: ViewPoint,
        /// Where it ends.
        end: ViewPoint,
        /// Which way it turns.
        turn: Turn,
    },
}

/// `(p - c)²` times `den²` of both, exact: the squared distance of `p` from `c`, over the
/// squared common denominator `p.den * c.den`.
fn squared(c: ViewPoint, p: ViewPoint) -> Result<i128, DrawingError> {
    let d = |a: i128, b: i128| {
        a.checked_mul(c.den)
            .zip(b.checked_mul(p.den))
            .and_then(|(l, r)| l.checked_sub(r))
            .and_then(|v| v.checked_mul(v))
    };
    d(p.x, c.x)
        .zip(d(p.y, c.y))
        .and_then(|(a, b)| a.checked_add(b))
        .ok_or(DrawingError::TooBig)
}

impl Shape {
    /// Whether it is a shape a drawing holds: a segment of some length; an arc whose ends are
    /// equally far from its centre, which is not one of them.
    fn check(self) -> Result<Self, DrawingError> {
        match self {
            Self::Segment { start, end } => {
                start.checked()?;
                end.checked()?;
                if start == end {
                    return Err(DrawingError::BadShape("a segment of no length".to_owned()));
                }
            }
            Self::Arc {
                centre, start, end, ..
            } => {
                centre.checked()?;
                start.checked()?;
                end.checked()?;
                // Over the denominators: |s - c|² / (s.den c.den)² against |e - c|² / (e.den c.den)².
                let (s, e) = (squared(centre, start)?, squared(centre, end)?);
                let sd = start
                    .den
                    .checked_mul(start.den)
                    .ok_or(DrawingError::TooBig)?;
                let ed = end.den.checked_mul(end.den).ok_or(DrawingError::TooBig)?;
                let l = s.checked_mul(ed).ok_or(DrawingError::TooBig)?;
                let r = e.checked_mul(sd).ok_or(DrawingError::TooBig)?;
                if s == 0 || l != r {
                    return Err(DrawingError::BadShape(
                        "an arc whose ends are not on one circle about its centre".to_owned(),
                    ));
                }
            }
        }
        Ok(self)
    }
}

/// Whether the view's plane cuts a line, or it is seen in projection.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// The boundary of a cut region: where the view's plane passes through the element.
    Cut,
    /// An edge seen beyond the cut, or in a view that cuts nothing.
    Projected,
}

/// What part of an element's shape a line came from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Entity {
    /// A face: a cut line lies in the face the plane crosses.
    Face,
    /// An edge.
    Edge,
}

/// Where a line came from: the element and the face or edge, by its persistent name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The element.
    #[serde(with = "kit_ops::id")]
    pub element: Id,
    /// A face or an edge.
    pub entity: Entity,
    /// Its persistent name (the kernel's `NameId`): stays with it through regeneration.
    pub name: String,
    /// What the element's operation calls it, such as `exterior`.
    pub role: String,
}

/// One piece of line work.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Cut or seen in projection.
    pub kind: LineKind,
    /// Where it came from.
    pub source: Source,
    /// The category its pen and pattern are looked up by: the element's, or a subcategory.
    pub category: CategoryId,
    /// Its shape.
    pub shape: Shape,
}

/// A closed cut region of one element, for the fill to come.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// The element cut.
    #[serde(with = "kit_ops::id")]
    pub element: Id,
    /// Its category.
    pub category: CategoryId,
    /// Its material, where the element has one.
    pub material: Option<MaterialId>,
    /// The loops: outlines counter-clockwise and holes clockwise, each of at least three points,
    /// each from its least point (by x, then y, exactly), the last joined back to the first.
    pub loops: Vec<Vec<ViewPoint>>,
}

/// A drawing of one view: its lines and regions, checked and in the canonical order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawing {
    view: View,
    lines: Vec<Line>,
    regions: Vec<Region>,
}

/// The stored form of the view.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredView {
    looking: Looking,
    cut: String,
}

/// The stored form.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    format: u32,
    view: StoredView,
    lines: Vec<Line>,
    regions: Vec<Region>,
}

impl Drawing {
    /// The drawing of `view` with `lines` and `regions`, each checked, in the canonical order.
    ///
    /// # Errors
    ///
    /// [`DrawingError::BadPoint`] and [`DrawingError::BadShape`] for a point or shape not as the
    /// types say, a loop of fewer than three points among them.
    pub fn new(
        view: View,
        mut lines: Vec<Line>,
        mut regions: Vec<Region>,
    ) -> Result<Self, DrawingError> {
        for line in &lines {
            line.shape.check()?;
        }
        for region in &regions {
            for ring in &region.loops {
                if ring.len() < 3 {
                    return Err(DrawingError::BadShape(
                        "a loop of fewer than three points".to_owned(),
                    ));
                }
                let mut first = None;
                for &p in ring {
                    p.checked()?;
                    let head = *first.get_or_insert(p);
                    if p.order(head)? == std::cmp::Ordering::Less {
                        return Err(DrawingError::BadShape(
                            "a loop that does not start at its least point".to_owned(),
                        ));
                    }
                }
            }
        }
        lines.sort();
        regions.sort();
        Ok(Self {
            view,
            lines,
            regions,
        })
    }

    /// The view.
    #[must_use]
    pub fn view(&self) -> View {
        self.view
    }

    /// The lines, in the canonical order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The regions, in the canonical order.
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// The stored bytes: the same drawing always gives the same bytes.
    ///
    /// # Errors
    ///
    /// [`DrawingError::Format`] if the writer fails, which it does not for these types.
    pub fn to_bytes(&self) -> Result<Vec<u8>, DrawingError> {
        serde_json::to_vec(&Stored {
            format: FORMAT_VERSION,
            view: StoredView {
                looking: self.view.looking,
                cut: Value::Length(self.view.cut).stored(),
            },
            lines: self.lines.clone(),
            regions: self.regions.clone(),
        })
        .map_err(|why| DrawingError::Format(why.to_string()))
    }

    /// The drawing stored as `bytes`, which must be in its one canonical form.
    ///
    /// # Errors
    ///
    /// [`DrawingError::Format`] for bytes that are not a drawing of this version, or not exactly
    /// what [`Drawing::to_bytes`] writes for it; the errors of [`Drawing::new`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DrawingError> {
        let stored: Stored =
            serde_json::from_slice(bytes).map_err(|why| DrawingError::Format(why.to_string()))?;
        if stored.format != FORMAT_VERSION {
            return Err(DrawingError::Format(format!(
                "format {} is not {FORMAT_VERSION}",
                stored.format
            )));
        }
        let Some(Value::Length(cut)) = Value::from_stored(&stored.view.cut) else {
            return Err(DrawingError::Format(
                "the view's cut is not a stored length".to_owned(),
            ));
        };
        let view = View {
            looking: stored.view.looking,
            cut,
        };
        let drawing = Self::new(view, stored.lines, stored.regions)?;
        if drawing.to_bytes()? != bytes {
            return Err(DrawingError::Format(
                "not the canonical form of this drawing".to_owned(),
            ));
        }
        Ok(drawing)
    }
}
