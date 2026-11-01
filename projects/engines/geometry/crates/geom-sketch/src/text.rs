//! The sketch text form, `sketch v1`: a sketch problem as lines of text that round-trip exactly, for
//! tests, bug reports and AI agents.
//!
//! What it does: [`write`] prints a sketch: its plane, then each entity and constraint in authoring
//! order, one to a line. [`parse`] reads that back. An id is written `sequence.key`; a number as the
//! shortest decimal that reads back to the same double, so a grid value is a whole number of steps.
//!
//! ```text
//! sketch v1
//! plane lattice 0 0 0 1 0 0 0 1 0 q0
//! entity 1.0 point 0 0
//! entity 1.1 point 300 400
//! entity 2.0 construction line 1.0 1.1 segment
//! constraint 3.0 driving 0 distance 1.0 1.1 500
//! ```
//!
//! Guarantees:
//!
//! - **Round trip.** `parse(write(s)) == s`, bit for bit, for every sketch.
//! - **Canonical.** A sketch has one text: `write(parse(t))` is the same whatever order `t`'s items
//!   came in, once they name only what came before them.
//! - **Never a panic.** Any text at all gives a sketch or [`SketchError::Text`] with its line, or
//!   the refusal [`Sketch::add_entity`] gives: the parser's fuzz target holds it to that
//!   (`geom-fuzz`'s `sketch_text`). A value the model refuses is refused here too.
//! - **Versioned.** The first line names the grammar. A later grammar is a new tag, and this one
//!   keeps reading its own (chapter 26.2).
//!
//! Method: a line-oriented text form on the model of ezpz's problem files, section 15.13
//! (`projects/engines/geometry/architecture/15-sketches.md`). `core`'s float formatting and parsing
//! round-trip every double (the shortest-digits guarantee of the standard library's `Display`).
//!
//! Consulted: ezpz (Zoo), MIT,
//!   <https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/README.md>,
//!   for the idea of a problem file; none of its grammar is taken.
//!
//! Cost: linear in the text, and O(log n) a line for the sorted insert. No recursion.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::str::SplitAsciiWhitespace;

use geom_num::{GridStep, P3};

use crate::model::{
    Constraint, ConstraintKind, Entity, EntityKind, Extent, ItemId, Side, Sketch, SketchError,
    Standing,
};
use crate::plane::{Lattice, PlaneFrame, Rotated, SketchPlane};

/// The grammar's first line.
const HEADER: &str = "sketch v1";

const fn step_name(step: GridStep) -> &'static str {
    match step {
        GridStep::Q0 => "q0",
        GridStep::Q1 => "q1",
        GridStep::Q2 => "q2",
    }
}

const fn extent_name(extent: Extent) -> &'static str {
    match extent {
        Extent::Segment => "segment",
        Extent::Ray => "ray",
        Extent::Infinite => "infinite",
    }
}

/// `value` after a space, or nothing if the string runs out of memory: a sketch that fits in memory
/// prints in a little more.
fn put(out: &mut String, value: impl core::fmt::Display) -> Result<(), SketchError> {
    write!(out, " {value}").map_err(|_| SketchError::OutOfMemory)
}

fn put_id(out: &mut String, id: ItemId) -> Result<(), SketchError> {
    write!(out, " {}.{}", id.sequence, id.key).map_err(|_| SketchError::OutOfMemory)
}

