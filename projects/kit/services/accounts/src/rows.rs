//! Who and what the directory holds. Part of the published store format (kit-store's FORMAT.md).
//! A person's row holds no email address: only its hash is stored, as the key sign-in looks up,
//! so a log kept forever keeps no address.

use kit_core::Id;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    #[serde(with = "kit_ops::id")]
    pub home_hub: Id,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hub {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    #[serde(with = "kit_ops::id")]
    pub hub: Id,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Team {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
}

/// A model in a team's WIP folder. `package` names what publishing it makes, as in "Arch package".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Model {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
    #[serde(with = "kit_ops::id")]
    pub team: Id,
    pub package: String,
}

/// How someone belongs to a hub.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HubRole {
    User,
    External,
}
