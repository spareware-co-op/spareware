//! A project's category table: the built-in tree plus the project's own subcategories.
//!
//! Only the project's own subcategories are stored; the built-in tree is in code. A table cannot
//! remove or rename a built-in, because no function here does either. Every change returns a new
//! table, so the caller (later, the edit log) decides when to keep it.

use crate::builtin::{BUILT_IN, BuiltIn, FIRST_USER_ID};
use crate::{CategoryId, CategoryKind};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What a lookup returns about one category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// Its permanent id.
    pub id: CategoryId,
    /// The name people see.
    pub name: String,
    /// What sort of thing it holds.
    pub kind: CategoryKind,
    /// The category it is a subcategory of; `None` for a top-level category.
    pub parent: Option<CategoryId>,
    /// Whether a view can cut elements of it. A project subcategory takes its parent's.
    pub cuttable: bool,
    /// True for the fixed tree, false for a project's own subcategory.
    pub built_in: bool,
}

impl From<&BuiltIn> for Category {
    fn from(b: &BuiltIn) -> Self {
        Self {
            id: b.id,
            name: b.name.to_string(),
            kind: b.kind,
            parent: b.parent,
            cuttable: b.cuttable,
            built_in: true,
        }
    }
}

/// Why a category request or a stored table was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CategoryError {
    /// No element kind has this name.
    UnknownKind(String),
    /// No category has this id.
    UnknownCategory(CategoryId),
    /// A subcategory can be added only under a built-in top-level category.
    ParentNotTopLevel(CategoryId),
    /// A name must have something in it besides spaces.
    EmptyName,
    /// The parent already has a subcategory of this name (case does not matter).
    DuplicateName {
        /// The parent.
        parent: CategoryId,
        /// The name asked for.
        name: String,
    },
    /// Every project id has been used.
    TableFull,
    /// The stored table is not a version this build reads.
    UnsupportedFormat(u32),
    /// The stored table is not valid; the text says why.
    Malformed(String),
}

impl fmt::Display for CategoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKind(k) => write!(f, "no element kind is named {k:?}"),
            Self::UnknownCategory(id) => write!(f, "no category has id {}", id.0),
            Self::ParentNotTopLevel(id) => {
                write!(f, "category {} is not a built-in top-level category", id.0)
            }
            Self::EmptyName => write!(f, "a subcategory needs a name"),
            Self::DuplicateName { parent, name } => {
                write!(
                    f,
                    "category {} already has a subcategory named {name:?}",
                    parent.0
                )
            }
            Self::TableFull => write!(f, "the project has no subcategory ids left"),
            Self::UnsupportedFormat(v) => write!(f, "category table format {v} is not supported"),
            Self::Malformed(why) => write!(f, "category table is not valid: {why}"),
        }
    }
}

impl std::error::Error for CategoryError {}

/// The stored table format this build writes (FORMAT.md). Earlier formats are read for ever.
const FORMAT: u32 = 1;

/// One project subcategory, as stored.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Stored {
    id: CategoryId,
    parent: CategoryId,
    name: String,
}

/// The stored form: field order here is the byte order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: u32,
    subcategories: Vec<Stored>,
}

/// A project's categories: the built-in tree and the project's own subcategories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryTable {
    /// Ascending by id, so the stored form is canonical.
    user: Vec<Stored>,
}

