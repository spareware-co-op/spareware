//! JavaScript number semantics, exactly as the prototype relies on them.
//!
//! The prototype reads saved projects loosely: `+x` and `Number(x)` turn text, null and
//! booleans into numbers, `+x || d` falls back on 0 and NaN, `x | 0` truncates to a 32-bit
//! integer, `x == null` treats a missing key and null alike, and `x === true` does not.
//! Its text outputs use `Number.prototype.toFixed`, `toPrecision` and `String(x)`, and its
//! formatting uses `Intl.NumberFormat`. This module reproduces each of those for finite and
//! non-finite values, so ported code gives the same numbers and the same text.
//!
//! Three types carry saved values without losing what the JavaScript can see:
//! [`Maybe`] keeps a key that is missing (undefined) apart from one that is null,
//! [`JsNum`] is any JSON value read through coercion (a number, text, a boolean, or an
//! array or object), and [`Num`] (`Maybe<JsNum>`) is the usual field type for a number,
//! flag or other scalar the engine coerces.
//! Method: the ECMAScript specification's ToNumber, StringToNumber, ToInt32, ToUint32,
//! Number::toString, Number.prototype.toFixed and toPrecision, and Math.round; V8's
//! algorithms for Math.round (ceiling, then step back) and Math.hypot (scaled, Kahan-summed);
//! ICU's behaviour for `Intl.NumberFormat` (it rounds the shortest round-trip decimal, half
//! away from zero). Every function is checked bit for bit, or character for character,
//! against Node 24 in tests/util_cases.rs (tools/transport-truth/gen-util-cases.mjs).
//!
//! Not covered: `Math.sin`, `cos`, `asin`, `atan2`, `exp`, `log` and `pow`. Rust's f64
//! methods call the platform's C library, which differs from V8's own implementations in the
//! last bit for roughly 1 to 15% of arguments (measured on Windows against Node 24).

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use serde_json::Value;

/* ---------------- white space and strings ---------------- */

/// True for the characters JavaScript's StringToNumber and `String.prototype.trim` strip (white space and line terminators).
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{A}' | '\u{B}' | '\u{C}' | '\u{D}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `String.prototype.trim`: removes JavaScript white space and line terminators from both ends.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/* ---------------- ToNumber ---------------- */

/// `Number(s)` for text: trims, reads "" as 0, accepts decimal, `0x` / `0o` / `0b` and `Infinity`, otherwise NaN.
///
/// Hexadecimal, octal and binary literals longer than 128 bits are summed in f64 and may
/// differ from JavaScript in the last bit (no saved project holds one).
pub fn string_to_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let b = t.as_bytes();
    if b.len() > 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            return parse_radix(&t[2..], radix);
        }
    }
    let (sign, body) = match b[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(body) {
        return f64::NAN;
    }
    // Rust's parser is correctly rounded, as StringToNumber is, and accepts this grammar
    t.parse::<f64>().unwrap_or(f64::NAN)
}

// StrUnsignedDecimalLiteral: digits [. digits] | . digits, then an optional exponent
fn is_decimal_literal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - frac_start;
    }
    if digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

fn parse_radix(s: &str, radix: u32) -> f64 {
    if s.is_empty() {
        return f64::NAN;
    }
    let mut exact: Option<u128> = Some(0);
    let mut approx = 0.0f64;
    for c in s.chars() {
        let Some(d) = c.to_digit(radix) else {
            return f64::NAN;
        };
        exact = exact
            .and_then(|v| v.checked_mul(radix as u128))
            .and_then(|v| v.checked_add(d as u128));
        approx = approx * radix as f64 + d as f64;
    }
    // u128 to f64 rounds to nearest, ties to even, as the specification's rounding does
    match exact {
        Some(v) => v as f64,
        None => approx,
    }
}

/// `Number(v)` for any JSON value: null 0, booleans 0 or 1, text by [`string_to_number`], arrays through their text, objects NaN.
pub fn value_to_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => bool_to_number(*b),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => string_to_number(s),
        Value::Array(_) => string_to_number(&value_to_js_string(v)),
        Value::Object(_) => f64::NAN,
    }
}

