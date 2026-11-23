//! Measuring what the evaluator builds: an engine of this crate's own behind the kernel's
//! `Engine` trait that keeps each extrusion exactly as it is asked for (its profile's corners and
//! holes and its vector, grid steps), so the solids a template gives can be measured exactly:
//! the box each lies in, and the area of an extrusion's face.
//!
//! It makes no geometry and answers no operation but extrusion: a subtraction's or a union's
//! result is kept as [`Measured::Combined`], which has no face to measure, and every other
//! operation is refused. The generator's templates build every solid it measures as one
//! extrusion; a measurement asked of anything else is refused, never guessed.
//!
//! Why our own and not a library's: a face's area must be exact in square grid steps (i128), the
//! unit construct-params keeps an area in, and the polygon area crates work in doubles. The
//! shoelace sum over whole grid points is exact in whole numbers, so it is written here, every
//! step a checked one.

use construct_params::{Area, Length};
use engine_geometry::value::engine::{
    Answer, Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Tolerance,
    Volume,
};
use engine_geometry::value::grid::{GridStep, P3};

/// One extrusion as it was asked for: grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prism {
    /// The profile's outer corners, in order round it.
    pub outer: Vec<[i64; 3]>,
    /// The profile's holes, each a loop of corners.
    pub holes: Vec<Vec<[i64; 3]>>,
    /// The vector it is swept along.
    pub by: [i64; 3],
}

/// A solid of the measuring engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Measured {
    /// One extrusion.
    Prism(Prism),
    /// What a subtraction or a union made: nothing is measured from it.
    Combined,
}

/// The box a solid lies in, grid steps per axis: lowest and highest x, y and z.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box3 {
    /// The lowest corner.
    pub low: [Length; 3],
    /// The highest corner.
    pub high: [Length; 3],
}

impl Box3 {
    /// The size along axis `axis` (0 x, 1 y, 2 z); `None` past an axis or past i64.
    #[must_use]
    pub fn span(&self, axis: usize) -> Option<Length> {
        let (low, high) = (self.low.get(axis)?, self.high.get(axis)?);
        high.checked_sub(*low)
    }

    /// Whether `other` lies within this box along axis `axis`.
    #[must_use]
    pub fn holds_along(&self, other: &Box3, axis: usize) -> bool {
        match (
            self.low.get(axis),
            self.high.get(axis),
            other.low.get(axis),
            other.high.get(axis),
        ) {
            (Some(lo), Some(hi), Some(olo), Some(ohi)) => lo <= olo && ohi <= hi,
            _ => false,
        }
    }
}

fn point(p: &P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

impl Prism {
    /// The axis it is swept along, if it is swept along one axis only.
    #[must_use]
    pub fn axis(&self) -> Option<usize> {
        match self.by {
            [x, 0, 0] if x != 0 => Some(0),
            [0, y, 0] if y != 0 => Some(1),
            [0, 0, z] if z != 0 => Some(2),
            _ => None,
        }
    }

    /// The box it lies in: its corners and its corners moved along its vector.
    #[must_use]
    pub fn bounds(&self) -> Option<Box3> {
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for p in &self.outer {
            let moved = [
                p[0].checked_add(self.by[0])?,
                p[1].checked_add(self.by[1])?,
                p[2].checked_add(self.by[2])?,
            ];
            for q in [*p, moved] {
                for (axis, value) in q.iter().enumerate() {
                    let lo = low.get_mut(axis)?;
                    *lo = (*lo).min(*value);
                    let hi = high.get_mut(axis)?;
                    *hi = (*hi).max(*value);
                }
            }
        }
        if self.outer.is_empty() {
            return None;
        }
        Some(Box3 {
            low: low.map(Length::from_steps),
            high: high.map(Length::from_steps),
        })
    }

    /// The area of its outline, holes not taken away: the face it is swept from, seen along its
    /// axis. `None` if it is not swept along one axis or the area is not whole square steps.
    #[must_use]
    pub fn outline_area(&self) -> Option<Area> {
        loop_area(&self.outer, self.axis()?)
    }

    /// The area of its face, holes taken away.
    #[must_use]
    pub fn face_area(&self) -> Option<Area> {
        let axis = self.axis()?;
        let mut area = loop_area(&self.outer, axis)?;
        for hole in &self.holes {
            area = area.checked_sub(loop_area(hole, axis)?)?;
        }
        Some(area)
    }
}

/// The area a loop of corners encloses, seen along `axis`, in square grid steps: the shoelace
/// sum over the other two coordinates, exactly. `None` for a loop of fewer than three corners,
/// a sum past i128, or an odd sum (a half square step, which no grid rectangle has).
fn loop_area(corners: &[[i64; 3]], axis: usize) -> Option<Area> {
    let (a, b) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        2 => (0, 1),
        _ => return None,
    };
    if corners.len() < 3 {
        return None;
    }
    let mut twice: i128 = 0;
    for (i, p) in corners.iter().enumerate() {
        let next = corners.get(i.checked_add(1)?).or_else(|| corners.first())?;
        let (pa, pb) = (i128::from(*p.get(a)?), i128::from(*p.get(b)?));
        let (qa, qb) = (i128::from(*next.get(a)?), i128::from(*next.get(b)?));
        let cross = pa.checked_mul(qb)?.checked_sub(qa.checked_mul(pb)?)?;
        twice = twice.checked_add(cross)?;
    }
    let twice = twice.checked_abs()?;
    if twice.checked_rem(2)? != 0 {
        return None;
    }
    Some(Area::from_square_steps(twice.checked_div(2)?))
}

