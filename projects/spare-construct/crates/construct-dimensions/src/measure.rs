//! Working a dimension out: each value, its witness lines, the figure it is drawn as and where its
//! text sits, from its references' places, every time it is asked.
//!
//! A linear dimension: each reference meets the dimension's offset line at a place
//! ([`crate::exact`]); a segment's value is the distance between two neighbours' places along it,
//! the nearest whole grid step of an exact square root, with `exact` saying whether the true value
//! is that step. For a horizontal or a vertical dimension the offset line has a unit direction, so
//! the value is a difference of two exact fractions and is exact whenever the references are on
//! whole steps. An ordinate dimension reads the same places, each value the signed distance of a
//! reference's place from the first's, in the line's direction.
//!
//! An angular dimension: the angle between the rays it names along its two lines, from 0 to 180
//! degrees, worked out by [`crate::trig`]: exact for a multiple of 45 degrees, otherwise the
//! nearest whole angle unit (within half a unit and 2^-80 of one). It is drawn as an arc about the
//! point where the lines meet. A radial or diameter dimension: its curve's radius or twice it,
//! exact (a radius is a whole number of steps). An arc length dimension: its arc's radius times
//! its sweep, the nearest step (within half a step and 2^-70 of one; never exact, pi being
//! irrational).
//!
//! A reference that cannot be found, or is not the shape its dimension needs (a curve for a line
//! dimension, a straight line for a radius), gives a [`DimensionWarning`] naming it and no value
//! for what depends on it: never a wrong number. The points a dimension is drawn through (an arc's
//! ends and middle, a leader's ends) are each within a step of the true point.

use crate::dimension::{Dimension, Kind};
use crate::exact::{Axis, Frac, round_root};
use crate::layout::{Direction, Layout, Ray};
use crate::locate::{Curve, Locator, Locus, Missing, Sweep};
use crate::trig;
use crate::warning::{
    DimensionWarning, OUT_OF_RANGE, REFERENCE_MISSING, REFERENCE_PARALLEL, Severity,
};
use construct_grids::Point;
use construct_params::{Angle, Length};
use kit_core::Id;

/// One value of a dimension: the stretch between two neighbouring references (linear), from the
/// first reference to another (ordinate), between its two lines (angular), or across its curve
/// (radial, diameter and arc length: `from` and `to` are both 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// The reference it starts at, from 0.
    pub from: usize,
    /// The reference it ends at: `from + 1` for a linear dimension.
    pub to: usize,
    /// Its value as a length, the nearest whole step to the true one; `None` when a reference it
    /// depends on cannot be used, and for an angle (`angle`). An ordinate value is signed: below
    /// zero against the line's direction.
    pub value: Option<Length>,
    /// An angular dimension's value, the nearest whole angle unit to the true one; `None` for
    /// every other kind, and when a reference cannot be used.
    pub angle: Option<Angle>,
    /// Whether the value is the true value, to the step or the unit.
    pub exact: bool,
    /// Where its text is centred: the segment's middle on the offset line, a reference's place
    /// on it (ordinate), the middle of an angle's or an arc's arc, or of a radius's line.
    pub middle: Option<Point>,
}

/// A witness line: from the reference, drawn at `from`, to where it meets the dimension's line or
/// arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Witness {
    /// The reference it belongs to, from 0.
    pub reference: usize,
    /// Where it starts: a point of the reference as drawn.
    pub from: Point,
    /// Where it ends: on the dimension's line or arc, to the nearest step.
    pub to: Point,
}

/// What a dimension is drawn as, beside its witness lines and text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Figure {
    /// A linear or an ordinate dimension: its offset line, through its witness lines' ends.
    Line,
    /// An angular or an arc length dimension: an arc of `radius` about `centre`, from `from` to
    /// `to`, anticlockwise or not, through `middle`.
    Arc {
        /// The arc's centre: where an angle's lines meet, or the measured arc's centre.
        centre: Point,
        /// Its radius.
        radius: Length,
        /// Where it starts.
        from: Point,
        /// Where it finishes.
        to: Point,
        /// Its middle, where the text is.
        middle: Point,
        /// Whether it runs anticlockwise from `from` to `to`.
        anticlockwise: bool,
        /// Whether it sweeps more than half a turn.
        large: bool,
    },
    /// A radial or a diameter dimension: a line from `from` (the centre, or the far side of the
    /// circle for a diameter) to `to`, on the curve.
    Leader {
        /// The curve's centre.
        centre: Point,
        /// Where the line starts.
        from: Point,
        /// Where it ends, on the curve, with its mark.
        to: Point,
    },
    /// Nothing can be drawn: a reference it needs cannot be used.
    Nothing,
}

