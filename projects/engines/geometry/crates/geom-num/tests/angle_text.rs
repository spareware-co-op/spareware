//! Angles as text (`geom_num::angle_text`, piece G359, 17.2): every notation read and written,
//! exhaustively over whole ranges of each instrument grid, against integer arithmetic done here.
//!
//! - **Exhaustive round trips.** Every whole-second reading of a full turn (in debug builds every
//!   seventeenth); every 0.01″ reading of one degree; every unit of three whole seconds; every
//!   0.00001 gon of one gon and every 0.001 gon of a full turn; every 0.000001° of one degree (in
//!   debug builds every seventh); every whole second of one degree and every 0.01″ of one minute
//!   in the compact form; every whole minute of a turn as a quadrant bearing. Each text reads back
//!   as the angle the integers here say, exactly, and is the text [`write`] gives for it.
//! - **The shortest exact text** of random angles reads back exactly, and exists in decimal
//!   degrees and gon just when the angle is a whole number of their last unit.
//! - **Rounding** is to nearest, ties to even, and says it is not exact.
//! - **Refusals** name the byte where the text goes wrong.
//!
//! Seeds: the randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![allow(
    clippy::many_single_char_names,
    reason = "sines and cosines are s and c, degrees, minutes and seconds d, m and s"
)]

use std::fmt::Write as _;

use geom_num::angle::{
    AngleUnits, Bearing, FULL_TURN, HALF_TURN, PER_DEGREE, PER_GON, QUARTER_TURN,
};
use geom_num::angle_text::{
    AngleTextError, MAX_WRITE_PLACES, Notation, parse, parse_quadrant, shortest, write,
    write_quadrant,
};

/// The default seed of the randomised test here.
const DEFAULT_SEED: u64 = 0x0359_7e47_5eed_0002;

/// `SplitMix64`, as the crate's other tests write it.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// `text` read in `notation`, which must be exact.
fn exact(text: &str, notation: Notation) -> i64 {
    let read = parse(text, notation).unwrap_or_else(|e| panic!("{text:?}: {e}"));
    assert!(read.is_exact(), "{text:?} is not exact");
    read.value().get()
}

/// `angle` written in `notation` to `places`, which must be exact, into `buffer`.
fn written(buffer: &mut String, angle: i64, notation: Notation, places: u8) -> &str {
    buffer.clear();
    let w = write(AngleUnits(angle), notation, places).unwrap();
    assert!(w.is_exact(), "{angle} in {notation:?} to {places}");
    write!(buffer, "{}", w.value()).unwrap();
    buffer
}

/// The units of a reading, computed here: degrees, minutes, seconds and hundredths of a second.
fn dms_units(d: i64, m: i64, s: i64, hundredths: i64) -> i64 {
    d * 9_000_000 + m * 150_000 + s * 2_500 + hundredths * 25
}

#[test]
fn every_whole_second_of_a_turn_reads_and_writes_back() {
    let step = if cfg!(debug_assertions) { 17 } else { 1 };
    let mut text = String::new();
    let mut buffer = String::new();
    for total in (0..360 * 3600).step_by(step) {
        let (d, m, s) = (total / 3600, total / 60 % 60, total % 60);
        text.clear();
        write!(text, "{d}\u{b0}{m:02}'{s:02}\"").unwrap();
        let units = dms_units(d, m, s, 0);
        assert_eq!(exact(&text, Notation::Dms), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Dms, 0), text);
    }
}

#[test]
fn every_hundredth_of_a_second_of_one_degree_reads_and_writes_back() {
    let mut text = String::new();
    let mut buffer = String::new();
    for total in 0..3600 * 100 {
        let (m, s, h) = (total / 6000, total / 100 % 60, total % 100);
        text.clear();
        write!(text, "-123\u{b0}{m:02}'{s:02}.{h:02}\"").unwrap();
        let units = -dms_units(123, m, s, h);
        assert_eq!(exact(&text, Notation::Dms), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Dms, 2), text);
    }
}

