//! Attacks on G107 (issue #415): the reference runner times every benchmark nightly at 1 and 8
//! threads, natively and in Chrome, Firefox and WebKit, and keeps the history against 23.1's
//! shares; it reports and does not block.
//!
//! Each test tries to break one claim of the done-when or of `geom_bench::timings`' guarantees:
//! the shares against every row and paragraph of 23.1 they come from, the record's strict reading
//! and its exact round trip, the assembly's completeness and its busy samples, the report's
//! comparisons and verdicts, the guest's threaded `time` and the tool's exit statuses, and the
//! runner's wiring (its pins, engines, thread counts, header order and job lines) against the code
//! that reads what it writes.

#![cfg(feature = "reference")]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken seeded record must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code, including its seeding helpers: a failure should stop loudly"
)]

use geom_bench::timings::{
    BROWSERS, DISTURBED_POINTS, ENGINES, FORMAT, Limit, MAX_THREADS, On, Record, SAMPLE_MS, SHARES,
    Status, THREADS, assemble, busy_between, duration, parse_busy, report, upper_median, utc,
};

const CHAPTER: &str = include_str!("../../../architecture/23-performance.md");
const RUNNER: &str = include_str!("../reference/nightly.sh");
const PAGE: &str = include_str!("../reference/page.mjs");
const DRIVER: &str = include_str!("../reference/browsers.mjs");
const WORKER: &str = include_str!("../reference/worker.mjs");
const VARIANTS: &str = include_str!("../../../../../../.github/workflows/kernel-variants.yml");

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// A record's header, up to and including the engines.
fn header(status: &str, engines: &[(&str, &str)]) -> String {
    let mut text = format!(
        "{FORMAT}\ncommit {COMMIT}\nnight 2026-10-31\nstarted 1000\nfinished 900000\n\
         status {status}\nmachine A processor\nprocessors 20\nperformance 0 1 10 11 12 13 18 19\n\
         os An operating system 1.0\ntoolchain rustc 1.99.0\nround 150\n"
    );
    for (engine, version) in engines {
        text.push_str(&format!("engine {engine} {version}\n"));
    }
    text
}

/// A whole sample record: a timing of every kind, and a failure.
fn sample() -> String {
    header(
        "complete",
        &[
            ("native", "x86_64-pc-windows-gnullvm"),
            ("chrome", "156.0, isolated"),
        ],
    ) + "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7\n\
           time native 8 predicate/orient2d/typical 7.76 86123 8.397 -\n\
           time chrome 1 predicate/orient2d/typical 12.125 5000 12.125 100\n\
           fail chrome 8 predicate/orient2d/typical exit status 1: the results changed\n"
}

/// The cells of 23.1's table: target, kernel's share, what the kernel does.
fn table_rows() -> Vec<[String; 3]> {
    let section = &CHAPTER[CHAPTER.find("## 23.1 ").unwrap()..CHAPTER.find("## 23.2 ").unwrap()];
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in section.lines() {
        if line.starts_with("| Target (whole program) |") {
            in_table = true;
            continue;
        }
        if !in_table || line.starts_with("| ---") {
            continue;
        }
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<String> = line
            .trim_start_matches("| ")
            .trim_end_matches(" |")
            .split(" | ")
            .map(str::to_string)
            .collect();
        assert_eq!(cells.len(), 3, "23.1's table has three columns: {line}");
        rows.push([cells[0].clone(), cells[1].clone(), cells[2].clone()]);
    }
    assert!(rows.len() >= 9, "23.1's table was not found or lost rows");
    rows
}

/// The paragraph of 23.1 that opens with `opening`.
fn paragraph(opening: &str) -> &'static str {
    CHAPTER
        .lines()
        .find(|l| l.starts_with(opening))
        .unwrap_or_else(|| panic!("23.1 has no paragraph opening {opening}"))
}

/// Every `<number> <unit>` in `text`, in nanoseconds, for the units s, ms and µs.
fn durations(text: &str) -> Vec<f64> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found = Vec::new();
    for pair in words.windows(2) {
        let Ok(value) = pair[0].parse::<f64>() else {
            continue;
        };
        let unit = pair[1].trim_end_matches([',', ';', '.', ')']);
        let scale = match unit {
            "s" => 1e9,
            "ms" => 1e6,
            "µs" => 1e3,
            _ => continue,
        };
        found.push(value * scale);
    }
    found
}