/// Whether segments held equal are equal now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EqualState {
    /// The segments, from 0, as stored.
    pub segments: Vec<usize>,
    /// `Some(true)` when their true values are the same, `Some(false)` when they are not, `None`
    /// when one cannot be worked out.
    pub equal: Option<bool>,
    /// `Some(true)` when their values, each the nearest whole step, are within one step of each
    /// other: equal to the grid, as made equal by moving their references (a length that does
    /// not divide into whole steps shares out to within one); `None` when one cannot be worked
    /// out.
    pub holds: Option<bool>,
}

/// A dimension worked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measured {
    /// The dimension.
    pub dimension: Id,
    /// What it measures.
    pub kind: Kind,
    /// Its values, in order.
    pub segments: Vec<Segment>,
    /// A witness line for each reference, in order; `None` where it cannot be drawn. An arc length
    /// dimension has two, both of its one reference: one at each end of its arc.
    pub witnesses: Vec<Option<Witness>>,
    /// What it is drawn as.
    pub figure: Figure,
    /// The distance from the first reference to the last of a linear dimension, when every
    /// reference can be used.
    pub overall: Option<Length>,
    /// Each stored equality and whether it holds.
    pub equalities: Vec<EqualState>,
    /// The references that could not be used (gone, no place in the view, along the offset line,
    /// too far, or not the shape needed), by position from 0, ascending: values that depend on
    /// one are not given.
    pub unusable: Vec<usize>,
    /// What went wrong, by reference, in order.
    pub warnings: Vec<DimensionWarning>,
}

fn point(at: [i64; 2]) -> Point {
    let [x, y] = at;
    Point::new(Length::from_steps(x), Length::from_steps(y))
}

pub(crate) fn steps(at: Point) -> [i64; 2] {
    [at.x.steps(), at.y.steps()]
}

pub(crate) fn warn(
    dimension: &Dimension,
    kind: &'static str,
    at: usize,
    words: &str,
) -> DimensionWarning {
    let reference = dimension.references().get(at);
    let mut elements = vec![dimension.id().clone()];
    if let Some(reference) = reference {
        elements.push(reference.element().clone());
    }
    DimensionWarning {
        kind,
        severity: Severity::Warning,
        elements,
        words: format!(
            "Dimension {}: reference {} ({}) {words}",
            dimension.id(),
            at.saturating_add(1),
            reference.map_or_else(String::new, crate::Reference::describe),
        ),
    }
}

/// The warning for a reference the locator cannot find.
fn missing(dimension: &Dimension, at: usize, Missing { detail, .. }: &Missing) -> DimensionWarning {
    warn(
        dimension,
        REFERENCE_MISSING,
        at,
        &format!("cannot be found: {detail}."),
    )
}

/// The warning for a curve where a straight line is needed.
fn curved(dimension: &Dimension, at: usize) -> DimensionWarning {
    let noun = dimension
        .references()
        .get(at)
        .map_or("reference", |reference| reference.feature().noun());
    warn(
        dimension,
        REFERENCE_MISSING,
        at,
        &format!("cannot be found: a curved {noun} has no straight line to measure to."),
    )
}

/// Where reference `at` meets the offset line, as a parameter along it, and its witness line; or
/// a warning pushed and neither.
pub(crate) fn place(
    dimension: &Dimension,
    locator: &dyn Locator,
    axis: &Axis,
    at: usize,
    warnings: &mut Vec<DimensionWarning>,
) -> (Option<Frac>, Option<Witness>) {
    let Some(reference) = dimension.references().get(at) else {
        return (None, None);
    };
    let found = match locator.locate(reference) {
        Ok(Locus::Point(q)) => Some((axis.of_point(steps(q)), q)),
        Ok(Locus::Line { through, direction }) => {
            let meeting = axis.of_line(steps(through), direction);
            if meeting.is_none() {
                warnings.push(warn(
                    dimension,
                    REFERENCE_PARALLEL,
                    at,
                    "runs along the dimension's line, so it has no place along it.",
                ));
            }
            meeting.map(|s| (s, through))
        }
        Ok(Locus::Arc(_)) => {
            warnings.push(curved(dimension, at));
            None
        }
        Err(why) => {
            warnings.push(missing(dimension, at, &why));
            None
        }
    };
    let Some((s, anchor)) = found else {
        return (None, None);
    };
    if let Some(on_line) = axis.point_at(&s) {
        let witness = Witness {
            reference: at,
            from: anchor,
            to: point(on_line),
        };
        (Some(s), Some(witness))
    } else {
        warnings.push(warn(
            dimension,
            OUT_OF_RANGE,
            at,
            "meets the dimension's line beyond the kernel's grid.",
        ));
        (None, None)
    }
}

