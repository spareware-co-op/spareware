//! The banned-source scanner: finds the markers that show code in the kernel's crates may come
//! from a source chapter 29 bans: Numerical Recipes, copyleft code, and Autodesk's hatch patterns
//! and linetypes.
//!
//! What it does: [`scan_text`] reads one file's text and returns every [`Hit`]: a Numerical Recipes
//! routine name, macro, variable name, author or step-control constant; a copyleft licence's notice
//! text; a statement that code was taken, ported, adapted or copied from a copyleft source; or an
//! Autodesk hatch pattern or linetype name or description. [`repo`] scans every file of the
//! kernel's crates and the facade's, with its named exemptions, and the binary `source-scan`
//! (`cargo run -p geom-test --bin source-scan -- check`) runs it in CI.
//!
//! Why: chapter 29.20, rule 7, "Never write from memory what you may not copy": people and AI
//! models remember code, and AI-written "clean-room" code has already reproduced proprietary
//! content elsewhere; "the banned-source scanner catches the common markers; review catches the
//! rest". Rule 2 lists the comments that show a source's chain of title is in doubt ("taken from",
//! "based on", "adapted from", "mirrors"), and risk R18 of chapter 31 names a scanner hit as the
//! early warning that copyleft or non-free code is hiding in the kernel.
//!
//! Guarantees:
//!
//! - **Every marker of [`markers`] is found, wherever it is**: in code, a comment, a string or a
//!   data file, matched by words, so punctuation, line breaks inside a phrase and comment marks
//!   (`//!`, `#`, `*`, `;`, `--`) do not hide it. A routine name is found as a part of an
//!   identifier (`ST_pythag`, `smosPythag`), with or without a trailing number or the precision
//!   prefix of the book's variants (`tqli2`, `dpythag`); a variable name as a word or a part; a
//!   phrase also when it is written as one identifier (`numerical_recipes`, `takenFromCgal`); a
//!   description also when a string literal escapes its backslashes (`/\\/\\/`); and a constant
//!   as a value however its decimal literal is written (`1.89e-4`, `0.000189`, `.000189`,
//!   `1.89E-04_f64`).
//! - **A taken-from statement needs a taking phrase and a banned source.** A hit needs a taking
//!   phrase ([`markers::TAKING`], its two words at most [`markers::TAKING_GAP`] words apart)
//!   followed within [`markers::WINDOW`] words by a copyleft source's
//!   name ([`markers::COPYLEFT_SOURCES`]), with no negation in the [`markers::NEGATION_REACH`]
//!   words before the phrase and in the same sentence, so "no code is taken from OCCT" is not a
//!   hit, and "based on CGAL's widening" and "No tests. Taken from CGAL." are.
//! - **Each hit names its line, category and marker**, in the order they occur; one marker on one
//!   line is reported once.
//! - **Words, not meaning.** A marker spelt with look-alike characters, split by punctuation
//!   inside a word, built at run time, written as an expression or a hexadecimal float, named
//!   after the phrase that takes it ("CGAL, from which this was taken"), or paraphrased is not
//!   found: rule 7 leaves the rest to review.
//! - **Deterministic.** The same text gives the same hits on every target: words are runs of
//!   Unicode letters, digits and `_`, compared after lower-casing, and numbers are compared as the
//!   doubles Rust parses them to, which is exact and the same everywhere.
//!
//! Method: matching words and short phrases of a written marker list, the method of license
//! scanners such as ScanCode (Apache-2.0), whose rules are lists of words matched against a
//! file's word sequence (<https://scancode-toolkit.readthedocs.io/en/stable/explanations/license-detection-reference.html>);
//! the markers and their sources are chapter 29.20, rules 2 and 7
//! (`projects/engines/geometry/architecture/29-sources-licences.md`), and [`markers`]; number
//! literals as the Rust Reference's float literal grammar
//! (<https://doc.rust-lang.org/reference/tokens.html#floating-point-literals>), which C's matches
//! for the forms the markers take.
//!
//! Cost: linear in the text's length times the number of markers, all of them short.

pub mod markers;
pub mod repo;

use core::fmt;

/// What a hit shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// A Numerical Recipes routine name, macro, variable name, author or constant.
    NumericalRecipes,
    /// Words of a copyleft licence's notice.
    CopyleftNotice,
    /// A statement that code was taken from a copyleft source.
    TakenFromCopyleft,
    /// An Autodesk hatch pattern or linetype name or description.
    AcadPattern,
}

impl Category {
    /// Every category.
    pub const ALL: [Category; 4] = [
        Category::NumericalRecipes,
        Category::CopyleftNotice,
        Category::TakenFromCopyleft,
        Category::AcadPattern,
    ];