/// The text a share's words must come from.
fn source(target: &str) -> String {
    if let Some(row) = table_rows().into_iter().find(|r| r[0] == target) {
        return format!("{} {}", row[1], row[2]);
    }
    match target {
        "In a browser" => paragraph("**In a browser.**").to_string(),
        "Computational design" => paragraph("**Computational design**").to_string(),
        _ => panic!(
            "share target {target} is neither a row of 23.1's table nor one of its paragraphs"
        ),
    }
}

#[test]
fn every_row_of_23_1_has_its_native_share() {
    for row in table_rows() {
        let shares: Vec<_> = SHARES.iter().filter(|s| s.target == row[0]).collect();
        assert!(!shares.is_empty(), "23.1's row `{}` has no share", row[0]);
        assert!(
            shares.iter().all(|s| s.on == On::Native),
            "{}: the table's shares are native",
            row[0]
        );
        // A row whose share is a time has a timed share for each time its share column names.
        let named = durations(&row[1]);
        let timed: Vec<f64> = shares
            .iter()
            .filter_map(|s| match s.limit {
                Limit::Within(ns) if row[1].contains(s.words) => Some(ns),
                _ => None,
            })
            .collect();
        assert_eq!(timed, named, "{}: the share column names {named:?}", row[0]);
    }
}

#[test]
fn every_share_says_what_23_1_says() {
    for share in SHARES {
        let text = source(share.target);
        assert!(
            text.contains(share.words),
            "{}: `{}` is not in 23.1's text for `{}`",
            share.id,
            share.words,
            share.target
        );
        match share.limit {
            Limit::Within(ns) => {
                assert_eq!(
                    durations(share.words).first().copied(),
                    Some(ns),
                    "{}: the limit is not what its words say",
                    share.id
                );
            }
            Limit::Estimate { low, high } => {
                let words: Vec<&str> = share.words.split_whitespace().collect();
                let named = durations(share.words);
                let number = |w: Option<&&str>| w.is_some_and(|w| w.parse::<f64>().is_ok());
                if let Some(i) = (1..words.len()).find(|&i| {
                    words[i] == "to" && number(words.get(i - 1)) && number(words.get(i + 1))
                }) {
                    // `<low> to <high> <unit>`: the low end takes the high end's unit.
                    let unit = named[0] / words[i + 1].parse::<f64>().unwrap();
                    assert_eq!(
                        (low, high),
                        (words[i - 1].parse::<f64>().unwrap() * unit, named[0]),
                        "{}",
                        share.id
                    );
                } else if let Some(&up_to) = named.first() {
                    assert_eq!((low, high), (0.0, up_to), "{}", share.id);
                } else {
                    // "within their shares": the native share of the same benchmark.
                    let native = SHARES
                        .iter()
                        .find(|n| n.on == On::Native && n.benchmark == share.benchmark)
                        .unwrap_or_else(|| {
                            panic!("{}: no native share of {}", share.id, share.benchmark)
                        });
                    assert_eq!(Limit::Within(high), native.limit, "{}", share.id);
                    assert_eq!(low, 0.0, "{}", share.id);
                }
            }
            Limit::NotTimed(why) => {
                assert!(
                    durations(share.words).is_empty(),
                    "{}: a share that names a time is timed",
                    share.id
                );
                assert!(!why.is_empty(), "{}", share.id);
            }
        }
    }
}

#[test]
fn shares_are_read_where_32_16_times_the_machine() {
    let mut ids: Vec<&str> = SHARES.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), SHARES.len(), "every share has its own id");
    for share in SHARES {
        let parts: Vec<&str> = share.benchmark.split('/').collect();
        assert_eq!(
            (parts.len(), parts[0], parts[2]),
            (3, "scenario", "full"),
            "{}",
            share.benchmark
        );
        let expected = match (share.on, share.id) {
            (On::Browsers, _) | (On::Native, "trivial_op") => 1,
            (On::Native, _) => 8,
        };
        assert_eq!(share.threads, expected, "{}", share.id);
        assert!(THREADS.contains(&share.threads), "{}", share.id);
        assert_eq!(
            SHARES
                .iter()
                .filter(|s| s.benchmark == share.benchmark && s.on == share.on)
                .count(),
            1,
            "{} is read once per engine kind",
            share.benchmark
        );
    }
    assert_eq!(BROWSERS, ["chrome", "firefox", "webkit"]);
    assert_eq!(ENGINES[1..], BROWSERS);
}

