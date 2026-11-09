//! Inputs for plane-based geometry: [`through`] (`Plane::through`), [`side`] (`Plane::side`),
//! [`tpi_new`] (`Tpi::new`), [`tpi_homogeneous`] (`Tpi::homogeneous`), [`side_of_tpi`]
//! (`Plane::side_of_tpi`), [`to_grid`] (`Tpi::to_grid`), and for the point where a line meets a
//! plane (G29) [`lpi_new`] (`Lpi::new`), [`lpi_homogeneous`] (`Lpi::homogeneous`), [`side_of_lpi`]
//! (`Plane::side_of_lpi`) and [`lpi_to_grid`] (`Lpi::to_grid`).
//!
//! Classes:
//!
//! - `typical`: planes through random points within ±2^30 steps; for `to_grid`, only points that
//!   land inside the frame.
//! - `degenerate`: collinear points (no plane); a point on the plane; for `Tpi::new`, each way three
//!   planes fail to meet in one point, in rotation (two parallel planes with equal normals, two
//!   with parallel but unequal normals, three planes through one line, a plane given twice); a
//!   meeting point on the fourth plane.
//! - `nearly_degenerate`: a point, or a meeting point, exactly one unit of n · p + d off the plane,
//!   the closest a grid point off it can be: the plane is spanned by two vectors of a unimodular
//!   lattice basis and the point lies one third basis vector away, so n · p + d = ±1 next to terms
//!   above 2^100.
//! - `extreme`: planes through points within 2^10 steps of ±(2^40 − 1) that span the frame, so
//!   coefficients come close to their bounds (a normal component of at least 2^80, against 2^83).
//!   The first `through` input is three corners of the bounding cube, whose normal components are
//!   all 4M² (M = 2^40 − 1): the largest the frame allows, because each component is twice the
//!   area of the triangle projected on a coordinate plane, and a triangle in a square of side 2M
//!   has area at most 2M².
//! - `on_grid`: three planes through one grid point, so committing it is exact.
//! - `overflow`: three planes that meet beyond the frame, more than 2^40 − 1 steps from the origin
//!   but less than 2^60, which takes `to_grid`'s longest path: 127 steps of long division.
//!
//! Method: the degenerate cases of plane-based geometry as `Tpi::new` documents them (after Campen
//! and Kobbelt 2010, <https://doi.org/10.1111/j.1467-8659.2009.01609.x>), with Shewchuk's input classes
//! (<https://doi.org/10.1007/PL00009321>, section 6) and unimodular bases (Knuth, volume 2,
//! section 4.5.2) for the nearly degenerate ones.
//!
//! Guarantees: every batch is full, the same on every target, and its classes are what they
//! claim (`tests/inputs.rs`); every result is checked against exact cross products, Cramer's
//! rule and exact rounding by `tests/attack_g06.rs`.
//!
//! Cost: a few draws of [`crate::rng::SplitMix64`] per input, once per batch, outside every
//! measured run.

use core::cmp::Ordering;
use geom_num::Sign;
use geom_num::grid::{Coord, P3};

use geom_num::plane::{Lpi, Moving, Plane, Tpi};

use super::predicate::unimodular3;
use super::{Class, M, TYPICAL_REACH, batch_of, extreme3, p3, rng, typical3, within};
use crate::rng::SplitMix64;

/// Largest component of a direction or spanning vector in degenerate inputs.
const STEP: i64 = 1 << 10;

/// The plane through three points, if they are not collinear.
fn plane([a, b, c]: [P3; 3]) -> Option<Plane> {
    Plane::through(a, b, c).ok()
}

/// A plane through three typical points.
fn typical_plane(g: &mut SplitMix64) -> Option<Plane> {
    plane([typical3(g), typical3(g), typical3(g)])
}

/// Whether a plane spans the frame: a normal component of at least 2^80, against the bound of
/// 2^83. Three extreme points near one corner make a small plane, which is not extreme.
fn spans(p: &Plane) -> bool {
    p.n().iter().any(|n| n.unsigned_abs() >= 1 << 80)
}

/// A plane through three extreme points that spans the frame.
fn extreme_plane(g: &mut SplitMix64) -> Option<Plane> {
    plane([extreme3(g), extreme3(g), extreme3(g)]).filter(spans)
}

