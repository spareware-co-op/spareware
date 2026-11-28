//! A wall's own choices for its joins and its wrapping (job 6.1, CON-A4-F2-P2 and P4), as its
//! record keeps them: whether joins are allowed at each of its ends, how each of its joints is
//! made, and its own wrapping over its type's. Pure data in one stored spelling each; the join
//! rule ([`crate::joins`]) reads them, construct-api's wall commands write them, and
//! construct-model's FORMAT.md ("Walls") says where they sit in the record (the layered wall's
//! recipe arguments [`JOINING`] and [`WRAPPING`]).
//!
//! - [`Joining`]: [`Joining::allowed`] at the start and at the end (both by default), and a
//!   [`Choice`] for each joint by the other wall's ID ([`Joining::with`]). A joint is two walls'
//!   and is kept once, by the wall whose ID is the lower of the two ([`keeper`]), so the two can
//!   never disagree; a choice naming a wall that no longer meets this one is not read.
//! - [`Choice`]: the joint's [`JoinKind`] (butt, mitre or square-off) and, for a butt or a
//!   square-off, which wall runs through ([`Choice::first`]).
//! - [`OwnWrap`]: the wall's own wrapping at its ends and at its inserts, each over its type's
//!   where it is given.
//!
//! Promises: [`Joining::to_json`] and [`OwnWrap::to_json`] write one spelling (keys sorted, as
//! `serde_json` writes a map), nothing for the defaults, and the readers refuse anything else.
//! Nothing here is interim.

use crate::error::WallError;
use crate::wrap::{AtEnds, AtInserts};
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeMap;

/// The layered wall's recipe argument that keeps its [`Joining`]; absent when it is the default.
pub const JOINING: &str = "joining";
/// The layered wall's recipe argument that keeps its [`OwnWrap`]; absent when it has none.
pub const WRAPPING: &str = "wrapping";

/// How a joint is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JoinKind {
    /// One wall's layers run through and the other's stop against them: where two layers of the
    /// same priority meet, the first wall's runs through. Every joint may be a butt; a T and a
    /// cross are only ever one.
    Butt,
    /// At a corner, layers of the same priority meet on the line from where their outer faces
    /// cross to where their inner faces cross: the default at a corner.
    Mitre,
    /// At a corner, a butt whose through wall ends square to its own line, past the corner's
    /// outer point, where every one of its layers runs through: at a right angle the same as a
    /// butt.
    SquareOff,
}

impl JoinKind {
    /// Every kind.
    pub const ALL: [Self; 3] = [Self::Butt, Self::Mitre, Self::SquareOff];

    /// Its stored name.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Butt => "butt",
            Self::Mitre => "mitre",
            Self::SquareOff => "square-off",
        }
    }

    /// The kind a stored name names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}

/// How one joint is made, as a person chose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// Its kind.
    pub kind: JoinKind,
    /// The wall that runs through, by ID: for a butt or a square-off at a corner, the one whose
    /// layers win where two of the same priority meet; at a cross, the one that is not cut, where
    /// both could be. `None` for a mitre, and read as the wall of the lower ID where it is wanted.
    pub first: Option<String>,
}

/// One of a wall's two ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WallEnd {
    /// Its start.
    Start,
    /// Its end.
    End,
}

impl WallEnd {
    /// Both, in order.
    pub const BOTH: [Self; 2] = [Self::Start, Self::End];

    /// Its stored name.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }

    /// The end a stored name names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::BOTH.into_iter().find(|end| end.key() == key)
    }

    /// Its place in a pair: 0 for the start, 1 for the end.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Start => 0,
            Self::End => 1,
        }
    }
}

/// A wall's own join choices. The default allows joins at both ends and chooses nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joining {
    /// Whether joins are allowed at its start and at its end.
    pub allowed: [bool; 2],
    /// Its choice for each joint it keeps, by the other wall's ID.
    pub with: BTreeMap<String, Choice>,
}

impl Default for Joining {
    fn default() -> Self {
        Self {
            allowed: [true, true],
            with: BTreeMap::new(),
        }
    }
}

/// Which of two walls keeps their joint's choice: the one whose ID is the lower.
#[must_use]
pub fn keeper<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a <= b { a } else { b }
}

fn malformed(what: &str, why: &str) -> WallError {
    WallError::Malformed(format!("{what}: {why}"))
}

