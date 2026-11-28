//! A wall as the warnings see it: its outline in plan and the direction of its reference line, on
//! a level; and the one exact test made on two of them, [`overlap_along_length`].
//!
//! What it promises: the test is on whole numbers only (no float, no tolerance). Every coordinate
//! is first checked to lie within the kernel's grid ([`LIMIT`]) and the outline to have at most
//! [`MAX_CORNERS`] corners, so every product below stays well inside an `i128` (a difference is
//! under 2^42, a product of two under 2^85, a sum of 64 of them under 2^91) and the wrapping
//! arithmetic used here can never wrap.
//!
//! Depends on kit-core. Nothing here is interim.

use kit_core::Id;
use std::fmt;

/// The farthest a coordinate may be from the origin, in grid steps: the kernel's grid,
/// `2^40 - 1` steps, the same bound construct-regen's wall plan keeps.
pub const LIMIT: i64 = 1_099_511_627_775;

/// The most corners an outline may have. A wall's strip has four.
pub const MAX_CORNERS: usize = 64;

/// Why an outline cannot be taken as a wall's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WallError {
    /// Fewer than three corners, or more than [`MAX_CORNERS`].
    Corners,
    /// A coordinate beyond the grid.
    TooBig,
    /// The reference line has no direction (its start and end are one point).
    NoDirection,
    /// The outline has no area.
    NoArea,
    /// The outline turns both ways: it is not convex.
    NotConvex,
}

impl fmt::Display for WallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Corners => "an outline has from 3 to 64 corners",
            Self::TooBig => "a coordinate is beyond the kernel's grid",
            Self::NoDirection => "a wall's line has no direction",
            Self::NoArea => "an outline has no area",
            Self::NotConvex => "an outline is not convex",
        })
    }
}

impl std::error::Error for WallError {}

/// A wall on a level: its outline in plan (counter-clockwise, convex) and its reference line's
/// direction, in the model's grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wall {
    id: Id,
    level: Id,
    direction: [i64; 2],
    outline: Vec<[i64; 2]>,
    low: [i64; 2],
    high: [i64; 2],
}

fn wide(point: [i64; 2]) -> [i128; 2] {
    point.map(i128::from)
}

fn minus(a: [i64; 2], b: [i64; 2]) -> [i128; 2] {
    let (a, b) = (wide(a), wide(b));
    [a[0].wrapping_sub(b[0]), a[1].wrapping_sub(b[1])]
}

fn cross(a: [i128; 2], b: [i128; 2]) -> i128 {
    a[0].wrapping_mul(b[1])
        .wrapping_sub(a[1].wrapping_mul(b[0]))
}

/// The edges of an outline, each as its two ends, wrapping round.
fn edges(outline: &[[i64; 2]]) -> impl Iterator<Item = ([i64; 2], [i64; 2])> + '_ {
    outline
        .iter()
        .copied()
        .zip(outline.iter().copied().cycle().skip(1))
}

impl Wall {
    /// The wall `id` on `level`, its reference line running along `direction` and its outline
    /// `outline` in plan, in grid steps, either way round.
    ///
    /// # Errors
    /// [`WallError`] for an outline that is not a convex polygon with an area on the grid, or a
    /// direction of zero length.
    pub fn new(
        id: Id,
        level: Id,
        direction: [i64; 2],
        outline: &[[i64; 2]],
    ) -> Result<Self, WallError> {
        if !(3..=MAX_CORNERS).contains(&outline.len()) {
            return Err(WallError::Corners);
        }
        let beyond = |v: i64| v.unsigned_abs() > LIMIT.unsigned_abs();
        if outline.iter().flatten().copied().any(beyond)
            || direction
                .iter()
                .any(|v| v.unsigned_abs() > LIMIT.unsigned_abs().saturating_mul(2))
        {
            return Err(WallError::TooBig);
        }
        if direction == [0, 0] {
            return Err(WallError::NoDirection);
        }
        let twice_area = edges(outline).fold(0_i128, |sum, (p, q)| {
            let (p, q) = (wide(p), wide(q));
            sum.wrapping_add(cross(p, q))
        });
        let mut outline = outline.to_vec();
        match twice_area.signum() {
            0 => return Err(WallError::NoArea),
            -1 => outline.reverse(),
            _ => {}
        }
        let turns = |at: usize| {
            let mut corners = outline.iter().copied().cycle().skip(at);
            match (corners.next(), corners.next(), corners.next()) {
                (Some(a), Some(b), Some(c)) => cross(minus(b, a), minus(c, b)) >= 0,
                _ => false,
            }
        };
        if !(0..outline.len()).all(turns) {
            return Err(WallError::NotConvex);
        }
        let low = [0, 1].map(|k| {
            outline
                .iter()
                .filter_map(|p| p.get(k).copied())
                .min()
                .unwrap_or(0)
        });
        let high = [0, 1].map(|k| {
            outline
                .iter()
                .filter_map(|p| p.get(k).copied())
                .max()
                .unwrap_or(0)
        });
        Ok(Self {
            id,
            level,
            direction,
            outline,
            low,
            high,
        })
    }

