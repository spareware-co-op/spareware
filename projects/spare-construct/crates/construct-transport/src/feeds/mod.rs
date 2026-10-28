//! Data feeds: traffic counts, GTFS public transport and other downloaded sources, turned into project data (port of packs/nz/feeds.js and the parsing side of data.js loaders).
//! There is no network here: each source has a function that builds the request (address, query, body) for the host to send, and a function that takes the answer as input.

mod ags;
mod arcgis;
mod control;
mod council;
mod fleet;
mod flow;
mod gtfs;
mod geo;
mod hazards;
pub mod jsregex;
mod live;
mod load;
mod nslr;
pub mod nztime;
mod sources;
mod tms;
mod vintage;
mod zip;

pub use ags::*;
pub use arcgis::*;
pub use control::*;
pub use council::*;
pub use fleet::*;
pub use flow::*;
pub use gtfs::*;
pub use geo::*;
pub use hazards::*;
pub use live::*;
pub use load::*;
pub use nslr::*;
pub use sources::*;
pub use tms::*;
pub use vintage::*;
pub use zip::*;
