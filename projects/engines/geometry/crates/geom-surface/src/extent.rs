//! Closed-form extents of the analytic surface kinds: the range of each coordinate of a partial
//! derivative over a parameter box, from `geom-curve`'s closed-form extents in one parameter.
//!
//! What it does: a coordinate of ∂^{i+j}S/∂u^i∂v^j of a kind written in `crate::terms`' form is
//! Σ M_{fg}·f(u)·g(v) over f and g each 1, t, cos πt or sin πt: a grid of coefficients, each the
//! enclosure of an exact real. [`terms`] reads the grid's shape and takes the closed form that fits
//! it:
//!
//! | Shape of the grid | Kinds | Extent |
//! | --- | --- | --- |
//! | No cos πv or sin πv: affine in v | plane, cylinder, cone, and their affine images | the extent in u (`geom_curve::extent`) at each end of the v-span, hulled: for each u the extremes in v are at its ends |
//! | No cos πu or sin πu: affine in u | (the same with u and v exchanged) | the same with u and v exchanged |
//! | One function g* of v times the whole of the u-part: g*(v)·G(u) + H(v) | sphere and its affine images | G's extent [G₋, G₊] in u; then, S being affine in G, the extent in v of G₋·g*(v) + H(v) and of G₊·g*(v) + H(v), hulled |
//! | (the same with u and v exchanged) | | the same with u and v exchanged |
//! | anything else | an affine image of a torus that is not a similarity | the v-span cut in pieces of at most a thirty-second of a half-turn (at most 64); on each, the second-order form in v about its middle m: S(u, m) ± r·`S_v`(u, m), two functions of u with closed-form extents, widened by ½r² times a bound on \|`S_vv`\| over the piece; hulled |
//!
//! A torus's own grid is not of one function of v times the u-part when read from its enclosed
//! terms, but its exact surface is: S = c + P(v)·G(u) + Q(v), with G(u) = cos πu·x̂ + sin πu·ŷ,
//! P(v) = R + r·cos πv and Q(v) = s·r·sin πv·n̂, for the same exact unit vectors x̂ and ŷ in every
//! term. [`factored`] takes that form from the kind's own data: G's extent [G₋, G₊] in u, then the
//! extent in v of G₋·P(v) + Q(v) and G₊·P(v) + Q(v), hulled.
//!
//! Guarantees:
//!
//! - **Certified.** Every route holds every value of the coordinate over the box: the extents in
//!   one parameter are certified (`geom_curve::extent`), and each step's reduction is exact over
//!   the reals: a function affine in a parameter takes its extremes over a span at the span's
//!   ends; a function affine in G takes them over [G₋, G₊] at its ends, and [G₋, G₊] holds G's
//!   values; on a piece, Taylor's theorem with the remainder ½δ²·`S_vv`(u, ξ) for some ξ in it.
//! - **The kinds' own extents.** Every route but the last gives the coordinate's extent over the
//!   box up to rounding; the last within ½r² times the second v-derivative's size, about 10⁻³ of
//!   it on pieces of a thirty-second of a half-turn.
//! - **Deterministic.** The same terms and box give the same bits on every target.
//!
//! Method: separation of the parameters by linearity (a bilinear or affine function's extremes on
//! a box lie on its edges), then `geom-curve`'s closed-form extents in one parameter
//! (`geom_curve::extent`), all in interval arithmetic (R. E. Moore, R. B. Kearfott and M. J. Cloud,
//! *Introduction to Interval Analysis*, SIAM (2009), <https://doi.org/10.1137/1.9780898717716>).
//!
//! Cost: at most three extents in one parameter per coordinate, or one per piece (at most 64) on
//! the last route.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': f and g the functions, i and j the orders, u and v the parameters, c the centre, s the axial sign"
)]

use geom_curve::CurveError;
use geom_curve::extent::{Scalar, extent, is_zero};
use geom_curve::tight::{self, point};
use geom_num::interval::Interval;
use geom_run::WorkMeter;

use alloc::vec::Vec;

