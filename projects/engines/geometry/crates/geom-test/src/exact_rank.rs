//! The exact-rank spike (piece G231; risk "Exact rank by Bareiss elimination", chapter 31, and
//! chapter 15.8): the rank of a sketch's Jacobian in exact rationals at a witness point, by Bareiss'
//! fraction-free elimination on big integers, held against the kernel's pivoted-QR diagnosis.
//!
//! What it does: [`jacobian`] writes the Jacobian of a sketch job's rules, in the polynomial form of
//! each (a distance by its square, a point's distance from a line by the square of the cross
//! product), at the job's starting points read as the exact rationals they are, with each row scaled
//! to whole numbers. [`bareiss_rank`] eliminates it without a division that leaves a remainder and
//! says the rank, which columns carry it, and what it cost. [`gauss_rank`] does the same with plain
//! rational elimination, the independent reference the tests hold [`bareiss_rank`] to.
//!
//! Guarantees:
//!
//! - **Exact.** Every entry is a whole number and every division exact (checked: a remainder is an
//!   error, never a rounded rank).
//! - **A witness, not a proof of all points.** The rank is that of the Jacobian at the point given.
//!   A rule's polynomial form has the gradient of the rule up to a nonzero factor wherever the rule
//!   holds, so at a configuration that meets the rules the ranks are the same; elsewhere they can
//!   differ, which is why the point is a seeded generic one.
//! - **What it cannot state.** An angle rule has no polynomial form over the rationals (its sine and
//!   cosine are not rational), so [`jacobian`] refuses a job that has one.
//!
//! Method: E. H. Bareiss, "Sylvester's identity and multistep integer-preserving Gaussian
//! elimination", Mathematics of Computation 22 (1968), <https://doi.org/10.2307/2004533>; the
//! sketch rank question is chapter 15.8's
//! (`projects/engines/geometry/architecture/15-sketches.md`). The recommendation the spike
//! reaches is in `projects/kit/notes/G231-exact-rank-spike.md`. Big integers are `dashu-int`
//! (MIT OR Apache-2.0), the version the NURBS reference pins; nothing here is for the kernel's
//! own code, which the note explains.
//!
//! Cost: Bareiss on an m by n matrix is O(m·n·rank) big-integer multiplications whose operands grow
//! to about rank times the entries' bit length; the note has the measured table.

use core::fmt::Write;
use std::time::Instant;

use dashu_int::IBig;
use dashu_int::ops::BitTest;
use dashu_ratio::RBig;

use crate::oracle::exact::read_float;
use crate::oracle::sketches::{Job, Rule};

/// What an elimination found and cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rank {
    /// The rank.
    pub rank: usize,
    /// The columns the pivots were found in, in order.
    pub pivots: Vec<usize>,
    /// The longest entry met, in bits.
    pub bits: u64,
    /// The big-integer multiplications made.
    pub products: u64,
}

fn to_rational(text: &str) -> Result<RBig, String> {
    let x = read_float(text).ok_or_else(|| format!("bad number {text:?}"))?;
    RBig::try_from(x).map_err(|_| format!("{text:?} is not a finite number"))
}

/// The coordinates of a job's points as exact rationals.
fn coordinates(job: &Job) -> Result<Vec<[RBig; 2]>, String> {
    job.points
        .iter()
        .map(|[x, y]| Ok([to_rational(x)?, to_rational(y)?]))
        .collect()
}

/// A row being built: one rational per variable (x of point 0, y of point 0, x of point 1, ...).
struct Row(Vec<RBig>);

impl Row {
    fn new(columns: usize) -> Row {
        Row(vec![RBig::ZERO; columns])
    }

    fn add(&mut self, point: usize, axis: usize, value: &RBig) {
        if let Some(slot) = self.0.get_mut(point * 2 + axis) {
            *slot += value;
        }
    }
}

fn sub(a: &[RBig; 2], b: &[RBig; 2]) -> [RBig; 2] {
    [&a[0] - &b[0], &a[1] - &b[1]]
}

