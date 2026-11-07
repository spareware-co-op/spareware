//! Transform and length edits (chapter 14.7): stretch, lengthen, arrays, mirror, scale, align,
//! divide and measure, with arc length certified by interval.
//!
//! What it does: a [`Similarity`] is a quarter-turn rotation, an optional mirror, a whole-number
//! scale and a whole-step shift, in that order; it places a grid point, a segment or a polyline on
//! the grid again, exactly. [`place`] applies it to a [`Shape`] and [`align`] finds the one that
//! takes two points to two others. [`stretch`] moves the vertices of a polyline inside a window by
//! a grid vector. [`array_rectangular`] and [`array_polar`] copy a shape by exact translations or
//! quarter turns. [`lengthen`] moves one end of a segment along its carrier by whole steps of its
//! primitive direction. [`divide`] and [`measure`] put exact rational points on a segment at equal
//! spacing. [`polyline_length`] is the arc length of a polyline as an [`Interval`] that holds the
//! true length.
//!
//! Guarantees:
//!
//! - **Exact where the grid allows.** A similarity maps grid points to grid points with checked
//!   whole-number arithmetic, so a quarter turn, a mirror, a whole scale and a shift lose nothing;
//!   a result off the grid or past the coordinate range is [`EditError::Range`], never a rounded
//!   value. A bulge is the tangent of a quarter of the sweep, so a map that keeps angles keeps it,
//!   and a mirror negates it: a placed arc keeps its sweep exactly.
//! - **Refused, not approximated.** A shape kind this module does not place is
//!   [`EditError::Unsupported`]; a polar array of a count that is no divisor of four quarter turns
//!   is [`EditError::NotExact`]; an alignment that no quarter turn and whole scale makes is too.
//! - **Certified lengths.** [`polyline_length`] bounds each straight segment by the interval
//!   square root of its exact squared length, and each arc by c (1 + b²) atan |b| / |b|, which is
//!   the arc of chord c and bulge b, in outward-rounded interval arithmetic; the true length lies
//!   in the interval returned.
//! - **Deterministic.** Pure functions of their arguments; the same bits on every target.
//!
//! Method: the edit table of chapter 14.7
//! (`projects/engines/geometry/architecture/14-2d-drafting.md`); the arc of a bulge is the
//! polyline bulge of the DXF reference, the arc's radius r = c (1 + b²) / (4 |b|) following from the
//! chord c = 2 r sin(θ / 2) and sin(2 atan b) = 2 b / (1 + b²); interval arithmetic after
//! [Moore, Kearfott and Cloud, *Introduction to Interval Analysis*, SIAM 2009,
//! doi:10.1137/1.9780898717716](https://doi.org/10.1137/1.9780898717716), as `geom-num` supplies it.
//!
//! Cost: linear in the vertices and copies; one interval arc tangent and one square root per
//! segment of a length.

use alloc::vec::Vec;
use core::fmt;

use geom_num::grid::{P2, Steps};
use geom_num::interval::Interval;
use geom_num::{NumError, carrier::Rational2};

use crate::carrier::Line2;
use crate::entity::{Bulge, EntityError, Ends, Frac, Polyline, Shape};
use crate::point::Pt2;

/// Why an edit was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    /// A result off the grid or past the coordinate range, or an integer too wide for the exact
    /// arithmetic.
    Range,
    /// A zero scale, a zero count, or two points that are one point.
    Degenerate,
    /// A count or alignment that no exact map gives.
    NotExact,
    /// A shape kind this module does not place, by its stable tag number.
    Unsupported(u8),
    /// The entity model refused the result.
    Entity(EntityError),
}

impl From<NumError> for EditError {
    fn from(_: NumError) -> Self {
        EditError::Range
    }
}

