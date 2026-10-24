//! `geom-bench-fuel`: the WebAssembly fuel gate's host (chapter 23.13, "WebAssembly fuel").
//!
//! It runs `geom-bench-guest.wasm` (built from `geom-bench` for `wasm32-wasip1`) in Wasmtime with
//! fuel metering on, which counts the WebAssembly operators the guest executes, deterministically:
//! the same module gives the same counts on every host. For every benchmark the guest lists, it
//! prints one line, the benchmark's name and the fuel of one pass over its batch, measured as the
//! difference between a run with two passes and a run with one (`geom_bench::guest` explains why
//! that difference is exactly one pass). It also has the guest check every benchmark's golden
//! digests on wasm32, so the results there must be the bits recorded natively (principle 5).
//!
//! ```text
//! geom-bench-fuel target/wasm32-wasip1/release/geom-bench-guest.wasm > fuel.txt
//! ```
//!
//! Exit status: 0 when every benchmark was measured and checked; 1 when any benchmark's golden
//! check failed, its run ended with a nonzero status or did not finish (a trap, or no fuel left),
//! each named on standard error while the others are still measured; 2 for a usage or setup error
//! (the module does not load or link, or the guest cannot list its benchmarks).
//!
//! Guarantees: every count printed is the fuel of exactly one pass, from two runs of one compiled
//! module in fresh stores, so it is the same on every host that runs this Wasmtime version; a
//! benchmark that cannot be measured or checked is never left out silently; and the host gives
//! the guest no files, clocks, randomness or environment, so nothing outside the module can
//! change a count.
//!
//! The guest needs six functions of WASI preview 1, which this host provides itself, in memory,
//! with no access to files, clocks or randomness: `args_sizes_get`, `args_get`,
//! `environ_sizes_get`, `environ_get` (an empty environment), `fd_write` (captured standard
//! output and error) and `proc_exit`. Any other import traps if called.
//!
//! Method: Wasmtime's fuel metering (`Config::consume_fuel`: "most WebAssembly instructions
//! consume 1 unit of fuel";
//! <https://github.com/bytecodealliance/wasmtime/blob/5afd63588b1f5520391c6a37fc38dfd3ccd607c2/crates/wasmtime/src/config.rs#L627-L646>),
//! and the six WASI preview 1 functions as the specification defines them
//! (<https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md>).
//!
//! Cost: the guest is compiled once; each benchmark then takes three runs of it (`run 1`, `run 2`
//! and `check`), each in a fresh store.

use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

use wasmtime::{Caller, Config, Engine, Error, Linker, Memory, Module, Store};

/// The Wasmtime version whose fuel model defines the counts (the one `Cargo.toml` pins).
const WASMTIME_VERSION: &str = "49.0.0";

/// The fuel each run starts with: far more than any benchmark needs, far below the limit.
const FUEL: u64 = 1 << 50;

/// The WASI module name.
const WASI: &str = "wasi_snapshot_preview1";

/// WASI errno: success.
const ERRNO_SUCCESS: i32 = 0;

/// WASI errno: bad file descriptor.
const ERRNO_BADF: i32 = 8;

/// WASI errno: an address outside the guest's memory.
const ERRNO_FAULT: i32 = 21;

/// What one run of the guest sees and leaves.
#[derive(Default)]
struct State {
    /// The guest's arguments, program name first.
    args: Vec<Vec<u8>>,
    /// What it wrote to standard output.
    stdout: Vec<u8>,
    /// What it wrote to standard error.
    stderr: Vec<u8>,
    /// The status it passed to `proc_exit`, if it called it.
    exit: Option<i32>,
}

/// The guest's exported memory.
fn memory(caller: &mut Caller<'_, State>) -> Option<Memory> {
    caller.get_export("memory")?.into_memory()
}

/// A guest address as an offset into its memory.
fn offset(address: i32) -> usize {
    // Guest addresses are unsigned 32-bit values, which fit usize on every host Wasmtime supports.
    usize::try_from(address.cast_unsigned()).unwrap_or(usize::MAX)
}

/// Writes `bytes` at `address` in the guest's memory; false if they do not fit.
fn put(caller: &mut Caller<'_, State>, address: usize, bytes: &[u8]) -> bool {
    match memory(caller) {
        Some(mem) => mem.write(caller, address, bytes).is_ok(),
        None => false,
    }
}

/// Reads `len` bytes at `address` in the guest's memory.
fn get(caller: &mut Caller<'_, State>, address: usize, len: usize) -> Option<Vec<u8>> {
    let mem = memory(caller)?;
    let mut buf = vec![0u8; len];
    mem.read(&*caller, address, &mut buf).ok()?;
    Some(buf)
}