#[test]
fn a_record_reads_back_as_written() {
    let text = sample();
    let record = Record::parse(&text).unwrap();
    assert_eq!(record.to_string(), text);
    assert_eq!(Record::parse(&record.to_string()).unwrap(), record);
    assert_eq!(record.status, Status::Complete);
    assert_eq!(record.performance, [0, 1, 10, 11, 12, 13, 18, 19]);
    assert_eq!(record.timings.len(), 3);
    assert_eq!(record.timings[1].busy, None);
    assert_eq!(record.timings[2].busy, Some(100));
    assert_eq!(
        record.failures[0].reason,
        "exit status 1: the results changed"
    );
    assert!(record.available("chrome") && !record.available("firefox"));
}

#[test]
fn reading_refuses_whatever_it_does_not_know() {
    let good = sample();
    let cases: &[(&str, &str, usize)] = &[
        (FORMAT, "spare-kit reference timings 2", 1),
        (
            "commit 0123",
            "commit 0123456789ABCDEF0123456789abcdef01234567",
            2,
        ),
        ("night 2026-10-31", "night 2026-13-01", 3),
        ("night 2026-10-31", "night 26-10-31", 3),
        ("finished 900000", "finished 999", 5),
        ("status complete", "status partial", 6),
        ("processors 20", "processors 0", 8),
        ("performance 0 1 10", "performance 1 0 10", 9),
        (
            "performance 0 1 10 11 12 13 18 19",
            "performance 0 1 10 11 12 13 18 20",
            9,
        ),
        ("round 150", "round 0", 12),
        ("engine chrome 156.0, isolated", "engine safari 18", 14),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 0 predicate/orient2d/typical 6.402 368550 6.402 7",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 65 predicate/orient2d/typical 6.402 368550 6.402 7",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 1 predicate/orient2d/typical NaN 368550 6.402 7",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 1 predicate/orient2d/typical -1 368550 6.402 7",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 1 predicate/orient2d/typical 6.402 368550 6.401 7",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 101",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402",
            15,
        ),
        (
            "time native 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            "time firefox 1 predicate/orient2d/typical 6.402 368550 6.402 7",
            15,
        ),
        (
            "time native 8 predicate/orient2d/typical 7.76",
            "time native 1 predicate/orient2d/typical 7.76",
            16,
        ),
        (
            "fail chrome 8 predicate/orient2d/typical exit status 1: the results changed",
            "fail chrome 8 predicate/orient2d/typical",
            18,
        ),
        (
            "fail chrome 8 predicate/orient2d/typical exit status 1: the results changed",
            "skip chrome 8 predicate/orient2d/typical",
            18,
        ),
    ];
    for &(from, to, line) in cases {
        assert!(good.contains(from), "the sample has no `{from}`");
        let bad = good.replacen(from, to, 1);
        let refused = Record::parse(&bad).expect_err(&format!("`{to}` was accepted"));
        assert_eq!(refused.line, line, "`{to}`: {refused}");
    }
    // Lines out of order, a missing header and missing engines.
    let swapped = good.replacen(
        &format!("commit {COMMIT}\nnight 2026-10-31\n"),
        &format!("night 2026-10-31\ncommit {COMMIT}\n"),
        1,
    );
    assert_eq!(Record::parse(&swapped).unwrap_err().line, 2);
    assert_eq!(Record::parse("").unwrap_err().line, 0);
    assert_eq!(
        Record::parse(&format!("{FORMAT}\ncommit {COMMIT}\n"))
            .unwrap_err()
            .line,
        0
    );
    let no_engines: String = good.lines().take(12).map(|l| format!("{l}\n")).collect();
    assert!(Record::parse(&no_engines).is_err());
    let backwards = header("complete", &[("chrome", "1"), ("native", "2")]);
    assert_eq!(Record::parse(&backwards).unwrap_err().line, 14);
}

/// Three busy samples, one a second.
const BUSY: &str = "2000 10\n3000 30\n4000 51\n";