    /// The category's name, as every hit writes it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Category::NumericalRecipes => "Numerical Recipes",
            Category::CopyleftNotice => "Copyleft notice",
            Category::TakenFromCopyleft => "Taken from copyleft",
            Category::AcadPattern => "Autodesk pattern",
        }
    }

    /// What the rule is and what to do about a hit.
    #[must_use]
    pub fn advice(self) -> &'static str {
        match self {
            Category::NumericalRecipes => {
                "Numerical Recipes code is never ported, read for porting or written from memory \
                 (29.4, 29.6; 29.20 rule 7): write the routine from the primary source chapter \
                 29.4 names, with its own names and constants derived and cited in the header"
            }
            Category::CopyleftNotice => {
                "no GPL, LGPL or AGPL code is ever copied into Spare Kit (29.2): remove what came \
                 with the notice and tell an owner (29.20 rule 11)"
            }
            Category::TakenFromCopyleft => {
                "copyleft code is never taken (29.2); a method read under the method-only \
                 procedure is credited with a Consulted: record that names its method note (29.20 \
                 rules 4 and 6): write that record instead, or, if code was taken, remove it and \
                 tell an owner (rules 2 and 11)"
            }
            Category::AcadPattern => {
                "no hatch patterns or linetypes from other products, from memory or otherwise \
                 (29.20 rule 7; 29.19): our own library is authored under the owner-approved \
                 process of build pieces G184 and G211"
            }
        }
    }
}

/// One marker found in a text.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    /// The line it starts on, counting from 1.
    pub line: usize,
    /// What it shows.
    pub category: Category,
    /// The marker as the text writes it (a phrase's words joined by spaces).
    pub marker: String,
}

impl fmt::Display for Hit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {:?}: {}",
            self.line,
            self.category.label(),
            self.marker,
            self.category.advice()
        )
    }
}

/// A run of word characters in a text, with the line it is on and the sentence it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Word<'t> {
    text: &'t str,
    line: usize,
    /// How many sentence marks ([`SENTENCE_MARKS`]) come before it in the text, so two words
    /// with the same count are in one sentence.
    sentence: usize,
}

/// The characters that end a sentence or a clause, for the reach of a negation: a negation in an
/// earlier sentence never cancels a taking phrase. A mark that ends no sentence (`e.g.`, a path's
/// `.rs`) only shortens a negation's reach, so the scan errs towards a hit.
const SENTENCE_MARKS: [char; 4] = ['.', '!', '?', ';'];

/// The runs of characters in `text` that `inside` accepts, each with its line and sentence.
fn runs(text: &str, inside: impl Fn(char) -> bool) -> Vec<Word<'_>> {
    let mut out = Vec::new();
    let mut line = 1_usize;
    let mut sentence = 0_usize;
    let mut start: Option<(usize, usize, usize)> = None;
    for (at, c) in text.char_indices() {
        if inside(c) {
            start.get_or_insert((at, line, sentence));
        } else {
            if let Some((from, on, within)) = start.take()
                && let Some(word) = text.get(from..at)
            {
                out.push(Word {
                    text: word,
                    line: on,
                    sentence: within,
                });
            }
            if c == '\n' {
                line = line.saturating_add(1);
            }
            if SENTENCE_MARKS.contains(&c) {
                sentence = sentence.saturating_add(1);
            }
        }
    }
    if let Some((from, on, within)) = start
        && let Some(word) = text.get(from..)
    {
        out.push(Word {
            text: word,
            line: on,
            sentence: within,
        });
    }
    out
}

/// Every word of `words` split into its identifier parts ([`identifier_parts`]), each keeping
/// its word's line and sentence, so `numerical_recipes` and `takenFromCgal` read as phrases.
fn parts_of<'t>(words: &[Word<'t>]) -> Vec<Word<'t>> {
    words
        .iter()
        .flat_map(|w| {
            identifier_parts(w.text).into_iter().map(move |part| Word {
                text: part,
                line: w.line,
                sentence: w.sentence,
            })
        })
        .collect()
}

/// Whether `c` is a word character: a letter, a digit or `_`.
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The parts of an identifier: split at `_` and where a lower-case letter or digit is followed by
/// a capital (`smosPythag` is `smos`, `Pythag`; `tred2` stays whole).
#[must_use]
pub fn identifier_parts(word: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for piece in word.split('_').filter(|p| !p.is_empty()) {
        let mut start = 0_usize;
        let mut previous: Option<char> = None;
        for (at, c) in piece.char_indices() {
            if c.is_uppercase()
                && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit())
                && let Some(part) = piece.get(start..at)
            {
                out.push(part);
                start = at;
            }
            previous = Some(c);
        }
        if let Some(part) = piece.get(start..) {
            out.push(part);
        }
    }
    out
}

