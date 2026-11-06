//! Who the directory holds. Part of the published store format (kit-store's FORMAT.md). A person's
//! row holds no email address: only its hash is stored, as the key sign-in looks up, so a log kept
//! forever keeps no address. Where someone belongs is the program's business, not this row's.

use kit_core::Id;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    #[serde(with = "kit_ops::id")]
    pub id: Id,
    pub name: String,
}