#[test]
fn busy_samples_cover_the_second_before_them() {
    let samples = parse_busy(BUSY).unwrap();
    assert_eq!(samples, [(2000, 10), (3000, 30), (4000, 51)]);
    // A job from 1500 to 1600 lies in the second the 2000 sample covers.
    assert_eq!(busy_between(&samples, 1500, 1600), Some(10));
    // A sample taken at the job's start covers the second before it: not the job.
    assert_eq!(busy_between(&samples, 2000, 2100), Some(30));
    // A sample overlaps a job when it was taken after the job started and its second began
    // before the job ended: the 4000 sample (covering 3000 to 4000) overlaps a job that ends at
    // 3001, but not one that ends at 3000.
    assert_eq!(busy_between(&samples, 1500, 3001), Some(30));
    assert_eq!(busy_between(&samples, 1500, 3000), Some(20));
    assert_eq!(busy_between(&samples, 1500, 4000 - SAMPLE_MS), Some(20));
    assert_eq!(busy_between(&samples, 1500, 3000 - SAMPLE_MS), Some(10));
    // Means round half up: (30 + 51) / 2 = 40.5.
    assert_eq!(busy_between(&samples, 2500, 3500), Some(41));
    assert_eq!(busy_between(&samples, 5000, 6000), None);
    assert!(parse_busy("2000 101\n").is_err());
    assert!(parse_busy("2000\n").is_err());
    assert!(parse_busy("x 1\n").is_err());
    assert_eq!(parse_busy("\n").unwrap(), []);
}

/// The names of a test night.
fn names() -> Vec<String> {
    vec!["g/a/typical".to_string(), "g/b/typical".to_string()]
}

/// A job line that finished.
fn done(engine: &str, threads: u32, start: u64, name: &str, ns: f64) -> String {
    format!(
        "job {engine} {threads} {start} {} 0 {name} {ns:.3} 1000 {threads} {ns:.3}\n",
        start + 900
    )
}

/// Every job of a whole two-benchmark night on native and chrome.
fn whole_night() -> String {
    let mut jobs = String::new();
    let mut at = 1500;
    for engine in ["native", "chrome"] {
        for threads in THREADS {
            for name in names() {
                jobs.push_str(&done(engine, threads, at, &name, 5.0));
                at += 1000;
            }
        }
    }
    jobs
}

#[test]
fn assembling_a_night_leaves_nothing_out() {
    let head = header("complete", &[("native", "n"), ("chrome", "c")]);
    let record = assemble(&head, BUSY, &whole_night(), &names()).unwrap();
    assert_eq!(record.timings.len(), 8);
    assert!(record.failures.is_empty());
    // Each job of 900 ms starting on the half second overlaps the seconds of two samples: 10 and
    // 30, 30 and 51 (40.5, rounded up), 51 and none; then none at all.
    assert_eq!(record.timings[0].busy, Some(20));
    assert_eq!(record.timings[1].busy, Some(41));
    assert_eq!(record.timings[2].busy, Some(51));
    assert_eq!(record.timings[3].busy, None);
    assert_eq!(Record::parse(&record.to_string()).unwrap(), record);

    // A complete night may not lose a job.
    let short: String = whole_night()
        .lines()
        .skip(1)
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(assemble(&head, BUSY, &short, &names()).is_err());
    // A night stopped at its deadline lists what it never reached.
    let stopped = assemble(
        &header("deadline", &[("native", "n"), ("chrome", "c")]),
        BUSY,
        &short,
        &names(),
    )
    .unwrap();
    assert_eq!(stopped.failures.len(), 1);
    assert_eq!(
        stopped.failures[0].reason,
        "not reached before the deadline"
    );
    assert_eq!(
        (
            stopped.failures[0].engine.as_str(),
            stopped.failures[0].threads
        ),
        ("native", 1)
    );
    // An engine that could not run fails every job, and is not available.
    let native_only: String = whole_night()
        .lines()
        .filter(|l| l.starts_with("job native"))
        .map(|l| format!("{l}\n"))
        .collect();
    let head_unavailable = header(
        "complete",
        &[("native", "n"), ("chrome", "unavailable: no Chrome")],
    );
    let partial = assemble(&head_unavailable, BUSY, &native_only, &names()).unwrap();
    assert_eq!(partial.failures.len(), 4);
    assert!(
        partial
            .failures
            .iter()
            .all(|x| x.engine == "chrome" && x.reason == "engine unavailable")
    );
    assert!(!partial.available("chrome"));
}

