//! A family as a document holds it (CON-A7-F2-P3, piece A73, job 7.6): its recipe in the one
//! written form, the content id that names that version, and where it came from, so that a
//! document opens and its placed families are built again the same on a server that has no
//! library of its own. FORMAT.md beside this crate, "A family in a document"; the choices are
//! note CON-A7-F2-P3-family-load-and-reload.
//!
//! - [`StoredFamily`]: the recipe, its [`ContentId`] and its [`Source`], read back strictly
//!   ([`StoredFamily::read`] takes only what [`StoredFamily::new`] writes).
//! - [`read_text`] and [`loadable`]: what a recipe must pass before a document takes it: no more
//!   than [`MOST_RECIPE_BYTES`] of text, the reader, the checker against the project's
//!   categories and materials, and every type it names evaluated within [`Limits`] (its solids'
//!   boxes, without an engine, and for a hosted family its opening), so a family that would not
//!   build is refused when it is loaded, naming the type, never later at a placed copy.
//! - [`LoadRefusal`]: each refusal, in words.
//!
//! Promises: the same recipe and source always give the same stored text; nothing panics; no
//! clock, no random number, no hash-map order. How the record sits among a document's element
//! records is construct-regen's (`families`), and the commands that load, reload and swap are
//! construct-api's. Nothing here is interim.

use crate::{
    ContentId, EvalError, Hosting, Key, Library, Limits, Problem, ReadError, Recipe, Turn,
    TypeValues, extents, opening_within,
};
use kit_core::Id;
use std::fmt;

/// The most bytes of recipe text a document takes for one family (256 KiB): far above any
/// family of the library, and small enough to sit in one record of the store.
pub const MOST_RECIPE_BYTES: usize = 262_144;

/// Where a document's family came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// The library, loaded by the family's permanent id.
    Library(Id),
    /// Recipe text given to the document.
    Text,
    /// Made by the door and window generator from the template of this file stem
    /// (`single-swing-door`).
    Generated(String),
}

/// How a library source is written: `library:` and the family's id.
const LIBRARY: &str = "library:";
/// How a recipe given as text is written.
const TEXT: &str = "text";
/// How a generated source is written: `generated:` and the template's stem.
const GENERATED: &str = "generated:";

impl Source {
    /// Its one written form: `library:<id>`, `text` or `generated:<template>`.
    #[must_use]
    pub fn write(&self) -> String {
        match self {
            Self::Library(id) => format!("{LIBRARY}{id}"),
            Self::Text => TEXT.to_owned(),
            Self::Generated(stem) => format!("{GENERATED}{stem}"),
        }
    }

    /// The source `text` writes, if it is one.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        if text == TEXT {
            return Some(Self::Text);
        }
        if let Some(id) = text.strip_prefix(LIBRARY) {
            return Id::parse(id).ok().map(Self::Library);
        }
        let stem = text.strip_prefix(GENERATED)?;
        let plain = !stem.is_empty()
            && stem
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        plain.then(|| Self::Generated(stem.to_owned()))
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Library(id) => write!(f, "the library ({id})"),
            Self::Text => f.write_str("recipe text"),
            Self::Generated(stem) => write!(f, "the generator ({stem})"),
        }
    }
}

/// A family as a document holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredFamily {
    /// The recipe.
    pub recipe: Recipe,
    /// The content id of the recipe's written form: the version placed copies name.
    pub content: ContentId,
    /// Where it came from.
    pub source: Source,
}

impl StoredFamily {
    /// `recipe` from `source`, under its content id.
    #[must_use]
    pub fn new(recipe: Recipe, source: Source) -> Self {
        Self {
            content: recipe.content_id(),
            recipe,
            source,
        }
    }

    /// The recipe in its one written form, as the document stores it.
    #[must_use]
    pub fn text(&self) -> String {
        self.recipe.to_json()
    }

