//! The schema types of chapter 22, and the entry that places one operation's schema in a stability
//! tier of chapter 25.
//!
//! What it does: re-exports `geom-op`'s schema vocabulary ([`OpSchema`] and its parts), and defines
//! [`Operation`], one entry of [`crate::operations::OPERATIONS`]: an operation's schema and its
//! [`Tier`]. A Stable operation keeps its schema under semantic versioning; a Preview one is being
//! tried, may change, is not usable by published plugins, and graduates or is dropped within 12
//! months of the day it was added (25), which its tier records.
//!
//! Guarantees:
//!
//! - **Data only.** [`Operation`] and [`Tier`] are constants made of `&'static` data, with no
//!   method and no trait implementation beyond derived ones.
//! - **A preview's age is recorded.** [`Tier::Preview`] holds the day the operation entered the
//!   facade, `YYYY-MM-DD`, so a release check can find any preview older than 12 months.
//!
//! Method: none; types for chapter 22's operation schemas and chapter 25's tiers
//! (`projects/engines/geometry/architecture/22-errors-queries-ai.md`,
//! `projects/engines/geometry/architecture/25-public-interface.md`).
//!
//! Cost: none at run time: entries are constants.

pub use geom_run::Phase;
pub use geom_op::{
    Charge, Class, DiagnosticSpec, ErrorSpec, Family, Field, OpSchema, RoleSpec, Unit,
};

/// A stability tier of chapter 25, for one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Semantic versioning: a breaking change is a new major version that runs beside the old one.
    Stable,
    /// Being tried: may change, not usable by published plugins, and graduates or is dropped within
    /// 12 months.
    Preview {
        /// The day the operation entered the facade, `YYYY-MM-DD`.
        since: &'static str,
    },
}

/// One operation the facade offers: its schema and its tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Operation {
    /// The operation's schema (chapter 22).
    pub schema: &'static OpSchema,
    /// Its stability tier (chapter 25).
    pub tier: Tier,
}
