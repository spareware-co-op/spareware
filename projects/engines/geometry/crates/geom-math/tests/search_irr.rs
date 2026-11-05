#![allow(clippy::all, clippy::pedantic, warnings)]
mod common;

use common::{exact, int, sign_minus_sqrt, step_down, step_up};
use geom_math::frame::steps_per_metre;
use geom_math::{CrsId, DD, FrameRecord, Rotation, Scale};
use geom_num::grid::{GridStep, P3};
use num_bigint::BigInt;
use num_integer::Roots;
use num_rational::BigRational;
use std::cmp::Ordering;

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
}

fn pow2(k: i32) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

fn egcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 { (a, 1, 0) } else { let (g, x, y) = egcd(b, a % b); (g, y, x - (a / b) * y) }
}

/// Is d the nearest double to V = (o + kn·a/(kd·√n))/S? (frames.rs's check)
fn correct(d: f64, o: i128, a: i128, kn: i128, kd: i128, s: i128, n2: u64) -> bool {
    let tm = |w: f64| ((exact(w) + exact(d)) / int(2) * int(s) - int(o)) * int(kd) / int(kn);
    let a = int(a);
    sign_minus_sqrt(&a, &tm(step_down(d)), n2) == Ordering::Greater
        && sign_minus_sqrt(&a, &tm(step_up(d)), n2) == Ordering::Less
}

#[test]
fn search() {
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let n_samples: usize = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(20000);
    let mut g = G(seed);
    let step = GridStep::Q0;
    let s = steps_per_metre(step) as i128;
    let sf = s as f64;
    let (mut up_cross, mut down_cross, mut wrong_h, mut samples, mut fallback) = (0, 0, 0, 0, 0);
    let mut skip = [0usize; 6];
    let mut shown_up = 0;
    let mut shown_down = 0;
    for _ in 0..n_samples {
        let c = g.range(1 << 20, 1 << 31) as i32 * if g.next() & 1 == 0 { 1 } else { -1 };
        let sn = g.range(1 << 20, 1 << 31) as i32 * if g.next() & 1 == 0 { 1 } else { -1 };
        let Ok(rot) = Rotation::direction(c, sn) else { skip[0] += 1; continue };
        if rot.exact_norm().is_some() { skip[1] += 1; continue; }
        let (c, sn) = (rot.cos() as i128, rot.sin() as i128);
        let n2 = rot.norm2();
        let (kn, kd) = (g.range(1 << 18, 1 << 20) as u32, g.range(1, 5) as u32);
        let scale = Scale::new(kn, kd).unwrap();
        let (kn, kd) = (scale.num() as i128, scale.den() as i128);
        let o = g.range(1 << 50, 1 << 55) as i128 * if g.next() & 1 == 0 { 1 } else { -1 };
        // target τ = V·S in steps: cancellation chosen so err/ulp spans about 2^-6 .. 2^0.
        let m = ((o.abs() >> 46) << g.range(0, 5)).max(2) as i64;
        let tau = g.range(m, 2 * m) as i128;
        let tau = if g.next() & 1 == 0 { tau } else { -tau };
        // A ≈ (τ − O)·√n·kd/kn.
        let t = BigInt::from(tau - o) * BigInt::from(kd);
        let sq = (&t * &t * BigInt::from(n2)).sqrt() / BigInt::from(kn);
        let a_big = if t < BigInt::from(0) { -sq } else { sq };
        let Ok(a) = i128::try_from(a_big) else { skip[2] += 1; continue };
        // c·x − s·y = a with small x.
        let (gg, u, v) = egcd(c, -sn);
        if gg.abs() != 1 { skip[3] += 1; continue; }
        let (u, v) = (u * gg, v * gg); // c·u + (−s)·v = 1
        let x0 = a * u; let y0 = -(a * v); // c·x0 − s·y0 = a
        let k = -x0.div_euclid(sn);
        let x = x0 + sn * k; let _ = y0; let y = (c * x - a) / sn;
        if c * x - sn * y != a || x.abs() >= 1 << 40 || y.abs() >= 1 << 40 { skip[4] += 1; if skip[4] < 3 { println!("solve c {c} s {sn} a {a} x {x} y {y}"); } continue; }
        let Ok(f) = FrameRecord::new(CrsId::Epsg(2193), step, [o as i64, 0, 0], rot, scale) else { skip[5] += 1; continue };
        let p = P3::new(x as i64, y as i64, 0).unwrap();
        samples += 1;
        // The conversion's double-double, as irrational_to_map computes it.
        let kn_a = kn * a;
        let num = DD::from_i128(kn_a).unwrap();
        let den = DD::from_i64(n2 as i64).sqrt().unwrap().mul_f64(kd as f64).unwrap();
        let q = num.div(den).unwrap();
        let od = DD::from_i128(o).unwrap();
        let metres = od.add(q).unwrap().div(DD::from_i64(s as i64)).unwrap();
        let (h, l) = (metres.hi(), metres.lo());
        let err = (h.abs() + q.hi().abs() / sf) * pow2(-98);
        let up = h.next_up() - h;
        let down = h - h.next_down();
        let ca = l + 2.0 * err < 0.5 * up;
        let cb = l - 2.0 * err > -0.5 * down;
        let got = f.to_map(p).unwrap().value().easting;
        assert!(correct(got, o, a, kn, kd, s, n2), "kernel wrong");
        if !(h != 0.0 && ca && cb) { fallback += 1; }
        if h != got {
            wrong_h += 1;
            assert!(!(h != 0.0 && ca && cb), "certificate accepted a wrong h");
            let upward = got > h;
            let line = format!("dir ({c}, {sn}) scale ({kn}, {kd}) origin {o} point ({x}, {y}) h {h:e} got {got:e} err/ulp {:.3} A {ca} B {cb}", err / up);
            if upward && cb { up_cross += 1; if shown_up < 12 { println!("UP   {line}"); shown_up += 1; } }
            if !upward && ca { down_cross += 1; if shown_down < 12 { println!("DOWN {line}"); shown_down += 1; } }
        }
    }
    println!("skip {skip:?}");
    println!("samples {samples} fallback {fallback} wrong_h {wrong_h} up_cross {up_cross} down_cross {down_cross}");
}
