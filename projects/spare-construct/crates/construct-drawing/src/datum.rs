//! Levels and grids in a section or an elevation: a line where each crosses the view, and its name
//! (job 8.1).
//!
//! What it does: [`marks`] gives the printed strokes and the names of the levels and grids a
//! section or an elevation shows, inside a frame (the box the view's drawing fills, [`frame`], or
//! the crop's box when the crop is on), as a person drawing one expects:
//!
//! - **A level** is a horizontal line across the frame at its height, its name written just past
//!   the frame's right end, on the line. A level outside the frame's heights is not shown.
//! - **A grid** (straight) is shown when its line in plan meets the part of the plan the view sees
//!   (its line's two ends across, and its depth) and is not parallel to the view's plane. It is a
//!   vertical line from the frame's bottom to its top, where the grid's line, run on as far as it
//!   needs, crosses the view's plane, with a circle (a bubble) standing on its top and its name in
//!   the circle. A grid that crosses the plane outside the frame's width is not shown.
//! - **An arc grid** is not shown yet, and is named in [`Marked::skipped`] with the reason: where
//!   its circle crosses the view's plane has a square root in it and is not a fraction of the grid.
//!
//! Lines are penned as their category's projected edges (the view's rules where it has them,
//! through [`crate::pens::style`], a hidden category left out); names are [`Label`]s, written at a
//! printed height of [`LABEL_HEIGHT`] (2.5 mm, one of the heights ISO 3098-0 lists for lettering on
//! technical drawings) in the line's colour. The sizes of the marks are our own: a bubble's radius
//! is [`BUBBLE_RADIUS_MM`] of paper, a level's name stands [`LABEL_GAP_MM`] of paper past its line.
//! Marks are not stored in a drawing and are not clipped by a crop: they are drawn over the picture
//! once it is cropped, inside the frame.
//!
//! Not yet: a datum's own extent per view (a grid drawn shorter than the frame, a level's head at
//! either end or both), the level's height written beside its name, and the elevation and section
//! marks of other views (job 8.4).

use construct_categories::CategoryId;
use construct_params::Length;
use construct_styles::{Colour, Micrometres, Scale};
use kit_core::Id;

use crate::DrawingError;
use crate::drawing::{Drawing, Entity, Line, LineKind, Shape, Source, Turn, ViewPoint};
use crate::look::opacity;
use crate::overlay::Q;
use crate::pens::{Styles, look_of, style};
use crate::picture::Stroke;
use crate::section::SectionView;
use crate::svg::PER_MM;
use crate::view::Looking;

/// The printed height of a datum's name: 2.5 mm, in micrometres.
pub const LABEL_HEIGHT: u32 = 2_500;
/// A grid bubble's printed radius, in millimetres of paper.
pub const BUBBLE_RADIUS_MM: i128 = 4;
/// How far past a level's line its name stands, in millimetres of paper.
pub const LABEL_GAP_MM: i128 = 2;

/// Where a datum is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatumAt {
    /// A level, at its height above the project's zero.
    Level {
        /// Its height.
        height: Length,
    },
    /// A straight grid line in plan, from one end to the other (each the project's x and y).
    Grid {
        /// Its first end.
        from: [Length; 2],
        /// Its second end.
        to: [Length; 2],
    },
    /// An arc grid: not shown in a section or an elevation yet.
    ArcGrid,
}

/// A level or a grid, as a section or an elevation draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datum {
    /// Its permanent ID.
    pub id: Id,
    /// Its category: levels or grids, which its line is penned by.
    pub category: CategoryId,
    /// The name people see.
    pub name: String,
    /// Where it is.
    pub at: DatumAt,
}

/// Where a label's point is along its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Anchor {
    /// The text starts at the point, on its baseline.
    Start,
    /// The text is centred on the point, across and up.
    Middle,
}

/// A name written on a drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// The words.
    pub text: String,
    /// Where it is written, in view coordinates.
    pub at: ViewPoint,
    /// How the words stand on that point.
    pub anchor: Anchor,
    /// The printed height of the letters.
    pub height: Micrometres,
    /// Their colour.
    pub colour: Colour,
    /// How much of full strength it prints at, in whole percent.
    pub opacity: u8,
    /// The element it names.
    pub element: Id,
    /// What it is: `level` or `grid`.
    pub role: String,
}

/// The marks of a view's levels and grids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marked {
    /// The lines and bubbles, in the order of the datums given.
    pub strokes: Vec<Stroke>,
    /// The names, in the same order.
    pub labels: Vec<Label>,
    /// The datums not shown that would have been, each with the reason.
    pub skipped: Vec<(Id, String)>,
}

fn whole(v: i128) -> Q {
    Q::whole(v)
}

