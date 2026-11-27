//! A detail line: a straight line or an arc, in one view, drawn with a line style.
//!
//! An arc is the sketch solver's: a centre, a start and an end, anticlockwise from the start to the
//! end, whose end must lie on the circle the start makes within the model's tolerance (0.01 mm).
//! The check is the solver's own (`check_drawn`), so a detail arc is as loose or as tight as any
//! arc of a floor or a family.

use crate::error::DetailError;
use crate::num::{at, point_in_grid};
use crate::record::{Props, Reader};
use crate::style::StyleRef;
use construct_categories::DETAIL_LINES;
use construct_grids::Point;
use construct_model::element::Element;
use construct_solver::{Sketch, check_drawn};
use kit_core::Id;

/// The `kind` of every detail line record, and its recipe's operation.
pub const LINE_KIND: &str = "detail.line";
/// The name every detail line record carries.
pub const LINE_NAME: &str = "Detail line";
/// How the line is drawn: `straight` or `arc`.
pub const SHAPE_KEY: &str = "@detail.line.shape";
/// The style, `builtin:<name>` or an own style's ID.
pub const STYLE_KEY: &str = "@detail.line.style";
/// The first point: a straight line's start, an arc's start.
pub const START_KEY: &str = "@detail.line.start";
/// The second point: a straight line's end, an arc's end.
pub const END_KEY: &str = "@detail.line.end";
/// An arc's centre.
pub const CENTRE_KEY: &str = "@detail.line.centre";

/// The shape of a detail line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// A straight line.
    Straight {
        /// Where it starts.
        start: Point,
        /// Where it ends.
        end: Point,
    },
    /// An arc, anticlockwise from its start to its end.
    Arc {
        /// Its centre.
        centre: Point,
        /// Where it starts.
        start: Point,
        /// Where it ends.
        end: Point,
    },
}

impl Curve {
    /// A straight line.
    ///
    /// # Errors
    /// [`DetailError::ZeroLength`] for two ends that are one point, [`DetailError::OutOfRange`]
    /// past the kernel's grid.
    pub fn straight(start: Point, end: Point) -> Result<Self, DetailError> {
        point_in_grid(start)?;
        point_in_grid(end)?;
        if start == end {
            return Err(DetailError::ZeroLength);
        }
        Ok(Self::Straight { start, end })
    }

    /// An arc about `centre`, anticlockwise from `start` to `end`.
    ///
    /// # Errors
    /// [`DetailError::BadArc`] with the solver's words when the three points do not make an arc
    /// (the centre at an end, the ends one point, or the end off the circle by more than 0.01 mm);
    /// [`DetailError::OutOfRange`] past the kernel's grid.
    pub fn arc(centre: Point, start: Point, end: Point) -> Result<Self, DetailError> {
        for p in [centre, start, end] {
            point_in_grid(p)?;
        }
        let bad = |why: &dyn std::fmt::Display| DetailError::BadArc(format!("Not an arc: {why}."));
        let mut sketch = Sketch::new();
        let mut add = |p: Point| {
            sketch
                .add_point(p.x.steps(), p.y.steps())
                .map_err(|e| bad(&e))
        };
        let (c, s, e) = (add(centre)?, add(start)?, add(end)?);
        sketch.add_arc(c, s, e).map_err(|e| bad(&e))?;
        check_drawn(&sketch).map_err(|e| bad(&e))?;
        Ok(Self::Arc { centre, start, end })
    }

    /// The points that define it: a straight line's ends; an arc's centre, start and end.
    #[must_use]
    pub fn points(&self) -> Vec<Point> {
        match *self {
            Self::Straight { start, end } => vec![start, end],
            Self::Arc { centre, start, end } => vec![centre, start, end],
        }
    }

    /// Whether it is an arc.
    #[must_use]
    pub fn is_arc(&self) -> bool {
        matches!(self, Self::Arc { .. })
    }

    /// Its start and end, in steps.
    #[must_use]
    pub fn ends(&self) -> ([i128; 2], [i128; 2]) {
        match *self {
            Self::Straight { start, end } | Self::Arc { start, end, .. } => (at(start), at(end)),
        }
    }
}

/// A detail line: its ID, its view, its style and its curve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailLine {
    id: Id,
    view: Id,
    style: StyleRef,
    curve: Curve,
}

impl DetailLine {
    /// A line.
    #[must_use]
    pub fn new(id: Id, view: Id, style: StyleRef, curve: Curve) -> Self {
        Self {
            id,
            view,
            style,
            curve,
        }
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

    /// Its line style.
    #[must_use]
    pub fn style(&self) -> &StyleRef {
        &self.style
    }

    /// Its shape.
    #[must_use]
    pub fn curve(&self) -> &Curve {
        &self.curve
    }

    /// The line with another shape (its points edited).
    #[must_use]
    pub fn with_curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// The line with another style.
    #[must_use]
    pub fn with_style(mut self, style: StyleRef) -> Self {
        self.style = style;
        self
    }

    /// The line as an element record of `document` (FORMAT.md beside this crate).
    #[must_use]
    pub fn to_record(&self, document: &Id) -> Element {
        let mut props = Props::new(DETAIL_LINES.0, &self.view);
        props.text(STYLE_KEY, &self.style.stored());
        match self.curve {
            Curve::Straight { start, end } => {
                props.text(SHAPE_KEY, "straight");
                props.point(START_KEY, start);
                props.point(END_KEY, end);
            }
            Curve::Arc { centre, start, end } => {
                props.text(SHAPE_KEY, "arc");
                props.point(CENTRE_KEY, centre);
                props.point(START_KEY, start);
                props.point(END_KEY, end);
            }
        }
        props.finish(&self.id, document, LINE_KIND, LINE_NAME, Vec::new())
    }

    /// Reads a line from its record; anything [`DetailLine::to_record`] would not write is refused.
    ///
    /// # Errors
    /// [`DetailError::Malformed`], naming the record.
    pub fn from_record(record: &Element) -> Result<Self, DetailError> {
        let read = Reader::new(record, LINE_KIND, "detail line")?;
        let wrap = |e: DetailError| read.bad(&e.to_string());
        let style = StyleRef::from_stored(read.text(STYLE_KEY)?).map_err(wrap)?;
        let curve = match read.text(SHAPE_KEY)? {
            "straight" => {
                Curve::straight(read.point(START_KEY)?, read.point(END_KEY)?).map_err(wrap)?
            }
            "arc" => Curve::arc(
                read.point(CENTRE_KEY)?,
                read.point(START_KEY)?,
                read.point(END_KEY)?,
            )
            .map_err(wrap)?,
            _ => return Err(read.bad("holds a shape that is not straight or arc")),
        };
        let line = Self::new(record.id.clone(), read.view()?, style, curve);
        read.same_as(&line.to_record(&record.document))?;
        Ok(line)
    }
}
