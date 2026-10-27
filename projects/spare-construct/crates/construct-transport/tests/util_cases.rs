//! The JavaScript number rules (jsnum) and the util.js helpers (util) against Node.
//!
//! Every case in tools/transport-truth/expected/util-cases.json (written by
//! gen-util-cases.mjs from the prototype's own code) must come out the same: numbers to the
//! bit (NaN equal to NaN, -0 apart from +0) and text exactly.
//! One exception: results that go through sin, cos, asin or atan2 (U.dist, U.bearing,
//! U.cumLen, U.polyLen, U.flat's kx) are checked to a relative 1e-9. Rust's f64 functions
//! call the platform's C library, which differs from V8's own implementations in the last bit
//! for about 1 to 15% of arguments, and a bearing between points a few metres apart magnifies
//! that (cancellation); the test prints how many came out bit-identical.

mod support;

use construct_transport::jsnum::{
    intl_format, math_hypot, math_max, math_min, math_round, number_to_js_string, string_to_number,
    to_fixed, to_int32, to_precision, to_uint32, value_to_js_string, value_to_number, value_truthy,
};
use construct_transport::util::{
    Heap, LatLng, along, ang_diff, bearing, clamp, csv, csv_cell, cum_len, dist, esc, flat, fmt,
    poly_len, r1, r3, r7, seg_dist, turn,
};
use serde_json::Value;
use std::sync::OnceLock;

fn cases() -> &'static Value {
    static C: OnceLock<Value> = OnceLock::new();
    C.get_or_init(|| support::load_json("expected/util-cases.json"))
}

fn group(name: &str) -> &'static Vec<Value> {
    cases()[name]
        .as_array()
        .unwrap_or_else(|| panic!("no case group {name}"))
}

// A number as gen-util-cases.mjs writes it ("NaN", "Infinity", "-Infinity", "-0" for those JSON cannot hold)
fn dec(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            "-0" => -0.0,
            _ => panic!("not an encoded number: {s:?}"),
        },
        _ => panic!("not an encoded number: {v}"),
    }
}

fn pt(v: &Value) -> LatLng {
    [dec(&v[0]), dec(&v[1])]
}

