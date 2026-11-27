//! A railing laid out along its lines: where every baluster stands, how long its rails are, where
//! its handrail runs, and how wide every opening is, each worked out exactly and rounded once
//! ([`crate::exact`]).
//!
//! The rule ([`lay_out`]), for each piece of line between two vertices (two posts):
//!
//! 1. **Balusters.** On a piece `len` long in plan, at spacing `s`, there are `ceil(len / s) - 1`
//!    of them, however they are justified: from the start each is `k s` along (`k` from 1); from
//!    the end `len - k s`; centred, `len / 2 + (k - (n + 1) / 2) s`, so the gaps at both ends are
//!    alike. A 3 m piece at 100 mm takes 29 however justified; 3.05 m takes 30 centred, 75 mm from
//!    each end. One that would stand inside a post is left out: nearer a post's centre than half the
//!    post's largest reach (half its width plus half its depth, or its radius) and half the
//!    baluster's depth along the path; so 3.05 m from the start keeps 29, its 30th 50 mm from the end
//!    post's centre being inside a 50 mm post.
//! 2. **Heights.** A baluster's foot is `bottom` above the floor, or on a flight above the pitch
//!    line where it stands; its top is the top rail's underside there, measured upright: `height`
//!    less the rail's depth, which on a slope is its depth stretched by `sqrt(run^2 + rise^2) / run`
//!    so the rail's true section is its profile. A baluster with no height left is refused.
//! 3. **Openings.** Between two upright members (post or baluster) the clear gap along the path:
//!    the distance between their centres less half of each one's depth along it (a post's least
//!    half side, or its radius, so a gap is never taken as narrower than it is). Under the
//!    balusters on the level, `bottom`. Under them on a flight, the opening of a tread, the riser
//!    above it and the balusters' foot line: the largest disc in it is the one in the corner of tread
//!    and riser, of diameter `(r + b) (g + r - sqrt(g^2 + r^2)) / r` for the flight's tallest riser
//!    `r`, its going `g` and `bottom` `b`, while that disc touches the riser below its top (its
//!    diameter at most `2 r`); a disc that would reach above the riser is taken as the opening's
//!    whole upright height there, `b + r`, which no disc in it exceeds. The upright opening under
//!    the balusters is `b` on the level and `b + r` on a flight.
//! 4. **Lengths.** Each piece of rail is `sqrt(len^2 + rise^2)` long, rounded once; a railing's rail
//!    length is the sum of its pieces' (so within half a step of the exact sum for each piece).
//! 5. **The handrail** runs `offset` in from the line on its inner side ([`crate::path::Hand`]):
//!    each vertex moved to where the two lines `offset` in from its pieces meet (the mitre), worked
//!    out exactly from both pieces' lengths and rounded once; its first and last vertex square to
//!    their piece.

use crate::error::RailingError;
use crate::exact::{Real, term};
use crate::kind::{Justify, Profile, RailingType};
use crate::path::{Flight, Hand, Line};
use construct_params::Length;
use num_bigint::BigInt;
use std::cmp::Ordering;

/// The most balusters a railing takes, in all.
pub const MOST_BALUSTERS: usize = 20_000;

/// What an opening is between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Between {
    /// Two upright members: posts or balusters.
    Members,
    /// The floor (or a landing) and the balusters' foot.
    Floor,
    /// A flight's treads and risers and the balusters' foot.
    Flight,
}

/// One opening of a railing, exactly, with where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    /// What it is between.
    pub between: Between,
    /// Its line, from 1.
    pub line: usize,
    /// Its piece of line, from 1.
    pub segment: usize,
    /// Its size, rounded once to the step (for words; it is compared exactly).
    pub shown: Length,
    exact: Real,
}

impl Opening {
    fn new(
        between: Between,
        line: usize,
        segment: usize,
        exact: Real,
    ) -> Result<Self, RailingError> {
        let shown = exact
            .nearest()
            .map(Length::from_steps)
            .ok_or_else(|| range("opening"))?;
        Ok(Self {
            between,
            line,
            segment,
            shown,
            exact,
        })
    }

