//! Prints this package's answers natively, for `tests/browser/same_bytes.mjs` to compare with the
//! browser's build byte for byte: the edition, then each case of a cases file (`tests/cases.json`)
//! built as the page builds it, one line each, `<name>`, a tab, then the answer.
//!
//! `cargo run --release -p construct-shapes-kernel --example answers -- tests/cases.json`

use construct_shapes_kernel::{build, build_hosting, edition};
use serde_json::Value;
use std::fmt::Write as _;
use std::io::Write as _;

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: answers <cases.json>")?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let cases: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let cases = cases
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{path}: no list of cases"))?;
    let mut out = format!("edition\t{}\n", edition());
    for case in cases {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{path}: a case without a name"))?;
        let answer = match (case.get("build"), case.get("host"), case.get("hosted")) {
            (Some(recipe), _, _) => build(&recipe.to_string()),
            (None, Some(host), Some(hosted)) => {
                build_hosting(&host.to_string(), &hosted.to_string())
            }
            _ => {
                return Err(format!(
                    "{path}: {name} has no build, nor a host and hosted"
                ));
            }
        };
        writeln!(out, "{name}\t{answer}").map_err(|e| e.to_string())?;
    }
    std::io::stdout()
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())
}
