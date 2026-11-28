//! A room's volume with the floors that stand in its space taken out, exactly (job 6.3,
//! CON-A4-F7-P1).
//!
//! A room is bounded below by its base (its level's elevation plus its offset) and above by its
//! upper limit (the level above, or a height). A floor in that space takes some of it: a floor
//! whose top face is above the base (a raised floor) takes what stands between the base and its
//! top, and the floor of the level above, whose underside dips below the upper limit, takes its
//! thickness from the room's top. Every column of the room, at plan point `(x, y)`, is the height
//! from the base to the upper limit less, for each floor over that point, the height of the floor's
//! slab inside it:
//!
//! `take(x, y) = max(0, min(top(x, y), upper) - max(top(x, y) - thickness, base))`
//!
//! where `top(x, y)` is the floor's top face, flat or one plane (a sloped floor, its thickness
//! plumb). The volume is the room's area times its height less the integral of `take` over the
//! part of each floor inside the room.
//!
//! How it is worked out, exactly: `take` is linear in the height of the floor's top, which is
//! linear in plan, so the plan splits into strips, between lines of equal height of the top, on
//! each of which `take` is one linear function; the integral of a linear function over a polygon is
//! a sum round its edges (Green's theorem), exact in rationals. The part of a floor inside the
//! room inside a strip is the intersection of three regions with holes, found by cutting every
//! edge where another region's edge meets it and keeping the pieces that lie inside the others,
//! the same way the room search keeps a wall's edge pieces. Nothing is a float and nothing is
//! rounded but the answer, once, to the nearest whole cubic step (a half up). A floor's openings
//! are holes in its loops, so what is cut out of a floor takes nothing.
//!
//! This is our own exact geometry, for the reason [`crate::plane`] gives, on the rationals of
//! [`crate::exact`]. Ceilings are not built yet (job 6.8), so a room is bounded by its levels and
//! the floors only; when ceilings land, a ceiling's underside is one more surface that bounds a
//! room from above, read by the same integral.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals (dashu-ratio) cannot overflow; the lint guards fixed-width numbers"
)]

use crate::error::RoomError;
use crate::exact::{Pt, Side, cross, meeting, side, sign, whole};
use crate::plane::{Found, LIMIT};
use construct_params::Volume;
use dashu_int::IBig;
use dashu_ratio::RBig;
use std::cmp::Ordering;

/// A floor as the volume reads it: the loops of its boundary in plan (the outer loop first, then
/// each hole), its top face (flat or one plane) and its thickness, in grid steps with heights above
/// the project's zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slab {
    /// The outer loop, then each hole, as grid points in either direction round.
    loops: Vec<Vec<[i64; 2]>>,
    /// The top face's height at plan point `(x, y)`: `ax * x + ay * y + c`.
    ax: RBig,
    ay: RBig,
    c: RBig,
    thickness: i64,
}

impl Slab {
    /// A floor whose top face is flat at the height `top`.
    #[must_use]
    pub fn flat(loops: Vec<Vec<[i64; 2]>>, top: i64, thickness: i64) -> Self {
        Self {
            loops,
            ax: RBig::ZERO,
            ay: RBig::ZERO,
            c: whole(top),
            thickness,
        }
    }

    /// A floor whose top face passes through the corners `points` (`[x, y, z]`, the loops' corners
    /// with the top's height at each): flat when they are all at one height, else the plane through
    /// the first and the first two after it that are not on one line.
    ///
    /// # Errors
    /// [`RoomError::NoPlane`] when there are no points, or the heights differ but no three corners
    /// make a plane.
    pub fn of_corners(
        loops: Vec<Vec<[i64; 2]>>,
        points: &[[i64; 3]],
        thickness: i64,
    ) -> Result<Self, RoomError> {
        let first = *points.first().ok_or(RoomError::NoPlane)?;
        if points.iter().all(|p| p[2] == first[2]) {
            return Ok(Self::flat(loops, first[2], thickness));
        }
        for (a, &second) in points.iter().enumerate().skip(1) {
            for &third in points.iter().skip(a.saturating_add(1)) {
                if let Ok(slab) = Self::sloped(loops.clone(), [first, second, third], thickness) {
                    // Three points on one line have no plane; a plane through them is upright
                    // only if they are on one line in plan, which `sloped` refuses too.
                    return Ok(slab);
                }
            }
        }
        Err(RoomError::NoPlane)
    }