impl Joining {
    /// Whether it is the default: nothing to keep.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether joins are allowed at `end`.
    #[must_use]
    pub fn allows(&self, end: WallEnd) -> bool {
        self.allowed.get(end.index()).copied().unwrap_or(true)
    }

    /// Its stored form: `{"disallow":[...],"with":{...}}`, each part only when it holds something
    /// (`disallow` the ends where joins are not allowed, start first; `with` each choice by the
    /// other wall's ID, `{"first":<ID>,"kind":<kind>}` with `first` only when it names one).
    /// `None` for the default.
    #[must_use]
    pub fn to_json(&self) -> Option<Json> {
        let mut out = Map::new();
        let disallowed: Vec<&str> = WallEnd::BOTH
            .into_iter()
            .filter(|&end| !self.allows(end))
            .map(WallEnd::key)
            .collect();
        if !disallowed.is_empty() {
            out.insert("disallow".to_owned(), json!(disallowed));
        }
        if !self.with.is_empty() {
            let mut with = Map::new();
            for (other, choice) in &self.with {
                let mut row = Map::new();
                if let Some(first) = &choice.first {
                    row.insert("first".to_owned(), json!(first));
                }
                row.insert("kind".to_owned(), json!(choice.kind.key()));
                with.insert(other.clone(), Json::Object(row));
            }
            out.insert("with".to_owned(), Json::Object(with));
        }
        (!out.is_empty()).then_some(Json::Object(out))
    }

    /// Reads [`Joining::to_json`]'s form; absent is the default.
    ///
    /// # Errors
    /// [`WallError::Malformed`] for anything that form would not be.
    pub fn from_json(value: Option<&Json>) -> Result<Self, WallError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let what = "a wall's joining";
        let object = value
            .as_object()
            .ok_or_else(|| malformed(what, "it is not an object"))?;
        let mut out = Self::default();
        for (key, part) in object {
            match key.as_str() {
                "disallow" => {
                    let ends = part
                        .as_array()
                        .ok_or_else(|| malformed(what, "disallow is not a list"))?;
                    for end in ends {
                        let end = end
                            .as_str()
                            .and_then(WallEnd::from_key)
                            .ok_or_else(|| malformed(what, "disallow names an end it has not"))?;
                        if let Some(slot) = out.allowed.get_mut(end.index()) {
                            *slot = false;
                        }
                    }
                }
                "with" => {
                    let rows = part
                        .as_object()
                        .ok_or_else(|| malformed(what, "with is not an object"))?;
                    for (other, row) in rows {
                        out.with.insert(other.clone(), choice(row, what)?);
                    }
                }
                other => return Err(malformed(what, &format!("{other} is not one of its parts"))),
            }
        }
        if out.to_json().as_ref() != Some(value) {
            return Err(malformed(what, "it is not in its one spelling"));
        }
        Ok(out)
    }

    /// The choice for the joint of walls `me` and `other` among `walls` (each wall's ID and
    /// joining): the one the keeper of the two holds ([`keeper`]).
    #[must_use]
    pub fn choice_of<'a>(
        me: (&'a str, &'a Joining),
        other: (&'a str, &'a Joining),
    ) -> Option<&'a Choice> {
        let ((a, ja), (b, jb)) = (me, other);
        if keeper(a, b) == a {
            ja.with.get(b)
        } else {
            jb.with.get(a)
        }
    }
}

fn choice(row: &Json, what: &str) -> Result<Choice, WallError> {
    let row = row
        .as_object()
        .ok_or_else(|| malformed(what, "a choice is not an object"))?;
    let kind = row
        .get("kind")
        .and_then(Json::as_str)
        .and_then(JoinKind::from_key)
        .ok_or_else(|| malformed(what, "a choice has no kind it knows"))?;
    let first = match row.get("first") {
        None => None,
        Some(Json::String(first)) => Some(first.clone()),
        Some(_) => return Err(malformed(what, "a choice's first is not an ID")),
    };
    if row.keys().any(|key| key != "kind" && key != "first") {
        return Err(malformed(what, "a choice has a part it does not know"));
    }
    Ok(Choice { kind, first })
}

/// A wall's own wrapping, each over its type's where it is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnWrap {
    /// At its free ends.
    pub ends: Option<AtEnds>,
    /// At its inserts.
    pub inserts: Option<AtInserts>,
}

