//! Bringing roads in and taking the network out: model.js `Importers`.
//!
//! [`osm`] reads an Overpass answer (the roads of OpenStreetMap) into a scenario and adds gateways where roads leave the area;
//! [`geojson`] reads GeoJSON lines as roads and writes the network as GeoJSON or a CSV of links. The downloading itself (an HTTP
//! request to an Overpass server) is not here: [`osm::overpass_query`] writes the query and the caller passes the answer in.

pub mod geojson;
pub mod jsparse;
pub mod osm;

pub use geojson::{geojson as export_geojson, import_geojson, links_csv};
pub use osm::{
    Bounds, OSM_CLASS, OVERPASS, OsmOpts, OverpassOpts, auto_gateways, osm_class, osm_to_scenario,
    overpass_query,
};
