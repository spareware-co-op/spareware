//! A system type's own data: one optional block on a type, owned by the crate that knows what it
//! means (walls, floors, roofs, families). This crate stores it, returns it and compares it byte
//! for byte, and never reads inside it.

use crate::error::TypeError;

/// The longest owner tag.
const MAX_OWNER: usize = 32;

/// An owner's data on a type: a short fixed tag naming the owning crate's kind of data, such as
/// `wall`, and that owner's canonical text. The owner keeps its text canonical; equal text is
/// equal data here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemData {
    owner: String,
    data: String,
}

impl SystemData {
    /// A block from its owner tag and its data.
    ///
    /// # Errors
    /// [`TypeError::BadSystem`] if the tag is not 1 to 32 characters of lowercase ASCII letters,
    /// digits and `-` starting with a letter, or the data is empty.
    pub fn new(owner: &str, data: &str) -> Result<Self, TypeError> {
        let mut chars = owner.chars();
        let starts = chars.next().is_some_and(|c| c.is_ascii_lowercase());
        let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !starts || !rest || owner.len() > MAX_OWNER {
            return Err(TypeError::BadSystem(format!(
                "owner tag {owner:?} is not 1 to 32 of a-z, 0-9 and -, starting with a letter"
            )));
        }
        if data.is_empty() {
            return Err(TypeError::BadSystem(format!(
                "the {owner:?} block has no data"
            )));
        }
        Ok(Self {
            owner: owner.to_owned(),
            data: data.to_owned(),
        })
    }

    /// The owner tag.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The owner's data, exactly as given.
    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }
}