impl OwnWrap {
    /// Its stored form, `{"ends":<choice>,"inserts":<choice>}` with each only when given; `None`
    /// when neither is.
    #[must_use]
    pub fn to_json(&self) -> Option<Json> {
        let mut out = Map::new();
        if let Some(ends) = self.ends {
            out.insert("ends".to_owned(), json!(ends.key()));
        }
        if let Some(inserts) = self.inserts {
            out.insert("inserts".to_owned(), json!(inserts.key()));
        }
        (!out.is_empty()).then_some(Json::Object(out))
    }

    /// Reads [`OwnWrap::to_json`]'s form; absent is none given.
    ///
    /// # Errors
    /// [`WallError::Malformed`] for anything that form would not be.
    pub fn from_json(value: Option<&Json>) -> Result<Self, WallError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let what = "a wall's own wrapping";
        let object = value
            .as_object()
            .ok_or_else(|| malformed(what, "it is not an object"))?;
        let text = |key: &str| object.get(key).map(|v| v.as_str().unwrap_or_default());
        let out = Self {
            ends: match text("ends") {
                None => None,
                Some(key) => Some(
                    AtEnds::from_key(key).ok_or_else(|| malformed(what, "ends is not a choice"))?,
                ),
            },
            inserts: match text("inserts") {
                None => None,
                Some(key) => Some(
                    AtInserts::from_key(key)
                        .ok_or_else(|| malformed(what, "inserts is not a choice"))?,
                ),
            },
        };
        if out.to_json().as_ref() != Some(value) {
            return Err(malformed(what, "it is not in its one spelling"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each form writes one spelling and reads back; the defaults write nothing; anything else is
    /// refused.
    #[test]
    fn choices_read_back_in_one_spelling() {
        let mut joining = Joining::default();
        assert_eq!(joining.to_json(), None);
        assert_eq!(Joining::from_json(None).unwrap(), joining);
        joining.allowed = [false, true];
        joining.with.insert(
            "w2".to_owned(),
            Choice {
                kind: JoinKind::Butt,
                first: Some("w2".to_owned()),
            },
        );
        joining.with.insert(
            "w3".to_owned(),
            Choice {
                kind: JoinKind::Mitre,
                first: None,
            },
        );
        let text = joining.to_json().unwrap();
        assert_eq!(
            text.to_string(),
            r#"{"disallow":["start"],"with":{"w2":{"first":"w2","kind":"butt"},"w3":{"kind":"mitre"}}}"#
        );
        assert_eq!(Joining::from_json(Some(&text)).unwrap(), joining);
        for bad in [
            json!({}),
            json!({"disallow": []}),
            json!({"disallow": ["middle"]}),
            json!({"with": {"w2": {"kind": "glued"}}}),
            json!({"with": {"w2": {"kind": "butt", "glue": 1}}}),
            json!({"order": 1}),
        ] {
            assert!(Joining::from_json(Some(&bad)).is_err(), "{bad}");
        }
        let own = OwnWrap {
            ends: Some(AtEnds::Exterior),
            inserts: None,
        };
        let text = own.to_json().unwrap();
        assert_eq!(text.to_string(), r#"{"ends":"exterior"}"#);
        assert_eq!(OwnWrap::from_json(Some(&text)).unwrap(), own);
        assert_eq!(OwnWrap::default().to_json(), None);
        for bad in [json!({}), json!({"ends": "both"}), json!({"inserts": 1})] {
            assert!(OwnWrap::from_json(Some(&bad)).is_err(), "{bad}");
        }
    }

    /// A joint's choice is the keeper's: the lower ID's, whichever wall asks.
    #[test]
    fn a_joint_is_kept_by_the_lower_id() {
        let mut a = Joining::default();
        a.with.insert(
            "b".to_owned(),
            Choice {
                kind: JoinKind::Butt,
                first: Some("b".to_owned()),
            },
        );
        let b = Joining::default();
        let from_a = Joining::choice_of(("a", &a), ("b", &b));
        let from_b = Joining::choice_of(("b", &b), ("a", &a));
        assert_eq!(from_a, from_b);
        assert_eq!(from_a.unwrap().kind, JoinKind::Butt);
        assert_eq!(keeper("b", "a"), "a");
    }
}
