//! The reference runner's nightly wall-clock timings (piece G107): the record of one night, its
//! assembly from the runner's job lines, chapter 23.1's shares, and the report that keeps the
//! history against them.
//!
//! What it does: the runner (`reference/nightly.sh`, beside this crate) times every benchmark of
//! the catalogue each night on the reference machine, at 1 and 8 threads ([`THREADS`]), natively
//! and in three browsers ([`ENGINES`]), each job with `geom-bench-guest time <name> <ms>
//! <threads>`, and writes one job line per benchmark ([`assemble`] says the syntax). [`assemble`]
//! turns those lines, the night's header and the machine's busy samples into a [`Record`], one
//! text file per night in the history; [`report`] reads the history and writes the night's report
//! in Markdown: each of 23.1's shares ([`SHARES`]) against the benchmark that measures it, what
//! moved since the night before, how each group scales from 1 to 8 threads and from native to
//! each browser, and the timings taken while something else was using the machine.
//!
//! The record, line by line (every line is a key, a space and its value; nothing else is read):
//!
//! ```text
//! spare-kit reference timings 1
//! commit <the 40-digit commit timed>
//! night <YYYY-MM-DD, the New Zealand date the night began>
//! started <milliseconds since 1970, UTC>
//! finished <milliseconds since 1970, UTC>
//! status complete | deadline
//! machine <the processor's name>
//! processors <logical processors>
//! performance <the performance cores' logical processor numbers, space-separated>
//! os <the operating system's name and build>
//! toolchain <rustc --version>
//! round <milliseconds per round>
//! engine <engine> <version, or "unavailable" and why>      (one per engine, in ENGINES order)
//! time <engine> <threads> <benchmark> <ns per input> <passes per round> <slowest thread's ns> <busy %|->
//! fail <engine> <threads> <benchmark> <why>
//! ```
//!
//! Guarantees:
//!
//! - **A record reads back as written:** [`Record::parse`] of a record's text gives the record
//!   again, and its text again, byte for byte ([`fmt::Display`]).
//! - **Strict reading:** a record with a line out of order, an unknown key, a malformed field, an
//!   engine or thread count it does not know, a busy share over 100% or the same benchmark twice
//!   for one engine and thread count is refused, with the line number.
//! - **Nothing missing in silence:** [`assemble`] gives every benchmark it is told of, on every
//!   engine of the header and at every thread count of [`THREADS`], exactly one `time` or `fail`
//!   line. A job the runner did not reach becomes a `fail` line only when the night stopped at its
//!   deadline (`status deadline`) or its engine was unavailable; otherwise [`assemble`] refuses.
//! - **It reports and never blocks:** [`report`] only writes text. Turning a miss into a block is
//!   piece G452's, under the owner's rule of 32.42 (a nightly miss confirmed by a same-night rerun
//!   blocks merges to `main` except the fix or a revert).
//! - **Deterministic:** no hash map (every map is a `BTreeMap`), no clock, no randomness and no
//!   platform maths; the report's statistics are medians and ratios of the recorded figures.
//!
//! Method: wall-clock timing as chapter 23.13 sets it for the dedicated runner ("Wall clock and
//! scaling"), on the machine and cores of decision 32.16, against the shares of 23.1; the timings
//! are the guest's rounds (divan's sample-and-median design, <https://docs.rs/divan/0.1.21/divan/>),
//! and the history is kept as text in a data branch of the repository, as
//! github-action-benchmark keeps its benchmark history in a branch of its own.
//!
//! - `projects/engines/geometry/architecture/23-performance.md`, sections 23.1 and 23.13.
//! - `projects/engines/geometry/architecture/32-decisions.md`, decisions 32.16 and 32.42.
//! - H. Hinnant, "chrono-Compatible Low-Level Date Algorithms", `civil_from_days`,
//!   <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>, for the report's dates.
//!
//! Consulted: github-action-benchmark, MIT,
//!   <https://github.com/benchmark-action/github-action-benchmark/blob/84ec6ea748925b6580796c05ce7a1c8fd0d15f3b/README.md>,
//!   for keeping benchmark history in a branch of the repository it measures. No code copied.
//!
//! Cost: reading and writing are linear in the lines; [`assemble`] and [`report`] sort each
//! night's timings, O(n log n) in the timings of the nights they read, and allocate their text.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

/// The record's first line: its format and version.
pub const FORMAT: &str = "spare-kit reference timings 1";

/// The engines timed, in the order a record lists them: the native Windows build on the
/// performance cores (32.16), then Chrome, Firefox and WebKit, Safari's engine (Playwright's
/// build; Safari itself runs only on macOS, and the co-op has no Mac to lend).
pub const ENGINES: [&str; 4] = ["native", "chrome", "firefox", "webkit"];

/// The browsers among [`ENGINES`].
pub const BROWSERS: [&str; 3] = ["chrome", "firefox", "webkit"];

/// The thread counts every benchmark is timed at (chapter 23.13: "at 1 and 8 threads").
pub const THREADS: [u32; 2] = [1, 8];

/// Most threads a record may name, as `geom-bench-guest time` accepts.
pub const MAX_THREADS: u32 = 64;

/// A timing is disturbed when the machine was busier, in percentage points, than the job itself
/// explains (its threads' share of the logical processors) by more than this.
pub const DISTURBED_POINTS: u32 = 10;

/// A change since the night before is listed when the time moved by more than this, in percent,
/// either way: principle 12's limit, which G452's statistical gate will enforce.
pub const MOVE_PERCENT: f64 = 5.0;

/// Nights of history the report shows for each share.
pub const SHARE_NIGHTS: usize = 7;

/// How a night ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every job ran.
    Complete,
    /// The night reached its deadline (08:00 in New Zealand, 28.17) before every job ran.
    Deadline,
}

impl Status {
    /// The word a record writes.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Status::Complete => "complete",
            Status::Deadline => "deadline",
        }
    }

    /// The status a record's word names.
    #[must_use]
    pub fn named(word: &str) -> Option<Status> {
        match word {
            "complete" => Some(Status::Complete),
            "deadline" => Some(Status::Deadline),
            _ => None,
        }
    }
}

