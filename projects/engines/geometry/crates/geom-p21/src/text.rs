//! Part 21's strings and reals as text: the string escapes decoded and written, and a double as
//! its shortest decimal.
//!
//! What it does: [`decode`] turns the bytes between a string's quotes into Unicode text, reading
//! the control directives `\\`, `\X\hh`, `\X2\...\X0\`, `\X4\...\X0\` and `\S\c`. [`encode`]
//! writes text back as a string every reader takes: printable ASCII as it is, a quote doubled, a
//! backslash doubled, and every other character in an `\X2\` or `\X4\` run. [`real`] writes a
//! finite double as Part 21 writes a real: always with a decimal point, the exponent after `E`.
//!
//! Guarantees:
//!
//! - **Decoding what was encoded gives the text back**, for every string, control characters
//!   included.
//! - **A backslash that opens no whole directive is itself.** So a Windows path written without
//!   doubled backslashes reads as it looks. A run that has its `\X0\` but whose digits are not
//!   whole groups naming Unicode characters is refused, or, when the caller asks for lossy text,
//!   each bad group becomes U+FFFD and is counted; the same holds for bytes that are not UTF-8.
//! - **A real reads back to the same bits.** [`real`] writes the shortest decimal that rounds to
//!   the double, and Rust's parser rounds correctly.
//! - **Linear time**, however many unterminated runs a string holds.
//!
//! Method: the string and real encodings of ISO 10303-21
//! (<https://www.iso.org/standard/63141.html>); the shortest round-trip decimal that chapter
//! 24.10 of `projects/engines/geometry/architecture/24-file-format-exchange.md` asks for is the
//! one Rust's `core::fmt` prints for a double (Grisu with a Dragon fallback, the same digits as
//! Ryū, Adams 2018, <https://doi.org/10.1145/3192366.3192369>).
//!
//! Cost: each function is linear in its input and allocates only by growing its output.
//!
//! Source: PORT from solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-record/src/step/p21.rs>
//!   <https://github.com/tangency-labs/solidcore/blob/a3a8244566081e0777284dc182f77a91bb1a456d/crates/solidcore-step/src/write.rs>
//!   Copyright (c) 2026 The solidcore contributors
//!   Changed for Spare Kit: `decode_string_with`, `string` and `real` in `no_std` Rust, writing
//!   into the caller's buffer. Decoding kept its one remembered search for a run's end. Hex
//!   digits are read as digits only (`from_str_radix` took a sign, so `\X\+A` read as a line
//!   feed); a bad group in a terminated run, an unpaired surrogate and bytes that are not UTF-8
//!   are a typed error, or counted replacements, where they were dropped or became `?` in
//!   silence. The writer keeps control characters, in `\X2\` runs, where they were dropped, joins
//!   neighbouring characters into one run, and refuses a real that is not finite where `0.` was
//!   written.
//! SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// A string refused: bytes that are not UTF-8, or a run that names no Unicode text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BadText;

/// The value of one hexadecimal digit.
fn digit(c: u8) -> Option<u32> {
    char::from(c).to_digit(16)
}

/// The value of `group` read as hexadecimal digits; `None` unless it is `width` digits, at most 8.
fn hex(group: &[u8], width: usize) -> Option<u32> {
    if group.len() != width {
        return None;
    }
    let mut value = 0_u32;
    for &c in group {
        value = value.checked_mul(16)?.checked_add(digit(c)?)?;
    }
    Some(value)
}

/// Where the first `\X0\` of `rest` starts.
fn find_end(rest: &[u8]) -> Option<usize> {
    rest.windows(4).position(|w| w == b"\\X0\\")
}

/// Decodes the bytes between a string's quotes onto the end of `out`, and says how many
/// replacement characters lossy decoding put in.
///
/// # Errors
///
/// [`BadText`], only when `lossy` is false; `out` may then hold part of the string.
pub(crate) fn decode(raw: &[u8], out: &mut String, lossy: bool) -> Result<u64, BadText> {
    decode_with(raw, out, lossy, find_end)
}