fn bool_to_number(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// `String(v)` for any JSON value: "null", "true", JavaScript number text, arrays joined with commas, objects "[object Object]".
pub fn value_to_js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_to_js_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        // Array.prototype.join writes null and undefined elements as empty text
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    value_to_js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// `!!v` for any JSON value: null, false, 0, NaN and "" are false; arrays and objects are true.
pub fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => truthy_number(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `!!x` for a number: false for 0, -0 and NaN.
pub fn truthy_number(x: f64) -> bool {
    !(x == 0.0 || x.is_nan())
}

/// `x || d` for a number already coerced (`+x || d`): 0, -0 and NaN fall through to d.
pub fn or_default(x: f64, d: f64) -> f64 {
    if truthy_number(x) { x } else { d }
}

/// `x | 0` (ToInt32): NaN and infinities give 0; otherwise truncated and wrapped modulo 2^32 into the signed range.
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let m = x.trunc().rem_euclid(4294967296.0);
    if m >= 2147483648.0 {
        (m - 4294967296.0) as i32
    } else {
        m as i32
    }
}

/// `x >>> 0` (ToUint32): NaN and infinities give 0; otherwise truncated and wrapped modulo 2^32.
pub fn to_uint32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4294967296.0) as u32
}

/* ---------------- Math ---------------- */

/// `Math.round`: the nearest integer, ties toward +Infinity; keeps -0 for -0.5 <= x <= -0 (V8's Float64Round).
pub fn math_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let c = x.ceil();
    if c - 0.5 <= x { c } else { c - 1.0 }
}

/// `Math.min(a, b)`: NaN if either is NaN, and -0 is below +0 (unlike `f64::min`).
pub fn math_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        };
    }
    if a < b { a } else { b }
}

/// `Math.max(a, b)`: NaN if either is NaN, and +0 is above -0 (unlike `f64::max`).
pub fn math_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        };
    }
    if a > b { a } else { b }
}

/// `Math.min(...xs)`: Infinity for no values.
pub fn math_min_of(xs: &[f64]) -> f64 {
    xs.iter().fold(f64::INFINITY, |m, &x| math_min(m, x))
}

/// `Math.max(...xs)`: -Infinity for no values.
pub fn math_max_of(xs: &[f64]) -> f64 {
    xs.iter().fold(f64::NEG_INFINITY, |m, &x| math_max(m, x))
}