    /// Whether it is at least `limit` wide, exactly.
    #[must_use]
    pub fn at_least(&self, limit: Length) -> bool {
        self.exact
            .compare_whole(i128::from(limit.steps()))
            .is_some_and(|o| o != Ordering::Less)
    }

    /// Whether it is wider than `limit`, exactly.
    #[must_use]
    pub fn wider_than(&self, limit: Length) -> bool {
        self.exact
            .compare_whole(i128::from(limit.steps()))
            .is_some_and(|o| o == Ordering::Greater)
    }
}

/// Where a baluster stands on its piece of line: `t = (a + q sqrt(len^2)) / 2` steps along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Along {
    pub(crate) a: i128,
    pub(crate) q: i128,
}

/// One baluster, laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Baluster {
    /// Its centre in plan, rounded once.
    pub centre: [i64; 2],
    /// Its foot's height, rounded once.
    pub foot: i64,
    /// Its top's height, rounded once.
    pub top: i64,
    pub(crate) along: Along,
}

/// One piece of line between two vertices, laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Its start in plan.
    pub start: [i64; 2],
    /// Its vector in plan.
    pub plan: [i64; 2],
    /// The height under it at its start (the vertex's going-on height).
    pub z0: i64,
    /// How far that height rises to its end.
    pub rise: i64,
    /// The flight it is on, if it is.
    pub flight: Option<Flight>,
    /// Its piece of top rail's length, rounded once.
    pub rail_length: Length,
    /// Its balusters, from its start.
    pub balusters: Vec<Baluster>,
    /// Its widest gap between upright members.
    pub widest: Opening,
    /// The largest disc under its balusters.
    pub under_disc: Opening,
    /// The upright opening under its balusters.
    pub under_rise: Opening,
}

impl Segment {
    /// `len^2`: the square of its length in plan.
    pub(crate) fn squared(&self) -> i128 {
        let (dx, dy) = (i128::from(self.plan[0]), i128::from(self.plan[1]));
        dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
    }
}

/// One line, laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineLayout {
    /// The line.
    pub line: Line,
    /// Its pieces, from its start.
    pub segments: Vec<Segment>,
    /// Its handrail's vertices in plan, one for each of the line's, if it has a handrail.
    pub handrail: Option<Vec<[i64; 2]>>,
    /// Its handrail's length, if it has one.
    pub handrail_length: Option<Length>,
}

/// A railing laid out, as the module header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Its lines.
    pub lines: Vec<LineLayout>,
    /// Its top rail's length, every piece's added.
    pub rail_length: Length,
    /// Its handrail's length, if it has one.
    pub handrail_length: Option<Length>,
    /// How many balusters.
    pub balusters: usize,
    /// How many posts: one at every vertex.
    pub posts: usize,
}

fn range(what: &str) -> RailingError {
    RailingError::OutOfRange {
        what: what.to_owned(),
    }
}

fn big(v: impl Into<BigInt>) -> BigInt {
    v.into()
}

/// A real from whole numbers.
fn real(a: i128, terms: &[(i128, i128)], d: i128) -> Result<Real, RailingError> {
    Real::new(
        big(a),
        terms.iter().map(|&(c, r)| (big(c), big(r))).collect(),
        big(d),
    )
    .ok_or_else(|| range("point"))
}

/// A real rounded once.
pub(crate) fn round(x: &Real, what: &str) -> Result<i64, RailingError> {
    x.nearest().ok_or_else(|| range(what))
}

/// Twice how far a post reaches from its centre, for leaving a baluster out: its width plus its
/// depth (at least its diagonal), or its diameter.
fn post_reach2(p: Profile) -> i128 {
    match p {
        Profile::Rectangle { width, depth } => {
            i128::from(width.steps()).saturating_add(i128::from(depth.steps()))
        }
        Profile::Round { diameter } => i128::from(diameter.steps()),
    }
}