/// One `\X2\` or `\X4\` run's digits, `width` to a character or UTF-16 code unit, onto `out`.
fn run(body: &[u8], width: usize, out: &mut String, lossy: bool) -> Result<u64, BadText> {
    let mut replaced = 0_u64;
    let mut replace = |out: &mut String| {
        replaced = replaced.saturating_add(1);
        out.push(char::REPLACEMENT_CHARACTER);
    };
    if width == 4 {
        // UTF-16: a high surrogate waits for the low one that must follow it. A group that is
        // not four digits pairs with nothing.
        let mut high: Option<u32> = None;
        for g in body.chunks(4) {
            let unit = hex(g, 4);
            let low = unit.filter(|u| (0xDC00..0xE000).contains(u));
            if let (Some(h), Some(l)) = (high, low) {
                high = None;
                // Unicode's pairing, 0x10000 + (h - 0xD800) * 0x400 + (l - 0xDC00): both
                // differences are below 2^10, so the sum is a character above U+FFFF.
                let code = (h.wrapping_sub(0xD800) << 10)
                    .wrapping_add(l.wrapping_sub(0xDC00))
                    .wrapping_add(0x1_0000);
                out.push(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
                continue;
            }
            if high.take().is_some() {
                replace(out);
            }
            match unit {
                Some(u) if (0xD800..0xDC00).contains(&u) => high = Some(u),
                Some(u) => match char::from_u32(u) {
                    Some(c) => out.push(c),
                    None => replace(out),
                },
                None => replace(out),
            }
        }
        if high.is_some() {
            replace(out);
        }
    } else {
        for g in body.chunks(8) {
            match hex(g, 8).and_then(char::from_u32) {
                Some(c) => out.push(c),
                None => replace(out),
            }
        }
    }
    if replaced > 0 && !lossy {
        return Err(BadText);
    }
    Ok(replaced)
}

/// [`decode`] with the search for a run's `\X0\` given, so a test can count its steps.
/// `find_end(rest)` is the offset of the first `\X0\` in `rest`.
///
/// A `\X2\` or `\X4\` with no `\X0\` after it is kept as written. A search that finds a
/// terminator ends the run that asked, and decoding goes on past it, so the next search starts
/// beyond everything this one compared. A search that finds none is remembered: none follows any
/// later position either. So the searches cover disjoint stretches of `raw`, and decoding is
/// linear however many unterminated runs the string holds.
pub(crate) fn decode_with(
    raw: &[u8],
    out: &mut String,
    lossy: bool,
    mut find_end: impl FnMut(&[u8]) -> Option<usize>,
) -> Result<u64, BadText> {
    let mut replaced = 0_u64;
    let mut i = 0_usize;
    // A search found no `\X0\` from some position to the end.
    let mut no_end = false;
    // Each pass takes one byte at least, so `raw` is decoded inside this bound; it also ends a
    // pass that would take none, which would otherwise repeat for ever.
    for _ in 0..=raw.len() {
        let Some(rest) = raw.get(i..).filter(|r| !r.is_empty()) else {
            break;
        };
        if !rest.starts_with(b"\\") {
            // Plain bytes up to the next backslash. No byte of a UTF-8 character is a backslash,
            // so a character is never split.
            let n = rest.iter().position(|c| *c == b'\\').unwrap_or(rest.len());
            let plain = rest.get(..n).unwrap_or(&[]);
            for chunk in plain.utf8_chunks() {
                out.push_str(chunk.valid());
                if !chunk.invalid().is_empty() {
                    if !lossy {
                        return Err(BadText);
                    }
                    replaced = replaced.saturating_add(1);
                    out.push(char::REPLACEMENT_CHARACTER);
                }
            }
            i = i.saturating_add(n);
            continue;
        }
        if rest.starts_with(b"\\\\") {
            out.push('\\');
            i = i.saturating_add(2);
            continue;
        }
        if rest.starts_with(b"\\X\\")
            && let Some(c) = rest
                .get(3..5)
                .and_then(|g| hex(g, 2))
                .and_then(char::from_u32)
        {
            out.push(c);
            i = i.saturating_add(5);
            continue;
        }
        let width = if rest.starts_with(b"\\X2\\") {
            Some(4_usize)
        } else if rest.starts_with(b"\\X4\\") {
            Some(8)
        } else {
            None
        };
        if let Some(width) = width {
            let found = if no_end {
                None
            } else {
                let found = find_end(rest).and_then(|end| i.checked_add(end));
                no_end = found.is_none();
                found
            };
            // The terminator starts at least three bytes on: only the tag's own closing backslash
            // can open it that early, in a run written `\X2\X0\`, which holds no digits.
            if let Some(end) = found.and_then(|end| end.checked_sub(i)) {
                let body = rest.get(4..end).unwrap_or(&[]);
                replaced = replaced.saturating_add(run(body, width, out, lossy)?);
                // A run may be followed straight away by the backslash of the next escape.
                i = i.saturating_add(end).saturating_add(4);
                continue;
            }
        }
        // `\S\` takes a basic-alphabet character (space to `~`); before any other byte, the
        // start of a UTF-8 character, adding 128 would split it, so the escape is kept as
        // written.
        if rest.starts_with(b"\\S\\")
            && let Some(c) = rest
                .get(3)
                .filter(|c| matches!(c, b' '..=b'~'))
                .and_then(|c| char::from_u32(u32::from(*c).saturating_add(128)))
        {
            out.push(c);
            i = i.saturating_add(4);
            continue;
        }
        out.push('\\');
        i = i.saturating_add(1);
    }
    Ok(replaced)
}

/// Which run the writer is inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    None,
    X2,
    X4,
}