#[test]
fn assembling_refuses_a_job_it_cannot_place() {
    let head = header("deadline", &[("native", "n")]);
    let refuse = |jobs: &str| assemble(&head, BUSY, jobs, &names()).unwrap_err();
    assert_eq!(
        refuse(&done("native", 1, 1500, "g/a/typical", 5.0).repeat(2)).line,
        2
    );
    assert_eq!(refuse(&done("chrome", 1, 1500, "g/a/typical", 5.0)).line, 1);
    assert_eq!(refuse(&done("native", 2, 1500, "g/a/typical", 5.0)).line, 1);
    assert_eq!(refuse(&done("native", 1, 1500, "g/z/typical", 5.0)).line, 1);
    assert_eq!(
        refuse("job native 1 1500 1400 0 g/a/typical 5 1 1 5\n").line,
        1
    );
    assert_eq!(
        refuse("job native 1 1500 1600 0 g/a/typical 5 1 8 5\n").line,
        1
    );
    assert_eq!(refuse("job native 1 1500 1600 0 g/a/typical 5 1\n").line, 1);
    assert_eq!(
        refuse("work native 1 1500 1600 0 g/a/typical 5 1 1 5\n").line,
        1
    );
    assert!(
        assemble(&sample(), BUSY, "", &names()).is_err(),
        "a header with timings is refused"
    );
    // A job that failed keeps its exit status and its message.
    let failed = assemble(&head, BUSY, "job native 1 1500 1600 1 g/a/typical the digests differ\njob native 8 1500 1600 2 g/b/typical\n", &names()).unwrap();
    assert_eq!(
        failed.failures[0].reason,
        "exit status 1: the digests differ"
    );
    assert_eq!(failed.failures[1].reason, "exit status 2");
}

/// A night with one quiet timing, one disturbed, one not sampled, and one of each kind of share.
fn night(night: &str, ns: f64, chrome: &str) -> Record {
    let text = header("complete", &[("native", "n"), ("chrome", chrome)])
        .replace("night 2026-10-31", &format!("night {night}"))
        + &format!(
            "time native 1 g/a/typical {ns} 1000 {ns} 5\n\
             time native 1 g/b/typical {ns} 1000 {ns} 90\n\
             time native 1 g/c/typical {ns} 1000 {ns} -\n\
             time native 8 g/a/typical {ns} 1000 {ns} 45\n\
             time native 8 scenario/door_move/full {} 1 {} 40\n\
             time native 8 scenario/level_change/full 700000000 1 700000000 40\n\
             time chrome 1 scenario/door_move_small/full 3000000 1 3000000 5\n\
             time chrome 1 scenario/level_change/full 7000000000 1 7000000000 5\n\
             fail chrome 1 scenario/open/full exit status 1: the results changed\n",
            ns * 1e6,
            ns * 1e6,
        );
    Record::parse(&text).unwrap()
}

