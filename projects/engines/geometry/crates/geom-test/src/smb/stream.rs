//! The text stream an `SMLib` database is read through: `std::istream`'s state and extraction rules,
//! as libstdc++ implements them, for the ASCII readers of [`crate::smb`].
//!
//! What it does: [`Stream`] holds a file's bytes, a position and the stream state (good, end of
//! data, failed). Its extractions are the ones `SMLib`'s readers use, `>>` into a word, an integer
//! or a double, and `ignore`, `get` and `unget`, each with libstdc++'s rules: a sentry that skips
//! white space and fails on a stream that is not good, `num_get`'s scan of the characters, and the
//! state each leaves.
//!
//! Guarantees:
//!
//! - **An extraction that fails stores nothing.** Each returns `None` when its sentry fails, as a
//!   C++ extraction leaves its variable unchanged, so a reader keeps what it had.
//! - **Numbers as libstdc++ scans them.** An integer is the longest run of digits after an
//!   optional sign; one out of range is clamped and fails the stream. A double is `num_get`'s
//!   characters (sign, digits, one point, one exponent) read by Rust's correctly rounded parser,
//!   which gives the double `strtod` gives in the "C" locale.
//! - **Never past the data.** Every step reads within the bytes held; at their end the stream
//!   reports end of data, and a read that finds nothing fails it.
//!
//! Method: the `std::istream` rules of C++17 (ISO/IEC 14882:2017, sections 30.7.4.1 and 25.4.2.1,
//! <https://www.iso.org/standard/68564.html>), as libstdc++ implements them and solidcore's port
//! of `SMLib`'s database reader follows them.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/7a593124c98e2bdfc7adc4fa94aeeb9b758cf450/crates/solidcore/src/topology/database_io.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: the ASCII side of `InputStream` alone (no binary reads, no write side,
//!   no byte swapping), with every step a checked or saturating operation and every byte read
//!   through `get`.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: linear in the bytes read; a double's characters are collected once.

/// `std::ctype<char>::is(space, c)` in the "C" locale.
fn is_space(c: u8) -> bool {
    c == b' ' || (b'\t'..=b'\r').contains(&c)
}

/// The scan of an integer's characters (`num_get::_M_extract_int`, base 10).
struct IntScan {
    negative: bool,
    magnitude: u64,
    overflow: bool,
    digits: bool,
}

/// A byte stream with `std::istream`'s state and extraction rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    data: Vec<u8>,
    pos: usize,
    end: bool,
    failed: bool,
    bad: bool,
}

