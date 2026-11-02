//! Spare Construct: shapes in the browser. The same `construct_regen::geometry()` the server builds
//! with, compiled to WebAssembly, so the two can't differ by construction, and the server's
//! checkpoints catch it if they ever do. JSON in, JSON out. `app/pkg` holds the built output;
//! `server/scripts/build-shapes.sh` makes it.

use construct_model::geometry::{Bounds, Built, Display, FaceName, Recipe};
use serde::Serialize;
use serde_json::{json, to_string};
use wasm_bindgen::prelude::wasm_bindgen;

/// What builds shapes here: the edition, as JSON.
#[wasm_bindgen]
#[must_use]
pub fn edition() -> String {
    to_string(&construct_regen::geometry().edition()).unwrap_or_default()
}

/// A built shape, as the page reads it.
#[derive(Serialize)]
struct Shape<'a> {
    body: String,
    bounds: Bounds,
    names: &'a [FaceName],
    displays: &'a [Display],
}

fn shape_json(built: &Built) -> String {
    let shape = Shape {
        body: built.body_id(),
        bounds: built.bounds,
        names: &built.names,
        displays: &built.displays,
    };
    json!({ "shape": shape }).to_string()
}

/// Builds a recipe, given as JSON. The answer is `{"shape": ...}`, or `{"failed": {code, message}}`
/// as the server stores the same failure.
#[wasm_bindgen]
#[must_use]
pub fn build(recipe: &str) -> String {
    let recipe: Recipe = match serde_json::from_str(recipe) {
        Ok(recipe) => recipe,
        Err(e) => {
            return json!({ "failed": { "code": "BadRecipe", "message": e.to_string() } })
                .to_string();
        }
    };
    match construct_regen::geometry().build(&recipe) {
        Ok(built) => shape_json(&built),
        Err(e) => json!({ "failed": { "code": e.code, "message": e.message } }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn a_recipe_builds_to_the_body_the_server_would_store() {
        let recipe = r#"{"owner":"construct","schema":1,"op":"block","op_version":1,
            "args":{"width":100,"depth":200,"height":300}}"#;
        let parsed: Recipe = serde_json::from_str(recipe).unwrap();
        let server = construct_regen::geometry().build(&parsed).unwrap();
        let answer: Value = serde_json::from_str(&build(recipe)).unwrap();
        assert_eq!(answer["shape"]["body"], server.body_id());
    }

    #[test]
    fn a_failure_comes_back_as_an_answer() {
        let recipe = r#"{"owner":"construct","schema":1,"op":"nothing","op_version":1,"args":{}}"#;
        let answer: Value = serde_json::from_str(&build(recipe)).unwrap();
        assert_eq!(answer["failed"]["code"], "UnknownOperation");
        let answer: Value = serde_json::from_str(&build("not json")).unwrap();
        assert_eq!(answer["failed"]["code"], "BadRecipe");
    }

    #[test]
    fn the_edition_reads_as_the_server_writes_it() {
        let written = construct_regen::geometry().edition();
        let read = serde_json::from_str(&edition()).unwrap();
        assert_eq!(written, read);
    }
}