/// The measuring engine.
#[derive(Clone, Copy, Debug, Default)]
pub struct Measure;

fn unanswered(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything: this engine only keeps extrusions to measure".to_owned(),
    }
}

impl Engine for Measure {
    type Solid = Measured;

    fn edition(&self) -> Edition {
        Edition {
            engine: "construct-doorgen.measure".to_owned(),
            version: "1".to_owned(),
            step: GridStep::Q0,
            tolerances: vec![Tolerance {
                name: "none: every measure is exact".to_owned(),
                value: 0.0,
                unit: "m".to_owned(),
            }],
            answers: vec![Answer {
                operation: "extrude".to_owned(),
                case: "every case: the profile and vector are kept as asked".to_owned(),
                by: "construct-doorgen's measuring engine".to_owned(),
            }],
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Measured, EngineError> {
        if profile.outer.len() < 3 || by == [0, 0, 0] {
            return Err(EngineError::Invalid {
                operation: "extrude",
                what: "a loop of three or more corners, swept".to_owned(),
            });
        }
        Ok(Measured::Prism(Prism {
            outer: profile.outer.iter().map(point).collect(),
            holes: profile
                .holes
                .iter()
                .map(|hole| hole.iter().map(point).collect())
                .collect(),
            by,
        }))
    }

    fn subtract(&mut self, _: &Measured, _: &Measured) -> Result<Measured, EngineError> {
        Ok(Measured::Combined)
    }

    fn subtract_pieces(
        &mut self,
        _: &Measured,
        _: &Measured,
    ) -> Result<Vec<Measured>, EngineError> {
        Err(unanswered("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Measured, EngineError> {
        Err(unanswered("loft"))
    }

    fn unite(&mut self, _: &Measured, _: &Measured) -> Result<Measured, EngineError> {
        Ok(Measured::Combined)
    }

    fn volume(&mut self, _: &Measured) -> Result<Volume, EngineError> {
        Err(unanswered("volume"))
    }

    fn mesh(&mut self, _: &Measured) -> Result<DisplayMesh, EngineError> {
        Err(unanswered("mesh"))
    }

    fn section(&mut self, _: &Measured, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unanswered("section"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]
    use super::*;

    #[test]
    fn a_rectangle_with_a_hole_has_its_exact_face_and_outline() {
        let prism = Prism {
            outer: vec![[0, 0, 0], [10, 0, 0], [10, 0, 6], [0, 0, 6]],
            holes: vec![vec![[2, 0, 2], [4, 0, 2], [4, 0, 4], [2, 0, 4]]],
            by: [0, 3, 0],
        };
        assert_eq!(prism.axis(), Some(1));
        assert_eq!(prism.outline_area(), Some(Area::from_square_steps(60)));
        assert_eq!(prism.face_area(), Some(Area::from_square_steps(56)));
        let b = prism.bounds().unwrap();
        assert_eq!(b.span(1), Some(Length::from_steps(3)));
    }

    #[test]
    fn a_half_square_step_or_a_slanted_sweep_is_not_measured() {
        let triangle = Prism {
            outer: vec![[0, 0, 0], [1, 0, 0], [0, 0, 1]],
            holes: vec![],
            by: [0, 1, 0],
        };
        assert_eq!(triangle.outline_area(), None);
        let slanted = Prism {
            by: [1, 1, 0],
            ..triangle
        };
        assert_eq!(slanted.axis(), None);
    }
}