fn write_entity(out: &mut String, entity: &Entity) -> Result<(), SketchError> {
    out.push_str("entity");
    put_id(out, entity.id)?;
    if entity.construction {
        out.push_str(" construction");
    }
    match &entity.kind {
        EntityKind::Point { x, y } => {
            out.push_str(" point");
            put(out, x)?;
            put(out, y)?;
        }
        EntityKind::Line { start, end, extent } => {
            out.push_str(" line");
            put_id(out, *start)?;
            put_id(out, *end)?;
            put(out, extent_name(*extent))?;
        }
        EntityKind::Circle { centre, radius } => {
            out.push_str(" circle");
            put_id(out, *centre)?;
            put(out, radius)?;
        }
        EntityKind::Arc {
            centre,
            radius,
            start,
            end,
        } => {
            out.push_str(" arc");
            put_id(out, *centre)?;
            put(out, radius)?;
            put_id(out, *start)?;
            put_id(out, *end)?;
        }
        EntityKind::Ellipse {
            centre,
            axis,
            minor,
        } => {
            out.push_str(" ellipse");
            put_id(out, *centre)?;
            put(out, axis[0])?;
            put(out, axis[1])?;
            put(out, minor)?;
        }
        EntityKind::Spline {
            degree,
            points,
            knots,
            weights,
        } => {
            out.push_str(" spline");
            put(out, degree)?;
            put(out, points.len())?;
            for id in points {
                put_id(out, *id)?;
            }
            put(out, knots.len())?;
            for x in knots {
                put(out, x)?;
            }
            put(out, weights.len())?;
            for x in weights {
                put(out, x)?;
            }
        }
        EntityKind::Parameter { curve, t, speed } => {
            out.push_str(" parameter");
            put_id(out, *curve)?;
            put(out, t)?;
            put(out, speed)?;
        }
        EntityKind::Scalar { value, size } => {
            out.push_str(" scalar");
            put(out, value)?;
            put(out, size)?;
        }
        EntityKind::Placement { offset, turn, size } => {
            out.push_str(" placement");
            put(out, offset[0])?;
            put(out, offset[1])?;
            put(out, turn)?;
            put(out, size)?;
        }
        EntityKind::Reference => out.push_str(" reference"),
    }
    out.push('\n');
    Ok(())
}

fn write_constraint(out: &mut String, constraint: &Constraint) -> Result<(), SketchError> {
    out.push_str("constraint");
    put_id(out, constraint.id)?;
    out.push_str(match constraint.standing {
        Standing::Driving => " driving",
        Standing::Reference => " reference",
    });
    put(out, constraint.priority)?;
    match constraint.kind {
        ConstraintKind::Coincident { a, b } => {
            out.push_str(" coincident");
            put_id(out, a)?;
            put_id(out, b)?;
        }
        ConstraintKind::Horizontal { line } => {
            out.push_str(" horizontal");
            put_id(out, line)?;
        }
        ConstraintKind::Vertical { line } => {
            out.push_str(" vertical");
            put_id(out, line)?;
        }
        ConstraintKind::Distance { a, b, distance } => {
            out.push_str(" distance");
            put_id(out, a)?;
            put_id(out, b)?;
            put(out, distance)?;
        }
        ConstraintKind::PointLine {
            point,
            line,
            distance,
            side,
        } => {
            out.push_str(" point-line");
            put_id(out, point)?;
            put_id(out, line)?;
            put(out, distance)?;
            out.push_str(match side {
                Side::Left => " left",
                Side::Right => " right",
            });
        }
        ConstraintKind::Fix {
            entity,
            slot,
            value,
        } => {
            out.push_str(" fix");
            put_id(out, entity)?;
            put(out, slot)?;
            put(out, value)?;
        }
    }
    out.push('\n');
    Ok(())
}

/// `sketch` in `sketch v1`.
///
/// # Errors
///
/// [`SketchError::OutOfMemory`].
pub fn write(sketch: &Sketch) -> Result<String, SketchError> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    let plane = sketch.plane();
    match plane.frame() {
        PlaneFrame::Lattice(lattice) => {
            out.push_str("plane lattice");
            let origin = lattice.origin()?;
            for x in [origin.x.get(), origin.y.get(), origin.z.get()] {
                put(&mut out, x)?;
            }
            for x in lattice.u().iter().chain(&lattice.v()) {
                put(&mut out, x)?;
            }
        }
        PlaneFrame::Rotated(rotated) => {
            out.push_str("plane rotated");
            let (origin, x_axis, y_axis) = rotated.parts();
            for x in origin.iter().chain(&x_axis).chain(&y_axis) {
                put(&mut out, x)?;
            }
        }
    }
    put(&mut out, step_name(plane.step()))?;
    out.push('\n');
    for entity in sketch.entities() {
        write_entity(&mut out, entity)?;
    }
    for constraint in sketch.constraints() {
        write_constraint(&mut out, constraint)?;
    }
    Ok(out)
}

