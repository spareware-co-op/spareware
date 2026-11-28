//! Every part of a laid-out railing as a solid on the kernel's grid, in the railing's frame (its
//! base at `z = 0`): each an extrusion of a flat profile along a vector (`Engine::extrude`), so the
//! builders need nothing else. Every corner is worked out exactly and rounded once
//! ([`crate::exact`]).
//!
//! - **A post** at every vertex: its profile in plan, square to the piece of line that leaves the
//!   vertex (at the last vertex, the one that comes in), from the height under the line as it comes
//!   in up to the top rail's top above the height as it goes on; so at the foot of a run, where the
//!   line steps up, the post reaches from the floor to the rail.
//! - **A baluster** where the layout put it: its profile in plan, square to its piece, from its foot
//!   up to its top (both rounded once by the layout).
//! - **A piece of top rail** for each piece of line, and of handrail along the handrail's line:
//!   its profile set upright in the plane square to whichever plan axis the piece runs nearer
//!   (`x = const` when `|dx| >= |dy|`, else `y = const`), so the profile is exactly flat, and swept
//!   along the piece. In that plane a point `u` across and `v` up in the rail's own section sits
//!   `u len / dx` along the other axis (`-u len / dy` in the plane `y = const`) and, on a slope,
//!   `v sqrt(run^2 + rise^2) / run` up: so the rail's true section, square to it, is its profile, at
//!   any angle and on any pitch. The profile's top is at the rail's height above the line's start.
//!   Where the line steps up at a run's foot no piece of rail joins the two heights: the post
//!   stands there.
//! - **Every piece is checked** before a builder asks the engine for it: its profile flat, with an
//!   area, and its sweep out of its plane, exactly ([`RailingError::NotFlat`] otherwise).
//! - **A round profile** is twelve corners on its circle, at every 30 degrees from its top for a
//!   rail and from `+x` for a post or a baluster; their sines and cosines are `0`, `1/2`,
//!   `sqrt(3)/2` and `1`, so every corner stays exact before its one rounding. Interim, as
//!   [`crate::kind`] says.

use crate::error::RailingError;
use crate::exact::{self, Real};
use crate::kind::{Profile, RailingType};
use crate::layout::{Layout, round};

/// What a piece is part of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// A post.
    Post,
    /// A piece of the top rail.
    TopRail,
    /// A piece of the handrail.
    Handrail,
    /// A baluster.
    Baluster,
}

impl Part {
    /// Its name, as a piece's function and the railings category's subcategory read it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::TopRail => "top-rail",
            Self::Handrail => "handrail",
            Self::Baluster => "baluster",
        }
    }
}

/// One piece of a railing: a flat profile swept along a vector, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Its ID: `post.<i>.<k>`, `rail.<i>.<k>`, `handrail.<i>.<k>` or `baluster.<i>.<k>.<j>`,
    /// for line `i`, vertex or piece of line `k` and baluster `j`, each from 1.
    pub id: String,
    /// What it is part of.
    pub part: Part,
    /// Its profile's corners, in order round it.
    pub profile: Vec<[i64; 3]>,
    /// The sweep.
    pub by: [i64; 3],
}

fn range(what: &str) -> RailingError {
    RailingError::OutOfRange {
        what: what.to_owned(),
    }
}

/// `2 cos` and `2 sin` of every 30 degrees from 0, each `a + b sqrt(3)`.
const TWELVE: [[(i128, i128); 2]; 12] = [
    [(2, 0), (0, 0)],
    [(0, 1), (1, 0)],
    [(1, 0), (0, 1)],
    [(0, 0), (2, 0)],
    [(-1, 0), (0, 1)],
    [(0, -1), (1, 0)],
    [(-2, 0), (0, 0)],
    [(0, -1), (-1, 0)],
    [(-1, 0), (0, -1)],
    [(0, 0), (-2, 0)],
    [(1, 0), (0, -1)],
    [(0, 1), (-1, 0)],
];

