//! CSV text for the data pack: cells, RFC 4180 quoting with the formula guard, and the ASCII file-name part (`slug`).

use crate::edit::json::JsonValue;
use crate::jsnum::{Maybe, Num, JsNum, is_js_space, js_trim, number_to_js_string};

/// A cell of a data pack table as the prototype's JavaScript holds it: text, a number, a flag, or nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    /// Text.
    S(String),
    /// A number (NaN and the infinities print as JavaScript prints them).
    N(f64),
    /// true or false.
    B(bool),
    /// null or undefined: an empty cell.
    Null,
}

impl Cell {
    /// Text from a string slice.
    pub fn s(t: &str) -> Cell {
        Cell::S(t.to_string())
    }
    /// A loose saved number.
    pub fn from_num(n: &Num) -> Cell {
        match n {
            Maybe::Missing | Maybe::Null => Cell::Null,
            Maybe::Set(JsNum::Number(x)) => Cell::N(*x),
            Maybe::Set(JsNum::Bool(b)) => Cell::B(*b),
            Maybe::Set(other) => Cell::S(other.to_js_string()),
        }
    }
    /// A loose JSON value.
    pub fn from_json(v: Option<&JsonValue>) -> Cell {
        match v {
            None | Some(JsonValue::Null) => Cell::Null,
            Some(JsonValue::Number(x)) => Cell::N(*x),
            Some(JsonValue::Bool(b)) => Cell::B(*b),
            Some(JsonValue::String(s)) => Cell::S(s.clone()),
            Some(other) => Cell::S(other.js_string()),
        }
    }
    /// `x || ''` for a loose JSON value, as text.
    pub fn or_blank(v: Option<&JsonValue>) -> Cell {
        match v {
            Some(x) if x.truthy() => Cell::from_json(Some(x)),
            _ => Cell::S(String::new()),
        }
    }
    fn text(&self) -> String {
        match self {
            Cell::S(s) => s.clone(),
            Cell::N(x) => number_to_js_string(*x),
            Cell::B(b) => b.to_string(),
            Cell::Null => String::new(),
        }
    }
}

/// `NUMLIKE`: `/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i`.
fn numlike(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let start = i;
    let digits = |i: &mut usize| {
        let s0 = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - s0
    };
    let n1 = digits(&mut i);
    if n1 > 0 {
        if i < b.len() && b[i] == b'.' {
            i += 1;
            digits(&mut i);
        }
    } else if i < b.len() && b[i] == b'.' {
        i += 1;
        if digits(&mut i) == 0 {
            return false;
        }
    } else {
        let _ = start;
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            j += 1;
        }
        let s0 = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == s0 {
            return false;
        }
        i = j;
    }
    i == b.len()
}