/// Upper-case hexadecimal digits.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// The lowest `digits` hexadecimal digits of `value`, most significant first.
fn push_hex(out: &mut Vec<u8>, value: u32, digits: u32) {
    for place in (0..digits).rev() {
        let nibble = value.checked_shr(place.saturating_mul(4)).unwrap_or(0) & 0xF;
        let d = usize::try_from(nibble)
            .ok()
            .and_then(|n| HEX.get(n))
            .copied()
            .unwrap_or(b'0');
        out.push(d);
    }
}

/// Writes `s` as a string literal, quotes included, onto the end of `out`.
pub(crate) fn encode(s: &str, out: &mut Vec<u8>) {
    out.push(b'\'');
    let mut state = Run::None;
    for c in s.chars() {
        let code = u32::from(c);
        let want = if (0x20..0x7F).contains(&code) {
            Run::None
        } else if code <= 0xFFFF {
            Run::X2
        } else {
            Run::X4
        };
        if want != state {
            if state != Run::None {
                out.extend_from_slice(b"\\X0\\");
            }
            match want {
                Run::None => {}
                Run::X2 => out.extend_from_slice(b"\\X2\\"),
                Run::X4 => out.extend_from_slice(b"\\X4\\"),
            }
            state = want;
        }
        match want {
            Run::None => match c {
                '\'' => out.extend_from_slice(b"''"),
                '\\' => out.extend_from_slice(b"\\\\"),
                _ => {
                    let mut utf8 = [0_u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
                }
            },
            Run::X2 => push_hex(out, code, 4),
            Run::X4 => push_hex(out, code, 8),
        }
    }
    if state != Run::None {
        out.extend_from_slice(b"\\X0\\");
    }
    out.push(b'\'');
}

/// Writes the finite double `x` as Part 21 writes a real onto the end of `out`: always with a
/// decimal point, the exponent after `E`. `scratch` is working space, emptied first.
///
/// Returns false, writing nothing, when `x` is not finite.
pub(crate) fn real(x: f64, scratch: &mut String, out: &mut Vec<u8>) -> bool {
    if !x.is_finite() {
        return false;
    }
    scratch.clear();
    // Writing into a `String` cannot fail.
    let _ = write!(scratch, "{x:?}");
    if let Some((mantissa, exponent)) = scratch.split_once('e') {
        out.extend_from_slice(mantissa.as_bytes());
        if !mantissa.contains('.') {
            out.push(b'.');
        }
        out.push(b'E');
        out.extend_from_slice(exponent.as_bytes());
    } else {
        out.extend_from_slice(scratch.as_bytes());
        if !scratch.contains('.') {
            out.push(b'.');
        }
    }
    true
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "tests: a failure stops the test"
)]
mod tests {
    use super::*;

    fn strict(raw: &[u8]) -> Result<String, BadText> {
        let mut out = String::new();
        decode(raw, &mut out, false).map(|_| out)
    }