/// Whether a side test certainly answered off the plane: `Positive` or `Negative`. A zero, or an
/// error (`BudgetExceeded`, which no plane and point can cause), is rejected rather than guessed.
fn off_plane(side: Result<Sign, geom_num::NumError>) -> bool {
    matches!(side, Ok(Sign::Positive | Sign::Negative))
}

/// The point where three planes meet, if they meet in exactly one point.
fn tpi([p, q, r]: [Plane; 3]) -> Option<Tpi> {
    Tpi::new(p, q, r).ok()
}

/// A small vector with components within ±[`STEP`].
fn small(g: &mut SplitMix64) -> [i64; 3] {
    [0; 3].map(|_| within(g, STEP))
}

/// A grid point as raw step counts.
fn raw(p: P3) -> [i64; 3] {
    [p.x.get(), p.y.get(), p.z.get()]
}

/// `p + v` for a grid point and raw step offsets.
fn offset(p: P3, [x, y, z]: [i64; 3]) -> P3 {
    let [px, py, pz] = raw(p);
    p3(px + x, py + y, pz + z)
}

/// `v · k`, componentwise.
fn scaled([x, y, z]: [i64; 3], k: i64) -> [i64; 3] {
    [x * k, y * k, z * k]
}

/// A pseudo-random plane through the grid point `at`.
fn plane_at(g: &mut SplitMix64, at: P3) -> Option<Plane> {
    let (u, v) = (small(g), small(g));
    plane([at, offset(at, u), offset(at, v)])
}

/// Three pseudo-random planes through the grid point `at`, meeting only there.
fn tpi_at(g: &mut SplitMix64, at: P3) -> Option<Tpi> {
    tpi([plane_at(g, at)?, plane_at(g, at)?, plane_at(g, at)?])
}

/// A meeting point of three typical planes.
fn typical_tpi(g: &mut SplitMix64) -> Option<Tpi> {
    tpi([typical_plane(g)?, typical_plane(g)?, typical_plane(g)?])
}

/// A meeting point of three extreme planes.
fn extreme_tpi(g: &mut SplitMix64) -> Option<Tpi> {
    tpi([extreme_plane(g)?, extreme_plane(g)?, extreme_plane(g)?])
}

/// A plane and a grid point exactly one unit of n · p + d from it: the plane through b, b + u and
/// b + v, and the point b + w, for a unimodular basis u, v, w, so n · (b + w) + d = n · w =
/// det[u; v; w] = 1 (with n = u × v). b is placed so that the point is `at`.
fn one_unit_off(g: &mut SplitMix64, at: P3) -> Option<Plane> {
    let [u, v, w] = unimodular3(g)?;
    let b = offset(at, scaled(w, -1));
    plane([b, offset(b, u), offset(b, v)])
}