    /// A floor whose top face is the plane through three points `[x, y, z]`.
    ///
    /// # Errors
    /// [`RoomError::NoPlane`] when the points are on one line or the plane is upright.
    pub fn sloped(
        loops: Vec<Vec<[i64; 2]>>,
        through: [[i64; 3]; 3],
        thickness: i64,
    ) -> Result<Self, RoomError> {
        let [p0, p1, p2] = through;
        let sub = |a: [i64; 3], b: [i64; 3]| -> [i128; 3] {
            [
                i128::from(a[0]) - i128::from(b[0]),
                i128::from(a[1]) - i128::from(b[1]),
                i128::from(a[2]) - i128::from(b[2]),
            ]
        };
        let (u, v) = (sub(p1, p0), sub(p2, p0));
        // Coordinates are within 2^41, so each product is under 2^84 and a difference of two under 2^85.
        let normal = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        if normal[2] == 0 {
            return Err(RoomError::NoPlane);
        }
        let rat = |n: i128| RBig::from(IBig::from(n));
        let nz = rat(normal[2]);
        let ax = -rat(normal[0]) / &nz;
        let ay = -rat(normal[1]) / &nz;
        let c = whole(p0[2]) - &ax * whole(p0[0]) - &ay * whole(p0[1]);
        Ok(Self {
            loops,
            ax,
            ay,
            c,
            thickness,
        })
    }
}

/// A region of the plan: its outer loop anticlockwise, then each hole clockwise.
struct Region {
    loops: Vec<Vec<Pt>>,
}

fn orient(ring: &[[i64; 2]], anticlockwise: bool) -> Vec<Pt> {
    let mut points: Vec<Pt> = ring.iter().copied().map(Pt::grid).collect();
    let twice = crate::exact::twice_area(&points);
    if (sign(&twice) == Ordering::Greater) != anticlockwise {
        points.reverse();
    }
    points
}

impl Region {
    fn of_loops(loops: &[Vec<[i64; 2]>]) -> Self {
        Self {
            loops: loops
                .iter()
                .enumerate()
                .map(|(k, ring)| orient(ring, k == 0))
                .collect(),
        }
    }

    fn of_found(found: &Found) -> Self {
        let ring = |corners: &[crate::plane::Corner], anticlockwise: bool| {
            let at: Vec<[i64; 2]> = corners.iter().map(|c| c.at).collect();
            orient(&at, anticlockwise)
        };
        let mut loops = vec![ring(&found.outer, true)];
        loops.extend(found.holes.iter().map(|hole| ring(hole, false)));
        Self { loops }
    }

    /// The low and high corners of its box.
    fn extent(&self) -> Option<(RBig, RBig, RBig, RBig)> {
        let mut points = self.loops.first()?.iter();
        let first = points.next()?;
        let mut out = (
            first.x.clone(),
            first.y.clone(),
            first.x.clone(),
            first.y.clone(),
        );
        for p in points {
            if p.x < out.0 {
                out.0.clone_from(&p.x);
            }
            if p.y < out.1 {
                out.1.clone_from(&p.y);
            }
            if p.x > out.2 {
                out.2.clone_from(&p.x);
            }
            if p.y > out.3 {
                out.3.clone_from(&p.y);
            }
        }
        Some(out)
    }
}

/// Where a point is against a region.
enum Rel {
    Inside,
    Outside,
    /// On a loop's edge, which runs along this vector (the region on its left).
    On(Pt),
}

