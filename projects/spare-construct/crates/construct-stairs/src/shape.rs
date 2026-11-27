//! Where every piece of a laid-out stair is, as a solid on the kernel's grid: one for each run
//! and one for each landing, in its base's frame (the stair's base at `z = 0`, the plan the
//! model's), every corner a grid point; and the headroom where one piece passes over another.
//!
//! **In plan.** The first run starts at the stair's `start`, on its right-hand side, and goes the
//! way of its `direction`; the stair lies to its left, `width` across. A run of `m` risers is
//! `(m - 1) * going` long. A landing is `length` long along the run that comes into it and as
//! wide as the stair, or twice as wide for a half turn, which brings the next run back beside the
//! last (no well between them). The next run starts at the landing's far edge (straight on), at
//! its left or right side's far end (a quarter turn), or beside the run before (a half turn).
//! Everything runs along the plan's axes, so every corner is the start moved by whole lengths.
//!
//! **A run's solid** is its side profile lofted across its width: the profile at its right-hand
//! side, and the same moved square to the left by the width (`Engine::loft` of two sections, every
//! wall of which is flat). In the run's frame (`a` along it from its first riser, `z` up):
//!
//! - its steps: the first riser at `a = 0` from its base `zb` up, then each tread and the riser
//!   above it, to the inner corner of its last tread at `a = (m - 1) * going`; its top riser
//!   belongs to the landing or the floor above;
//! - its underside: the line of the steps' inner corners lowered square to itself by the waist,
//!   `D = round(waist * sqrt(L^2 + h^2) / L)` below it measured upright, `L` the run's length and
//!   `h` the rise from its first to its last nosing (its slope `h / L`), the one exact integer
//!   square root of the nearest-step rule; the underside meets the run's base at a point rounded
//!   to the grid once, and the run is a solid block where the waist reaches below its base;
//! - its base: the floor (`z = 0`) for the first run; for a run after a landing, the landing's
//!   underside, so the two meet flush;
//! - its top end: under a landing the run reaches on below the landing until its underside meets
//!   the landing's underside (that point rounded once, and never past the landing's far edge), so
//!   the run and the landing meet as one piece of concrete and the run's part of its top riser
//!   is closed; the last run ends square at its last inner corner, its top riser being the floor
//!   above's edge.
//!
//! **A landing's solid** is its outline extruded straight up from its underside, `waist` below
//! its top, to its top.
//!
//! **Headroom.** Where one piece passes over another that is not next to it, the clear height
//! from the lower piece's pitch line (the line of its nosings; a landing's top) up to the upper
//! piece's underside is at least the rule set's least headroom, everywhere over the two in plan:
//! worked exactly at the corners of where they overlap (each run's underside is straight on each
//! side of where it meets its base, so the corners of those parts are enough).

use crate::error::StairError;
use crate::layout::Layout;
use crate::stair::{Direction, Stair, Turn};
use construct_params::Length;

/// The material every piece is drawn in: construct-materials' default concrete. A stair type
/// (later) will carry its own.
pub const MATERIAL: &str = "concrete-regular";

/// A solid, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Solid {
    /// The solid lofted from the first section to the second (`Engine::loft`): a run.
    Loft {
        /// The two sections, of one count of corners, each the other moved.
        sections: [Vec<[i64; 3]>; 2],
    },
    /// The profile swept along `by` (`Engine::extrude`): a landing.
    Extrude {
        /// The profile's corners, anticlockwise from above.
        profile: Vec<[i64; 3]>,
        /// The sweep.
        by: [i64; 3],
    },
}

/// What a piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    /// A run, with its count of treads.
    Run {
        /// How many treads it has.
        treads: usize,
    },
    /// A landing.
    Landing,
}

/// One piece of a stair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Its ID: `run.<k>` or `landing.<k>`, `k` from 1 at the bottom.
    pub id: String,
    /// What it is.
    pub kind: PieceKind,
    /// Its outline in plan, anticlockwise from above: the four corners of what it covers.
    pub plan: [[i64; 2]; 4],
    /// Its solid.
    pub solid: Solid,
}