impl Stream {
    /// A stream over `data`, at its start, in the good state.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Stream {
            data,
            pos: 0,
            end: false,
            failed: false,
            bad: false,
        }
    }

    /// `good()`: no state bit set.
    #[must_use]
    pub fn good(&self) -> bool {
        !self.end && !self.failed && !self.bad
    }

    /// `eof()`.
    #[must_use]
    pub fn eof(&self) -> bool {
        self.end
    }

    /// `fail()`: the fail or the bad bit.
    #[must_use]
    pub fn fail(&self) -> bool {
        self.failed || self.bad
    }

    /// The bytes not yet read.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn step(&mut self) {
        self.pos = self.pos.saturating_add(1);
    }

    fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// `istream::sentry`: fails, setting the fail bit, unless the stream is good; with `skip`, white
    /// space is skipped first, and running out of data sets the end and fail bits.
    fn sentry(&mut self, skip: bool) -> bool {
        if self.good() && skip {
            while let Some(c) = self.peek() {
                if !is_space(c) {
                    break;
                }
                self.step();
            }
            if self.at_end() {
                self.end = true;
            }
        }
        if self.good() {
            true
        } else {
            self.failed = true;
            false
        }
    }

    /// `istream::ignore(n, delim)`: skips bytes until `n` were skipped, the data ends (end bit), or
    /// `delim` was skipped.
    pub fn ignore(&mut self, n: usize, delim: u8) {
        if !self.sentry(false) || n == 0 {
            return;
        }
        let mut count = 0usize;
        while count < n {
            let Some(c) = self.peek() else {
                self.end = true;
                break;
            };
            self.step();
            count = count.saturating_add(1);
            if c == delim {
                break;
            }
        }
    }

    /// `istream::get()`: the next byte, or `None` (end and fail bits) at the end of the data.
    pub fn get(&mut self) -> Option<u8> {
        if !self.sentry(false) {
            return None;
        }
        let c = self.peek();
        if c.is_some() {
            self.step();
        } else {
            self.end = true;
            self.failed = true;
        }
        c
    }

    /// `istream::unget()`: clears the end bit, then steps back one byte (bad bit at the start).
    pub fn unget(&mut self) {
        self.end = false;
        if !self.sentry(false) {
            return;
        }
        match self.pos.checked_sub(1) {
            Some(p) => self.pos = p,
            None => self.bad = true,
        }
    }

    /// `is >> char_array` into an array of `size` bytes: the next word of at most `size - 1` bytes,
    /// the rest left in the stream; `None` when the sentry fails, and the fail bit for an empty word.
    pub fn word(&mut self, size: usize) -> Option<Vec<u8>> {
        if !self.sentry(true) {
            return None;
        }
        let limit = size.saturating_sub(1);
        let start = self.pos;
        while self.pos.saturating_sub(start) < limit {
            match self.peek() {
                Some(c) if !is_space(c) => self.step(),
                _ => break,
            }
        }
        if self.at_end() {
            self.end = true;
        }
        let word = self.data.get(start..self.pos).unwrap_or_default().to_vec();
        if word.is_empty() {
            self.failed = true;
        }
        Some(word)
    }

    /// Scans an optional sign and digits; `max(negative)` is the largest magnitude the target takes.
    fn scan_int(&mut self, max: impl Fn(bool) -> u64) -> IntScan {
        let mut negative = false;
        if let Some(c @ (b'-' | b'+')) = self.peek() {
            negative = c == b'-';
            self.step();
        }
        let mut digits = false;
        let mut magnitude: u64 = 0;
        let mut overflow = false;
        let limit = max(negative);
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            digits = true;
            let digit = u64::from(c.saturating_sub(b'0'));
            match magnitude
                .checked_mul(10)
                .and_then(|m| m.checked_add(digit))
                .filter(|&m| m <= limit)
            {
                Some(m) => magnitude = m,
                None => overflow = true,
            }
            self.step();
        }
        IntScan {
            negative,
            magnitude,
            overflow,
            digits,
        }
    }

    /// `num_get::get(long&)` after the sentry.
    fn long_value(&mut self) -> i64 {
        let scan = self.scan_int(|negative| {
            if negative {
                i64::MIN.unsigned_abs()
            } else {
                i64::MAX.unsigned_abs()
            }
        });
        let value = if !scan.digits {
            self.failed = true;
            0
        } else if scan.overflow {
            self.failed = true;
            if scan.negative { i64::MIN } else { i64::MAX }
        } else if scan.negative {
            // At most 2^63, which -i64::MIN is.
            0_i64.checked_sub_unsigned(scan.magnitude).unwrap_or(i64::MIN)
        } else {
            i64::try_from(scan.magnitude).unwrap_or(i64::MAX)
        };
        if self.at_end() {
            self.end = true;
        }
        value
    }

    /// `is >> long`: `None` when the sentry fails; else the value, 0 with the fail bit when there are
    /// no digits, clamped with the fail bit when out of range.
    pub fn long(&mut self) -> Option<i64> {
        if !self.sentry(true) {
            return None;
        }
        Some(self.long_value())
    }

    /// `is >> unsigned long`: a leading `-` negates modulo 2^64, as libstdc++ does.
    pub fn ulong(&mut self) -> Option<u64> {
        if !self.sentry(true) {
            return None;
        }
        let scan = self.scan_int(|_| u64::MAX);
        let value = if !scan.digits {
            self.failed = true;
            0
        } else if scan.overflow {
            self.failed = true;
            u64::MAX
        } else if scan.negative {
            scan.magnitude.wrapping_neg()
        } else {
            scan.magnitude
        };
        if self.at_end() {
            self.end = true;
        }
        Some(value)
    }

    /// `is >> int`: a `long` clamped to `i32`, with the fail bit when out of range.
    pub fn int(&mut self) -> Option<i32> {
        if !self.sentry(true) {
            return None;
        }
        let l = self.long_value();
        Some(i32::try_from(l).unwrap_or_else(|_| {
            self.failed = true;
            if l < 0 { i32::MIN } else { i32::MAX }
        }))
    }

    /// `is >> short`: a `long` clamped to `i16`, with the fail bit when out of range.
    pub fn short(&mut self) -> Option<i16> {
        if !self.sentry(true) {
            return None;
        }
        let l = self.long_value();
        Some(i16::try_from(l).unwrap_or_else(|_| {
            self.failed = true;
            if l < 0 { i16::MIN } else { i16::MAX }
        }))
    }

    /// `is >> double`: `num_get`'s characters, then the "C" locale's `strtod`. 0 with the fail bit
    /// when they form no number, ±`f64::MAX` with the fail bit when it overflows.
    pub fn double(&mut self) -> Option<f64> {
        if !self.sentry(true) {
            return None;
        }
        let mut text = String::new();
        if let Some(c @ (b'+' | b'-')) = self.peek() {
            text.push(char::from(c));
            self.step();
        }
        let mut mantissa = false;
        while self.peek() == Some(b'0') {
            if !mantissa {
                text.push('0');
                mantissa = true;
            }
            self.step();
        }
        let mut point = false;
        let mut exponent = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                text.push(char::from(c));
                mantissa = true;
            } else if c == b'.' && !point && !exponent {
                text.push('.');
                point = true;
            } else if (c == b'e' || c == b'E') && !exponent && mantissa {
                text.push('e');
                exponent = true;
                self.step();
                match self.peek() {
                    Some(s @ (b'+' | b'-')) => text.push(char::from(s)),
                    Some(_) => continue,
                    None => break,
                }
            } else {
                break;
            }
            self.step();
        }
        let value = match text.parse::<f64>() {
            Ok(v) if v == f64::INFINITY => {
                self.failed = true;
                f64::MAX
            }
            Ok(v) if v == f64::NEG_INFINITY => {
                self.failed = true;
                -f64::MAX
            }
            Ok(v) => v,
            Err(_) => {
                self.failed = true;
                0.0
            }
        };
        if self.at_end() {
            self.end = true;
        }
        Some(value)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "tests: a failure stops the test"
)]
mod tests {
    use super::*;

