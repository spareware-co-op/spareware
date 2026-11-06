// SPDX-License-Identifier: Apache-2.0
//! Attack tests from SMLib's confirmed defects, batch 6: `geom-format`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: a defined refusal or the same answer, and never a panic,
//! a stack overflow, a hang or a different body.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

// geom-topo's fixtures, whose module allows the ones these tests do not use.
#[path = "../../geom-topo/tests/common/mod.rs"]
mod topo;

use geom_format::body::{read_body, write_body};
use geom_format::cbor::Decoder;
use geom_format::kinds::{DEPTH, read_curve, read_surface};
use topo::{many_faces, meter, wall};

/// solidcore plan/upstream-issues/UB-3--polymorphic-curve-and-surface-readers-recurse-without-a.md
/// (b4de2149): a file of 200000 nested offset records overflowed the C++ reader's stack. Each
/// level here is the two bytes that open an offset surface (an array of five, the offset code),
/// and the same bytes as a curve, as a file would carry them. Ours reads nesting with a depth
/// bound and refuses the file at the bound.
#[test]
fn ub_3_a_file_of_200000_nested_offsets_is_refused_not_followed() {
    const LEVELS: usize = 200_000;
    let mut surface_bytes = Vec::with_capacity(2 * LEVELS);
    let mut curve_bytes = Vec::with_capacity(2 * LEVELS);
    for _ in 0..LEVELS {
        // array(5), unsigned 11: an offset surface; array(4), unsigned 13: an offset curve.
        surface_bytes.extend_from_slice(&[0x85, 0x0b]);
        curve_bytes.extend_from_slice(&[0x84, 0x0d]);
    }
    assert!(read_surface(&mut Decoder::new(&surface_bytes, DEPTH), &mut meter()).is_err());
    assert!(read_curve::<3>(&mut Decoder::new(&curve_bytes, DEPTH), &mut meter()).is_err());
    assert!(read_curve::<2>(&mut Decoder::new(&curve_bytes, DEPTH), &mut meter()).is_err());
    // Nested arrays of one item, which no kind is: the same.
    let arrays = vec![0x81_u8; LEVELS];
    assert!(read_surface(&mut Decoder::new(&arrays, DEPTH), &mut meter()).is_err());
    assert!(read_curve::<3>(&mut Decoder::new(&arrays, DEPTH), &mut meter()).is_err());
}

/// solidcore plan/upstream-issues/WR-470--smbrep-readfromfile-succeeds-on-a-brep-its-own-validatepointers-rejects.md
/// (b4de2149): single-byte overwrites of stored B-reps read back as successes with radial edge
/// use lists its own validation then rejected. A stored body is bound to its bytes by content
/// IDs, so ours owes the same: a changed byte in any record, the index or a chunk, is a refusal,
/// or the body read back is the body stored. A different body is never an `Ok`.
#[test]
fn wr_470_a_changed_byte_in_a_stored_body_is_refused_or_changes_nothing() {
    for body in [wall(), many_faces(3)] {
        let stored = write_body(&body, &mut meter()).unwrap();
        assert_eq!(read_body(&stored, &mut meter()).unwrap(), body);
        for which in 0..=stored.chunks.len() {
            let len = if which == stored.chunks.len() {
                stored.index.payload.len()
            } else {
                stored.chunks[which].payload.len()
            };
            let step = if len > 600 { 7 } else { 1 };
            for at in (0..len).step_by(step) {
                for flip in [0x01_u8, 0x80, 0xff] {
                    let mut bad = stored.clone();
                    let record = if which == bad.chunks.len() {
                        &mut bad.index
                    } else {
                        &mut bad.chunks[which]
                    };
                    record.payload[at] ^= flip;
                    if let Ok(back) = read_body(&bad, &mut meter()) {
                        assert_eq!(
                            back, body,
                            "record {which}, byte {at}, xor {flip:#x} read back a different body"
                        );
                    }
                }
            }
        }
    }
}