    /// The family a document stores as `text` (the recipe's written form), `content` (its
    /// content id) and `source` (the source's written form). Strict: the text must be the
    /// recipe's one written form, and the content id that of those bytes.
    ///
    /// # Errors
    /// Why not, in words.
    pub fn read(text: &str, content: &str, source: &str) -> Result<Self, String> {
        let recipe = Recipe::from_json(text)
            .map_err(|e| format!("its recipe does not read: {} at {}", e.reason(), e.at()))?;
        let family = Self::new(
            recipe,
            Source::read(source).ok_or_else(|| format!("{source} is not a family's source"))?,
        );
        if family.text() != text {
            return Err("its recipe is not in its one written form".to_owned());
        }
        if family.content.as_str() != content {
            return Err(format!(
                "its content id {content} is not that of its recipe, {}",
                family.content
            ));
        }
        Ok(family)
    }

    /// The keys of the types it names, in the recipe's order.
    #[must_use]
    pub fn type_keys(&self) -> Vec<Key> {
        self.recipe
            .named_types()
            .iter()
            .map(|t| t.key().clone())
            .collect()
    }
}

/// Why a document does not take a recipe.
#[derive(Clone, Debug)]
pub enum LoadRefusal {
    /// The text is longer than [`MOST_RECIPE_BYTES`].
    TooLong {
        /// Its length in bytes.
        bytes: usize,
    },
    /// The text is not a recipe.
    Unreadable(ReadError),
    /// The checker's problems, in document order.
    Problems(Vec<Problem>),
    /// It names no type, so nothing could be placed of it.
    NoTypes,
    /// A type does not evaluate within the limits, or its opening does not solve.
    Type {
        /// The type's key.
        key: Key,
        /// The evaluator's refusal.
        error: EvalError,
    },
}

impl fmt::Display for LoadRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { bytes } => write!(
                f,
                "the recipe is {bytes} bytes, over the {MOST_RECIPE_BYTES} a document takes"
            ),
            Self::Unreadable(e) => {
                write!(f, "the recipe does not read: {} at {}", e.reason(), e.at())
            }
            Self::Problems(problems) => {
                f.write_str("the recipe does not check: ")?;
                for (n, problem) in problems.iter().enumerate() {
                    if n > 0 {
                        f.write_str("; ")?;
                    }
                    write!(f, "{problem}")?;
                }
                Ok(())
            }
            Self::NoTypes => f.write_str("the recipe names no type"),
            Self::Type { key, error } => write!(f, "its type {key} does not build: {error}"),
        }
    }
}

/// The recipe `text` holds, if it is no longer than [`MOST_RECIPE_BYTES`] and reads.
///
/// # Errors
/// [`LoadRefusal::TooLong`] or [`LoadRefusal::Unreadable`].
pub fn read_text(text: &str) -> Result<Recipe, LoadRefusal> {
    if text.len() > MOST_RECIPE_BYTES {
        return Err(LoadRefusal::TooLong { bytes: text.len() });
    }
    Recipe::from_json(text).map_err(LoadRefusal::Unreadable)
}

/// Whether a document takes `recipe`: its written form within [`MOST_RECIPE_BYTES`], the
/// checker's rules against `library`, at least one type, and every type it names evaluated
/// within `limits` (each solid's box, and for a family hosted in a wall its opening).
///
/// # Errors
/// The first [`LoadRefusal`] in that order.
pub fn loadable(recipe: &Recipe, library: &Library<'_>, limits: Limits) -> Result<(), LoadRefusal> {
    let bytes = recipe.to_json().len();
    if bytes > MOST_RECIPE_BYTES {
        return Err(LoadRefusal::TooLong { bytes });
    }
    recipe.check(library).map_err(LoadRefusal::Problems)?;
    let types = recipe.named_types();
    if types.is_empty() {
        return Err(LoadRefusal::NoTypes);
    }
    for ty in types {
        let key = ty.key();
        let refused = |error: EvalError| LoadRefusal::Type {
            key: key.clone(),
            error,
        };
        let values = TypeValues::of(recipe, key).map_err(refused)?;
        extents(recipe, &values, limits, Turn::None).map_err(refused)?;
        if matches!(recipe.hosting, Hosting::Wall { .. }) {
            opening_within(recipe, &values, limits).map_err(refused)?;
        }
    }
    Ok(())
}