/// A little-endian u32 at `address` in the guest's memory.
fn get_u32(caller: &mut Caller<'_, State>, address: usize) -> Option<u32> {
    let bytes = get(caller, address, 4)?;
    let array: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(array))
}

/// A count or size as the u32 WASI passes.
fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `args_sizes_get(argc, size)`: the number of arguments and the bytes they need.
fn args_sizes_get(mut c: Caller<'_, State>, argc: i32, size: i32) -> i32 {
    let count = u32_of(c.data().args.len());
    let bytes = u32_of(c.data().args.iter().map(|a| a.len() + 1).sum());
    let ok = put(&mut c, offset(argc), &count.to_le_bytes())
        && put(&mut c, offset(size), &bytes.to_le_bytes());
    if ok { ERRNO_SUCCESS } else { ERRNO_FAULT }
}

/// `args_get(argv, buf)`: a pointer to each argument at `argv`, the NUL-terminated arguments at
/// `buf`.
fn args_get(mut c: Caller<'_, State>, argv: i32, buf: i32) -> i32 {
    let args = c.data().args.clone();
    let (mut pointer, mut at) = (offset(argv), offset(buf));
    for mut text in args {
        text.push(0);
        let ok = put(&mut c, pointer, &u32_of(at).to_le_bytes()) && put(&mut c, at, &text);
        if !ok {
            return ERRNO_FAULT;
        }
        pointer = pointer.saturating_add(4);
        at = at.saturating_add(text.len());
    }
    ERRNO_SUCCESS
}

/// `environ_sizes_get(count, size)`: an empty environment.
fn environ_sizes_get(mut c: Caller<'_, State>, count: i32, size: i32) -> i32 {
    let ok = put(&mut c, offset(count), &0u32.to_le_bytes())
        && put(&mut c, offset(size), &0u32.to_le_bytes());
    if ok { ERRNO_SUCCESS } else { ERRNO_FAULT }
}

/// `environ_get(environ, buf)`: nothing to write for an empty environment.
fn environ_get(_: Caller<'_, State>, _: i32, _: i32) -> i32 {
    ERRNO_SUCCESS
}

/// `fd_write(fd, iovs, len, written)`: gathers the buffers for standard output or error.
fn fd_write(mut c: Caller<'_, State>, fd: i32, iovs: i32, len: i32, written: i32) -> i32 {
    if fd != 1 && fd != 2 {
        return ERRNO_BADF;
    }
    let mut bytes = Vec::new();
    for i in 0..usize::try_from(len).unwrap_or(0) {
        // Each iovec is a buffer address and a length, two little-endian u32s. An address past the
        // end of memory saturates, and reading there fails.
        let iov = offset(iovs).saturating_add(i.saturating_mul(8));
        let (Some(ptr), Some(n)) = (get_u32(&mut c, iov), get_u32(&mut c, iov.saturating_add(4)))
        else {
            return ERRNO_FAULT;
        };
        let address = usize::try_from(ptr).unwrap_or(usize::MAX);
        let Some(chunk) = get(&mut c, address, usize::try_from(n).unwrap_or(usize::MAX)) else {
            return ERRNO_FAULT;
        };
        bytes.extend_from_slice(&chunk);
    }
    let total = u32_of(bytes.len());
    if fd == 1 {
        c.data_mut().stdout.extend(bytes);
    } else {
        c.data_mut().stderr.extend(bytes);
    }
    if put(&mut c, offset(written), &total.to_le_bytes()) {
        ERRNO_SUCCESS
    } else {
        ERRNO_FAULT
    }
}

/// `proc_exit(status)`: records the status and stops the guest.
fn proc_exit(mut c: Caller<'_, State>, status: i32) -> Result<(), Error> {
    c.data_mut().exit = Some(status);
    Err(Error::msg("proc_exit"))
}

/// The six WASI preview 1 functions the guest imports; every other import traps if called.
fn linker(engine: &Engine, module: &Module) -> Result<Linker<State>, Error> {
    let mut linker = Linker::new(engine);
    linker.func_wrap(WASI, "args_sizes_get", args_sizes_get)?;
    linker.func_wrap(WASI, "args_get", args_get)?;
    linker.func_wrap(WASI, "environ_sizes_get", environ_sizes_get)?;
    linker.func_wrap(WASI, "environ_get", environ_get)?;
    linker.func_wrap(WASI, "fd_write", fd_write)?;
    linker.func_wrap(WASI, "proc_exit", proc_exit)?;
    linker.define_unknown_imports_as_traps(module)?;
    Ok(linker)
}