/// The segments between neighbouring places, and each one's exact square (to compare by).
fn segments_of(
    dimension: &Dimension,
    axis: &Axis,
    places: &[Option<Frac>],
    warnings: &mut Vec<DimensionWarning>,
) -> (Vec<Segment>, Vec<Option<Frac>>) {
    let mut squares = Vec::new();
    let mut segments = Vec::new();
    for (from, pair) in places.windows(2).enumerate() {
        let to = from.saturating_add(1);
        let a = pair.first().and_then(Option::as_ref);
        let b = pair.get(1).and_then(Option::as_ref);
        let square = a.zip(b).map(|(a, b)| axis.squared_between(a, b));
        let rounded = square.as_ref().and_then(round_root);
        if square.is_some() && rounded.is_none() {
            warnings.push(warn(
                dimension,
                OUT_OF_RANGE,
                to,
                "is so far from the one before it that the value does not fit the kernel's grid.",
            ));
        }
        let middle = a
            .zip(b)
            .and_then(|(a, b)| axis.point_between(a, b))
            .map(point);
        segments.push(Segment {
            from,
            to,
            value: rounded.map(|(value, _)| Length::from_steps(value)),
            angle: None,
            exact: rounded.is_some_and(|(_, whole)| whole),
            middle,
        });
        squares.push(square);
    }
    (segments, squares)
}

/// The values of an ordinate dimension: each reference's signed distance from the first.
fn ordinates_of(
    dimension: &Dimension,
    axis: &Axis,
    places: &[Option<Frac>],
    warnings: &mut Vec<DimensionWarning>,
) -> Vec<Segment> {
    let origin = places.first().and_then(Option::as_ref);
    let mut segments = Vec::new();
    for (to, here) in places.iter().enumerate().skip(1) {
        let here = here.as_ref();
        let rounded = origin.zip(here).and_then(|(o, h)| {
            let distance = round_root(&axis.squared_between(o, h));
            if distance.is_none() {
                warnings.push(warn(
                    dimension,
                    OUT_OF_RANGE,
                    to,
                    "is so far from the first that the value does not fit the kernel's grid.",
                ));
            }
            distance.map(|(steps, whole)| {
                (
                    if h < o {
                        steps.checked_neg()
                    } else {
                        Some(steps)
                    },
                    whole,
                )
            })
        });
        let value = rounded.and_then(|(steps, _)| steps).map(Length::from_steps);
        segments.push(Segment {
            from: 0,
            to,
            value,
            angle: None,
            exact: value.is_some() && rounded.is_some_and(|(_, whole)| whole),
            middle: here.and_then(|h| axis.point_at(h)).map(point),
        });
    }
    segments
}

fn unusable_of(places: &[bool]) -> Vec<usize> {
    places
        .iter()
        .enumerate()
        .filter(|(_, usable)| !**usable)
        .map(|(at, _)| at)
        .collect()
}

