// SPDX-License-Identifier: Apache-2.0
//! The law catalogue's row for formats (chapter 28.3; piece G155), on `geom-format`.
//!
//! - **Write, read, write is byte-identical.** Every curve and surface kind the catalogue draws
//!   (`tests/truth`), a stored body (its index and chunks), a record, a whole file and a CBOR item
//!   written, read and written again give the same bytes; a CBOR item through its text form and
//!   back gives the same bytes too.
//! - **Reading what was written gives back the same value.** Kinds, bodies, records and files
//!   compare equal to what was put in (a body's content ID is a function of the body).
//! - **Readers never panic on any bytes.** Every reader of the format is handed random bytes,
//!   and valid encodings with a byte changed, dropped or added, and may answer only `Ok` or a
//!   typed error; a panic is a finding. The readers: the strict CBOR decoder and checker, the text
//!   reader, the record decoder, the file reader and salvage, the header, names, history, recipe,
//!   operation and attribute records, and the curve and surface kinds.
//!
//! The values of the exchange crates' formats join the row as those crates are built. Seeds: each
//! law prints `seed: <hex>` and replays with `SPAREKIT_SEED`; pull requests run the fixed default
//! seed and the nightly workflow a fresh one.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unwrap_used,
    clippy::large_enum_variant,
    clippy::similar_names,
    clippy::expect_used,
    reason = "tests: oracles and generators written as the formulas read, and a failure stops the test"
)]

// The kinds and their truth, which the shared catalogue draws on (piece G69).
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/laws/mod.rs"]
mod laws;
#[allow(
    dead_code,
    unreachable_pub,
    reason = "the catalogue uses the transitions' truth only"
)]
#[path = "../../geom-curve/tests/series/mod.rs"]
mod series;
#[path = "catalogue/mod.rs"]
mod catalogue;
mod truth;

use std::panic::{AssertUnwindSafe, catch_unwind};

use catalogue::{Draw, Findings};
use geom_bool::combine::unite;
use geom_bool::operand::{Operand, OperandKey, OperandKind};
use geom_curve::CurveKind;
use geom_format::body::{read_body, write_body};
use geom_format::cbor::{self, Decoder};
use geom_format::file::{self, HEADER_TAG};
use geom_format::header::Header;
use geom_format::kinds::{DEPTH, read_curve, read_surface, write_curve, write_surface};
use geom_format::record::{COPY_ALWAYS, REQUIRED, Record};
use geom_format::source::SliceSource;
use geom_make::{BoxInput, make_box};
use geom_num::{Exactness, P3, Steps};
use geom_op::cbor::Encoder;
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, Exec, WorkMeter};
use geom_topo::Body;
use truth::Rng;

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([9; 16])), Budget::UNLIMITED)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().take(48).map(|b| format!("{b:02x}")).collect()
}

fn curve_bytes<const D: usize>(c: &CurveKind<D>) -> Result<Vec<u8>, String> {
    let mut e = Encoder::new();
    write_curve(&mut e, c).map_err(|x| format!("write: {x}"))?;
    Ok(e.finish().map_err(|x| format!("finish: {x}"))?.into_bytes())
}

fn surface_bytes(s: &geom_surface::SurfaceKind) -> Result<Vec<u8>, String> {
    let mut e = Encoder::new();
    write_surface(&mut e, s).map_err(|x| format!("write: {x}"))?;
    Ok(e.finish().map_err(|x| format!("finish: {x}"))?.into_bytes())
}

fn kind_name<const D: usize>(c: &CurveKind<D>) -> &'static str {
    truth::curve_name(c)
}

/// Draws the catalogue's kinds from the law's seed and checks `write, read, write`.
fn kinds_trip<const D: usize>(f: &mut Findings, rng: &mut Rng, grid: bool) {
    for (kind, _) in truth::curves::<D>(rng, grid) {
        f.ran();
        let name = format!("{}{D}", kind_name(&kind));
        let bytes = match curve_bytes(&kind) {
            Ok(b) => b,
            Err(why) => {
                f.add(&format!("{name} cannot be written"), || why);
                continue;
            }
        };
        let mut d = Decoder::new(&bytes, DEPTH);
        match read_curve::<D>(&mut d, &mut meter()) {
            Ok(back) => {
                f.check(back == kind, &format!("{name} reads back as written"), || {
                    format!("{kind:?}")
                });
                f.check(
                    curve_bytes(&back).as_deref() == Ok(bytes.as_slice()),
                    &format!("{name}: write, read, write"),
                    || hex(&bytes),
                );
                f.check(d.end() == Ok(bytes.len()), &format!("{name} is read to its end"), || {
                    hex(&bytes)
                });
            }
            Err(why) => f.add(&format!("{name} cannot be read back"), || format!("{why}")),
        }
    }
}