impl From<EntityError> for EditError {
    fn from(e: EntityError) -> Self {
        EditError::Entity(e)
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Range => f.write_str("the result is off the grid or past its range"),
            EditError::Degenerate => f.write_str("a zero scale or count, or two equal points"),
            EditError::NotExact => f.write_str("no exact map gives this"),
            EditError::Unsupported(tag) => write!(f, "shape kind {tag} is not placed here"),
            EditError::Entity(e) => write!(f, "the entity model refused the result: {e:?}"),
        }
    }
}

impl core::error::Error for EditError {}

/// A map of the plane that takes the grid to itself: first `turns` quarter turns counter-clockwise
/// about the origin, then a mirror in the x axis if `mirror`, then the whole-number `scale`, then
/// the shift. A negative `scale` is a half turn as well; a mirror reverses the sense of every arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Similarity {
    turns: u8,
    mirror: bool,
    scale: i64,
    shift: [i64; 2],
}

impl Similarity {
    /// The map that does nothing.
    pub const IDENTITY: Similarity = Similarity {
        turns: 0,
        mirror: false,
        scale: 1,
        shift: [0, 0],
    };

    /// A shift by (`dx`, `dy`).
    #[must_use]
    pub const fn translation(dx: i64, dy: i64) -> Similarity {
        Similarity {
            shift: [dx, dy],
            ..Similarity::IDENTITY
        }
    }

    /// `turns` quarter turns counter-clockwise about `about`.
    ///
    /// # Errors
    ///
    /// [`EditError::Range`] if the fixed point's image cannot be written in whole steps.
    pub fn quarter_turns(turns: u8, about: P2) -> Result<Similarity, EditError> {
        Similarity::fixing(
            Similarity {
                turns: turns % 4,
                ..Similarity::IDENTITY
            },
            about,
        )
    }

    /// The whole scale `k` about `about`; a negative `k` also turns by a half turn.
    ///
    /// # Errors
    ///
    /// [`EditError::Degenerate`] for `k` = 0; [`EditError::Range`] if the shift does not fit.
    pub fn scaling(k: i64, about: P2) -> Result<Similarity, EditError> {
        if k == 0 {
            return Err(EditError::Degenerate);
        }
        Similarity::fixing(
            Similarity {
                scale: k,
                ..Similarity::IDENTITY
            },
            about,
        )
    }

    /// The mirror in the horizontal line y = `y`.
    ///
    /// # Errors
    ///
    /// [`EditError::Range`] if the shift does not fit.
    pub fn mirror_in_row(y: i64) -> Result<Similarity, EditError> {
        Similarity::fixing(
            Similarity {
                mirror: true,
                ..Similarity::IDENTITY
            },
            P2::new(0, y).map_err(|_| EditError::Range)?,
        )
    }

    /// The mirror in the vertical line x = `x`.
    ///
    /// # Errors
    ///
    /// [`EditError::Range`] if the shift does not fit.
    pub fn mirror_in_column(x: i64) -> Result<Similarity, EditError> {
        // A mirror in the x axis, then a half turn, is the mirror in the y axis.
        Similarity::fixing(
            Similarity {
                turns: 2,
                mirror: true,
                ..Similarity::IDENTITY
            },
            P2::new(x, 0).map_err(|_| EditError::Range)?,
        )
    }

    /// Whether the map reverses the sense of rotation.
    #[must_use]
    pub const fn mirrors(&self) -> bool {
        self.mirror
    }

    /// The linear part's image of a vector, in 128 bits.
    fn linear(&self, v: [i128; 2]) -> Option<[i128; 2]> {
        let [mut x, mut y] = v;
        for _ in 0..self.turns {
            (x, y) = (y.checked_neg()?, x);
        }
        if self.mirror {
            y = y.checked_neg()?;
        }
        let k = i128::from(self.scale);
        Some([x.checked_mul(k)?, y.checked_mul(k)?])
    }

