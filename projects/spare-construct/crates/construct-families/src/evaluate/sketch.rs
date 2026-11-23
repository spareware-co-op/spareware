//! A recipe's sketch as construct-solver's sketch at one type's values, checked as stored, and
//! its profiles as the engine's corners.
//!
//! Each point, line, arc, circle and constraint maps one to one onto the solver's own (arcs
//! before circles in the solver's one list of curves), every constraint length the length its
//! formula gives at these values, in grid steps. The sketch is then checked as drawn
//! (`construct_solver::check_drawn`): its stored points and radii must already meet every
//! constraint within the model tolerance, or it is refused as not solved. Solving it there is the
//! second half of evaluation (job 5.5), which hands the same solver sketch to
//! `construct_solver::solve` instead and takes its points.
//!
//! A profile's corners are its lines' points in loop order, placed in the family's coordinates
//! by the sketch's plane: on `xy` a sketch's (first, second) is (x, y) at height z; on `xz`
//! (x, z) at y; on `yz` (y, z) at x (FORMAT.md, "Coordinates").

use super::error::{EvalError, Place, Reason};
use super::values::Worked;
use crate::check::Sort;
use crate::model::{Base, Constraint as Written, Key, Recipe, Sketch as RecipeSketch};
use construct_params::{Length, Value};
use construct_solver::{
    Constraint, CurveId, LineId, Point, PointId, Sketch, SolveError, check_drawn,
};
use engine_geometry::value::grid::P3;
use std::collections::BTreeMap;

/// What a key names in one sketch.
#[derive(Clone, Copy)]
enum Item {
    Point(PointId),
    Line(LineId),
    Curve(CurveId),
}

impl Item {
    fn sort(self) -> Sort {
        match self {
            Self::Point(_) => Sort::Point,
            Self::Line(_) => Sort::Line,
            Self::Curve(_) => Sort::Curve,
        }
    }
}

/// A sketch at one type's values, checked as stored, on its plane.
pub(crate) struct Drawn {
    base: Base,
    offset: Length,
    sketch: Sketch,
    items: BTreeMap<String, Item>,
}

/// A stored length of a sketch's point or radius, in its one spelling.
fn stored_length(text: &str) -> Result<Length, Reason> {
    match Value::from_stored(text) {
        Some(value @ Value::Length(length)) if value.stored() == text => Ok(length),
        _ => Err(Reason::NotALength {
            text: text.to_owned(),
        }),
    }
}

/// Builds the solver's sketch for a recipe's sketch, one item at a time.
struct Builder<'r> {
    key: &'r Key,
    sketch: Sketch,
    items: BTreeMap<String, Item>,
    /// Each curve's stored radius, in the solver's curve order.
    radii: Vec<i64>,
    /// Each curve's key, in the solver's curve order.
    curve_keys: Vec<Key>,
}