/// Every decimal number literal in `text`, as the double it denotes, with its literal and line:
/// digits, an optional fraction, an optional exponent, `_` separators and any suffix (`f64`, C's
/// `f` or `L`), starting where no letter, digit, `_` or `.` comes just before; a literal may open
/// with its point (`.000189`, as C writes it). Hexadecimal float literals (C's `0x1.8p-13`) are not
/// read.
fn numbers(text: &str) -> Vec<(f64, &str, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut line = 1_usize;
    let mut at = 0_usize;
    while let Some(&b) = bytes.get(at) {
        if b == b'\n' {
            line = line.saturating_add(1);
        }
        let opens_with_point =
            b == b'.' && bytes.get(at.saturating_add(1)).is_some_and(u8::is_ascii_digit);
        let starts = (b.is_ascii_digit() || opens_with_point)
            && at
                .checked_sub(1)
                .and_then(|p| bytes.get(p))
                .is_none_or(|&p| !(p.is_ascii_alphanumeric() || p == b'_' || p == b'.'));
        if !starts {
            at = at.saturating_add(1);
            continue;
        }
        let digits = |mut k: usize| {
            while bytes
                .get(k)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'_')
            {
                k = k.saturating_add(1);
            }
            k
        };
        let mut end = digits(at);
        if bytes.get(end) == Some(&b'.') && bytes.get(end.saturating_add(1)).is_some_and(u8::is_ascii_digit) {
            end = digits(end.saturating_add(1));
        }
        if bytes.get(end).is_some_and(|c| *c == b'e' || *c == b'E') {
            let mut k = end.saturating_add(1);
            if bytes.get(k).is_some_and(|c| *c == b'+' || *c == b'-') {
                k = k.saturating_add(1);
            }
            if bytes.get(k).is_some_and(u8::is_ascii_digit) {
                end = digits(k);
            }
        }
        while end > at && bytes.get(end.saturating_sub(1)) == Some(&b'_') {
            end = end.saturating_sub(1);
        }
        if let Some(literal) = text.get(at..end) {
            let mut cleaned: String = literal.chars().filter(|c| *c != '_').collect();
            if cleaned.starts_with('.') {
                cleaned.insert(0, '0');
            }
            if let Ok(value) = cleaned.parse::<f64>() {
                out.push((value, literal, line));
            }
        }
        at = end.max(at.saturating_add(1));
    }
    out
}

/// Where a taking phrase that starts at word `at` ends (the index after its last word), if one
/// does: a one-word phrase is that word; a two-word phrase is its first word followed, within
/// [`markers::TAKING_GAP`] words, by its second, so "taken, with changes, from" is still "taken
/// from".
fn taking_end(words: &[String], at: usize) -> Option<usize> {
    let first = words.get(at)?;
    markers::TAKING.iter().find_map(|phrase| match phrase {
        [only] => (first == only).then(|| at.saturating_add(1)),
        [head, tail] if first == head => (1..=markers::TAKING_GAP).find_map(|gap| {
            let k = at.saturating_add(gap);
            words
                .get(k)
                .is_some_and(|w| w == tail)
                .then(|| k.saturating_add(1))
        }),
        _ => None,
    })
}

/// Whether the lower-cased words starting at `at` are `phrase`.
fn phrase_at(words: &[String], at: usize, phrase: &[&str]) -> bool {
    phrase
        .iter()
        .enumerate()
        .all(|(k, p)| words.get(at.saturating_add(k)).is_some_and(|w| w == p))
}