    /// The map with the linear part of `base` that leaves `about` where it is.
    fn fixing(base: Similarity, about: P2) -> Result<Similarity, EditError> {
        let a = [i128::from(about.x.get()), i128::from(about.y.get())];
        let image = base.linear(a).ok_or(EditError::Range)?;
        let shift = [
            a[0].checked_sub(image[0]).ok_or(EditError::Range)?,
            a[1].checked_sub(image[1]).ok_or(EditError::Range)?,
        ];
        Ok(Similarity {
            shift: [
                i64::try_from(shift[0]).map_err(|_| EditError::Range)?,
                i64::try_from(shift[1]).map_err(|_| EditError::Range)?,
            ],
            ..base
        })
    }

    /// `self` after `first`: the map that does `first`, then `self`.
    ///
    /// # Errors
    ///
    /// [`EditError::Range`] if a number of the composite does not fit.
    pub fn after(&self, first: &Similarity) -> Result<Similarity, EditError> {
        // The linear parts compose: a mirror in `first` reverses the turns of `self`.
        let turns = if first.mirror {
            (first.turns.wrapping_add(4).wrapping_sub(self.turns % 4)) % 4
        } else {
            (first.turns + self.turns) % 4
        };
        let k = first
            .scale
            .checked_mul(self.scale)
            .ok_or(EditError::Range)?;
        let image = self
            .linear([i128::from(first.shift[0]), i128::from(first.shift[1])])
            .ok_or(EditError::Range)?;
        let shift = [
            image[0]
                .checked_add(i128::from(self.shift[0]))
                .ok_or(EditError::Range)?,
            image[1]
                .checked_add(i128::from(self.shift[1]))
                .ok_or(EditError::Range)?,
        ];
        Ok(Similarity {
            turns,
            mirror: first.mirror != self.mirror,
            scale: k,
            shift: [
                i64::try_from(shift[0]).map_err(|_| EditError::Range)?,
                i64::try_from(shift[1]).map_err(|_| EditError::Range)?,
            ],
        })
    }

    /// The image of a grid point.
    ///
    /// # Errors
    ///
    /// [`EditError::Range`] if the image is past the coordinate range.
    pub fn point(&self, p: P2) -> Result<P2, EditError> {
        let v = self
            .linear([i128::from(p.x.get()), i128::from(p.y.get())])
            .ok_or(EditError::Range)?;
        let x = v[0]
            .checked_add(i128::from(self.shift[0]))
            .ok_or(EditError::Range)?;
        let y = v[1]
            .checked_add(i128::from(self.shift[1]))
            .ok_or(EditError::Range)?;
        P2::new(
            i64::try_from(x).map_err(|_| EditError::Range)?,
            i64::try_from(y).map_err(|_| EditError::Range)?,
        )
        .map_err(|_| EditError::Range)
    }

    /// The map that undoes `self`, if it is one on the grid: a scale of one or minus one.
    ///
    /// # Errors
    ///
    /// [`EditError::NotExact`] for any other scale (the inverse would be a division).
    pub fn inverse(&self) -> Result<Similarity, EditError> {
        if self.scale.unsigned_abs() != 1 {
            return Err(EditError::NotExact);
        }
        // Undo the shift, then the linear part: the linear part of a quarter turn, mirror and unit
        // scale has the inverse found by trying the sixteen candidates, which are all there are.
        let probe = [i128::from(1_i8), i128::from(2_i8)];
        let image = self.linear(probe).ok_or(EditError::Range)?;
        for turns in 0..4_u8 {
            for mirror in [false, true] {
                for scale in [1_i64, -1] {
                    let back = Similarity {
                        turns,
                        mirror,
                        scale,
                        shift: [0, 0],
                    };
                    if back.linear(image) == Some(probe) {
                        let moved = back
                            .linear([
                                i128::from(self.shift[0]),
                                i128::from(self.shift[1]),
                            ])
                            .ok_or(EditError::Range)?;
                        let sx = i64::try_from(moved[0].checked_neg().ok_or(EditError::Range)?)
                            .map_err(|_| EditError::Range)?;
                        let sy = i64::try_from(moved[1].checked_neg().ok_or(EditError::Range)?)
                            .map_err(|_| EditError::Range)?;
                        return Ok(Similarity {
                            shift: [sx, sy],
                            ..back
                        });
                    }
                }
            }
        }
        Err(EditError::NotExact)
    }
}

