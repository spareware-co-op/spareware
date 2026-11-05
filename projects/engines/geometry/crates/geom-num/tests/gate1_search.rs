//! Mutant search harness (never committed): runs the public maths functions over CORE-MATH's
//! worst-case lists plus edge and random inputs, and compares every outcome with the unmutated
//! build's (GATE1_GEN=1 writes them). Prints the first differing input per function.
#![allow(clippy::all, clippy::pedantic, missing_docs)]

#[path = "math_reference.rs"]
mod reference;

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use geom_num::interval::Interval;
use geom_num::{NumError, Real, math};

static CUR_A: AtomicU64 = AtomicU64::new(0);
static CUR_B: AtomicU64 = AtomicU64::new(0);
static CUR_F: AtomicU64 = AtomicU64::new(0);
static TICK: AtomicU64 = AtomicU64::new(0);

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    /// A double with a random significand in the binade [2^e, 2^(e+1)).
    fn binade(&mut self, e: i64) -> f64 {
        let m = self.next() & ((1u64 << 52) - 1);
        if e < -1022 {
            let s = (-1022 - e) as u32;
            if s >= 52 {
                return f64::from_bits(1);
            }
            return f64::from_bits(((1u64 << 52) | m) >> (s + 1)).max(f64::from_bits(1));
        }
        f64::from_bits((((e + 1023) as u64) << 52) | m)
    }
    fn any_finite(&mut self) -> f64 {
        loop {
            let x = f64::from_bits(self.next());
            if x.is_finite() {
                return x;
            }
        }
    }
}

fn enc(r: Result<Real, NumError>) -> u64 {
    match r {
        Ok(v) => v.get().to_bits(),
        Err(NumError::NotFinite) => 0xfff8_0000_0000_0001,
        Err(NumError::OutsideDomain) => 0xfff8_0000_0000_0002,
        Err(_) => 0xfff8_0000_0000_0003,
    }
}

fn enc_iv(r: Result<Interval, NumError>) -> (u64, u64) {
    match r {
        Ok(v) => (v.lo().to_bits(), v.hi().to_bits()),
        Err(NumError::NotFinite) => (0xfff8_0000_0000_0001, 0),
        Err(NumError::OutsideDomain) => (0xfff8_0000_0000_0002, 0),
        Err(_) => (0xfff8_0000_0000_0003, 0),
    }
}

const PANIC: u64 = 0xfff8_0000_0000_0004;

fn parse(token: &str) -> Option<f64> {
    let t = token.trim();
    let lower = t.to_ascii_lowercase();
    if t.is_empty() || lower.contains("nan") || lower.contains("inf") {
        return None;
    }
    if lower.contains("0x") {
        Some(reference::hx_rounded(t))
    } else {
        t.parse().ok()
    }
}