/// Twice a post's least half extent, for an opening: its smaller side, or its diameter.
fn post_side2(p: Profile) -> i128 {
    match p {
        Profile::Rectangle { width, depth } => i128::from(width.steps().min(depth.steps())),
        Profile::Round { diameter } => i128::from(diameter.steps()),
    }
}

/// How many balusters a piece of `len^2 = n` takes at `spacing`: `ceil(len / s) - 1`.
fn count(n: i128, spacing: i64) -> Result<i128, RailingError> {
    let s = i128::from(spacing);
    let root = u128::try_from(n)
        .map_err(|_| range("piece of line"))?
        .isqrt();
    let root = i128::try_from(root).map_err(|_| range("piece of line"))?;
    let m = root.checked_div(s).ok_or_else(|| range("spacing"))?;
    let exact = m.saturating_mul(s).saturating_mul(m.saturating_mul(s)) == n;
    Ok(if exact { m.saturating_sub(1) } else { m })
}

/// Where each of `n` balusters stands along a piece, by `justify`, from its start.
fn positions(n: i128, spacing: i64, justify: Justify) -> Vec<Along> {
    let s = i128::from(spacing);
    let mut out: Vec<Along> = (1..=n)
        .map(|k| match justify {
            Justify::Start => Along {
                a: k.saturating_mul(s).saturating_mul(2),
                q: 0,
            },
            Justify::End => Along {
                a: k.saturating_mul(s).saturating_mul(2).saturating_neg(),
                q: 2,
            },
            Justify::Centre => Along {
                a: k.saturating_mul(2)
                    .saturating_sub(n)
                    .saturating_sub(1)
                    .saturating_mul(s),
                q: 1,
            },
        })
        .collect();
    if justify == Justify::End {
        out.reverse();
    }
    out
}

/// The gap between two members at `p` and `q` along a piece (`q` after `p`), `ep` and `eq` twice
/// their half depths: `(aq - ap - ep - eq + (qq - qp) len) / 2`.
fn gap(p: Along, ep: i128, q: Along, eq: i128, n: i128) -> Result<Real, RailingError> {
    real(
        q.a.saturating_sub(p.a)
            .saturating_sub(ep)
            .saturating_sub(eq),
        &[(q.q.saturating_sub(p.q), n)],
        2,
    )
}

/// The heights of a baluster's foot and top at `t` along a piece, exactly.
fn heights(
    kind: &RailingType,
    seg_n: i128,
    z0: i64,
    rise: i64,
    t: Along,
) -> Result<(Real, Real), RailingError> {
    let bottom = i128::from(kind.balusters.bottom.steps());
    let height = i128::from(kind.top_rail.height.steps());
    let depth = i128::from(kind.top_rail.profile.depth().steps());
    let z0 = i128::from(z0);
    if rise == 0 {
        return Ok((
            Real::whole(z0.saturating_add(bottom)),
            Real::whole(z0.saturating_add(height).saturating_sub(depth)),
        ));
    }
    // A sloped piece runs along an axis, so its length in plan is whole.
    let run = i128::try_from(u128::try_from(seg_n).map_err(|_| range("flight"))?.isqrt())
        .map_err(|_| range("flight"))?;
    let rise = i128::from(rise);
    let twice = run.saturating_mul(2);
    // z0 + rise t / run, with t = (a + q run) / 2.
    let climb = rise.saturating_mul(t.a.saturating_add(t.q.saturating_mul(run)));
    let foot = real(
        twice
            .saturating_mul(z0.saturating_add(bottom))
            .saturating_add(climb),
        &[],
        twice,
    )?;
    let slope = run
        .saturating_mul(run)
        .saturating_add(rise.saturating_mul(rise));
    let top = real(
        twice
            .saturating_mul(z0.saturating_add(height))
            .saturating_add(climb),
        &[(depth.saturating_mul(2).saturating_neg(), slope)],
        twice,
    )?;
    Ok((foot, top))
}