use crate::param::ParamBox;
use crate::terms::{Fun, Terms};
use geom_num::math;

/// π rounded down.
const PI_LO: f64 = f64::from_bits(0x4009_21FB_5444_2D18);
/// π rounded up.
const PI_HI: f64 = f64::from_bits(0x4009_21FB_5444_2D19);

/// The coefficients of one coordinate on f(u)·g(v): row f, column g, each in the order 1, t,
/// cos πt, sin πt.
type Grid = [[Interval; 4]; 4];

/// A function a + b·t + c·cos πt + d·sin πt of one parameter, by its four coefficients.
type Wave = [Interval; 4];

/// A function's place in a grid's rows or columns.
fn index(f: Fun) -> usize {
    match f {
        Fun::One => 0,
        Fun::Lin => 1,
        Fun::Cos => 2,
        Fun::Sin => 3,
    }
}

/// π^n enclosed.
fn pi_power(n: usize) -> Result<Interval, CurveError> {
    let pi = Interval::new(PI_LO, PI_HI)?;
    let mut p = point(1.0)?;
    for _ in 0..n {
        p = tight::mul(p, pi)?;
    }
    Ok(p)
}

/// The n-th derivative of f: a sign, π's power and the function, or `None` for zero.
fn derive(f: Fun, n: usize) -> Option<(f64, usize, Fun)> {
    match (f, n) {
        (Fun::One | Fun::Lin, 0) => Some((1.0, 0, f)),
        (Fun::Lin, 1) => Some((1.0, 0, Fun::One)),
        (Fun::One | Fun::Lin, _) => None,
        // cos πt shifts a quarter turn per derivative: cos, −sin, −cos, sin.
        (Fun::Cos, _) => Some(match n & 3 {
            0 => (1.0, n, Fun::Cos),
            1 => (-1.0, n, Fun::Sin),
            2 => (-1.0, n, Fun::Cos),
            _ => (1.0, n, Fun::Sin),
        }),
        // sin, cos, −sin, −cos.
        (Fun::Sin, _) => Some(match n & 3 {
            0 => (1.0, n, Fun::Sin),
            1 => (1.0, n, Fun::Cos),
            2 => (-1.0, n, Fun::Sin),
            _ => (-1.0, n, Fun::Cos),
        }),
    }
}

/// The cell of row f, column g.
fn cell(grid: &mut Grid, f: usize, g: usize) -> Result<&mut Interval, CurveError> {
    grid.get_mut(f)
        .and_then(|row| row.get_mut(g))
        .ok_or(CurveError::Defect)
}

/// The grids of ∂^{i+j}S/∂u^i∂v^j's three coordinates.
fn grids(t: &Terms, (i, j): (usize, usize)) -> Result<[Grid; 3], CurveError> {
    let zero = point(0.0)?;
    let mut out = [[[zero; 4]; 4]; 3];
    if i == 0 && j == 0 {
        for (g, &c) in out.iter_mut().zip(&t.ce) {
            *cell(g, 0, 0)? = c;
        }
    }
    for term in &t.terms {
        let (Some((su, pu, fu)), Some((sv, pv, fv))) = (derive(term.u, i), derive(term.v, j))
        else {
            continue;
        };
        let factor = tight::mul(point(su * sv)?, pi_power(pu.saturating_add(pv))?)?;
        for (g, &w) in out.iter_mut().zip(&term.we) {
            // A coordinate the term does not move stays exactly zero, so the grid's shape shows.
            if is_zero(w) {
                continue;
            }
            let slot = cell(g, index(fu), index(fv))?;
            *slot = tight::add(*slot, tight::mul(factor, w)?)?;
        }
    }
    Ok(out)
}

/// The grid with rows and columns exchanged: the same function with u and v exchanged.
fn transposed(grid: &Grid) -> Result<Grid, CurveError> {
    let mut out = *grid;
    for (f, row) in grid.iter().enumerate() {
        for (g, &x) in row.iter().enumerate() {
            *cell(&mut out, g, f)? = x;
        }
    }
    Ok(out)
}