fn steps(length: Length) -> i128 {
    i128::from(length.steps())
}

/// The box `drawing` fills across the view's width: from one end of its line to the other, and from
/// the lowest of its points and of `datums`' levels to the highest. `None` when there is nothing to
/// measure the heights by.
///
/// # Errors
///
/// [`DrawingError::TooBig`] for a number past 128 bits.
pub fn frame(
    view: &SectionView,
    drawing: &Drawing,
    datums: &[Datum],
) -> Result<Option<[ViewPoint; 2]>, DrawingError> {
    let mut heights: Vec<Q> = Vec::new();
    for line in drawing.lines() {
        let (Shape::Segment { start, end } | Shape::Arc { start, end, .. }) = line.shape;
        heights.push(Q::fraction(start.y, start.den)?);
        heights.push(Q::fraction(end.y, end.den)?);
    }
    for region in drawing.regions() {
        for p in region.loops.iter().flatten() {
            heights.push(Q::fraction(p.y, p.den)?);
        }
    }
    for datum in datums {
        if let DatumAt::Level { height } = datum.at {
            heights.push(whole(steps(height)));
        }
    }
    let (Some(lo), Some(hi)) = (heights.iter().min(), heights.iter().max()) else {
        return Ok(None);
    };
    let [x0, x1] = view.ends()?;
    Ok(Some([Q::point(&whole(x0), lo)?, Q::point(&whole(x1), hi)?]))
}