fn relate(p: &Pt, region: &Region) -> Rel {
    for ring in &region.loops {
        if let Side::On(k) = side(p, ring) {
            let (Some(a), Some(b)) = (ring.get(k), ring.get(k.saturating_add(1)).or(ring.first()))
            else {
                continue;
            };
            return Rel::On(b.less(a));
        }
    }
    let mut inside = region
        .loops
        .first()
        .is_some_and(|outer| side(p, outer) == Side::Inside);
    for hole in region.loops.iter().skip(1) {
        if side(p, hole) == Side::Inside {
            inside = false;
        }
    }
    if inside { Rel::Inside } else { Rel::Outside }
}

/// The directed edge pieces that bound the intersection of `regions`, each with the intersection
/// on its left.
fn bounding_pieces(regions: &[&Region]) -> Vec<(Pt, Pt)> {
    let mut kept = Vec::new();
    for (r, region) in regions.iter().enumerate() {
        for ring in &region.loops {
            for (k, a) in ring.iter().enumerate() {
                let Some(b) = ring.get(k.saturating_add(1)).or(ring.first()) else {
                    continue;
                };
                let mut cuts = vec![RBig::ZERO, RBig::ONE];
                for (s, other) in regions.iter().enumerate() {
                    if s == r {
                        continue;
                    }
                    for other_ring in &other.loops {
                        for (j, c) in other_ring.iter().enumerate() {
                            let Some(d) =
                                other_ring.get(j.saturating_add(1)).or(other_ring.first())
                            else {
                                continue;
                            };
                            cuts.extend(meeting(a, b, c, d));
                        }
                    }
                }
                cuts.sort();
                cuts.dedup();
                for pair in cuts.windows(2) {
                    let [t0, t1] = pair else { continue };
                    let (p, q) = (a.toward(b, t0), a.toward(b, t1));
                    let mid = p.mid(&q);
                    let along = q.less(&p);
                    let keeps = regions.iter().enumerate().all(|(s, other)| {
                        if s == r {
                            return true;
                        }
                        match relate(&mid, other) {
                            Rel::Inside => true,
                            Rel::Outside => false,
                            // Along each other the same way: kept once, from the first region.
                            // The other way round: the regions meet on opposite sides, no area.
                            Rel::On(dir) => {
                                sign(&crate::exact::dot(&along, &dir)) == Ordering::Greater && s > r
                            }
                        }
                    });
                    if keeps {
                        kept.push((p, q));
                    }
                }
            }
        }
    }
    kept
}

/// A linear function of plan: `c0 + cx * x + cy * y`.
struct Linear {
    c0: RBig,
    cx: RBig,
    cy: RBig,
}

/// The integral of `f` over the intersection of `regions`.
fn integral(regions: &[&Region], f: &Linear) -> RBig {
    let mut twice_area = RBig::ZERO;
    let mut mx6 = RBig::ZERO;
    let mut my6 = RBig::ZERO;
    for (a, b) in bounding_pieces(regions) {
        let turn = cross(&a, &b);
        mx6 += (&a.x + &b.x) * &turn;
        my6 += (&a.y + &b.y) * &turn;
        twice_area += turn;
    }
    let two = whole(2);
    let six = whole(6);
    &f.c0 * twice_area / two + &f.cx * mx6 / &six + &f.cy * my6 / six
}

fn constant(c0: RBig) -> Linear {
    Linear {
        c0,
        cx: RBig::ZERO,
        cy: RBig::ZERO,
    }
}

/// How much of a column, between `base` and `upper`, a slab whose top is at `s` and that is
/// `thickness` thick takes.
fn column(s: &RBig, thickness: &RBig, base: &RBig, upper: &RBig) -> RBig {
    let top = if s < upper { s.clone() } else { upper.clone() };
    let under = s - thickness;
    let bottom = if under > *base { under } else { base.clone() };
    let height = top - bottom;
    if height > RBig::ZERO {
        height
    } else {
        RBig::ZERO
    }
}

fn magnitude(v: &RBig) -> RBig {
    if *v < RBig::ZERO {
        -v.clone()
    } else {
        v.clone()
    }
}

