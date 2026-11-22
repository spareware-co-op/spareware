//! Numbered pens: the printed width each draws with, for each view scale.
//!
//! There are two tables, because two kinds of line behave differently:
//!
//! - [`PenTable`]: model lines (edges of walls, floors, doors and the rest of the building). A
//!   pen has one width for each scale in the table's list, so the same pen draws thinner on a
//!   small-scale drawing.
//! - [`AnnotationPens`]: annotation lines (dimensions, tags, text boxes, leaders) and perspective
//!   lines. A pen has one width at every scale, because annotation is printed at a fixed size
//!   whatever the view's scale.
//!
//! Every function that changes a table returns a new, checked table and leaves the old one as it
//! was. The edit log's wiring is a later job.

use crate::error::StyleError;
use crate::units::{Micrometres, Scale, check_name};
use serde::{Deserialize, Serialize};

/// The widest a pen may print: ten millimetres.
const WIDEST_PEN_MICROMETRES: u32 = 10_000;

/// The most scales a table may list.
const MOST_SCALES: usize = 64;

/// The most pens a table may hold.
const MOST_PENS: usize = 256;

fn check_pen_width(width: Micrometres) -> Result<(), StyleError> {
    if width.get() > WIDEST_PEN_MICROMETRES {
        return Err(StyleError::BadLength {
            micrometres: width.get(),
        });
    }
    Ok(())
}

/// A numbered model-line pen: its name and its printed width at each of its table's scales, in
/// the table's order.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Pen {
    /// Its number, from 1. Object styles name a pen by this number.
    pub number: u16,
    /// The name people see.
    pub name: String,
    /// One width for each of the table's scales, in the same order.
    pub widths: Vec<Micrometres>,
}

/// The pens for model lines, and the list of scales their widths are given for.
///
/// **Lookup rule.** The scales are listed from the largest scale (smallest denominator) to the
/// smallest. Asking for a width at a scale that is listed takes that row. A scale between two
/// listed ones takes the nearer coarser row: the listed scale just past it, the one with the next
/// larger denominator (1:75 in a list with 1:50 and 1:100 takes the 1:100 row). A scale larger
/// than the first listed takes the first row, and a scale smaller than the last listed takes the
/// last row. No width is ever worked out between rows.
///
/// A pen never gets thicker as the scale gets smaller: a table whose widths grow with the
/// denominator is refused.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(try_from = "RawPenTable", into = "RawPenTable")]
pub struct PenTable {
    scales: Vec<Scale>,
    pens: Vec<Pen>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RawPenTable {
    scales: Vec<Scale>,
    pens: Vec<Pen>,
}

impl TryFrom<RawPenTable> for PenTable {
    type Error = StyleError;

    fn try_from(raw: RawPenTable) -> Result<Self, StyleError> {
        Self::new(raw.scales, raw.pens)
    }
}

impl From<PenTable> for RawPenTable {
    fn from(table: PenTable) -> Self {
        Self {
            scales: table.scales,
            pens: table.pens,
        }
    }
}

impl PenTable {
    /// A table of `pens` for `scales`, checked.
    ///
    /// # Errors
    /// A [`StyleError`] when the scales are empty, too many or not strictly ascending; when the
    /// pens are empty, too many, or their numbers are zero, repeated or out of order; when a pen
    /// has the wrong number of widths, a width above ten millimetres or a width that grows toward
    /// a smaller scale; or when a name is bad or repeated.
    pub fn new(scales: Vec<Scale>, pens: Vec<Pen>) -> Result<Self, StyleError> {
        if scales.is_empty() {
            return Err(StyleError::NoScales);
        }
        if scales.len() > MOST_SCALES {
            return Err(StyleError::TooManyScales);
        }
        if !scales.windows(2).all(|pair| match pair {
            [a, b] => a < b,
            _ => true,
        }) {
            return Err(StyleError::ScalesNotAscending);
        }
        if pens.is_empty() {
            return Err(StyleError::NoPens);
        }
        if pens.len() > MOST_PENS {
            return Err(StyleError::TooManyPens);
        }
        let mut last_number = 0_u16;
        for pen in &pens {
            if pen.number <= last_number {
                return Err(StyleError::PensNotAscending);
            }
            last_number = pen.number;
            check_name(&pen.name)?;
            check_widths(pen.number, scales.len(), &pen.widths)?;
        }
        check_names_differ(pens.iter().map(|pen| pen.name.as_str()))?;
        Ok(Self { scales, pens })
    }

    /// A table this crate builds itself from fixed data that a test checks with [`PenTable::new`].
    pub(crate) fn assemble(scales: Vec<Scale>, pens: Vec<Pen>) -> Self {
        Self { scales, pens }
    }

    /// The scales widths are given for, largest scale first.
    #[must_use]
    pub fn scales(&self) -> &[Scale] {
        &self.scales
    }

    /// The pens, by number.
    #[must_use]
    pub fn pens(&self) -> &[Pen] {
        &self.pens
    }

    /// The pen with this number.
    #[must_use]
    pub fn pen(&self, number: u16) -> Option<&Pen> {
        self.pens.iter().find(|pen| pen.number == number)
    }

    /// The position, among the table's scales, of the row that serves `scale`: see the lookup
    /// rule in the type's documentation.
    #[must_use]
    pub fn row_for(&self, scale: Scale) -> usize {
        // The first listed scale that is the asked scale or coarser. Past the last, the last.
        let past = self.scales.partition_point(|listed| *listed < scale);
        past.min(self.scales.len().saturating_sub(1))
    }

