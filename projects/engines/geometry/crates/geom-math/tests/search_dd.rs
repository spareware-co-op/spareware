#![allow(clippy::all, clippy::pedantic, warnings)]
mod common;

use common::{exact, int, is_integer, round_half_even, to_i128};
use geom_math::{DD, Transform};
use geom_num::grid::P3;
use num_rational::BigRational;

struct G(u64);
impl G {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo) as u64)) as i64
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}
fn pow2(k: i32) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

fn float_decides(row: [f64; 4], v: [i64; 3]) -> bool {
    let [l0, l1, l2, t] = row;
    let [x, y, z] = v.map(|c| c as f64);
    let (a, b, c) = (l0 * x, l1 * y, l2 * z);
    if !a.is_finite() || !b.is_finite() || !c.is_finite() { return false; }
    let s = a + b + c + t;
    let sigma = a.abs() + b.abs() + c.abs() + t.abs();
    if !s.is_finite() || !sigma.is_finite() { return false; }
    let err = sigma * pow2(-50) + pow2(-1022) * pow2(-48);
    if s >= pow2(41) || s <= -pow2(41) { return false; }
    let r = s.round_ties_even();
    let frac = (s - r).abs();
    frac > err && frac < 0.5 - err
}

fn dd_filter(row: [f64; 4], v: [i64; 3], var: u8) -> Option<i128> {
    let [l0, l1, l2, t] = row;
    let [x, y, z] = v.map(|c| c as f64);
    let p0 = DD::exact_product(l0, x)?;
    let p1 = DD::exact_product(l1, y)?;
    let p2 = DD::exact_product(l2, z)?;
    let s = p0.add(p1).ok()?.add(p2).ok()?.add_f64(t).ok()?;
    let (a, b, c, d) = (p0.hi().abs(), p1.hi().abs(), p2.hi().abs(), t.abs());
    let sigma = match var {
        5 => a - b + c + d,
        6 => a + b - c + d,
        7 => a + b + c - d,
        8 => a * b + c + d,
        9 => (a + b) * c + d,
        10 => (a + b + c) * d,
        _ => a + b + c + d,
    };
    let err = if var == 11 { sigma * pow2(-98) * pow2(-52) } else { sigma * pow2(-98) + pow2(-52) };
    if s.hi() >= pow2(41) || s.hi() <= -pow2(41) { return None; }
    let cand = s.round_shifted(0)?.value();
    let frac = ((s.hi() - cand as f64) + s.lo()).abs();
    let ok = match var {
        12 => frac > err && frac > 0.5 - err,
        13 => frac > err && frac < 0.5 + err,
        14 => frac > err && frac < 0.5 / err,
        _ => frac > err && frac < 0.5 - err,
    };
    ok.then_some(cand)
}

#[test]
fn search() {
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let n: usize = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(200000);
    let mut g = G(seed);
    let mut found = [0usize; 15];
    let mut tried = [0usize; 5];
    for i in 0..n {
        let big = |g: &mut G| pow2(g.range(14, 22) as i32) * (1.0 + g.unit());
        let xs = |g: &mut G| g.range(1 << 38, 1 << 40) * if g.next() & 1 == 0 { 1 } else { -1 };
        let kind = i % 5;
        let (mut row, v);
        match kind {
            0 => {
                // S1: p0 huge, p1 tiny (lost in p0 + p1), p2 = -p0, t = -p1: V = 0 exactly.
                let l0 = big(&mut g);
                let x = xs(&mut g);
                let l1 = pow2(g.range(-60, -40) as i32) * (1.0 + g.unit());
                row = [l0, l1, -l0, -l1];
                v = [x, 1, x];
            }
            3 => {
                // S4: p0 = 2^k*x exact, p1 tiny lost in p0 + p1, p2 = -p1, t = -p0: V = 0.
                let x = xs(&mut g) | 1;
                let l1 = pow2(g.range(-40, -33) as i32) * (1.0 + g.unit());
                row = [pow2(g.range(18, 23) as i32), l1, -l1, 0.0];
                row[3] = -(row[0] * x as f64);
                let y = xs(&mut g) | 1;
                v = [x, y, y];
            }
            4 => {
                // S5: p0 a full product of moderate size, p1 = B big, p2 = -B, t near -h0.
                let l0 = pow2(g.range(-30, 0) as i32) * (1.0 + g.unit());
                let x = xs(&mut g);
                let b = pow2(g.range(18, 23) as i32);
                let y = xs(&mut g) | 1;
                row = [l0, b, -b, 0.0];
                v = [x, y, y];
            }
            1 => {
                let (l0, l2) = (big(&mut g), big(&mut g));
                let (x, z) = (xs(&mut g).abs(), xs(&mut g).abs());
                let y = -xs(&mut g).abs();
                let l1 = (l0 * x as f64 + l2 * z as f64) / -(y as f64) * -1.0;
                row = [l0, -l1, l2, 0.0];
                v = [x, y, z];
            }
            _ => {
                let (l0, l1, l2) = (big(&mut g), big(&mut g), big(&mut g));
                let (x, y, z) = (xs(&mut g).abs(), xs(&mut g).abs(), xs(&mut g).abs());
                row = [l0, l1, l2, 0.0];
                v = [x, y, z];
            }
        }
        let partial: BigRational = exact(row[0]) * int(v[0].into()) + exact(row[1]) * int(v[1].into())
            + exact(row[2]) * int(v[2].into());
        if kind == 1 || kind == 2 || kind == 4 {
            let whole = round_half_even(&partial);
            let half = if g.next() & 1 == 0 { int(0) } else { BigRational::new(1.into(), 2.into()) };
            let target = int(to_i128(&whole)) + half;
            let want_t = target - &partial;
            let t = num_traits::ToPrimitive::to_f64(&want_t).unwrap();
            if exact(t) != want_t { continue; }
            row[3] = t;
        }
        let val = &partial + exact(row[3]);
        let want = to_i128(&round_half_even(&val));
        if want.unsigned_abs() >= 1 << 40 { continue; }
        let want_exact = is_integer(&val);
        let Ok(tr) = Transform::from_rows([row, [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]) else { continue };
        let Ok(p) = P3::new(v[0], v[1], v[2]) else { continue };
        let Ok(got) = tr.apply_grid(p) else { continue };
        tried[kind] += 1;
        assert_eq!(i128::from(got.value().x.get()), want, "kernel");
        assert_eq!(got.is_exact(), want_exact, "kernel exact");
        if float_decides(row, v) { continue; }
        if let Some(r) = dd_filter(row, v, 0) { assert!(r == want && !want_exact, "dd accepted wrongly"); continue; }
        for var in 5..=14u8 {
            if let Some(r) = dd_filter(row, v, var) {
                if r != want || want_exact {
                    found[var as usize] += 1;
                    if found[var as usize] <= 2 {
                        println!("DD{var} S{kind} row {:?} v {v:?} want {want} exact {want_exact} got {r}", row.map(f64::to_bits));
                    }
                }
            }
        }
    }
    println!("found {found:?} tried {tried:?}");
}
