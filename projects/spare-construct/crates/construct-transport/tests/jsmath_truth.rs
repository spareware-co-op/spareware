//! The ports of V8's Math.exp, sin, cos and asin against Node's answers, bit for bit
//! (expected/jsmath-cases.json, from gen-math.mjs), and Math.pow's special cases; the general case of
//! pow is the platform's and must be within one unit in the last place of Node's.

mod support;

use construct_transport::jsmath;
use serde_json::Value;
use support::load_json;

fn bits(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().expect("hex"), 16).expect("hex digits"))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

// distance in units in the last place between two finite numbers of the same sign
fn ulps(a: f64, b: f64) -> u64 {
    a.to_bits().abs_diff(b.to_bits())
}

#[test]
fn exp_sin_cos_and_asin_match_node_bit_for_bit() {
    let cases = load_json("expected/jsmath-cases.json");
    let fns: [(&str, fn(f64) -> f64); 4] = [
        ("exp", jsmath::exp),
        ("sin", jsmath::sin),
        ("cos", jsmath::cos),
        ("asin", jsmath::asin),
    ];
    let mut n = 0;
    for (name, f) in fns {
        let list = cases[name].as_array().expect("cases");
        for c in list {
            let (x, want) = (bits(&c[0]), bits(&c[1]));
            let got = f(x);
            assert!(same(got, want), "{name}({x:e}): node {want:e}, port {got:e}");
            n += 1;
        }
    }
    assert!(n > 7000, "{n} cases");
}

#[test]
fn pow_special_cases_exact_and_the_rest_within_one_ulp() {
    let cases = load_json("expected/jsmath-cases.json");
    let (mut exact, mut near, mut off_by_one) = (0, 0, 0);
    for c in cases["pow"].as_array().expect("pow cases") {
        let (x, y, want) = (bits(&c[0]), bits(&c[1]), bits(&c[2]));
        let got = jsmath::pow(x, y);
        let special = y == 0.0 || y == 1.0 || y == 2.0 || y == 0.5 || y.is_nan() || y.is_infinite() || x == 0.0 || x.is_infinite() || x.is_nan() || x == 1.0 || x == -1.0;
        if special {
            assert!(same(got, want), "pow({x:e}, {y:e}): node {want:e}, port {got:e}");
            exact += 1;
        } else if same(got, want) {
            near += 1;
        } else {
            assert!(
                got.is_finite() == want.is_finite() && got.signum() == want.signum() && ulps(got, want) <= 1,
                "pow({x:e}, {y:e}): node {want:e}, port {got:e}"
            );
            off_by_one += 1;
        }
    }
    assert!(exact > 300 && near > 2000, "{exact} special, {near} equal");
    // about 0.03% of general arguments differ in the last bit
    assert!(off_by_one < 20, "{off_by_one} differ in the last bit");
}