/// One line's words, with the line's number for its refusal.
struct Words<'t> {
    words: SplitAsciiWhitespace<'t>,
    line: usize,
}

impl<'t> Words<'t> {
    fn bad(&self) -> SketchError {
        SketchError::Text { line: self.line }
    }

    fn word(&mut self) -> Result<&'t str, SketchError> {
        self.words
            .next()
            .ok_or(SketchError::Text { line: self.line })
    }

    fn real(&mut self) -> Result<f64, SketchError> {
        let word = self.word()?;
        let value: f64 = word.parse().map_err(|_| self.bad())?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(self.bad())
        }
    }

    fn whole<T: core::str::FromStr>(&mut self) -> Result<T, SketchError> {
        let word = self.word()?;
        word.parse().map_err(|_| self.bad())
    }

    fn id(&mut self) -> Result<ItemId, SketchError> {
        let word = self.word()?;
        let (sequence, key) = word.split_once('.').ok_or(self.bad())?;
        Ok(ItemId::new(
            sequence.parse().map_err(|_| self.bad())?,
            key.parse().map_err(|_| self.bad())?,
        ))
    }

    fn end(&mut self) -> Result<(), SketchError> {
        if self.words.next().is_none() {
            Ok(())
        } else {
            Err(self.bad())
        }
    }
}

fn read_plane(words: &mut Words<'_>) -> Result<SketchPlane, SketchError> {
    if words.word()? != "plane" {
        return Err(words.bad());
    }
    let kind = words.word()?;
    let frame = match kind {
        "lattice" => {
            let origin =
                P3::new(words.whole()?, words.whole()?, words.whole()?).map_err(|_| words.bad())?;
            let u = [words.whole()?, words.whole()?, words.whole()?];
            let v = [words.whole()?, words.whole()?, words.whole()?];
            PlaneFrame::Lattice(Lattice::new(origin, u, v)?)
        }
        "rotated" => {
            let origin =
                P3::new(words.whole()?, words.whole()?, words.whole()?).map_err(|_| words.bad())?;
            let x_axis = [words.real()?, words.real()?, words.real()?];
            let y_axis = [words.real()?, words.real()?, words.real()?];
            PlaneFrame::Rotated(Rotated::new(origin, x_axis, y_axis)?)
        }
        _ => return Err(words.bad()),
    };
    let step = match words.word()? {
        "q0" => GridStep::Q0,
        "q1" => GridStep::Q1,
        "q2" => GridStep::Q2,
        _ => return Err(words.bad()),
    };
    words.end()?;
    Ok(SketchPlane::new(frame, step))
}

/// A list on a line, its length first, bounded by what the line can hold: every item is at least a
/// word.
fn read_list<'t, T>(
    words: &mut Words<'t>,
    mut one: impl FnMut(&mut Words<'t>) -> Result<T, SketchError>,
) -> Result<Vec<T>, SketchError> {
    let count: usize = words.whole()?;
    let mut list = Vec::new();
    for _ in 0..count {
        let item = one(words)?;
        list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
        list.push(item);
    }
    Ok(list)
}