/// The scalar a + b·t + c·cos πt + d·sin πt.
fn wave([a, b, c, d]: Wave) -> Scalar {
    Scalar::Wave {
        p: a,
        q: b,
        a: c,
        b: d,
    }
}

/// x·w + y, coefficient by coefficient.
fn affine(x: Interval, w: &Wave, y: &Wave) -> Result<Wave, CurveError> {
    let mut out = *y;
    for (o, &c) in out.iter_mut().zip(w) {
        *o = tight::add(*o, tight::mul(x, c)?)?;
    }
    Ok(out)
}

/// The extent of g·P(v) + Q(v) over the v-span for every g of `gx` (an enclosure of G's range),
/// for a function affine in g: the hull at g's two ends.
fn affine_in(
    gx: Interval,
    p: &Wave,
    q: &Wave,
    span: (f64, f64),
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, CurveError> {
    let lo = extent(&wave(affine(point(gx.lo())?, p, q)?), span, meter)?;
    let hi = extent(&wave(affine(point(gx.hi())?, p, q)?), span, meter)?;
    Ok(Interval::new(
        tight::smaller(lo.lo(), hi.lo()),
        tight::larger(lo.hi(), hi.hi()),
    )?)
}

/// The value of each of the four functions over `t`.
fn basis(t: Interval) -> Result<Wave, CurveError> {
    Ok([point(1.0)?, t, t.cospi()?, t.sinpi()?])
}

/// The sum along each row of its coefficients times `values`: the coefficients in u at fixed
/// values of v's functions.
fn rows(grid: &Grid, values: &Wave) -> Result<Wave, CurveError> {
    let mut out = [point(0.0)?; 4];
    for (o, row) in out.iter_mut().zip(grid) {
        for (&m, &x) in row.iter().zip(values) {
            if !is_zero(m) {
                *o = tight::add(*o, tight::mul(m, x)?)?;
            }
        }
    }
    Ok(out)
}

/// The extent of one coordinate's grid over the box, u along the rows (module documentation).
fn grid_extent(
    grid: &Grid,
    ((u0, u1), (v0, v1)): ParamBox,
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, CurveError> {
    let zero_column = |g: usize| {
        grid.iter()
            .all(|row| row.get(g).is_none_or(|&x| is_zero(x)))
    };
    // Affine in v: the extent in u at each end of the v-span.
    if zero_column(2) && zero_column(3) {
        let mut out: Option<Interval> = None;
        for v in [v0, v1] {
            let e = extent(&wave(rows(grid, &basis(point(v)?)?)?), (u0, u1), meter)?;
            out = Some(join(out, e)?);
        }
        return out.ok_or(CurveError::Defect);
    }
    // One function g* of v times the whole u-part: g*(v)·G(u) + H(v).
    let [one, lin, cos, sin] = grid;
    let mut star: Option<usize> = None;
    let mut single = true;
    for g in 0..4 {
        let used = [lin, cos, sin]
            .iter()
            .any(|row| row.get(g).is_some_and(|&x| !is_zero(x)));
        if used {
            if star.is_some() {
                single = false;
            }
            star = Some(g);
        }
    }
    let Some(g) = star else {
        // No u at all: H(v) alone.
        return extent(&wave(*one), (v0, v1), meter);
    };
    if single {
        let pick = |row: &[Interval; 4]| row.get(g).copied().ok_or(CurveError::Defect);
        let zero = point(0.0)?;
        let gu = [zero, pick(lin)?, pick(cos)?, pick(sin)?];
        let gx = extent(&wave(gu), (u0, u1), meter)?;
        let mut p = [zero; 4];
        *p.get_mut(g).ok_or(CurveError::Defect)? = point(1.0)?;
        return affine_in(gx, &p, one, (v0, v1), meter);
    }
    Err(CurveError::Defect)
}

/// The last route (module documentation): `grid` neither affine in v nor one function of v times
/// its u-part, `second` the grid of its second derivative in v. On each piece of the v-span, with
/// middle m and half-width r, for each u:
/// S(u, m + δ) = S(u, m) + δ·`S_v`(u, m) + ½δ²·`S_vv`(u, ξ). The first two terms are affine in δ,
/// so they lie between their values at δ = ±r, two functions of u whose extents are closed forms;
/// the last lies within ½r² of the bound on |`S_vv`| over the piece.
fn second_order(
    grid: &Grid,
    second: &Grid,
    ((u0, u1), (v0, v1)): ParamBox,
    meter: &mut WorkMeter<'_>,
) -> Result<Interval, CurveError> {
    let pi = Interval::new(PI_LO, PI_HI)?;
    let mut out: Option<Interval> = None;
    for (a, b) in pieces((v0, v1))? {
        let m = f64::midpoint(a, b);
        let r = tight::larger(
            tight::sub(point(m)?, point(a)?)?.hi(),
            tight::sub(point(b)?, point(m)?)?.hi(),
        );
        let pm = point(m)?;
        let value = rows(grid, &basis(pm)?)?;
        // The v-derivatives of 1, v, cos πv and sin πv at m.
        let slope = rows(
            grid,
            &[
                point(0.0)?,
                point(1.0)?,
                tight::mul(pi, pm.sinpi()?)?.neg(),
                tight::mul(pi, pm.cospi()?)?,
            ],
        )?;
        let plus = extent(&wave(affine(point(r)?, &slope, &value)?), (u0, u1), meter)?;
        let minus = extent(&wave(affine(point(-r)?, &slope, &value)?), (u0, u1), meter)?;
        let curvature =
            wave(rows(second, &basis(Interval::new(a, b)?)?)?).over(Interval::new(u0, u1)?)?;
        let reach = tight::mul(point(0.5 * r * r)?, point(tight::magnitude(curvature))?)?.hi();
        let e = Interval::new(
            tight::smaller(plus.lo(), minus.lo()),
            tight::larger(plus.hi(), minus.hi()),
        )?;
        out = Some(join(out, tight::add(e, Interval::new(-reach, reach)?)?)?);
    }
    out.ok_or(CurveError::Defect)
}

/// The span cut into pieces of at most 1/32 of a half-turn, at most 64 of them, joining exactly.
fn pieces((lo, hi): (f64, f64)) -> Result<Vec<(f64, f64)>, CurveError> {
    let n = math::ceil((hi - lo) * 32.0)?.get();
    let count: u32 = if n >= 64.0 {
        64
    } else if n >= 1.0 {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "n is a whole number in [1, 64)"
        )]
        let m = n as u32;
        m
    } else {
        1
    };
    let (width, total) = (hi - lo, f64::from(count));
    let mut out = Vec::with_capacity(64);
    let mut start = lo;
    for i in 1..=count {
        let end = if i == count {
            hi
        } else {
            lo + width * (f64::from(i) / total)
        };
        // Rounding may step past either end; the piece stays in [start, hi].
        let end = tight::larger(tight::smaller(end, hi), start);
        out.push((start, end));
        start = end;
    }
    Ok(out)
}