fn wc_lines(function: &str) -> Vec<String> {
    let dir = std::env::var_os("CORE_MATH_DIR").expect("CORE_MATH_DIR");
    let path = PathBuf::from(dir).join(function).join(format!("{function}.wc"));
    let text = std::fs::read_to_string(&path).expect("wc file");
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn wc_inputs(function: &str) -> Vec<f64> {
    wc_lines(function)
        .iter()
        .filter_map(|line| line.split_whitespace().next().and_then(parse))
        .collect()
}

fn pow2(k: i64) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

/// Neighbours of x: x itself and up to n steps either side.
fn around(out: &mut Vec<f64>, x: f64, n: usize) {
    out.push(x);
    let (mut u, mut d) = (x, x);
    for _ in 0..n {
        u = u.next_up();
        d = d.next_down();
        out.push(u);
        out.push(d);
    }
}

fn half_turn_inputs() -> Vec<f64> {
    let mut v = Vec::new();
    let mut l = wc_inputs("sinpi");
    l.extend(wc_inputs("cospi"));
    // sinpi set (b) images, as math_full_worst_cases does.
    let lo = f64::MIN_POSITIVE;
    let set_b: Vec<f64> = l.iter().copied().filter(|x| (lo..2.0 * lo).contains(x)).collect();
    for e in (1..=968).step_by(7) {
        let scale = pow2(e);
        l.extend(set_b.iter().map(|x| x * scale));
    }
    for &x in &l {
        v.push(x);
        v.push(-x);
    }
    // Edges: multiples of 2^-j near many magnitudes, and their neighbours.
    for j in 0..14 {
        for k in -40i64..=40 {
            let x = k as f64 * pow2(-j);
            around(&mut v, x, 3);
        }
    }
    for e in -1074..=1023 {
        let x = pow2(e);
        around(&mut v, x, 2);
        around(&mut v, -x, 2);
        around(&mut v, 1.5 * x, 1);
        around(&mut v, 0.75 * x, 1);
    }
    let mut r = Rng(1);
    for _ in 0..150_000 {
        v.push(r.any_finite());
        let e = r.below(70) as i64 - 60;
        let x = r.binade(e);
        v.push(if r.next() & 1 == 0 { x } else { -x });
        v.push(r.uniform(-4.0, 4.0));
        // near a multiple of 1/2^j
        let j = r.below(13) as i64;
        let k = r.below(1 << 14) as f64 - 8192.0;
        let mut x = k * pow2(-j);
        for _ in 0..r.below(6) {
            x = if r.next() & 1 == 0 { x.next_up() } else { x.next_down() };
        }
        v.push(x);
    }
    for _ in 0..30_000 {
        let e = r.below(1100) as i64 - 1074;
        v.push(r.binade(e));
        v.push(-r.binade(e));
    }
    v
}

fn exp_inputs() -> Vec<f64> {
    let mut v = wc_inputs("exp");
    for t in [
        709.782712893384,
        -745.1332191019411,
        -708.3964185322641,
        -744.4400719213812,
        -745.1332191019412,
        0.0,
        1.0,
        -1.0,
        2.0f64.powi(-54),
        -(2.0f64.powi(-54)),
        2.0f64.powi(-53),
        -(2.0f64.powi(-53)),
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        1e-300,
        -1e-300,
    ] {
        around(&mut v, t, 40);
    }
    for e in -1074..=10 {
        around(&mut v, pow2(e), 2);
        around(&mut v, -pow2(e), 2);
    }
    let mut r = Rng(2);
    for _ in 0..150_000 {
        v.push(r.uniform(-746.0, 710.0));
        v.push(r.uniform(-746.0, -707.0));
        v.push(r.uniform(-1.0, 1.0));
        let e = r.below(70) as i64 - 60;
        let x = r.binade(e);
        v.push(if r.next() & 1 == 0 { x } else { -x });
    }
    for _ in 0..20_000 {
        v.push(r.any_finite());
        let e = r.below(1084) as i64 - 1074;
        v.push(r.binade(e));
        v.push(-r.binade(e));
    }
    v
}

fn ln_inputs() -> Vec<f64> {
    let mut v = wc_inputs("log");
    for e in -1074..=1023 {
        around(&mut v, pow2(e), 3);
        around(&mut v, 1.5 * pow2(e), 1);
    }
    around(&mut v, 1.0, 400);
    around(&mut v, f64::MAX, 3);
    for t in [0.0, -0.0, -1.0, -f64::MIN_POSITIVE, f64::MIN_POSITIVE] {
        v.push(t);
    }
    let mut r = Rng(3);
    for _ in 0..150_000 {
        v.push(r.any_finite().abs());
        let e = r.below(2098) as i64 - 1074;
        v.push(r.binade(e));
        v.push(r.uniform(0.5, 2.0));
        v.push(r.uniform(0.9, 1.1));
        let x: f64 = 1.0;
        let k = r.below(1 << 20) as i64 - (1 << 19);
        v.push(f64::from_bits((x.to_bits() as i64 + k) as u64));
    }
    v
}

fn atan2_inputs() -> Vec<(f64, f64)> {
    let mut v = Vec::new();
    for line in wc_lines("atan2") {
        let mut parts = line.split(',');
        let (Some(y), Some(x)) = (parts.next().and_then(parse), parts.next().and_then(parse)) else {
            continue;
        };
        for (y, x) in [(y, x), (-y, x), (y, -x), (-y, -x), (x, y), (-x, y), (x, -y), (-x, -y)] {
            v.push((y, x));
        }
    }
    // atan2 is scale-invariant: each worst case scaled exactly by a power of two into the
    // binades of the scaling branches (x > 2^1022, x < 2^-920) and into the subnormal range.
    let base: Vec<(f64, f64)> = v.iter().copied().step_by(8).collect();
    for (y, x) in base {
        if y == 0.0 || x == 0.0 || !y.is_finite() || !x.is_finite() {
            continue;
        }
        let e = (y.abs().max(x.abs())).log2().floor() as i64;
        for target in [1023i64, 1022, -919, -921, -1000, -1021, -1022, -1023, -1030, -1040, -1050, -1060, -1070] {
            let k = target - e;
            let (mut sy, mut sx) = (y, x);
            // scale in steps that stay exact while normal
            let mut kk = k;
            while kk != 0 {
                let step = kk.clamp(-600, 600);
                sy *= pow2(step).max(f64::MIN_POSITIVE);
                sx *= pow2(step).max(f64::MIN_POSITIVE);
                if step < -1022 || step > 1023 {
                    break;
                }
                kk -= step;
            }
            if sy == 0.0 || sx == 0.0 || !sy.is_finite() || !sx.is_finite() {
                continue;
            }
            // keep only exact scalings: the ratio and each value's bits survive the round trip
            let back = |s: f64| {
                let mut b = s;
                let mut kk = -k;
                while kk != 0 {
                    let step = kk.clamp(-600, 600);
                    b *= pow2(step);
                    kk -= step;
                }
                b
            };
            if back(sy) == y && back(sx) == x {
                for (a, b) in [(sy, sx), (-sy, sx), (sy, -sx), (-sy, -sx), (sx, sy), (-sx, sy), (sx, -sy), (-sx, -sy)] {
                    v.push((a, b));
                }
            }
        }
    }
    let specials = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MAX,
        -f64::MAX,
        2.0,
        0.5,
        3.0,
        1e-300,
        1e300,
    ];
    for &y in &specials {
        for &x in &specials {
            v.push((y, x));
            v.push((y.next_up(), x));
            v.push((y, x.next_down()));
        }
    }
    let mut r = Rng(4);
    for _ in 0..120_000 {
        v.push((r.any_finite(), r.any_finite()));
        let ey = r.below(140) as i64 - 70;
        let d = r.below(140) as i64 - 70;
        let (y, x) = (r.binade(ey), r.binade((ey - d).clamp(-1074, 1023)));
        let sy = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
        let sx = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
        v.push((sy * y, sx * x));
        let e = r.below(2098) as i64 - 1074;
        let f = r.below(2098) as i64 - 1074;
        v.push((sy * r.binade(e), sx * r.binade(f)));
        // near the diagonals and axes
        let ex = r.below(40) as i64 - 20;
        let x = r.binade(ex);
        let mut y = x;
        for _ in 0..r.below(4) {
            y = y.next_up();
        }
        v.push((sy * y, sx * x));
        let ey = r.below(200) as i64 - 1074;
        let ex = r.below(80) as i64 - 40;
        let (a, b) = (r.binade(ey), r.binade(ex));
        v.push((sy * a, sx * b));
    }
    v
}

