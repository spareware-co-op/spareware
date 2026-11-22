//! Spare Construct: shapes in the browser, built by the geometry kernel's engine. The builder the
//! server runs when its geometry setting is `kernel` (construct-regen's `kernel::Kernel` over
//! geom-solidcore's `SolidcoreEngine` on the building grid, as the server's `src/geometry.rs`
//! makes it), compiled to WebAssembly behind the same interface as the stand-in's package
//! (`construct-shapes`): [`edition`] and [`build`], JSON in and JSON out, so the page loads one
//! package or the other by a document's edition and reads what comes back the same way. And
//! [`build_hosting`]: a wall with the doors hosted in it cut, for the page once the hosting job
//! hands a wall its doors.
//!
//! `app/pkg-kernel` holds the built output; `server/scripts/build-shapes-kernel.sh` makes it.
//!
//! Why it lives here: it links solidcore, which never enters the root workspace (decision 32.70),
//! so it is a member of the server's workspace, whose Cargo.lock already pins solidcore and every
//! crate it adds for the server's `kernel` feature: the browser builds with the very engine, at
//! the very versions, the server builds with. Note CON-A3-F1-P1-kernel-in-the-browser.
//!
//! Guarantees: the answers are the stand-in package's for the same builder (`tests/package.rs`),
//! and the same bytes natively and in the browser: `tests/browser/same_bytes.mjs` builds
//! `tests/cases.json` with the browser's build and compares every answer with
//! `examples/answers.rs`'s, byte for byte.
//!
//! Depends on: `construct-model` (the shapes' types), `construct-regen` (the kernel builder),
//! `geom-solidcore` (the engine), `serde`, `serde_json` and `wasm-bindgen`.
//!
//! Interim: solidcore answers behind the engine seam where the kernel's own exact operations do
//! not yet (the interim register, `geom-solidcore/tests/interim.json`, says which part answers
//! each operation and what replaces it); moving to an exact engine of the kernel's own changes
//! [`builder`]'s one line. The page builds each element from its own recipe, so a wall it builds
//! has no openings cut until the hosting job (CON-A4-F3-P1) hands it its doors (the register's
//! row for this package).

use construct_model::element::Placement;
use construct_model::geometry::{
    Bounds, Built, Display, FaceName, Geometry, GeometryError, Recipe,
};
use construct_regen::kernel::{Hosted, Kernel, STEP};
use geom_solidcore::engine::SolidcoreEngine;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};
use std::sync::OnceLock;
use wasm_bindgen::prelude::wasm_bindgen;

/// The builder, made once: the kernel builder over solidcore's engine on the building grid, as the
/// server's `kernel` setting makes it.
fn builder() -> Result<&'static Kernel<SolidcoreEngine>, GeometryError> {
    static BUILDER: OnceLock<Result<Kernel<SolidcoreEngine>, GeometryError>> = OnceLock::new();
    BUILDER
        .get_or_init(|| Kernel::new(SolidcoreEngine::new(STEP)))
        .as_ref()
        .map_err(Clone::clone)
}

/// What builds shapes here: the edition, as JSON. Empty when the builder can't be made, which the
/// page reads as a package that can't build.
#[wasm_bindgen]
#[must_use]
pub fn edition() -> String {
    builder()
        .ok()
        .and_then(|k| to_string(&k.edition()).ok())
        .unwrap_or_default()
}

/// A built shape, as the page reads it.
#[derive(Serialize)]
struct Shape<'a> {
    body: String,
    bounds: Bounds,
    names: &'a [FaceName],
    displays: &'a [Display],
}

/// The answer the page reads: `{"shape": ...}`, or `{"failed": {code, message}}` as the server
/// stores the same failure.
fn answer(built: Result<Built, GeometryError>) -> String {
    match built {
        Ok(built) => {
            let shape = Shape {
                body: built.body_id(),
                bounds: built.bounds,
                names: &built.names,
                displays: &built.displays,
            };
            json!({ "shape": shape }).to_string()
        }
        Err(e) => json!({ "failed": { "code": e.code, "message": e.message } }).to_string(),
    }
}

/// Text that is not what was asked for, under the code the stand-in's package gives it.
fn bad(e: &serde_json::Error) -> GeometryError {
    GeometryError {
        code: "BadRecipe".to_owned(),
        message: e.to_string(),
    }
}

/// Builds a recipe, given as JSON, with `geometry`: the answer [`build`] gives, for any builder.
#[must_use]
pub fn build_with(geometry: &dyn Geometry, recipe: &str) -> String {
    answer(
        serde_json::from_str::<Recipe>(recipe)
            .map_err(|e| bad(&e))
            .and_then(|recipe| geometry.build(&recipe)),
    )
}

/// Builds a recipe, given as JSON. The answer is `{"shape": ...}`, or `{"failed": {code, message}}`
/// as the server stores the same failure.
#[wasm_bindgen]
#[must_use]
pub fn build(recipe: &str) -> String {
    match builder() {
        Ok(k) => build_with(k, recipe),
        Err(e) => answer(Err(e)),
    }
}

/// A host as [`build_hosting`] reads it: its recipe and its placement.
#[derive(Deserialize)]
struct Host {
    recipe: Recipe,
    placement: Placement,
}

/// An element hosted in it, by its permanent ID.
#[derive(Deserialize)]
struct Guest {
    key: String,
    recipe: Recipe,
    placement: Placement,
}

fn hosting(host: &str, hosted: &str) -> Result<Built, GeometryError> {
    let host: Host = serde_json::from_str(host).map_err(|e| bad(&e))?;
    let guests: Vec<Guest> = serde_json::from_str(hosted).map_err(|e| bad(&e))?;
    let k = builder()?;
    let units = k.units();
    let hosted = guests
        .iter()
        .map(|g| {
            Ok(Hosted {
                key: &g.key,
                recipe: &g.recipe,
                at: units.offset(&host.placement, &g.placement)?,
            })
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    k.build_hosting(&host.recipe, &hosted)
}

/// Builds a host with an opening cut for each element hosted in it: `host` is
/// `{recipe, placement}`, `hosted` a list of `{key, recipe, placement}` by each element's permanent
/// ID, placements in the model's whole millimetres. Each is placed in the host by the difference
/// of their placements and cut in the order of their keys, as the kernel builder's
/// `build_hosting` does. The answer is [`build`]'s.
#[wasm_bindgen]
#[must_use]
pub fn build_hosting(host: &str, hosted: &str) -> String {
    answer(hosting(host, hosted))
}