/// One benchmark timed on one engine at one thread count.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// One of [`ENGINES`].
    pub engine: String,
    /// The threads that ran the benchmark at once.
    pub threads: u32,
    /// The benchmark's catalogue name.
    pub benchmark: String,
    /// Nanoseconds per input: the upper median of the threads' medians.
    pub ns: f64,
    /// The fewest passes any thread's last round held.
    pub passes: u64,
    /// The slowest thread's median, in nanoseconds per input.
    pub slowest: f64,
    /// How busy the whole machine was during the job, in percent, if it was sampled.
    pub busy: Option<u32>,
}

/// A benchmark that was not timed on one engine at one thread count, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// One of [`ENGINES`].
    pub engine: String,
    /// The thread count.
    pub threads: u32,
    /// The benchmark's catalogue name.
    pub benchmark: String,
    /// Why: what the guest printed, or that the night or the engine did not reach it.
    pub reason: String,
}

/// One night on the reference machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The commit timed, 40 lower-case hexadecimal digits.
    pub commit: String,
    /// The New Zealand date the night began, `YYYY-MM-DD`.
    pub night: String,
    /// When the night's first job started, in milliseconds since 1970 (UTC).
    pub started: u64,
    /// When its last job ended, in milliseconds since 1970 (UTC).
    pub finished: u64,
    /// How the night ended.
    pub status: Status,
    /// The processor's name.
    pub machine: String,
    /// How many logical processors the machine has.
    pub processors: u32,
    /// The logical processor numbers of the performance cores the timings ran on.
    pub performance: Vec<u32>,
    /// The operating system's name and build.
    pub os: String,
    /// The compiler, as `rustc --version` prints it.
    pub toolchain: String,
    /// Milliseconds per round of the guest's `time`.
    pub round_ms: u32,
    /// Each engine of the night and its version (or `unavailable` and why), in [`ENGINES`] order.
    pub engines: Vec<(String, String)>,
    /// The timings, in the order the runner took them.
    pub timings: Vec<Timing>,
    /// The benchmarks not timed, in the order the runner met them.
    pub failures: Vec<Failure>,
}

/// Why a record, a header or a job line was refused: the line number (from 1) and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The line, counted from 1; 0 for the text as a whole.
    pub line: usize,
    /// What is wrong.
    pub what: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.what)
        } else {
            write!(f, "line {}: {}", self.line, self.what)
        }
    }
}

/// A refusal at `line`.
fn refuse(line: usize, what: impl Into<String>) -> Refused {
    Refused {
        line,
        what: what.into(),
    }
}

/// The header keys, in the order a record writes them, after the format line.
const HEADER: [&str; 11] = [
    "commit",
    "night",
    "started",
    "finished",
    "status",
    "machine",
    "processors",
    "performance",
    "os",
    "toolchain",
    "round",
];

/// Whether `text` is one field: not empty, and without white space.
fn is_field(text: &str) -> bool {
    !text.is_empty() && !text.contains(char::is_whitespace)
}

/// Whether `text` is free text a record can hold on one line: something, with no line break,
/// and without white space at either end.
fn is_text(text: &str) -> bool {
    !text.is_empty() && !text.contains(['\n', '\r']) && text.trim() == text
}

/// Whether `night` is a date written `YYYY-MM-DD` with a month from 01 to 12 and a day from 01 to
/// 31.
fn is_night(night: &str) -> bool {
    let bytes = night.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let two = |at: usize| -> Option<u32> { night.get(at..at + 2)?.parse().ok() };
    bytes.len() == 10
        && digits(0..4)
        && bytes.get(4) == Some(&b'-')
        && digits(5..7)
        && bytes.get(7) == Some(&b'-')
        && digits(8..10)
        && two(5).is_some_and(|m| (1..=12).contains(&m))
        && two(8).is_some_and(|d| (1..=31).contains(&d))
}