/// The shape of a stair: its pieces, from the bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The pieces, a run, a landing, a run and so on.
    pub pieces: Vec<Piece>,
}

fn range(what: &str) -> StairError {
    StairError::OutOfRange {
        what: what.to_owned(),
    }
}

/// `a - b`, exactly: two `i64`s differ by less than an `i128` holds.
fn diff(a: i64, b: i64) -> i128 {
    i128::from(a).saturating_sub(i128::from(b))
}

/// `p + k * v`, checked.
fn moved(p: [i64; 2], v: [i64; 2], k: i64) -> Result<[i64; 2], StairError> {
    let step = |c: i64, d: i64| {
        d.checked_mul(k)
            .and_then(|m| c.checked_add(m))
            .ok_or_else(|| range("a corner of the stair"))
    };
    Ok([step(p[0], v[0])?, step(p[1], v[1])?])
}

/// The nearest whole number to `p / q` for `q > 0`, a half up.
fn round_div(p: i128, q: i128) -> Result<i128, StairError> {
    p.checked_mul(2)
        .and_then(|twice| twice.checked_add(q))
        .and_then(|n| n.checked_div_euclid(q.checked_mul(2)?))
        .ok_or_else(|| range("a point of a run's underside"))
}

/// How far a run's underside stands below the line of its inner corners, measured upright, for a
/// waist square to it: `round(waist * sqrt(l^2 + h^2) / l)` steps, exactly (the module header).
fn upright_waist(waist: i64, l: i64, h: i64) -> Result<i64, StairError> {
    let too_big = || range("a run's waist");
    let (w, l, h) = (
        u128::try_from(waist).map_err(|_| too_big())?,
        u128::try_from(l).map_err(|_| too_big())?,
        u128::try_from(h).map_err(|_| too_big())?,
    );
    // nearest(sqrt(X) / l) = (isqrt(4X) + l) div 2l, for X = w^2 (l^2 + h^2).
    let four_x = l
        .checked_mul(l)
        .and_then(|ll| h.checked_mul(h).and_then(|hh| ll.checked_add(hh)))
        .and_then(|s| w.checked_mul(w).and_then(|ww| ww.checked_mul(s)))
        .and_then(|x| x.checked_mul(4))
        .ok_or_else(too_big)?;
    let root = four_x.isqrt();
    let d = root
        .checked_add(l)
        .and_then(|n| n.checked_div(l.checked_mul(2)?))
        .ok_or_else(too_big)?;
    i64::try_from(d).map_err(|_| too_big())
}

/// A run's profile in its own frame, `(a, z)` in grid steps, as the module header says, and how
/// far along it reaches.
struct Profile {
    corners: Vec<[i64; 2]>,
    reach: i64,
    /// Where its underside meets its base, and the underside's top end: the bottom boundary is
    /// `zb` up to `foot.0`, then the straight line to `top`.
    foot: [i64; 2],
    top: [i64; 2],
}

