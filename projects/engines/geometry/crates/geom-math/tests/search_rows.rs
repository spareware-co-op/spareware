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

/// float_row's decision with a chosen sigma and err formula: Some(r) when it accepts.
fn float_filter(row: [f64; 4], v: [i64; 3], variant: u8) -> Option<i64> {
    let [l0, l1, l2, t] = row;
    let [x, y, z] = v.map(|c| c as f64);
    let (a, b, c) = (l0 * x, l1 * y, l2 * z);
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return None;
    }
    let s = a + b + c + t;
    let (sa, sb, sc, st) = (a.abs(), b.abs(), c.abs(), t.abs());
    let sigma = match variant {
        1 => sa * sb + sc + st,
        2 => (sa + sb) * sc + st,
        3 => (sa + sb + sc) * st,
        _ => sa + sb + sc + st,
    };
    if !s.is_finite() || !sigma.is_finite() {
        return None;
    }
    let err = if variant == 4 { sigma * pow2(-50) * (pow2(-1022) * pow2(-48)) } else { sigma * pow2(-50) + pow2(-1022) * pow2(-48) };
    if s >= pow2(41) || s <= -pow2(41) {
        return None;
    }
    let r = s.round_ties_even();
    let frac = (s - r).abs();
    if frac > err && frac < 0.5 - err {
        return Some(r as i64);
    }
    None
}

/// filtered_row's decision with variants: 0 original, 1 err = sigma·F·SLACK, 2 frac < 0.5 + err,
/// 3 frac > 0.5 − err (`<`→`>`), 4 frac < 0.5/err.
fn dd_filter(row: [f64; 4], v: [i64; 3], variant: u8) -> Option<i128> {
    let [l0, l1, l2, t] = row;
    let [x, y, z] = v.map(|c| c as f64);
    let p0 = DD::exact_product(l0, x)?;
    let p1 = DD::exact_product(l1, y)?;
    let p2 = DD::exact_product(l2, z)?;
    let s = p0.add(p1).ok()?.add(p2).ok()?.add_f64(t).ok()?;
    let sigma = p0.hi().abs() + p1.hi().abs() + p2.hi().abs() + t.abs();
    let err = if variant == 1 { sigma * pow2(-98) * pow2(-52) } else { sigma * pow2(-98) + pow2(-52) };
    if s.hi() >= pow2(41) || s.hi() <= -pow2(41) {
        return None;
    }
    let cand = s.round_shifted(0)?.value();
    let frac = ((s.hi() - cand as f64) + s.lo()).abs();
    let ok = match variant {
        2 => frac > err && frac < 0.5 + err,
        3 => frac > err && frac > 0.5 - err,
        4 => frac > err && frac < 0.5 / err,
        _ => frac > err && frac < 0.5 - err,
    };
    ok.then_some(cand)
}

#[test]
fn search() {
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let n: usize = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(200000);
    let mut g = G(seed);
    let mut found = [0usize; 10];
    for _ in 0..n {
        // Products of random magnitudes; one cancelling pair; t chosen to land the float value
        // near a whole step or a midpoint.
        let mag = |g: &mut G| pow2(g.range(-30, 30) as i32) * (1.0 + g.unit());
        let mut row = [mag(&mut g), mag(&mut g), mag(&mut g), 0.0];
        let mut v = [g.range(-(1 << 39), 1 << 39), g.range(-(1 << 39), 1 << 39), g.range(-(1 << 39), 1 << 39)];
        // Random zeros and small coordinates.
        for i in 0..3 {
            match g.next() % 6 {
                0 => v[i] = 0,
                1 => v[i] = g.range(-8, 9),
                2 => row[i] = 0.0,
                _ => {}
            }
            if g.next() & 1 == 0 { row[i] = -row[i]; }
        }
        // Make the first two products nearly cancel, sometimes.
        if g.next() % 2 == 0 && v[0] != 0 && v[1] != 0 {
            let want = -(row[0] * v[0] as f64);
            row[1] = want / v[1] as f64 * (1.0 + (g.unit() - 0.5) * pow2(-40));
        }
        let partial = row[0] * v[0] as f64 + row[1] * v[1] as f64 + row[2] * v[2] as f64;
        if !partial.is_finite() || partial.abs() >= pow2(40) { continue; }
        let target = partial.round_ties_even() + if g.next() & 1 == 0 { 0.5 } else { 0.0 };
        let fine = (g.unit() - 0.5) * pow2(g.range(-70, -20) as i32);
        row[3] = (target - partial) + fine;
        let Ok(tr) = Transform::from_rows([[row[0], row[1], row[2], row[3]], [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]) else { continue };
        let Ok(p) = P3::new(v[0], v[1], v[2]) else { continue };
        // Exact value and its rounding.
        let val: BigRational = exact(row[0]) * int(v[0].into()) + exact(row[1]) * int(v[1].into())
            + exact(row[2]) * int(v[2].into()) + exact(row[3]);
        let want = to_i128(&round_half_even(&val));
        if want.unsigned_abs() >= 1 << 40 { continue; }
        let want_exact = is_integer(&val);
        let Ok(got) = tr.apply_grid(p) else { continue };
        assert_eq!(i128::from(got.value().x.get()), want, "kernel");
        assert_eq!(got.is_exact(), want_exact, "kernel exact");
        let wrong = |r: i128| r != want || want_exact;
        let orig = float_filter(row, v, 0);
        if let Some(r) = orig { assert!(!wrong(r.into()), "float filter accepted wrongly"); continue; }
        for variant in 1..=4u8 {
            if let Some(r) = float_filter(row, v, variant) {
                if wrong(r.into()) {
                    found[variant as usize] += 1;
                    if found[variant as usize] <= 3 {
                        println!("FLOAT{variant} row {:?} v {v:?} want {want} exact {want_exact} got {r}", row.map(f64::to_bits));
                    }
                }
            }
        }
        let dorig = dd_filter(row, v, 0);
        if let Some(r) = dorig { assert!(!wrong(r), "dd filter accepted wrongly"); continue; }
        for variant in 1..=4u8 {
            if let Some(r) = dd_filter(row, v, variant) {
                if wrong(r) {
                    found[4 + variant as usize] += 1;
                    if found[4 + variant as usize] <= 3 {
                        println!("DD{variant} row {:?} v {v:?} want {want} exact {want_exact} got {r}", row.map(f64::to_bits));
                    }
                }
            }
        }
    }
    println!("found {found:?}");
}
