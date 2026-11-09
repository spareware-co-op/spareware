//! The text form: SKG's deterministic CBOR written as extended diagnostic notation, with every float
//! a hexadecimal float, for diffs, fixtures and AI agents; never for storage (24.6).
//!
//! What it does: [`to_text`] decodes one item with the strict decoder and writes it as text. The
//! layout is fixed, so equal bytes give equal text:
//!
//! - unsigned and negative integers in decimal; `false`, `true` and `null`;
//! - floats as hexadecimal floats, `0x1.8p+1` (RFC 8610 Appendix G.5), normalised to a leading
//!   `1` (zero is `0x0p+0`, with `-` for −0.0), with the fraction's trailing zero digits left out;
//! - byte strings as `h'...'` in lower-case hexadecimal, with no spaces;
//! - text strings in double quotes, with `"` and `\` escaped, the control characters U+0000 to
//!   U+001F and U+007F written as JSON escapes, and every other character as itself;
//! - a tag as its number and its byte string in brackets, `86(h'...')`;
//! - an empty array or map as `[]` or `{}`; any other one item or pair per line, indented two
//!   spaces a level, with commas between them and the closing bracket on a line of its own.
//!
//! The text's float width needs no encoding indicator: SKG's canonical form fixes each float's
//! width by its value, so [`super::parse::from_text`] gives back the same bytes.
//!
//! Guarantees:
//!
//! - **Lossless.** For any bytes the strict decoder accepts, `from_text(to_text(bytes))` is the same
//!   bytes, floats bit for bit.
//! - **One text per item.** The layout depends only on the bytes.
//! - **Refuses what the decoder refuses.** Bytes that are not SKG's deterministic CBOR give the
//!   decoder's error and no text.
//!
//! Method: RFC 8949 section 8 (diagnostic notation), <https://www.rfc-editor.org/rfc/rfc8949#section-8>,
//! and RFC 8610 Appendix G (extended diagnostic notation; G.5, hexadecimal floats),
//! <https://www.rfc-editor.org/rfc/rfc8610#appendix-G>; text escapes as JSON's, RFC 8259 section 7,
//! <https://www.rfc-editor.org/rfc/rfc8259#section-7>. A hexadecimal float writes a double's 52
//! fraction bits as 13 hexadecimal digits after `0x1.` and its unbiased exponent after `p`; a
//! subnormal is first scaled by 2^64, exactly, to read its leading bit as a normal double's.
//!
//! Cost: O(n) time in the input and the text, which is at most about three bytes per input byte
//! (hexadecimal) plus the indentation; the text is built in one `String`. The formatting stack holds
//! one entry per open container or tag, at most the depth limit. Recursion: none.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use geom_run::WorkMeter;

use super::decode::{Decoder, check_end};
use super::error::CborError;
use super::item::Item;

/// The 52 fraction bits of a double.
const FRACTION: u64 = (1 << 52) - 1;

/// 2^64, the exact scaling that makes a subnormal double normal.
const TWO_POW_64: f64 = f64::from_bits((1023 + 64) << 52);

/// One open container or tag while writing.
#[derive(Clone, Copy, Debug)]
enum Open {
    /// An array with this many items still to write.
    Array { left: u64 },
    /// A map with this many pairs still to write, and whether a key is next.
    Map { left: u64, key_next: bool },
    /// A tag, closed by `)` after its one item.
    Tag,
}

