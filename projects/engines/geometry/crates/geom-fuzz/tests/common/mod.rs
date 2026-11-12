//! What geom-fuzz's test files share: [`holds`], which turns a harness's verdict into a panic, and
//! under libFuzzer on Linux the alarm that makes libFuzzer's per-input time limit work (issue
//! #764).

use geom_fuzz::Finding;

/// Panics with `verdict`'s finding, if it has one.
///
/// A bolero test's body hands its verdict here rather than returning the [`Finding`], so that the
/// same test can be a Kani proof (the crate README, the rules of the layout). Under Kani, bolero's
/// engine turns a returned error into its `PanicError`, whose constructor may capture a backtrace
/// (`bolero-engine`'s `panic.rs`), and Kani would have to explore that capture, which kept the
/// first proofs in symbolic execution for an hour. A panic under Kani is a failed property and
/// nothing more: Kani's `panic!` formats nothing, and its report traces every value. Under
/// `cargo test`, libFuzzer and `ClusterFuzzLite` a panic fails the input as a returned finding
/// would, and its message is the finding.
///
/// Under libFuzzer on Linux, the first call also starts [`alarm`]'s thread, so that an input
/// running past libFuzzer's `-timeout` stops the run. Every target's `_fuzz` test ends each input
/// here (`tests/attack_g14.rs` holds them to it), and libFuzzer always runs the empty input first,
/// so the alarm is on from the second input of every run.
pub fn holds(verdict: Result<(), Finding>) {
    #[cfg(all(fuzzing_libfuzzer, target_os = "linux"))]
    alarm::start();
    if let Err(finding) = verdict {
        panic!("{finding}");
    }
}

/// libFuzzer's per-input time limit, made to work under the test harness (issue #764).
///
/// libFuzzer enforces `-timeout` from its `SIGALRM` handler: a timer sends the signal every
/// `timeout / 2 + 1` seconds, and the handler, if the input running has run for `timeout` seconds
/// or more, prints `ALARM`, writes the input as `timeout-<sha1>` beside the crash files and exits
/// with code 70. But the handler acts only on the thread that started the fuzzer
/// (`Fuzzer::AlarmCallback` in libFuzzer's `FuzzerLoop.cpp` begins
/// `if (!InFuzzingThread()) return;`), and `cargo bolero test` starts it inside the test harness,
/// which runs the test on a thread of its own. The timer's signal goes to the process, the main
/// thread takes it, and the handler returns: a 37-second input ran on under `-timeout=10`.
///
/// So this thread sends `SIGALRM` to the fuzzing thread itself, once a second. libFuzzer's own
/// handler then runs there and decides, by its own clock, its own `-timeout` and its own rule, as
/// it does when it runs on the main thread; nothing here measures an input. Between inputs, and
/// while an input is younger than a second, the handler returns at once. A second's tick stops an
/// input between `timeout` and `timeout + 1` seconds, where libFuzzer's own timer would stop it
/// between `timeout` and about `1.5 · timeout`.
///
/// Started only if the process catches `SIGALRM` (libFuzzer installs its handler when `-timeout`
/// is above zero), since the signal's default action ends the process.
///
/// Consulted: libFuzzer as bolero-libfuzzer 0.13.2 vendors it (`libfuzzer/FuzzerLoop.cpp`,
/// `Fuzzer::AlarmCallback` and `Fuzzer::ReadAndExecuteSeedCorpora`; `libfuzzer/FuzzerUtilPosix.cpp`,
/// `SetTimer`), Apache-2.0 WITH LLVM-exception, and cargo-bolero 0.13.5's `src/test_target.rs`,
/// MIT, for how the test is started. No code copied. The signal is sent with `nix` 0.31.3 (MIT),
/// whose `pthread_kill` is safe Rust over libc's, as this crate forbids unsafe code.
#[cfg(all(fuzzing_libfuzzer, target_os = "linux"))]
mod alarm {
    use std::sync::Once;
    use std::time::Duration;

    use nix::sys::pthread::{pthread_kill, pthread_self};
    use nix::sys::signal::Signal;

    /// How often the fuzzing thread is sent the signal.
    const TICK: Duration = Duration::from_secs(1);

    /// Starts the alarm thread for the calling thread, the fuzzing thread, once per process.
    pub fn start() {
        static STARTED: Once = Once::new();
        STARTED.call_once(|| {
            if !caught(Signal::SIGALRM) {
                return;
            }
            let fuzzing = pthread_self();
            // The fuzzing thread lives until the process exits (bolero's libFuzzer engine exits
            // the process from it), so the thread named here is never one that has ended.
            let started = std::thread::Builder::new()
                .name("libFuzzer alarm".to_owned())
                .spawn(move || {
                    while pthread_kill(fuzzing, Signal::SIGALRM).is_ok() {
                        std::thread::sleep(TICK);
                    }
                });
            if let Err(error) = started {
                eprintln!("geom-fuzz: no alarm thread, so -timeout is not enforced: {error}");
            }
        });
    }

    /// Whether this process catches `signal`: its bit in the `SigCgt` mask of `/proc/self/status`
    /// (proc(5)), bit `n - 1` for signal `n`.
    fn caught(signal: Signal) -> bool {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return false;
        };
        let Some(mask) = status
            .lines()
            .find_map(|line| line.strip_prefix("SigCgt:"))
            .and_then(|hex| u64::from_str_radix(hex.trim(), 16).ok())
        else {
            return false;
        };
        u32::try_from(signal as i32)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|bit| mask.checked_shr(bit))
            .is_some_and(|m| m & 1 == 1)
    }
}