#[test]
fn every_unit_of_three_seconds_has_a_shortest_text_that_reads_back() {
    let mut buffer = String::new();
    for start in [0, 359 * 9_000_000 + 59 * 150_000 + 57 * 2_500, 1 << 50] {
        for units in start..start + 3 * 2_500 {
            for notation in [Notation::Dms, Notation::Compact] {
                let w = shortest(AngleUnits(units), notation).unwrap();
                assert!(w.places() <= 4);
                buffer.clear();
                write!(buffer, "{w}").unwrap();
                assert_eq!(exact(&buffer, notation), units, "{buffer}");
                // It is the shortest: one place fewer is not exact.
                if w.places() > 0 {
                    let fewer = write(AngleUnits(units), notation, w.places() - 1).unwrap();
                    assert!(!fewer.is_exact());
                }
            }
        }
    }
}

#[test]
fn every_hundred_thousandth_of_a_gon_reads_and_writes_back() {
    let mut text = String::new();
    let mut buffer = String::new();
    for k in 0..100_000i64 {
        text.clear();
        write!(text, "399.{k:05} gon").unwrap();
        let units = 399 * PER_GON + k * 81;
        assert_eq!(exact(&text, Notation::Gon), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Gon, 5), text);
    }
    for k in 0..400_000i64 {
        let units = k * 8_100;
        text.clear();
        write!(text, "{}.{:03} gon", k / 1000, k % 1000).unwrap();
        assert_eq!(exact(&text, Notation::Gon), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Gon, 3), text);
    }
}

#[test]
fn every_millionth_of_a_degree_of_one_degree_reads_and_writes_back() {
    let step = if cfg!(debug_assertions) { 7 } else { 1 };
    let mut text = String::new();
    let mut buffer = String::new();
    for k in (0..1_000_000i64).step_by(step) {
        text.clear();
        write!(text, "-7.{k:06}\u{b0}").unwrap();
        let units = -(7 * PER_DEGREE + k * 9);
        assert_eq!(exact(&text, Notation::Degrees), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Degrees, 6), text);
    }
}

#[test]
fn the_compact_form_reads_and_writes_every_second_of_a_degree() {
    let mut text = String::new();
    let mut buffer = String::new();
    for total in 0..3600 {
        let (m, s) = (total / 60, total % 60);
        text.clear();
        write!(text, "45.{m:02}{s:02}").unwrap();
        let units = dms_units(45, m, s, 0);
        assert_eq!(exact(&text, Notation::Compact), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Compact, 0), text);
    }
    for total in 0..6000 {
        let (s, h) = (total / 100, total % 100);
        text.clear();
        write!(text, "200.59{s:02}{h:02}").unwrap();
        let units = dms_units(200, 59, s, h);
        assert_eq!(exact(&text, Notation::Compact), units, "{text}");
        assert_eq!(written(&mut buffer, units, Notation::Compact, 2), text);
    }
    // Fewer than four digits are padded: 12.3 is 12°30′, and 12 is 12°.
    assert_eq!(exact("12.3", Notation::Compact), dms_units(12, 30, 0, 0));
    assert_eq!(exact("12.305", Notation::Compact), dms_units(12, 30, 50, 0));
    assert_eq!(exact("12", Notation::Compact), dms_units(12, 0, 0, 0));
    assert_eq!(exact("-0.0001", Notation::Compact), -dms_units(0, 0, 1, 0));
}