fn times(k: i32, x: &RBig) -> RBig {
    RBig::from(k) * x
}

/// Adds `by` times `grad` (a gradient on the two coordinates of a difference `to - from`) to a row.
fn difference(row: &mut Row, from: usize, to: usize, grad: &[RBig; 2], by: i32) {
    for axis in 0..2 {
        let g = times(by, &grad[axis]);
        row.add(to, axis, &g);
        row.add(from, axis, &-g);
    }
}

/// The Jacobian of `job`'s rules at its starting points, a row for each equation, scaled to whole
/// numbers.
///
/// # Errors
///
/// A job with an angle rule, or one that names a point it does not have or holds a number that is
/// not finite.
pub fn jacobian(job: &Job) -> Result<Vec<Vec<IBig>>, String> {
    let p = coordinates(job)?;
    let n = p.len();
    let columns = n * 2;
    let at = |i: usize| p.get(i).ok_or_else(|| format!("no point {i}"));
    let mut rows: Vec<Row> = Vec::new();
    for rule in &job.rules {
        match rule {
            Rule::Fix { point } => {
                at(*point)?;
                for axis in 0..2 {
                    let mut r = Row::new(columns);
                    r.add(*point, axis, &RBig::ONE);
                    rows.push(r);
                }
            }
            Rule::Distance { a, b, .. } => {
                let u = sub(at(*b)?, at(*a)?);
                let mut r = Row::new(columns);
                difference(&mut r, *a, *b, &u, 2);
                rows.push(r);
            }
            Rule::Level { a, b } | Rule::Plumb { a, b } => {
                at(*a)?;
                at(*b)?;
                let axis = usize::from(matches!(rule, Rule::Level { .. }));
                let mut r = Row::new(columns);
                r.add(*b, axis, &RBig::ONE);
                r.add(*a, axis, &-RBig::ONE);
                rows.push(r);
            }
            Rule::Parallel { lines: l } | Rule::Perpendicular { lines: l } => {
                let (u, v) = (sub(at(l[1])?, at(l[0])?), sub(at(l[3])?, at(l[2])?));
                // cross(u, v) has the gradient (vy, -vx) on u and (-uy, ux) on v; dot(u, v) has
                // (vx, vy) on u and (ux, uy) on v.
                let (gu, gv) = if matches!(rule, Rule::Parallel { .. }) {
                    ([v[1].clone(), -&v[0]], [-&u[1], u[0].clone()])
                } else {
                    ([v[0].clone(), v[1].clone()], [u[0].clone(), u[1].clone()])
                };
                let mut r = Row::new(columns);
                difference(&mut r, l[0], l[1], &gu, 1);
                difference(&mut r, l[2], l[3], &gv, 1);
                rows.push(r);
            }
            Rule::EqualLength { lines: l } => {
                let (u, v) = (sub(at(l[1])?, at(l[0])?), sub(at(l[3])?, at(l[2])?));
                let mut r = Row::new(columns);
                difference(&mut r, l[0], l[1], &u, 2);
                difference(&mut r, l[2], l[3], &v, -2);
                rows.push(r);
            }
            Rule::Midpoint { point, a, b } => {
                for axis in 0..2 {
                    let mut r = Row::new(columns);
                    r.add(*point, axis, &RBig::from(2));
                    r.add(*a, axis, &-RBig::ONE);
                    r.add(*b, axis, &-RBig::ONE);
                    rows.push(r);
                }
            }
            Rule::Mirror { a, b, line } => {
                let (pa, pb) = (at(*a)?, at(*b)?);
                let (o, q) = (at(line[0])?, at(line[1])?);
                let w = sub(q, o);
                // dot(b - a, w) = 0
                let ba = sub(pb, pa);
                let mut first = Row::new(columns);
                difference(&mut first, *a, *b, &w, 1);
                difference(&mut first, line[0], line[1], &ba, 1);
                rows.push(first);
                // cross(w, a + b - 2 o) = 0, with s = a + b - 2 o
                let s = [
                    &pa[0] + &pb[0] - times(2, &o[0]),
                    &pa[1] + &pb[1] - times(2, &o[1]),
                ];
                let (gw, gs) = ([s[1].clone(), -&s[0]], [-&w[1], w[0].clone()]);
                let mut second = Row::new(columns);
                difference(&mut second, line[0], line[1], &gw, 1);
                for axis in 0..2 {
                    second.add(*a, axis, &gs[axis]);
                    second.add(*b, axis, &gs[axis]);
                    second.add(line[0], axis, &times(-2, &gs[axis]));
                }
                rows.push(second);
            }
            Rule::FromLine {
                point,
                line,
                signed,
            } => {
                let (k, a, b) = (at(*point)?, at(line[0])?, at(line[1])?);
                let (u, w) = (sub(b, a), sub(k, a));
                let c = &u[0] * &w[1] - &u[1] * &w[0];
                let d = to_rational(signed)?;
                let d2 = &d * &d;
                // g = c^2 - d^2 |u|^2: 2c dc - d^2 d|u|^2, dc = (wy, -wx | -uy, ux), d|u|^2 = 2u.
                let gu = [
                    times(2, &(&c * &w[1])) - times(2, &(&d2 * &u[0])),
                    -times(2, &(&c * &w[0])) - times(2, &(&d2 * &u[1])),
                ];
                let gw = [-times(2, &(&c * &u[1])), times(2, &(&c * &u[0]))];
                let mut r = Row::new(columns);
                difference(&mut r, line[0], line[1], &gu, 1);
                difference(&mut r, line[0], *point, &gw, 1);
                rows.push(r);
            }
            Rule::Angle { .. } => {
                return Err("an angle rule has no polynomial form over the rationals".to_owned());
            }
        }
    }
    Ok(rows.into_iter().map(|r| whole(&r.0)).collect())
}