    fn lossy(raw: &[u8]) -> (String, u64) {
        let mut out = String::new();
        let n = decode(raw, &mut out, true).unwrap();
        (out, n)
    }

    fn written(s: &str) -> String {
        let mut out = Vec::new();
        encode(s, &mut out);
        String::from_utf8(out).unwrap()
    }

    fn real_text(x: f64) -> Option<String> {
        let mut out = Vec::new();
        real(x, &mut String::new(), &mut out).then(|| String::from_utf8(out).unwrap())
    }

    #[test]
    fn directives_decode() {
        assert_eq!(strict(b"\\X\\E9").unwrap(), "\u{e9}");
        assert_eq!(strict(b"a\\X2\\00E900E8\\X0\\b").unwrap(), "a\u{e9}\u{e8}b");
        assert_eq!(strict(b"\\").unwrap(), "\\");
        assert_eq!(strict(b"\\\\").unwrap(), "\\");
        assert_eq!(
            strict(b"C:\\Users\\part.stp").unwrap(),
            "C:\\Users\\part.stp"
        );
        // One run per character, as some writers write them.
        assert_eq!(
            strict(b"\\X2\\5706\\X0\\\\X2\\89D2\\X0\\20").unwrap(),
            "\u{5706}\u{89d2}20"
        );
        assert_eq!(
            strict(b"\\X4\\0001F600\\X0\\\\X2\\00E9\\X0\\\\S\\a").unwrap(),
            "\u{1f600}\u{e9}\u{e1}"
        );
        // A surrogate pair in an `\X2\` run is one character.
        assert_eq!(strict(b"\\X2\\D83DDE00\\X0\\").unwrap(), "\u{1f600}");
        // `\S\` takes a basic-alphabet character; before a UTF-8 one it is kept as written.
        assert_eq!(strict("\\S\\\u{e9}".as_bytes()).unwrap(), "\\S\\\u{e9}");
        // An unterminated run, a short `\X\` and one with no digits are kept as written.
        assert_eq!(strict(b"\\X2\\00E9").unwrap(), "\\X2\\00E9");
        assert_eq!(strict(b"\\X\\E").unwrap(), "\\X\\E");
        assert_eq!(strict(b"\\X\\+A").unwrap(), "\\X\\+A");
        // An empty run is nothing.
        assert_eq!(strict(b"a\\X2\\\\X0\\b").unwrap(), "ab");
        assert_eq!(strict(b"a\\X2\\X0\\b").unwrap(), "ab");
    }

    #[test]
    fn text_that_is_not_unicode_is_refused_or_replaced_and_counted() {
        for (raw, text, count) in [
            (&b"\\X2\\00E\\X0\\"[..], "\u{fffd}", 1),
            (b"\\X2\\00G9\\X0\\", "\u{fffd}", 1),
            (b"\\X2\\+0E9\\X0\\", "\u{fffd}", 1),
            (b"\\X2\\D83D\\X0\\", "\u{fffd}", 1),
            (b"\\X2\\DE00\\X0\\", "\u{fffd}", 1),
            (b"\\X2\\D83D0041\\X0\\", "\u{fffd}A", 1),
            (b"\\X2\\D83Dzzzz\\X0\\", "\u{fffd}\u{fffd}", 2),
            (b"\\X2\\D83DD83DDE00\\X0\\", "\u{fffd}\u{1f600}", 1),
            (b"\\X2\\0041zzzz0042\\X0\\", "A\u{fffd}B", 1),
            (b"\\X4\\00110000\\X0\\", "\u{fffd}", 1),
            (b"\\X4\\0000D800\\X0\\", "\u{fffd}", 1),
            (b"\\X4\\0001F60\\X0\\", "\u{fffd}", 1),
            (b"\\X4\\0001F600zz\\X0\\", "\u{1f600}\u{fffd}", 1),
            (b"a\xffb\xfe", "a\u{fffd}b\u{fffd}", 2),
            (b"\xe9t\xe9", "\u{fffd}t\u{fffd}", 2),
        ] {
            assert_eq!(strict(raw), Err(BadText), "{raw:?}");
            assert_eq!(lossy(raw), (text.to_string(), count), "{raw:?}");
        }
        assert_eq!(lossy(b"plain"), ("plain".to_string(), 0));
    }