/// Whether `commit` is 40 lower-case hexadecimal digits.
fn is_commit(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A thread count from 1 to [`MAX_THREADS`].
fn threads_field(text: &str, line: usize) -> Result<u32, Refused> {
    text.parse::<u32>()
        .ok()
        .filter(|n| (1..=MAX_THREADS).contains(n))
        .ok_or_else(|| {
            refuse(
                line,
                format!("a thread count must be 1 to {MAX_THREADS}, not {text}"),
            )
        })
}

/// A time in nanoseconds: a finite number, zero or more.
fn ns_field(text: &str, line: usize) -> Result<f64, Refused> {
    text.parse::<f64>()
        .ok()
        .filter(|ns| ns.is_finite() && *ns >= 0.0)
        .ok_or_else(|| {
            refuse(
                line,
                format!("a time must be a number of nanoseconds, not {text}"),
            )
        })
}

/// One of [`ENGINES`].
fn engine_field(text: &str, line: usize) -> Result<String, Refused> {
    if ENGINES.contains(&text) {
        Ok(text.to_string())
    } else {
        Err(refuse(line, format!("unknown engine {text}")))
    }
}

/// A benchmark's name: one field.
fn benchmark_field(text: &str, line: usize) -> Result<String, Refused> {
    if is_field(text) {
        Ok(text.to_string())
    } else {
        Err(refuse(line, "a benchmark name is one field"))
    }
}

/// Splits `line` into its key and the rest, at the first space.
fn key_value(line: &str) -> (&str, &str) {
    line.split_once(' ').unwrap_or((line, ""))
}

impl Record {
    /// Reads a record, refusing anything it does not know (see the module documentation).
    ///
    /// # Errors
    ///
    /// The first line that is out of order, unknown or malformed, or a benchmark named twice for
    /// one engine and thread count.
    pub fn parse(text: &str) -> Result<Record, Refused> {
        let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l)).peekable();
        match lines.next() {
            Some((_, first)) if first == FORMAT => {}
            Some((n, _)) => return Err(refuse(n, format!("the first line must be `{FORMAT}`"))),
            None => return Err(refuse(0, "the record is empty")),
        }
        let mut values: Vec<&str> = Vec::with_capacity(HEADER.len());
        for key in HEADER {
            let Some((n, line)) = lines.next() else {
                return Err(refuse(0, format!("the header ends before `{key}`")));
            };
            let (found, value) = key_value(line);
            if found != key {
                return Err(refuse(n, format!("expected `{key}`, found `{found}`")));
            }
            values.push(value);
        }
        let field = |i: usize| values.get(i).copied().unwrap_or("");
        let header_line = |i: usize| i + 2;
        let commit = field(0);
        if !is_commit(commit) {
            return Err(refuse(
                header_line(0),
                "a commit is 40 lower-case hexadecimal digits",
            ));
        }
        let night = field(1);
        if !is_night(night) {
            return Err(refuse(header_line(1), "a night is a date, YYYY-MM-DD"));
        }
        let millis = |i: usize| -> Result<u64, Refused> {
            field(i)
                .parse()
                .map_err(|_| refuse(header_line(i), "a time is whole milliseconds since 1970"))
        };
        let started = millis(2)?;
        let finished = millis(3)?;
        if finished < started {
            return Err(refuse(
                header_line(3),
                "the night finished before it started",
            ));
        }
        let status = Status::named(field(4))
            .ok_or_else(|| refuse(header_line(4), "the status is `complete` or `deadline`"))?;
        let text_field = |i: usize| -> Result<String, Refused> {
            if is_text(field(i)) {
                Ok(field(i).to_string())
            } else {
                Err(refuse(
                    header_line(i),
                    format!("`{}` needs a value", HEADER.get(i).copied().unwrap_or("")),
                ))
            }
        };
        let machine = text_field(5)?;
        let processors: u32 = field(6).parse().ok().filter(|&n| n > 0).ok_or_else(|| {
            refuse(
                header_line(6),
                "the processor count is a whole number above 0",
            )
        })?;
        let mut performance: Vec<u32> = Vec::new();
        for core in field(7).split(' ') {
            let core: u32 = core
                .parse()
                .ok()
                .filter(|&c| c < processors)
                .ok_or_else(|| {
                    refuse(
                        header_line(7),
                        format!("`{core}` is not a logical processor of {processors}"),
                    )
                })?;
            if performance.last().is_some_and(|&last| last >= core) {
                return Err(refuse(
                    header_line(7),
                    "the performance cores are listed once each, in increasing order",
                ));
            }
            performance.push(core);
        }
        let os = text_field(8)?;
        let toolchain = text_field(9)?;
        let round_ms: u32 = field(10)
            .parse()
            .ok()
            .filter(|ms| (1..=60_000).contains(ms))
            .ok_or_else(|| refuse(header_line(10), "a round is 1 to 60000 milliseconds"))?;

        let mut engines: Vec<(String, String)> = Vec::new();
        while let Some(&(n, line)) = lines.peek() {
            let (key, value) = key_value(line);
            if key != "engine" {
                break;
            }
            lines.next();
            let (name, version) = key_value(value);
            let name = engine_field(name, n)?;
            if !is_text(version) {
                return Err(refuse(n, "an engine needs its version"));
            }
            let order = |engine: &str| ENGINES.iter().position(|&e| e == engine);
            if engines
                .last()
                .is_some_and(|(last, _)| order(last) >= order(&name))
            {
                return Err(refuse(
                    n,
                    "engines are listed once each, in the order of ENGINES",
                ));
            }
            engines.push((name, version.to_string()));
        }
        if engines.is_empty() {
            return Err(refuse(0, "a record names at least one engine"));
        }

        let mut record = Record {
            commit: commit.to_string(),
            night: night.to_string(),
            started,
            finished,
            status,
            machine,
            processors,
            performance,
            os,
            toolchain,
            round_ms,
            engines,
            timings: Vec::new(),
            failures: Vec::new(),
        };
        let mut seen: BTreeMap<(String, u32, String), usize> = BTreeMap::new();
        for (n, line) in lines {
            let (key, rest) = key_value(line);
            let (engine, threads, benchmark) = match key {
                "time" => {
                    let timing = parse_timing(rest, n)?;
                    let at = (
                        timing.engine.clone(),
                        timing.threads,
                        timing.benchmark.clone(),
                    );
                    record.timings.push(timing);
                    at
                }
                "fail" => {
                    let mut parts = rest.splitn(4, ' ');
                    let mut next = || parts.next().unwrap_or("");
                    let engine = engine_field(next(), n)?;
                    let threads = threads_field(next(), n)?;
                    let benchmark = benchmark_field(next(), n)?;
                    let reason = next();
                    if !is_text(reason) {
                        return Err(refuse(n, "a failure needs its reason"));
                    }
                    record.failures.push(Failure {
                        engine: engine.clone(),
                        threads,
                        benchmark: benchmark.clone(),
                        reason: reason.to_string(),
                    });
                    (engine, threads, benchmark)
                }
                _ => return Err(refuse(n, format!("unknown line `{key}`"))),
            };
            if !record.engines.iter().any(|(name, _)| *name == engine) {
                return Err(refuse(n, format!("engine {engine} is not in the header")));
            }
            if let Some(first) = seen.insert((engine, threads, benchmark.clone()), n) {
                return Err(refuse(
                    n,
                    format!(
                        "{benchmark} is already on line {first} for this engine and thread count"
                    ),
                ));
            }
        }
        Ok(record)
    }

    /// Whether `engine` was available this night.
    #[must_use]
    pub fn available(&self, engine: &str) -> bool {
        self.engines
            .iter()
            .any(|(name, version)| name == engine && !version.starts_with("unavailable"))
    }

    /// The timing of `benchmark` on `engine` at `threads`, if the night took one.
    #[must_use]
    pub fn timing(&self, engine: &str, threads: u32, benchmark: &str) -> Option<&Timing> {
        self.timings
            .iter()
            .find(|t| t.engine == engine && t.threads == threads && t.benchmark == benchmark)
    }

    /// Whether `timing` was disturbed: the machine was busier than its own threads explain by more
    /// than [`DISTURBED_POINTS`].
    #[must_use]
    pub fn disturbed(&self, timing: &Timing) -> bool {
        let own = (u64::from(timing.threads) * 100).div_ceil(u64::from(self.processors.max(1)));
        timing
            .busy
            .is_some_and(|busy| u64::from(busy) > own + u64::from(DISTURBED_POINTS))
    }

    /// Whether `timing` is known to be quiet: the machine was sampled during its job, and it was
    /// not disturbed. Only quiet timings are compared from night to night.
    #[must_use]
    pub fn quiet(&self, timing: &Timing) -> bool {
        timing.busy.is_some() && !self.disturbed(timing)
    }
}

