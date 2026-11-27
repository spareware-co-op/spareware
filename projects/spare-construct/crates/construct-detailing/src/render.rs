//! A view's detail items as what a drawing lays over its model ([`construct_drawing::over::Over`]).
//!
//! What is drawn, in this order (the order of [`Over`] too): the regions by ID (a masking region is
//! an opaque fill in the paper's white and hides what is under it; a filled region is its pattern's
//! lines clipped exactly to its outline, by construct-drawing's hatch), then the lines: each
//! region's boundary, the detail lines, the lines of every component and every repeat, and a
//! mark for every spot slope that can be read. Every line prints at its style's pen for the view's
//! scale. An item whose category the view hides is left out; an item that cannot be drawn is named
//! in [`Rendered::skipped`] with the reason, so one item never blanks the view.
//!
//! A spot slope is drawn as an arrow along the direction water runs, 8 mm of paper long, from its
//! point. Its numbers wait for the text engine (job 7.4): they are in the listing.
//!
//! Line patterns print solid for now, as every line of a drawing does (construct-drawing's SVG
//! header says so).

#![allow(
    clippy::arithmetic_side_effects,
    reason = "the only unchecked operators here are on num-bigint whole numbers of any size, which cannot overflow, with a divisor above zero; every fixed-width step is a checked method"
)]
use crate::component::{Library, Placed};
use crate::detailing::ViewItems;
use crate::line::Curve;
use crate::placed::along;
use crate::region::{Fill, Region};
use crate::slope::{Faces, Reading};
use crate::style::{BuiltIn, OwnStyles, Printed, StyleRef};
use construct_categories::{
    CategoryId, DETAIL_COMPONENTS, DETAIL_LINES, FILLED_REGIONS, MASKING_REGIONS, SPOT_DIMENSIONS,
};
use construct_drawing::drawing::{LineKind, Shape, Turn, ViewPoint};
use construct_drawing::hatch::{FillPatterns, hatch};
use construct_drawing::over::{Over, OverFill, OverHatch};
use construct_drawing::picture::Stroke;
use construct_styles::{Colour, LineStyles, Scale};
use kit_core::Id;
use num_bigint::BigInt;

/// The paper's colour, which a masking region fills with.
pub const PAPER: Colour = Colour(255, 255, 255);
/// How long a spot slope's arrow is on paper, in whole millimetres.
pub const ARROW_MM: i64 = 8;

/// What a drawing is made at: the scale, and the tables its items name things in.
pub struct Settings<'a> {
    /// The view's scale.
    pub scale: Scale,
    /// The project's pens and line patterns.
    pub lines: &'a LineStyles,
    /// The project's fill patterns.
    pub fills: &'a FillPatterns,
    /// The document's own line styles.
    pub own: &'a OwnStyles,
    /// The component library.
    pub library: &'a Library,
    /// Whether the view shows a category (its graphics rules: a category it hides is left out).
    pub shown: &'a dyn Fn(CategoryId) -> bool,
}

/// An item that could not be drawn, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// The item.
    pub item: Id,
    /// The reason, in words.
    pub why: String,
}

/// What [`render`] gives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    /// What to lay over the model.
    pub over: Over,
    /// What was left out.
    pub skipped: Vec<Skipped>,
}

fn view_point(p: [i128; 2]) -> ViewPoint {
    ViewPoint::grid(p[0], p[1])
}

struct Pen<'a> {
    settings: &'a Settings<'a>,
}

impl Pen<'_> {
    fn printed(&self, style: &StyleRef) -> Result<Printed, String> {
        self.settings
            .own
            .printed(style, self.settings.lines, self.settings.scale)
            .map_err(|e| e.to_string())
    }

    fn stroke(
        &self,
        shape: Shape,
        style: &StyleRef,
        item: &Id,
        role: &str,
    ) -> Result<Stroke, String> {
        let p = self.printed(style)?;
        Ok(Stroke {
            shape,
            kind: LineKind::Projected,
            width: p.width,
            colour: p.colour,
            pattern: p.pattern,
            opacity: 100,
            element: item.clone(),
            role: role.to_owned(),
        })
    }

    fn segment(
        &self,
        a: [i128; 2],
        b: [i128; 2],
        style: &StyleRef,
        item: &Id,
        role: &str,
    ) -> Result<Stroke, String> {
        self.stroke(
            Shape::Segment {
                start: view_point(a),
                end: view_point(b),
            },
            style,
            item,
            role,
        )
    }

    /// The pattern lines of `outline` (anticlockwise) in `pattern`'s pen.
    fn pattern(
        &self,
        outline: &[[i128; 2]],
        pattern: &str,
        colour: Colour,
        item: &Id,
    ) -> Result<Option<OverHatch>, String> {
        let found = self
            .settings
            .fills
            .get(pattern)
            .ok_or_else(|| format!("There is no fill pattern {pattern:?}."))?;
        let ring: Vec<ViewPoint> = outline.iter().map(|p| view_point(*p)).collect();
        let lines = hatch(&[ring], found, self.settings.scale).map_err(|e| e.to_string())?;
        if lines.is_empty() {
            return Ok(None);
        }
        let width = self
            .settings
            .lines
            .model_pens
            .width(found.pen, self.settings.scale)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Some(OverHatch {
            lines,
            width,
            colour,
            element: item.clone(),
        }))
    }
}