fn pts(v: &Value) -> Vec<LatLng> {
    v.as_array().unwrap().iter().map(pt).collect()
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn usize_of(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}

// Collects mismatches so one run reports them all (up to 30)
struct Check {
    what: &'static str,
    cases: usize,
    fails: Vec<String>,
    // cases checked by close() and how many of them were bit-identical anyway
    near: usize,
    near_exact: usize,
}

// Relative tolerance for results of the platform's sin, cos, asin and atan2
const TRIG_REL: f64 = 1e-9;

impl Check {
    fn new(what: &'static str) -> Self {
        Check {
            what,
            cases: 0,
            fails: Vec::new(),
            near: 0,
            near_exact: 0,
        }
    }
    fn close(&mut self, label: impl FnOnce() -> String, actual: f64, expected: f64) {
        self.cases += 1;
        self.near += 1;
        if (actual.is_nan() && expected.is_nan()) || actual.to_bits() == expected.to_bits() {
            self.near_exact += 1;
            return;
        }
        let near = (actual - expected).abs() <= TRIG_REL * expected.abs().max(1.0);
        if !near {
            self.fails.push(format!(
                "{}: expected {:e}, got {:e}",
                label(),
                expected,
                actual
            ));
        }
    }
    fn num(&mut self, label: impl FnOnce() -> String, actual: f64, expected: f64) {
        self.cases += 1;
        let same = (actual.is_nan() && expected.is_nan()) || actual.to_bits() == expected.to_bits();
        if !same {
            self.fails.push(format!(
                "{}: expected {:e}, got {:e}",
                label(),
                expected,
                actual
            ));
        }
    }
    fn text(&mut self, label: impl FnOnce() -> String, actual: &str, expected: &str) {
        self.cases += 1;
        if actual != expected {
            self.fails.push(format!(
                "{}: expected {:?}, got {:?}",
                label(),
                expected,
                actual
            ));
        }
    }
    fn truth(&mut self, label: impl FnOnce() -> String, actual: bool, expected: bool) {
        self.cases += 1;
        if actual != expected {
            self.fails.push(format!(
                "{}: expected {}, got {}",
                label(),
                expected,
                actual
            ));
        }
    }
    fn done(self) {
        assert!(self.cases > 0, "{}: no cases", self.what);
        if self.near > 0 {
            eprintln!(
                "{}: {} of {} results through the platform's trig functions are bit-identical to V8's",
                self.what, self.near_exact, self.near
            );
        }
        if !self.fails.is_empty() {
            panic!(
                "{}: {} of {} cases differ from the JavaScript:\n  {}",
                self.what,
                self.fails.len(),
                self.cases,
                self.fails
                    .iter()
                    .take(30)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n  ")
            );
        }
    }
}

#[test]
fn js_to_number() {
    let mut c = Check::new("Number(text)");
    for k in group("stringToNumber") {
        c.num(
            || format!("{:?}", k[0]),
            string_to_number(text(&k[0])),
            dec(&k[1]),
        );
    }
    c.done();
    let mut c = Check::new("Number(value), String(value), !!value");
    for k in group("valueToNumber") {
        c.num(|| k[0].to_string(), value_to_number(&k[0]), dec(&k[1]));
    }
    for k in group("valueToString") {
        c.text(|| k[0].to_string(), &value_to_js_string(&k[0]), text(&k[1]));
    }
    for k in group("truthy") {
        c.truth(
            || k[0].to_string(),
            value_truthy(&k[0]),
            k[1].as_bool().unwrap(),
        );
    }
    c.done();
}

#[test]
fn js_int32_and_round() {
    let mut c = Check::new("x | 0, x >>> 0, Math.round");
    for k in group("toInt32") {
        c.num(
            || format!("{} | 0", k[0]),
            to_int32(dec(&k[0])) as f64,
            dec(&k[1]),
        );
    }
    for k in group("toUint32") {
        c.num(
            || format!("{} >>> 0", k[0]),
            to_uint32(dec(&k[0])) as f64,
            dec(&k[1]),
        );
    }
    for k in group("round") {
        c.num(
            || format!("Math.round({})", k[0]),
            math_round(dec(&k[0])),
            dec(&k[1]),
        );
    }
    for k in group("hypot") {
        let a: Vec<f64> = k[0].as_array().unwrap().iter().map(dec).collect();
        c.num(|| format!("Math.hypot{:?}", a), math_hypot(&a), dec(&k[1]));
    }
    for k in group("minMax") {
        let (a, b) = (dec(&k[0]), dec(&k[1]));
        c.num(|| format!("Math.min({a}, {b})"), math_min(a, b), dec(&k[2]));
        c.num(|| format!("Math.max({a}, {b})"), math_max(a, b), dec(&k[3]));
    }
    c.done();
}

#[test]
fn js_number_text() {
    let mut c = Check::new("String(x), toFixed, toPrecision, Intl.NumberFormat");
    for k in group("numberToString") {
        c.text(
            || format!("String({})", k[0]),
            &number_to_js_string(dec(&k[0])),
            text(&k[1]),
        );
    }
    for k in group("toFixed") {
        let (x, d) = (dec(&k[0]), usize_of(&k[1]));
        c.text(
            || format!("({:e}).toFixed({d})", x),
            &to_fixed(x, d),
            text(&k[2]),
        );
    }
    for k in group("toPrecision") {
        let (x, p) = (dec(&k[0]), usize_of(&k[1]));
        c.text(
            || format!("({:e}).toPrecision({p})", x),
            &to_precision(x, p),
            text(&k[2]),
        );
    }
    for k in group("intl") {
        let (x, d) = (dec(&k[0]), usize_of(&k[1]));
        c.text(
            || format!("Intl en-NZ ({:e}, {d})", x),
            &intl_format(x, d),
            text(&k[2]),
        );
    }
    c.done();
}

#[test]
fn util_geodesy() {
    let mut c = Check::new("U.dist, U.bearing, U.cumLen, U.polyLen, U.along, U.flat");
    for k in group("dist") {
        let (a, b) = (pt(&k[0]), pt(&k[1]));
        c.close(|| format!("dist({a:?}, {b:?})"), dist(&a, &b), dec(&k[2]));
    }
    for k in group("bearing") {
        let (a, b) = (pt(&k[0]), pt(&k[1]));
        c.close(
            || format!("bearing({a:?}, {b:?})"),
            bearing(&a, &b),
            dec(&k[2]),
        );
    }
    for k in group("cumLen") {
        let p = pts(&k[0]);
        let got = cum_len(&p);
        let want: Vec<f64> = k[1].as_array().unwrap().iter().map(dec).collect();
        c.truth(
            || format!("cumLen length {p:?}"),
            got.len() == want.len(),
            true,
        );
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            c.close(|| format!("cumLen {p:?} [{i}]"), *g, *w);
        }
        c.close(|| format!("polyLen {p:?}"), poly_len(&p), dec(&k[2]));
    }
    // along on the JavaScript's own cumulative lengths, so it is checked to the bit
    for k in group("along") {
        let p = pts(&k[0]);
        let cum: Vec<f64> = k[1].as_array().unwrap().iter().map(dec).collect();
        for a in k[2].as_array().unwrap() {
            let s = dec(&a[0]);
            let r = along(&p, &cum, s);
            c.num(|| format!("along({p:?}, {s}).p[0]"), r.p[0], dec(&a[1][0]));
            c.num(|| format!("along({p:?}, {s}).p[1]"), r.p[1], dec(&a[1][1]));
            c.num(|| format!("along({p:?}, {s}).i"), r.i as f64, dec(&a[2]));
            c.num(|| format!("along({p:?}, {s}).f"), r.f, dec(&a[3]));
        }
    }
    for k in group("flat") {
        let lat0 = dec(&k[0]);
        let f = flat(lat0);
        let p = pt(&k[3]);
        c.close(|| format!("flat({lat0}).kx"), f.kx, dec(&k[1]));
        c.num(|| format!("flat({lat0}).ky"), f.ky, dec(&k[2]));
        c.close(|| format!("flat({lat0}).x({p:?})"), f.x(&p), dec(&k[4]));
        c.num(|| format!("flat({lat0}).y({p:?})"), f.y(&p), dec(&k[5]));
    }
    c.done();
}

