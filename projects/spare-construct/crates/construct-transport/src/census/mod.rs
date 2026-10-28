//! Census and existing land use turned into zones, trip generation and zoned yield: a port of the prototype's packs/nz/landuse-plus.js.
//!
//! Existing land use from official data (docs/spec/existing-landuse): Stats NZ Census 2023 by SA1 (dwellings, households, vehicles, travel to work and
//! education), Stats NZ employees by SA2, the Ministry of Education school and early learning directory and Auckland Unitary Plan base zones. Each is
//! requested for the study area ([`requests`]), read ([`parse`]) and placed in the scenario's zones with its source ([`fill`]). Trip generation
//! ([`tripgen`]) works out person and vehicle trips per zone and activity in the AT ITA Guidelines order and loads the result into the model; zoned yield
//! ([`yields`]) gives the land use a zoned area can hold; [`traffic`] says where development traffic goes and compares junctions without and with it.
//!
//! This crate does no HTTP: the host provides [`requests::Net`], and the functions that download build the same requests and give the same messages as the
//! prototype. Everything that only draws (the tab, the inspector sections, the report tables and the figure) is not ported; the numbers and text they are
//! made from are.

pub mod alloc;
pub mod consts;
pub mod fill;
pub mod geo;
pub mod parse;
pub mod requests;
pub mod traffic;
pub mod tripgen;
pub mod yields;

pub use alloc::*;
pub use consts::*;
pub use fill::*;
pub use geo::*;
pub use parse::*;
pub use requests::*;
pub use traffic::*;
pub use tripgen::*;
pub use yields::*;