fn edges(corners: &[[i128; 2]]) -> Vec<([i128; 2], [i128; 2])> {
    corners
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            let b = corners
                .get(i.saturating_add(1))
                .or_else(|| corners.first())?;
            Some((*a, *b))
        })
        .collect()
}

fn shown(settings: &Settings<'_>, category: CategoryId) -> bool {
    (settings.shown)(category)
}

fn draw_region(pen: &Pen<'_>, region: &Region, out: &mut Over) -> Result<(), String> {
    let ring = region.anticlockwise();
    match region.fill() {
        Fill::Masking => out.fills.push(OverFill {
            outline: ring.iter().map(|p| view_point(*p)).collect(),
            colour: PAPER,
            element: region.id().clone(),
        }),
        Fill::Pattern { pattern, colour } => {
            if let Some(found) = pen.pattern(&ring, pattern, *colour, region.id())? {
                out.hatches.push(found);
            }
        }
    }
    Ok(())
}

fn draw_placed(pen: &Pen<'_>, placed: &Placed, item: &Id, out: &mut Over) -> Result<(), String> {
    for fill in &placed.fills {
        let mut ring: Vec<[i128; 2]> = fill
            .outline
            .iter()
            .map(|p| [i128::from(p[0]), i128::from(p[1])])
            .collect();
        // A turned or mirrored outline may run clockwise: hatch wants anticlockwise.
        let twice: i128 = ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(a, b)| {
                a[0].saturating_mul(b[1])
                    .saturating_sub(a[1].saturating_mul(b[0]))
            })
            .fold(0, i128::saturating_add);
        if twice < 0 {
            ring.reverse();
        }
        if let Some(found) = pen.pattern(&ring, &fill.pattern, fill.colour, item)? {
            out.hatches.push(found);
        }
    }
    Ok(())
}

fn placed_lines(
    pen: &Pen<'_>,
    placed: &Placed,
    item: &Id,
    strokes: &mut Vec<Stroke>,
) -> Result<(), String> {
    for line in &placed.lines {
        let at = |p: [i64; 2]| [i128::from(p[0]), i128::from(p[1])];
        strokes.push(pen.segment(
            at(line.from),
            at(line.to),
            &StyleRef::Built(line.style),
            item,
            "component",
        )?);
    }
    Ok(())
}

/// The arrow of a spot slope: its shaft along the way water runs from `at`, and two barbs.
fn arrow(
    pen: &Pen<'_>,
    at: [i128; 2],
    reading: &Reading,
    item: &Id,
    scale: Scale,
) -> Result<Vec<Stroke>, String> {
    let [fx, fy] = reading.fall;
    let n = BigInt::from(fx) * BigInt::from(fx) + BigInt::from(fy) * BigInt::from(fy);
    // The arrow is ARROW_MM of paper: ARROW_MM x the scale's denominator millimetres of the model.
    let length = BigInt::from(ARROW_MM)
        * BigInt::from(scale.denominator())
        * BigInt::from(construct_params::Length::STEPS_PER_MM);
    let step = |k: &BigInt, d: i128| -> Result<i128, String> {
        along(k, d, &n).map(i128::from).map_err(|e| e.to_string())
    };
    let tip = [
        at[0].saturating_add(step(&length, fx)?),
        at[1].saturating_add(step(&length, fy)?),
    ];
    // The barbs: a quarter of the shaft back, an eighth of it to each side.
    let back = &length / 4;
    let side = &length / 8;
    let barb = |sign: i128| -> Result<[i128; 2], String> {
        let s = |v: i128| v.saturating_mul(sign);
        Ok([
            tip[0]
                .saturating_sub(step(&back, fx)?)
                .saturating_add(s(step(&side, fy.saturating_neg())?)),
            tip[1]
                .saturating_sub(step(&back, fy)?)
                .saturating_add(s(step(&side, fx)?)),
        ])
    };
    let style = StyleRef::Built(BuiltIn::Standard);
    Ok(vec![
        pen.segment(at, tip, &style, item, "slope")?,
        pen.segment(tip, barb(1)?, &style, item, "slope")?,
        pen.segment(tip, barb(-1)?, &style, item, "slope")?,
    ])
}

