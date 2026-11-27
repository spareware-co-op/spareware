//! What a railing reads back, worked out each time from its layout and never stored, as read-only
//! parameters a page shows, a schedule lists and a tag reads.
//!
//! With fixed IDs ([`read_only`]): [`RAIL_LENGTH`] (the top rail's length, every piece along its
//! slope), [`HANDRAIL_LENGTH`] (the handrail's, none without one), [`BALUSTERS`] and [`POSTS`]
//! (how many), and [`HEIGHT`] (the top rail's height).

use crate::error::RailingError;
use crate::layout::Layout;
use crate::railing::Railing;
use construct_params::{Definition, Holder, Kind, Value};
use kit_core::Id;

/// The read-only parameter of a railing's top rail length.
pub const RAIL_LENGTH: &str = "railing.rail-length";
/// The read-only parameter of a railing's handrail length.
pub const HANDRAIL_LENGTH: &str = "railing.handrail-length";
/// The read-only parameter of how many balusters a railing has.
pub const BALUSTERS: &str = "railing.balusters";
/// The read-only parameter of how many posts a railing has.
pub const POSTS: &str = "railing.posts";
/// The read-only parameter of a railing's top rail height.
pub const HEIGHT: &str = "railing.height";

fn define(id: &str, name: &str, kind: Kind) -> Result<Definition, RailingError> {
    let id = Id::parse(id).map_err(|_| RailingError::Malformed(format!("{id:?} is not an ID")))?;
    Definition::new(id, name, kind, Holder::Instance)
        .map_err(|e| RailingError::Malformed(e.to_string()))
}

fn whole(n: usize) -> Result<Value, RailingError> {
    i64::try_from(n)
        .map(Value::Whole)
        .map_err(|_| RailingError::OutOfRange {
            what: "count".to_owned(),
        })
}

/// A railing's read-only parameters with their values, in the module header's order.
///
/// # Errors
/// [`RailingError::OutOfRange`] for a count past an `i64`, which the most balusters rules out;
/// [`RailingError::Malformed`] never, for these fixed definitions.
pub fn read_only(
    railing: &Railing,
    layout: &Layout,
) -> Result<Vec<(Definition, Option<Value>)>, RailingError> {
    Ok(vec![
        (
            define(RAIL_LENGTH, "Rail length", Kind::Length)?,
            Some(Value::Length(layout.rail_length)),
        ),
        (
            define(HANDRAIL_LENGTH, "Handrail length", Kind::Length)?,
            layout.handrail_length.map(Value::Length),
        ),
        (
            define(BALUSTERS, "Balusters", Kind::Whole)?,
            Some(whole(layout.balusters)?),
        ),
        (
            define(POSTS, "Posts", Kind::Whole)?,
            Some(whole(layout.posts)?),
        ),
        (
            define(HEIGHT, "Height", Kind::Length)?,
            Some(Value::Length(railing.kind().top_rail.height)),
        ),
    ])
}
