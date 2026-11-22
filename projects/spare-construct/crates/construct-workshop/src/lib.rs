//! Spare Construct: Workshop, parts built by the geometry kernel.
//!
//! Workshop is Spare Construct's discipline for things that are made rather than built: parts drawn
//! as sketches and built from features, with a history to roll back and edit and parameters to drive
//! them (feature map area CON-A27). This crate is its first slice on the kernel: it takes a part's
//! recipe, as JSON, and builds it through `engine-geometry`, the kernel's one stable facade, the way
//! every Spare Construct discipline reaches the kernel.
//!
//! What it builds today is what the facade has: a convex profile extruded as a prism (`make.prism`),
//! a circle as a cylinder (`make.cylinder`), and the transforms mirror, pattern and move
//! (`transform.mirror`, `transform.pattern`, `transform.move`), each on the kernel's grid for parts
//! (6.25 nm). Volumes of flat-faced bodies come from the kernel's exact `query.volume`, sizes from
//! `query.bounding_box` and face counts from `query.faces`. Everything else a part can hold (joins and
//! cuts, revolve, sweep, fillet, chamfer, shell, holes) is kept in the history and answered as waiting,
//! with the kernel pieces it waits for, so it lights up as they land.
//!
//! The recipe and the answer have the same form as the browser prototype's builder
//! (`demo/workshop/js/build.js`), so the Workshop page can use either; the page uses this one, compiled
//! to WebAssembly into `demo/workshop/pkg` by `server/scripts/build-workshop.sh`.

pub mod build;

pub use build::{Answer, build_json, build_part};

use wasm_bindgen::prelude::wasm_bindgen;

/// Builds a part's recipe, given as JSON, and gives the answer as JSON (see [`build`]).
#[wasm_bindgen(js_name = build)]
#[must_use]
pub fn build_wasm(recipe: &str) -> String {
    build_json(recipe)
}

/// What builds parts here, as JSON: the kernel's facade and this crate.
#[wasm_bindgen]
#[must_use]
pub fn edition() -> String {
    serde_json::json!({
        "kernel": "engine-geometry",
        "builder": concat!("construct-workshop ", env!("CARGO_PKG_VERSION")),
        "grid": "Q1, 6.25 nm",
    })
    .to_string()
}