/// `a / d` and `c sqrt(r) / d`, each a product of whole numbers, as reals.
fn ratio(a: &[i128], d: &[i128]) -> Result<Real, RailingError> {
    exact::ratio(a, d).ok_or_else(|| range("corner"))
}

fn term(c: &[i128], r: &[i128], d: &[i128]) -> Result<Real, RailingError> {
    exact::term(c, r, d).ok_or_else(|| range("corner"))
}

/// `2 cos` and `2 sin` of `30 k` degrees, each `a + b sqrt(3)`.
fn twelfth(k: usize) -> [(i128, i128); 2] {
    TWELVE.get(k % 12).copied().unwrap_or([(2, 0), (0, 0)])
}

/// A centre in plan, `x = (a + root sqrt(n)) / den` along each axis.
#[derive(Clone, Copy)]
struct Centre {
    x: (i128, i128),
    y: (i128, i128),
    den: i128,
}

impl Centre {
    /// A grid point.
    fn at(p: [i64; 2]) -> Self {
        Self {
            x: (i128::from(p[0]), 0),
            y: (i128::from(p[1]), 0),
            den: 1,
        }
    }

    fn real(self, (a, root): (i128, i128), n: i128) -> Result<Real, RailingError> {
        Ok(ratio(&[a], &[self.den])?.plus(&term(&[root], &[n], &[self.den])?))
    }
}

/// A profile's corners in plan about `centre`, square to the direction `d`, at height `z`: each
/// corner one real, rounded once. A rectangle's corner is `centre + (along d + across (-dy, dx)) /
/// len` for `along` a half depth and `across` a half width either way; a round one's `centre +
/// diameter (2 cos, 2 sin) / 4` every 30 degrees from `+x`.
fn plan_corners(
    profile: Profile,
    centre: Centre,
    d: [i128; 2],
    z: i64,
) -> Result<Vec<[i64; 3]>, RailingError> {
    let n = d[0]
        .saturating_mul(d[0])
        .saturating_add(d[1].saturating_mul(d[1]));
    let (cx, cy) = (centre.real(centre.x, n)?, centre.real(centre.y, n)?);
    let mut out = Vec::new();
    match profile {
        Profile::Rectangle { width, depth } => {
            let (w, dd) = (i128::from(width.steps()), i128::from(depth.steps()));
            let two_n = n.saturating_mul(2);
            let (back, right) = (dd.saturating_neg(), w.saturating_neg());
            for (along, across) in [(back, right), (dd, right), (dd, w), (back, w)] {
                // (along dx - across dy) sqrt(n) / 2n, and (along dy + across dx) sqrt(n) / 2n.
                let ox = along
                    .saturating_mul(d[0])
                    .saturating_sub(across.saturating_mul(d[1]));
                let oy = along
                    .saturating_mul(d[1])
                    .saturating_add(across.saturating_mul(d[0]));
                out.push([
                    round(&cx.plus(&term(&[ox], &[n], &[two_n])?), "corner")?,
                    round(&cy.plus(&term(&[oy], &[n], &[two_n])?), "corner")?,
                    z,
                ]);
            }
        }
        Profile::Round { diameter } => {
            let dm = i128::from(diameter.steps());
            for k in 0..12 {
                let [(ca, cb), (sa, sb)] = twelfth(k);
                let off = |a: i128, bb: i128| -> Result<Real, RailingError> {
                    Ok(ratio(&[dm, a], &[4])?.plus(&term(&[dm, bb], &[3], &[4])?))
                };
                out.push([
                    round(&cx.plus(&off(ca, cb)?), "corner")?,
                    round(&cy.plus(&off(sa, sb)?), "corner")?,
                    z,
                ]);
            }
        }
    }
    Ok(out)
}