    fn stream(text: &str) -> Stream {
        Stream::new(text.as_bytes().to_vec())
    }

    #[test]
    fn numbers_read_as_libstdcxx_reads_them() {
        let mut s = stream("  12 -3 0.0001000000000000 1e3 +7 x");
        assert_eq!(s.ulong(), Some(12));
        assert_eq!(s.long(), Some(-3));
        assert_eq!(s.double(), Some(0.0001));
        assert_eq!(s.double(), Some(1000.0));
        assert_eq!(s.int(), Some(7));
        assert!(s.good());
        // No digits: 0, and the stream fails; the next extraction stores nothing.
        assert_eq!(s.long(), Some(0));
        assert!(s.fail());
        assert_eq!(s.long(), None);
    }

    #[test]
    fn out_of_range_values_clamp_and_fail() {
        let mut s = stream("99999999999999999999 70000");
        assert_eq!(s.long(), Some(i64::MAX));
        assert!(s.fail());
        let mut s = stream("70000");
        assert_eq!(s.short(), Some(i16::MAX));
        assert!(s.fail());
        let mut s = stream("-1");
        assert_eq!(s.ulong(), Some(u64::MAX));
        let mut s = stream("-9223372036854775808");
        assert_eq!(s.long(), Some(i64::MIN));
        assert!(!s.fail());
    }

    #[test]
    fn words_lines_and_the_end() {
        let mut s = stream("Version 42\nnext line\n");
        assert_eq!(s.word(8).unwrap(), b"Version");
        assert_eq!(s.ulong(), Some(42));
        s.ignore(256, b'\n');
        assert_eq!(s.word(3).unwrap(), b"ne");
        assert_eq!(s.word(256).unwrap(), b"xt");
        s.ignore(256, b'\n');
        s.ignore(256, b'\n');
        assert!(s.eof());
        assert_eq!(s.remaining(), 0);
        let mut s = stream("\nV");
        assert_eq!(s.get(), Some(b'\n'));
        assert_eq!(s.get(), Some(b'V'));
        s.unget();
        assert_eq!(s.get(), Some(b'V'));
        assert_eq!(s.get(), None);
        assert!(s.fail() && s.eof());
    }
}