fn read_entity(words: &mut Words<'_>) -> Result<Entity, SketchError> {
    let id = words.id()?;
    let mut name = words.word()?;
    let construction = name == "construction";
    if construction {
        name = words.word()?;
    }
    let kind = match name {
        "point" => EntityKind::Point {
            x: words.real()?,
            y: words.real()?,
        },
        "line" => {
            let (start, end) = (words.id()?, words.id()?);
            let extent = match words.word()? {
                "segment" => Extent::Segment,
                "ray" => Extent::Ray,
                "infinite" => Extent::Infinite,
                _ => return Err(words.bad()),
            };
            EntityKind::Line { start, end, extent }
        }
        "circle" => EntityKind::Circle {
            centre: words.id()?,
            radius: words.real()?,
        },
        "arc" => EntityKind::Arc {
            centre: words.id()?,
            radius: words.real()?,
            start: words.id()?,
            end: words.id()?,
        },
        "ellipse" => EntityKind::Ellipse {
            centre: words.id()?,
            axis: [words.real()?, words.real()?],
            minor: words.real()?,
        },
        "spline" => EntityKind::Spline {
            degree: words.whole()?,
            points: read_list(words, Words::id)?,
            knots: read_list(words, Words::real)?,
            weights: read_list(words, Words::real)?,
        },
        "parameter" => EntityKind::Parameter {
            curve: words.id()?,
            t: words.real()?,
            speed: words.real()?,
        },
        "scalar" => EntityKind::Scalar {
            value: words.real()?,
            size: words.real()?,
        },
        "placement" => EntityKind::Placement {
            offset: [words.real()?, words.real()?],
            turn: words.real()?,
            size: words.real()?,
        },
        "reference" => EntityKind::Reference,
        _ => return Err(words.bad()),
    };
    words.end()?;
    Ok(Entity {
        id,
        kind,
        construction,
    })
}

fn read_constraint(words: &mut Words<'_>) -> Result<Constraint, SketchError> {
    let id = words.id()?;
    let standing = match words.word()? {
        "driving" => Standing::Driving,
        "reference" => Standing::Reference,
        _ => return Err(words.bad()),
    };
    let priority = words.whole()?;
    let kind = match words.word()? {
        "coincident" => ConstraintKind::Coincident {
            a: words.id()?,
            b: words.id()?,
        },
        "horizontal" => ConstraintKind::Horizontal { line: words.id()? },
        "vertical" => ConstraintKind::Vertical { line: words.id()? },
        "distance" => ConstraintKind::Distance {
            a: words.id()?,
            b: words.id()?,
            distance: words.real()?,
        },
        "point-line" => ConstraintKind::PointLine {
            point: words.id()?,
            line: words.id()?,
            distance: words.real()?,
            side: match words.word()? {
                "left" => Side::Left,
                "right" => Side::Right,
                _ => return Err(words.bad()),
            },
        },
        "fix" => ConstraintKind::Fix {
            entity: words.id()?,
            slot: words.whole()?,
            value: words.real()?,
        },
        _ => return Err(words.bad()),
    };
    words.end()?;
    Ok(Constraint {
        id,
        kind,
        standing,
        priority,
    })
}

/// The sketch `text` writes.
///
/// # Errors
///
/// [`SketchError::Text`] for text that is not `sketch v1`, with its line; what
/// [`Sketch::add_entity`] and [`Sketch::add_constraint`] refuse.
pub fn parse(text: &str) -> Result<Sketch, SketchError> {
    let mut lines = text.lines().enumerate();
    let (_, first) = lines.next().ok_or(SketchError::Text { line: 1 })?;
    if first.trim_end() != HEADER {
        return Err(SketchError::Text { line: 1 });
    }
    let mut lines = lines.map(|(i, line)| Words {
        words: line.split_ascii_whitespace(),
        line: i.saturating_add(1),
    });
    let mut plane = lines.next().ok_or(SketchError::Text { line: 2 })?;
    let mut sketch = Sketch::new(read_plane(&mut plane)?);
    for mut words in lines {
        match words.words.next() {
            None => {}
            Some("entity") => sketch.add_entity(read_entity(&mut words)?)?,
            Some("constraint") => sketch.add_constraint(read_constraint(&mut words)?)?,
            Some(_) => return Err(words.bad()),
        }
    }
    Ok(sketch)
}
