//! IFC `GlobalId`s made from our own IDs, the same on every export.
//!
//! An IFC `GlobalId` is a 128-bit number written in 22 characters of a 64-character alphabet (the
//! compressed form of ISO 16739-1, section on IfcGloballyUniqueId): the first character holds the
//! top two bits, each of the other 21 holds six. Ours is the first 16 bytes of the SHA-256 of a
//! seed made of what the thing is and its permanent ID, so it needs no random number and no clock,
//! and an element keeps its `GlobalId` through every export, edit and undo.

use sha2::{Digest, Sha256};

/// The 64 characters, in value order.
const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// The `GlobalId` of the thing `seed` names.
#[must_use]
pub fn global_id(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    // A SHA-256 digest is 32 bytes, so its first 16 are always there.
    let mut number = digest
        .first_chunk::<16>()
        .map_or(0, |bytes| u128::from_be_bytes(*bytes));
    let mut out = [b'0'; 22];
    // The last 21 characters take six bits each, from the bottom; the first takes what is left,
    // the top two bits.
    for slot in out.iter_mut().skip(1).rev() {
        let six = usize::try_from(number & 63).unwrap_or(0);
        *slot = ALPHABET.get(six).copied().unwrap_or(b'0');
        number = number.checked_shr(6).unwrap_or(0);
    }
    let two = usize::try_from(number & 3).unwrap_or(0);
    if let Some(first) = out.first_mut() {
        *first = ALPHABET.get(two).copied().unwrap_or(b'0');
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::global_id;

    /// 22 characters, the first one of the four a two-bit value allows, all from the alphabet.
    #[test]
    fn a_global_id_has_the_published_shape() {
        for seed in ["wall:doc:W-s", "storey:doc:L-1", "", "x"] {
            let id = global_id(seed);
            assert_eq!(id.len(), 22);
            assert!(id.starts_with(['0', '1', '2', '3']), "{id}");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
            );
        }
    }

    /// The same seed gives the same id and another seed another.
    #[test]
    fn a_global_id_is_stable_and_distinct() {
        assert_eq!(global_id("wall:doc:W-s"), global_id("wall:doc:W-s"));
        assert_ne!(global_id("wall:doc:W-s"), global_id("wall:doc:W-n"));
    }
}