fn interval_inputs() -> Vec<(f64, f64)> {
    let mut r = Rng(5);
    let mut ends = Vec::new();
    for j in 0..6 {
        for k in -12i64..=12 {
            let x = k as f64 * pow2(-j);
            around(&mut ends, x, 2);
        }
    }
    let mut v = Vec::new();
    for _ in 0..60_000 {
        let a = if r.next() & 1 == 0 {
            ends[r.below(ends.len() as u64) as usize]
        } else {
            r.uniform(-6.0, 6.0)
        };
        let b = if r.next() & 1 == 0 {
            ends[r.below(ends.len() as u64) as usize]
        } else {
            a + r.uniform(0.0, 3.0)
        };
        v.push((a.min(b), a.max(b)));
    }
    v
}

fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

/// Runs every case; returns the outcome words.
fn run_all(report: &mut Vec<String>, expected: Option<&[u64]>) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    let mut first: std::collections::BTreeMap<&'static str, String> = Default::default();
    let mut count: std::collections::BTreeMap<&'static str, u64> = Default::default();
    let mut check = |name: &'static str, out: &mut Vec<u64>, a: f64, b: f64, w: u64| {
        let i = out.len();
        out.push(w);
        TICK.fetch_add(1, Ordering::Relaxed);
        if let Some(e) = expected {
            if e.get(i) != Some(&w) {
                *count.entry(name).or_default() += 1;
                first.entry(name).or_insert_with(|| {
                    format!(
                        "DIFF {name} a={:#018x} b={:#018x} ({a:e}, {b:e}) got={w:#018x} want={:#018x}",
                        a.to_bits(),
                        b.to_bits(),
                        e.get(i).copied().unwrap_or(0)
                    )
                });
            }
        }
    };
    let set = |f: u64, a: f64, b: f64| {
        CUR_F.store(f, Ordering::Relaxed);
        CUR_A.store(a.to_bits(), Ordering::Relaxed);
        CUR_B.store(b.to_bits(), Ordering::Relaxed);
    };
    for x in half_turn_inputs() {
        set(1, x, 0.0);
        let w = guarded(|| enc(math::sinpi(x))).unwrap_or(PANIC);
        check("sinpi", &mut out, x, 0.0, w);
        set(2, x, 0.0);
        let w = guarded(|| enc(math::cospi(x))).unwrap_or(PANIC);
        check("cospi", &mut out, x, 0.0, w);
        set(3, x, 0.0);
        let w = guarded(|| enc(math::unproven::sinpi(x))).unwrap_or(PANIC);
        check("u_sinpi", &mut out, x, 0.0, w);
        set(4, x, 0.0);
        let w = guarded(|| enc(math::unproven::cospi(x))).unwrap_or(PANIC);
        check("u_cospi", &mut out, x, 0.0, w);
    }
    for x in exp_inputs() {
        set(5, x, 0.0);
        let w = guarded(|| enc(math::exp(x))).unwrap_or(PANIC);
        check("exp", &mut out, x, 0.0, w);
    }
    for x in ln_inputs() {
        set(6, x, 0.0);
        let w = guarded(|| enc(math::ln(x))).unwrap_or(PANIC);
        check("ln", &mut out, x, 0.0, w);
    }
    for (y, x) in atan2_inputs() {
        set(7, y, x);
        let w = guarded(|| enc(math::atan2(y, x))).unwrap_or(PANIC);
        check("atan2", &mut out, y, x, w);
    }
    let ivs = interval_inputs();
    for pair in ivs.chunks_exact(2) {
        let (Ok(yv), Ok(xv)) = (Interval::new(pair[0].0, pair[0].1), Interval::new(pair[1].0, pair[1].1)) else {
            continue;
        };
        set(9, pair[0].0, pair[1].0);
        let (p, q) = guarded(|| enc_iv(yv.atan2(xv))).unwrap_or((PANIC, 0));
        check("iv_atan2", &mut out, pair[0].0, pair[1].0, p);
        check("iv_atan2_hi", &mut out, pair[0].1, pair[1].1, q);
    }
    for (a, b) in ivs {
        let Ok(iv) = Interval::new(a, b) else { continue };
        set(8, a, b);
        let (p, q) = guarded(|| enc_iv(iv.sinpi())).unwrap_or((PANIC, 0));
        check("iv_sinpi", &mut out, a, b, p);
        check("iv_sinpi_hi", &mut out, a, b, q);
        let (p, q) = guarded(|| enc_iv(iv.cospi())).unwrap_or((PANIC, 0));
        check("iv_cospi", &mut out, a, b, p);
        check("iv_cospi_hi", &mut out, a, b, q);
        let (p, q) = guarded(|| enc_iv(iv.exp())).unwrap_or((PANIC, 0));
        check("iv_exp", &mut out, a, b, p);
        check("iv_exp_hi", &mut out, a, b, q);
        let (p, q) = guarded(|| enc_iv(iv.ln())).unwrap_or((PANIC, 0));
        check("iv_ln", &mut out, a, b, p);
        check("iv_ln_hi", &mut out, a, b, q);
    }
    for (name, line) in &first {
        report.push(format!("{line} (count {})", count.get(name).copied().unwrap_or(0)));
    }
    out
}