#[test]
fn util_angles_rounding_and_segments() {
    let mut c = Check::new("U.angDiff, U.turn, U.segDist, U.clamp, U.r7, U.r1, U.r3");
    for k in group("angDiff") {
        let (a, b) = (dec(&k[0]), dec(&k[1]));
        c.num(|| format!("angDiff({a}, {b})"), ang_diff(a, b), dec(&k[2]));
    }
    for k in group("turn") {
        let (a, b) = (dec(&k[0]), dec(&k[1]));
        c.num(|| format!("turn({a}, {b})"), turn(a, b), dec(&k[2]));
    }
    for k in group("segDist") {
        let a: Vec<f64> = k[0].as_array().unwrap().iter().map(dec).collect();
        let r = seg_dist(a[0], a[1], a[2], a[3], a[4], a[5]);
        c.num(|| format!("segDist{a:?}.d"), r.d, dec(&k[1]));
        c.num(|| format!("segDist{a:?}.t"), r.t, dec(&k[2]));
        c.num(|| format!("segDist{a:?}.x"), r.x, dec(&k[3]));
        c.num(|| format!("segDist{a:?}.y"), r.y, dec(&k[4]));
    }
    for k in group("clamp") {
        let (x, a, b) = (dec(&k[0]), dec(&k[1]), dec(&k[2]));
        c.num(
            || format!("clamp({x}, {a}, {b})"),
            clamp(x, a, b),
            dec(&k[3]),
        );
    }
    for (name, f) in [("r7", r7 as fn(f64) -> f64), ("r1", r1), ("r3", r3)] {
        for k in group(name) {
            let x = dec(&k[0]);
            c.num(|| format!("{name}({x:e})"), f(x), dec(&k[1]));
        }
    }
    c.done();
}

#[test]
fn util_heap() {
    let mut c = Check::new("U.Heap");
    for (n, run) in group("heap").iter().enumerate() {
        let mut h = Heap::with_capacity(4);
        let mut pops = Vec::new();
        for op in run["ops"].as_array().unwrap() {
            if op[0] == "push" {
                h.push(dec(&op[1]), op[2].as_i64().unwrap() as i32);
            } else {
                let v = h.pop();
                pops.push((v, h.top));
            }
        }
        let want = run["pops"].as_array().unwrap();
        c.truth(|| format!("run {n}: pops"), pops.len() == want.len(), true);
        for (i, ((v, top), w)) in pops.iter().zip(want).enumerate() {
            c.num(|| format!("run {n} pop {i}: value"), *v as f64, dec(&w[0]));
            c.num(|| format!("run {n} pop {i}: top"), *top, dec(&w[1]));
        }
        c.truth(|| format!("run {n}: empty at the end"), h.is_empty(), true);
    }
    c.done();
}

#[test]
fn util_formats_and_text() {
    let mut c = Check::new("U.fmt, U.esc, U.csvCell, U.csv");
    for k in group("fmt") {
        let x = dec(&k[0]);
        for d in 0..3 {
            c.text(
                || format!("fmt.num({x:e}, {d})"),
                &fmt::num(x, d),
                text(&k[1][d]),
            );
            c.text(
                || format!("fmt.signed({x:e}, {d})"),
                &fmt::signed(x, d),
                text(&k[7][d]),
            );
            c.text(
                || format!("fmt.signedPct({x:e}, {d})"),
                &fmt::signed_pct(x, d),
                text(&k[8][d]),
            );
        }
        for d in 0..2 {
            c.text(
                || format!("fmt.pct({x:e}, {d})"),
                &fmt::pct(x, d),
                text(&k[3][d]),
            );
        }
        c.text(|| format!("fmt.int({x:e})"), &fmt::int(x), text(&k[2]));
        c.text(|| format!("fmt.money({x:e})"), &fmt::money(x), text(&k[4]));
        c.text(|| format!("fmt.dur({x:e})"), &fmt::dur(x), text(&k[5]));
        c.text(|| format!("fmt.len({x:e})"), &fmt::len(x), text(&k[6]));
        c.text(|| format!("fmt.clock({x:e})"), &fmt::clock(x), text(&k[9]));
    }
    for k in group("esc") {
        c.text(|| format!("esc({})", k[0]), &esc(text(&k[0])), text(&k[1]));
    }
    for k in group("csvCell") {
        c.text(
            || format!("csvCell({})", k[0]),
            &csv_cell(&k[0]),
            text(&k[1]),
        );
    }
    for k in group("csv") {
        let rows: Vec<Vec<Value>> = serde_json::from_value(k[0].clone()).unwrap();
        c.text(|| format!("csv({})", k[0]), &csv(&rows), text(&k[1]));
    }
    c.done();
}