/// The text form of the one item `bytes` hold, nested at most `depth` deep.
///
/// # Errors
///
/// [`CborError`] for the first rule of SKG's canonical form the bytes break, for bytes after the
/// item, or for the meter's stop.
pub fn to_text(bytes: &[u8], depth: usize, meter: &mut WorkMeter<'_>) -> Result<String, CborError> {
    let mut decoder = Decoder::new(bytes, depth);
    let mut out = String::new();
    let mut open: Vec<Open> = Vec::new();
    while let Some(item) = decoder.next(meter)? {
        separate(&mut out, &open);
        match item {
            Item::Uint(n) => push(&mut out, format_args!("{n}")),
            Item::Nint(n) => match n.checked_add(1) {
                Some(m) => push(&mut out, format_args!("-{m}")),
                None => out.push_str("-18446744073709551616"),
            },
            Item::Bytes(b) => {
                out.push_str("h'");
                for byte in b {
                    push(&mut out, format_args!("{byte:02x}"));
                }
                out.push('\'');
            }
            Item::Text(t) => quote(&mut out, t),
            Item::Float(x) => hex_float(&mut out, x),
            Item::Bool(b) => out.push_str(if b { "true" } else { "false" }),
            Item::Null => out.push_str("null"),
            Item::Array(0) => out.push_str("[]"),
            Item::Map(0) => out.push_str("{}"),
            Item::Array(n) => {
                out.push('[');
                open.push(Open::Array { left: n });
                continue;
            }
            Item::Map(n) => {
                out.push('{');
                open.push(Open::Map {
                    left: n,
                    key_next: true,
                });
                continue;
            }
            Item::Tag(n) => {
                push(&mut out, format_args!("{n}("));
                open.push(Open::Tag);
                continue;
            }
        }
        close(&mut out, &mut open);
    }
    check_end(&decoder, bytes)?;
    Ok(out)
}

/// Writes what comes before an item: nothing for a map's value or a tag's content, otherwise a
/// comma after the previous item and a new line indented to the item's level.
fn separate(out: &mut String, open: &[Open]) {
    let after_another = match open.last() {
        Some(Open::Array { .. } | Open::Map { key_next: true, .. }) => {
            !out.ends_with('[') && !out.ends_with('{')
        }
        _ => return,
    };
    if after_another {
        out.push(',');
    }
    newline(out, open_levels(open));
}

/// Records that an item was written, closing every container and tag it completes, and writing
/// `: ` after a map's key.
fn close(out: &mut String, open: &mut Vec<Open>) {
    while let Some(top) = open.last_mut() {
        match top {
            Open::Tag => out.push(')'),
            Open::Array { left } => {
                *left = left.saturating_sub(1);
                if *left != 0 {
                    return;
                }
                let levels = open_levels(open).saturating_sub(1);
                newline(out, levels);
                out.push(']');
            }
            Open::Map { left, key_next } => {
                if *key_next {
                    *key_next = false;
                    out.push_str(": ");
                    return;
                }
                *key_next = true;
                *left = left.saturating_sub(1);
                if *left != 0 {
                    return;
                }
                let levels = open_levels(open).saturating_sub(1);
                newline(out, levels);
                out.push('}');
            }
        }
        open.pop();
    }
}

/// How many arrays and maps are open.
fn open_levels(open: &[Open]) -> usize {
    open.iter().filter(|o| !matches!(o, Open::Tag)).count()
}

/// A new line, indented two spaces per level.
fn newline(out: &mut String, levels: usize) {
    out.push('\n');
    for _ in 0..levels {
        out.push_str("  ");
    }
}

/// Appends formatted text; writing to a `String` cannot fail.
fn push(out: &mut String, args: core::fmt::Arguments<'_>) {
    let _ = out.write_fmt(args);
}

/// Appends `text` in double quotes, escaping `"`, `\` and the control characters.
fn quote(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\u{0}'..='\u{1f}' | '\u{7f}' => push(out, format_args!("\\u{:04x}", u32::from(c))),
            _ => out.push(c),
        }
    }
    out.push('"');
}

/// Appends the finite double `x` as a hexadecimal float, `[-]0x1.<fraction>p<sign><exponent>`, or
/// `[-]0x0p+0` for a zero.
pub(crate) fn hex_float(out: &mut String, x: f64) {
    if x.is_sign_negative() {
        out.push('-');
    }
    let magnitude = x.abs();
    if magnitude == 0.0 {
        out.push_str("0x0p+0");
        return;
    }
    // A subnormal times 2^64 is a normal double, exactly; its exponent is then read 64 lower.
    let (normal, offset) = if magnitude < f64::MIN_POSITIVE {
        (magnitude * TWO_POW_64, 1023 + 64)
    } else {
        (magnitude, 1023)
    };
    let bits = normal.to_bits();
    let field = bits >> 52;
    let fraction = bits & FRACTION;
    out.push_str("0x1");
    if fraction != 0 {
        let mut digits = String::new();
        push(&mut digits, format_args!("{fraction:013x}"));
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
    out.push('p');
    out.push(if field >= offset { '+' } else { '-' });
    push(out, format_args!("{}", field.abs_diff(offset)));
}