impl Builder<'_> {
    fn at_item(&self, item: &Key, reason: Reason) -> EvalError {
        EvalError::new(
            Place::Item {
                sketch: self.key.clone(),
                item: item.clone(),
            },
            reason,
        )
    }

    fn add(&mut self, key: &Key, item: Item) -> Result<(), EvalError> {
        if self.items.contains_key(key.as_str()) {
            return Err(self.at_item(
                key,
                Reason::Duplicate {
                    key: key.as_str().to_owned(),
                },
            ));
        }
        self.items.insert(key.as_str().to_owned(), item);
        Ok(())
    }

    fn find(&self, key: &Key, wanted: Sort) -> Result<Item, Reason> {
        let Some(item) = self.items.get(key.as_str()).copied() else {
            return Err(Reason::Missing {
                sort: wanted,
                key: key.as_str().to_owned(),
            });
        };
        let good = matches!(
            (item, wanted),
            (Item::Point(_), Sort::Point)
                | (Item::Line(_), Sort::Line)
                | (Item::Curve(_), Sort::Curve)
        );
        if good {
            Ok(item)
        } else {
            Err(Reason::WrongSort {
                key: key.as_str().to_owned(),
                wanted,
                found: item.sort(),
            })
        }
    }

    fn point(&self, key: &Key) -> Result<PointId, Reason> {
        match self.find(key, Sort::Point)? {
            Item::Point(id) => Ok(id),
            other => Err(wrong(key, Sort::Point, other)),
        }
    }

    fn line(&self, key: &Key) -> Result<LineId, Reason> {
        match self.find(key, Sort::Line)? {
            Item::Line(id) => Ok(id),
            other => Err(wrong(key, Sort::Line, other)),
        }
    }

    fn curve(&self, key: &Key) -> Result<CurveId, Reason> {
        match self.find(key, Sort::Curve)? {
            Item::Curve(id) => Ok(id),
            other => Err(wrong(key, Sort::Curve, other)),
        }
    }

    /// The solver's constraint for a written one, its lengths worked out.
    fn constraint(&self, written: &Written, worked: &Worked) -> Result<Constraint, Reason> {
        let steps = |text: &str| worked.length(text).map(Length::steps);
        Ok(match written {
            Written::Coincident { a, b } => Constraint::Coincident(self.point(a)?, self.point(b)?),
            Written::Horizontal { line } => Constraint::Horizontal(self.line(line)?),
            Written::Vertical { line } => Constraint::Vertical(self.line(line)?),
            Written::Parallel { a, b } => Constraint::Parallel(self.line(a)?, self.line(b)?),
            Written::Perpendicular { a, b } => {
                Constraint::Perpendicular(self.line(a)?, self.line(b)?)
            }
            Written::EqualLength { a, b } => Constraint::EqualLength(self.line(a)?, self.line(b)?),
            Written::Distance { a, b, length } => Constraint::Distance {
                a: self.point(a)?,
                b: self.point(b)?,
                steps: steps(length)?,
            },
            Written::PointLineDistance {
                point,
                line,
                length,
                side,
            } => Constraint::PointLineDistance {
                point: self.point(point)?,
                line: self.line(line)?,
                steps: steps(length)?,
                side: *side,
            },
            Written::Fixed { point } => Constraint::Fixed(self.point(point)?),
            Written::PointOnLine { point, line } => {
                Constraint::PointOnLine(self.point(point)?, self.line(line)?)
            }
            Written::PointOnCurve { point, curve } => {
                Constraint::PointOnCurve(self.point(point)?, self.curve(curve)?)
            }
            Written::Concentric { a, b } => Constraint::Concentric(self.curve(a)?, self.curve(b)?),
            Written::EqualRadius { a, b } => {
                Constraint::EqualRadius(self.curve(a)?, self.curve(b)?)
            }
            Written::Radius { curve, length } => Constraint::Radius {
                curve: self.curve(curve)?,
                steps: steps(length)?,
            },
            Written::Diameter { curve, length } => Constraint::Diameter {
                curve: self.curve(curve)?,
                steps: steps(length)?,
            },
            Written::TangentLine { line, curve, side } => Constraint::TangentLine {
                line: self.line(line)?,
                curve: self.curve(curve)?,
                side: *side,
            },
            Written::TangentCurves { a, b, touch } => Constraint::TangentCurves {
                a: self.curve(a)?,
                b: self.curve(b)?,
                touch: *touch,
            },
        })
    }
}

fn wrong(key: &Key, wanted: Sort, found: Item) -> Reason {
    Reason::WrongSort {
        key: key.as_str().to_owned(),
        wanted,
        found: found.sort(),
    }
}

