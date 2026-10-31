//! `ssi-runner`: answers one surface/surface case, under the ssi-corpus's runner contract.
//!
//! ```text
//! SSI_STAND_IN=<name> ssi-runner <case.json> <mode> <tol> <out.json>
//! ```
//!
//! The corpus's scorer calls this once per case, in a process of its own. It places the case on the
//! grid and writes the answer of the stand-in `SSI_STAND_IN` names, `truth` if it is not set
//! ([`geom_test::ssi::standin`]). `<tol>` is a number, or `default`.
//!
//! Guarantees: it writes only `<out.json>`. A case it cannot place is a failure with a message and
//! no answer, which the scorer records as a crash: it is never answered as if it had been placed.
//! Exit codes: 0 when it wrote an answer; 1 when the case could not be placed; 2 for a usage error
//! or a file that cannot be read or written.
//!
//! Method: the runner contract of the corpus's README,
//! <https://github.com/jyviko/ssi-corpus/blob/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9/README.md>.
//!
//! Cost: one case's placement, and its answer.
//!
//! Consulted: ssi-corpus, MIT OR Apache-2.0,
//!   <https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9>, for its
//!   case schema, its runner contract and its scoring rules.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use geom_test::ssi::SsiError;
use geom_test::ssi::case::Case;
use geom_test::ssi::gate::accuracy_of;
use geom_test::ssi::place::place;
use geom_test::ssi::standin::StandIn;

/// Prints `message`, and gives the exit code of a usage error.
fn usage(message: &str) -> ExitCode {
    eprintln!("ssi-runner: {message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [case, _mode, tol, out] = args.as_slice() else {
        return usage("usage: ssi-runner <case.json> <mode> <tol> <out.json>");
    };
    let accuracy = if tol == "default" {
        None
    } else {
        match tol.parse::<f64>() {
            Ok(tol) => Some(accuracy_of(tol)),
            Err(e) => return usage(&format!("{tol}: {e}")),
        }
    };
    let name = env::var("SSI_STAND_IN").unwrap_or_else(|_| "truth".to_owned());
    let Some(stand_in) = StandIn::named(&name) else {
        return usage(&format!("no stand-in called {name}"));
    };
    let case = match Case::read(Path::new(case)) {
        Ok(case) => case,
        Err(e) => return usage(&e.to_string()),
    };
    let start = Instant::now();
    let placed = match place(&case, accuracy) {
        Ok(placed) => placed,
        Err(e @ SsiError::Unplaced { .. }) => {
            eprintln!("ssi-runner: {e}");
            return ExitCode::from(1);
        }
        Err(e) => return usage(&e.to_string()),
    };
    let mut answer = match stand_in.answer(&case, &placed, accuracy) {
        Ok(answer) => answer,
        Err(e) => return usage(&e.to_string()),
    };
    answer.time_ms = start.elapsed().as_secs_f64() * 1_000.0;
    let text = match serde_json::to_string(&answer) {
        Ok(text) => text,
        Err(e) => return usage(&e.to_string()),
    };
    match fs::write(out, text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => usage(&format!("{out}: {e}")),
    }
}