#[test]
fn search() {
    std::panic::set_hook(Box::new(|_| {}));
    // Watchdog: a call that runs for more than 20 s is a hang; report its input and stop.
    std::thread::spawn(|| {
        let mut last = TICK.load(Ordering::Relaxed);
        let mut since = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let now = TICK.load(Ordering::Relaxed);
            if now != last {
                last = now;
                since = Instant::now();
            } else if since.elapsed() > Duration::from_secs(20) {
                println!(
                    "HANG f={} a={:#018x} b={:#018x}",
                    CUR_F.load(Ordering::Relaxed),
                    CUR_A.load(Ordering::Relaxed),
                    CUR_B.load(Ordering::Relaxed)
                );
                std::io::stdout().flush().ok();
                std::process::exit(3);
            }
        }
    });
    let dir = PathBuf::from(std::env::var_os("GATE1_SEARCH_DIR").expect("GATE1_SEARCH_DIR"));
    let path = dir.join("expected.bin");
    let mut report = Vec::new();
    if std::env::var_os("GATE1_GEN").is_some() {
        let out = run_all(&mut report, None);
        let bytes: Vec<u8> = out.iter().flat_map(|w| w.to_le_bytes()).collect();
        std::fs::write(&path, bytes).expect("write expected");
        println!("GEN {} outcomes", out.len());
        return;
    }
    let bytes = std::fs::read(&path).expect("read expected");
    let expected: Vec<u64> = bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let out = run_all(&mut report, Some(&expected));
    for line in &report {
        println!("{line}");
    }
    assert_eq!(out.len(), expected.len(), "LEN mismatch");
    assert!(report.is_empty(), "differences found");
}