/// A rail's profile, upright in the axis plane square to its piece `d` (with `rise`) at `start`,
/// its top at `top`, as the module header says.
fn rail_corners(
    profile: Profile,
    start: [i64; 2],
    top: i64,
    d: [i64; 2],
    rise: i64,
) -> Result<Vec<[i64; 3]>, RailingError> {
    let (dx, dy) = (i128::from(d[0]), i128::from(d[1]));
    let n = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
    let along_x = dx.abs() >= dy.abs();
    // In the plane x = const a point `u` across is `u len / dx` along y; in y = const, `-u len /
    // dy` along x.
    let major = if along_x { dx } else { dy.saturating_neg() };
    if major == 0 {
        return Err(range("rail"));
    }
    // The section's corners `(u, v)` from its top, each `(a + b sqrt 3) / 4` steps.
    let corners: Vec<[(i128, i128); 2]> = match profile {
        Profile::Rectangle { width, depth } => {
            let w2 = i128::from(width.steps()).saturating_mul(2);
            let d4 = i128::from(depth.steps()).saturating_mul(4);
            vec![
                [(w2.saturating_neg(), 0), (0, 0)],
                [(w2, 0), (0, 0)],
                [(w2, 0), (d4.saturating_neg(), 0)],
                [(w2.saturating_neg(), 0), (d4.saturating_neg(), 0)],
            ]
        }
        Profile::Round { diameter } => {
            let dm = i128::from(diameter.steps());
            // From the top (90 degrees), every 30 degrees: u = r cos, v = r sin - r.
            (0_usize..12)
                .map(|k| {
                    let [(ca, cb), (sa, sb)] = twelfth(k.saturating_add(3));
                    [
                        (dm.saturating_mul(ca), dm.saturating_mul(cb)),
                        (
                            dm.saturating_mul(sa.saturating_sub(2)),
                            dm.saturating_mul(sb),
                        ),
                    ]
                })
                .collect()
        }
    };
    let up = i128::from(rise);
    let slope = n.saturating_add(up.saturating_mul(up));
    // A sloped piece runs along an axis: its length in plan is whole.
    let run = i128::try_from(u128::try_from(n).map_err(|_| range("rail"))?.isqrt())
        .map_err(|_| range("rail"))?;
    let other = i128::from(if along_x { start[1] } else { start[0] });
    let four_major = major.saturating_mul(4);
    let four_run = run.saturating_mul(4);
    let mut out = Vec::new();
    for [(ua, ub), (va, vb)] in corners {
        let cross = ratio(&[other], &[1])?
            .plus(&term(&[ua], &[n], &[four_major])?)
            .plus(&term(&[ub], &[3, n], &[four_major])?);
        let height = if rise == 0 {
            ratio(&[top.into()], &[1])?
                .plus(&ratio(&[va], &[4])?)
                .plus(&term(&[vb], &[3], &[4])?)
        } else {
            ratio(&[top.into()], &[1])?
                .plus(&term(&[va], &[slope], &[four_run])?)
                .plus(&term(&[vb], &[3, slope], &[four_run])?)
        };
        let c = round(&cross, "rail")?;
        let z = round(&height, "rail")?;
        out.push(if along_x {
            [start[0], c, z]
        } else {
            [c, start[1], z]
        });
    }
    Ok(out)
}