/// The opening under a flight's balusters, as the module header's rule 3 says: the largest disc,
/// and the upright height.
fn under_flight(flight: Flight, bottom: i64) -> Result<(Real, Real), RailingError> {
    let r = i128::from(flight.riser.steps());
    let g = i128::from(flight.going.steps());
    let b = i128::from(bottom);
    let upright = Real::whole(r.saturating_add(b));
    if r <= 0 {
        return Ok((upright.clone(), upright));
    }
    let rb = r.saturating_add(b);
    let disc = real(
        rb.saturating_mul(g.saturating_add(r)),
        &[(
            rb.saturating_neg(),
            g.saturating_mul(g).saturating_add(r.saturating_mul(r)),
        )],
        r,
    )?;
    let fits = disc
        .compare_whole(r.saturating_mul(2))
        .is_some_and(|o| o != Ordering::Greater);
    Ok((if fits { disc } else { upright.clone() }, upright))
}

/// The handrail's vertex at `k` of `line`, `offset` in on its inner side, exactly and rounded once.
fn handrail_vertex(line: &Line, at: usize, offset: i64) -> Result<[i64; 2], RailingError> {
    let vertex = line.vertices.get(at).ok_or_else(|| range("handrail"))?;
    let o = i128::from(offset).saturating_mul(match line.inner {
        Hand::Left => 1,
        Hand::Right => -1,
    });
    let vector = |from: usize, to: usize| -> Option<[i128; 2]> {
        let start = line.vertices.get(from)?.at;
        let end = line.vertices.get(to)?.at;
        Some([
            i128::from(end[0]).saturating_sub(i128::from(start[0])),
            i128::from(end[1]).saturating_sub(i128::from(start[1])),
        ])
    };
    let d_in = at.checked_sub(1).and_then(|j| vector(j, at));
    let d_out = vector(at, at.saturating_add(1));
    let sq = |d: [i128; 2]| {
        d[0].saturating_mul(d[0])
            .saturating_add(d[1].saturating_mul(d[1]))
    };
    let (vx, vy) = (i128::from(vertex.at[0]), i128::from(vertex.at[1]));
    let square = |d: [i128; 2]| -> Result<[Real; 2], RailingError> {
        // v + o (-dy, dx) / len.
        let n = sq(d);
        Ok([
            real(
                vx.saturating_mul(n),
                &[(o.saturating_mul(d[1]).saturating_neg(), n)],
                n,
            )?,
            real(vy.saturating_mul(n), &[(o.saturating_mul(d[0]), n)], n)?,
        ])
    };
    let [across, up] = match (d_in, d_out) {
        (Some(d), None) | (None, Some(d)) => square(d)?,
        (Some(d1), Some(d2)) => {
            let cross = d1[0]
                .saturating_mul(d2[1])
                .saturating_sub(d1[1].saturating_mul(d2[0]));
            if cross == 0 {
                square(d1)?
            } else {
                let (n1, n2) = (sq(d1), sq(d2));
                let c = d1[0]
                    .saturating_mul(d2[0])
                    .saturating_add(d1[1].saturating_mul(d2[1]));
                // v + o m, the mitre m = (n1 + n2) / (1 + n1 . n2) of the two unit normals,
                // rationalised: over cross^2, sqrt(n1) and sqrt(n2) times whole numbers.
                let bx1 = c
                    .saturating_mul(d2[1])
                    .saturating_sub(d1[1].saturating_mul(n2));
                let bx2 = c
                    .saturating_mul(d1[1])
                    .saturating_sub(d2[1].saturating_mul(n1));
                let by1 = d1[0]
                    .saturating_mul(n2)
                    .saturating_sub(c.saturating_mul(d2[0]));
                let by2 = d2[0]
                    .saturating_mul(n1)
                    .saturating_sub(c.saturating_mul(d1[0]));
                let mitre = |v: i128, b1: i128, b2: i128| -> Result<Real, RailingError> {
                    let part = |bb: i128, nn: i128| {
                        term(&[o, bb], &[nn], &[cross, cross]).ok_or_else(|| range("handrail"))
                    };
                    Ok(Real::whole(v).plus(&part(b1, n1)?).plus(&part(b2, n2)?))
                };
                [mitre(vx, bx1, bx2)?, mitre(vy, by1, by2)?]
            }
        }
        (None, None) => return Err(range("handrail")),
    };
    Ok([round(&across, "handrail")?, round(&up, "handrail")?])
}