/// The rest of a `time` line: engine, threads, benchmark, ns, passes, slowest and busy.
fn parse_timing(rest: &str, n: usize) -> Result<Timing, Refused> {
    let parts: Vec<&str> = rest.split(' ').collect();
    let [engine, threads, benchmark, ns, passes, slowest, busy] = parts.as_slice() else {
        return Err(refuse(
            n,
            "a timing has seven fields: engine, threads, benchmark, ns, passes, slowest, busy",
        ));
    };
    let engine = engine_field(engine, n)?;
    let threads = threads_field(threads, n)?;
    let benchmark = benchmark_field(benchmark, n)?;
    let ns = ns_field(ns, n)?;
    let passes: u64 = passes
        .parse()
        .map_err(|_| refuse(n, format!("the passes are a whole number, not {passes}")))?;
    let slowest = ns_field(slowest, n)?;
    if slowest < ns {
        return Err(refuse(
            n,
            "the slowest thread cannot be faster than the median",
        ));
    }
    let busy = match *busy {
        "-" => None,
        text => Some(
            text.parse::<u32>()
                .ok()
                .filter(|&b| b <= 100)
                .ok_or_else(|| {
                    refuse(
                        n,
                        format!("busy is a percentage from 0 to 100 or `-`, not {text}"),
                    )
                })?,
        ),
    };
    Ok(Timing {
        engine,
        threads,
        benchmark,
        ns,
        passes,
        slowest,
        busy,
    })
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{FORMAT}")?;
        writeln!(f, "commit {}", self.commit)?;
        writeln!(f, "night {}", self.night)?;
        writeln!(f, "started {}", self.started)?;
        writeln!(f, "finished {}", self.finished)?;
        writeln!(f, "status {}", self.status.word())?;
        writeln!(f, "machine {}", self.machine)?;
        writeln!(f, "processors {}", self.processors)?;
        let performance: Vec<String> = self.performance.iter().map(u32::to_string).collect();
        writeln!(f, "performance {}", performance.join(" "))?;
        writeln!(f, "os {}", self.os)?;
        writeln!(f, "toolchain {}", self.toolchain)?;
        writeln!(f, "round {}", self.round_ms)?;
        for (engine, version) in &self.engines {
            writeln!(f, "engine {engine} {version}")?;
        }
        for t in &self.timings {
            let busy = t.busy.map_or_else(|| "-".to_string(), |b| b.to_string());
            writeln!(
                f,
                "time {} {} {} {} {} {} {busy}",
                t.engine, t.threads, t.benchmark, t.ns, t.passes, t.slowest
            )?;
        }
        for x in &self.failures {
            writeln!(
                f,
                "fail {} {} {} {}",
                x.engine, x.threads, x.benchmark, x.reason
            )?;
        }
        Ok(())
    }
}

/// The machine's busy samples: each `<milliseconds since 1970> <percent busy over the second
/// before>`, one per line, as the runner's sampler writes them.
///
/// # Errors
///
/// A line that is not two whole numbers, or a percentage over 100.
pub fn parse_busy(text: &str) -> Result<Vec<(u64, u32)>, Refused> {
    let mut samples = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.is_empty() {
            continue;
        }
        let (at, busy) = key_value(line);
        let at: u64 = at
            .parse()
            .map_err(|_| refuse(n, "a sample's time is whole milliseconds since 1970"))?;
        let busy: u32 = busy.parse().ok().filter(|&b| b <= 100).ok_or_else(|| {
            refuse(
                n,
                "a sample's busy share is a whole percentage from 0 to 100",
            )
        })?;
        samples.push((at, busy));
    }
    Ok(samples)
}

/// Milliseconds each busy sample covers: the sampler writes, once a second, how busy the machine
/// was over the second before (`reference/busy.ps1`).
pub const SAMPLE_MS: u64 = 1000;

/// The mean busy share of the samples whose second overlaps the job from `start` to `end`
/// (a sample taken at `at` covers `at - SAMPLE_MS` to `at`: so `at > start` and
/// `at - SAMPLE_MS < end`), rounded half up, or `None` when no sample overlaps it.
#[must_use]
pub fn busy_between(samples: &[(u64, u32)], start: u64, end: u64) -> Option<u32> {
    let (sum, count) = samples
        .iter()
        .filter(|&&(at, _)| at > start && at.saturating_sub(SAMPLE_MS) < end)
        .fold((0u64, 0u64), |(sum, count), &(_, busy)| {
            (sum + u64::from(busy), count + 1)
        });
    if count == 0 {
        return None;
    }
    u32::try_from((2 * sum + count) / (2 * count)).ok()
}

/// One job line of the runner, as [`assemble`] reads it.
struct Job {
    engine: String,
    threads: u32,
    start: u64,
    end: u64,
    outcome: Result<Timing, Failure>,
}

