//! `String.prototype.localeCompare` for the text the prototype sorts: road, junction and zone names.
//!
//! The prototype orders names with `a.localeCompare(b)`, which in Node and the browsers is the ICU
//! root collation (Unicode Collation Algorithm): letters ignore case and accents first, then
//! accents, then case (lower case first); spaces and punctuation sort before digits, and digits before letters;
//! digits compare as characters ("10" before "2"). This module reproduces that for ASCII, the
//! Latin-1 and Latin Extended-A letters (including the macron vowels of Maori place names), the common
//! punctuation and symbols of Latin-1 and General Punctuation, and falls back on code point order after the
//! letters for anything else (other scripts, emoji), which ICU orders by script. Accent order and punctuation
//! order were read from Node 24 (ICU 78, CLDR 48) and are checked against it in tests/edit_truth.rs.

use std::cmp::Ordering;

/// Punctuation and symbols in collation order (all of them sort before digits), as ICU's root order lists them.
const SYMBOLS: &str = "\t\n\u{b}\u{c}\r \u{a0}_-\u{2010}\u{2011}\u{2013}\u{2014},;:!\u{a1}?\u{bf}.\u{2026}\u{b7}'\u{2018}\u{2019}\"\u{201c}\u{201d}\u{ab}\u{bb}()[]{}\u{a7}\u{b6}@*/&#%\u{2022}`\u{b4}^\u{af}\u{a8}\u{b8}\u{b0}\u{a9}\u{ae}+\u{b1}\u{d7}<=>\u{ac}|\u{a6}~\u{a4}\u{a2}$\u{a3}\u{a5}\u{20ac}";

/// Combining marks in secondary-weight order (acute first), the ones the letter table uses.
const MARKS: [u32; 18] = [
    0x301, 0x300, 0x306, 0x302, 0x30c, 0x30a, 0x308, 0x30b, 0x303, 0x307, 0x338, 0x327, 0x328,
    0x304, 0x30f, 0x311, 0x31b, 0x326,
];