/// The pieces of a railing of `kind` laid out as `layout`, as the module header says: for each
/// line, its posts, its pieces of top rail, its pieces of handrail and its balusters.
///
/// # Errors
/// [`RailingError::OutOfRange`] for a corner beyond what a step count holds;
/// [`RailingError::NoLength`] for a piece of handrail its offset closes up.
#[allow(
    clippy::too_many_lines,
    reason = "every part of a line in the module header's order"
)]
pub fn shape(kind: &RailingType, layout: &Layout) -> Result<Vec<Piece>, RailingError> {
    let height = kind.top_rail.height.steps();
    let mut pieces = Vec::new();
    for (i, laid) in layout.lines.iter().enumerate() {
        let line = i.saturating_add(1);
        let vertices = &laid.line.vertices;
        let last = vertices.len().saturating_sub(1);
        for (k, v) in vertices.iter().enumerate() {
            let seg = laid
                .segments
                .get(k.min(last.saturating_sub(1)))
                .ok_or_else(|| range("post"))?;
            let d = [i128::from(seg.plan[0]), i128::from(seg.plan[1])];
            let top = v
                .high
                .checked_add(height)
                .and_then(|t| t.checked_sub(v.low))
                .ok_or_else(|| range("post"))?;
            pieces.push(Piece {
                id: format!("post.{line}.{}", k.saturating_add(1)),
                part: Part::Post,
                profile: plan_corners(kind.posts, Centre::at(v.at), d, v.low)?,
                by: [0, 0, top],
            });
        }
        for (k, seg) in laid.segments.iter().enumerate() {
            let number = k.saturating_add(1);
            let top = seg.z0.checked_add(height).ok_or_else(|| range("rail"))?;
            pieces.push(Piece {
                id: format!("rail.{line}.{number}"),
                part: Part::TopRail,
                profile: rail_corners(kind.top_rail.profile, seg.start, top, seg.plan, seg.rise)?,
                by: [seg.plan[0], seg.plan[1], seg.rise],
            });
        }
        if let (Some(handrail), Some(points)) = (&kind.handrail, &laid.handrail) {
            for (k, (pair, seg)) in points.windows(2).zip(&laid.segments).enumerate() {
                let (Some(a), Some(z)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let d = [
                    z[0].checked_sub(a[0]).ok_or_else(|| range("handrail"))?,
                    z[1].checked_sub(a[1]).ok_or_else(|| range("handrail"))?,
                ];
                if d == [0, 0] {
                    return Err(RailingError::NoLength {
                        segment: k.saturating_add(1),
                    });
                }
                let top = seg
                    .z0
                    .checked_add(handrail.height.steps())
                    .ok_or_else(|| range("handrail"))?;
                pieces.push(Piece {
                    id: format!("handrail.{line}.{}", k.saturating_add(1)),
                    part: Part::Handrail,
                    profile: rail_corners(handrail.profile, *a, top, d, seg.rise)?,
                    by: [d[0], d[1], seg.rise],
                });
            }
        }
        for (k, seg) in laid.segments.iter().enumerate() {
            let d = [i128::from(seg.plan[0]), i128::from(seg.plan[1])];
            let n = seg.squared();
            for (j, bal) in seg.balusters.iter().enumerate() {
                // Its centre p + t d / len, t = (a + q len) / 2: over 2n, `2n p + q n d` and
                // `a d` times sqrt(n).
                let two_n = n.saturating_mul(2);
                let c = |p: i64, dd: i128| {
                    (
                        two_n
                            .saturating_mul(i128::from(p))
                            .saturating_add(bal.along.q.saturating_mul(n).saturating_mul(dd)),
                        bal.along.a.saturating_mul(dd),
                    )
                };
                pieces.push(Piece {
                    id: format!(
                        "baluster.{line}.{}.{}",
                        k.saturating_add(1),
                        j.saturating_add(1)
                    ),
                    part: Part::Baluster,
                    profile: plan_corners(
                        kind.balusters.profile,
                        Centre {
                            x: c(seg.start[0], d[0]),
                            y: c(seg.start[1], d[1]),
                            den: two_n,
                        },
                        d,
                        bal.foot,
                    )?,
                    by: [
                        0,
                        0,
                        bal.top
                            .checked_sub(bal.foot)
                            .ok_or_else(|| range("baluster"))?,
                    ],
                });
            }
        }
    }
    // Our own check of every piece before the engine is asked for it.
    if let Some(bad) = pieces
        .iter()
        .find(|p| !exact::flat_and_swept(&p.profile, p.by))
    {
        return Err(RailingError::NotFlat {
            piece: bad.id.clone(),
        });
    }
    Ok(pieces)
}