/// Whether the segment from `p` to `q` meets the closed box from `lo` to `hi` (each the plan's x
/// and y), exactly.
fn meets(p: [i128; 2], q: [i128; 2], lo: [i128; 2], hi: [i128; 2]) -> Result<bool, DrawingError> {
    // The part of [0, 1] where p + t (q - p) is inside both slabs.
    let (mut t0, mut t1) = (whole(0), whole(1));
    for ((a, b), (l, h)) in p.into_iter().zip(q).zip(lo.into_iter().zip(hi)) {
        let d = b.checked_sub(a).ok_or(DrawingError::TooBig)?;
        if d == 0 {
            if a < l || a > h {
                return Ok(false);
            }
            continue;
        }
        let at = |v: i128| -> Result<Q, DrawingError> {
            Q::fraction(v.checked_sub(a).ok_or(DrawingError::TooBig)?, d)
        };
        let (e0, e1) = (at(l)?, at(h)?);
        let (enter, leave) = if e0 <= e1 { (e0, e1) } else { (e1, e0) };
        t0 = t0.max(enter);
        t1 = t1.min(leave);
        if t0 > t1 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The drawing's x where the grid from `from` to `to` crosses the view's plane, if it is shown.
fn crossing(
    view: &SectionView,
    from: [Length; 2],
    to: [Length; 2],
) -> Result<Option<Q>, DrawingError> {
    let looking = view.view().looking;
    let (p, q) = (from.map(steps), to.map(steps));
    // The plan's axis across the view (along its line) and the axis it looks along.
    let (across, deep) = match looking {
        Looking::North | Looking::South => (0, 1),
        Looking::East | Looking::West | Looking::Down => (1, 0),
    };
    let pick = |v: [i128; 2], k: usize| v.get(k).copied().ok_or(DrawingError::TooBig);
    let (pa, pd, qa, qd) = (
        pick(p, across)?,
        pick(p, deep)?,
        pick(q, across)?,
        pick(q, deep)?,
    );
    if pd == qd {
        // Parallel to the plane.
        return Ok(None);
    }
    let ([s0, s1], [b0, b1]) = (view.span()?, view.band()?);
    let (mut lo, mut hi) = ([0_i128; 2], [0_i128; 2]);
    for (k, (l, h)) in [(across, (s0, s1)), (deep, (b0, b1))] {
        if let (Some(lo_k), Some(hi_k)) = (lo.get_mut(k), hi.get_mut(k)) {
            *lo_k = l;
            *hi_k = h;
        }
    }
    if !meets(p, q, lo, hi)? {
        return Ok(None);
    }
    let cut = steps(view.view().cut);
    // pa + (cut - pd) (qa - pa) / (qd - pd), the plan's coordinate across.
    let at = whole(pa).plus(
        &whole(cut.checked_sub(pd).ok_or(DrawingError::TooBig)?)
            .times(&whole(qa.checked_sub(pa).ok_or(DrawingError::TooBig)?))
            .over(&whole(qd.checked_sub(pd).ok_or(DrawingError::TooBig)?))?,
    );
    let [rx, ry, _] = looking.right();
    let sign = rx.checked_add(ry).ok_or(DrawingError::TooBig)?;
    Ok(Some(at.times(&whole(sign))))
}

/// A length of paper at the scale, in the model's grid steps.
fn paper(mm: i128, scale: Scale) -> Result<i128, DrawingError> {
    mm.checked_mul(i128::from(scale.denominator()))
        .and_then(|v| v.checked_mul(PER_MM))
        .ok_or(DrawingError::TooBig)
}

fn line_of(datum: &Datum, role: &str, shape: Shape) -> Line {
    Line {
        kind: LineKind::Projected,
        source: Source {
            element: datum.id.clone(),
            entity: Entity::Edge,
            name: datum.name.clone(),
            role: role.to_owned(),
        },
        category: datum.category,
        shape,
    }
}

/// The marks of `datums` in `view`, inside `frame` (its least and greatest corners), printed at
/// `scale` with `styles`.
///
/// # Errors
///
/// What [`crate::pens::style`] refuses for a datum's category; [`DrawingError::TooBig`] for a
/// number past 128 bits; [`DrawingError::BadShape`] for a frame that is not a box.
pub fn marks(
    view: &SectionView,
    datums: &[Datum],
    frame: [ViewPoint; 2],
    styles: Styles<'_>,
    scale: Scale,
) -> Result<Marked, DrawingError> {
    let [lo, hi] = frame;
    let x = |p: ViewPoint| Q::fraction(p.x, p.den);
    let y = |p: ViewPoint| Q::fraction(p.y, p.den);
    let (x0, x1, y0, y1) = (x(lo)?, x(hi)?, y(lo)?, y(hi)?);
    if x0 > x1 || y0 > y1 {
        return Err(DrawingError::BadShape(
            "a frame whose corners are not in order".to_owned(),
        ));
    }
    let height =
        Micrometres::new(LABEL_HEIGHT).map_err(|e| DrawingError::Style(format!("{e:?}")))?;
    let radius = whole(paper(BUBBLE_RADIUS_MM, scale)?);
    let gap = whole(paper(LABEL_GAP_MM, scale)?);
    let mut out = Marked::default();
    for datum in datums {
        let (role, segment, label) = match &datum.at {
            DatumAt::ArcGrid => {
                out.skipped.push((
                    datum.id.clone(),
                    "an arc grid is not shown in a section or an elevation yet: where its circle \
                     crosses the view's plane is not a fraction of the grid"
                        .to_owned(),
                ));
                continue;
            }
            DatumAt::Level { height: h } => {
                let h = whole(steps(*h));
                if h < y0 || h > y1 {
                    continue;
                }
                let start = Q::point(&x0, &h)?;
                let end = Q::point(&x1, &h)?;
                let at = Q::point(&x1.plus(&gap), &h)?;
                ("level", Shape::Segment { start, end }, (at, Anchor::Start))
            }
            DatumAt::Grid { from, to } => {
                let Some(gx) = crossing(view, *from, *to)? else {
                    continue;
                };
                if gx < x0 || gx > x1 {
                    continue;
                }
                let start = Q::point(&gx, &y0)?;
                let end = Q::point(&gx, &y1)?;
                let centre = Q::point(&gx, &y1.plus(&radius))?;
                (
                    "grid",
                    Shape::Segment { start, end },
                    (centre, Anchor::Middle),
                )
            }
        };
        let line = line_of(datum, role, segment);
        let look = look_of(styles, &line)?;
        if look.as_ref().is_some_and(|l| !l.shown) {
            continue;
        }
        let pen = style(styles, &line, scale)?;
        let tone = look.as_ref().map_or(100, opacity);
        let stroke = |shape: Shape| Stroke {
            shape,
            kind: LineKind::Projected,
            width: pen.width,
            colour: pen.colour,
            pattern: pen.pattern,
            opacity: tone,
            element: datum.id.clone(),
            role: role.to_owned(),
        };
        out.strokes.push(stroke(line.shape));
        if let (Anchor::Middle, Shape::Segment { start, .. }) = (label.1, line.shape) {
            // The bubble: a circle standing on the line's top end, as two half turns.
            let (cx, cy) = (Q::fraction(start.x, start.den)?, y1.plus(&radius));
            let right = Q::point(&cx.plus(&radius), &cy)?;
            let left = Q::point(&cx.minus(&radius), &cy)?;
            let centre = Q::point(&cx, &cy)?;
            for (a, b) in [(right, left), (left, right)] {
                out.strokes.push(stroke(Shape::Arc {
                    centre,
                    start: a,
                    end: b,
                    turn: Turn::Counterclockwise,
                }));
            }
        }
        out.labels.push(Label {
            text: datum.name.clone(),
            at: label.0,
            anchor: label.1,
            height,
            colour: pen.colour,
            opacity: tone,
            element: datum.id.clone(),
            role: role.to_owned(),
        });
    }
    Ok(out)
}