/// The profile of a run whose risers' tops are `z` (from its first to its last), on base `zb`,
/// `going` deep, under a landing whose underside is at `cap` and which reaches `room` past the
/// run's end, or under the floor above (`cap` none).
fn profile(
    run: usize,
    z: &[i64],
    zb: i64,
    going: i64,
    waist: i64,
    cap: Option<(i64, i64)>,
) -> Result<Profile, StairError> {
    let count = z.len();
    let (Some(&z1), Some(&zm)) = (z.first(), z.last()) else {
        return Err(range("a run with no risers"));
    };
    let below_top = z.get(count.saturating_sub(2)).copied().unwrap_or(zb);
    let treads = i64::try_from(count.saturating_sub(1)).map_err(|_| range("a run"))?;
    let length = going.checked_mul(treads).ok_or_else(|| range("a run"))?;
    let climb = zm.checked_sub(z1).ok_or_else(|| range("a run"))?;
    if length <= 0 || climb <= 0 {
        return Err(range("a run of fewer than two risers"));
    }
    let drop = upright_waist(waist, length, climb)?;
    let mut corners = vec![[0, zb]];
    for (i, &zi) in z.iter().enumerate().take(count.saturating_sub(1)) {
        let i = i64::try_from(i).map_err(|_| range("a run"))?;
        let nosing = going.checked_mul(i).ok_or_else(|| range("a run"))?;
        let inner = nosing.checked_add(going).ok_or_else(|| range("a run"))?;
        corners.push([nosing, zi]);
        corners.push([inner, zi]);
    }
    // The underside: z = below_top - drop + climb (a - length) / length. Where it reaches height
    // `at`: a = length + (at - below_top + drop) length / climb, rounded once.
    let reaching = |at: i64| -> Result<i64, StairError> {
        let rise = i128::from(at)
            .checked_sub(i128::from(below_top))
            .and_then(|r| r.checked_add(i128::from(drop)))
            .ok_or_else(|| range("a run's underside"))?;
        let along = rise
            .checked_mul(i128::from(length))
            .ok_or_else(|| range("a run's underside"))?;
        let a = round_div(along, i128::from(climb))?
            .checked_add(i128::from(length))
            .ok_or_else(|| range("a run's underside"))?;
        i64::try_from(a).map_err(|_| range("a run's underside"))
    };
    let top = match cap {
        Some((zu, room)) if zu > zb => {
            if zu != below_top {
                corners.push([length, zu]);
            }
            let far = length.checked_add(room).ok_or_else(|| range("a landing"))?;
            let a = reaching(zu)?.clamp(length, far);
            if a > length {
                corners.push([a, zu]);
            }
            [a, zu]
        }
        _ => {
            let low = below_top.checked_sub(drop).ok_or_else(|| range("a run"))?;
            if low > zb {
                corners.push([length, low]);
                [length, low]
            } else {
                corners.push([length, zb]);
                let reach = length;
                return Ok(Profile {
                    corners,
                    reach,
                    foot: [length, zb],
                    top: [length, zb],
                });
            }
        }
    };
    let foot_a = reaching(zb)?.clamp(0, top[0]);
    if foot_a > 0 {
        corners.push([foot_a, zb]);
    }
    let shape = Profile {
        corners,
        reach: top[0],
        foot: [foot_a, zb],
        top,
    };
    // Every inner corner stands above the underside, or the waist is too thin to pass under it.
    for (i, &zi) in z.iter().enumerate().take(count.saturating_sub(1)) {
        let i = i64::try_from(i)
            .ok()
            .and_then(|i| i.checked_add(1))
            .ok_or_else(|| range("a run"))?;
        let a = going.checked_mul(i).ok_or_else(|| range("a run"))?;
        if a > shape.foot[0] && !above_underside(&shape, a, zi)? {
            return Err(StairError::WaistTooThin {
                run,
                waist: Length::from_steps(waist),
            });
        }
    }
    Ok(shape)
}

/// Whether the point `(a, z)`, with `a` past the underside's foot, stands strictly above it.
fn above_underside(p: &Profile, a: i64, z: i64) -> Result<bool, StairError> {
    let [a0, z0] = p.foot;
    let [a1, z1] = p.top;
    if a1 <= a0 {
        return Ok(z > z0.max(z1));
    }
    // z > z0 + (z1 - z0)(a - a0)/(a1 - a0), crosswise.
    let left = diff(z, z0).checked_mul(diff(a1, a0));
    let right = diff(z1, z0).checked_mul(diff(a, a0));
    match (left, right) {
        (Some(left), Some(right)) => Ok(left > right),
        _ => Err(range("a run's underside")),
    }
}