/// Point triples for [`Plane::through`].
#[must_use]
pub fn through(class: Class) -> Vec<[P3; 3]> {
    let mut g = rng("plane::through", class);
    match class {
        Class::Typical => batch_of(|_| {
            let t = [typical3(&mut g), typical3(&mut g), typical3(&mut g)];
            plane(t).map(|_| t)
        }),
        Class::Degenerate => batch_of(|_| {
            // a, a + s·d and a + t·d lie on one line.
            let a = typical3(&mut g);
            let d = small(&mut g);
            let (s, t) = (within(&mut g, STEP), within(&mut g, STEP));
            Some([a, offset(a, scaled(d, s)), offset(a, scaled(d, t))])
        }),
        Class::Extreme => batch_of(|n| {
            let t = if n == 0 {
                // Three corners of the bounding cube: every normal component is 4M², the largest
                // the frame allows (see the module documentation).
                [p3(-M, -M, -M), p3(M, M, -M), p3(M, -M, M)]
            } else {
                [extreme3(&mut g), extreme3(&mut g), extreme3(&mut g)]
            };
            plane(t).filter(spans).map(|_| t)
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// (plane, grid point) pairs for [`Plane::side`].
#[must_use]
pub fn side(class: Class) -> Vec<(Plane, P3)> {
    side_from("plane::side", class)
}

/// [`side`]'s classes drawn from the seed of `op`: the batch another benchmark of the same
/// plane test measures (the generated filter and exact value, [`super::generated`]), on inputs of its own.
pub(crate) fn side_from(op: &str, class: Class) -> Vec<(Plane, P3)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (p, q) = (typical_plane(&mut g)?, typical3(&mut g));
            off_plane(p.side(q)).then_some((p, q))
        }),
        Class::Degenerate => batch_of(|_| {
            // a + s(b − a) + u(c − a) lies on the plane through a, b and c.
            let t @ [a, b, c] = [(); 3].map(|()| typical3(&mut g));
            let p = plane(t)?;
            let [s, u] = [(); 2].map(|()| g.range(-3, 3));
            let ([ax, ay, az], [bx, by, bz], [cx, cy, cz]) = (raw(a), raw(b), raw(c));
            let on = [
                s * (bx - ax) + u * (cx - ax),
                s * (by - ay) + u * (cy - ay),
                s * (bz - az) + u * (cz - az),
            ];
            Some((p, offset(a, on)))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let at = typical3(&mut g);
            Some((one_unit_off(&mut g, at)?, at))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_plane(&mut g)?, extreme3(&mut g)))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Plane triples for [`Tpi::new`].
#[must_use]
pub fn tpi_new(class: Class) -> Vec<[Plane; 3]> {
    let mut g = rng("plane::tpi_new", class);
    match class {
        Class::Typical => batch_of(|_| {
            let planes = [
                typical_plane(&mut g)?,
                typical_plane(&mut g)?,
                typical_plane(&mut g)?,
            ];
            tpi(planes).map(|_| planes)
        }),
        Class::Degenerate => batch_of(|n| {
            let t @ [a, b, c] = [(); 3].map(|()| typical3(&mut g));
            let first = plane(t)?;
            let other = typical_plane(&mut g)?;
            Some(match n % 4 {
                // Parallel, equal normals: the first plane moved by a whole-step offset.
                0 => {
                    let shift = small(&mut g);
                    [first, other, plane([a, b, c].map(|p| offset(p, shift)))?]
                }
                // Parallel, unequal normals: a copy scaled by two about a moved point, so its
                // normal is four times the first's, and reversed every other time.
                1 => {
                    let base = offset(a, small(&mut g));
                    let ([ax, ay, az], [bx, by, bz], [cx, cy, cz]) = (raw(a), raw(b), raw(c));
                    let u = [2 * (bx - ax), 2 * (by - ay), 2 * (bz - az)];
                    let v = [2 * (cx - ax), 2 * (cy - ay), 2 * (cz - az)];
                    let copy = if n % 8 == 1 {
                        [base, offset(base, u), offset(base, v)]
                    } else {
                        [base, offset(base, v), offset(base, u)]
                    };
                    [first, plane(copy)?, other]
                }
                // Three planes through one line: the line through a and b, and a third point each.
                2 => {
                    let mut through_ab = || plane([a, b, typical3(&mut g)]);
                    [through_ab()?, through_ab()?, through_ab()?]
                }
                // A plane given twice.
                _ => [first, other, first],
            })
        }),
        Class::Extreme => batch_of(|_| {
            let planes = [
                extreme_plane(&mut g)?,
                extreme_plane(&mut g)?,
                extreme_plane(&mut g)?,
            ];
            tpi(planes).map(|_| planes)
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Meeting points for [`Tpi::homogeneous`].
#[must_use]
pub fn tpi_homogeneous(class: Class) -> Vec<Tpi> {
    let mut g = rng("plane::tpi_homogeneous", class);
    match class {
        Class::Typical => batch_of(|_| typical_tpi(&mut g)),
        Class::Extreme => batch_of(|_| extreme_tpi(&mut g)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (plane, meeting point) pairs for [`Plane::side_of_tpi`].
#[must_use]
pub fn side_of_tpi(class: Class) -> Vec<(Plane, Tpi)> {
    side_of_tpi_from("plane::side_of_tpi", class)
}

/// [`side_of_tpi`]'s classes drawn from the seed of `op`: the batch the generated `tpi_side` and
/// `tpi_weight` benchmarks measure ([`super::generated`]), on inputs of their own.
pub(crate) fn side_of_tpi_from(op: &str, class: Class) -> Vec<(Plane, Tpi)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (p, t) = (typical_plane(&mut g)?, typical_tpi(&mut g)?);
            off_plane(p.side_of_tpi(&t)).then_some((p, t))
        }),
        Class::Degenerate => batch_of(|_| {
            // Four planes through one grid point: the meeting point of three lies on the fourth.
            let at = typical3(&mut g);
            Some((plane_at(&mut g, at)?, tpi_at(&mut g, at)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            // Three planes meet exactly at a grid point; the fourth is one unit from that point.
            let at = typical3(&mut g);
            Some((one_unit_off(&mut g, at)?, tpi_at(&mut g, at)?))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_plane(&mut g)?, extreme_tpi(&mut g)?))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// n · at + d for a plane and a grid point, exactly, or `None` past `i128`, which no plane within
/// its budget and grid point reaches (|n · at + d| < 3 · 2^83 · 2^40 + 2^125 < 2^127).
fn value_at(p: &Plane, at: P3) -> Option<i128> {
    let [nx, ny, nz] = p.n();
    let [x, y, z] = raw(at).map(i128::from);
    nx.checked_mul(x)?
        .checked_add(ny.checked_mul(y)?)?
        .checked_add(nz.checked_mul(z)?)?
        .checked_add(p.d())
}

/// Whether an order was decided either way: a tie, or an error, is rejected rather than guessed.
fn apart(order: Result<Ordering, geom_num::NumError>) -> bool {
    matches!(order, Ok(Ordering::Less | Ordering::Greater))
}

/// The planes of two meeting points as `generated::tpi_height` takes them, the first point's three
/// then the second's.
fn height_args(a: &Tpi, b: &Tpi) -> [i128; 24] {
    let mut out = [0; 24];
    for (k, q) in a.planes().iter().chain(b.planes()).enumerate() {
        let [x, y, z] = q.n();
        for (slot, v) in out.iter_mut().skip(4 * k).zip([x, y, z, q.d()]) {
            *slot = v;
        }
    }
    out
}

/// Whether `generated::tpi_height`'s filter cannot order the two points: the nearly degenerate
/// class's claim.
fn height_undecided(a: &Tpi, b: &Tpi) -> bool {
    let [
        n1x,
        n1y,
        n1z,
        d1,
        n2x,
        n2y,
        n2z,
        d2,
        n3x,
        n3y,
        n3z,
        d3,
        n4x,
        n4y,
        n4z,
        d4,
        n5x,
        n5y,
        n5z,
        d5,
        n6x,
        n6y,
        n6z,
        d6,
    ] = height_args(a, b);
    geom_num::generated::tpi_height::filter(
        n1x, n1y, n1z, d1, n2x, n2y, n2z, d2, n3x, n3y, n3z, d3, n4x, n4y, n4z, d4, n5x, n5y, n5z,
        d5, n6x, n6y, n6z, d6,
    )
    .is_none()
}

/// Pairs of meeting points for [`Tpi::cmp_height`] (piece G251).
#[must_use]
pub fn cmp_height(class: Class) -> Vec<(Tpi, Tpi)> {
    cmp_height_from("plane::cmp_height", class)
}

/// [`cmp_height`]'s classes drawn from the seed of `op`: the batch the generated `tpi_height`
/// benchmarks measure ([`super::generated`]), on inputs of their own.
///
/// - `typical`: two meeting points of typical planes at different heights.
/// - `degenerate`: two meeting points of planes through two grid points at one height, apart.
/// - `nearly_degenerate`: a meeting point at a grid point, and the meeting of two planes through
///   that point and a third one unit of n · p + d off it, a sliver away, which the filter cannot
///   order.
/// - `extreme`: two meeting points of extreme planes.
pub(crate) fn cmp_height_from(op: &str, class: Class) -> Vec<(Tpi, Tpi)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (a, b) = (typical_tpi(&mut g)?, typical_tpi(&mut g)?);
            apart(a.cmp_height(&b)).then_some((a, b))
        }),
        Class::Degenerate => batch_of(|_| {
            let at = typical3(&mut g);
            let [_, _, z] = raw(at);
            let level = p3(
                within(&mut g, TYPICAL_REACH),
                within(&mut g, TYPICAL_REACH),
                z,
            );
            (level != at).then_some(())?;
            Some((tpi_at(&mut g, at)?, tpi_at(&mut g, level)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let at = typical3(&mut g);
            let a = tpi_at(&mut g, at)?;
            let near = tpi([
                plane_at(&mut g, at)?,
                plane_at(&mut g, at)?,
                one_unit_off(&mut g, at)?,
            ])?;
            (apart(a.cmp_height(&near)) && height_undecided(&a, &near)).then_some((a, near))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_tpi(&mut g)?, extreme_tpi(&mut g)?))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// The largest speed a moving plane may have, 2^125 − 1 (`Moving::at_speed`).
pub const SPEED_LIMIT: i128 = (1 << 125) - 1;

/// A plane moving at `speed`, if the speed is within its budget.
fn moving(p: Plane, speed: i128) -> Option<Moving> {
    Moving::at_speed(p, speed).ok()
}

/// Four typical planes at typical speeds, within ±2^60.
fn typical_meeting(g: &mut SplitMix64) -> Option<[Moving; 4]> {
    let mut one = || moving(typical_plane(g)?, i128::from(within(g, 1 << 60)));
    Some([one()?, one()?, one()?, one()?])
}

/// Four extreme planes, each at the largest speed either way.
fn extreme_meeting(g: &mut SplitMix64) -> Option<[Moving; 4]> {
    let mut one = || moving(extreme_plane(g)?, i128::from(g.sign()) * SPEED_LIMIT);
    Some([one()?, one()?, one()?, one()?])
}

/// Three planes standing still through the grid point `at`, meeting only there, and a typical
/// plane moving at its own value at `at`, v = n · at + d, plus `nudge` away from zero: the four
/// meet when the moving plane reaches `at`, at t = −v / (v ± `nudge`), so at t = −1 for no nudge.
fn meeting_at(g: &mut SplitMix64, at: P3, nudge: i128) -> Option<[Moving; 4]> {
    let still = [plane_at(g, at)?, plane_at(g, at)?, plane_at(g, at)?];
    tpi(still)?;
    let slant = typical_plane(g)?;
    let v = value_at(&slant, at)?;
    (v != 0).then_some(())?;
    let speed = v.checked_add(nudge.checked_mul(v.signum())?)?;
    let [p, q, r] = still.map(|s| moving(s, 0));
    Some([p?, q?, r?, moving(slant, speed)?])
}

/// The planes and speeds of two meetings as `generated::mitre_time` takes them, the first four
/// then the second's.
fn meeting_args(a: &[Moving; 4], b: &[Moving; 4]) -> [i128; 40] {
    let mut out = [0; 40];
    for (k, m) in a.iter().chain(b).enumerate() {
        let [x, y, z] = m.plane().n();
        for (slot, v) in out
            .iter_mut()
            .skip(5 * k)
            .zip([x, y, z, m.plane().d(), m.speed()])
        {
            *slot = v;
        }
    }
    out
}

/// Whether `generated::mitre_time`'s filter cannot order the two meetings: the nearly degenerate
/// class's claim.
fn meeting_undecided(a: &[Moving; 4], b: &[Moving; 4]) -> bool {
    let [
        n1x,
        n1y,
        n1z,
        d1,
        s1,
        n2x,
        n2y,
        n2z,
        d2,
        s2,
        n3x,
        n3y,
        n3z,
        d3,
        s3,
        n4x,
        n4y,
        n4z,
        d4,
        s4,
        n5x,
        n5y,
        n5z,
        d5,
        s5,
        n6x,
        n6y,
        n6z,
        d6,
        s6,
        n7x,
        n7y,
        n7z,
        d7,
        s7,
        n8x,
        n8y,
        n8z,
        d8,
        s8,
    ] = meeting_args(a, b);
    geom_num::generated::mitre_time::filter(
        n1x, n1y, n1z, d1, s1, n2x, n2y, n2z, d2, s2, n3x, n3y, n3z, d3, s3, n4x, n4y, n4z, d4, s4,
        n5x, n5y, n5z, d5, s5, n6x, n6y, n6z, d6, s6, n7x, n7y, n7z, d7, s7, n8x, n8y, n8z, d8, s8,
    )
    .is_none()
}

/// Pairs of meetings of four moving planes for [`Moving::cmp_meeting`] (piece G251).
#[must_use]
pub fn cmp_meeting(class: Class) -> Vec<([Moving; 4], [Moving; 4])> {
    cmp_meeting_from("plane::cmp_meeting", class)
}

/// [`cmp_meeting`]'s classes drawn from the seed of `op`: the batch the generated `mitre_time`
/// benchmarks measure ([`super::generated`]), on inputs of their own.
///
/// - `typical`: two meetings of typical planes at typical speeds, at different times.
/// - `degenerate`: two meetings at one time: in each, three planes standing still through one grid
///   point and a fourth moving at its own value there, so each meets at t = −1.
/// - `nearly_degenerate`: the same, the second's speed one more away from zero, so it meets a
///   sliver after −1, which the filter cannot order.
/// - `extreme`: two meetings of extreme planes, each at the largest speed either way.
pub(crate) fn cmp_meeting_from(op: &str, class: Class) -> Vec<([Moving; 4], [Moving; 4])> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (a, b) = (typical_meeting(&mut g)?, typical_meeting(&mut g)?);
            apart(Moving::cmp_meeting(&a, &b)).then_some((a, b))
        }),
        Class::Degenerate => batch_of(|_| {
            let (p, q) = (typical3(&mut g), typical3(&mut g));
            Some((meeting_at(&mut g, p, 0)?, meeting_at(&mut g, q, 0)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            let (p, q) = (typical3(&mut g), typical3(&mut g));
            let (a, b) = (meeting_at(&mut g, p, 0)?, meeting_at(&mut g, q, 1)?);
            (apart(Moving::cmp_meeting(&a, &b)) && meeting_undecided(&a, &b)).then_some((a, b))
        }),
        Class::Extreme => batch_of(|_| {
            let (a, b) = (extreme_meeting(&mut g)?, extreme_meeting(&mut g)?);
            Moving::cmp_meeting(&a, &b).is_ok().then_some((a, b))
        }),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// A time, a whole number of steps within `reach`, as `Moving::cmp_meeting_time` takes it.
fn time_within(g: &mut SplitMix64, reach: i64) -> Option<Coord> {
    Coord::new(within(g, reach)).ok()
}

/// Meetings of four moving planes with a time for [`Moving::cmp_meeting_time`] (piece G273).
///
/// - `typical`: typical planes at typical speeds, and a time within 2^20 steps.
/// - `degenerate`: three planes standing still through one grid point and a fourth moving at its
///   own value there, which meet at t = −1, and the time −1: the meeting is exactly then.
/// - `extreme`: extreme planes at the largest speed either way, and a time at either end of the
///   range.
#[must_use]
pub fn cmp_meeting_time(class: Class) -> Vec<([Moving; 4], Coord)> {
    let mut g = rng("plane::cmp_meeting_time", class);
    match class {
        Class::Typical => batch_of(|_| {
            let four = typical_meeting(&mut g)?;
            let time = time_within(&mut g, 1 << 20)?;
            Moving::cmp_meeting_time(&four, time)
                .is_ok()
                .then_some((four, time))
        }),
        Class::Degenerate => batch_of(|_| {
            let at = typical3(&mut g);
            let four = meeting_at(&mut g, at, 0)?;
            let time = Coord::new(-1).ok()?;
            (Moving::cmp_meeting_time(&four, time) == Ok(Ordering::Equal)).then_some((four, time))
        }),
        Class::Extreme => batch_of(|_| {
            let four = extreme_meeting(&mut g)?;
            let time = Coord::new(g.sign() * M).ok()?;
            Moving::cmp_meeting_time(&four, time)
                .is_ok()
                .then_some((four, time))
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Meetings of four moving planes and a fifth plane for [`Moving::side_at_meeting`] (piece
/// G273).
///
/// - `typical`: typical planes at typical speeds.
/// - `degenerate`: the fifth plane is one of the four, so the meeting lies on it.
/// - `extreme`: extreme planes at the largest speed either way.
#[must_use]
pub fn side_at_meeting(class: Class) -> Vec<([Moving; 4], Moving)> {
    let mut g = rng("plane::side_at_meeting", class);
    match class {
        Class::Typical => batch_of(|_| {
            let four = typical_meeting(&mut g)?;
            let other = moving(typical_plane(&mut g)?, i128::from(within(&mut g, 1 << 60)))?;
            Moving::side_at_meeting(&four, &other)
                .is_ok()
                .then_some((four, other))
        }),
        Class::Degenerate => batch_of(|k| {
            let at = typical3(&mut g);
            let four = meeting_at(&mut g, at, 0)?;
            let other = *four.get(k % 4)?;
            (Moving::side_at_meeting(&four, &other) == Ok(Sign::Zero)).then_some((four, other))
        }),
        Class::Extreme => batch_of(|_| {
            let four = extreme_meeting(&mut g)?;
            let other = moving(extreme_plane(&mut g)?, i128::from(g.sign()) * SPEED_LIMIT)?;
            Moving::side_at_meeting(&four, &other)
                .is_ok()
                .then_some((four, other))
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Meeting points for [`Tpi::to_grid`].
#[must_use]
pub fn to_grid(class: Class) -> Vec<Tpi> {
    let mut g = rng("plane::to_grid", class);
    match class {
        Class::Typical => batch_of(|_| typical_tpi(&mut g).filter(|t| t.to_grid().is_ok())),
        Class::Extreme => batch_of(|_| extreme_tpi(&mut g).filter(|t| t.to_grid().is_ok())),
        Class::OnGrid => batch_of(|_| {
            let at = typical3(&mut g);
            tpi_at(&mut g, at)
        }),
        Class::Overflow => batch_of(|_| far_tpi(&mut g)),
        Class::Degenerate | Class::NearlyDegenerate | Class::WrongDirection => Vec::new(),
    }
}

/// Three planes meeting at (x0, y0 + d·2^39, z0), beyond the frame (|y| ≥ 2^40) but below 2^60:
///
/// - z = z0, through (x0, y0, z0), (x0 + 1, y0, z0) and (x0, y0 + 1, z0);
/// - z = z0 + d − (y − y0) / 2^39, through (x0, y0, z0 + d), (x0 + 1, y0, z0 + d) and
///   (x0, y0 + 2^39, z0 + d − 1), which reaches z = z0 at y = y0 + d·2^39;
/// - x = x0, through (x0, 0, 0), (x0, 1, 0) and (x0, 0, 1).
///
/// With y0 = −2^39 + r (0 ≤ r ≤ 2^20) and 3 ≤ d ≤ 2^20 the meeting point's y is at least 2^40,
/// and every defining point is inside the frame. A pseudo-random sign mirrors y.
fn far_tpi(g: &mut SplitMix64) -> Option<Tpi> {
    const SPAN: i64 = 1 << 39;
    let [x0, z0] = [(); 2].map(|()| within(g, TYPICAL_REACH));
    let d = g.range(3, 1 << 20);
    let y0 = -SPAN + g.range(0, 1 << 20);
    let s = g.sign();
    let at = |x: i64, y: i64, z: i64| p3(x, s * y, z);
    let floor = plane([at(x0, y0, z0), at(x0 + 1, y0, z0), at(x0, y0 + 1, z0)])?;
    let top = z0 + d;
    let ramp = plane([
        at(x0, y0, top),
        at(x0 + 1, y0, top),
        at(x0, y0 + SPAN, top - 1),
    ])?;
    let wall = plane([at(x0, 0, 0), at(x0, 1, 0), at(x0, 0, 1)])?;
    tpi([floor, ramp, wall])
}

/// The point where the line from `p` to `q` meets `plane`, if it meets it in exactly one point.
fn lpi(p: P3, q: P3, plane: Plane) -> Option<Lpi> {
    Lpi::new(p, q, plane).ok()
}

/// A line through the grid point `at` meeting a pseudo-random plane through `at`, so the meeting
/// point is `at` itself.
fn lpi_at(g: &mut SplitMix64, at: P3) -> Option<Lpi> {
    let u = small(g);
    lpi(offset(at, scaled(u, -1)), offset(at, u), plane_at(g, at)?)
}

/// A line between two typical points meeting a typical plane.
fn typical_lpi(g: &mut SplitMix64) -> Option<Lpi> {
    lpi(typical3(g), typical3(g), typical_plane(g)?)
}

/// A line between two extreme points meeting a plane that spans the frame.
fn extreme_lpi(g: &mut SplitMix64) -> Option<Lpi> {
    lpi(extreme3(g), extreme3(g), extreme_plane(g)?)
}

/// (p, q, plane) triples for [`Lpi::new`].
#[must_use]
pub fn lpi_new(class: Class) -> Vec<(P3, P3, Plane)> {
    let mut g = rng("plane::lpi_new", class);
    match class {
        Class::Typical => batch_of(|_| {
            let (p, q, pl) = (typical3(&mut g), typical3(&mut g), typical_plane(&mut g)?);
            lpi(p, q, pl).map(|_| (p, q, pl))
        }),
        Class::Degenerate => batch_of(|n| {
            // The plane through a, b, c and a line along b − a: parallel to the plane, or inside
            // it when it starts at a, in rotation. Its weight n · (q − p) is exactly zero.
            let [a, b, c] = [(); 3].map(|()| typical3(&mut g));
            let pl = plane([a, b, c])?;
            let start = if n % 2 == 0 {
                a
            } else {
                offset(a, small(&mut g))
            };
            let ([bx, by, bz], [ax, ay, az]) = (raw(b), raw(a));
            let along = [bx - ax, by - ay, bz - az];
            Some((start, offset(start, along), pl))
        }),
        Class::Extreme => batch_of(|_| {
            let (p, q, pl) = (extreme3(&mut g), extreme3(&mut g), extreme_plane(&mut g)?);
            lpi(p, q, pl).map(|_| (p, q, pl))
        }),
        Class::NearlyDegenerate | Class::Overflow | Class::OnGrid | Class::WrongDirection => {
            Vec::new()
        }
    }
}

/// Line-plane points for [`Lpi::homogeneous`].
#[must_use]
pub fn lpi_homogeneous(class: Class) -> Vec<Lpi> {
    let mut g = rng("plane::lpi_homogeneous", class);
    match class {
        Class::Typical => batch_of(|_| typical_lpi(&mut g)),
        Class::Extreme => batch_of(|_| extreme_lpi(&mut g)),
        Class::Degenerate
        | Class::NearlyDegenerate
        | Class::Overflow
        | Class::OnGrid
        | Class::WrongDirection => Vec::new(),
    }
}

/// (plane, line-plane point) pairs for [`Plane::side_of_lpi`].
#[must_use]
pub fn side_of_lpi(class: Class) -> Vec<(Plane, Lpi)> {
    side_of_lpi_from("plane::side_of_lpi", class)
}

/// [`side_of_lpi`]'s classes drawn from the seed of `op`: the batch the generated `lpi_side` and
/// `lpi_weight` benchmarks measure ([`super::generated`]), on inputs of their own.
pub(crate) fn side_of_lpi_from(op: &str, class: Class) -> Vec<(Plane, Lpi)> {
    let mut g = rng(op, class);
    match class {
        Class::Typical => batch_of(|_| {
            let (pl, l) = (typical_plane(&mut g)?, typical_lpi(&mut g)?);
            off_plane(pl.side_of_lpi(&l)).then_some((pl, l))
        }),
        Class::Degenerate => batch_of(|_| {
            // The line meets its plane at a grid point that the classifying plane passes through.
            let at = typical3(&mut g);
            Some((plane_at(&mut g, at)?, lpi_at(&mut g, at)?))
        }),
        Class::NearlyDegenerate => batch_of(|_| {
            // The line meets its plane at a grid point one unit from the classifying plane.
            let at = typical3(&mut g);
            Some((one_unit_off(&mut g, at)?, lpi_at(&mut g, at)?))
        }),
        Class::Extreme => batch_of(|_| Some((extreme_plane(&mut g)?, extreme_lpi(&mut g)?))),
        Class::Overflow | Class::OnGrid | Class::WrongDirection => Vec::new(),
    }
}

/// Line-plane points for [`Lpi::to_grid`].
#[must_use]
pub fn lpi_to_grid(class: Class) -> Vec<Lpi> {
    let mut g = rng("plane::lpi_to_grid", class);
    match class {
        Class::Typical => batch_of(|_| typical_lpi(&mut g).filter(|l| l.to_grid().is_ok())),
        Class::Extreme => batch_of(|_| extreme_lpi(&mut g).filter(|l| l.to_grid().is_ok())),
        Class::OnGrid => batch_of(|_| {
            let at = typical3(&mut g);
            lpi_at(&mut g, at)
        }),
        Class::Overflow => batch_of(|_| far_lpi(&mut g)),
        Class::Degenerate | Class::NearlyDegenerate | Class::WrongDirection => Vec::new(),
    }
}

/// A line that meets a plane outside the frame: the plane x = 0 and the line from
/// (−M + r, y0, z0) to (−M + r + 1, y0 + s, z0), whose weight is 1, so the point is s · (M − r)
/// past the start in y: beyond the frame for s ≥ 4 and r ≤ 2^20.
fn far_lpi(g: &mut SplitMix64) -> Option<Lpi> {
    let wall = plane([p3(0, 0, 0), p3(0, 1, 0), p3(0, 0, 1)])?;
    let r = g.range(0, 1 << 20);
    let (y0, z0) = (within(g, TYPICAL_REACH), within(g, TYPICAL_REACH));
    let s = g.sign() * g.range(4, 1 << 20);
    lpi(p3(-M + r, y0, z0), p3(-M + r + 1, y0 + s, z0), wall)
}