/// Every marker in `text`, by line, category and marker.
#[must_use]
pub fn scan_text(text: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    let words = runs(text, word_char);
    let lower: Vec<String> = words.iter().map(|w| w.text.to_lowercase()).collect();

    for (k, word) in words.iter().enumerate() {
        let low = lower.get(k).map_or("", String::as_str);
        let parts = identifier_parts(word.text);
        let mut nr = markers::NR_MACROS_EXACT.contains(&word.text);
        nr |= markers::NR_VARIABLES.contains(&low);
        nr |= parts
            .iter()
            .any(|part| markers::NR_VARIABLES.contains(&part.to_lowercase().as_str()));
        nr |= parts.iter().any(|part| is_nr_name(part));
        if nr {
            hits.push(hit(word.line, Category::NumericalRecipes, word.text));
        }
        if is_acad_iso(low) {
            hits.push(hit(word.line, Category::AcadPattern, word.text));
        }
    }

    // Phrases, on the words as written and again on their identifier parts, so a phrase written as
    // one identifier (`numerical_recipes`, `takenFromCgal`) is found too.
    phrases(&words, &mut hits);
    phrases(&parts_of(&words), &mut hits);

    for term in runs(text, |c| word_char(c) || c == '-') {
        let bare = term.text.trim_matches(['-', '_']).replace('_', "-");
        if markers::ACAD_PATTERNS
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&bare))
        {
            hits.push(hit(
                term.line,
                Category::AcadPattern,
                term.text.trim_matches(['-', '_']),
            ));
        }
    }

    for marks in [CommentMarks::Spaced, CommentMarks::Any] {
        let (flat, lines) = flatten(text, marks);
        for description in markers::ACAD_DESCRIPTIONS {
            let (wanted, _) = flatten(description, marks);
            let mut from = 0_usize;
            while let Some(found) = flat.get(from..).and_then(|rest| rest.find(&wanted)) {
                let at = from.saturating_add(found);
                let line = lines.get(at).copied().unwrap_or(1);
                hits.push(hit(line, Category::AcadPattern, description));
                from = at.saturating_add(wanted.len().max(1));
            }
        }
    }

    for (value, literal, line) in numbers(text) {
        if markers::NR_VALUES.contains(&value) {
            hits.push(hit(line, Category::NumericalRecipes, literal));
        }
    }

    hits.sort();
    hits.dedup();
    hits
}

/// A hit.
fn hit(line: usize, category: Category, marker: &str) -> Hit {
    Hit {
        line,
        category,
        marker: marker.to_owned(),
    }
}