/// A run's walking surface and underside, for the headroom: in its frame.
struct Span {
    name: String,
    origin: [i64; 2],
    along: [i64; 2],
    across: [i64; 2],
    /// How far it reaches along, and across.
    reach: i64,
    width: i64,
    /// The walking surface: from `(0, from)` to `(walk, to)`, the pitch line of a run or a
    /// landing's top (`from == to`); `walk` how far along it is walked.
    walk: i64,
    from: i64,
    to: i64,
    /// The underside: `base` up to `foot`, then straight to `top`; a landing's is flat.
    base: i64,
    foot: [i64; 2],
    top: [i64; 2],
}

/// A height as a fraction `num / den`, `den > 0`.
#[derive(Clone, Copy)]
struct Frac {
    num: i128,
    den: i128,
}

impl Frac {
    fn whole(v: i64) -> Self {
        Self {
            num: i128::from(v),
            den: 1,
        }
    }

    /// The value at `a` of the line from `(a0, z0)` to `(a1, z1)`, `a1 > a0`.
    fn on_line(a0: i64, z0: i64, a1: i64, z1: i64, a: i64) -> Option<Self> {
        let den = diff(a1, a0);
        let num = i128::from(z0)
            .checked_mul(den)?
            .checked_add(diff(z1, z0).checked_mul(diff(a, a0))?)?;
        Some(Self { num, den })
    }

    /// `self - other`, as `num / den`.
    fn less(self, other: Self) -> Option<Self> {
        Some(Self {
            num: self
                .num
                .checked_mul(other.den)?
                .checked_sub(other.num.checked_mul(self.den)?)?,
            den: self.den.checked_mul(other.den)?,
        })
    }
}

impl Span {
    /// Where a plan point is along this span's run, from its origin, or none past the grid.
    fn along_of(&self, p: [i64; 2]) -> Option<i64> {
        let dx = p[0].checked_sub(self.origin[0])?;
        let dy = p[1].checked_sub(self.origin[1])?;
        dx.checked_mul(self.along[0])?
            .checked_add(dy.checked_mul(self.along[1])?)
    }

    /// The plan box it covers out to `reach` along, `(lo, hi)`.
    fn plan(&self, reach: i64) -> Result<([i64; 2], [i64; 2]), StairError> {
        let near = self.origin;
        let far = moved(near, self.along, reach)?;
        let corners = [
            near,
            far,
            moved(far, self.across, self.width)?,
            moved(near, self.across, self.width)?,
        ];
        let lo = corners.iter().fold([i64::MAX, i64::MAX], |m, c| {
            [m[0].min(c[0]), m[1].min(c[1])]
        });
        let hi = corners.iter().fold([i64::MIN, i64::MIN], |m, c| {
            [m[0].max(c[0]), m[1].max(c[1])]
        });
        Ok((lo, hi))
    }

    /// The height of its walking surface `a` along it.
    fn walk_at(&self, a: i64) -> Option<Frac> {
        if self.walk <= 0 || self.from == self.to {
            return Some(Frac::whole(self.from));
        }
        Frac::on_line(0, self.from, self.walk, self.to, a)
    }

    /// The height of its underside `a` along it.
    fn under_at(&self, a: i64) -> Option<Frac> {
        if a <= self.foot[0] || self.top[0] <= self.foot[0] {
            return Some(Frac::whole(self.base));
        }
        Frac::on_line(self.foot[0], self.foot[1], self.top[0], self.top[1], a)
    }
}

/// Whether `a < b` for fractions with positive denominators.
fn below(a: Frac, b: Frac) -> Result<bool, StairError> {
    let (l, r) = a
        .num
        .checked_mul(b.den)
        .zip(b.num.checked_mul(a.den))
        .ok_or_else(|| range("a clearance"))?;
    Ok(l < r)
}

