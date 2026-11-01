//! General surface with surface, first part: subdivision until no pair of patches can hide a loop,
//! and the start points every branch must pass (12.8, steps 1 to 5).
//!
//! What it does: [`subdivide`] keeps pairs of patches on a work list. A pair is culled when the
//! patches are proven apart, settled when one coordinate of n₁ × n₂ keeps its sign over it, and
//! split otherwise. Where each settled pair's patch edges meet the other patch are its start
//! points, which G170 marches between.
//!
//! Guarantees:
//!
//! - **A settled pair holds no loop.** n₁ × n₂ is the tangent of any intersection branch. Where one
//!   of its coordinates keeps its sign that coordinate of space moves strictly one way along every
//!   branch, so no branch closes, and the normals are never parallel, so there is no tangent or
//!   singular point in the pair.
//! - **Every component reaches a start point, or a flag.** A pair is dropped only when its patches
//!   are proven apart. So every branch lies in settled pairs or flagged ones, and a branch in a
//!   settled pair, being no loop, crosses the pair's boundary.
//! - **Everything is accounted for.** A branch through a settled pair enters and leaves, so each
//!   pair holds an even number of start points, or its count goes to [`Subdivided::flagged`].
//! - **Nothing is decided below the floor.** A pair still undecided there is flagged, for 12.9.
//!
//! Method: Hohmeyer's subdivision (UCB/CSD-92-681, 1992,
//! <https://www2.eecs.berkeley.edu/Pubs/TechRpts/1992/6140.html>) with Patrikalakis and Maekawa's
//! structure (12.8). Chapter 12.8 asked this piece to re-derive Hohmeyer's weaker criterion, Gauss maps a
//! plane can separate. As the chapter states it, it is false: the lens between z = x² + y² − 1 and
//! z = 1 − x² − y² has outward normals a plane separates, and meets in a closed circle. It is not
//! used. What is used needs no thesis: the test above is the tangent's own.
//!
//! Nor does completeness need the chapter's step 4, the search for collinear-normal points. Every
//! pair ends apart, settled or flagged, so a loop around such a point is found all the same: the
//! pairs along the loop shrink until its tangent keeps a sign, and the pair at the point itself is
//! culled, since the gap holds its patches apart. Step 4 saves work only near a tangency, where
//! pairs otherwise run down to the floor and are flagged, so it belongs with G171's analysis of
//! them, not here.
//!
//! The two surfaces are cut at different shares, off the middle. Two surfaces parameterised alike
//! would otherwise be cut along the same lines, and a branch would cross the corners of both, where
//! neither edge can be told from the other.
//!
//! A fat plane here is the mean-value form of a patch along its middle normal, rather than the
//! control net's slab, so it serves every kind and tightens with the square of the patch's width.
//!
//! Cost: a pair takes two bounds, and two sets of partials if neither culls it; pairs number about
//! the length of the intersection over the patch size where the normals part, near one per step of
//! a branch. Each settled pair's eight edges run the engine of [`crate::solve`]. All of it is
//! charged to the meter. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are chapter 12.8's: S the surfaces, u and v their parameters, n the normals"
)]

use alloc::vec::Vec;

use geom_curve::CurveError;
use geom_curve::tight::larger;
use geom_num::NumError;
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Limit};
use geom_run::WorkMeter;
use geom_surface::{ParamBox as Region, ParamSurface, SurfaceKind};

use crate::IntersectError;
use crate::project::{boxed, dot, image, negative, positive};
use crate::result::{Test, Unresolved};
use crate::solve::{Field, solve};

/// What a pair of patches costs the meter, besides the kinds' own charges.
const STEP: u64 = 1;

/// Where the first surface's patches are cut, as a share of their width (module documentation).
const FIRST_CUT: f64 = 0.4375;

/// Where the second surface's are.
const SECOND_CUT: f64 = 0.3125;

/// A settled pair of patches, and where its branches enter and leave.
#[derive(Clone, Debug, PartialEq)]
pub struct Settled {
    /// The patch of the first surface.
    pub first: Region,
    /// The patch of the second.
    pub second: Region,
    /// Its start points: (u₁, v₁, u₂, v₂), each holding one exact point.
    pub starts: Vec<[Interval; 4]>,
}

/// A pair of patches left undecided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlaggedPair {
    /// The patch of the first surface.
    pub first: Region,
    /// The patch of the second.
    pub second: Region,
    /// Why.
    pub why: Unresolved,
}