    /// The decoded text, with the windows its terminator searches compare counted: the test
    /// stops at the first one beyond the text's length, which a decoder searching the same tail
    /// again reaches.
    fn decode_counted(raw: &[u8]) -> String {
        let mut steps = 0;
        let mut out = String::new();
        decode_with(raw, &mut out, false, |rest| {
            rest.windows(4).position(|w| {
                steps += 1;
                assert!(
                    steps <= raw.len(),
                    "more search steps than {} bytes",
                    raw.len()
                );
                w == b"\\X0\\"
            })
        })
        .unwrap();
        out
    }

    #[test]
    fn run_terminators_are_searched_once() {
        // The space keeps a tag's closing backslash from pairing with the next tag's opening one
        // as an escaped `\\`, which would leave only the first tag to search from.
        for tag in [&b"\\X2\\ "[..], b"\\X4\\ "] {
            let raw = tag.repeat(100_000);
            // No `\X0\` anywhere: every tag is kept as written.
            assert_eq!(decode_counted(&raw).as_bytes(), raw.as_slice());
        }
        let raw = b"\\X2\\00E9\\X0\\".repeat(100_000);
        assert_eq!(decode_counted(&raw), "\u{e9}".repeat(100_000));
    }

    #[test]
    fn strings_are_written_so_they_read_back() {
        assert_eq!(written(""), "''");
        assert_eq!(written("it's"), "'it''s'");
        assert_eq!(written("a\\b"), "'a\\\\b'");
        assert_eq!(written("\u{e9}\u{e8}x"), "'\\X2\\00E900E8\\X0\\x'");
        assert_eq!(
            written("\u{e9}\u{1f600}\u{1f601}\n"),
            "'\\X2\\00E9\\X0\\\\X4\\0001F6000001F601\\X0\\\\X2\\000A\\X0\\'"
        );
        assert_eq!(written("\u{7f}~ "), "'\\X2\\007F\\X0\\~ '");
        for s in [
            "",
            "plain",
            "it's",
            "''",
            "\\",
            "\\\\",
            "\\X2\\",
            "\\X2\\00E9\\X0\\",
            "\\X\\E9",
            "\\S\\a",
            "\\X0\\",
            "a\tb\r\n\0",
            "\u{e9}\\\u{e9}",
            "\u{ffff}\u{10000}\u{10ffff}",
            "\u{5706}\u{89d2}20",
        ] {
            let text = written(s);
            assert!(text.is_ascii());
            let inner = &text.as_bytes()[1..text.len() - 1];
            // What sits between the quotes, as the reader hands it over: a doubled quote is one.
            let raw = String::from_utf8(inner.to_vec())
                .unwrap()
                .replace("''", "'");
            assert_eq!(strict(raw.as_bytes()).unwrap(), s, "{text}");
        }
    }

    #[test]
    fn reals_are_written_with_a_point_and_read_back_to_the_same_bits() {
        for (x, text) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (15.0, "15.0"),
            (-2.5, "-2.5"),
            (1e-3, "0.001"),
            (1e-7, "1.E-7"),
            (1.5e-7, "1.5E-7"),
            (1e21, "1.E21"),
            (1.25e300, "1.25E300"),
            (f64::MAX, "1.7976931348623157E308"),
            (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
            (5e-324, "5.E-324"),
            (0.1, "0.1"),
        ] {
            assert_eq!(real_text(x).as_deref(), Some(text));
            let back: f64 = text.parse().unwrap();
            assert_eq!(back.to_bits(), x.to_bits(), "{text}");
        }
        for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert_eq!(real_text(x), None);
        }
    }

    #[test]
    fn hex_reads_digits_only() {
        assert_eq!(hex(b"00e9", 4), Some(0xE9));
        assert_eq!(hex(b"FFFFFFFF", 8), Some(u32::MAX));
        for bad in [&b"+A"[..], b"-1", b" 1", b"0x"] {
            assert_eq!(hex(bad, 2), None, "{bad:?}");
        }
        assert_eq!(hex(b"00E", 4), None);
        assert_eq!(hex(b"0001F600", 4), None);
    }
}