/// The place a refusal of the solver's check names: the constraint or curve it is about.
fn place_of(key: &Key, curve_keys: &[Key], e: &SolveError) -> Place {
    let curve = |id: CurveId| {
        usize::try_from(id.0)
            .ok()
            .and_then(|n| curve_keys.get(n))
            .map(|item| Place::Item {
                sketch: key.clone(),
                item: item.clone(),
            })
    };
    let constraint = |n: u32| {
        usize::try_from(n).ok().map(|index| Place::Constraint {
            sketch: key.clone(),
            index,
        })
    };
    let found = match e {
        SolveError::RoundingRefused { constraint: c, .. }
        | SolveError::Degenerate { constraint: c } => constraint(c.0),
        SolveError::ArcEndRefused { curve: c, .. } | SolveError::DegenerateCurve { curve: c } => {
            curve(*c)
        }
        _ => None,
    };
    found.unwrap_or_else(|| Place::Sketch(key.clone()))
}

/// The recipe's sketch `key` at these values, on its plane, checked as stored.
pub(crate) fn resolve(recipe: &Recipe, key: &Key, worked: &Worked) -> Result<Drawn, EvalError> {
    let Some(written) = recipe.sketches.iter().find(|s| s.key == *key) else {
        return Err(EvalError::new(
            Place::Sketch(key.clone()),
            Reason::Missing {
                sort: Sort::Sketch,
                key: key.as_str().to_owned(),
            },
        ));
    };
    let Some(plane) = recipe.planes.iter().find(|p| p.key == written.plane) else {
        return Err(EvalError::new(
            Place::Sketch(key.clone()),
            Reason::Missing {
                sort: Sort::Plane,
                key: written.plane.as_str().to_owned(),
            },
        ));
    };
    let offset = worked
        .length(&plane.offset)
        .map_err(|reason| EvalError::new(Place::Plane(plane.key.clone()), reason))?;
    let builder = build(key, written, worked)?;
    let Builder {
        sketch,
        items,
        radii,
        curve_keys,
        ..
    } = builder;
    let positions: Vec<Point> = sketch.points().to_vec();
    let sketch = sketch
        .with_positions(&positions, &radii)
        .map_err(|e| EvalError::new(Place::Sketch(key.clone()), Reason::BadSketch(e)))?;
    if let Err(e) = check_drawn(&sketch) {
        let at = place_of(key, &curve_keys, &e);
        return Err(EvalError::new(at, Reason::NotSolved(e)));
    }
    Ok(Drawn {
        base: plane.base,
        offset,
        sketch,
        items,
    })
}

/// Every item and constraint of a written sketch, in the solver's terms.
fn build<'r>(
    key: &'r Key,
    written: &RecipeSketch,
    worked: &Worked,
) -> Result<Builder<'r>, EvalError> {
    let mut b = Builder {
        key,
        sketch: Sketch::new(),
        items: BTreeMap::new(),
        radii: Vec::new(),
        curve_keys: Vec::new(),
    };
    for p in &written.points {
        let x = stored_length(&p.x).map_err(|r| b.at_item(&p.key, r))?;
        let y = stored_length(&p.y).map_err(|r| b.at_item(&p.key, r))?;
        let id = b
            .sketch
            .add_point(x.steps(), y.steps())
            .map_err(|e| b.at_item(&p.key, Reason::BadSketch(e)))?;
        b.add(&p.key, Item::Point(id))?;
    }
    for l in &written.lines {
        let ends = b.point(&l.start).and_then(|s| Ok((s, b.point(&l.end)?)));
        let (start, end) = ends.map_err(|r| b.at_item(&l.key, r))?;
        let id = b
            .sketch
            .add_line(start, end)
            .map_err(|e| b.at_item(&l.key, Reason::BadSketch(e)))?;
        b.add(&l.key, Item::Line(id))?;
    }
    for a in &written.arcs {
        let radius = stored_length(&a.radius).map_err(|r| b.at_item(&a.key, r))?;
        let ends = b
            .point(&a.centre)
            .and_then(|c| Ok((c, b.point(&a.start)?, b.point(&a.end)?)));
        let (centre, start, end) = ends.map_err(|r| b.at_item(&a.key, r))?;
        let id = b
            .sketch
            .add_arc(centre, start, end)
            .map_err(|e| b.at_item(&a.key, Reason::BadSketch(e)))?;
        b.add(&a.key, Item::Curve(id))?;
        b.radii.push(radius.steps());
        b.curve_keys.push(a.key.clone());
    }
    for c in &written.circles {
        let radius = stored_length(&c.radius).map_err(|r| b.at_item(&c.key, r))?;
        let centre = b.point(&c.centre).map_err(|r| b.at_item(&c.key, r))?;
        let id = b
            .sketch
            .add_circle(centre, radius.steps())
            .map_err(|e| b.at_item(&c.key, Reason::BadSketch(e)))?;
        b.add(&c.key, Item::Curve(id))?;
        b.radii.push(radius.steps());
        b.curve_keys.push(c.key.clone());
    }
    for (index, c) in written.constraints.iter().enumerate() {
        let at = |reason| {
            EvalError::new(
                Place::Constraint {
                    sketch: key.clone(),
                    index,
                },
                reason,
            )
        };
        let constraint = b.constraint(c, worked).map_err(at)?;
        b.sketch
            .add_constraint(constraint)
            .map_err(|e| at(Reason::BadSketch(e)))?;
    }
    Ok(b)
}