/// Reads one job line: `job <engine> <threads> <start ms> <end ms> <exit status> <text>`, where an
/// exit status of 0 has the guest's line for the text
/// (`<name> <ns> <passes> <threads> <slowest>`, its threads those of the job), and any other has
/// the benchmark's name and why it failed.
fn parse_job(line: &str, n: usize) -> Result<Job, Refused> {
    let mut parts = line.splitn(7, ' ');
    let mut next = || parts.next().unwrap_or("");
    if next() != "job" {
        return Err(refuse(n, "a job line starts with `job`"));
    }
    let engine = engine_field(next(), n)?;
    let threads = threads_field(next(), n)?;
    let millis = |text: &str| -> Result<u64, Refused> {
        text.parse()
            .map_err(|_| refuse(n, "a job's start and end are whole milliseconds since 1970"))
    };
    let start = millis(next())?;
    let end = millis(next())?;
    if end < start {
        return Err(refuse(n, "a job ended before it started"));
    }
    let status: u8 = next()
        .parse()
        .map_err(|_| refuse(n, "a job's exit status is a number"))?;
    let text = next();
    let outcome = if status == 0 {
        let fields: Vec<&str> = text.split(' ').collect();
        let [name, ns, passes, line_threads, slowest] = fields.as_slice() else {
            return Err(refuse(
                n,
                "a finished job's text is the guest's line: name, ns, passes, threads, slowest",
            ));
        };
        if threads_field(line_threads, n)? != threads {
            return Err(refuse(
                n,
                "the guest timed a different thread count from the job's",
            ));
        }
        let timing = parse_timing(
            &format!("{engine} {threads} {name} {ns} {passes} {slowest} -"),
            n,
        )?;
        Ok(timing)
    } else {
        let (name, why) = key_value(text);
        let benchmark = benchmark_field(name, n)?;
        let why = why.trim();
        Err(Failure {
            engine: engine.clone(),
            threads,
            benchmark,
            reason: if why.is_empty() {
                format!("exit status {status}")
            } else {
                format!("exit status {status}: {why}")
            },
        })
    };
    Ok(Job {
        engine,
        threads,
        start,
        end,
        outcome,
    })
}

/// Builds the night's record: the header (a record's lines up to and including the engines, with
/// no timings), the busy samples ([`parse_busy`]) and the runner's job lines, against the names of
/// every benchmark of the catalogue the night timed.
///
/// Each timing's busy share is the mean of the samples taken while its job ran. Every benchmark of
/// `names`, on every engine of the header and at every count of [`THREADS`], gets exactly one line:
/// its timing, its failure, or, for a job that never ran, a failure saying the night stopped at its
/// deadline or the engine was unavailable.
///
/// # Errors
///
/// A malformed header, sample or job line; a job for an engine the header does not name, a thread
/// count outside [`THREADS`] or a benchmark outside `names`; the same job twice; or a job missing
/// from a night whose status is `complete`, on an engine that was available.
pub fn assemble(header: &str, busy: &str, jobs: &str, names: &[String]) -> Result<Record, Refused> {
    let mut record = Record::parse(header)?;
    if !record.timings.is_empty() || !record.failures.is_empty() {
        return Err(refuse(
            0,
            "the header holds timings; it ends with the engines",
        ));
    }
    let samples = parse_busy(busy)?;
    let known: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut done: BTreeMap<(String, u32, String), ()> = BTreeMap::new();
    for (i, line) in jobs.lines().enumerate() {
        let n = i + 1;
        if line.is_empty() {
            continue;
        }
        let job = parse_job(line, n)?;
        if !record.engines.iter().any(|(name, _)| *name == job.engine) {
            return Err(refuse(
                n,
                format!("engine {} is not in the header", job.engine),
            ));
        }
        if !THREADS.contains(&job.threads) {
            return Err(refuse(
                n,
                format!(
                    "the nightly times at {THREADS:?} threads, not {}",
                    job.threads
                ),
            ));
        }
        let benchmark = match &job.outcome {
            Ok(timing) => timing.benchmark.clone(),
            Err(failure) => failure.benchmark.clone(),
        };
        if !known.contains_key(benchmark.as_str()) {
            return Err(refuse(
                n,
                format!("{benchmark} is not a benchmark of the catalogue"),
            ));
        }
        if done
            .insert((job.engine.clone(), job.threads, benchmark.clone()), ())
            .is_some()
        {
            return Err(refuse(
                n,
                format!(
                    "{benchmark} was already timed on {} at {} threads",
                    job.engine, job.threads
                ),
            ));
        }
        match job.outcome {
            Ok(mut timing) => {
                timing.busy = busy_between(&samples, job.start, job.end);
                record.timings.push(timing);
            }
            Err(failure) => record.failures.push(failure),
        }
    }
    let engines: Vec<(String, bool)> = record
        .engines
        .iter()
        .map(|(engine, _)| (engine.clone(), record.available(engine)))
        .collect();
    for (engine, available) in engines {
        for threads in THREADS {
            for name in names {
                if done.contains_key(&(engine.clone(), threads, name.clone())) {
                    continue;
                }
                let reason = if !available {
                    "engine unavailable"
                } else if record.status == Status::Deadline {
                    "not reached before the deadline"
                } else {
                    return Err(refuse(
                        0,
                        format!(
                            "{name} was not timed on {engine} at {threads} threads, and the night is complete"
                        ),
                    ));
                };
                record.failures.push(Failure {
                    engine: engine.clone(),
                    threads,
                    benchmark: name.clone(),
                    reason: reason.to_string(),
                });
            }
        }
    }
    // Read back what was built, so a record that [`Record::parse`] would refuse is never written.
    Record::parse(&record.to_string()).map_err(|refused| {
        refuse(
            0,
            format!("the assembled record does not read back: {refused}"),
        )
    })
}

/// What a share limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Limit {
    /// An accepted share (32.16): the time must stay within it, in nanoseconds.
    Within(f64),
    /// An estimate to be measured (23.1's browser shares): a time from `low` to `high` nanoseconds.
    Estimate {
        /// The estimate's low end.
        low: f64,
        /// The estimate's high end.
        high: f64,
    },
    /// A share that is not a time: why nothing is timed for it.
    NotTimed(&'static str),
}

/// Which engines a share applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// The native build on the reference machine.
    Native,
    /// Each of [`BROWSERS`].
    Browsers,
}