/// A linear or ordinate dimension.
fn measure_line(dimension: &Dimension, locator: &dyn Locator) -> Measured {
    let (origin, d) = dimension.layout().line();
    let axis = Axis {
        origin: steps(origin),
        d,
    };
    let mut warnings = Vec::new();
    let mut places: Vec<Option<Frac>> = Vec::new();
    let mut witnesses: Vec<Option<Witness>> = Vec::new();
    for at in 0..dimension.references().len() {
        let (found, witness) = place(dimension, locator, &axis, at, &mut warnings);
        places.push(found);
        witnesses.push(witness);
    }
    let unusable = unusable_of(&places.iter().map(Option::is_some).collect::<Vec<_>>());
    if dimension.is_ordinate() {
        let segments = ordinates_of(dimension, &axis, &places, &mut warnings);
        return Measured {
            dimension: dimension.id().clone(),
            kind: dimension.kind(),
            segments,
            witnesses,
            figure: Figure::Line,
            overall: None,
            equalities: Vec::new(),
            unusable,
            warnings,
        };
    }
    let (segments, squares) = segments_of(dimension, &axis, &places, &mut warnings);
    let overall = match (
        places.first().and_then(Option::as_ref),
        places.last().and_then(Option::as_ref),
        places.iter().all(Option::is_some),
    ) {
        (Some(first), Some(last), true) => round_root(&axis.squared_between(first, last))
            .map(|(value, _)| Length::from_steps(value)),
        _ => None,
    };
    let equalities = dimension
        .equalities()
        .iter()
        .map(|group| {
            let known: Option<Vec<&Frac>> = group
                .iter()
                .map(|&segment| squares.get(segment).and_then(Option::as_ref))
                .collect();
            let values: Option<Vec<i64>> = group
                .iter()
                .map(|&segment| {
                    segments
                        .get(segment)
                        .and_then(|s| s.value)
                        .map(Length::steps)
                })
                .collect();
            EqualState {
                segments: group.clone(),
                equal: known.map(|known| known.windows(2).all(|pair| pair.first() == pair.get(1))),
                holds: values.map(|values| {
                    let low = values.iter().min();
                    let high = values.iter().max();
                    low.zip(high)
                        .is_some_and(|(low, high)| high.saturating_sub(*low) <= 1)
                }),
            }
        })
        .collect();
    Measured {
        dimension: dimension.id().clone(),
        kind: dimension.kind(),
        segments,
        witnesses,
        figure: Figure::Line,
        overall,
        unusable,
        equalities,
        warnings,
    }
}

/// A dimension whose figure cannot be drawn and whose one value is unknown, with what went wrong.
fn unmeasured(
    dimension: &Dimension,
    unusable: Vec<usize>,
    warnings: Vec<DimensionWarning>,
) -> Measured {
    let to = usize::from(dimension.kind() == Kind::Angular);
    Measured {
        dimension: dimension.id().clone(),
        kind: dimension.kind(),
        segments: vec![Segment {
            from: 0,
            to,
            value: None,
            angle: None,
            exact: false,
            middle: None,
        }],
        witnesses: vec![None; dimension.references().len()],
        figure: Figure::Nothing,
        overall: None,
        equalities: Vec::new(),
        unusable,
        warnings,
    }
}

/// The straight line reference `at` is, or a warning.
fn line_of(
    dimension: &Dimension,
    locator: &dyn Locator,
    at: usize,
) -> Result<(Point, [i64; 2]), DimensionWarning> {
    let Some(reference) = dimension.references().get(at) else {
        return Err(warn(dimension, REFERENCE_MISSING, at, "is not there."));
    };
    match locator.locate(reference) {
        Ok(Locus::Line { through, direction }) => Ok((through, direction)),
        Ok(Locus::Point(_)) => Err(warn(
            dimension,
            REFERENCE_MISSING,
            at,
            "is a point: an angle is measured between two lines.",
        )),
        Ok(Locus::Arc(_)) => Err(curved(dimension, at)),
        Err(why) => Err(missing(dimension, at, &why)),
    }
}

/// The curve reference 0 is, or a warning.
fn curve_of(dimension: &Dimension, locator: &dyn Locator) -> Result<Curve, DimensionWarning> {
    let Some(reference) = dimension.references().first() else {
        return Err(warn(dimension, REFERENCE_MISSING, 0, "is not there."));
    };
    match locator.locate(reference) {
        Ok(Locus::Arc(curve)) => Ok(curve),
        Ok(Locus::Point(_) | Locus::Line { .. }) => Err(warn(
            dimension,
            REFERENCE_MISSING,
            0,
            "is straight: only an arc or a circle has a radius, a diameter or an arc length.",
        )),
        Err(why) => Err(missing(dimension, 0, &why)),
    }
}

fn cross(a: [i64; 2], b: [i64; 2]) -> i128 {
    let [ax, ay] = a.map(i128::from);
    let [bx, by] = b.map(i128::from);
    // Each product is under 2^126 in size, so neither saturates; their difference saturates only
    // past 2^127, never to zero, so a zero here is exact.
    ax.saturating_mul(by).saturating_sub(ay.saturating_mul(bx))
}