impl Drawn {
    /// The corners of a loop of the sketch's lines, in loop order, `height` along the plane's
    /// normal from the plane.
    pub(crate) fn corners(&self, lines: &[Key], height: Length) -> Result<Vec<P3>, Reason> {
        let mut ends = Vec::with_capacity(lines.len());
        for key in lines {
            let Some(item) = self.items.get(key.as_str()).copied() else {
                return Err(Reason::Missing {
                    sort: Sort::Line,
                    key: key.as_str().to_owned(),
                });
            };
            let Item::Line(id) = item else {
                return Err(wrong(key, Sort::Line, item));
            };
            let line = self.sketch.line(id).ok_or(Reason::NotALoop)?;
            ends.push((line.start, line.end));
        }
        let order = in_loop(&ends).ok_or(Reason::NotALoop)?;
        let level = self
            .offset
            .checked_add(height)
            .ok_or(Reason::OutOfRange)?
            .steps();
        order
            .into_iter()
            .map(|id| {
                let p = self.sketch.point(id).ok_or(Reason::NotALoop)?;
                let [x, y, z] = match self.base {
                    Base::Xy => [p.x, p.y, level],
                    Base::Xz => [p.x, level, p.y],
                    Base::Yz => [level, p.x, p.y],
                };
                P3::new(x, y, z).map_err(|_| Reason::OutOfRange)
            })
            .collect()
    }

    /// `depth` along the plane's normal, as the engine's vector.
    pub(crate) fn along(&self, depth: Length) -> [i64; 3] {
        let d = depth.steps();
        match self.base {
            Base::Xy => [0, 0, d],
            Base::Xz => [0, d, 0],
            Base::Yz => [d, 0, 0],
        }
    }
}

/// The points of a loop of lines in order, starting at the first line's start: three or more
/// lines, each sharing a point with the next and the last with the first.
fn in_loop(ends: &[(PointId, PointId)]) -> Option<Vec<PointId>> {
    let ((first_start, first_end), rest) = ends.split_first()?;
    if rest.len() < 2 {
        return None;
    }
    let mut order = vec![*first_start, *first_end];
    let mut at = *first_end;
    for (start, end) in rest {
        at = if *start == at {
            *end
        } else if *end == at {
            *start
        } else {
            return None;
        };
        order.push(at);
    }
    // The last point is the first again.
    (order.pop() == Some(*first_start)).then_some(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_is_its_points_in_order_either_way_round() {
        let p = PointId;
        let square = [(p(0), p(1)), (p(2), p(1)), (p(2), p(3)), (p(0), p(3))];
        assert_eq!(in_loop(&square), Some(vec![p(0), p(1), p(2), p(3)]));
        assert_eq!(in_loop(&square[..2]), None);
        let open = [(p(0), p(1)), (p(1), p(2)), (p(2), p(3))];
        assert_eq!(in_loop(&open), None);
        assert_eq!(in_loop(&[]), None);
    }
}