fn built_in_top_level(id: CategoryId) -> Result<&'static BuiltIn, CategoryError> {
    match BUILT_IN.iter().find(|b| b.id == id) {
        Some(b) if b.parent.is_none() => Ok(b),
        Some(_) => Err(CategoryError::ParentNotTopLevel(id)),
        None if id.0 >= FIRST_USER_ID => Err(CategoryError::ParentNotTopLevel(id)),
        None => Err(CategoryError::UnknownCategory(id)),
    }
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl CategoryTable {
    /// A project with no subcategories of its own.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One category by id: built-in or the project's own.
    #[must_use]
    pub fn get(&self, id: CategoryId) -> Option<Category> {
        if let Some(b) = BUILT_IN.iter().find(|b| b.id == id) {
            return Some(Category::from(b));
        }
        self.user
            .iter()
            .find(|u| u.id == id)
            .and_then(Self::user_category)
    }

    fn user_category(u: &Stored) -> Option<Category> {
        let parent = BUILT_IN.iter().find(|b| b.id == u.parent)?;
        Some(Category {
            id: u.id,
            name: u.name.clone(),
            kind: parent.kind,
            parent: Some(u.parent),
            cuttable: parent.cuttable,
            built_in: false,
        })
    }

    /// The subcategories of a category: built-in ones in tree order, then the project's in id
    /// order. Empty for an id that has none, or none at all.
    #[must_use]
    pub fn children(&self, parent: CategoryId) -> Vec<Category> {
        let built = BUILT_IN
            .iter()
            .filter(|b| b.parent == Some(parent))
            .map(Category::from);
        let own = self
            .user
            .iter()
            .filter(|u| u.parent == parent)
            .filter_map(Self::user_category);
        built.chain(own).collect()
    }

    /// Every category: the built-in tree in order, then the project's own by id.
    #[must_use]
    pub fn all(&self) -> Vec<Category> {
        BUILT_IN
            .iter()
            .map(Category::from)
            .chain(self.user.iter().filter_map(Self::user_category))
            .collect()
    }

    /// The id the next project subcategory takes: one past the highest used, never reused.
    ///
    /// # Errors
    ///
    /// [`CategoryError::TableFull`] when the ids have run out.
    pub fn next_id(&self) -> Result<CategoryId, CategoryError> {
        match self.user.last() {
            None => Ok(CategoryId(FIRST_USER_ID)),
            Some(last) => last
                .id
                .0
                .checked_add(1)
                .map(CategoryId)
                .ok_or(CategoryError::TableFull),
        }
    }

    /// The table with one more subcategory under a built-in top-level category, and its id. The
    /// name is trimmed and must be new among the parent's subcategories, built-in and the
    /// project's, compared without regard to case. This table is left as it was.
    ///
    /// # Errors
    ///
    /// Any [`CategoryError`] for a bad parent, an empty name, or a repeated name.
    pub fn add_subcategory(
        &self,
        parent: CategoryId,
        name: &str,
    ) -> Result<(Self, CategoryId), CategoryError> {
        built_in_top_level(parent)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(CategoryError::EmptyName);
        }
        if self
            .children(parent)
            .iter()
            .any(|c| same_name(&c.name, name))
        {
            return Err(CategoryError::DuplicateName {
                parent,
                name: name.to_string(),
            });
        }
        let id = self.next_id()?;
        let mut next = self.clone();
        next.user.push(Stored {
            id,
            parent,
            name: name.to_string(),
        });
        Ok((next, id))
    }

    /// The table's stored bytes: the same table always gives the same bytes (FORMAT.md).
    ///
    /// # Errors
    ///
    /// [`CategoryError::Malformed`] if the text cannot be written, which a valid table never hits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CategoryError> {
        let wire = Wire {
            format: FORMAT,
            subcategories: self.user.clone(),
        };
        serde_json::to_vec(&wire).map_err(|e| CategoryError::Malformed(e.to_string()))
    }

    /// Reads a stored table. Every rule of [`Self::add_subcategory`] is checked again, and ids
    /// must run on from [`FIRST_USER_ID`] with no gap, so stored bytes cannot break the tree.
    ///
    /// # Errors
    ///
    /// [`CategoryError::UnsupportedFormat`] for a format this build does not know, otherwise
    /// [`CategoryError::Malformed`] or the rule that the entry broke.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CategoryError> {
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|e| CategoryError::Malformed(e.to_string()))?;
        if wire.format != FORMAT {
            return Err(CategoryError::UnsupportedFormat(wire.format));
        }
        let mut table = Self::new();
        for entry in wire.subcategories {
            let expected = table.next_id()?;
            if entry.id != expected {
                return Err(CategoryError::Malformed(format!(
                    "id {} should be {}",
                    entry.id.0, expected.0
                )));
            }
            if entry.name != entry.name.trim() {
                return Err(CategoryError::Malformed(format!(
                    "name {:?} is not trimmed",
                    entry.name
                )));
            }
            table = table.add_subcategory(entry.parent, &entry.name)?.0;
        }
        Ok(table)
    }
}