/// `row` times the least common multiple of its denominators.
fn whole(row: &[RBig]) -> Vec<IBig> {
    let mut lcm = IBig::ONE;
    for q in row {
        let d = IBig::from(q.denominator().clone());
        let g = gcd(&lcm, &d);
        lcm = &lcm / &g * &d;
    }
    row.iter()
        .map(|q| {
            let scaled = q * RBig::from(lcm.clone());
            IBig::from(scaled.numerator().clone()) / IBig::from(scaled.denominator().clone())
        })
        .collect()
}

fn gcd(a: &IBig, b: &IBig) -> IBig {
    let (mut a, mut b) = (a.clone(), b.clone());
    while b != IBig::ZERO {
        let r = &a % &b;
        a = b;
        b = r;
    }
    a
}

fn bits(x: &IBig) -> u64 {
    u64::try_from(x.bit_len()).unwrap_or(u64::MAX)
}

/// The rank of `matrix` by Bareiss' fraction-free elimination.
///
/// # Errors
///
/// Rows of unequal length, or a division that leaves a remainder (which the method rules out: a
/// defect, never a rounded answer).
pub fn bareiss_rank(matrix: &[Vec<IBig>]) -> Result<Rank, String> {
    let columns = matrix.first().map_or(0, Vec::len);
    if matrix.iter().any(|r| r.len() != columns) {
        return Err("rows of unequal length".to_owned());
    }
    let mut m: Vec<Vec<IBig>> = matrix.to_vec();
    let rows = m.len();
    let (mut r, mut previous) = (0_usize, IBig::ONE);
    let (mut longest, mut products) = (0_u64, 0_u64);
    let mut pivots = Vec::new();
    for c in 0..columns {
        let Some(found) = (r..rows).find(|&i| m.get(i).is_some_and(|row| row[c] != IBig::ZERO))
        else {
            continue;
        };
        m.swap(r, found);
        let pivot = m[r][c].clone();
        for i in r + 1..rows {
            let lead = m[i][c].clone();
            for j in c + 1..columns {
                let top = &pivot * &m[i][j] - &lead * &m[r][j];
                products += 2;
                let (q, rem) = (&top / &previous, &top % &previous);
                if rem != IBig::ZERO {
                    return Err("a Bareiss division left a remainder".to_owned());
                }
                longest = longest.max(bits(&q));
                m[i][j] = q;
            }
            m[i][c] = IBig::ZERO;
        }
        previous = pivot;
        pivots.push(c);
        r += 1;
    }
    for row in matrix {
        for x in row {
            longest = longest.max(bits(x));
        }
    }
    Ok(Rank {
        rank: r,
        pivots,
        bits: longest,
        products,
    })
}