/// `Math.hypot(...xs)` as V8 computes it: scaled by the largest value, Kahan-summed squares (not the C library's hypot).
pub fn math_hypot(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut max = 0.0f64;
    let mut one_nan = false;
    for &x in xs {
        if x.is_nan() {
            one_nan = true;
        } else if x.abs() > max {
            max = x.abs();
        }
    }
    if max == f64::INFINITY {
        return f64::INFINITY;
    }
    if one_nan {
        return f64::NAN;
    }
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;
    for &x in xs {
        let n = x.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/* ---------------- decimal digits ---------------- */

// Shortest round-trip digits of a finite x > 0, as JavaScript's Number::toString picks them: ASCII
// digits without leading or trailing zeros, and n such that x = 0.d1d2d3... times 10^n.
// Rust's shortest formatting gives the closest shortest digits, but breaks an exact tie between two
// of them upward; the specification (and ICU) take the even one, so a tie is checked and corrected
// (1642031110231315.25 is "1642031110231315.2" in JavaScript, not ".3").
fn shortest_digits(x: f64) -> (Vec<u8>, i32) {
    let s = format!("{:e}", x);
    let (mant, exp) = s.split_once('e').expect("exponent form");
    let exp: i32 = exp.parse().expect("exponent");
    let mut digits: Vec<u8> = mant.bytes().filter(|c| c.is_ascii_digit()).collect();
    strip_zeros(&mut digits);
    let n = exp + 1;
    // The last digit counts units of 10^unit; a tie means x is exactly half a unit from two candidates
    let unit = n - digits.len() as i32;
    let (exact, e10) = exact_digits(x);
    if e10 == unit - 1 && exact.last() == Some(&b'5') {
        let mut down = exact[..exact.len() - 1].to_vec();
        if down.is_empty() {
            down.push(b'0');
        }
        let up = round_digits(&exact, e10, -unit);
        let even = if down.last().is_some_and(|d| (d - b'0').is_multiple_of(2)) {
            down
        } else {
            up
        };
        let text = format!("{}e{}", ascii(&even), unit);
        if even != digits && text.parse::<f64>() == Ok(x) {
            let n2 = unit + even.len() as i32;
            let mut d2 = even;
            strip_zeros(&mut d2);
            return (d2, n2);
        }
    }
    (digits, n)
}

fn strip_zeros(d: &mut Vec<u8>) {
    while d.len() > 1 && d.last() == Some(&b'0') {
        d.pop();
    }
}

// A natural number in base 1e9 limbs, little-endian: just enough to expand a double exactly.
struct Big(Vec<u64>);

const LIMB: u64 = 1_000_000_000;

impl Big {
    fn from_u64(mut v: u64) -> Big {
        let mut l = Vec::new();
        while v > 0 {
            l.push(v % LIMB);
            v /= LIMB;
        }
        if l.is_empty() {
            l.push(0);
        }
        Big(l)
    }
    fn mul_small(&mut self, k: u64) {
        let mut carry = 0u64;
        for limb in &mut self.0 {
            let t = *limb * k + carry;
            *limb = t % LIMB;
            carry = t / LIMB;
        }
        while carry > 0 {
            self.0.push(carry % LIMB);
            carry /= LIMB;
        }
    }
    fn mul_pow2(&mut self, mut e: u32) {
        while e >= 29 {
            self.mul_small(1 << 29);
            e -= 29;
        }
        if e > 0 {
            self.mul_small(1 << e);
        }
    }
    fn mul_pow5(&mut self, mut e: u32) {
        while e >= 13 {
            self.mul_small(1_220_703_125);
            e -= 13;
        }
        if e > 0 {
            self.mul_small(5u64.pow(e));
        }
    }
    fn digits(&self) -> Vec<u8> {
        let mut s = self.0.last().copied().unwrap_or(0).to_string();
        for limb in self.0.iter().rev().skip(1) {
            s.push_str(&format!("{:09}", limb));
        }
        s.into_bytes()
    }
}

// Exact decimal expansion of a finite x > 0: digits without leading or trailing zeros, and e with x = digits times 10^e
fn exact_digits(x: f64) -> (Vec<u8>, i32) {
    let bits = x.to_bits();
    let exp_bits = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e2) = if exp_bits == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), exp_bits - 1075)
    };
    let mut big = Big::from_u64(m);
    let mut e10 = 0;
    if e2 >= 0 {
        big.mul_pow2(e2 as u32);
    } else {
        // m times 2^e2 = m times 5^-e2 times 10^e2
        big.mul_pow5((-e2) as u32);
        e10 = e2;
    }
    let mut d = big.digits();
    while d.len() > 1 && d.last() == Some(&b'0') {
        d.pop();
        e10 += 1;
    }
    (d, e10)
}

// The whole number nearest digits times 10^(exp + frac), a tie rounding up (the value is a magnitude,
// so this is round half away from zero): its decimal digits, without leading zeros ("0" for zero).
fn round_digits(digits: &[u8], exp: i32, frac: i32) -> Vec<u8> {
    let shift = exp as i64 + frac as i64;
    let mut v: Vec<u8>;
    if shift >= 0 {
        v = digits.to_vec();
        v.extend(std::iter::repeat_n(b'0', shift as usize));
    } else {
        let drop = (-shift) as usize;
        if drop > digits.len() {
            return vec![b'0'];
        }
        let keep = digits.len() - drop;
        v = digits[..keep].to_vec();
        if v.is_empty() {
            v.push(b'0');
        }
        if digits[keep] >= b'5' {
            let mut i = v.len();
            loop {
                if i == 0 {
                    v.insert(0, b'1');
                    break;
                }
                i -= 1;
                if v[i] == b'9' {
                    v[i] = b'0';
                } else {
                    v[i] += 1;
                    break;
                }
            }
        }
    }
    let first = v.iter().position(|&c| c != b'0').unwrap_or(v.len() - 1);
    v.drain(..first);
    v
}

fn ascii(v: &[u8]) -> &str {
    std::str::from_utf8(v).expect("ASCII digits")
}

/* ---------------- number to text ---------------- */