#[test]
fn every_kind_reads_back_as_written_and_writes_the_same_bytes() {
    let mut rng = Rng(geom_test::SplitMix64::new(
        geom_test::seed::resolve("formats: kinds", geom_test::Seed(0x7001)).unwrap(),
    ));
    let mut f = Findings::default();
    for round in 0..12 {
        let grid = round % 2 == 0;
        kinds_trip::<3>(&mut f, &mut rng, grid);
        kinds_trip::<2>(&mut f, &mut rng, grid);
        for (kind, _) in truth::surfaces(&mut rng, grid) {
            f.ran();
            let name = truth::surface_name(&kind);
            let bytes = match surface_bytes(&kind) {
                Ok(b) => b,
                Err(why) => {
                    f.add(&format!("{name} cannot be written"), || why);
                    continue;
                }
            };
            match read_surface(&mut Decoder::new(&bytes, DEPTH), &mut meter()) {
                Ok(back) => {
                    f.check(back == kind, &format!("{name} reads back as written"), || {
                        format!("{kind:?}")
                    });
                    f.check(
                        surface_bytes(&back).as_deref() == Ok(bytes.as_slice()),
                        &format!("{name}: write, read, write"),
                        || hex(&bytes),
                    );
                }
                Err(why) => f.add(&format!("{name} cannot be read back"), || format!("{why}")),
            }
        }
    }
    f.assert_none("formats: kinds", 100);
}

fn box_body(low: [i64; 3], size: [u64; 3]) -> Body {
    let input = BoxInput {
        min: P3::new(low[0], low[1], low[2]).unwrap(),
        size: size.map(Steps),
    };
    make_box(&ctx(), &input).unwrap().result.body
}

#[test]
fn a_stored_body_reads_back_equal_and_writes_the_same_records() {
    let mut d = Draw::new("formats: bodies", 0x7002);
    let mut f = Findings::default();
    for _ in 0..40 {
        let a = box_body(
            [0; 3].map(|_: i64| d.between(-5, 5)),
            [0; 3].map(|_: u64| d.between(1, 6) as u64),
        );
        let body = if d.one_in(2) {
            let b = box_body(
                [0; 3].map(|_: i64| d.between(-5, 5)),
                [0; 3].map(|_: u64| d.between(1, 6) as u64),
            );
            let operand = |n: u8, body: &Body| Operand {
                key: OperandKey([n; 16]),
                body,
                kind: OperandKind::Solid,
                exactness: Exactness::Exact,
            };
            match unite(&ctx(), &Exec::serial(), &[operand(1, &a), operand(2, &b)]) {
                Ok(o) => o.result,
                Err(e) => {
                    f.add("a union the kernel would not build", || format!("{e:?}"));
                    continue;
                }
            }
        } else {
            a
        };
        f.ran();
        let stored = match write_body(&body, &mut meter()) {
            Ok(s) => s,
            Err(e) => {
                f.add("a body cannot be written", || format!("{e}"));
                continue;
            }
        };
        match read_body(&stored, &mut meter()) {
            Ok(back) => {
                f.check(back == body, "a body reads back equal", || "a box body".to_owned());
                match write_body(&back, &mut meter()) {
                    Ok(again) => {
                        f.check(again == stored, "write, read, write: the same records", || {
                            "a box body".to_owned()
                        });
                        f.check(
                            again.content_id() == stored.content_id(),
                            "the content ID is a function of the body",
                            || "a box body".to_owned(),
                        );
                    }
                    Err(e) => f.add("a body read back cannot be written", || format!("{e}")),
                }
            }
            Err(e) => f.add("a body cannot be read back", || format!("{e}")),
        }
    }
    f.assert_none("formats: bodies", 30);
}

fn random_record(d: &mut Draw, tag_pool: &[u64]) -> Record {
    let len = d.pick(40);
    let mut e = Encoder::new();
    // One CBOR item for the payload: a byte string of random bytes.
    e.bytes(&d.bytes(len)).unwrap();
    Record {
        tag: tag_pool[d.pick(tag_pool.len())],
        flags: if d.one_in(2) { REQUIRED } else { 0 } | if d.one_in(3) { COPY_ALWAYS } else { 0 },
        schema: d.below(4),
        depends: Vec::new(),
        payload: e.finish().unwrap().into_bytes(),
    }
}