/// Where the line through `a1` along `e1` meets the one through `a2` along `e2`, to the nearest
/// step; `None` for parallel lines or a point past the grid.
fn meeting(a1: [i64; 2], e1: [i64; 2], a2: [i64; 2], e2: [i64; 2]) -> Option<[i64; 2]> {
    let axis = Axis { origin: a1, d: e1 };
    let t = axis.of_line(a2, e2)?;
    axis.point_at(&t)
}

/// An angular dimension.
fn measure_angle(
    dimension: &Dimension,
    locator: &dyn Locator,
    first: Ray,
    second: Ray,
    radius: Length,
) -> Measured {
    let lines = [
        line_of(dimension, locator, 0),
        line_of(dimension, locator, 1),
    ];
    let mut warnings = Vec::new();
    let mut unusable = Vec::new();
    let mut found = Vec::new();
    for (at, line) in lines.into_iter().enumerate() {
        match line {
            Ok(line) => found.push(line),
            Err(warning) => {
                warnings.push(warning);
                unusable.push(at);
            }
        }
    }
    let Ok([(a1, e1), (a2, e2)]) = <[(Point, [i64; 2]); 2]>::try_from(found) else {
        return unmeasured(dimension, unusable, warnings);
    };
    if cross(e1, e2) == 0 {
        warnings.push(warn(
            dimension,
            REFERENCE_PARALLEL,
            1,
            "runs parallel to reference 1, so the two make no angle.",
        ));
        return unmeasured(dimension, vec![1], warnings);
    }
    let rays = (first.along(e1), second.along(e2));
    let vertex = meeting(steps(a1), e1, steps(a2), e2);
    let angle = rays
        .0
        .zip(rays.1)
        .and_then(|(r1, r2)| trig::between(r1, r2));
    let drawn = vertex.zip(rays.0).zip(rays.1).and_then(|((v, r1), r2)| {
        let found = angle?;
        let start = trig::direction_of(r1)?;
        let end = trig::direction_of(r2)?;
        let middle = trig::middle_of(&start, found.units, found.anticlockwise)?;
        let at = |theta| trig::point_at_radians(v, radius.steps(), theta).map(point);
        Some((
            Figure::Arc {
                centre: point(v),
                radius,
                from: at(&start)?,
                to: at(&end)?,
                middle: at(&middle)?,
                anticlockwise: found.anticlockwise,
                large: false,
            },
            found,
        ))
    });
    let Some((figure, found)) = drawn else {
        warnings.push(warn(
            dimension,
            OUT_OF_RANGE,
            1,
            "meets reference 1 beyond the kernel's grid, so the angle cannot be drawn.",
        ));
        return unmeasured(dimension, vec![1], warnings);
    };
    let Figure::Arc {
        from, to, middle, ..
    } = figure
    else {
        return unmeasured(dimension, vec![1], warnings);
    };
    Measured {
        dimension: dimension.id().clone(),
        kind: Kind::Angular,
        segments: vec![Segment {
            from: 0,
            to: 1,
            value: None,
            angle: Some(Angle::from_units(found.units)),
            exact: found.exact,
            middle: Some(middle),
        }],
        witnesses: vec![
            Some(Witness {
                reference: 0,
                from: a1,
                to: from,
            }),
            Some(Witness {
                reference: 1,
                from: a2,
                to,
            }),
        ],
        figure,
        overall: None,
        equalities: Vec::new(),
        unusable,
        warnings,
    }
}

/// A radial or a diameter dimension.
fn measure_radius(
    dimension: &Dimension,
    locator: &dyn Locator,
    toward: Direction,
    across: bool,
) -> Measured {
    let curve = match curve_of(dimension, locator) {
        Ok(curve) => curve,
        Err(warning) => return unmeasured(dimension, vec![0], vec![warning]),
    };
    let centre = steps(curve.centre);
    let r = curve.radius.steps();
    let on_curve = trig::direction_of(toward.steps())
        .and_then(|theta| trig::point_at_radians(centre, r, &theta));
    let value = if across { r.checked_mul(2) } else { Some(r) };
    let drawn = on_curve.zip(value).and_then(|(to, value)| {
        let from = if across {
            [
                centre[0].checked_mul(2)?.checked_sub(to[0])?,
                centre[1].checked_mul(2)?.checked_sub(to[1])?,
            ]
        } else {
            centre
        };
        let middle = [
            centre[0].checked_add(to[0])?.div_euclid(2),
            centre[1].checked_add(to[1])?.div_euclid(2),
        ];
        Some((from, to, middle, value))
    });
    let Some((from, to, middle, value)) = drawn else {
        let warning = warn(
            dimension,
            OUT_OF_RANGE,
            0,
            "is so far out that its radius cannot be drawn on the kernel's grid.",
        );
        return unmeasured(dimension, vec![0], vec![warning]);
    };
    Measured {
        dimension: dimension.id().clone(),
        kind: dimension.kind(),
        segments: vec![Segment {
            from: 0,
            to: 0,
            value: Some(Length::from_steps(value)),
            angle: None,
            exact: true,
            middle: Some(point(middle)),
        }],
        witnesses: vec![None],
        figure: Figure::Leader {
            centre: curve.centre,
            from: point(from),
            to: point(to),
        },
        overall: None,
        equalities: Vec::new(),
        unusable: Vec::new(),
        warnings: Vec::new(),
    }
}