/// `String(x)` for a number (Number::toString): shortest round-trip digits, exponent form below 1e-6 and from 1e21.
pub fn number_to_js_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x < 0.0 {
        return format!("-{}", number_to_js_string(-x));
    }
    if x.is_infinite() {
        return "Infinity".to_string();
    }
    let (d, n) = shortest_digits(x);
    let k = d.len() as i32;
    let s = ascii(&d);
    if k <= n && n <= 21 {
        return format!("{}{}", s, "0".repeat((n - k) as usize));
    }
    if 0 < n && n <= 21 {
        return format!("{}.{}", &s[..n as usize], &s[n as usize..]);
    }
    if -6 < n && n <= 0 {
        return format!("0.{}{}", "0".repeat((-n) as usize), s);
    }
    let e = n - 1;
    let es = if e >= 0 {
        format!("+{}", e)
    } else {
        format!("-{}", -e)
    };
    if k == 1 {
        format!("{}e{}", s, es)
    } else {
        format!("{}.{}e{}", &s[..1], &s[1..], es)
    }
}

/// `x.toFixed(digits)` (digits 0 to 100): exact decimal value rounded half up; from 1e21 the `String(x)` form.
pub fn to_fixed(x: f64, digits: usize) -> String {
    assert!(digits <= 100, "toFixed digits must be 0 to 100");
    if !x.is_finite() {
        return number_to_js_string(x);
    }
    // -0 is not below 0, so it prints without a sign; -0.001 rounds to "-0.00"
    let (sign, a) = if x < 0.0 { ("-", -x) } else { ("", x) };
    if a >= 1e21 {
        return format!("{}{}", sign, number_to_js_string(a));
    }
    let n = if a == 0.0 {
        vec![b'0']
    } else {
        let (d, e) = exact_digits(a);
        round_digits(&d, e, digits as i32)
    };
    format!("{}{}", sign, with_point(n, digits))
}

// Digits of a whole number n read as n / 10^frac: a point before the last frac digits, zeros padded in front
fn with_point(mut n: Vec<u8>, frac: usize) -> String {
    if frac == 0 {
        return ascii(&n).to_string();
    }
    if n.len() <= frac {
        let mut z = vec![b'0'; frac + 1 - n.len()];
        z.extend_from_slice(&n);
        n = z;
    }
    let k = n.len() - frac;
    format!("{}.{}", ascii(&n[..k]), ascii(&n[k..]))
}

/// `x.toPrecision(p)` (p 1 to 100): exact decimal value rounded half up to p significant digits.
pub fn to_precision(x: f64, p: usize) -> String {
    assert!(
        (1..=100).contains(&p),
        "toPrecision precision must be 1 to 100"
    );
    if !x.is_finite() {
        return number_to_js_string(x);
    }
    let (sign, a) = if x < 0.0 { ("-", -x) } else { ("", x) };
    let (m, e): (Vec<u8>, i32) = if a == 0.0 {
        (vec![b'0'; p], 0)
    } else {
        let (d, de) = exact_digits(a);
        let e0 = d.len() as i32 - 1 + de;
        let n = round_digits(&d, de, p as i32 - 1 - e0);
        if n.len() > p {
            (n[..p].to_vec(), e0 + 1)
        } else {
            (n, e0)
        }
    };
    let s = ascii(&m);
    let pi = p as i32;
    if e < -6 || e >= pi {
        let mut out = format!("{}{}", sign, &s[..1]);
        if p != 1 {
            out.push('.');
            out.push_str(&s[1..]);
        }
        out.push('e');
        out.push_str(&if e >= 0 {
            format!("+{}", e)
        } else {
            format!("-{}", -e)
        });
        return out;
    }
    if e == pi - 1 {
        return format!("{}{}", sign, s);
    }
    if e >= 0 {
        let k = (e + 1) as usize;
        return format!("{}{}.{}", sign, &s[..k], &s[k..]);
    }
    format!("{}0.{}{}", sign, "0".repeat((-(e + 1)) as usize), s)
}

/// `new Intl.NumberFormat('en-NZ', { minimumFractionDigits: d, maximumFractionDigits: d }).format(x)` for finite x.
///
/// ICU rounds the shortest round-trip decimal (so 1.005 gives "1.01", where toFixed gives
/// "1.00"), half away from zero; thousands are grouped with commas; the minus sign is a
/// hyphen-minus and is kept for -0 and for values that round to zero ("-0", "-0.0").
pub fn intl_format(x: f64, d: usize) -> String {
    let neg = x.is_sign_negative();
    let a = x.abs();
    let n = if a == 0.0 {
        vec![b'0']
    } else {
        let (digits, pos) = shortest_digits(a);
        // a = 0.digits times 10^pos = digits times 10^(pos - len)
        round_digits(&digits, pos - digits.len() as i32, d as i32)
    };
    let text = with_point(n, d);
    let (int, frac) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text.as_str(), None),
    };
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/* ---------------- numbers as JSON ---------------- */