/// Every phrase marker in `words`: the book's name and authors, copyleft notices, and taking
/// phrases followed by a copyleft source's name, each found at the word it starts on.
fn phrases(words: &[Word<'_>], hits: &mut Vec<Hit>) {
    let lower: Vec<String> = words.iter().map(|w| w.text.to_lowercase()).collect();
    for (k, word) in words.iter().enumerate() {
        let phrase_hit = |list: &[&[&str]]| {
            list.iter()
                .find(|p| phrase_at(&lower, k, p))
                .map(|p| join_words(words, k, p.len()))
        };
        if let Some(found) = phrase_hit(&markers::NR_PHRASES) {
            hits.push(hit(word.line, Category::NumericalRecipes, &found));
        }
        if let Some(found) = phrase_hit(&markers::COPYLEFT_NOTICES) {
            hits.push(hit(word.line, Category::CopyleftNotice, &found));
        }
        if let Some(after) = taking_end(&lower, k) {
            let source = (after..after.saturating_add(markers::WINDOW)).find_map(|s| {
                markers::COPYLEFT_SOURCES
                    .iter()
                    .find(|p| phrase_at(&lower, s, p))
                    .map(|p| (s, p.len()))
            });
            if !negates(words, &lower, k)
                && let Some((s, len)) = source
            {
                let span = s.saturating_add(len).saturating_sub(k);
                hits.push(hit(
                    word.line,
                    Category::TakenFromCopyleft,
                    &join_words(words, k, span),
                ));
            }
        }
    }
}

/// The prefixes of the book's single- and double-precision variants of a routine (`dpythag`,
/// `spythag`), which a part may carry before the routine's name.
const PRECISION_PREFIXES: [char; 2] = ['d', 's'];

/// Whether an identifier part is a Numerical Recipes routine or macro name, ignoring case: the
/// part itself, the part without trailing digits (`tqli2` is `tqli`; `tred2` is itself a name),
/// or either without a precision prefix (`dpythag` is `pythag`).
fn is_nr_name(part: &str) -> bool {
    let lower = part.to_lowercase();
    let stem = lower.trim_end_matches(|c: char| c.is_ascii_digit()).to_owned();
    let mut names = vec![lower.clone(), stem.clone()];
    for name in [&lower, &stem] {
        if let Some(rest) = name.strip_prefix(PRECISION_PREFIXES) {
            names.push(rest.to_owned());
        }
    }
    names.iter().any(|name| {
        markers::NR_ROUTINES.contains(&name.as_str())
            || markers::NR_MACROS
                .iter()
                .any(|m| m.eq_ignore_ascii_case(name))
    })
}

/// Whether the taking phrase at word `at` is cancelled by a negation in the
/// [`markers::NEGATION_REACH`] words before it, in the same sentence ([`SENTENCE_MARKS`]), one not
/// followed by a limiting word such as "only" ("no code is taken from" is cancelled; "not only
/// taken from" is not, and neither is "No tests. Taken from").
fn negates(words: &[Word<'_>], lower: &[String], at: usize) -> bool {
    let sentence = words.get(at).map(|w| w.sentence);
    let start = at.saturating_sub(markers::NEGATION_REACH);
    (start..at).any(|k| {
        words.get(k).map(|w| w.sentence) == sentence
            && lower
                .get(k)
                .is_some_and(|w| markers::NEGATIONS.contains(&w.as_str()))
            && !lower
                .get(k.saturating_add(1))
                .is_some_and(|w| markers::NEGATION_LIMITS.contains(&w.as_str()))
    })
}

/// The punctuation that opens a comment or a quoted line in the languages and files the scan reads
/// (`//!`, `///`, `#`, `*`, `;`, `--`, `%`, `>`), which [`flatten`] drops from a line's start.
const COMMENT_MARKS: [char; 8] = ['/', '!', '#', '*', ';', '-', '%', '>'];

/// Which run of [`COMMENT_MARKS`] at a line's start [`flatten`] drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentMarks {
    /// Only a run followed by white space or the line's end, as comments are written (`// a`), so
    /// a line that opens with marker-like text (`/\/\/`) keeps it.
    Spaced,
    /// Any run, so a comment written without a space (`//a`) is dropped too.
    Any,
}

/// `text` lower-cased, with every run of white space and the comment marks at each line's start
/// (`marks`) read as one space, every run of backslashes read as one (a string literal's escape,
/// `\\`, reads as the backslash it writes), and the line of each byte of the result.
fn flatten(text: &str, marks: CommentMarks) -> (String, Vec<usize>) {
    let mut out = String::new();
    let mut lines = Vec::new();
    let mut line = 1_usize;
    let mut pending_space = false;
    for (k, raw) in text.lines().enumerate() {
        if k > 0 {
            line = line.saturating_add(1);
            pending_space = true;
        }
        let trimmed = raw.trim_start();
        let rest = trimmed.trim_start_matches(COMMENT_MARKS);
        let drop = match marks {
            CommentMarks::Any => true,
            CommentMarks::Spaced => rest.chars().next().is_none_or(char::is_whitespace),
        };
        let body = if drop { rest.trim_start() } else { trimmed };
        for c in body.chars() {
            if c.is_whitespace() {
                pending_space = true;
                continue;
            }
            if c == '\\' && !pending_space && out.ends_with('\\') {
                continue;
            }
            if pending_space && !out.is_empty() {
                out.push(' ');
                lines.push(line);
            }
            pending_space = false;
            for lower in c.to_lowercase() {
                out.push(lower);
                lines.resize(out.len(), line);
            }
        }
    }
    (out, lines)
}

/// The words `at .. at + count` of a text as it writes them, joined by spaces.
fn join_words(words: &[Word<'_>], at: usize, count: usize) -> String {
    words
        .get(at..at.saturating_add(count))
        .unwrap_or_default()
        .iter()
        .map(|w| w.text)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a lower-cased word is an ISO linetype or pattern name: `acad_iso`, two digits, `w100`.
fn is_acad_iso(word: &str) -> bool {
    let (prefix, suffix) = markers::ACAD_ISO;
    word.strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
        .is_some_and(|digits| digits.len() == 2 && digits.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_at_underscores_and_capitals() {
        assert_eq!(identifier_parts("ST_pythag"), ["ST", "pythag"]);
        assert_eq!(identifier_parts("smosPythag"), ["smos", "Pythag"]);
        assert_eq!(identifier_parts("tred2Step"), ["tred2", "Step"]);
        assert_eq!(identifier_parts("PythagoreanSum"), ["Pythagorean", "Sum"]);
        assert_eq!(identifier_parts("__x__"), ["x"]);
    }

    #[test]
    fn numbers_are_read_as_their_values() {
        let found: Vec<(f64, &str, usize)> = numbers("a = 1.89e-4;\nb=0.000_189 c1 x.5 2E+3_f64");
        assert_eq!(
            found,
            [
                (1.89e-4, "1.89e-4", 1),
                (0.000189, "0.000_189", 2),
                (2000.0, "2E+3", 2)
            ]
        );
    }

    #[test]
    fn runs_carry_their_lines() {
        let words = runs("a b\n\nc", word_char);
        let lines: Vec<(&str, usize)> = words.iter().map(|w| (w.text, w.line)).collect();
        assert_eq!(lines, [("a", 1), ("b", 1), ("c", 3)]);
    }

    #[test]
    fn iso_names_need_two_digits() {
        assert!(is_acad_iso("acad_iso02w100"));
        assert!(!is_acad_iso("acad_iso2w100"));
        assert!(!is_acad_iso("acad_proxy_entity"));
    }
}