/// Precomposed letters of Latin-1 and Latin Extended-A: (letter, base letter, combining mark).
const DECOMP: &[(char, char, u32)] = &[
    ('\u{c0}', 'A', 0x300),
    ('\u{c1}', 'A', 0x301),
    ('\u{c2}', 'A', 0x302),
    ('\u{c3}', 'A', 0x303),
    ('\u{c4}', 'A', 0x308),
    ('\u{c5}', 'A', 0x30a),
    ('\u{c7}', 'C', 0x327),
    ('\u{c8}', 'E', 0x300),
    ('\u{c9}', 'E', 0x301),
    ('\u{ca}', 'E', 0x302),
    ('\u{cb}', 'E', 0x308),
    ('\u{cc}', 'I', 0x300),
    ('\u{cd}', 'I', 0x301),
    ('\u{ce}', 'I', 0x302),
    ('\u{cf}', 'I', 0x308),
    ('\u{d1}', 'N', 0x303),
    ('\u{d2}', 'O', 0x300),
    ('\u{d3}', 'O', 0x301),
    ('\u{d4}', 'O', 0x302),
    ('\u{d5}', 'O', 0x303),
    ('\u{d6}', 'O', 0x308),
    ('\u{d9}', 'U', 0x300),
    ('\u{da}', 'U', 0x301),
    ('\u{db}', 'U', 0x302),
    ('\u{dc}', 'U', 0x308),
    ('\u{dd}', 'Y', 0x301),
    ('\u{e0}', 'a', 0x300),
    ('\u{e1}', 'a', 0x301),
    ('\u{e2}', 'a', 0x302),
    ('\u{e3}', 'a', 0x303),
    ('\u{e4}', 'a', 0x308),
    ('\u{e5}', 'a', 0x30a),
    ('\u{e7}', 'c', 0x327),
    ('\u{e8}', 'e', 0x300),
    ('\u{e9}', 'e', 0x301),
    ('\u{ea}', 'e', 0x302),
    ('\u{eb}', 'e', 0x308),
    ('\u{ec}', 'i', 0x300),
    ('\u{ed}', 'i', 0x301),
    ('\u{ee}', 'i', 0x302),
    ('\u{ef}', 'i', 0x308),
    ('\u{f1}', 'n', 0x303),
    ('\u{f2}', 'o', 0x300),
    ('\u{f3}', 'o', 0x301),
    ('\u{f4}', 'o', 0x302),
    ('\u{f5}', 'o', 0x303),
    ('\u{f6}', 'o', 0x308),
    ('\u{f9}', 'u', 0x300),
    ('\u{fa}', 'u', 0x301),
    ('\u{fb}', 'u', 0x302),
    ('\u{fc}', 'u', 0x308),
    ('\u{fd}', 'y', 0x301),
    ('\u{ff}', 'y', 0x308),
    ('\u{100}', 'A', 0x304),
    ('\u{101}', 'a', 0x304),
    ('\u{102}', 'A', 0x306),
    ('\u{103}', 'a', 0x306),
    ('\u{104}', 'A', 0x328),
    ('\u{105}', 'a', 0x328),
    ('\u{106}', 'C', 0x301),
    ('\u{107}', 'c', 0x301),
    ('\u{108}', 'C', 0x302),
    ('\u{109}', 'c', 0x302),
    ('\u{10a}', 'C', 0x307),
    ('\u{10b}', 'c', 0x307),
    ('\u{10c}', 'C', 0x30c),
    ('\u{10d}', 'c', 0x30c),
    ('\u{10e}', 'D', 0x30c),
    ('\u{10f}', 'd', 0x30c),
    ('\u{112}', 'E', 0x304),
    ('\u{113}', 'e', 0x304),
    ('\u{114}', 'E', 0x306),
    ('\u{115}', 'e', 0x306),
    ('\u{116}', 'E', 0x307),
    ('\u{117}', 'e', 0x307),
    ('\u{118}', 'E', 0x328),
    ('\u{119}', 'e', 0x328),
    ('\u{11a}', 'E', 0x30c),
    ('\u{11b}', 'e', 0x30c),
    ('\u{11c}', 'G', 0x302),
    ('\u{11d}', 'g', 0x302),
    ('\u{11e}', 'G', 0x306),
    ('\u{11f}', 'g', 0x306),
    ('\u{120}', 'G', 0x307),
    ('\u{121}', 'g', 0x307),
    ('\u{122}', 'G', 0x327),
    ('\u{123}', 'g', 0x327),
    ('\u{124}', 'H', 0x302),
    ('\u{125}', 'h', 0x302),
    ('\u{128}', 'I', 0x303),
    ('\u{129}', 'i', 0x303),
    ('\u{12a}', 'I', 0x304),
    ('\u{12b}', 'i', 0x304),
    ('\u{12c}', 'I', 0x306),
    ('\u{12d}', 'i', 0x306),
    ('\u{12e}', 'I', 0x328),
    ('\u{12f}', 'i', 0x328),
    ('\u{130}', 'I', 0x307),
    ('\u{134}', 'J', 0x302),
    ('\u{135}', 'j', 0x302),
    ('\u{136}', 'K', 0x327),
    ('\u{137}', 'k', 0x327),
    ('\u{139}', 'L', 0x301),
    ('\u{13a}', 'l', 0x301),
    ('\u{13b}', 'L', 0x327),
    ('\u{13c}', 'l', 0x327),
    ('\u{13d}', 'L', 0x30c),
    ('\u{13e}', 'l', 0x30c),
    ('\u{143}', 'N', 0x301),
    ('\u{144}', 'n', 0x301),
    ('\u{145}', 'N', 0x327),
    ('\u{146}', 'n', 0x327),
    ('\u{147}', 'N', 0x30c),
    ('\u{148}', 'n', 0x30c),
    ('\u{14c}', 'O', 0x304),
    ('\u{14d}', 'o', 0x304),
    ('\u{14e}', 'O', 0x306),
    ('\u{14f}', 'o', 0x306),
    ('\u{150}', 'O', 0x30b),
    ('\u{151}', 'o', 0x30b),
    ('\u{154}', 'R', 0x301),
    ('\u{155}', 'r', 0x301),
    ('\u{156}', 'R', 0x327),
    ('\u{157}', 'r', 0x327),
    ('\u{158}', 'R', 0x30c),
    ('\u{159}', 'r', 0x30c),
    ('\u{15a}', 'S', 0x301),
    ('\u{15b}', 's', 0x301),
    ('\u{15c}', 'S', 0x302),
    ('\u{15d}', 's', 0x302),
    ('\u{15e}', 'S', 0x327),
    ('\u{15f}', 's', 0x327),
    ('\u{160}', 'S', 0x30c),
    ('\u{161}', 's', 0x30c),
    ('\u{162}', 'T', 0x327),
    ('\u{163}', 't', 0x327),
    ('\u{164}', 'T', 0x30c),
    ('\u{165}', 't', 0x30c),
    ('\u{168}', 'U', 0x303),
    ('\u{169}', 'u', 0x303),
    ('\u{16a}', 'U', 0x304),
    ('\u{16b}', 'u', 0x304),
    ('\u{16c}', 'U', 0x306),
    ('\u{16d}', 'u', 0x306),
    ('\u{16e}', 'U', 0x30a),
    ('\u{16f}', 'u', 0x30a),
    ('\u{170}', 'U', 0x30b),
    ('\u{171}', 'u', 0x30b),
    ('\u{172}', 'U', 0x328),
    ('\u{173}', 'u', 0x328),
    ('\u{174}', 'W', 0x302),
    ('\u{175}', 'w', 0x302),
    ('\u{176}', 'Y', 0x302),
    ('\u{177}', 'y', 0x302),
    ('\u{178}', 'Y', 0x308),
    ('\u{179}', 'Z', 0x301),
    ('\u{17a}', 'z', 0x301),
    ('\u{17b}', 'Z', 0x307),
    ('\u{17c}', 'z', 0x307),
    ('\u{17d}', 'Z', 0x30c),
    ('\u{17e}', 'z', 0x30c),
];