/// The hull of an optional enclosure and another.
fn join(a: Option<Interval>, b: Interval) -> Result<Interval, CurveError> {
    Ok(match a {
        None => b,
        Some(a) => Interval::new(
            tight::smaller(a.lo(), b.lo()),
            tight::larger(a.hi(), b.hi()),
        )?,
    })
}

/// Whether a grid is affine in v (no cos πv or sin πv) or has one function of v times its u-part:
/// the shapes read with u along the rows.
fn reads_by_rows(grid: &Grid) -> bool {
    let column_used = |g: usize| {
        grid.iter()
            .any(|row| row.get(g).is_some_and(|&x| !is_zero(x)))
    };
    let affine_in_v = !column_used(2) && !column_used(3);
    let [_, lin, cos, sin] = grid;
    let columns = (0..4)
        .filter(|&g| {
            [lin, cos, sin]
                .iter()
                .any(|row| row.get(g).is_some_and(|&x| !is_zero(x)))
        })
        .count();
    affine_in_v || columns <= 1
}

/// The extent of ∂^{i+j}S/∂u^i∂v^j over the region, coordinate by coordinate, from the kind's
/// terms (module documentation); over a region of one point, the terms' enclosure there.
///
/// # Errors
///
/// [`CurveError::NotFinite`] if a value overflows; [`CurveError::Stopped`] by the meter.
pub(crate) fn terms(
    t: &Terms,
    ((u0, u1), (v0, v1)): ParamBox,
    order: (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], CurveError> {
    if u0 == u1 && v0 == v1 {
        // One point: the terms' own enclosure there.
        return t.enclose(point(u0)?, point(v0)?, order);
    }
    let mut out = [point(0.0)?; 3];
    let (i, j) = order;
    let mut second: Option<[Grid; 3]> = None;
    for (k, (o, grid)) in out.iter_mut().zip(&grids(t, order)?).enumerate() {
        let swapped = transposed(grid)?;
        *o = if reads_by_rows(grid) {
            grid_extent(grid, ((u0, u1), (v0, v1)), meter)?
        } else if reads_by_rows(&swapped) {
            grid_extent(&swapped, ((v0, v1), (u0, u1)), meter)?
        } else {
            if second.is_none() {
                second = Some(grids(t, (i, j.checked_add(2).ok_or(CurveError::Defect)?))?);
            }
            let vv = second
                .as_ref()
                .and_then(|g| g.get(k))
                .ok_or(CurveError::Defect)?;
            second_order(grid, vv, ((u0, u1), (v0, v1)), meter)?
        };
    }
    Ok(out)
}

/// The derivative of a + b·t + c·cos πt + d·sin πt, n times.
fn derived(w: Wave, n: usize) -> Result<Wave, CurveError> {
    let pi = Interval::new(PI_LO, PI_HI)?;
    let zero = point(0.0)?;
    let mut out = w;
    // π·x, exactly zero when x is.
    let times_pi = |x: Interval| {
        if is_zero(x) {
            Ok(zero)
        } else {
            tight::mul(pi, x)
        }
    };
    for _ in 0..n {
        let [_, b, c, d] = out;
        out = [b, zero, times_pi(d)?, times_pi(c)?.neg()];
    }
    Ok(out)
}

/// A surface c + P(v)·G(u) + Q(v), G(u) = a·cos πu + b·sin πu, coordinate by coordinate: the
/// factored form of a torus (module documentation).
pub(crate) struct Factored {
    /// c, enclosed.
    pub(crate) center: [Interval; 3],
    /// G's coefficients (a, b) in each coordinate: the exact x̂ and ŷ, enclosed.
    pub(crate) g: [(Interval, Interval); 3],
    /// P, the same in every coordinate.
    pub(crate) p: Wave,
    /// Q in each coordinate.
    pub(crate) q: [Wave; 3],
}

/// The extent of ∂^{i+j}S/∂u^i∂v^j over the region for a factored surface (module documentation).
///
/// # Errors
///
/// As [`terms`].
pub(crate) fn factored(
    f: &Factored,
    (us, vs): ParamBox,
    (i, j): (usize, usize),
    meter: &mut WorkMeter<'_>,
) -> Result<[Interval; 3], CurveError> {
    let zero = point(0.0)?;
    let p = derived(f.p, j)?;
    let mut out = [zero; 3];
    for ((o, &(a, b)), (&c, &q)) in out.iter_mut().zip(&f.g).zip(f.center.iter().zip(&f.q)) {
        // G⁽ⁱ⁾: a quarter turn and a factor π per derivative.
        let [_, _, ga, gb] = derived([zero, zero, a, b], i)?;
        let gx = extent(&wave([zero, zero, ga, gb]), us, meter)?;
        let mut rest = if i == 0 { derived(q, j)? } else { [zero; 4] };
        if i == 0 && j == 0 {
            let [r0, r1, r2, r3] = rest;
            rest = [tight::add(r0, c)?, r1, r2, r3];
        }
        *o = affine_in(gx, &p, &rest, vs, meter)?;
    }
    Ok(out)
}