/// A number as `JSON.stringify` writes it: integers as integers, -0 as 0, NaN and infinities as null.
pub fn json_number(x: f64) -> Value {
    if !x.is_finite() {
        return Value::Null;
    }
    if x.fract() == 0.0 {
        if (-9223372036854775808.0..9223372036854775808.0).contains(&x) {
            return Value::from(x as i64);
        }
        if (0.0..18446744073709551616.0).contains(&x) {
            return Value::from(x as u64);
        }
    }
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// A number as tools/transport-truth/gen-expected.mjs writes results: finite as [`json_number`], otherwise "NaN", "Infinity" or "-Infinity".
pub fn plain_number(x: f64) -> Value {
    if x.is_finite() {
        json_number(x)
    } else {
        Value::String(number_to_js_string(x))
    }
}

/* ---------------- saved values ---------------- */

/// A saved key that may be missing (undefined), null, or set: the JavaScript `x === undefined` / `x === null` distinction.
///
/// Struct fields of this type carry `#[serde(default, skip_serializing_if = "Maybe::is_missing")]`,
/// so a missing key stays missing and an explicit null stays null when a project is written back.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Maybe<T> {
    /// The key is not there (`undefined`).
    #[default]
    Missing,
    /// The key holds `null`.
    Null,
    /// The key holds a value.
    Set(T),
}

impl<T> Maybe<T> {
    /// True when the key is missing (used to skip it when writing).
    pub fn is_missing(&self) -> bool {
        matches!(self, Maybe::Missing)
    }
    /// True when the key holds null.
    pub fn is_null(&self) -> bool {
        matches!(self, Maybe::Null)
    }
    /// `x == null`: true when missing or null.
    pub fn is_nullish(&self) -> bool {
        !matches!(self, Maybe::Set(_))
    }
    /// True when the key holds a value.
    pub fn is_set(&self) -> bool {
        matches!(self, Maybe::Set(_))
    }
    /// The value, if set.
    pub fn get(&self) -> Option<&T> {
        match self {
            Maybe::Set(v) => Some(v),
            _ => None,
        }
    }
    /// The value for changing, if set.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            Maybe::Set(v) => Some(v),
            _ => None,
        }
    }
    /// The value, if set, by value.
    pub fn into_option(self) -> Option<T> {
        match self {
            Maybe::Set(v) => Some(v),
            _ => None,
        }
    }
}

impl<T> From<T> for Maybe<T> {
    fn from(v: T) -> Self {
        Maybe::Set(v)
    }
}

impl Maybe<String> {
    /// The text, if set.
    pub fn as_str(&self) -> Option<&str> {
        self.get().map(String::as_str)
    }
    /// `x || d` for text: missing, null and "" give d.
    pub fn str_or<'a>(&'a self, d: &'a str) -> &'a str {
        match self.as_str() {
            Some(s) if !s.is_empty() => s,
            _ => d,
        }
    }
    /// `x === s`: true only when set to exactly s.
    pub fn is(&self, s: &str) -> bool {
        self.as_str() == Some(s)
    }
}

impl<T: Serialize> Serialize for Maybe<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Maybe::Missing => s.serialize_none(),
            Maybe::Null => s.serialize_unit(),
            Maybe::Set(v) => v.serialize(s),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Maybe<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // A key that is present reaches here; JSON null becomes Null, anything else the value
        Ok(match Option::<T>::deserialize(d)? {
            None => Maybe::Null,
            Some(v) => Maybe::Set(v),
        })
    }
}

/// Any saved JSON value that the JavaScript reads through coercion: a number, text, a boolean, or an array or object.
///
/// It keeps what the file held (so a project writes back unchanged) and gives the
/// JavaScript reading of it: [`JsNum::to_number`] is `Number(x)`, [`JsNum::truthy`] is `!!x`.
/// Null is not a `JsNum`: a field holds `Maybe::Null` instead (see [`Num`]).
#[derive(Clone, Debug, PartialEq)]
pub enum JsNum {
    /// A JSON number.
    Number(f64),
    /// Text, read by `Number(x)` as [`string_to_number`] does.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// An array or object (or null when deserialized outside a [`Maybe`]).
    Other(Value),
}