#[test]
fn every_whole_minute_of_a_turn_is_a_quadrant_bearing_that_reads_back() {
    let mut buffer = String::new();
    for minute in 0..360 * 60 {
        let b = Bearing::new(AngleUnits(minute * 150_000));
        let q = write_quadrant(b, 0).unwrap();
        assert!(q.is_exact());
        buffer.clear();
        write!(buffer, "{}", q.value()).unwrap();
        let back = parse_quadrant(&buffer).unwrap();
        assert!(back.is_exact());
        assert_eq!(back.value(), b, "{buffer}");
    }
    let text = |b: i64| {
        write_quadrant(Bearing::new(AngleUnits(b)), 0)
            .unwrap()
            .value()
            .to_string()
    };
    assert_eq!(text(0), "N 0\u{b0}00'00\" E");
    assert_eq!(text(QUARTER_TURN), "N 90\u{b0}00'00\" E");
    assert_eq!(text(HALF_TURN), "S 0\u{b0}00'00\" W");
    assert_eq!(text(HALF_TURN + QUARTER_TURN), "S 90\u{b0}00'00\" W");
    assert_eq!(text(FULL_TURN - PER_DEGREE), "N 1\u{b0}00'00\" W");
    assert_eq!(text(QUARTER_TURN + PER_DEGREE), "S 89\u{b0}00'00\" E");
}

#[test]
fn quadrant_bearings_read_the_four_quadrants_and_refuse_the_rest() {
    let at = |t: &str| parse_quadrant(t).unwrap().value().units().get();
    let theta = dms_units(12, 34, 56, 0);
    assert_eq!(at("N 12\u{b0}34'56\" E"), theta);
    assert_eq!(at("s12d34'56\"e"), HALF_TURN - theta);
    assert_eq!(
        at("  S 12\u{b0} 34\u{2032} 56\u{2033} W  "),
        HALF_TURN + theta
    );
    assert_eq!(at("N 12\u{b0}34'56\" W"), FULL_TURN - theta);
    assert_eq!(at("N 0\u{b0} W"), 0);
    let rounded = parse_quadrant("N 0\u{b0}00'00.0002\" E").unwrap();
    assert!(!rounded.is_exact());
    assert_eq!(
        parse_quadrant("N 90\u{b0}00'00.01\" E"),
        Err(AngleTextError::QuadrantAngle)
    );
    assert_eq!(
        parse_quadrant("N -5\u{b0} E"),
        Err(AngleTextError::QuadrantAngle)
    );
    assert_eq!(
        parse_quadrant("E 5\u{b0} N"),
        Err(AngleTextError::Unexpected { at: 0 })
    );
    assert_eq!(
        parse_quadrant("N 5\u{b0} X"),
        Err(AngleTextError::Unexpected { at: 6 })
    );
    assert_eq!(parse_quadrant("N E"), Err(AngleTextError::Empty));
    assert_eq!(
        parse_quadrant("N"),
        Err(AngleTextError::Unexpected { at: 0 })
    );
    assert_eq!(
        parse_quadrant("N 5\u{b0} 1 E"),
        Err(AngleTextError::MissingSymbol { at: 8 })
    );
    assert_eq!(
        parse_quadrant("N 2000000000000\u{b0} E"),
        Err(AngleTextError::QuadrantAngle)
    );
}

#[test]
fn the_symbols_and_spaces_degrees_minutes_and_seconds_take() {
    let theta = dms_units(12, 34, 56, 78);
    for text in [
        "12\u{b0}34'56.78\"",
        "12d34'56.78\"",
        "12D34'56.78\"",
        "12\u{ba}34\u{2032}56.78\u{2033}",
        "12\u{2da}34\u{2019}56.78\u{201d}",
        "12\u{b0}34\u{b4}56.78''",
        " + 12 \u{b0} 34 ' 56.78 \" ",
    ] {
        assert_eq!(exact(text, Notation::Dms), theta, "{text}");
    }
    for text in [
        "-12\u{b0}34'56.78\"",
        "\u{2212}12\u{b0}34'56.78\"",
        "\u{2013} 12\u{b0}34'56.78\"",
    ] {
        assert_eq!(exact(text, Notation::Dms), -theta, "{text}");
    }
    // Any part may be left out, and the last given may have a fraction.
    assert_eq!(exact("34'", Notation::Dms), dms_units(0, 34, 0, 0));
    assert_eq!(exact("56.78\"", Notation::Dms), dms_units(0, 0, 56, 78));
    assert_eq!(
        exact("12\u{b0}56\"", Notation::Dms),
        dms_units(12, 0, 56, 0)
    );
    assert_eq!(exact("12.5\u{b0}", Notation::Dms), dms_units(12, 30, 0, 0));
    assert_eq!(
        exact("12\u{b0}34.5'", Notation::Dms),
        dms_units(12, 34, 30, 0)
    );
    assert_eq!(exact("400\u{b0}", Notation::Dms), 400 * PER_DEGREE);
    assert_eq!(exact("-0\u{b0}", Notation::Dms), 0);
    // Decimal degrees and gon take their symbols or none.
    assert_eq!(
        exact("12.5", Notation::Degrees),
        12 * PER_DEGREE + PER_DEGREE / 2
    );
    assert_eq!(
        exact("12.5 d", Notation::Degrees),
        12 * PER_DEGREE + PER_DEGREE / 2
    );
    assert_eq!(exact("100g", Notation::Gon), QUARTER_TURN);
    assert_eq!(exact("-100", Notation::Gon), -QUARTER_TURN);
}