/// The strip of the plan where the slab's top is between `s0` and `s1`: a quadrilateral far wider
/// than any plan the grid holds, anticlockwise.
fn strip(slab: &Slab, s0: &RBig, s1: &RBig) -> Region {
    let nn = &slab.ax * &slab.ax + &slab.ay * &slab.ay;
    let reach = whole(1_i64 << 42) * (magnitude(&slab.ax) + magnitude(&slab.ay));
    let at = |s: &RBig, w: &RBig| Pt {
        x: &slab.ax * (s - &slab.c) / &nn - &slab.ay * w / &nn,
        y: &slab.ay * (s - &slab.c) / &nn + &slab.ax * w / &nn,
    };
    let low = -reach.clone();
    Region {
        loops: vec![vec![
            at(s0, &low),
            at(s1, &low),
            at(s1, &reach),
            at(s0, &reach),
        ]],
    }
}

/// What `slab` takes of the room `room` between `base` and `upper`.
fn take(room: &Region, slab: &Slab, base: &RBig, upper: &RBig) -> RBig {
    let floor = Region::of_loops(&slab.loops);
    let (Some(a), Some(b)) = (room.extent(), floor.extent()) else {
        return RBig::ZERO;
    };
    if a.0 > b.2 || b.0 > a.2 || a.1 > b.3 || b.1 > a.3 {
        return RBig::ZERO;
    }
    let thickness = whole(slab.thickness);
    if slab.ax == RBig::ZERO && slab.ay == RBig::ZERO {
        let each = column(&slab.c, &thickness, base, upper);
        if each == RBig::ZERO {
            return RBig::ZERO;
        }
        return integral(&[room, &floor], &constant(each));
    }
    let mut marks = vec![
        base.clone(),
        base + &thickness,
        upper.clone(),
        upper + &thickness,
    ];
    marks.sort();
    marks.dedup();
    let mut total = RBig::ZERO;
    for pair in marks.windows(2) {
        let [s0, s1] = pair else { continue };
        let (g0, g1) = (
            column(s0, &thickness, base, upper),
            column(s1, &thickness, base, upper),
        );
        if g0 == RBig::ZERO && g1 == RBig::ZERO {
            continue;
        }
        // Along the strip the take is g0 + m (s - s0), with s the top's height at the point.
        let m = (&g1 - &g0) / (s1 - s0);
        let rise = &g0 - &m * s0 + &m * &slab.c;
        let f = Linear {
            c0: rise,
            cx: &m * &slab.ax,
            cy: &m * &slab.ay,
        };
        let band = strip(slab, s0, s1);
        total += integral(&[room, &floor, &band], &f);
    }
    total
}

/// The nearest whole number to `v`, a half rounding up.
fn nearest(v: &RBig) -> Option<i128> {
    let up = (v + whole(1) / whole(2)).floor();
    i128::try_from(up).ok()
}

/// The volume of the room `found`, from `base` to `upper` (heights above the project's zero, in
/// grid steps, `upper` above `base`), less what each of `slabs` takes of it, rounded once to the
/// nearest whole cubic step.
///
/// # Errors
/// [`RoomError::NoHeight`] for an upper limit at or below the base; [`RoomError::TooBig`] for a
/// coordinate beyond the kernel's grid or a volume the type cannot hold.
pub fn net_volume(
    found: &Found,
    base: i64,
    upper: i64,
    slabs: &[Slab],
) -> Result<Volume, RoomError> {
    if upper <= base {
        return Err(RoomError::NoHeight);
    }
    let beyond = |v: &i64| v.unsigned_abs() > LIMIT.unsigned_abs();
    if found.outer.iter().any(|c| c.at.iter().any(beyond))
        || slabs
            .iter()
            .any(|slab| slab.loops.iter().flatten().flatten().any(beyond))
    {
        return Err(RoomError::TooBig);
    }
    let room = Region::of_found(found);
    let (lo, hi) = (whole(base), whole(upper));
    let gross = integral(&[&room], &constant(&hi - &lo));
    let mut net = gross;
    for slab in slabs {
        net -= take(&room, slab, &lo, &hi);
    }
    if net < RBig::ZERO {
        net = RBig::ZERO;
    }
    let cubic = nearest(&net).ok_or(RoomError::TooBig)?;
    Ok(Volume::from_cubic_steps(cubic))
}