impl JsNum {
    /// `Number(x)` / `+x`.
    pub fn to_number(&self) -> f64 {
        match self {
            JsNum::Number(x) => *x,
            JsNum::Text(s) => string_to_number(s),
            JsNum::Bool(b) => bool_to_number(*b),
            JsNum::Other(v) => value_to_number(v),
        }
    }
    /// `!!x`.
    pub fn truthy(&self) -> bool {
        match self {
            JsNum::Number(x) => truthy_number(*x),
            JsNum::Text(s) => !s.is_empty(),
            JsNum::Bool(b) => *b,
            JsNum::Other(v) => value_truthy(v),
        }
    }
    /// `String(x)`.
    pub fn to_js_string(&self) -> String {
        match self {
            JsNum::Number(x) => number_to_js_string(*x),
            JsNum::Text(s) => s.clone(),
            JsNum::Bool(b) => b.to_string(),
            JsNum::Other(v) => value_to_js_string(v),
        }
    }
    /// The text, when the value is text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsNum::Text(s) => Some(s),
            _ => None,
        }
    }
    /// The value as JSON, as the file held it.
    pub fn to_value(&self) -> Value {
        match self {
            JsNum::Number(x) => json_number(*x),
            JsNum::Text(s) => Value::String(s.clone()),
            JsNum::Bool(b) => Value::Bool(*b),
            JsNum::Other(v) => v.clone(),
        }
    }
}

impl From<f64> for JsNum {
    fn from(x: f64) -> Self {
        JsNum::Number(x)
    }
}

impl From<bool> for JsNum {
    fn from(b: bool) -> Self {
        JsNum::Bool(b)
    }
}

impl From<&str> for JsNum {
    fn from(s: &str) -> Self {
        JsNum::Text(s.to_string())
    }
}

impl From<Value> for JsNum {
    fn from(v: Value) -> Self {
        match v {
            Value::Number(n) => JsNum::Number(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(s) => JsNum::Text(s),
            Value::Bool(b) => JsNum::Bool(b),
            other => JsNum::Other(other),
        }
    }
}

impl Serialize for JsNum {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            JsNum::Number(x) => json_number(*x).serialize(s),
            JsNum::Text(t) => s.serialize_str(t),
            JsNum::Bool(b) => s.serialize_bool(*b),
            JsNum::Other(v) => v.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for JsNum {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(JsNum::from(Value::deserialize(d)?))
    }
}

/// The usual field type for a saved number, flag or other scalar: missing, null, or a [`JsNum`].
pub type Num = Maybe<JsNum>;

impl Maybe<JsNum> {
    /// `+x` / `Number(x)`: missing gives NaN, null gives 0.
    pub fn num(&self) -> f64 {
        match self {
            Maybe::Missing => f64::NAN,
            Maybe::Null => 0.0,
            Maybe::Set(v) => v.to_number(),
        }
    }
    /// `+x || d`: 0 and NaN (so missing, null, "" and false too) give d.
    pub fn or(&self, d: f64) -> f64 {
        or_default(self.num(), d)
    }
    /// `x | 0`.
    pub fn int32(&self) -> i32 {
        to_int32(self.num())
    }
    /// `!!x`.
    pub fn truthy(&self) -> bool {
        match self {
            Maybe::Set(v) => v.truthy(),
            _ => false,
        }
    }
    /// `x === true`.
    pub fn is_true(&self) -> bool {
        matches!(self, Maybe::Set(JsNum::Bool(true)))
    }
    /// `x === false`.
    pub fn is_false(&self) -> bool {
        matches!(self, Maybe::Set(JsNum::Bool(false)))
    }
    /// `String(x)`: "undefined" when missing, "null" when null.
    pub fn js_string(&self) -> String {
        match self {
            Maybe::Missing => "undefined".to_string(),
            Maybe::Null => "null".to_string(),
            Maybe::Set(v) => v.to_js_string(),
        }
    }
    /// A number field set to x.
    pub fn number(x: f64) -> Self {
        Maybe::Set(JsNum::Number(x))
    }
}

impl From<f64> for Maybe<JsNum> {
    fn from(x: f64) -> Self {
        Maybe::Set(JsNum::Number(x))
    }
}

impl From<bool> for Maybe<JsNum> {
    fn from(b: bool) -> Self {
        Maybe::Set(JsNum::Bool(b))
    }
}

/// A missing [`Num`], for lookups that find nothing (`obj[k]` of an absent key).
pub static MISSING: Num = Maybe::Missing;
