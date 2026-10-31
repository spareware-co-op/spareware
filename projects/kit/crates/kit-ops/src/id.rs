//! Reads and writes [`Id`] in rows: `#[serde(with = "kit_ops::id")]`. These live here because
//! `kit-core` must stay free of dependencies, `serde` included. A row can't smuggle in a bad ID:
//! reading one runs the same checks as any other.

use kit_core::Id;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer>(id: &Id, serializer: S) -> Result<S::Ok, S::Error> {
    id.as_str().serialize(serializer)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Id, D::Error> {
    let text = String::deserialize(deserializer)?;
    Id::parse(&text).map_err(serde::de::Error::custom)
}

/// The same for an ID that may be missing.
pub mod option {
    use super::{Deserialize, Deserializer, Id, Serialize, Serializer};

    pub fn serialize<S: Serializer>(id: &Option<Id>, serializer: S) -> Result<S::Ok, S::Error> {
        id.as_ref().map(Id::as_str).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Id>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        text.map(|text| Id::parse(&text).map_err(serde::de::Error::custom)).transpose()
    }
}