/// What [`subdivide`] leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Subdivided {
    /// The settled pairs holding a branch, in the order the work list took them.
    pub settled: Vec<Settled>,
    /// What could not be decided.
    pub flagged: Vec<FlaggedPair>,
}

/// `a × b`.
fn cross(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([
        a1.mul(b2)?.sub(a2.mul(b1)?)?,
        a2.mul(b0)?.sub(a0.mul(b2)?)?,
        a0.mul(b1)?.sub(a1.mul(b0)?)?,
    ])
}

/// A span as an interval.
fn span((a, b): (f64, f64)) -> Result<Interval, NumError> {
    Interval::new(a, b)
}

/// An interval's ends as a span.
const fn ends(x: Interval) -> (f64, f64) {
    (x.lo(), x.hi())
}

/// The side a parameter is read from, inside its domain.
fn inward(at: f64, (_, hi): (f64, f64)) -> Limit {
    if at < hi {
        Limit::FromRight
    } else {
        Limit::FromLeft
    }
}

/// The partials of `surface` to `order` over `region`, or `None` where they cannot be bounded.
fn partials(
    surface: &SurfaceKind,
    region: Region,
    order: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<[Interval; 3]>>, IntersectError> {
    let (us, vs) = surface.domain();
    let sides = (inward(region.0.0, us), inward(region.1.0, vs));
    match surface.partials_over(region, sides, order, meter) {
        Ok(found) => Ok(Some(found)),
        Err(CurveError::Unbounded) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Whether two boxes in space are apart.
fn apart(a: &[Interval; 3], b: &[Interval; 3]) -> bool {
    a.iter()
        .zip(b)
        .any(|(x, y)| x.hi() < y.lo() || y.hi() < x.lo())
}

/// Whether `other` lies clear of the mean-value slab of `surface` over `region` along its middle
/// normal: the fat plane of 12.8's step 1.
fn clear_of_slab(
    surface: &SurfaceKind,
    region: Region,
    other: &[Interval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<bool, IntersectError> {
    let ((a, b), (c, d)) = region;
    let (m, n) = (a / 2.0 + b / 2.0, c / 2.0 + d / 2.0);
    let Some(whole) = partials(surface, region, 1, meter)? else {
        return Ok(false);
    };
    let Some(middle) = partials(surface, ((m, m), (n, n)), 1, meter)? else {
        return Ok(false);
    };
    let ([_, su, sv, ..], [s, mu, mv, ..]) = (whole.as_slice(), middle.as_slice()) else {
        return Err(IntersectError::Internal);
    };
    let normal = cross(mu, mv)?;
    let u = span((a, b))?.sub(Interval::point(m)?)?;
    let v = span((c, d))?.sub(Interval::point(n)?)?;
    let slab = dot(&normal, s)?
        .add(dot(&normal, su)?.mul(u)?)?
        .add(dot(&normal, sv)?.mul(v)?)?;
    let away = dot(&normal, other)?;
    Ok(away.hi() < slab.lo() || slab.hi() < away.lo())
}

/// A box around a patch.
fn around(
    surface: &SurfaceKind,
    region: Region,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], IntersectError> {
    boxed(&surface.bound(region, meter)?)
}

/// A box around a patch's normals.
fn normals(
    surface: &SurfaceKind,
    region: Region,
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], IntersectError> {
    let bounds: Bounds<3> = surface.normal_bound(region, meter)?;
    boxed(&bounds)
}

/// Whether the normals over two patches may be parallel: no coordinate of n₁ × n₂ keeps its sign.
fn may_be_parallel(a: &[Interval; 3], b: &[Interval; 3]) -> Result<bool, NumError> {
    let tangent = cross(a, b)?;
    Ok(!tangent.iter().any(|&x| positive(x) || negative(x)))
}

/// The partials of an edge and of the patch it meets.
type Read = (Vec<[Interval; 3]>, Vec<[Interval; 3]>);

/// `a − b`.
fn minus_box(a: &[Interval; 3], b: &[Interval; 3]) -> Result<[Interval; 3], NumError> {
    let [a0, a1, a2] = *a;
    let [b0, b1, b2] = *b;
    Ok([a0.sub(b0)?, a1.sub(b1)?, a2.sub(b2)?])
}

/// A point as a box of one point.
fn point(x: f64) -> Result<Interval, NumError> {
    Interval::point(x)
}

/// One edge of a patch against the other patch: `S₁(t) − S₂(s, r)`, three equations in three
/// unknowns, for 12.8's step 5.
struct Edge<'a> {
    /// The surface whose edge it is.
    own: &'a SurfaceKind,
    /// The surface it meets.
    other: &'a SurfaceKind,
    /// The fixed parameter's value.
    at: f64,
    /// Whether u is the one that is fixed.
    fixed_u: bool,
    floor: f64,
}

impl Edge<'_> {
    /// The region of `t` along the edge.
    fn along(&self, t: Interval) -> Region {
        if self.fixed_u {
            ((self.at, self.at), ends(t))
        } else {
            (ends(t), (self.at, self.at))
        }
    }

    /// The residuals over `x`, and the partials they came from.
    fn read(
        &self,
        x: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Read>, IntersectError> {
        let [t, s, r] = *x;
        let a = partials(self.own, self.along(t), 1, meter)?;
        let b = partials(self.other, (ends(s), ends(r)), 1, meter)?;
        Ok(a.zip(b))
    }
}

impl Field<3> for Edge<'_> {
    fn at(&self, x: [f64; 3], meter: &mut WorkMeter<'_>) -> Result<[Interval; 3], IntersectError> {
        let [t, s, r] = x;
        let boxed = [point(t)?, point(s)?, point(r)?];
        match self.over(&boxed, meter)? {
            Some(r) => Ok(r),
            None => Err(IntersectError::Internal),
        }
    }

    fn over(
        &self,
        x: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[Interval; 3]>, IntersectError> {
        let Some((a, b)) = self.read(x, meter)? else {
            return Ok(None);
        };
        let ([s, ..], [t, ..]) = (a.as_slice(), b.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        Ok(Some(minus_box(s, t)?))
    }

    fn slope(
        &self,
        x: &[Interval; 3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<[[Interval; 3]; 3]>, IntersectError> {
        let Some((a, b)) = self.read(x, meter)? else {
            return Ok(None);
        };
        let ([_, su, sv, ..], [_, tu, tv, ..]) = (a.as_slice(), b.as_slice()) else {
            return Err(IntersectError::Internal);
        };
        let along = if self.fixed_u { sv } else { su };
        let row = |k: usize| -> Option<[Interval; 3]> {
            Some([*along.get(k)?, tu.get(k)?.neg(), tv.get(k)?.neg()])
        };
        match (row(0), row(1), row(2)) {
            (Some(x), Some(y), Some(z)) => Ok(Some([x, y, z])),
            _ => Err(IntersectError::Internal),
        }
    }

    fn reach(&self, x: &[Interval; 3], meter: &mut WorkMeter<'_>) -> Result<f64, IntersectError> {
        let [t, s, r] = *x;
        let Some(a) = partials(self.own, self.along(t), 0, meter)? else {
            return Ok(f64::INFINITY);
        };
        let [own, ..] = a.as_slice() else {
            return Err(IntersectError::Internal);
        };
        let b = around(self.other, (ends(s), ends(r)), meter)?;
        Ok(larger(image(own), image(&b)) / self.floor)
    }
}

/// Whether two parameter boxes of a pair overlap.
fn same(a: &[Interval; 4], b: &[Interval; 4]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.lo() <= y.hi() && y.lo() <= x.hi())
}

/// The start points of one settled pair, or `None` if they could not all be proven.
fn starts_of(
    first: &SurfaceKind,
    second: &SurfaceKind,
    pair: (Region, Region),
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Vec<[Interval; 4]>>, IntersectError> {
    let (one, two) = pair;
    let mut out: Vec<[Interval; 4]> = Vec::new();
    for (own, other, here, there, swapped) in [
        (first, second, one, two, false),
        (second, first, two, one, true),
    ] {
        let ((a, b), (c, d)) = here;
        for (at, fixed_u) in [(a, true), (b, true), (c, false), (d, false)] {
            let edge = Edge {
                own,
                other,
                at,
                fixed_u,
                floor,
            };
            let length = if fixed_u { (c, d) } else { (a, b) };
            let domain = [span(length)?, span(there.0)?, span(there.1)?];
            let found = solve(&edge, domain, |_, _| Ok(true), meter)?;
            if !found.flagged.is_empty() {
                return Ok(None);
            }
            for zero in found.zeros {
                let [t, s, r] = zero.enclosure;
                let held = point(at)?;
                let on = if fixed_u { [held, t] } else { [t, held] };
                let [x, y] = on;
                let whole = if swapped { [s, r, x, y] } else { [x, y, s, r] };
                if !out.iter().any(|known| same(known, &whole)) {
                    out.push(whole);
                }
            }
        }
    }
    Ok(Some(out))
}

/// The point `share` of the way across a span, if it has one strictly inside.
fn across((a, b): (f64, f64), share: f64) -> Option<f64> {
    let m = a + (b - a) * share;
    (a < m && m < b).then_some(m)
}

/// `region` cut across its longer side in space, or `None` if it has no room.
fn halved(region: Region, speed: [f64; 2], share: f64) -> Option<(Region, Region)> {
    let ((a, b), (c, d)) = region;
    let [su, sv] = speed;
    let cut_u = || across((a, b), share).map(|m| (((a, m), (c, d)), ((m, b), (c, d))));
    let cut_v = || across((c, d), share).map(|m| (((a, b), (c, m)), ((a, b), (m, d))));
    if (b - a) * su >= (d - c) * sv {
        cut_u().or_else(cut_v)
    } else {
        cut_v().or_else(cut_u)
    }
}

/// The speeds of a patch along its parameters.
fn speeds(
    surface: &SurfaceKind,
    region: Region,
    meter: &mut WorkMeter<'_>,
) -> Result<[f64; 2], IntersectError> {
    let Some(found) = partials(surface, region, 1, meter)? else {
        return Ok([1.0, 1.0]);
    };
    let [_, su, sv, ..] = found.as_slice() else {
        return Err(IntersectError::Internal);
    };
    Ok([Interval::norm(su)?.hi(), Interval::norm(sv)?.hi()])
}

/// Subdivides the pair of `first` and `second` until each piece is apart, settled or flagged, to a
/// floor of `floor` in space, and finds the settled pairs' start points.
///
/// # Errors
///
/// [`IntersectError::InvalidInput`] for a floor that is not positive;
/// [`IntersectError::BudgetExceeded`] and [`IntersectError::Cancelled`];
/// [`IntersectError::UnprovenMaths`] where a kind's bound needs one.
pub fn subdivide(
    first: &SurfaceKind,
    second: &SurfaceKind,
    floor: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<Subdivided, IntersectError> {
    if floor.is_nan() || floor <= 0.0 {
        return Err(IntersectError::InvalidInput);
    }
    let mut out = Subdivided {
        settled: Vec::new(),
        flagged: Vec::new(),
    };
    let mut work = alloc::vec![(first.domain(), second.domain())];
    while let Some((a, b)) = work.pop() {
        meter.charge(STEP)?;
        let (over_a, over_b) = (around(first, a, meter)?, around(second, b, meter)?);
        if apart(&over_a, &over_b)
            || clear_of_slab(first, a, &over_b, meter)?
            || clear_of_slab(second, b, &over_a, meter)?
        {
            continue;
        }
        let (na, nb) = (normals(first, a, meter)?, normals(second, b, meter)?);
        if !may_be_parallel(&na, &nb)? {
            match starts_of(first, second, (a, b), floor, meter)? {
                Some(starts) if starts.len() % 2 == 0 => {
                    if !starts.is_empty() {
                        out.settled.push(Settled {
                            first: a,
                            second: b,
                            starts,
                        });
                    }
                }
                _ => out.flagged.push(FlaggedPair {
                    first: a,
                    second: b,
                    why: Unresolved::UndecidedAtFloor {
                        test: Test::Exclusion,
                    },
                }),
            }
            continue;
        }
        if larger(image(&over_a), image(&over_b)) <= floor {
            out.flagged.push(FlaggedPair {
                first: a,
                second: b,
                why: Unresolved::UndecidedAtFloor {
                    test: Test::LoopFreedom,
                },
            });
            continue;
        }
        // The wider patch is the one split: two pairs, where splitting both would make four.
        if image(&over_a) >= image(&over_b) {
            if let Some((x, y)) = halved(a, speeds(first, a, meter)?, FIRST_CUT) {
                work.push((y, b));
                work.push((x, b));
                continue;
            }
        } else if let Some((x, y)) = halved(b, speeds(second, b, meter)?, SECOND_CUT) {
            work.push((a, y));
            work.push((a, x));
            continue;
        }
        out.flagged.push(FlaggedPair {
            first: a,
            second: b,
            why: Unresolved::UndecidedAtFloor {
                test: Test::LoopFreedom,
            },
        });
    }
    Ok(out)
}