#[test]
fn the_report_compares_quiet_timings_and_judges_the_shares() {
    assert!(report(&[]).contains("No night has been recorded yet."));
    let first = night("2026-10-30", 3.0, "156.0");
    let only = report(std::slice::from_ref(&first));
    assert!(only.contains("This is the first night recorded."));
    // 3 ms door move: within its 4 ms share; the 700 ms level change is over its 600 ms.
    assert!(only.contains("| door_move | Move a door in under 16 ms | 4.00 ms | native | 8 | `scenario/door_move/full` | 3.00 ms | within |"), "{only}");
    assert!(only.contains("| level_change | Change a level height touching 2,000 walls in under 1 s | 600 ms | native | 8 | `scenario/level_change/full` | 700 ms | **over**"), "{only}");
    assert!(only.contains("| browser_door_move | In a browser | estimate: up to 4.00 ms | chrome | 1 | `scenario/door_move_small/full` | 3.00 ms | inside the estimate |"), "{only}");
    assert!(only.contains("| browser_level_change | In a browser | estimate: 1.00 s to 6.00 s | chrome | 1 | `scenario/level_change/full` | 7.00 s | outside the estimate |"), "{only}");
    assert!(only.contains("| browser_open | In a browser | estimate: up to 1.00 s | chrome | 1 | `scenario/open/full` | - | failed: exit status 1: the results changed |"), "{only}");
    assert!(
        only.contains("| open | ")
            && only.contains("not measured yet: no such benchmark (piece G451 adds it)")
    );
    assert!(
        only.contains("| firefox | 1 | `scenario/door_move_small/full` | - | firefox not run |"),
        "{only}"
    );
    assert!(only.contains("| navigate | ") && only.contains("| not timed |"));

    // The next night: every time 10% slower. Only the quiet ones are listed (g/a at 1 and 8
    // threads, and the 40%-busy scenarios at 8 threads); g/b was disturbed, g/c not sampled.
    let second = night("2026-10-31", 3.3, "157.0");
    let both = report(&[first.clone(), second.clone()]);
    assert!(both.contains("| chrome | 157.0 (was 156.0) |"), "{both}");
    assert!(
        both.contains("| native | 1 | `g/a/typical` | 3.00 ns | 3.30 ns | +10.0% |"),
        "{both}"
    );
    assert!(
        both.contains("| native | 8 | `g/a/typical` | 3.00 ns | 3.30 ns | +10.0% |"),
        "{both}"
    );
    assert!(
        both.contains("| native | 8 | `scenario/door_move/full` | 3.00 ms | 3.30 ms | +10.0% |"),
        "{both}"
    );
    assert!(
        !both.contains("| native | 1 | `g/b/typical` | 3.00 ns"),
        "a disturbed timing is not compared"
    );
    assert!(
        !both.contains("| native | 1 | `g/c/typical` | 3.00 ns"),
        "an unsampled timing is not compared"
    );
    assert!(
        both.contains("| native | 1 | `g/b/typical` | 3.30 ns | 90% |"),
        "{both}"
    );
    assert!(both.contains(": 3. Wall-clock time is noisy"), "{both}");
    assert!(
        both.contains("| door_move | ") && both.contains("| 3.00 ms, 3.30 ms |"),
        "{both}"
    );
    // A move of 5% or less is not listed.
    let third = night("2026-11-01", 3.3 * 1.04, "157.0");
    assert!(report(&[second.clone(), third]).contains(": 0. Wall-clock time is noisy"));
    // The same history gives the same report.
    assert_eq!(report(&[first.clone(), second.clone()]), both);
}

#[test]
fn disturbed_means_busier_than_the_job_explains() {
    let record = night("2026-10-30", 3.0, "156.0");
    let at = |busy: u32, threads: u32| {
        let mut t = record.timings[0].clone();
        t.busy = Some(busy);
        t.threads = threads;
        t
    };
    // One thread of twenty logical processors explains 5 points; eight explain 40.
    assert!(!record.disturbed(&at(5 + DISTURBED_POINTS, 1)));
    assert!(record.disturbed(&at(6 + DISTURBED_POINTS, 1)));
    assert!(!record.disturbed(&at(40 + DISTURBED_POINTS, 8)));
    assert!(record.disturbed(&at(41 + DISTURBED_POINTS, 8)));
    let mut unsampled = at(0, 1);
    unsampled.busy = None;
    assert!(!record.disturbed(&unsampled) && !record.quiet(&unsampled));
    assert!(record.quiet(&at(0, 1)));
}