/// One of chapter 23.1's kernel shares, and the benchmark that measures it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share {
    /// A short name for the report.
    pub id: &'static str,
    /// The whole program's target, as the first column of 23.1's table words it, or the
    /// paragraph of 23.1 it comes from.
    pub target: &'static str,
    /// The share as 23.1 words it, a fragment of its table row or paragraph.
    pub words: &'static str,
    /// The limit those words set.
    pub limit: Limit,
    /// The engines it applies to.
    pub on: On,
    /// The thread count its figure is read at: 8 for the machine of 32.16, timed on its 8
    /// performance cores; 1 for a single trivial operation and for the browsers, where the kernel
    /// runs serially in one Web Worker (23.4).
    pub threads: u32,
    /// The benchmark that measures it: the full-scale scenario that piece G451 adds, named
    /// `scenario/<id>/full`.
    pub benchmark: &'static str,
}

/// Chapter 23.1's shares, accepted by the owner in decision 32.16, and its browser estimates.
/// Times are in nanoseconds. `tests/attack_g107.rs` checks every row and paragraph of 23.1 they
/// come from.
#[rustfmt::skip]
pub static SHARES: &[Share] = &[
    Share { id: "open", target: "Open the 230,000-element model to first view in under 3 s; a 20-building block in under 5 s", words: "1 s", limit: Limit::Within(1e9), on: On::Native, threads: 8, benchmark: "scenario/open/full" },
    Share { id: "open_block", target: "Open the 230,000-element model to first view in under 3 s; a 20-building block in under 5 s", words: "1.5 s for the block", limit: Limit::Within(1.5e9), on: On::Native, threads: 8, benchmark: "scenario/open_block/full" },
    Share { id: "door_move", target: "Move a door in under 16 ms", words: "4 ms", limit: Limit::Within(4e6), on: On::Native, threads: 8, benchmark: "scenario/door_move/full" },
    Share { id: "level_change", target: "Change a level height touching 2,000 walls in under 1 s", words: "600 ms", limit: Limit::Within(6e8), on: On::Native, threads: 8, benchmark: "scenario/level_change/full" },
    Share { id: "navigate", target: "Navigate 1,000,000 instances at 60 fps", words: "Nothing per frame", limit: Limit::NotTimed("the kernel is never in the frame loop"), on: On::Native, threads: 8, benchmark: "scenario/navigate/full" },
    Share { id: "plan_first", target: "Plan view: first draw under 500 ms, under 16 ms after an edit", words: "250 ms first", limit: Limit::Within(2.5e8), on: On::Native, threads: 8, benchmark: "scenario/plan_first/full" },
    Share { id: "plan_edit", target: "Plan view: first draw under 500 ms, under 16 ms after an edit", words: "4 ms per edit", limit: Limit::Within(4e6), on: On::Native, threads: 8, benchmark: "scenario/plan_edit/full" },
    Share { id: "instance_memory", target: "Memory per placed instance: under 256 bytes plus shared type geometry", words: "About 140 bytes", limit: Limit::NotTimed("bytes, not time: G451's instance-memory scenario counts them"), on: On::Native, threads: 8, benchmark: "scenario/instance_memory/full" },
    Share { id: "save", target: "Save in under 100 ms, never blocking", words: "Nothing on the save path", limit: Limit::NotTimed("saving appends to the operation log; encoding runs in background compaction, timed by save_local and save_level"), on: On::Native, threads: 8, benchmark: "scenario/save/full" },
    Share { id: "save_local", target: "Save in under 100 ms, never blocking", words: "at most 10 ms for a local edit", limit: Limit::Within(1e7), on: On::Native, threads: 8, benchmark: "scenario/save_local/full" },
    Share { id: "save_level", target: "Save in under 100 ms, never blocking", words: "about 100 ms for the level change", limit: Limit::Within(1e8), on: On::Native, threads: 8, benchmark: "scenario/save_level/full" },
    Share { id: "replay", target: "Another user sees an edit in under 200 ms on a LAN", words: "4 ms for a door move", limit: Limit::Within(4e6), on: On::Native, threads: 8, benchmark: "scenario/replay/full" },
    Share { id: "print", target: "Print a 100-sheet set to PDF in under 60 s", words: "40 s", limit: Limit::Within(4e10), on: On::Native, threads: 8, benchmark: "scenario/print/full" },
    Share { id: "trivial_op", target: "Computational design", words: "at most 5 µs of overhead for a trivial operation on native targets", limit: Limit::Within(5e3), on: On::Native, threads: 1, benchmark: "scenario/trivial_op/full" },
    Share { id: "browser_door_move", target: "In a browser", words: "a door move within 4 ms on hosts with up to about 10 openings", limit: Limit::Estimate { low: 0.0, high: 4e6 }, on: On::Browsers, threads: 1, benchmark: "scenario/door_move_small/full" },
    Share { id: "browser_level_change", target: "In a browser", words: "the 2,000-wall level change about 1 to 6 s serially", limit: Limit::Estimate { low: 1e9, high: 6e9 }, on: On::Browsers, threads: 1, benchmark: "scenario/level_change/full" },
    Share { id: "browser_open", target: "In a browser", words: "open and plan view within their shares for the chunks in view", limit: Limit::Estimate { low: 0.0, high: 1e9 }, on: On::Browsers, threads: 1, benchmark: "scenario/open/full" },
    Share { id: "browser_plan_first", target: "In a browser", words: "open and plan view within their shares for the chunks in view", limit: Limit::Estimate { low: 0.0, high: 2.5e8 }, on: On::Browsers, threads: 1, benchmark: "scenario/plan_first/full" },
    Share { id: "browser_plan_edit", target: "In a browser", words: "open and plan view within their shares for the chunks in view", limit: Limit::Estimate { low: 0.0, high: 4e6 }, on: On::Browsers, threads: 1, benchmark: "scenario/plan_edit/full" },
];

/// A time in nanoseconds, written with three significant figures in the largest unit under it.
#[must_use]
pub fn duration(ns: f64) -> String {
    let (value, unit) = if ns >= 1e9 {
        (ns / 1e9, "s")
    } else if ns >= 1e6 {
        (ns / 1e6, "ms")
    } else if ns >= 1e3 {
        (ns / 1e3, "µs")
    } else {
        (ns, "ns")
    };
    let decimals = if value >= 100.0 {
        0
    } else if value >= 10.0 {
        1
    } else {
        2
    };
    format!("{value:.decimals$} {unit}")
}