/// Places a polyline through a similarity: every vertex moves, every bulge keeps its value (or
/// changes sign under a mirror), every width scales with the map.
///
/// # Errors
///
/// [`EditError::Range`] if a vertex or width leaves its range.
pub fn place_polyline(poly: &Polyline, map: &Similarity) -> Result<Polyline, EditError> {
    let vertices = poly
        .vertices()
        .iter()
        .map(|p| map.point(*p))
        .collect::<Result<Vec<P2>, EditError>>()?;
    let bulges = poly
        .bulges()
        .iter()
        .map(|b| {
            if map.mirrors() {
                Frac::new(b.numerator().saturating_neg(), b.denominator())
            } else {
                Ok(*b)
            }
        })
        .collect::<Result<Vec<Bulge>, EntityError>>()?;
    let widths = match poly.widths() {
        None => None,
        Some(w) => {
            let k = map.scale.unsigned_abs();
            Some(
                w.iter()
                    .map(|(a, b)| {
                        Ok((
                            Steps(a.0.checked_mul(k).ok_or(EditError::Range)?),
                            Steps(b.0.checked_mul(k).ok_or(EditError::Range)?),
                        ))
                    })
                    .collect::<Result<Vec<(Steps, Steps)>, EditError>>()?,
            )
        }
    };
    Ok(Polyline::new(vertices, bulges, widths, poly.is_closed())?)
}

/// The carrier of the line through two distinct grid points.
fn carrier_through(a: P2, b: P2) -> Result<Line2, EditError> {
    let (ax, ay) = (i128::from(a.x.get()), i128::from(a.y.get()));
    let (bx, by) = (i128::from(b.x.get()), i128::from(b.y.get()));
    let dx = bx.checked_sub(ax).ok_or(EditError::Range)?;
    let dy = by.checked_sub(ay).ok_or(EditError::Range)?;
    if dx == 0 && dy == 0 {
        return Err(EditError::Degenerate);
    }
    let na = dy.checked_neg().ok_or(EditError::Range)?;
    let c = na
        .checked_mul(ax)
        .and_then(|l| dx.checked_mul(ay).and_then(|r| l.checked_add(r)))
        .ok_or(EditError::Range)?;
    Ok(Line2::new(na, dx, c)?)
}

/// Places a shape through a similarity: a grid point, a segment between grid points, a polyline.
///
/// # Errors
///
/// [`EditError::Unsupported`] for any other kind or an end that is not a grid point;
/// [`EditError::Range`] if a coordinate leaves its range.
pub fn place(shape: &Shape, map: &Similarity) -> Result<Shape, EditError> {
    match shape {
        Shape::Point(p) => Ok(Shape::Point(map.point(*p)?)),
        Shape::Line {
            ends: Ends::Segment(Pt2::Grid(a), Pt2::Grid(b)),
            ..
        } => {
            let (a, b) = (map.point(*a)?, map.point(*b)?);
            Ok(Shape::Line {
                carrier: carrier_through(a, b)?,
                ends: Ends::Segment(Pt2::Grid(a), Pt2::Grid(b)),
            })
        }
        Shape::Polyline(poly) => Ok(Shape::Polyline(place_polyline(poly, map)?)),
        other => Err(EditError::Unsupported(other.tag().as_u8())),
    }
}