#[test]
fn the_report_writes_times_and_dates_plainly() {
    assert_eq!(duration(5.943), "5.94 ns");
    assert_eq!(duration(12.6), "12.6 ns");
    assert_eq!(duration(999.0), "999 ns");
    assert_eq!(duration(1234.0), "1.23 µs");
    assert_eq!(duration(4e6), "4.00 ms");
    assert_eq!(duration(2.5e8), "250 ms");
    assert_eq!(duration(1.5e9), "1.50 s");
    assert_eq!(duration(4e10), "40.0 s");
    assert_eq!(upper_median(vec![3.0, 1.0, 2.0, 4.0]), Some(3.0));
    assert_eq!(upper_median(vec![2.0, 1.0, 3.0]), Some(2.0));
    assert_eq!(upper_median(Vec::new()), None);
    assert_eq!(utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(utc(951_782_400_000), "2000-02-29 00:00 UTC");
    assert_eq!(utc(4_107_542_399_000), "2100-02-28 23:59 UTC");
    assert_eq!(utc(1_793_387_591_163), "2026-10-30 19:13 UTC");
}

/// Runs a program of this package and returns its exit status and output.
#[cfg(any(unix, windows))]
fn run(program: &str, args: &[&str]) -> (i32, String, String) {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[cfg(any(unix, windows))]
#[test]
fn the_guest_times_at_a_thread_count() {
    let guest = env!("CARGO_BIN_EXE_geom-bench-guest");
    for threads in ["1", "2"] {
        let (status, out, err) = run(guest, &["time", "predicate/orient2d/typical", "5", threads]);
        assert_eq!(status, 0, "{err}");
        let fields: Vec<&str> = out.trim_end().split(' ').collect();
        assert_eq!(fields.len(), 5, "{out}");
        assert_eq!(
            (fields[0], fields[3]),
            ("predicate/orient2d/typical", threads)
        );
        let (median, passes, slowest): (f64, u64, f64) = (
            fields[1].parse().unwrap(),
            fields[2].parse().unwrap(),
            fields[4].parse().unwrap(),
        );
        assert!(median > 0.0 && passes > 0 && slowest >= median, "{out}");
        if threads == "1" {
            assert_eq!(fields[1], fields[4], "one thread is its own slowest");
        }
    }
    // The variant workflow's form is unchanged: three fields.
    let (status, out, _) = run(guest, &["time", "predicate/orient2d/typical", "5"]);
    assert_eq!((status, out.trim_end().split(' ').count()), (0, 3), "{out}");
    let max = MAX_THREADS.to_string();
    let over = (MAX_THREADS + 1).to_string();
    for args in [
        ["time", "predicate/orient2d/typical", "5", "0"],
        ["time", "predicate/orient2d/typical", "5", over.as_str()],
        ["time", "predicate/orient2d/typical", "5", "x"],
        ["time", "predicate/orient2d/typical", "0", "1"],
        ["time", "no/such/benchmark", "5", "1"],
        ["time", "no/such/benchmark", "5", max.as_str()],
    ] {
        let (status, _, err) = run(guest, &args);
        assert_eq!(status, 2, "{args:?}: {err}");
        assert!(
            err.contains("usage: geom-bench-guest time <name> <milliseconds> <threads>"),
            "{err}"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn the_tool_assembles_checks_and_reports() {
    let tool = env!("CARGO_BIN_EXE_geom-bench-timings");
    let dir = std::env::temp_dir().join(format!("geom-bench-g107-{}", std::process::id()));
    let history = dir.join("history");
    std::fs::create_dir_all(&history).unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path.to_string_lossy().into_owned()
    };
    let head = write("header.txt", &header("complete", &[("native", "n")]));
    let busy = write("busy.txt", BUSY);
    let jobs = write(
        "jobs.txt",
        &(done("native", 1, 1500, "predicate/orient2d/typical", 6.0)
            + &done("native", 8, 2500, "predicate/orient2d/typical", 7.0)),
    );
    let listed = write("names.txt", "predicate/orient2d/typical\n");
    let (status, record, err) = run(tool, &["assemble", &head, &busy, &jobs, &listed]);
    assert_eq!(status, 0, "{err}");
    assert_eq!(Record::parse(&record).unwrap().timings.len(), 2);
    let unknown = write("unknown.txt", "no/such/benchmark\n");
    assert_eq!(run(tool, &["assemble", &head, &busy, &jobs, &unknown]).0, 1);
    let twice = write(
        "twice.txt",
        "predicate/orient2d/typical\npredicate/orient2d/typical\n",
    );
    assert_eq!(run(tool, &["assemble", &head, &busy, &jobs, &twice]).0, 1);
    let saved = history.join("2026-10-31-1.txt");
    std::fs::write(&saved, &record).unwrap();
    let (status, out, _) = run(tool, &["check", &saved.to_string_lossy()]);
    assert_eq!(
        (status, out.contains("2 timings, 0 failures")),
        (0, true),
        "{out}"
    );
    let (status, out, _) = run(tool, &["report", &history.to_string_lossy()]);
    assert_eq!(status, 0);
    assert!(
        out.starts_with("# Reference timings: the night of 2026-10-31"),
        "{out}"
    );
    let broken = write("broken.txt", "not a record\n");
    assert_eq!(run(tool, &["check", &broken]).0, 1);
    assert_eq!(
        run(tool, &["report", &dir.join("none").to_string_lossy()]).0,
        1
    );
    assert_eq!(run(tool, &[]).0, 2);
    assert_eq!(run(tool, &["assemble", &head]).0, 2);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The value of `NAME=value` in the runner.
fn runner_setting(name: &str) -> &'static str {
    RUNNER
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("nightly.sh sets no {name}"))
}

#[test]
fn the_runner_writes_what_the_tool_reads() {
    // The engines and thread counts it runs are the record's.
    assert_eq!(runner_setting("ENGINES_ALL"), ENGINES.join(","));
    let threads = format!(
        "for threads in {}; do",
        THREADS.map(|t| t.to_string()).join(" ")
    );
    assert!(
        RUNNER.matches(threads.as_str()).count() >= 2,
        "native and the browsers' fill use {threads}"
    );
    let page = format!(
        "const THREADS = [{}];",
        THREADS.map(|t| t.to_string()).join(", ")
    );
    assert!(PAGE.contains(&page), "page.mjs times at {THREADS:?}");
    // Every worker runs one guest at one thread; the native jobs pass the night's thread count.
    assert!(PAGE.contains(r#"["geom-bench-guest", "time", name, String(round), "1"]"#));
    assert!(RUNNER.contains(r".\\guest.exe time $name $round $threads"));
    // Job lines in the order `assemble` reads them.
    assert!(RUNNER.contains(r#"echo "job native $threads $start $end $code $text""#));
    assert!(PAGE.contains("`${count} ${start} ${end} ${status} ${text}`"));
    assert!(DRIVER.contains("`job ${engine} ${line}\\n`"));
    assert!(RUNNER.contains(r#"echo "job $e $threads $at $at 5 $name $why""#));
    // The header in the record's order.
    let block = &RUNNER[RUNNER.find("# --- The record").unwrap()
        ..RUNNER.find(r#"} > "$tonight/header.txt""#).unwrap()];
    let keys: Vec<&str> = block
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            l.strip_prefix("echo \"")
                .or_else(|| l.strip_prefix("sed -n 's/^"))
                .map(|rest| rest.split([' ', '"']).next().unwrap())
        })
        .collect();
    assert_eq!(
        keys,
        [
            "spare-kit",
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
            "engine"
        ]
    );
    assert!(block.contains(&format!("echo \"{FORMAT}\"")));
    // A job starts only if it ends by the deadline, with the same margin in the browsers.
    assert!(RUNNER.contains("8 * round + 5000"));
    assert!(PAGE.contains("Date.now() + 8 * round > deadline"));
    // Recorded nights are whole nights.
    assert!(RUNNER.contains("--push records whole nights only"));
}

#[test]
fn the_runner_pins_what_it_runs() {
    // The browser packages are the ones the variant workflow pins.
    let playwright = runner_setting("PLAYWRIGHT");
    let shim = runner_setting("WASI_SHIM");
    assert!(
        VARIANTS.contains(&format!("playwright@{playwright}")),
        "kernel-variants.yml pins another Playwright"
    );
    assert!(
        VARIANTS.contains(&format!("@bjorn3/browser_wasi_shim@{shim}")),
        "kernel-variants.yml pins another shim"
    );
    assert!(RUNNER.contains("playwright@$PLAYWRIGHT @bjorn3/browser_wasi_shim@$WASI_SHIM"));
    // llvm-mingw by release and SHA-256.
    let sha = runner_setting("LLVM_MINGW_SHA256");
    assert!(
        sha.len() == 64
            && sha
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "{sha}"
    );
    let release = runner_setting("LLVM_MINGW");
    let tag = release.split('-').nth(2).unwrap();
    assert_eq!(
        runner_setting("LLVM_MINGW_URL"),
        format!(
            "https://github.com/mstorsjo/llvm-mingw/releases/download/{tag}/$LLVM_MINGW.tar.xz"
        )
    );
    assert!(RUNNER.contains("sha256sum --check --strict"));
    // The browsers are pinned to the performance cores, and the worker uses the pinned shim.
    assert!(RUNNER.contains(r".\\pin.cmd $mask8 node browsers.mjs"));
    assert!(WORKER.contains("./node_modules/@bjorn3/browser_wasi_shim/dist/index.js"));
    // The page is isolated, so each browser's timer is as fine as it allows.
    for header in ["Cross-Origin-Opener-Policy", "Cross-Origin-Embedder-Policy"] {
        assert!(
            DRIVER.contains(header),
            "browsers.mjs does not send {header}"
        );
    }
}