/// The least clear height from `lower`'s walking surface up to `upper`'s underside where they
/// overlap in plan, or none where they do not overlap.
fn clearance(lower: &Span, upper: &Span) -> Result<Option<Frac>, StairError> {
    let (llo, lhi) = lower.plan(lower.walk)?;
    let (ulo, uhi) = upper.plan(upper.reach)?;
    let lo = [llo[0].max(ulo[0]), llo[1].max(ulo[1])];
    let hi = [lhi[0].min(uhi[0]), lhi[1].min(uhi[1])];
    if lo[0] >= hi[0] || lo[1] >= hi[1] {
        return Ok(None);
    }
    // The corners of the overlap, and the line across it where the upper's underside meets its
    // base: on each side of that line both heights are straight, so their least gap is at a
    // corner.
    let mut xs = vec![lo[0], hi[0]];
    let mut ys = vec![lo[1], hi[1]];
    let kink = moved(upper.origin, upper.along, upper.foot[0])?;
    if upper.along[0] != 0 && kink[0] > lo[0] && kink[0] < hi[0] {
        xs.push(kink[0]);
    }
    if upper.along[1] != 0 && kink[1] > lo[1] && kink[1] < hi[1] {
        ys.push(kink[1]);
    }
    let mut least: Option<Frac> = None;
    for &x in &xs {
        for &y in &ys {
            let gap = lower
                .along_of([x, y])
                .zip(upper.along_of([x, y]))
                .and_then(|(a_low, a_up)| upper.under_at(a_up).zip(lower.walk_at(a_low)))
                .and_then(|(under, walk)| under.less(walk))
                .ok_or_else(|| range("a clearance"))?;
            least = Some(match least {
                Some(l) if !below(gap, l)? => l,
                _ => gap,
            });
        }
    }
    Ok(least)
}