/// The map that takes `p0` to `q0` and `p1` to `q1` by a quarter turn, a whole scale and a shift.
///
/// # Errors
///
/// [`EditError::Degenerate`] if `p0` = `p1` or `q0` = `q1`; [`EditError::NotExact`] if no quarter
/// turn and whole scale takes the vector `p0 p1` to `q0 q1`.
pub fn align(p0: P2, p1: P2, q0: P2, q1: P2) -> Result<Similarity, EditError> {
    let diff = |a: P2, b: P2| {
        [
            i128::from(b.x.get()) - i128::from(a.x.get()),
            i128::from(b.y.get()) - i128::from(a.y.get()),
        ]
    };
    let (v, w) = (diff(p0, p1), diff(q0, q1));
    if v == [0, 0] || w == [0, 0] {
        return Err(EditError::Degenerate);
    }
    for turns in 0..4_u8 {
        for scale in [1_i64, -1] {
            let probe = Similarity {
                turns,
                mirror: false,
                scale: 1,
                shift: [0, 0],
            };
            let Some(r) = probe.linear(v) else {
                continue;
            };
            // w must be a whole multiple of r: w = k r.
            let k = if r[0] != 0 {
                w[0].checked_div(r[0])
            } else {
                w[1].checked_div(r[1])
            };
            let Some(k) = k.filter(|k| *k >= 1) else {
                continue;
            };
            if r[0].checked_mul(k) != Some(w[0]) || r[1].checked_mul(k) != Some(w[1]) {
                continue;
            }
            let _ = scale;
            let linear = Similarity {
                scale: i64::try_from(k).map_err(|_| EditError::Range)?,
                ..probe
            };
            let image = linear
                .linear([i128::from(p0.x.get()), i128::from(p0.y.get())])
                .ok_or(EditError::Range)?;
            let shift = [
                i64::try_from(i128::from(q0.x.get()) - image[0]).map_err(|_| EditError::Range)?,
                i64::try_from(i128::from(q0.y.get()) - image[1]).map_err(|_| EditError::Range)?,
            ];
            return Ok(Similarity { shift, ..linear });
        }
    }
    Err(EditError::NotExact)
}

/// The window of a stretch: the rectangle of grid points with both coordinates between the corners,
/// inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// The corner with the least coordinates.
    pub low: P2,
    /// The corner with the greatest coordinates.
    pub high: P2,
}

impl Window {
    /// Whether `p` is inside or on the edge.
    #[must_use]
    pub fn holds(&self, p: P2) -> bool {
        p.x.get() >= self.low.x.get()
            && p.x.get() <= self.high.x.get()
            && p.y.get() >= self.low.y.get()
            && p.y.get() <= self.high.y.get()
    }
}

/// Moves the vertices of `poly` inside `window` by (`dx`, `dy`). A bulge keeps its value, so an arc
/// whose ends both move is moved exactly and an arc with one end moved keeps its sweep exactly (its
/// circle changes with its chord).
///
/// # Errors
///
/// [`EditError::Range`] if a moved vertex leaves the range.
pub fn stretch(poly: &Polyline, window: &Window, dx: i64, dy: i64) -> Result<Polyline, EditError> {
    let shift = Similarity::translation(dx, dy);
    let vertices = poly
        .vertices()
        .iter()
        .map(|p| if window.holds(*p) { shift.point(*p) } else { Ok(*p) })
        .collect::<Result<Vec<P2>, EditError>>()?;
    Ok(Polyline::new(
        vertices,
        poly.bulges().to_vec(),
        poly.widths().map(<[(Steps, Steps)]>::to_vec),
        poly.is_closed(),
    )?)
}

/// A rectangular array of `rows` by `columns` copies, spaced by (`dx`, `dy`) along columns and
/// (`row_dx`, `row_dy`) along rows, in row-major order, the original first.
///
/// # Errors
///
/// [`EditError::Degenerate`] for no rows or no columns; whatever [`place`] refuses.
pub fn array_rectangular(
    shape: &Shape,
    rows: u32,
    columns: u32,
    step: [i64; 2],
    row_step: [i64; 2],
) -> Result<Vec<Shape>, EditError> {
    if rows == 0 || columns == 0 {
        return Err(EditError::Degenerate);
    }
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let dx = i64::from(c)
                .checked_mul(step[0])
                .and_then(|a| i64::from(r).checked_mul(row_step[0]).and_then(|b| a.checked_add(b)))
                .ok_or(EditError::Range)?;
            let dy = i64::from(c)
                .checked_mul(step[1])
                .and_then(|a| i64::from(r).checked_mul(row_step[1]).and_then(|b| a.checked_add(b)))
                .ok_or(EditError::Range)?;
            out.push(place(shape, &Similarity::translation(dx, dy))?);
        }
    }
    Ok(out)
}