/// The upper median of `values` (the element at half their count once sorted), or `None` for none.
#[must_use]
pub fn upper_median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied()
}

/// A benchmark's group: its name up to the first `/`.
fn group(benchmark: &str) -> &str {
    benchmark.split('/').next().unwrap_or(benchmark)
}

/// The engines a share is read on.
fn share_engines(share: &Share) -> &'static [&'static str] {
    match share.on {
        On::Native => &["native"],
        On::Browsers => &BROWSERS,
    }
}

/// What one night says of a share on one engine: the time, or why there is none.
fn share_value(night: &Record, share: &Share, engine: &str) -> Result<f64, String> {
    if !night.engines.iter().any(|(name, _)| name == engine) || !night.available(engine) {
        return Err(format!("{engine} not run"));
    }
    match night.timing(engine, share.threads, share.benchmark) {
        Some(t) => Ok(t.ns),
        None => match night.failures.iter().find(|x| {
            x.engine == engine && x.threads == share.threads && x.benchmark == share.benchmark
        }) {
            Some(x) => Err(format!("failed: {}", x.reason)),
            None => Err("not measured yet: no such benchmark (piece G451 adds it)".to_string()),
        },
    }
}

/// The verdict on a time against a share's limit.
fn verdict(limit: &Limit, ns: f64) -> &'static str {
    match *limit {
        Limit::Within(share) if ns <= share => "within",
        Limit::Within(_) => "**over** (a miss; G452 confirms it with a same-night rerun)",
        Limit::Estimate { low, high } if ns >= low && ns <= high => "inside the estimate",
        Limit::Estimate { .. } => "outside the estimate",
        Limit::NotTimed(_) => "not timed",
    }
}

/// The limit as the report writes it.
fn limit_text(limit: &Limit) -> String {
    match *limit {
        Limit::Within(ns) => duration(ns),
        Limit::Estimate { low: 0.0, high } => format!("estimate: up to {}", duration(high)),
        Limit::Estimate { low, high } => {
            format!("estimate: {} to {}", duration(low), duration(high))
        }
        Limit::NotTimed(why) => format!("not a time: {why}"),
    }
}

