//! `geom-bench-guest`: the program the WebAssembly fuel gate runs inside Wasmtime, built for
//! `wasm32-wasip1`. It also runs natively. Everything it does is in `geom_bench::guest`, whose
//! documentation gives its commands, its exit status and how the fuel gate uses them; this program
//! only passes on its arguments, output and status.
//!
//! Guarantees: those of `geom_bench::guest`, unchanged: the exit status and output are exactly the
//! ones it returns.
//!
//! Method: fuel metering (`Config::consume_fuel`,
//! <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>)
//! by differencing two runs (see `geom_bench::guest`).
//!
//! Cost: that of the command it runs (`geom_bench::guest`).

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = geom_bench::guest::main(&args);
    // Nothing useful can be done if standard output or error is closed, so failures are ignored.
    let _ = std::io::stdout().write_all(outcome.stdout.as_bytes());
    let _ = std::io::stderr().write_all(outcome.stderr.as_bytes());
    ExitCode::from(outcome.status)
}