/// A polar array of `copies` about `centre`, each turned by 360 / `copies` degrees: exact only for
/// one, two and four copies (whole quarter turns), the original first.
///
/// # Errors
///
/// [`EditError::Degenerate`] for no copies; [`EditError::NotExact`] for any count but 1, 2 or 4.
pub fn array_polar(shape: &Shape, centre: P2, copies: u32) -> Result<Vec<Shape>, EditError> {
    let step = match copies {
        0 => return Err(EditError::Degenerate),
        1 | 4 => 4_u32.checked_div(copies).ok_or(EditError::Degenerate)?,
        2 => 2,
        _ => return Err(EditError::NotExact),
    };
    (0..copies)
        .map(|i| {
            let turns = u8::try_from(i.checked_mul(step).ok_or(EditError::Range)? % 4)
                .map_err(|_| EditError::Range)?;
            place(shape, &Similarity::quarter_turns(turns, centre)?)
        })
        .collect()
}

/// Which end of a segment a lengthening moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The first end.
    Start,
    /// The second end.
    Finish,
}

fn gcd(a: u128, b: u128) -> u128 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let r = a.checked_rem(b).unwrap_or(0);
        a = b;
        b = r;
    }
    a
}

/// The segment from `a` to `b` with the end `end` moved along its carrier by `change` whole steps
/// of the primitive direction (the direction divided by the greatest common divisor of its parts);
/// a negative `change` shortens. The new end is an exact grid point on the carrier.
///
/// # Errors
///
/// [`EditError::Degenerate`] for equal ends, or a change that carries an end through the other;
/// [`EditError::Range`] past the coordinate range.
pub fn lengthen(a: P2, b: P2, end: End, change: i64) -> Result<(P2, P2), EditError> {
    let dx = i128::from(b.x.get()) - i128::from(a.x.get());
    let dy = i128::from(b.y.get()) - i128::from(a.y.get());
    let g = i128::try_from(gcd(dx.unsigned_abs(), dy.unsigned_abs())).map_err(|_| EditError::Range)?;
    if g == 0 {
        return Err(EditError::Degenerate);
    }
    let (ux, uy) = (dx / g, dy / g);
    // The segment holds g primitive steps; the change may not use them all.
    let (sign, moved) = match end {
        End::Start => (-1_i128, a),
        End::Finish => (1_i128, b),
    };
    let c = i128::from(change);
    if c.checked_neg().is_some_and(|n| n >= g) {
        return Err(EditError::Degenerate);
    }
    let nx = i128::from(moved.x.get())
        .checked_add(sign.checked_mul(c).and_then(|s| s.checked_mul(ux)).ok_or(EditError::Range)?)
        .ok_or(EditError::Range)?;
    let ny = i128::from(moved.y.get())
        .checked_add(sign.checked_mul(c).and_then(|s| s.checked_mul(uy)).ok_or(EditError::Range)?)
        .ok_or(EditError::Range)?;
    let new = P2::new(
        i64::try_from(nx).map_err(|_| EditError::Range)?,
        i64::try_from(ny).map_err(|_| EditError::Range)?,
    )
    .map_err(|_| EditError::Range)?;
    Ok(match end {
        End::Start => (new, b),
        End::Finish => (a, new),
    })
}