/// An arc length dimension.
fn measure_arc(dimension: &Dimension, locator: &dyn Locator, offset: Length) -> Measured {
    let curve = match curve_of(dimension, locator) {
        Ok(curve) => curve,
        Err(warning) => return unmeasured(dimension, vec![0], vec![warning]),
    };
    let Sweep::Between { from, to } = curve.sweep else {
        let warning = warn(
            dimension,
            REFERENCE_MISSING,
            0,
            "is a whole circle: it has no ends to measure an arc length between.",
        );
        return unmeasured(dimension, vec![0], vec![warning]);
    };
    let centre = steps(curve.centre);
    let r = curve.radius.steps();
    let sweep = to
        .units()
        .checked_sub(from.units())
        .map(|s| s.rem_euclid(trig::FULL_TURN));
    let outer = r.checked_add(offset.steps()).filter(|outer| *outer > 0);
    let drawn = sweep.zip(outer).and_then(|(sweep, outer)| {
        let value = trig::arc_length(r, sweep)?;
        let at = |radius: i64, units: i64| trig::point_at(centre, radius, units).map(point);
        let start = trig::radians_of(from.units().rem_euclid(trig::FULL_TURN))?;
        let middle = trig::middle_of(&start, sweep, true)?;
        let figure = Figure::Arc {
            centre: curve.centre,
            radius: Length::from_steps(outer),
            from: at(outer, from.units())?,
            to: at(outer, to.units())?,
            middle: trig::point_at_radians(centre, outer, &middle).map(point)?,
            anticlockwise: true,
            large: sweep > trig::HALF_TURN,
        };
        let witnesses = [
            (at(r, from.units())?, at(outer, from.units())?),
            (at(r, to.units())?, at(outer, to.units())?),
        ];
        Some((value, figure, witnesses))
    });
    let Some((value, figure, witnesses)) = drawn else {
        let warning = warn(
            dimension,
            OUT_OF_RANGE,
            0,
            "cannot have its arc length drawn there: the dimension's arc would have no radius, or \
             lie beyond the kernel's grid.",
        );
        return unmeasured(dimension, vec![0], vec![warning]);
    };
    let Figure::Arc { middle, .. } = figure else {
        return unmeasured(dimension, vec![0], Vec::new());
    };
    Measured {
        dimension: dimension.id().clone(),
        kind: Kind::ArcLength,
        segments: vec![Segment {
            from: 0,
            to: 0,
            value: Some(Length::from_steps(value)),
            angle: None,
            exact: false,
            middle: Some(middle),
        }],
        witnesses: witnesses
            .into_iter()
            .map(|(from, to)| {
                Some(Witness {
                    reference: 0,
                    from,
                    to,
                })
            })
            .collect(),
        figure,
        overall: None,
        equalities: Vec::new(),
        unusable: Vec::new(),
        warnings: Vec::new(),
    }
}

/// Works `dimension` out, asking `locator` where each of its references is.
#[must_use]
pub fn measure(dimension: &Dimension, locator: &dyn Locator) -> Measured {
    match *dimension.layout() {
        Layout::Horizontal { .. } | Layout::Vertical { .. } | Layout::Aligned { .. } => {
            measure_line(dimension, locator)
        }
        Layout::Angular {
            first,
            second,
            radius,
        } => measure_angle(dimension, locator, first, second, radius),
        Layout::Radial { toward } => measure_radius(dimension, locator, toward, false),
        Layout::Diameter { toward } => measure_radius(dimension, locator, toward, true),
        Layout::ArcLength { offset } => measure_arc(dimension, locator, offset),
    }
}