/// A length `sqrt(n)` rounded once, as a length.
fn root_length(n: i128, what: &str) -> Result<Length, RailingError> {
    let x = real(0, &[(1, n)], 1)?;
    Ok(Length::from_steps(round(&x, what)?))
}

/// A railing of `kind` laid out along `lines`, as the module header says.
///
/// # Errors
/// [`RailingError::SlopedOffAxis`] for a sloped piece not along an axis;
/// [`RailingError::NoBalusterHeight`] for balusters with no height; [`RailingError::OutOfRange`]
/// for more than [`MOST_BALUSTERS`] or a point beyond what a step count holds.
#[allow(
    clippy::too_many_lines,
    reason = "each piece of line laid out in the module header's order"
)]
pub fn lay_out(kind: &RailingType, lines: Vec<Line>) -> Result<Layout, RailingError> {
    let bal = &kind.balusters;
    let reach2 = post_reach2(kind.posts);
    let side2 = post_side2(kind.posts);
    let depth2 = i128::from(bal.profile.depth().steps());
    let keep2 = reach2.saturating_add(depth2);
    let mut laid = Vec::with_capacity(lines.len());
    let (mut rail_length, mut handrail_length) = (Length::from_steps(0), None::<Length>);
    let (mut balusters, mut posts) = (0_usize, 0_usize);
    for (i, line) in lines.into_iter().enumerate() {
        let number = i.saturating_add(1);
        posts = posts.saturating_add(line.vertices.len());
        let mut segments = Vec::new();
        for (k, pair) in line.vertices.windows(2).enumerate() {
            let (Some(p), Some(q)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let seg = k.saturating_add(1);
            let plan = [
                q.at[0]
                    .checked_sub(p.at[0])
                    .ok_or_else(|| range("piece of line"))?,
                q.at[1]
                    .checked_sub(p.at[1])
                    .ok_or_else(|| range("piece of line"))?,
            ];
            let rise = q
                .low
                .checked_sub(p.high)
                .ok_or_else(|| range("piece of line"))?;
            if rise != 0 && plan[0] != 0 && plan[1] != 0 {
                return Err(RailingError::SlopedOffAxis {
                    line: number,
                    segment: seg,
                });
            }
            let (dx, dy) = (i128::from(plan[0]), i128::from(plan[1]));
            let n = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
            let up = i128::from(rise);
            let piece = root_length(n.saturating_add(up.saturating_mul(up)), "rail")?;
            rail_length = rail_length
                .checked_add(piece)
                .ok_or_else(|| range("rail length"))?;
            let wanted = count(n, bal.spacing.steps())?;
            if usize::try_from(wanted)
                .ok()
                .and_then(|w| w.checked_add(balusters))
                .is_none_or(|all| all > MOST_BALUSTERS)
            {
                return Err(range(&format!("count of balusters, over {MOST_BALUSTERS}")));
            }
            let mut here = Vec::new();
            for t in positions(wanted, bal.spacing.steps(), bal.justify) {
                // Kept when it stands clear of both posts.
                let after_start = real(t.a.saturating_sub(keep2), &[(t.q, n)], 1)?;
                let before_end = real(
                    t.a.saturating_add(keep2).saturating_neg(),
                    &[(2_i128.saturating_sub(t.q), n)],
                    1,
                )?;
                let clear = |x: &Real| x.sign().is_some_and(|o| o != Ordering::Less);
                if !(clear(&after_start) && clear(&before_end)) {
                    continue;
                }
                let (foot, top) = heights(kind, n, p.high, rise, t)?;
                let foot = round(&foot, "baluster")?;
                let top = round(&top, "baluster")?;
                if top <= foot {
                    return Err(RailingError::NoBalusterHeight {
                        line: number,
                        segment: seg,
                    });
                }
                // Its centre: p + t d / len, t = (a + q len) / 2.
                let two_n = n.saturating_mul(2);
                let centre = |c: i64, d: i128| -> Result<i64, RailingError> {
                    let x = real(
                        two_n
                            .saturating_mul(i128::from(c))
                            .saturating_add(t.q.saturating_mul(n).saturating_mul(d)),
                        &[(t.a.saturating_mul(d), n)],
                        two_n,
                    )?;
                    round(&x, "baluster")
                };
                here.push(Baluster {
                    centre: [centre(p.at[0], dx)?, centre(p.at[1], dy)?],
                    foot,
                    top,
                    along: t,
                });
            }
            balusters = balusters.saturating_add(here.len());
            // The members along it: a post, its balusters, a post.
            let mut members: Vec<(Along, i128)> = vec![(Along { a: 0, q: 0 }, side2)];
            members.extend(here.iter().map(|b| (b.along, depth2)));
            members.push((Along { a: 0, q: 2 }, side2));
            let mut widest: Option<Real> = None;
            for w in members.windows(2) {
                let (Some(&(pa, ea)), Some(&(pb, eb))) = (w.first(), w.get(1)) else {
                    continue;
                };
                let g = gap(pa, ea, pb, eb, n)?;
                widest = Some(match widest {
                    Some(held) if held.compare(&g) != Some(Ordering::Less) => held,
                    _ => g,
                });
            }
            let widest = widest.ok_or_else(|| range("piece of line"))?;
            let flight = line.flights.get(k).copied().flatten();
            let (disc, upright) = if let (Some(f), true) = (flight, rise != 0) {
                under_flight(f, bal.bottom.steps())?
            } else {
                let b = Real::whole(i128::from(bal.bottom.steps()));
                (b.clone(), b)
            };
            let between_floor = if rise == 0 {
                Between::Floor
            } else {
                Between::Flight
            };
            segments.push(Segment {
                start: p.at,
                plan,
                z0: p.high,
                rise,
                flight: if rise == 0 { None } else { flight },
                rail_length: piece,
                balusters: here,
                widest: Opening::new(Between::Members, number, seg, widest)?,
                under_disc: Opening::new(between_floor, number, seg, disc)?,
                under_rise: Opening::new(between_floor, number, seg, upright)?,
            });
        }
        let (handrail, length) = match &kind.handrail {
            None => (None, None),
            Some(h) => {
                let points = (0..line.vertices.len())
                    .map(|k| handrail_vertex(&line, k, h.offset.steps()))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut total = Length::from_steps(0);
                for (pair, seg) in points.windows(2).zip(&segments) {
                    let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    let dx = i128::from(b[0]).saturating_sub(i128::from(a[0]));
                    let dy = i128::from(b[1]).saturating_sub(i128::from(a[1]));
                    let up = i128::from(seg.rise);
                    let n = dx
                        .saturating_mul(dx)
                        .saturating_add(dy.saturating_mul(dy))
                        .saturating_add(up.saturating_mul(up));
                    total = total
                        .checked_add(root_length(n, "handrail")?)
                        .ok_or_else(|| range("handrail length"))?;
                }
                handrail_length = Some(
                    handrail_length
                        .unwrap_or(Length::from_steps(0))
                        .checked_add(total)
                        .ok_or_else(|| range("handrail length"))?,
                );
                (Some(points), Some(total))
            }
        };
        laid.push(LineLayout {
            line,
            segments,
            handrail,
            handrail_length: length,
        });
    }
    Ok(Layout {
        lines: laid,
        rail_length,
        handrail_length,
        balusters,
        posts,
    })
}