/// `csvCell(v)`: RFC 4180 text: a cell with a comma, quote, CR or LF is quoted. Text that a spreadsheet would read as a formula
/// (starting with = + - @, a tab or a CR, and not a number) gets a leading apostrophe.
pub(crate) fn csv_cell(v: &Cell) -> String {
    let mut s = v.text();
    if let Cell::S(t) = v
        && t.starts_with(['=', '+', '-', '@', '\t', '\r'])
        && !numlike(js_trim(t))
    {
        s = format!("'{s}");
    }
    if s.contains(['"', ',', '\r', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
}

/// `csvText(rows)`.
pub(crate) fn csv_text(rows: &[Vec<Cell>]) -> String {
    rows.iter().map(|r| r.iter().map(csv_cell).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\r\n")
}

/// The Latin letters with marks, ligatures and superscripts that NFKD turns into ASCII (the rest of what the prototype's
/// `normalize('NFKD')` changes becomes hyphens either way).
fn decompose(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{c0}' | '\u{c1}' | '\u{c2}' | '\u{c3}' | '\u{c4}' | '\u{c5}' => "A",
        '\u{e0}' | '\u{e1}' | '\u{e2}' | '\u{e3}' | '\u{e4}' | '\u{e5}' => "a",
        '\u{c7}' => "C",
        '\u{e7}' => "c",
        '\u{c8}' | '\u{c9}' | '\u{ca}' | '\u{cb}' => "E",
        '\u{e8}' | '\u{e9}' | '\u{ea}' | '\u{eb}' => "e",
        '\u{cc}' | '\u{cd}' | '\u{ce}' | '\u{cf}' => "I",
        '\u{ec}' | '\u{ed}' | '\u{ee}' | '\u{ef}' => "i",
        '\u{d1}' => "N",
        '\u{f1}' => "n",
        '\u{d2}' | '\u{d3}' | '\u{d4}' | '\u{d5}' | '\u{d6}' => "O",
        '\u{f2}' | '\u{f3}' | '\u{f4}' | '\u{f5}' | '\u{f6}' => "o",
        '\u{d9}' | '\u{da}' | '\u{db}' | '\u{dc}' => "U",
        '\u{f9}' | '\u{fa}' | '\u{fb}' | '\u{fc}' => "u",
        '\u{dd}' => "Y",
        '\u{fd}' | '\u{ff}' => "y",
        '\u{100}' | '\u{102}' | '\u{104}' => "A",
        '\u{101}' | '\u{103}' | '\u{105}' => "a",
        '\u{106}' | '\u{108}' | '\u{10a}' | '\u{10c}' => "C",
        '\u{107}' | '\u{109}' | '\u{10b}' | '\u{10d}' => "c",
        '\u{10e}' => "D",
        '\u{10f}' => "d",
        '\u{112}' | '\u{114}' | '\u{116}' | '\u{118}' | '\u{11a}' => "E",
        '\u{113}' | '\u{115}' | '\u{117}' | '\u{119}' | '\u{11b}' => "e",
        '\u{11c}' | '\u{11e}' | '\u{120}' | '\u{122}' => "G",
        '\u{11d}' | '\u{11f}' | '\u{121}' | '\u{123}' => "g",
        '\u{124}' => "H",
        '\u{125}' => "h",
        '\u{128}' | '\u{12a}' | '\u{12c}' | '\u{12e}' | '\u{130}' => "I",
        '\u{129}' | '\u{12b}' | '\u{12d}' | '\u{12f}' => "i",
        '\u{134}' => "J",
        '\u{135}' => "j",
        '\u{136}' => "K",
        '\u{137}' => "k",
        '\u{139}' | '\u{13b}' | '\u{13d}' => "L",
        '\u{13a}' | '\u{13c}' | '\u{13e}' => "l",
        '\u{143}' | '\u{145}' | '\u{147}' => "N",
        '\u{144}' | '\u{146}' | '\u{148}' => "n",
        '\u{14c}' | '\u{14e}' | '\u{150}' => "O",
        '\u{14d}' | '\u{14f}' | '\u{151}' => "o",
        '\u{154}' | '\u{156}' | '\u{158}' => "R",
        '\u{155}' | '\u{157}' | '\u{159}' => "r",
        '\u{15a}' | '\u{15c}' | '\u{15e}' | '\u{160}' => "S",
        '\u{15b}' | '\u{15d}' | '\u{15f}' | '\u{161}' => "s",
        '\u{162}' | '\u{164}' => "T",
        '\u{163}' | '\u{165}' => "t",
        '\u{168}' | '\u{16a}' | '\u{16c}' | '\u{16e}' | '\u{170}' | '\u{172}' => "U",
        '\u{169}' | '\u{16b}' | '\u{16d}' | '\u{16f}' | '\u{171}' | '\u{173}' => "u",
        '\u{174}' => "W",
        '\u{175}' => "w",
        '\u{176}' | '\u{178}' => "Y",
        '\u{177}' => "y",
        '\u{179}' | '\u{17b}' | '\u{17d}' => "Z",
        '\u{17a}' | '\u{17c}' | '\u{17e}' => "z",
        '\u{fb00}' => "ff",
        '\u{fb01}' => "fi",
        '\u{fb02}' => "fl",
        '\u{fb03}' => "ffi",
        '\u{fb04}' => "ffl",
        '\u{b2}' => "2",
        '\u{b3}' => "3",
        '\u{b9}' => "1",
        '\u{2070}' => "0",
        '\u{2074}' => "4",
        '\u{2075}' => "5",
        '\u{2076}' => "6",
        '\u{2077}' => "7",
        '\u{2078}' => "8",
        '\u{2079}' => "9",
        _ => return None,
    })
}

/// `slug(s)`: an ASCII file-name part: macrons and other accents dropped ('Ōtāhuhu Road' becomes 'otahuhu-road').
pub(crate) fn slug(s: &str) -> String {
    let mut t = String::new();
    for c in s.chars() {
        if ('\u{300}'..='\u{36f}').contains(&c) {
            continue;
        }
        match decompose(c) {
            Some(x) => t.push_str(x),
            None => t.push(c),
        }
    }
    let lower = t.to_lowercase();
    // /[^a-z0-9]+/g to '-', then trim the hyphens at both ends
    let mut out = String::new();
    let mut run = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            run = false;
        } else if !run {
            out.push('-');
            run = true;
        }
    }
    let out = out.trim_matches('-');
    let cut: String = out.chars().take(40).collect();
    cut.trim_end_matches('-').to_string()
}

/// Whether the text has any character the prototype's `\s` matches.
#[allow(dead_code)]
pub(crate) fn has_space(s: &str) -> bool {
    s.chars().any(is_js_space)
}