/// The rank of `matrix` by plain elimination over the rationals: the independent reference.
///
/// # Errors
///
/// Rows of unequal length.
pub fn gauss_rank(matrix: &[Vec<IBig>]) -> Result<usize, String> {
    let columns = matrix.first().map_or(0, Vec::len);
    if matrix.iter().any(|r| r.len() != columns) {
        return Err("rows of unequal length".to_owned());
    }
    let mut m: Vec<Vec<RBig>> = matrix
        .iter()
        .map(|r| r.iter().map(|x| RBig::from(x.clone())).collect())
        .collect();
    let rows = m.len();
    let mut r = 0_usize;
    for c in 0..columns {
        let Some(found) = (r..rows).find(|&i| m[i][c] != RBig::ZERO) else {
            continue;
        };
        m.swap(r, found);
        for i in r + 1..rows {
            if m[i][c] == RBig::ZERO {
                continue;
            }
            let factor = &m[i][c] / &m[r][c];
            for j in c..columns {
                let delta = &factor * &m[r][j];
                m[i][j] -= delta;
            }
        }
        r += 1;
    }
    Ok(r)
}

/// A chain of `points` points with `rows` equations: the Jacobian of a long chain of lengths and
/// plumb or level lines, for the cost table. Its entries are whole numbers of at most `bits` bits.
///
/// # Errors
///
/// Never, but the shape is the one [`jacobian`] returns.
pub fn chain_matrix(points: usize, seed: u64) -> Result<Vec<Vec<IBig>>, String> {
    let mut state = seed | 1;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        i64::try_from(state >> 40).unwrap_or(0) % 2001 - 1000
    };
    let columns = points * 2;
    let mut rows = vec![vec![IBig::ZERO; columns]; 2];
    rows[0][0] = IBig::ONE;
    rows[1][1] = IBig::ONE;
    for k in 1..points {
        let (dx, dy) = (next(), next());
        let mut level = vec![IBig::ZERO; columns];
        level[2 * k + 1] = IBig::ONE;
        level[2 * k - 1] = -IBig::ONE;
        rows.push(level);
        let mut length = vec![IBig::ZERO; columns];
        length[2 * k] = IBig::from(2 * dx + 2001);
        length[2 * k - 2] = IBig::from(-2 * dx - 2001);
        length[2 * k + 1] = IBig::from(2 * dy + 1);
        length[2 * k - 1] = IBig::from(-2 * dy - 1);
        rows.push(length);
    }
    Ok(rows)
}

/// A table of what Bareiss costs on chains of the given sizes: points, unknowns, rank, longest entry
/// in bits, multiplications, milliseconds.
///
/// # Errors
///
/// What [`bareiss_rank`] refuses.
pub fn cost_table(sizes: &[usize]) -> Result<String, String> {
    let mut out = String::from("points unknowns rank bits products milliseconds\n");
    for &points in sizes {
        let matrix = chain_matrix(points, 0x231)?;
        let start = Instant::now();
        let rank = bareiss_rank(&matrix)?;
        let ms = start.elapsed().as_millis();
        writeln!(
            out,
            "{points} {} {} {} {} {ms}",
            points * 2,
            rank.rank,
            rank.bits,
            rank.products
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(out)
}