#[test]
fn refusals_name_the_byte_where_the_text_goes_wrong() {
    let err = |text: &str, notation| parse(text, notation).unwrap_err();
    assert_eq!(err("", Notation::Dms), AngleTextError::Empty);
    assert_eq!(err("  -  ", Notation::Dms), AngleTextError::Empty);
    assert_eq!(err("x", Notation::Degrees), AngleTextError::Empty);
    assert_eq!(
        err("12", Notation::Dms),
        AngleTextError::MissingSymbol { at: 2 }
    );
    assert_eq!(
        err("12\u{b0}x", Notation::Dms),
        AngleTextError::Unexpected { at: 4 }
    );
    assert_eq!(
        err("12'34\u{b0}", Notation::Dms),
        AngleTextError::OutOfOrder { at: 5 }
    );
    assert_eq!(
        err("12\u{b0}3\u{b0}", Notation::Dms),
        AngleTextError::OutOfOrder { at: 5 }
    );
    assert_eq!(
        err("12.5\u{b0}3'", Notation::Dms),
        AngleTextError::FractionNotLast { at: 6 }
    );
    assert_eq!(
        err("12\u{b0}60'", Notation::Dms),
        AngleTextError::MinutesOutOfRange { at: 4 }
    );
    assert_eq!(
        err("12\u{b0}59'60\"", Notation::Dms),
        AngleTextError::SecondsOutOfRange { at: 7 }
    );
    assert_eq!(
        err("12.\u{b0}", Notation::Dms),
        AngleTextError::Unexpected { at: 2 }
    );
    assert_eq!(
        err("12.5x", Notation::Degrees),
        AngleTextError::Unexpected { at: 4 }
    );
    assert_eq!(
        err("1 2", Notation::Gon),
        AngleTextError::Unexpected { at: 2 }
    );
    let long = format!("1.{}", "1".repeat(31));
    assert_eq!(
        err(&long, Notation::Degrees),
        AngleTextError::TooManyDigits { at: 2 }
    );
    let wide = "1".repeat(21);
    assert_eq!(
        err(&wide, Notation::Degrees),
        AngleTextError::TooManyDigits { at: 0 }
    );
    assert_eq!(
        err("2000000000000\u{b0}", Notation::Dms),
        AngleTextError::TooLarge
    );
    assert_eq!(
        err("12.6000", Notation::Compact),
        AngleTextError::MinutesOutOfRange { at: 3 }
    );
    assert_eq!(
        err("12.0060", Notation::Compact),
        AngleTextError::SecondsOutOfRange { at: 5 }
    );
    assert_eq!(
        err("2000000000000.0", Notation::Compact),
        AngleTextError::TooLarge
    );
    assert_eq!(
        write(AngleUnits(1), Notation::Dms, MAX_WRITE_PLACES + 1),
        Err(AngleTextError::TooManyPlaces)
    );
    // Every error says something in words.
    for e in [
        AngleTextError::Empty,
        AngleTextError::Unexpected { at: 1 },
        AngleTextError::MissingSymbol { at: 1 },
        AngleTextError::OutOfOrder { at: 1 },
        AngleTextError::FractionNotLast { at: 1 },
        AngleTextError::MinutesOutOfRange { at: 1 },
        AngleTextError::SecondsOutOfRange { at: 1 },
        AngleTextError::TooManyDigits { at: 1 },
        AngleTextError::TooLarge,
        AngleTextError::QuadrantAngle,
        AngleTextError::TooManyPlaces,
    ] {
        assert!(e.to_string().len() > 10, "{e:?}");
    }
}

