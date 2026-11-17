//! JavaScript's text-to-number functions, for data read from OpenStreetMap tags and downloaded files:
//! `parseInt(s, 10)` and `parseFloat(s)`.

use crate::jsnum::{is_js_space, split_digits, value_to_js_string};
use serde_json::Value;

/// `parseInt(s, 10)`: skips leading white space, reads an optional sign and as many digits as follow, ignores the rest;
/// NaN when there are no digits ("50 km/h" is 50, "2;3" is 2, "x" is NaN).
pub fn parse_int10(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_space);
    let (neg, rest) = split_sign(t);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    // Decimal digits always parse as an f64 (rounded, as parseInt rounds)
    let Ok(v) = digits.parse::<f64>() else {
        return f64::NAN;
    };
    if neg { -v } else { v }
}

// A leading "-" or "+" off the text: whether it was "-", and the rest
fn split_sign(t: &str) -> (bool, &str) {
    if let Some(rest) = t.strip_prefix('-') {
        (true, rest)
    } else {
        (false, t.strip_prefix('+').unwrap_or(t))
    }
}

/// `parseFloat(s)`: skips leading white space and reads the longest decimal number at the start (an optional sign, digits with
/// an optional point, an optional exponent, or "Infinity"); NaN when there is none ("12.5m" is 12.5, ".5" is 0.5, "1e" is 1).
pub fn parse_float(s: &str) -> f64 {
    let (neg, body) = split_sign(s.trim_start_matches(is_js_space));
    if body.starts_with("Infinity") {
        return if neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let (int, mut rest) = split_digits(body.as_bytes());
    let mut any_digit = !int.is_empty();
    if let Some((b'.', frac)) = rest.split_first() {
        let (frac_digits, after) = split_digits(frac);
        any_digit |= !frac_digits.is_empty();
        rest = after;
    }
    if !any_digit {
        return f64::NAN;
    }
    // An exponent counts only with at least one digit after it
    if let Some((b'e' | b'E', exp)) = rest.split_first() {
        let exp = match exp.split_first() {
            Some((b'+' | b'-', unsigned)) => unsigned,
            _ => exp,
        };
        let (exp_digits, after) = split_digits(exp);
        if !exp_digits.is_empty() {
            rest = after;
        }
    }
    // The literal is everything before rest: ASCII, so its end is a character boundary, and a
    // decimal literal always parses
    let literal = body.get(..body.len().saturating_sub(rest.len())).unwrap_or("");
    let v: f64 = literal.parse().unwrap_or(f64::NAN);
    if neg { -v } else { v }
}

/// `parseInt(x, 10)` of a JSON value or a missing key (`parseInt` first turns its argument into text).
#[must_use]
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
