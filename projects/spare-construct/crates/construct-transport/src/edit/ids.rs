//! Ids and the clock: `U.uid` and `new Date()` of the prototype, with the random source and the time injected.
//!
//! `U.uid(prefix)` is `prefix + Date.now().toString(36).slice(-4) + (seq++).toString(36) +
//! Math.random().toString(36).slice(2, 5)`. [`Ids`] does the same with any [`Rng`] (the
//! truth-test harness's seeded generator, or a closure over the browser's `Math.random`) and a
//! clock given in milliseconds, so one seed gives the same ids as the JavaScript.

use crate::jsnum::{f64_to_i64, f64_to_usize, u64_to_f64, usize_to_f64};
use engine_sim::Rng;

/// Where a model operation gets new ids and the time: `U.uid(prefix)` and `new Date()`.
pub trait IdSource {
    /// `U.uid(prefix)`: a new id such as "nidc05pnh".
    fn uid(&mut self, prefix: &str) -> String;
    /// `Date.now()`: milliseconds since 1970-01-01 UTC.
    fn now_ms(&self) -> f64;
    /// `new Date().toISOString()`.
    fn now_iso(&self) -> String {
        iso_string(self.now_ms())
    }
    /// `new Date().toISOString().slice(0, 10)`: today's date as YYYY-MM-DD.
    fn today(&self) -> String {
        iso_string(self.now_ms()).chars().take(10).collect()
    }
}

/// The prototype's id generator over an injected random source and clock.
#[derive(Clone, Debug)]
pub struct Ids<R: Rng> {
    rng: R,
    now_ms: f64,
    seq: u64,
}

impl<R: Rng> Ids<R> {
    /// A generator with the counter at 0, as util.js starts it.
    pub fn new(rng: R, now_ms: f64) -> Self {
        Ids {
            rng,
            now_ms,
            seq: 0,
        }
    }
    /// Sets the counter (the next id uses this number).
    #[must_use]
    pub fn with_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }
    /// Moves the clock.
    pub fn set_now_ms(&mut self, now_ms: f64) {
        self.now_ms = now_ms;
    }
    /// The counter's next value.
    pub fn seq(&self) -> u64 {
        self.seq
    }
    /// The random source, for the caller to keep drawing from it.
    pub fn rng_mut(&mut self) -> &mut R {
        &mut self.rng
    }
}

impl<R: Rng> IdSource for Ids<R> {
    fn uid(&mut self, prefix: &str) -> String {
        let t = integer_radix36(self.now_ms);
        let t: Vec<char> = t.chars().collect();
        let tail: String = t.iter().skip(t.len().saturating_sub(4)).collect();
        let n = integer_radix36(u64_to_f64(self.seq));
        // Bound: one step per id made, far below u64::MAX
        self.seq = self.seq.saturating_add(1);
        let r = self.rng.next_f64();
        let s = number_to_radix36(r);
        let rs: String = s.chars().skip(2).take(3).collect();
        format!("{prefix}{tail}{n}{rs}")
    }
    fn now_ms(&self) -> f64 {
        self.now_ms
    }
}

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

// The base-36 digit for d; every caller passes d below 36, so the fallback is never used
fn radix_digit(d: usize) -> u8 {
    DIGITS.get(d).copied().unwrap_or(b'0')
}

/// `x.toString(36)` for a whole number below 2^53 (a negative number gets a minus sign).
#[must_use]
pub fn integer_radix36(x: f64) -> String {
    let neg = x < 0.0;
    // Below 2^53 (as documented) the conversion is exact; NaN gives 0, as `as u64` did
    let mut n = f64_to_i64(x.abs().floor()).unsigned_abs();
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(radix_digit(usize::try_from(n % 36).unwrap_or(0)));
        n /= 36;
    }
    if neg {
        out.push(b'-');
    }
    out.reverse();
    // Every byte is an ASCII digit, letter or '-', so each is its own char
    out.into_iter().map(char::from).collect()
}