/// A time in UTC from milliseconds since 1970, `YYYY-MM-DD hh:mm UTC` (the proleptic Gregorian
/// calendar, by Howard Hinnant's `civil_from_days`).
#[must_use]
pub fn utc(millis: u64) -> String {
    let seconds = millis / 1000;
    let days = seconds / 86_400;
    let (hour, minute) = ((seconds % 86_400) / 3600, (seconds % 3600) / 60);
    // civil_from_days, for days since 1970-01-01 (never negative here).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

/// The night's report, in Markdown, from the history: every record, oldest first. The last is the
/// night reported; the one before it, if any, is what it is compared with.
///
/// It never blocks anything: a share missed is reported, and G452 turns misses into a block.
#[must_use]
pub fn report(history: &[Record]) -> String {
    let mut out = String::new();
    let Some(night) = history.last() else {
        return "# Reference timings\n\nNo night has been recorded yet.\n".to_string();
    };
    let before = history.len().checked_sub(2).and_then(|i| history.get(i));
    let _ = writeln!(out, "# Reference timings: the night of {}\n", night.night);
    let _ = writeln!(
        out,
        "Commit `{}`, {} to {}, {}. {} on its performance cores ({}), {}; {}; rounds of {} ms. \
         Timings report and never block: G452 makes a confirmed miss block (32.42).\n",
        night.commit,
        utc(night.started),
        utc(night.finished),
        match night.status {
            Status::Complete => "every job ran",
            Status::Deadline => "stopped at the 08:00 deadline",
        },
        night.machine,
        night
            .performance
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        night.os,
        night.toolchain,
        night.round_ms,
    );
    let _ = writeln!(
        out,
        "| Engine | Version | Timings | Failures | Disturbed | Not sampled |"
    );
    let _ = writeln!(out, "| --- | --- | ---: | ---: | ---: | ---: |");
    for (engine, version) in &night.engines {
        let timings: Vec<&Timing> = night
            .timings
            .iter()
            .filter(|t| t.engine == *engine)
            .collect();
        let failures = night
            .failures
            .iter()
            .filter(|x| x.engine == *engine)
            .count();
        let disturbed = timings.iter().filter(|t| night.disturbed(t)).count();
        let unsampled = timings.iter().filter(|t| t.busy.is_none()).count();
        let changed = before
            .and_then(|b| b.engines.iter().find(|(e, _)| e == engine))
            .filter(|(_, v)| v != version)
            .map(|(_, v)| format!(" (was {v})"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "| {engine} | {version}{changed} | {} | {failures} | {disturbed} | {unsampled} |",
            timings.len()
        );
    }

    let _ = writeln!(out, "\n## The kernel's shares (23.1, accepted in 32.16)\n");
    let _ = writeln!(
        out,
        "| Share | Target | Limit | Engine | Threads | Benchmark | This night | Verdict | Last {SHARE_NIGHTS} nights |"
    );
    let _ = writeln!(
        out,
        "| --- | --- | --- | --- | ---: | --- | ---: | --- | --- |"
    );
    let recent: Vec<&Record> = history.iter().rev().take(SHARE_NIGHTS).rev().collect();
    for share in SHARES {
        for &engine in share_engines(share) {
            let (value, verdict_text) = match (&share.limit, share_value(night, share, engine)) {
                (Limit::NotTimed(_), _) => ("-".to_string(), "not timed".to_string()),
                (limit, Ok(ns)) => (duration(ns), verdict(limit, ns).to_string()),
                (_, Err(why)) => ("-".to_string(), why),
            };
            let trend: Vec<String> = recent
                .iter()
                .map(|r| share_value(r, share, engine).map_or_else(|_| "-".to_string(), duration))
                .collect();
            let _ = writeln!(
                out,
                "| {} | {} | {} | {engine} | {} | `{}` | {value} | {verdict_text} | {} |",
                share.id,
                share.target,
                limit_text(&share.limit),
                share.threads,
                share.benchmark,
                trend.join(", ")
            );
        }
    }

    let _ = writeln!(out, "\n## Changes since the night before\n");
    match before {
        None => {
            let _ = writeln!(out, "This is the first night recorded.");
        }
        Some(before) => {
            let mut moves: Vec<(f64, &Timing, f64)> = Vec::new();
            for t in &night.timings {
                if !night.quiet(t) {
                    continue;
                }
                let Some(b) = before.timing(&t.engine, t.threads, &t.benchmark) else {
                    continue;
                };
                if !before.quiet(b) || b.ns <= 0.0 {
                    continue;
                }
                let change = (t.ns - b.ns) / b.ns * 100.0;
                if change.abs() > MOVE_PERCENT {
                    moves.push((change, t, b.ns));
                }
            }
            moves.sort_by(|a, b| {
                b.0.abs()
                    .total_cmp(&a.0.abs())
                    .then_with(|| a.1.engine.cmp(&b.1.engine))
                    .then_with(|| a.1.threads.cmp(&b.1.threads))
                    .then_with(|| a.1.benchmark.cmp(&b.1.benchmark))
            });
            let _ = writeln!(
                out,
                "Quiet timings (sampled, and not disturbed) that moved by more than {MOVE_PERCENT}% \
                 against the night of {} \
                 (commit `{}`): {}. Wall-clock time is noisy; a move is a lead to look at, not a \
                 verdict.\n",
                before.night,
                before.commit,
                moves.len()
            );
            if !moves.is_empty() {
                let _ = writeln!(
                    out,
                    "| Engine | Threads | Benchmark | Before | Now | Change |"
                );
                let _ = writeln!(out, "| --- | ---: | --- | ---: | ---: | ---: |");
                for (change, t, was) in &moves {
                    let _ = writeln!(
                        out,
                        "| {} | {} | `{}` | {} | {} | {change:+.1}% |",
                        t.engine,
                        t.threads,
                        t.benchmark,
                        duration(*was),
                        duration(t.ns)
                    );
                }
            }
        }
    }

    let _ = writeln!(out, "\n## How each group scales\n");
    let _ = writeln!(
        out,
        "Medians over each group's benchmarks: the time per input at {} threads over the time at \
         {}, on each engine (1.00 is perfect scaling), and each browser's time at {} thread over \
         the native time.\n",
        THREADS.get(1).copied().unwrap_or(8),
        THREADS.first().copied().unwrap_or(1),
        THREADS.first().copied().unwrap_or(1),
    );
    let engines: Vec<&str> = night.engines.iter().map(|(e, _)| e.as_str()).collect();
    let mut header = vec!["Group".to_string()];
    header.extend(engines.iter().map(|e| format!("{e} 8/1")));
    header.extend(
        engines
            .iter()
            .filter(|e| **e != "native")
            .map(|e| format!("{e}/native")),
    );
    let _ = writeln!(out, "| {} |", header.join(" | "));
    let _ = writeln!(out, "|{}", " --- |".repeat(header.len()));
    let mut groups: BTreeMap<&str, ()> = BTreeMap::new();
    for t in &night.timings {
        groups.insert(group(&t.benchmark), ());
    }
    for g in groups.keys() {
        let ratio = |engine: &str, threads: u32, over_engine: &str, over_threads: u32| -> String {
            let ratios: Vec<f64> = night
                .timings
                .iter()
                .filter(|t| t.engine == engine && t.threads == threads && group(&t.benchmark) == *g)
                .filter_map(|t| {
                    let base = night.timing(over_engine, over_threads, &t.benchmark)?;
                    (base.ns > 0.0).then(|| t.ns / base.ns)
                })
                .collect();
            upper_median(ratios).map_or_else(|| "-".to_string(), |r| format!("{r:.2}"))
        };
        let mut row = vec![format!("`{g}`")];
        row.extend(engines.iter().map(|e| ratio(e, 8, e, 1)));
        row.extend(
            engines
                .iter()
                .filter(|e| **e != "native")
                .map(|e| ratio(e, 1, "native", 1)),
        );
        let _ = writeln!(out, "| {} |", row.join(" | "));
    }

    let disturbed: Vec<&Timing> = night
        .timings
        .iter()
        .filter(|t| night.disturbed(t))
        .collect();
    let _ = writeln!(out, "\n## Disturbed timings\n");
    let _ = writeln!(
        out,
        "Timings taken while the machine was busier than the job's own threads explain by more \
         than {DISTURBED_POINTS} points: {}. They, and the timings no busy sample covers, are left out of the changes above.\n",
        disturbed.len()
    );
    if !disturbed.is_empty() {
        let _ = writeln!(out, "| Engine | Threads | Benchmark | Time | Busy |");
        let _ = writeln!(out, "| --- | ---: | --- | ---: | ---: |");
        for t in disturbed {
            let _ = writeln!(
                out,
                "| {} | {} | `{}` | {} | {}% |",
                t.engine,
                t.threads,
                t.benchmark,
                duration(t.ns),
                t.busy.unwrap_or(0)
            );
        }
    }

    let _ = writeln!(out, "\n## Failures\n");
    let _ = writeln!(out, "Benchmarks not timed: {}.\n", night.failures.len());
    if !night.failures.is_empty() {
        // One row per reason and engine, so a whole engine that was unavailable is one row.
        let mut by_reason: BTreeMap<(&str, &str), Vec<String>> = BTreeMap::new();
        for x in &night.failures {
            by_reason
                .entry((x.engine.as_str(), x.reason.as_str()))
                .or_default()
                .push(format!("`{}` at {}", x.benchmark, x.threads));
        }
        let _ = writeln!(out, "| Engine | Why | Count | First |");
        let _ = writeln!(out, "| --- | --- | ---: | --- |");
        for ((engine, reason), items) in by_reason {
            let _ = writeln!(
                out,
                "| {engine} | {reason} | {} | {} |",
                items.len(),
                items.first().map_or("", String::as_str)
            );
        }
    }
    out
}