#[test]
fn records_and_files_read_back_equal_and_write_the_same_bytes() {
    let mut d = Draw::new("formats: records and files", 0x7003);
    let mut f = Findings::default();
    for _ in 0..80 {
        f.ran();
        let record = random_record(&mut d, &[3, 10, 11, 16, 99]);
        match record.encode(&mut meter()) {
            Ok(bytes) => match Record::decode(&bytes, &mut meter()) {
                Ok((back, used)) => {
                    f.check(back == record && used == bytes.len(), "a record reads back equal", || {
                        format!("{record:?}")
                    });
                    f.check(
                        back.encode(&mut meter()).as_ref() == Ok(&bytes),
                        "a record: write, read, write",
                        || hex(&bytes),
                    );
                }
                Err(e) => f.add("a record cannot be read back", || format!("{e:?}")),
            },
            Err(e) => f.add("a record cannot be written", || format!("{e:?}")),
        }
        let header = Header::v1("law catalogue", Steps(d.below(1000) + 1))
            .record()
            .unwrap();
        let records: Vec<Record> = (0..d.pick(6))
            .map(|_| random_record(&mut d, &[10, 11, 12]))
            .collect();
        let Ok(bytes) = file::write(&header, &records, &mut meter()) else {
            f.add("a file cannot be written", || format!("{} records", records.len()));
            continue;
        };
        let known = |tag: u64, _: u64| tag == 10 || tag == 11 || tag == 12;
        match file::read(&mut SliceSource::new(&bytes), known, &mut meter()) {
            Ok(read) => {
                f.check(read.header == header, "a file's header reads back equal", || {
                    hex(&bytes)
                });
                f.check(read.records == records, "a file's records read back equal", || {
                    hex(&bytes)
                });
                f.check(
                    file::write(&read.header, &read.records, &mut meter()).as_ref() == Ok(&bytes),
                    "a file: write, read, write",
                    || hex(&bytes),
                );
                f.check(Header::read(&read.header.payload, &mut meter()).is_ok(), "the header record parses", || {
                    hex(&bytes)
                });
            }
            Err(e) => f.add("a file cannot be read back", || format!("{e:?}")),
        }
        f.check(HEADER_TAG == header.tag, "the header record's tag", || String::new());
    }
    f.assert_none("formats: records and files", 70);
}

/// A random CBOR item of the subset the format uses, written by the kernel's encoder.
fn item(e: &mut Encoder, d: &mut Draw, depth: usize) {
    let leaf = depth == 0 || d.one_in(3);
    match if leaf { d.below(5) } else { 5 + d.below(2) } {
        0 => e.uint(d.0.next_u64() >> d.below(64)).unwrap(),
        1 => e.int(-(i64::try_from(d.0.next_u64() >> 2).unwrap()) - 1).unwrap(),
        2 => {
            let n = d.pick(20);
            e.bytes(&d.bytes(n)).unwrap();
        }
        3 => {
            let n = d.pick(12);
            let s: String = (0..n).map(|_| char::from(b'a' + u8::try_from(d.below(26)).unwrap())).collect();
            e.text(&s).unwrap();
        }
        4 => e.float([0.5, -2.0, 1.0e10, 3.141592653589793, 1.0 / 3.0][d.pick(5)]).unwrap(),
        5 => {
            let n = d.pick(4);
            e.array(n as u64).unwrap();
            for _ in 0..n {
                item(e, d, depth - 1);
            }
        }
        _ => {
            let n = d.pick(3);
            e.array(n as u64).unwrap();
            for _ in 0..n {
                item(e, d, depth - 1);
            }
        }
    }
}

#[test]
fn a_cbor_item_checks_and_goes_through_its_text_and_back_to_the_same_bytes() {
    let mut d = Draw::new("formats: cbor", 0x7004);
    let mut f = Findings::default();
    for _ in 0..200 {
        f.ran();
        let mut e = Encoder::new();
        item(&mut e, &mut d, 3);
        let bytes = e.finish().unwrap().into_bytes();
        f.check(cbor::check(&bytes, DEPTH, &mut meter()).is_ok(), "the encoder's bytes pass the strict decoder", || {
            hex(&bytes)
        });
        match cbor::to_text(&bytes, DEPTH, &mut meter()) {
            Ok(text) => match cbor::from_text(&text, DEPTH, &mut meter()) {
                Ok(again) => f.check(again == bytes, "bytes, text, bytes", || format!("{text}")),
                Err(e) => f.add("the text of an item cannot be read", || format!("{text}: {e:?}")),
            },
            Err(e) => f.add("an item has no text", || format!("{e:?}: {}", hex(&bytes))),
        }
    }
    f.assert_none("formats: cbor", 150);
}

