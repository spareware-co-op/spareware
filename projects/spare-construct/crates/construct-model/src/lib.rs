//! Spare Construct: data model.
//!
//! Elements, types, instances, parameters, relationships, units and coordinates. Map:
//! projects/spare-construct/ARCHITECTURE.md, "Data model"; notes A02, A05 and A06.
//!
//! - [`geometry`]: the one join between Spare Construct and whatever builds its shapes.
//! - [`element`]: what an element is: inputs only, with its recipe. Shapes are derived from it.
//! - [`rows`]: hubs, projects, teams, folders, packages and links (CON-A25, CON-A26).
//! - [`keys`]: Spare Construct's tables in the store, a published format (FORMAT.md).

pub mod element;
pub mod geometry;
pub mod keys;
pub mod rows;
