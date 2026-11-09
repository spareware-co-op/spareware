//! Zip item names: the names no reader may write outside its folder by (zip-slip), and the part
//! names the Open Packaging Conventions allow.
//!
//! What it does: [`escapes`] says whether an item name could reach outside the folder an archive
//! is read into: an empty name, a backslash or NUL, a drive (`C:`), a leading `/`, or a `.` or `..`
//! segment. [`check`] holds an item name, read as the part name `/` + name, to the part-name rules
//! [M1.1] to [M1.10] of ECMA-376 Part 2 (ISO/IEC 29500-2), and returns the first it breaks as a
//! [`PartRule`]. Equivalence of part names, [M1.12] (case-insensitive ASCII),
//! is checked over the whole archive by its caller with [`fold`].
//!
//! Guarantees:
//!
//! - **Each rule exactly as written.** A segment is non-empty [M1.3], holds only `pchar`
//!   characters of RFC 3986 [M1.6], no percent-encoded `/` or `\` [M1.7] and no percent-encoded
//!   unreserved character [M1.8], does not end in `.` [M1.9] and is not all dots [M1.10]; the
//!   name does not end in `/` [M1.5] and is not empty [M1.1].
//! - **Total.** Every function answers for every string, with no panic.
//!
//! Method: ECMA-376 Part 2, Open Packaging Conventions, its part-name rules M1.1 to M1.12, <https://ecma-international.org/publications-and-standards/standards/ecma-376/>; the
//! `pchar`, `unreserved` and `pct-encoded` productions of RFC 3986, section 3.3,
//! <https://www.rfc-editor.org/rfc/rfc3986#section-3.3>; zip-slip as Snyk describes it,
//! <https://security.snyk.io/research/zip-slip-vulnerability>.
//!
//! Consulted: Apache POI, Apache-2.0,
//!   <https://github.com/apache/poi/blob/519e0a3c3945364e8ea75c172fe1a15fb6d4a202/poi-ooxml/src/main/java/org/apache/poi/openxml4j/opc/PackagePartName.java>,
//!   for the rules' wording and their order of checks (empty, slashes, segments, pchar, escapes).
//!   No code copied.
//!
//! Cost: linear in the name. No allocation except [`fold`]'s copy. No recursion.

use alloc::string::String;

/// The first part-name rule of ECMA-376 Part 2 a name breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartRule {
    /// [M1.1] The name is empty.
    Empty,
    /// [M1.3] A segment is empty (`//`).
    EmptySegment,
    /// [M1.5] The name ends in `/`.
    TrailingSlash,
    /// [M1.6] A segment holds a character that is not a `pchar`, or a `%` not followed by two
    /// hexadecimal digits.
    NotPchar,
    /// [M1.7] A segment holds a percent-encoded `/` or `\`.
    EncodedSlash,
    /// [M1.8] A segment holds a percent-encoded unreserved character.
    EncodedUnreserved,
    /// [M1.9] A segment ends in `.`.
    TrailingDot,
    /// [M1.10] A segment holds only dots.
    AllDots,
    /// [M1.12] Two names are equal as case-insensitive ASCII.
    Equivalent,
}

/// Whether the name could reach outside the folder it is read into.
#[must_use]
pub fn escapes(name: &str) -> bool {
    if name.is_empty() || name.starts_with('/') || name.contains(['\\', '\0']) {
        return true;
    }
    let first = name.split('/').next().unwrap_or("");
    if first.len() >= 2 && first.as_bytes().get(1) == Some(&b':') {
        return true;
    }
    name.split('/').any(|s| s == "." || s == "..")
}

const fn unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~')
}

const fn pchar(c: u8) -> bool {
    unreserved(c)
        || matches!(
            c,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        )
}

const fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c.saturating_sub(b'0')),
        b'a'..=b'f' => Some(c.saturating_sub(b'a').saturating_add(10)),
        b'A'..=b'F' => Some(c.saturating_sub(b'A').saturating_add(10)),
        _ => None,
    }
}

/// The rules one segment breaks, [M1.6] to [M1.10].
fn segment(s: &[u8]) -> Result<(), PartRule> {
    if s.is_empty() {
        return Err(PartRule::EmptySegment);
    }
    let mut at = 0_usize;
    while let Some(&c) = s.get(at) {
        if c == b'%' {
            let pair = s.get(at.saturating_add(1)..at.saturating_add(3));
            let Some(&[h, l]) = pair else {
                return Err(PartRule::NotPchar);
            };
            let (Some(h), Some(l)) = (hex(h), hex(l)) else {
                return Err(PartRule::NotPchar);
            };
            // Two hexadecimal digits: at most 0xFF.
            let decoded = h.saturating_mul(16).saturating_add(l);
            if decoded == b'/' || decoded == b'\\' {
                return Err(PartRule::EncodedSlash);
            }
            if unreserved(decoded) {
                return Err(PartRule::EncodedUnreserved);
            }
            at = at.saturating_add(3);
        } else if pchar(c) {
            at = at.saturating_add(1);
        } else {
            return Err(PartRule::NotPchar);
        }
    }
    if s.ends_with(b".") {
        // [M1.10] is the narrower rule for a segment of dots alone.
        return Err(if s.iter().all(|&c| c == b'.') {
            PartRule::AllDots
        } else {
            PartRule::TrailingDot
        });
    }
    Ok(())
}

/// The first part-name rule the item name `name`, read as the part name `/name`, breaks.
///
/// # Errors
///
/// The rule.
pub fn check(name: &str) -> Result<(), PartRule> {
    if name.is_empty() {
        return Err(PartRule::Empty);
    }
    if name.ends_with('/') {
        return Err(PartRule::TrailingSlash);
    }
    name.split('/').try_for_each(|s| segment(s.as_bytes()))
}

/// The name as [M1.12] compares it: ASCII letters folded to lower case.
#[must_use]
pub fn fold(name: &str) -> String {
    name.to_ascii_lowercase()
}
