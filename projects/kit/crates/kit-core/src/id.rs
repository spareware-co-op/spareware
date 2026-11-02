//! Permanent IDs.
//!
//! An ID is a short piece of plain text, safe inside a storage key, a file path and a web address
//! without escaping. Nothing that has been stored under an ID may ever change meaning, so these
//! rules are part of the published store format (kit-store's FORMAT.md).
//!
//! New IDs are 128 bits: 48 of time and 80 of randomness, written in 26 characters of Crockford's
//! base 32, the layout ULID uses. Sorted as text they sort by time, and two machines that have
//! never spoken can't pick the same one, so offline work never collides. The random bytes are the
//! caller's to supply: this crate has no dependencies, so no random number source of its own.

use std::fmt;
use std::str::FromStr;

/// The longest ID. A SHA-256 hash in hexadecimal is exactly this long.
const MAX_LEN: usize = 64;
/// How many characters a new ID has.
const NEW_LEN: usize = 26;
/// Crockford's base 32: no I, L, O or U, so an ID read aloud or by eye can't be misread.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A checked permanent ID. The only way to make one is through these checks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(String);

/// Why some text isn't an ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdError {
    /// There was no text.
    Empty,
    /// Longer than 64 characters.
    TooLong,
    /// It must start with a letter or digit, and hold only letters, digits, `.`, `_` and `-`.
    BadCharacter,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "an ID can't be empty",
            Self::TooLong => "an ID can't be longer than 64 characters",
            Self::BadCharacter => "an ID holds only letters, digits, dots, dashes and underscores",
        })
    }
}

impl std::error::Error for IdError {}

impl Id {
    /// Checks text from outside and makes an ID of it.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let first = text.chars().next().ok_or(IdError::Empty)?;
        if text.len() > MAX_LEN {
            return Err(IdError::TooLong);
        }
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        if !first.is_ascii_alphanumeric() || !text.chars().all(plain) {
            return Err(IdError::BadCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    /// A new 128-bit ID from the time in milliseconds since 1970 and ten random bytes.
    #[must_use]
    pub fn from_time_and_random(unix_ms: u64, random: [u8; 10]) -> Self {
        let mut number = u128::from(unix_ms & 0xFFFF_FFFF_FFFF) << 80;
        for (index, byte) in random.iter().enumerate() {
            number |= u128::from(*byte) << (8 * (9 - index));
        }
        let mut text = [0u8; NEW_LEN];
        for slot in text.iter_mut().rev() {
            let digit = (number & 31) as usize;
            *slot = ALPHABET.get(digit).copied().unwrap_or(b'0');
            number >>= 5;
        }
        Self(text.iter().map(|byte| char::from(*byte)).collect())
    }

    /// The ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Id {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, IdError> {
        Self::parse(text)
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_and_hashes_are_ids() {
        assert!(Id::parse("anna").is_ok());
        assert!(Id::parse("company-1").is_ok());
        assert!(Id::parse("arch-package-v12").is_ok());
        assert!(Id::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn anything_that_could_break_a_key_or_a_path_is_refused() {
        assert_eq!(Id::parse(""), Err(IdError::Empty));
        assert_eq!(Id::parse(&"a".repeat(65)), Err(IdError::TooLong));
        for bad in [
            "a/b", "a:b", "..", ".hidden", "-x", "a b", "a\nb", "é", "a\0",
        ] {
            assert_eq!(Id::parse(bad), Err(IdError::BadCharacter), "{bad:?}");
        }
    }

    #[test]
    fn a_new_id_is_a_valid_id_of_26_characters() {
        let id = Id::from_time_and_random(1_793_413_539_019, [7; 10]);
        assert_eq!(id.as_str().len(), NEW_LEN);
        assert_eq!(Id::parse(id.as_str()).as_ref(), Ok(&id));
    }

    #[test]
    fn new_ids_sort_by_time_and_differ_by_randomness() {
        let early = Id::from_time_and_random(1000, [255; 10]);
        let late = Id::from_time_and_random(1001, [0; 10]);
        assert!(early < late);
        assert_ne!(
            Id::from_time_and_random(5, [1; 10]),
            Id::from_time_and_random(5, [2; 10])
        );
    }

    #[test]
    fn the_layout_of_a_new_id_never_changes() {
        // Recorded: IDs already stored must keep their meaning.
        let id = Id::from_time_and_random(0x0123_4567_89AB, [0xFF; 10]);
        assert_eq!(id.as_str(), "014D2PF2DBZZZZZZZZZZZZZZZZ");
    }
}
