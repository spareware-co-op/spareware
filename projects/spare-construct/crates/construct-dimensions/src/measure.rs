//! Working a dimension out: each segment's value, its witness lines and where its text sits, from
//! its references' places, every time it is asked.
//!
//! Each reference meets the dimension's offset line at a place ([`crate::exact`]); a segment's
//! value is the distance between two neighbours' places along it, the nearest whole grid step of
//! an exact square root, with `exact` saying whether the true value is that step. For a
//! horizontal or a vertical dimension the offset line has a unit direction, so the value is a
//! difference of two exact fractions and is exact whenever the references are on whole steps. A
//! reference that cannot be found gives a [`DimensionWarning`] naming it and no value for the
//! segments it ends: never a wrong number.

use crate::dimension::Dimension;
use crate::exact::{Axis, Frac, round_root};
use crate::locate::{Locator, Locus, Missing};
use crate::warning::{
    DimensionWarning, OUT_OF_RANGE, REFERENCE_MISSING, REFERENCE_PARALLEL, Severity,
};
use construct_grids::Point;
use construct_params::Length;
use kit_core::Id;

/// One segment: the stretch between two neighbouring references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// The reference it starts at, from 0.
    pub from: usize,
    /// The reference it ends at: `from + 1`.
    pub to: usize,
    /// Its value, the nearest whole step to the true one; `None` when a reference it depends on
    /// cannot be used.
    pub value: Option<Length>,
    /// Whether `value` is the true value, to the step.
    pub exact: bool,
    /// The middle of the segment on the offset line, where its text is centred.
    pub middle: Option<Point>,
}

/// A witness line: from the reference, drawn at `from`, to where it meets the offset line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Witness {
    /// The reference it belongs to, from 0.
    pub reference: usize,
    /// Where it starts: a point of the reference as drawn.
    pub from: Point,
    /// Where it ends: on the offset line, to the nearest step.
    pub to: Point,
}

/// Whether segments held equal are equal now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EqualState {
    /// The segments, from 0, as stored.
    pub segments: Vec<usize>,
    /// `Some(true)` when their true values are the same, `Some(false)` when they are not, `None`
    /// when one cannot be worked out. Nothing here changes a reference to make them so.
    pub equal: Option<bool>,
}

/// A dimension worked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measured {
    /// The dimension.
    pub dimension: Id,
    /// Its segments, in order.
    pub segments: Vec<Segment>,
    /// A witness line for each reference, in order; `None` where it cannot be drawn.
    pub witnesses: Vec<Option<Witness>>,
    /// The distance from the first reference to the last, when every reference can be used.
    pub overall: Option<Length>,
    /// Each stored equality and whether it holds.
    pub equalities: Vec<EqualState>,
    /// The references that could not be used (gone, no place in the view, along the offset line or
    /// too far), by position from 0, ascending: segments that end at one have no value.
    pub unusable: Vec<usize>,
    /// What went wrong, by reference, in order.
    pub warnings: Vec<DimensionWarning>,
}

fn point(at: [i64; 2]) -> Point {
    let [x, y] = at;
    Point::new(Length::from_steps(x), Length::from_steps(y))
}

fn steps(at: Point) -> [i64; 2] {
    [at.x.steps(), at.y.steps()]
}

fn warn(dimension: &Dimension, kind: &'static str, at: usize, words: &str) -> DimensionWarning {
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

/// Where reference `at` meets the offset line, as a parameter along it, and its witness line; or
/// a warning pushed and neither.
fn place(
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
        Err(Missing { detail, .. }) => {
            warnings.push(warn(
                dimension,
                REFERENCE_MISSING,
                at,
                &format!("cannot be found: {detail}."),
            ));
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
            exact: rounded.is_some_and(|(_, whole)| whole),
            middle,
        });
        squares.push(square);
    }
    (segments, squares)
}

/// Works `dimension` out, asking `locator` where each of its references is.
#[must_use]
pub fn measure(dimension: &Dimension, locator: &dyn Locator) -> Measured {
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
    let (segments, squares) = segments_of(dimension, &axis, &places, &mut warnings);
    let unusable = places
        .iter()
        .enumerate()
        .filter(|(_, place)| place.is_none())
        .map(|(at, _)| at)
        .collect();
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
            EqualState {
                segments: group.clone(),
                equal: known.map(|known| known.windows(2).all(|pair| pair.first() == pair.get(1))),
            }
        })
        .collect();
    Measured {
        dimension: dimension.id().clone(),
        segments,
        witnesses,
        overall,
        unusable,
        equalities,
        warnings,
    }
}