/// `x.toString(36)` for a finite number with a fractional part (V8's `DoubleToRadixCString`): the digits of the
/// fraction run until the remaining fraction is below half the gap to the next double, with round-half-even carry.
///
/// The whole part must be below 2^53. NaN, infinities and 0 give "NaN", "Infinity", "-Infinity" and "0".
#[must_use]
pub fn number_to_radix36(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let negative = x < 0.0;
    let value = x.abs();
    let mut integer = value.floor();
    let mut fraction = value - integer;
    // The gap to the next double, halved (at least the smallest denormal)
    // value is finite, so its bits are below those of Infinity and the step never saturates
    let next = f64::from_bits(value.to_bits().saturating_add(1));
    let mut delta = 0.5 * (next - value);
    delta = delta.max(f64::from_bits(1));
    let mut frac_digits: Vec<u8> = Vec::new();
    if fraction >= delta {
        loop {
            fraction *= 36.0;
            delta *= 36.0;
            // fraction is in 0..36 here, so the digit fits u8
            let digit = f64_to_usize(fraction);
            frac_digits.push(u8::try_from(digit).unwrap_or(u8::MAX));
            fraction -= usize_to_f64(digit);
            // Round to even
            if (fraction > 0.5 || (fraction == 0.5 && (digit & 1) == 1)) && fraction + delta > 1.0 {
                // Carry into the digits already written
                loop {
                    match frac_digits.pop() {
                        None => {
                            integer += 1.0;
                            break;
                        }
                        Some(d) => {
                            // d + 1 < 36
                            if d < 35 {
                                frac_digits.push(d.saturating_add(1));
                                break;
                            }
                        }
                    }
                }
                break;
            }
            if fraction < delta {
                break;
            }
        }
    }
    let mut s = String::new();
    if negative {
        s.push('-');
    }
    s.push_str(&integer_radix36(integer));
    if !frac_digits.is_empty() {
        s.push('.');
        for d in frac_digits {
            s.push(char::from(radix_digit(usize::from(d))));
        }
    }
    s
}

/// `new Date(ms).toISOString()`: "2026-10-21T00:00:00.000Z" (years outside 0 to 9999 get a sign and six digits).
#[must_use]
pub fn iso_string(ms: f64) -> String {
    // A Date holds whole milliseconds: the time is cut toward zero (TimeClip)
    let ms = ms.trunc();
    let day_ms = 86_400_000.0;
    let days = (ms / day_ms).floor();
    let rem = ms - days * day_ms;
    let (yr, mon, day) = civil_from_days(f64_to_i64(days));
    let hr = (rem / 3_600_000.0).floor();
    let mi = ((rem - hr * 3_600_000.0) / 60_000.0).floor();
    let sec = ((rem - hr * 3_600_000.0 - mi * 60_000.0) / 1000.0).floor();
    let milli = rem - hr * 3_600_000.0 - mi * 60_000.0 - sec * 1000.0;
    let year = if (0..=9999).contains(&yr) {
        format!("{yr:04}")
    } else if yr < 0 {
        format!("-{:06}", yr.unsigned_abs())
    } else {
        format!("+{yr:06}")
    };
    format!("{year}-{mon:02}-{day:02}T{hr:02}:{mi:02}:{sec:02}.{milli:03}Z")
}

/// Days since 1970-01-01 to (year, month 1-12, day 1-31), proleptic Gregorian (Howard Hinnant's algorithm).
// Bound: a Date's day count is within 10^8 of 0 (TimeClip), so no step comes near i64's range and
// the saturating forms never saturate; doe is in 0..146097, yoe in 0..400, doy in 0..366 and mp in 0..12,
// so the day (1 to 31) and month (1 to 12) always fit u32.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = doe
        .saturating_sub(doe / 1460)
        .saturating_add(doe / 36_524)
        .saturating_sub(doe / 146_096)
        / 365;
    let y = yoe.saturating_add(era.saturating_mul(400));
    let doy = doe.saturating_sub(
        yoe.saturating_mul(365)
            .saturating_add(yoe / 4)
            .saturating_sub(yoe / 100),
    );
    let mp = doy.saturating_mul(5).saturating_add(2) / 153;
    let d = u32::try_from(
        doy.saturating_sub(mp.saturating_mul(153).saturating_add(2) / 5)
            .saturating_add(1),
    )
    .unwrap_or(1);
    let m = u32::try_from(if mp < 10 {
        mp.saturating_add(3)
    } else {
        mp.saturating_sub(9)
    })
    .unwrap_or(1);
    (if m <= 2 { y.saturating_add(1) } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(iso_string(0.0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_string(1_792_540_800_000.0), "2026-10-21T00:00:00.000Z");
        assert_eq!(
            iso_string(951_782_400_000.0 + 86_399_999.0),
            "2000-02-29T23:59:59.999Z"
        );
        assert_eq!(iso_string(-1.0), "1969-12-31T23:59:59.999Z");
    }

    #[test]
    fn radix() {
        assert_eq!(integer_radix36(35.0), "z");
        assert_eq!(integer_radix36(36.0), "10");
        assert_eq!(number_to_radix36(0.5), "0.i");
    }
}