    /// The wall's element ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The level it stands on.
    #[must_use]
    pub fn level(&self) -> &Id {
        &self.level
    }

    /// Its outline, counter-clockwise.
    #[must_use]
    pub fn outline(&self) -> &[[i64; 2]] {
        &self.outline
    }
}

/// Whether every corner of `b` is on or to the right of an edge of `a` (a counter-clockwise
/// outline has its inside to the left): then `b` shares no area with `a`.
fn shut_out(a: &Wall, b: &Wall) -> bool {
    edges(&a.outline).any(|(p, q)| {
        let side = minus(q, p);
        b.outline.iter().all(|r| cross(side, minus(*r, p)) <= 0)
    })
}

/// Whether two walls overlap along their length: they are on one level, their reference lines
/// are exactly parallel (the cross product of their directions is zero), and their outlines share
/// a region of positive area. Two convex outlines share such a region exactly when no edge of
/// either has the whole of the other on or beyond it (the separating axis test, on whole
/// numbers). So walls that only meet at an end, or run side by side with faces touching, or cross
/// or join at an angle, do not overlap; two parallel walls that share any length and any width
/// do, whatever their thickness.
#[must_use]
pub fn overlap_along_length(a: &Wall, b: &Wall) -> bool {
    a.level == b.level
        && cross(wide(a.direction), wide(b.direction)) == 0
        && a.low[0] < b.high[0]
        && b.low[0] < a.high[0]
        && a.low[1] < b.high[1]
        && b.low[1] < a.high[1]
        && !shut_out(a, b)
        && !shut_out(b, a)
}

/// Whether some point lies on both outlines, the edges included: two convex outlines are apart
/// exactly when an edge of one has the whole of the other strictly on its outer side (the
/// separating axis test, on whole numbers), so they meet when no edge does.
fn meet(a: &Wall, b: &Wall) -> bool {
    let apart = |a: &Wall, b: &Wall| {
        edges(&a.outline).any(|(p, q)| {
            let side = minus(q, p);
            b.outline.iter().all(|r| cross(side, minus(*r, p)) < 0)
        })
    };
    !apart(a, b) && !apart(b, a)
}

/// Whether `other` joins `host` and stands across the stretch from `low` to `high` along `axis`
/// (0 for x, 1 for y) of the host's line: they are on one level, are not the same wall, their
/// reference lines are not parallel (so one meets the other at a joint, a corner, a T or a cross),
/// their outlines meet (an edge shared, or an overlap), and the joined wall's outline spans a
/// positive length of the stretch. Whole numbers only: a joined wall that only touches the
/// stretch's end does not count, and one that lies in it by one grid step does.
#[must_use]
pub fn crosses_stretch(host: &Wall, other: &Wall, axis: usize, low: i64, high: i64) -> bool {
    host.level == other.level
        && host.id != other.id
        && cross(wide(host.direction), wide(other.direction)) != 0
        && meet(host, other)
        && other.low.get(axis).is_some_and(|v| *v < high)
        && other.high.get(axis).is_some_and(|v| low < *v)
}