/// How one run of the guest ended.
struct Run {
    /// The exit status.
    status: i32,
    /// The fuel the whole run consumed, instantiation included.
    fuel: u64,
    /// Its standard output.
    stdout: Vec<u8>,
    /// Its standard error.
    stderr: Vec<u8>,
}

/// The engine, module and linker every run shares.
struct Guest {
    /// The engine, with fuel metering on.
    engine: Engine,
    /// The compiled guest.
    module: Module,
    /// Its imports.
    linker: Linker<State>,
}

impl Guest {
    /// Compiles the guest at `path`.
    fn load(path: &str) -> Result<Guest, Error> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        let module = Module::from_file(&engine, path)?;
        let linker = linker(&engine, &module)?;
        Ok(Guest {
            engine,
            module,
            linker,
        })
    }

    /// Runs the guest's `_start` with these arguments (after the program name), in a fresh store.
    fn run(&self, args: &[&str]) -> Result<Run, Error> {
        let mut all = vec![b"geom-bench-guest".to_vec()];
        all.extend(args.iter().map(|a| a.as_bytes().to_vec()));
        let mut store = Store::new(
            &self.engine,
            State {
                args: all,
                ..State::default()
            },
        );
        store.set_fuel(FUEL)?;
        let instance = self.linker.instantiate(&mut store, &self.module)?;
        let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
        let result = start.call(&mut store, ());
        let fuel = FUEL.saturating_sub(store.get_fuel()?);
        let status = match (result, store.data().exit) {
            (_, Some(status)) => status,
            (Ok(()), None) => 0,
            (Err(error), None) => return Err(error),
        };
        let state = store.into_data();
        Ok(Run {
            status,
            fuel,
            stdout: state.stdout,
            stderr: state.stderr,
        })
    }
}

/// Measures and checks every benchmark; the count file goes to `out`, problems to `problems`.
fn measure(path: &str, out: &mut String, problems: &mut Vec<String>) -> Result<(), Error> {
    let guest = Guest::load(path)?;
    let list = guest.run(&["list"])?;
    if list.status != 0 {
        let message = format!(
            "the guest could not list its benchmarks (status {})",
            list.status
        );
        return Err(Error::msg(message));
    }
    let names = String::from_utf8_lossy(&list.stdout).into_owned();
    let _ = writeln!(
        out,
        "# WebAssembly fuel of one pass over each benchmark's batch: Wasmtime {WASMTIME_VERSION}, \
         wasm32-wasip1."
    );
    for name in names.lines().filter(|n| !n.is_empty()) {
        // A trap (the guest panicked, or ran out of fuel) is this benchmark's problem: it is named,
        // and the others are still measured.
        let runs = guest
            .run(&["run", name, "1"])
            .and_then(|one| Ok((one, guest.run(&["run", name, "2"])?)))
            .and_then(|(one, two)| Ok((one, two, guest.run(&["check", name])?)));
        let (one, two, check) = match runs {
            Ok(runs) => runs,
            Err(error) => {
                problems.push(format!("{name}: the guest did not finish: {error}"));
                continue;
            }
        };
        if one.status != 0 || two.status != 0 {
            problems.push(format!(
                "{name}: the guest failed to run it (status {} and {})",
                one.status, two.status
            ));
            continue;
        }
        if check.status != 0 {
            let message = String::from_utf8_lossy(&check.stderr).trim().to_owned();
            problems.push(format!(
                "{name}: golden digests differ on wasm32: {message}"
            ));
        }
        match two.fuel.checked_sub(one.fuel) {
            Some(pass) => {
                let _ = writeln!(out, "{name} {pass}");
            }
            None => problems.push(format!("{name}: two passes used less fuel than one")),
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        let _ = writeln!(
            std::io::stderr(),
            "usage: geom-bench-fuel <geom-bench-guest.wasm>"
        );
        return ExitCode::from(2);
    };
    let mut out = String::new();
    let mut problems = Vec::new();
    if let Err(error) = measure(path, &mut out, &mut problems) {
        let _ = writeln!(std::io::stderr(), "geom-bench-fuel: {error}");
        return ExitCode::from(2);
    }
    let _ = std::io::stdout().write_all(out.as_bytes());
    for problem in &problems {
        let _ = writeln!(std::io::stderr(), "geom-bench-fuel: {problem}");
    }
    if problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
