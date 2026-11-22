//! Names: what a grid or a reference plane may be called, how two names are compared, and the
//! rule that suggests the next one.
//!
//! **Comparing.** Two names are the same when their lower-case forms are equal, so `A` and `a`
//! are one name ([`same_name`]). The name itself keeps the case it was written in.
//!
//! **Suggesting.** [`next_name`] gives the name that follows one, by the first of these rules
//! that fits, always working on the end of the name and leaving what comes before it as it is:
//!
//! 1. A trailing run of digits goes up by one, keeping its width. `1` gives `2`, `9` gives `10`,
//!    `09` gives `10`, `009` gives `010`, `A-3` gives `A-4`.
//! 2. Otherwise a trailing run of letters of one case goes up in the order a, b, ..., z, aa, ab,
//!    ..., az, ba, ..., zz, aaa, in the case it has. `A` gives `B`, `Z` gives `AA`, `AZ` gives
//!    `BA`, `zz` gives `aaa`, `1Z` gives `1AA`.
//! 3. Otherwise (an empty name, or one ending in a symbol) `1` is added at the end: `A-` gives
//!    `A-1`.
//!
//! No letter is skipped, so `I` and `O` are names like any other.

use crate::GridError;

/// The most characters a name may have.
pub const MAX_NAME_CHARS: usize = 64;

/// A name as it is kept: trimmed, not empty, at most [`MAX_NAME_CHARS`] characters, no control
/// characters.
///
/// # Errors
///
/// [`GridError::EmptyName`] for a name of spaces only; [`GridError::BadName`] for one too long or
/// holding a control character.
pub fn clean_name(name: &str) -> Result<String, GridError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(GridError::EmptyName);
    }
    if name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(GridError::BadName);
    }
    Ok(name.to_owned())
}

/// Whether two names are one name: equal without regard to case.
#[must_use]
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Adds one to a run of ASCII digits, keeping its width: `009` is `010`, `99` is `100`.
fn count_up(digits: &str) -> String {
    let mut out: Vec<u8> = digits.bytes().collect();
    let mut carried = true;
    for digit in out.iter_mut().rev() {
        if !carried {
            break;
        }
        if *digit == b'9' {
            *digit = b'0';
        } else {
            // A digit below nine: one more is still a digit.
            *digit = digit.saturating_add(1);
            carried = false;
        }
    }
    let mut text = String::from_utf8(out).unwrap_or_default();
    if carried {
        text.insert(0, '1');
    }
    text
}

/// Adds one to a run of ASCII letters of one case: `Z` is `AA`, `AZ` is `BA`.
fn letters_up(letters: &str) -> String {
    let mut out: Vec<u8> = letters.bytes().collect();
    let (first, last) = if letters.bytes().all(|b| b.is_ascii_uppercase()) {
        (b'A', b'Z')
    } else {
        (b'a', b'z')
    };
    let mut carried = true;
    for letter in out.iter_mut().rev() {
        if !carried {
            break;
        }
        if *letter == last {
            *letter = first;
        } else {
            *letter = letter.saturating_add(1);
            carried = false;
        }
    }
    let mut text = String::from_utf8(out).unwrap_or_default();
    if carried {
        text.insert(0, char::from(first));
    }
    text
}

/// The split of a name into what comes before its last run of one sort of character and that
/// run: `("A-", "3")` for `A-3`.
fn split_run(name: &str, belongs: fn(char) -> bool) -> (&str, &str) {
    let start = name
        .char_indices()
        .rev()
        .take_while(|(_, c)| belongs(*c))
        .last()
        .map_or(name.len(), |(at, _)| at);
    name.split_at(start)
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}

fn is_lower(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// The name that follows `name`, by the rules in this module's header. The same name always gives
/// the same answer.
#[must_use]
pub fn next_name(name: &str) -> String {
    let (head, digits) = split_run(name, is_digit);
    if !digits.is_empty() {
        return format!("{head}{}", count_up(digits));
    }
    for belongs in [is_upper as fn(char) -> bool, is_lower] {
        let (head, letters) = split_run(name, belongs);
        if !letters.is_empty() {
            return format!("{head}{}", letters_up(letters));
        }
    }
    format!("{name}1")
}

/// The first name from `after` on that `taken` does not hold, going by [`next_name`]; with no
/// `after`, `1`. `taken` says whether a name is in use (compare with [`same_name`]).
#[must_use]
pub fn suggest(after: Option<&str>, taken: impl Fn(&str) -> bool) -> String {
    let mut name = match after {
        Some(after) => next_name(after),
        None => "1".to_owned(),
    };
    // Each step gives a different name and the set is finite, so this ends; the bound keeps the
    // promise even for a `taken` that says yes to everything.
    for _ in 0..100_000 {
        if !taken(&name) {
            break;
        }
        name = next_name(&name);
    }
    name
}
