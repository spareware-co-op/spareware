//! JavaScript's text-to-number functions, for data read from OpenStreetMap tags and downloaded files:
//! `parseInt(s, 10)` and `parseFloat(s)`.

use crate::jsnum::{is_js_space, value_to_js_string};
use serde_json::Value;

/// `parseInt(s, 10)`: skips leading white space, reads an optional sign and as many digits as follow, ignores the rest;
/// NaN when there are no digits ("50 km/h" is 50, "2;3" is 2, "x" is NaN).
pub fn parse_int10(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_space);
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return f64::NAN;
    }
    let v: f64 = digits.parse().expect("digits parse");
    if neg { -v } else { v }
}

/// `parseFloat(s)`: skips leading white space and reads the longest decimal number at the start (an optional sign, digits with
/// an optional point, an optional exponent, or "Infinity"); NaN when there is none ("12.5m" is 12.5, ".5" is 0.5, "1e" is 1).
pub fn parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_space);
    let b = t.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    if t[i..].starts_with("Infinity") {
        return if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let start = i;
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    // An exponent counts only with at least one digit after it
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let ds = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds {
            i = j;
        }
    }
    let v: f64 = t[start..i].parse().expect("a decimal literal parses");
    if neg { -v } else { v }
}

/// `parseInt(x, 10)` of a JSON value or a missing key (`parseInt` first turns its argument into text).
pub fn parse_int10_value(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(v) => parse_int10(&value_to_js_string(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints() {
        assert_eq!(parse_int10("50"), 50.0);
        assert_eq!(parse_int10("  -3 lanes"), -3.0);
        assert_eq!(parse_int10("2;3"), 2.0);
        assert_eq!(parse_int10("0x10"), 0.0);
        assert!(parse_int10("x").is_nan());
        assert!(parse_int10("").is_nan());
        assert!(parse_int10("-").is_nan());
    }

    #[test]
    fn floats() {
        assert_eq!(parse_float("12.5m"), 12.5);
        assert_eq!(parse_float(".5"), 0.5);
        assert_eq!(parse_float("5."), 5.0);
        assert_eq!(parse_float("1e"), 1.0);
        assert_eq!(parse_float("1e+2x"), 100.0);
        assert_eq!(parse_float("-Infinityx"), f64::NEG_INFINITY);
        assert!(parse_float(".").is_nan());
        assert!(parse_float("abc").is_nan());
    }
}