    /// The printed width of pen `number` at `scale`.
    ///
    /// # Errors
    /// [`StyleError::NoSuchPen`] when no pen has that number.
    pub fn width(&self, number: u16, scale: Scale) -> Result<Micrometres, StyleError> {
        let pen = self
            .pen(number)
            .ok_or(StyleError::NoSuchPen { pen: number })?;
        pen.widths
            .get(self.row_for(scale))
            .copied()
            .ok_or(StyleError::NoSuchPen { pen: number })
    }

    /// This table with the widths of pen `number` replaced by `widths` (one for each scale).
    ///
    /// # Errors
    /// [`StyleError::NoSuchPen`] when no pen has that number, or any refusal of [`PenTable::new`]
    /// about the new widths. The table itself is unchanged either way.
    pub fn with_pen_widths(&self, number: u16, widths: &[Micrometres]) -> Result<Self, StyleError> {
        if self.pen(number).is_none() {
            return Err(StyleError::NoSuchPen { pen: number });
        }
        let pens = self
            .pens
            .iter()
            .map(|pen| {
                if pen.number == number {
                    Pen {
                        widths: widths.to_vec(),
                        ..pen.clone()
                    }
                } else {
                    pen.clone()
                }
            })
            .collect();
        Self::new(self.scales.clone(), pens)
    }
}

fn check_widths(pen: u16, scales: usize, widths: &[Micrometres]) -> Result<(), StyleError> {
    if widths.len() != scales {
        return Err(StyleError::WidthCount {
            pen,
            expected: scales,
            found: widths.len(),
        });
    }
    for width in widths {
        check_pen_width(*width)?;
    }
    for (index, pair) in widths.windows(2).enumerate() {
        if let [before, after] = pair
            && after > before
        {
            return Err(StyleError::WidthGrows {
                pen,
                scale_index: index.saturating_add(1),
            });
        }
    }
    Ok(())
}

fn check_names_differ<'a>(names: impl Iterator<Item = &'a str>) -> Result<(), StyleError> {
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        if seen.contains(&name) {
            return Err(StyleError::DuplicateName);
        }
        seen.push(name);
    }
    Ok(())
}

/// A numbered annotation and perspective-line pen: one printed width at every scale.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnnotationPen {
    /// Its number, from 1.
    pub number: u16,
    /// The name people see.
    pub name: String,
    /// The width printed whatever the view's scale.
    pub width: Micrometres,
}

/// The pens for annotation and perspective lines. See the module's documentation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(try_from = "RawAnnotationPens", into = "RawAnnotationPens")]
pub struct AnnotationPens {
    pens: Vec<AnnotationPen>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RawAnnotationPens {
    pens: Vec<AnnotationPen>,
}

impl TryFrom<RawAnnotationPens> for AnnotationPens {
    type Error = StyleError;

    fn try_from(raw: RawAnnotationPens) -> Result<Self, StyleError> {
        Self::new(raw.pens)
    }
}

impl From<AnnotationPens> for RawAnnotationPens {
    fn from(table: AnnotationPens) -> Self {
        Self { pens: table.pens }
    }
}

impl AnnotationPens {
    /// A table of `pens`, checked.
    ///
    /// # Errors
    /// A [`StyleError`] when there are none or too many, their numbers are zero, repeated or out
    /// of order, a width is above ten millimetres, or a name is bad or repeated.
    pub fn new(pens: Vec<AnnotationPen>) -> Result<Self, StyleError> {
        if pens.is_empty() {
            return Err(StyleError::NoPens);
        }
        if pens.len() > MOST_PENS {
            return Err(StyleError::TooManyPens);
        }
        let mut last_number = 0_u16;
        for pen in &pens {
            if pen.number <= last_number {
                return Err(StyleError::PensNotAscending);
            }
            last_number = pen.number;
            check_name(&pen.name)?;
            check_pen_width(pen.width)?;
        }
        check_names_differ(pens.iter().map(|pen| pen.name.as_str()))?;
        Ok(Self { pens })
    }

    /// A table this crate builds itself from fixed data that a test checks with
    /// [`AnnotationPens::new`].
    pub(crate) fn assemble(pens: Vec<AnnotationPen>) -> Self {
        Self { pens }
    }

    /// The pens, by number.
    #[must_use]
    pub fn pens(&self) -> &[AnnotationPen] {
        &self.pens
    }

    /// The printed width of pen `number`. There is one width: the view's scale does not change it.
    ///
    /// # Errors
    /// [`StyleError::NoSuchPen`] when no pen has that number.
    pub fn width(&self, number: u16) -> Result<Micrometres, StyleError> {
        self.pens
            .iter()
            .find(|pen| pen.number == number)
            .map(|pen| pen.width)
            .ok_or(StyleError::NoSuchPen { pen: number })
    }

    /// This table with the width of pen `number` replaced.
    ///
    /// # Errors
    /// [`StyleError::NoSuchPen`] when no pen has that number, or [`StyleError::BadLength`] for a
    /// width above ten millimetres. The table itself is unchanged either way.
    pub fn with_width(&self, number: u16, width: Micrometres) -> Result<Self, StyleError> {
        if self.pens.iter().all(|pen| pen.number != number) {
            return Err(StyleError::NoSuchPen { pen: number });
        }
        let pens = self
            .pens
            .iter()
            .map(|pen| {
                if pen.number == number {
                    AnnotationPen {
                        width,
                        ..pen.clone()
                    }
                } else {
                    pen.clone()
                }
            })
            .collect();
        Self::new(pens)
    }
}