/// The shape of a stair laid out as `layout`, as the module header says.
///
/// # Errors
/// [`StairError::WaistTooThin`] for a waist whose underside does not pass below every step;
/// [`StairError::Headroom`] where one piece passes over another with too little headroom;
/// [`StairError::OutOfRange`] for a corner beyond what the grid holds.
#[allow(
    clippy::too_many_lines,
    reason = "the runs and landings laid end to end in one place, as the module header lists them"
)]
pub fn shape(stair: &Stair, layout: &Layout) -> Result<Shape, StairError> {
    let width = layout.width.steps();
    let going = layout.going.steps();
    let waist = layout.waist.steps();
    let twice = width.checked_mul(2).ok_or_else(|| range("the width"))?;
    let mut dir: Direction = stair.direction();
    let [sx, sy] = stair.start();
    let mut foot = [sx.steps(), sy.steps()];
    let mut pieces = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    for (k, run) in layout.runs.iter().enumerate() {
        let number = k.saturating_add(1);
        let along = dir.unit();
        let left = dir.left().unit();
        let length = run.length.steps();
        let base = run.base.steps();
        let zb = if k == 0 {
            0
        } else {
            base.checked_sub(waist).ok_or_else(|| range("a run"))?
        };
        let mut z = Vec::with_capacity(run.risers);
        let mut height = base;
        for riser in layout
            .risers
            .get(run.first..run.first.saturating_add(run.risers))
            .unwrap_or_default()
        {
            height = height
                .checked_add(riser.steps())
                .ok_or_else(|| range("a run"))?;
            z.push(height);
        }
        let landing = layout.landings.get(k);
        let cap = match landing {
            Some(landing) => Some((
                height
                    .checked_sub(waist)
                    .ok_or_else(|| range("a landing"))?,
                landing.length.steps(),
            )),
            None => None,
        };
        let profile = profile(number, &z, zb, going, waist, cap)?;
        let far = moved(foot, along, profile.reach)?;
        let mut right = Vec::with_capacity(profile.corners.len());
        let mut other = Vec::with_capacity(profile.corners.len());
        for &[a, h] in &profile.corners {
            let p = moved(foot, along, a)?;
            let q = moved(p, left, width)?;
            right.push([p[0], p[1], h]);
            other.push([q[0], q[1], h]);
        }
        pieces.push(Piece {
            id: format!("run.{number}"),
            kind: PieceKind::Run {
                treads: run.risers.saturating_sub(1),
            },
            plan: [
                foot,
                far,
                moved(far, left, width)?,
                moved(foot, left, width)?,
            ],
            solid: Solid::Loft {
                sections: [right, other],
            },
        });
        spans.push(Span {
            name: format!("run {number}"),
            origin: foot,
            along,
            across: left,
            reach: profile.reach,
            width,
            walk: length,
            from: z.first().copied().unwrap_or(zb),
            to: z.last().copied().unwrap_or(zb),
            base: zb,
            foot: profile.foot,
            top: profile.top,
        });
        let Some(landing) = landing else {
            continue;
        };
        let end = moved(foot, along, length)?;
        let long = landing.length.steps();
        let top = landing.height.steps();
        let under = top.checked_sub(waist).ok_or_else(|| range("a landing"))?;
        let ahead = moved(end, along, long)?;
        let (from, across, next_foot, next_dir) = match landing.turn {
            Turn::Straight => (end, width, ahead, dir),
            Turn::Left => (end, width, moved(ahead, left, width)?, dir.left()),
            Turn::Right => (
                end,
                width,
                moved(ahead, along, width.saturating_neg())?,
                dir.right(),
            ),
            Turn::BackLeft => (end, twice, moved(end, left, twice)?, dir.back()),
            Turn::BackRight => (
                moved(end, left, width.saturating_neg())?,
                twice,
                end,
                dir.back(),
            ),
        };
        let near_far = moved(from, along, long)?;
        let outline = [
            from,
            near_far,
            moved(near_far, left, across)?,
            moved(from, left, across)?,
        ];
        pieces.push(Piece {
            id: format!("landing.{number}"),
            kind: PieceKind::Landing,
            plan: outline,
            solid: Solid::Extrude {
                profile: outline.iter().map(|p| [p[0], p[1], under]).collect(),
                by: [0, 0, waist],
            },
        });
        spans.push(Span {
            name: format!("landing {number}"),
            origin: from,
            along,
            across: left,
            reach: long,
            width: across,
            walk: long,
            from: top,
            to: top,
            base: under,
            foot: [0, under],
            top: [0, under],
        });
        foot = next_foot;
        dir = next_dir;
    }
    // Headroom: a piece over one that is not next to it, from the lower's walking surface up to
    // the upper's underside.
    let least = Frac::whole(layout.headroom.length.steps());
    for (i, lower) in spans.iter().enumerate() {
        for upper in spans.iter().skip(i.saturating_add(2)) {
            let Some(gap) = clearance(lower, upper)? else {
                continue;
            };
            if below(gap, least)? {
                let steps = gap.num.checked_div_euclid(gap.den).unwrap_or(i128::MIN);
                return Err(StairError::Headroom {
                    lower: lower.name.clone(),
                    upper: upper.name.clone(),
                    clearance: Length::from_steps(i64::try_from(steps).unwrap_or(i64::MIN)),
                    least: layout.headroom.named("the least headroom above it"),
                });
            }
        }
    }
    Ok(Shape { pieces })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::arithmetic_side_effects,
        clippy::indexing_slicing,
        reason = "test code; the rules are for the crate's own code"
    )]

    use super::*;

    /// A run whose risers are far from even (as no layout gives, but a caller of the profile
    /// could): its second inner corner, 101 steps up, lies far below the underside the waist and
    /// the slope from its first nosing to its last put under its last inner corner, so it is
    /// refused rather than drawn folded.
    #[test]
    fn a_run_whose_underside_would_cut_a_step_is_refused() {
        let err = profile(3, &[100, 101, 300, 301], 0, 100, 1, None)
            .err()
            .unwrap();
        assert_eq!(
            err,
            StairError::WaistTooThin {
                run: 3,
                waist: Length::from_steps(1)
            }
        );
        // Even risers pass with the same waist.
        assert!(profile(3, &[100, 200, 300, 400], 0, 100, 1, None).is_ok());
    }
}