/// What is being drawn: the three layers of [`Over`], in the order they are laid, and what was
/// left out.
#[derive(Default)]
struct Acc {
    fills: Vec<OverFill>,
    hatches: Vec<OverHatch>,
    strokes: Vec<Stroke>,
    skipped: Vec<Skipped>,
}

impl Acc {
    fn skip(&mut self, item: &Id, why: String) {
        self.skipped.push(Skipped {
            item: item.clone(),
            why,
        });
    }

    fn regions(&mut self, pen: &Pen<'_>, items: &ViewItems<'_>) {
        for region in &items.regions {
            let category = if region.is_masking() {
                MASKING_REGIONS
            } else {
                FILLED_REGIONS
            };
            if !shown(pen.settings, category) {
                continue;
            }
            let mut one = Over::none();
            if let Err(why) = draw_region(pen, region, &mut one) {
                self.skip(region.id(), why);
                continue;
            }
            self.fills.extend(one.fills);
            self.hatches.extend(one.hatches);
            if let Some(style) = region.boundary() {
                let drawn: Result<Vec<Stroke>, String> = edges(&region.anticlockwise())
                    .into_iter()
                    .map(|(a, b)| pen.segment(a, b, style, region.id(), "boundary"))
                    .collect();
                match drawn {
                    Ok(found) => self.strokes.extend(found),
                    Err(why) => self.skip(region.id(), why),
                }
            }
        }
    }

    fn lines(&mut self, pen: &Pen<'_>, items: &ViewItems<'_>) {
        if !shown(pen.settings, DETAIL_LINES) {
            return;
        }
        for line in &items.lines {
            let at = |p| view_point(crate::num::at(p));
            let shape = match *line.curve() {
                Curve::Straight { start, end } => Shape::Segment {
                    start: at(start),
                    end: at(end),
                },
                Curve::Arc { centre, start, end } => Shape::Arc {
                    centre: at(centre),
                    start: at(start),
                    end: at(end),
                    turn: Turn::Counterclockwise,
                },
            };
            match pen.stroke(shape, line.style(), line.id(), "detail line") {
                Ok(stroke) => self.strokes.push(stroke),
                Err(why) => self.skip(line.id(), why),
            }
        }
    }

    /// Lays `placed` (one component, or every copy of a repeat) over the view.
    fn placed(&mut self, pen: &Pen<'_>, item: &Id, placed: &[Placed]) {
        let mut over = Over::none();
        let mut strokes = Vec::new();
        let drawn = placed.iter().try_for_each(|one| {
            draw_placed(pen, one, item, &mut over)?;
            placed_lines(pen, one, item, &mut strokes)
        });
        match drawn {
            Ok(()) => {
                self.hatches.extend(over.hatches);
                self.strokes.extend(strokes);
            }
            Err(why) => self.skip(item, why),
        }
    }

    fn components(&mut self, pen: &Pen<'_>, items: &ViewItems<'_>) {
        if !shown(pen.settings, DETAIL_COMPONENTS) {
            return;
        }
        let library = pen.settings.library;
        for item in &items.components {
            let drawn = library
                .get(item.recipe())
                .ok_or_else(|| format!("There is no detail component {:?}.", item.recipe()))
                .and_then(|r| item.draw(r).map_err(|e| e.to_string()));
            match drawn {
                Ok(placed) => self.placed(pen, item.id(), &[placed]),
                Err(why) => self.skip(item.id(), why),
            }
        }
        for item in &items.repeats {
            match item.draw(library) {
                Ok(placed) => self.placed(pen, item.id(), &placed),
                Err(e) => self.skip(item.id(), e.to_string()),
            }
        }
    }

    fn slopes(&mut self, pen: &Pen<'_>, items: &ViewItems<'_>, faces: &dyn Faces) {
        if !shown(pen.settings, SPOT_DIMENSIONS) {
            return;
        }
        for item in &items.slopes {
            let drawn = item.read(faces).map_err(|e| e.to_string()).and_then(|r| {
                arrow(
                    pen,
                    crate::num::at(item.at()),
                    &r,
                    item.id(),
                    pen.settings.scale,
                )
            });
            match drawn {
                Ok(found) => self.strokes.extend(found),
                Err(why) => self.skip(item.id(), why),
            }
        }
    }
}

/// The view's detail items as what lies over its model.
#[must_use]
pub fn render(items: &ViewItems<'_>, settings: &Settings<'_>, faces: &dyn Faces) -> Rendered {
    let pen = Pen { settings };
    let mut acc = Acc::default();
    acc.regions(&pen, items);
    acc.lines(&pen, items);
    acc.components(&pen, items);
    acc.slopes(&pen, items, faces);
    Rendered {
        over: Over {
            fills: acc.fills,
            hatches: acc.hatches,
            strokes: acc.strokes,
        },
        skipped: acc.skipped,
    }
}