/// The `parts` − 1 points that cut the segment from `a` to `b` into equal parts, as exact rational
/// points, in order from `a`.
///
/// # Errors
///
/// [`EditError::Degenerate`] for fewer than two parts or equal ends; [`EditError::Range`] if the
/// rational points leave the exact budget.
pub fn divide(a: P2, b: P2, parts: u32) -> Result<Vec<Rational2>, EditError> {
    if parts < 2 || (a.x.get() == b.x.get() && a.y.get() == b.y.get()) {
        return Err(EditError::Degenerate);
    }
    let n = i128::from(parts);
    (1..parts)
        .map(|i| {
            let i = i128::from(i);
            let x = i128::from(a.x.get())
                .checked_mul(n - i)
                .and_then(|l| i128::from(b.x.get()).checked_mul(i).and_then(|r| l.checked_add(r)))
                .ok_or(EditError::Range)?;
            let y = i128::from(a.y.get())
                .checked_mul(n - i)
                .and_then(|l| i128::from(b.y.get()).checked_mul(i).and_then(|r| l.checked_add(r)))
                .ok_or(EditError::Range)?;
            Ok(Rational2::new(x, y, n)?)
        })
        .collect()
}

/// The grid points along the segment from `a` to `b` at every `every` primitive steps from `a`,
/// not counting `a`, and not past `b`.
///
/// # Errors
///
/// [`EditError::Degenerate`] for equal ends or `every` of zero.
pub fn measure(a: P2, b: P2, every: u32) -> Result<Vec<P2>, EditError> {
    let dx = i128::from(b.x.get()) - i128::from(a.x.get());
    let dy = i128::from(b.y.get()) - i128::from(a.y.get());
    let g = i128::try_from(gcd(dx.unsigned_abs(), dy.unsigned_abs())).map_err(|_| EditError::Range)?;
    if g == 0 || every == 0 {
        return Err(EditError::Degenerate);
    }
    let (ux, uy) = (dx / g, dy / g);
    let mut out = Vec::new();
    let mut at = i128::from(every);
    while at <= g {
        let x = i128::from(a.x.get()) + ux * at;
        let y = i128::from(a.y.get()) + uy * at;
        out.push(
            P2::new(
                i64::try_from(x).map_err(|_| EditError::Range)?,
                i64::try_from(y).map_err(|_| EditError::Range)?,
            )
            .map_err(|_| EditError::Range)?,
        );
        at += i128::from(every);
    }
    Ok(out)
}

/// An interval that holds the integer `v` exactly.
fn holding(v: i128) -> Result<Interval, EditError> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "the interval is widened by a step each way, which holds the rounding"
    )]
    let x = v as f64;
    Ok(Interval::new(x.next_down(), x.next_up())?)
}

/// The length of the segment of chord (`dx`, `dy`) and bulge `bulge`, as an interval that holds it.
fn segment_length(dx: i128, dy: i128, bulge: &Bulge) -> Result<Interval, EditError> {
    let square = dx
        .checked_mul(dx)
        .and_then(|a| dy.checked_mul(dy).and_then(|b| a.checked_add(b)))
        .ok_or(EditError::Range)?;
    let chord = holding(square)?.sqrt()?;
    if bulge.is_zero() {
        return Ok(chord);
    }
    let b = holding(i128::from(bulge.numerator().unsigned_abs()))?
        .div(holding(i128::from(bulge.denominator()))?)?;
    let one = holding(1)?;
    let angle = b.atan2(one)?;
    // c (1 + b²) atan |b| / |b|
    let factor = one.add(b.square()?)?;
    Ok(chord.mul(factor)?.mul(angle)?.div(b)?)
}

/// The arc length of a polyline as an interval that holds it.
///
/// # Errors
///
/// [`EditError::Range`] if a squared length leaves the exact range.
pub fn polyline_length(poly: &Polyline) -> Result<Interval, EditError> {
    let v = poly.vertices();
    let mut total = Interval::point(0.0)?;
    for (i, bulge) in poly.bulges().iter().enumerate() {
        let next = i.checked_add(1).and_then(|n| n.checked_rem(v.len())).ok_or(EditError::Range)?;
        let (Some(a), Some(b)) = (v.get(i), v.get(next)) else {
            return Err(EditError::Range);
        };
        let dx = i128::from(b.x.get()) - i128::from(a.x.get());
        let dy = i128::from(b.y.get()) - i128::from(a.y.get());
        total = total.add(segment_length(dx, dy, bulge)?)?;
    }
    Ok(total)
}
