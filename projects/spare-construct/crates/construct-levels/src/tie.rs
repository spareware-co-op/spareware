//! How an element is tied to a level: the level's permanent ID and an offset from it, in the
//! element record's `props`; and the one function that says how high a tied thing sits.
//!
//! An element's base is tied under [`LEVEL_KEY`] and [`LEVEL_OFFSET_KEY`], always together. A
//! wall's top may be tied too, under [`TOP_KEY`] and [`TOP_OFFSET_KEY`], to the level its top
//! follows. An element with no tie stands on the level its old integer names ([`crate::implied`]),
//! with no offset; an element with neither is on no level.

use crate::error::LevelError;
use crate::implied::implied_id;
use crate::level::Level;
use construct_model::element::Element;
use construct_params::{Length, Value};
use kit_core::Id;
use std::collections::BTreeMap;

/// On an element: the permanent ID of the level its base is tied to.
pub const LEVEL_KEY: &str = "@level";
/// On an element: its base's offset from that level, in construct-params' stored length text.
pub const LEVEL_OFFSET_KEY: &str = "@level.offset";
/// On a wall: the permanent ID of the level its top is tied to.
pub const TOP_KEY: &str = "@top.level";
/// On a wall: its top's offset from that level, in construct-params' stored length text.
pub const TOP_OFFSET_KEY: &str = "@top.offset";
/// Every `props` key a tie is written under.
pub const TIE_KEYS: [&str; 4] = [LEVEL_KEY, LEVEL_OFFSET_KEY, TOP_KEY, TOP_OFFSET_KEY];

/// A plan view's range: the level and the offset keys of its top, cut plane, bottom and view depth,
/// each a tie to a level. construct-views writes them (it reads this list as its own), and a
/// level a view's range is tied to is not deleted behind the view's back ([`crate::tied_to`]).
pub const RANGE_TIES: [(&str, &str); 4] = [
    ("@view.range.top", "@view.range.top.offset"),
    ("@view.range.cut", "@view.range.cut.offset"),
    ("@view.range.bottom", "@view.range.bottom.offset"),
    ("@view.range.depth", "@view.range.depth.offset"),
];

/// Whether a `props` key is one a tie is written under.
#[must_use]
pub fn is_tie_key(key: &str) -> bool {
    TIE_KEYS.contains(&key)
}

/// A tie to a level: the level, and how far above it (below, when negative) the tied thing is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tie {
    /// The level's permanent ID.
    pub level: Id,
    /// The offset from the level's elevation.
    pub offset: Length,
}

/// A tie under two keys of `props`, if it is there: both keys, or neither.
fn read(
    props: &BTreeMap<String, String>,
    key: &str,
    offset_key: &str,
) -> Result<Option<Tie>, LevelError> {
    let malformed = |why: &str| Err(LevelError::Malformed(format!("A tie under {key} {why}.")));
    match (props.get(key), props.get(offset_key)) {
        (None, None) => Ok(None),
        (Some(level), Some(offset)) => {
            let Ok(level) = Id::parse(level) else {
                return malformed("does not name a level by its ID");
            };
            match Value::from_stored(offset) {
                Some(Value::Length(length)) if Value::Length(length).stored() == *offset => {
                    Ok(Some(Tie {
                        level,
                        offset: length,
                    }))
                }
                _ => malformed("has no offset in its one spelling"),
            }
        }
        _ => malformed("has its level or its offset without the other"),
    }
}

/// Writes a tie under two keys of `props`, or takes both away.
fn write(props: &mut BTreeMap<String, String>, key: &str, offset_key: &str, tie: Option<&Tie>) {
    if let Some(tie) = tie {
        props.insert(key.to_owned(), tie.level.to_string());
        props.insert(offset_key.to_owned(), Value::Length(tie.offset).stored());
    } else {
        props.remove(key);
        props.remove(offset_key);
    }
}

/// The level an element's base is tied to: its tie, else the level its old integer names, else
/// none.
///
/// # Errors
/// [`LevelError::Malformed`] for a tie that is not as written.
pub fn base_of(element: &Element) -> Result<Option<Tie>, LevelError> {
    if let Some(tie) = read(&element.props, LEVEL_KEY, LEVEL_OFFSET_KEY)? {
        return Ok(Some(tie));
    }
    Ok(implied_id(element.level).map(|level| Tie {
        level,
        offset: Length::from_steps(0),
    }))
}

/// The level an element's top is tied to, if it has a top tie.
///
/// # Errors
/// [`LevelError::Malformed`] for a tie that is not as written.
pub fn top_of(element: &Element) -> Result<Option<Tie>, LevelError> {
    read(&element.props, TOP_KEY, TOP_OFFSET_KEY)
}

/// Whether any of an element's range ties ([`RANGE_TIES`]), the ones a plan view has, is to the level
/// `id`.
///
/// # Errors
/// [`LevelError::Malformed`] for a tie that is not as written.
pub fn range_tied_to(element: &Element, id: &Id) -> Result<bool, LevelError> {
    for (key, offset_key) in RANGE_TIES {
        if read(&element.props, key, offset_key)?.is_some_and(|tie| tie.level == *id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Ties an element's base in its `props`, or unties it with `None`.
pub fn write_base(props: &mut BTreeMap<String, String>, tie: Option<&Tie>) {
    write(props, LEVEL_KEY, LEVEL_OFFSET_KEY, tie);
}

/// Ties a wall's top in its `props`, or unties it with `None`.
pub fn write_top(props: &mut BTreeMap<String, String>, tie: Option<&Tie>) {
    write(props, TOP_KEY, TOP_OFFSET_KEY, tie);
}

/// How high above the project's zero something tied to `level` with `offset` sits: the level's
/// elevation plus the offset. Everything that asks how high a tied thing is asks this.
///
/// # Errors
/// [`LevelError::OutOfRange`] when the sum does not fit a length.
pub fn height_of(level: &Level, offset: Length) -> Result<Length, LevelError> {
    level
        .elevation()
        .checked_add(offset)
        .ok_or_else(|| LevelError::OutOfRange(level.name().to_owned()))
}

/// The height from one tie up to another: a wall from its base to its top, each through
/// [`height_of`]. Negative when the top is below the base.
///
/// # Errors
/// [`LevelError::OutOfRange`] when a height or the difference does not fit a length.
pub fn span(base: (&Level, Length), top: (&Level, Length)) -> Result<Length, LevelError> {
    let (from, to) = (height_of(base.0, base.1)?, height_of(top.0, top.1)?);
    to.checked_sub(from)
        .ok_or_else(|| LevelError::OutOfRange(top.0.name().to_owned()))
}