#[test]
fn rounding_is_to_nearest_ties_to_even_and_is_reported() {
    let read = |text: &str| {
        let r = parse(text, Notation::Dms).unwrap();
        (r.value().get(), r.is_exact())
    };
    // 0.0004″ is one unit: 0.0002″ is half a unit, 0.0006″ one and a half.
    assert_eq!(read("0.0002\""), (0, false));
    assert_eq!(read("0.0006\""), (2, false));
    assert_eq!(read("0.0003\""), (1, false));
    assert_eq!(read("0.0004\""), (1, true));
    assert_eq!(read("-0.0006\""), (-2, false));
    assert_eq!(read("0.000200000000000000000000000001\""), (1, false));
    // 0.000001 gon is 8.1 units, a finer reading than the unit.
    let g = parse("0.000001 gon", Notation::Gon).unwrap();
    assert_eq!((g.value().get(), g.is_exact()), (8, false));
    // Writing rounds too: a unit in decimal degrees to six places is 0.000000111°.
    let w = write(AngleUnits(1), Notation::Degrees, 6).unwrap();
    assert!(!w.is_exact());
    assert_eq!(w.value().to_string(), "0.000000\u{b0}");
    let w = write(AngleUnits(-5), Notation::Degrees, 6).unwrap();
    assert_eq!(w.value().to_string(), "-0.000001\u{b0}");
    // A rounding that carries into the minutes and degrees.
    let w = write(AngleUnits(dms_units(10, 59, 59, 99) + 24), Notation::Dms, 1).unwrap();
    assert_eq!(w.value().to_string(), "11\u{b0}00'00.0\"");
    assert_eq!(w.value().notation(), Notation::Dms);
    assert_eq!(w.value().places(), 1);
    // The ends of i64 write and read back.
    let mut buffer = String::new();
    for v in [i64::MAX, i64::MIN + 1, -1, 0] {
        let w = shortest(AngleUnits(v), Notation::Dms).unwrap();
        buffer.clear();
        write!(buffer, "{w}").unwrap();
        assert_eq!(exact(&buffer, Notation::Dms), v);
    }
    assert!(shortest(AngleUnits(i64::MIN), Notation::Dms).is_some());
}

#[test]
fn shortest_texts_read_back_and_exist_in_degrees_and_gon_just_when_whole() {
    let mut rng = Rng::for_test("shortest_texts_read_back_and_exist_in_degrees_and_gon");
    let mut buffer = String::new();
    for i in 0..20_000 {
        let v = match i % 4 {
            0 => rng.next() as i64,
            1 => (rng.next() % (2 * FULL_TURN as u64)) as i64 - FULL_TURN,
            2 => ((rng.next() % 1_000_000) as i64) * 81,
            _ => ((rng.next() % 1_000_000) as i64) * 9,
        };
        for notation in [
            Notation::Dms,
            Notation::Compact,
            Notation::Degrees,
            Notation::Gon,
        ] {
            let whole = match notation {
                Notation::Degrees => v % 9 == 0,
                Notation::Gon => v % 81 == 0,
                _ => true,
            };
            match shortest(AngleUnits(v), notation) {
                Some(w) => {
                    assert!(whole, "{v} {notation:?}");
                    buffer.clear();
                    write!(buffer, "{w}").unwrap();
                    assert_eq!(exact(&buffer, notation), v, "{buffer}");
                }
                None => assert!(!whole, "{v} {notation:?}"),
            }
        }
    }
}
