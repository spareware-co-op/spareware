//! Setting up a study area: the downloads it asks for and how their answers become the base network, gateways, zones and the first model run (a port of the
//! prototype's studyarea.js, with the count and crash downloads of data.js `DataHub.loadAll` that it uses).
//!
//! [`setup`] does it all for the current scenario: roads from OpenStreetMap, gateways where main roads leave the area, counts and crashes, existing land use
//! zones, gateway flows from the counts, the AM and PM runs and the optional fit to counts. [`estimate`] says how big an area is and whether it is too big.
//! The downloads go through the host's [`crate::census::Net`]; the modules here build the same requests and give the same messages as the prototype.

pub mod counts;
pub mod mapbox;
pub mod setup;

pub use counts::*;
pub use mapbox::*;
pub use setup::*;