/// Hands `bytes` to every reader of the format; a panic is the only way to fail.
fn read_everything(bytes: &[u8]) -> Vec<&'static str> {
    let mut panicked = Vec::new();
    let mut guard = |name: &'static str, run: &dyn Fn()| {
        if catch_unwind(AssertUnwindSafe(run)).is_err() {
            panicked.push(name);
        }
    };
    guard("cbor::check", &|| {
        let _ = cbor::check(bytes, DEPTH, &mut meter());
    });
    guard("Decoder::next", &|| {
        let mut d = Decoder::new(bytes, DEPTH);
        let mut m = meter();
        for _ in 0..64 {
            match d.next(&mut m) {
                Ok(Some(_)) => {}
                _ => break,
            }
        }
    });
    guard("cbor::item_len", &|| {
        let _ = cbor::item_len(bytes, DEPTH, &mut meter());
    });
    guard("cbor::from_text", &|| {
        let _ = cbor::from_text(&String::from_utf8_lossy(bytes), DEPTH, &mut meter());
    });
    guard("Record::decode", &|| {
        let _ = Record::decode(bytes, &mut meter());
    });
    guard("file::read", &|| {
        let _ = file::read(&mut SliceSource::new(bytes), |_, _| true, &mut meter());
    });
    guard("file::salvage", &|| {
        let _ = file::salvage(bytes, &mut meter());
    });
    guard("Header::read", &|| {
        let _ = Header::read(bytes, &mut meter());
    });
    guard("read_names", &|| {
        let _ = geom_format::names::read_names(bytes, 1, &mut meter());
    });
    guard("read_topo_name", &|| {
        let _ = geom_format::names::read_topo_name(bytes, &mut meter());
    });
    guard("read_history", &|| {
        let _ = geom_format::history::read_history(bytes, &mut meter());
    });
    guard("read_recipe", &|| {
        let _ = geom_format::recipe::read_recipe(bytes, &mut meter());
    });
    guard("read_operation", &|| {
        let _ = geom_format::operation::read_operation(bytes, &mut meter());
    });
    guard("read_attributes", &|| {
        let _ = geom_format::attributes::read_attributes(bytes, &mut meter());
    });
    guard("read_curve::<2>", &|| {
        let _ = read_curve::<2>(&mut Decoder::new(bytes, DEPTH), &mut meter());
    });
    guard("read_curve::<3>", &|| {
        let _ = read_curve::<3>(&mut Decoder::new(bytes, DEPTH), &mut meter());
    });
    guard("read_surface", &|| {
        let _ = read_surface(&mut Decoder::new(bytes, DEPTH), &mut meter());
    });
    panicked
}

/// Changes `bytes` the way damage does: a byte replaced, dropped or added, or the end cut off.
fn damaged(d: &mut Draw, bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for _ in 0..=d.below(3) {
        if out.is_empty() {
            break;
        }
        let at = d.pick(out.len());
        match d.below(4) {
            0 => out[at] = u8::try_from(d.below(256)).unwrap(),
            1 => {
                out.remove(at);
            }
            2 => out.insert(at, u8::try_from(d.below(256)).unwrap()),
            _ => out.truncate(at),
        }
    }
    out
}

#[test]
fn readers_never_panic_on_any_bytes() {
    let mut d = Draw::new("formats: readers on any bytes", 0x7005);
    let mut rng = Rng(geom_test::SplitMix64::new(geom_test::Seed(d.0.next_u64())));
    let mut f = Findings::default();
    // Valid encodings to damage: kinds, a record, a file, a header.
    let mut valid: Vec<Vec<u8>> = Vec::new();
    for (kind, _) in truth::curves::<3>(&mut rng, true) {
        valid.extend(curve_bytes(&kind));
    }
    for (kind, _) in truth::surfaces(&mut rng, true) {
        valid.extend(surface_bytes(&kind));
    }
    let header = Header::v1("law catalogue", Steps(7)).record().unwrap();
    valid.push(header.payload.clone());
    valid.extend(header.encode(&mut meter()));
    let records: Vec<Record> = (0..3).map(|_| random_record(&mut d, &[10, 11])).collect();
    valid.extend(file::write(&header, &records, &mut meter()));
    f.ran();
    for round in 0..600 {
        let bytes = match round % 3 {
            0 => {
                let n = d.pick(48);
                let mut b = d.bytes(n);
                // Lean on CBOR's own first bytes so the readers get past the first check.
                if let Some(first) = b.first_mut() {
                    *first = [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0xa1, 0x58, 0x40, 0x1b][d.pick(10)];
                }
                b
            }
            _ => {
                let base = &valid[d.pick(valid.len())];
                damaged(&mut d, base)
            }
        };
        f.ran();
        for name in read_everything(&bytes) {
            f.add(&format!("{name} panicked"), || hex(&bytes));
        }
    }
    f.assert_none("formats: readers on any bytes", 600);
}
