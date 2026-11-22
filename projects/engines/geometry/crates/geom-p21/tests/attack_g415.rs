//! Attacks on what piece G415 claims: the split of the reads never matters, memory does not grow
//! with the file, the work is metered and stops, mangled texts are refused inside their input, and
//! writing is a fixed point.
//!
//! Random inputs come from a fixed seed, printed as `seed: <hex>`; `SPAREKIT_SEED=<hex>` replaces
//! it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::format_push_string,
    clippy::range_plus_one,
    clippy::naive_bytecount,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    reason = "tests: a failure stops the test"
)]

mod common;

use common::{Drip, Synthetic, events, events_of, exchange, meter};
use geom_format::source::SliceSource;
use geom_p21::{ErrorKind, Event, Exchange, Options, Reader, Totals};
use geom_run::{Budget, Cancel, Stop, WorkMeter};

/// `SplitMix64` (Vigna), enough for test inputs.
struct Rng(u64);

impl Rng {
    fn seeded(label: &str) -> Self {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0x4715_2026_1111_0415);
        println!("{label} seed: {seed:x}");
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// One parameter in Part 21's text, nested at most `depth` deep.
fn param(rng: &mut Rng, depth: usize, out: &mut String) {
    const STRINGS: [&str; 10] = [
        "",
        "x",
        "it''s",
        "a\\\\b",
        "\\X2\\00E9\\X0\\",
        "\\X4\\0001F600\\X0\\",
        "\\S\\a",
        "\\X\\E9",
        "C:\\dir",
        "\u{e9}t\u{e9}",
    ];
    const NUMBERS: [&str; 10] = [
        "0",
        "-7",
        "+12",
        "0.",
        "1.5",
        "-2.5E-3",
        "1.E21",
        "1E5",
        ".5",
        "9223372036854775807",
    ];
    let kinds = if depth == 0 { 8 } else { 10 };
    match rng.below(kinds) {
        0 => out.push('$'),
        1 => out.push('*'),
        2 | 3 => out.push_str(NUMBERS[rng.below(NUMBERS.len())]),
        4 => {
            out.push('\'');
            out.push_str(STRINGS[rng.below(STRINGS.len())]);
            out.push('\'');
        }
        5 => out.push_str([".T.", ".f.", ".UNSPECIFIED.", "._A1."][rng.below(4)]),
        6 => out.push_str(["\"0\"", "\"3FA\"", "\"12\""][rng.below(3)]),
        7 => out.push_str(&format!("#{}", rng.below(50))),
        kind => {
            if kind == 9 {
                out.push_str(["LENGTH_MEASURE", "!u", "T"][rng.below(3)]);
            }
            out.push('(');
            for i in 0..rng.below(4) {
                if i > 0 {
                    out.push(',');
                }
                param(rng, depth - 1, out);
            }
            out.push(')');
        }
    }
}

/// `NAME(params)`.
fn record(rng: &mut Rng, out: &mut String) {
    out.push_str(["A", "B_SPLINE", "cartesian_point", "!USER", "Z9"][rng.below(5)]);
    out.push('(');
    for i in 0..rng.below(5) {
        if i > 0 {
            out.push_str([",", " ,\n", ",/* c */"][rng.below(3)]);
        }
        param(rng, 3, out);
    }
    out.push(')');
}

/// A Part 21 file, or, half the time, one damaged in a place or two.
fn mangled(rng: &mut Rng) -> Vec<u8> {
    const PIECES: [&str; 30] = [
        "ISO-10303-21;",
        "HEADER;",
        "ENDSEC;",
        "DATA;",
        "END-ISO-10303-21;",
        "#1=",
        "#",
        "A(",
        "(",
        ")",
        ");",
        ",",
        ";",
        "$",
        "'",
        "''",
        "\\X2\\",
        "00E9",
        "\\X0\\",
        "\\",
        "1",
        "-",
        ".5",
        ".",
        "\"",
        "/*",
        "*/",
        "\n",
        "E",
        "\u{e9}",
    ];
    let mut text = String::from("ISO-10303-21;\nHEADER;");
    if rng.below(2) == 0 {
        text.push_str("FILE_DESCRIPTION(('d'),'2;1');FILE_NAME('n','t',('a'),(''),'p','s','');\nFILE_SCHEMA(('S'));");
    }
    if rng.below(4) == 0 {
        record(rng, &mut text);
        text.push(';');
    }
    text.push_str("ENDSEC;\n");
    if rng.below(6) == 0 {
        text.push_str("ANCHOR;<a>=#1;'ENDSEC;'ENDSEC;");
    }
    let mut id = 0;
    for _ in 0..rng.below(3) {
        text.push_str(["DATA;\n", "DATA('d',('S'));"][rng.below(2)]);
        for _ in 0..rng.below(5) {
            // Mostly rising ids, sometimes a repeat.
            id += rng.below(3);
            text.push_str(&format!("#{id}="));
            if rng.below(4) == 0 {
                text.push('(');
                for _ in 0..1 + rng.below(3) {
                    record(rng, &mut text);
                }
                text.push(')');
            } else {
                record(rng, &mut text);
            }
            text.push_str(";\n");
        }
        text.push_str("ENDSEC;\n");
    }
    text.push_str("END-ISO-10303-21;\n");
    let mut bytes = text.into_bytes();
    if rng.below(2) == 0 {
        for _ in 0..1 + rng.below(2) {
            let at = rng.below(bytes.len());
            match rng.below(3) {
                0 => bytes[at] = rng.next() as u8,
                1 => {
                    let end = (at + 1 + rng.below(6)).min(bytes.len());
                    bytes.drain(at..end);
                }
                _ => {
                    let piece = PIECES[rng.below(PIECES.len())].as_bytes();
                    bytes.splice(at..at, piece.iter().copied());
                }
            }
            if bytes.is_empty() {
                break;
            }
        }
    }
    bytes
}

#[test]
fn the_split_of_the_reads_never_matters() {
    let mut rng = Rng::seeded("split");
    let mut accepted = 0;
    for round in 0..3000 {
        let _ = round;
        let text = mangled(&mut rng);
        let whole = events(&text);
        accepted += usize::from(whole.1.is_none());
        if let Some(e) = whole.1 {
            assert!(e.offset <= text.len() as u64);
        }
        let step = 1 + rng.below(7);
        let buffer_bytes = rng.below(9);
        let options = Options {
            buffer_bytes,
            ..Options::DEFAULT
        };
        let dripped = events_of(Drip { rest: &text, step }, options);
        assert_eq!(
            dripped,
            whole,
            "{:?} in steps of {step}, buffer {buffer_bytes}",
            String::from_utf8_lossy(&text)
        );
    }
    assert!(accepted > 50, "only {accepted} texts were files");
}

#[test]
fn accepted_texts_write_to_a_fixed_point() {
    let mut rng = Rng::seeded("fixed point");
    let mut accepted = 0;
    for _ in 0..6000 {
        let text = mangled(&mut rng);
        let lossy = Options {
            lossy_text: true,
            ..Options::DEFAULT
        };
        let read = |text: &[u8], options| {
            Exchange::read(
                SliceSource::new(text),
                options,
                Totals::DEFAULT,
                &mut meter(),
            )
        };
        let strict = read(&text, Options::DEFAULT);
        let Ok(first) = read(&text, lossy) else {
            assert!(strict.is_err(), "strict text reads what lossy text refuses");
            continue;
        };
        if let Ok(strict) = &strict {
            assert_eq!(strict, &first);
            assert_eq!(first.replaced, 0);
        } else {
            assert!(first.replaced > 0);
        }
        accepted += 1;
        let written = first.write(Vec::new()).unwrap();
        // The canonical form holds only ASCII, and reads back strictly.
        assert!(written.is_ascii());
        let second = exchange(&written).unwrap();
        assert!(second.duplicates.is_empty());
        assert_eq!(second.entities.len(), first.entities.len());
        assert_eq!(second.header, first.header);
        assert_eq!(second.write(Vec::new()).unwrap(), written);
    }
    assert!(accepted > 100, "only {accepted} texts were files");
}

#[test]
fn memory_does_not_grow_with_the_file() {
    // About 25 MB here; `a_file_beyond_four_gibibytes_reads_in_the_same_memory` goes past 2^32.
    let held = read_synthetic(300_000);
    // The buffer, 64 KiB, and one instance's tokens.
    assert!((64 << 10..80 << 10).contains(&held), "{held} bytes held");
}

#[test]
#[ignore = "reads 4.4 GiB: minutes in a release build; run by hand and in the piece's evidence"]
fn a_file_beyond_four_gibibytes_reads_in_the_same_memory() {
    let held = read_synthetic(52_000_000);
    assert!(held < 80 << 10, "{held} bytes held");
}

/// Reads a synthetic file of `instances` instances and gives the most the reader ever held.
fn read_synthetic(instances: u64) -> usize {
    let mut source = Synthetic::new(instances);
    let mut meter = meter();
    let mut reader = Reader::new(&mut source, Options::DEFAULT, &mut meter);
    let (mut seen, mut held, mut next) = (0_u64, 0, 1);
    while let Some(event) = reader.next_event().unwrap() {
        if let Event::Instance(id) = event {
            assert_eq!(id, next);
            next += 1;
            seen += 1;
        }
        held = held.max(reader.held_bytes());
    }
    let (offset, line) = (reader.offset(), reader.line());
    assert_eq!(seen, instances);
    // The line feed after the end is handed over by the source and never read.
    assert_eq!(offset + 1, source.bytes);
    assert_eq!(line, instances + 9);
    if instances > 50_000_000 {
        assert!(offset > 1 << 32);
    }
    // Every byte was charged, one unit each.
    assert_eq!(
        meter.used().get(geom_run::BudgetKind::Work),
        u128::from(offset + 1)
    );
    held
}

impl geom_format::source::ByteSource for &mut Synthetic {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, geom_format::source::SourceError> {
        (**self).read(buf)
    }
}

struct Always;

impl Cancel for Always {
    fn requested(&self) -> bool {
        true
    }
}

#[test]
fn the_meter_stops_the_read() {
    let text: Vec<u8> = {
        let mut source = Synthetic::new(2000);
        let mut text = vec![0; 1 << 20];
        let n = geom_format::source::ByteSource::read(&mut source, &mut text).unwrap();
        text.truncate(n);
        text
    };
    let options = Options {
        buffer_bytes: 256,
        ..Options::DEFAULT
    };
    let read = |meter: &mut WorkMeter<'_>| {
        Exchange::read(SliceSource::new(&text), options, Totals::DEFAULT, meter)
    };
    let mut exact = WorkMeter::new(Budget::new(text.len() as u64, u64::MAX));
    assert_eq!(read(&mut exact).unwrap().entities.len(), 2000);
    let mut short = WorkMeter::new(Budget::new(text.len() as u64 - 1, u64::MAX));
    let e = read(&mut short).unwrap_err();
    let ErrorKind::Stopped(stopped) = e.kind else {
        panic!("{e}");
    };
    assert!(matches!(stopped.reason(), Stop::OverBudget(_)));
    // A cancellation is read within `POLL_EVERY` refills.
    let cancel = Always;
    let mut cancelled = WorkMeter::new(Budget::UNLIMITED).with_cancel(&cancel);
    let e = read(&mut cancelled).unwrap_err();
    let ErrorKind::Stopped(stopped) = e.kind else {
        panic!("{e}");
    };
    assert_eq!(stopped.reason(), Stop::Cancelled);
    assert!(e.offset <= 256 * geom_run::POLL_EVERY as u64, "{e}");
}