/// One collation element: primary, secondary and tertiary weights (0 = ignorable at that level).
type Ce = (u32, u32, u32);

const SEC_BASE: u32 = 0x20;
const LETTER_PRIMARY: u32 = 1000;
const DIGIT_PRIMARY: u32 = 500;
const FALLBACK_PRIMARY: u32 = 100_000;

fn symbol_primary(c: char) -> Option<u32> {
    SYMBOLS.chars().position(|s| s == c).map(|i| 10 + i as u32)
}

fn mark_secondary(cp: u32) -> u32 {
    match MARKS.iter().position(|&m| m == cp) {
        Some(i) => SEC_BASE + 1 + i as u32,
        None => SEC_BASE + 100 + cp,
    }
}

fn letter_ce(base: char) -> Ce {
    let lower = base.to_ascii_lowercase();
    let p = LETTER_PRIMARY + (lower as u32 - 'a' as u32) * 4;
    (p, SEC_BASE, if base.is_ascii_uppercase() { 8 } else { 2 })
}

/// The collation elements of a string.
fn elements(s: &str) -> Vec<Ce> {
    let mut out: Vec<Ce> = Vec::with_capacity(s.len() + 2);
    for c in s.chars() {
        let cp = c as u32;
        if c.is_ascii_alphabetic() {
            out.push(letter_ce(c));
        } else if c.is_ascii_digit() {
            out.push((DIGIT_PRIMARY + (cp - '0' as u32), SEC_BASE, 2));
        } else if let Some(p) = symbol_primary(c) {
            out.push((p, SEC_BASE, 2));
        } else if (0x300..=0x36f).contains(&cp) {
            // A combining mark: no primary weight, its own secondary
            out.push((0, mark_secondary(cp), 2));
        } else if let Some(&(_, base, mark)) = DECOMP.iter().find(|d| d.0 == c) {
            out.push(letter_ce(base));
            out.push((0, mark_secondary(mark), 2));
        } else {
            match c {
                '\u{e6}' | '\u{c6}' => {
                    let t = if c == '\u{c6}' { 9 } else { 3 };
                    out.push((letter_ce('a').0, SEC_BASE, t));
                    out.push((letter_ce('e').0, SEC_BASE, t));
                }
                '\u{153}' | '\u{152}' => {
                    let t = if c == '\u{152}' { 9 } else { 3 };
                    out.push((letter_ce('o').0, SEC_BASE, t));
                    out.push((letter_ce('e').0, SEC_BASE, t));
                }
                '\u{df}' => {
                    out.push((letter_ce('s').0, SEC_BASE, 3));
                    out.push((letter_ce('s').0, SEC_BASE, 3));
                }
                '\u{f8}' | '\u{d8}' => {
                    out.push(letter_ce(if c == '\u{d8}' { 'O' } else { 'o' }));
                    out.push((0, mark_secondary(0x338), 2));
                }
                '\u{142}' | '\u{141}' => {
                    out.push(letter_ce(if c == '\u{141}' { 'L' } else { 'l' }));
                    out.push((0, mark_secondary(0x338), 2));
                }
                '\u{111}' | '\u{110}' => {
                    out.push(letter_ce(if c == '\u{110}' { 'D' } else { 'd' }));
                    out.push((0, mark_secondary(0x338), 2));
                }
                '\u{131}' => out.push((letter_ce('i').0, SEC_BASE + 200, 2)),
                // Controls, format characters and joiners are ignorable
                _ if cp < 0x20
                    || (0x7f..=0x9f).contains(&cp)
                    || cp == 0xad
                    || (0x200b..=0x200f).contains(&cp)
                    || cp == 0xfeff => {}
                // Anything else: after the letters, in code point order
                _ => out.push((FALLBACK_PRIMARY + cp, SEC_BASE, 2)),
            }
        }
    }
    out
}

fn level(ces: &[Ce], pick: impl Fn(&Ce) -> u32) -> Vec<u32> {
    ces.iter().map(pick).filter(|&w| w != 0).collect()
}

/// `a.localeCompare(b)` as an ordering: primary weights first (letters without case or accent), then
/// accents, then case; equal strings (including a letter with its accent written as two characters) are Equal.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (ea, eb) = (elements(a), elements(b));
    for pick in [|c: &Ce| c.0, |c: &Ce| c.1, |c: &Ce| c.2] {
        let o = level(&ea, pick).cmp(&level(&eb, pick));
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

/// `a.localeCompare(b)` as the number the JavaScript returns (below 0, 0 or above 0), for sort comparators.
pub fn locale_compare_f64(a: &str, b: &str) -> f64 {
    match locale_compare(a, b) {
        Ordering::Less => -1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => 1.0,
    }
}
