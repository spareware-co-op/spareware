//! The replay hash of piece G190: ellipses and quartics over a fixed corpus, hashed with SHA-256,
//! must give the same digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed seed: seeded ellipses at every scale up to each budget, every question
//! of `geom_num::conic` asked of them and of the carriers and points about them, the crossings of
//! `geom_num::one_root` with them, how each pair of ellipses meets, and how the roots of two pairs
//! compare. Each answer and each error is one byte and each crossing and quartic its integers, so a
//! target that decides one sign differently changes the digest. The digest lives in the store; a
//! change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "e, l, m, p, q, r, s and z are the corpus's curves and points"
)]
#![allow(clippy::unwrap_used, clippy::panic)]

use geom_num::arc::{Contact, RADIUS_BITS};
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_num::conic::{Conic, Ellipse2, Meeting, RATIO_BITS};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root};
use geom_num::quartic::{Quartic, RealRoot, compare};
use geom_num::{NumError, Sign};
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

const GOLDEN: &str = "geom-num/conic";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4731_3930_436f_6e69;

/// A signed integer of at most `bits` bits, its size drawn too.
fn signed(rng: &mut SplitMix64, bits: u32) -> i128 {
    let size = 1 + (rng.next_u64() % u64::from(bits)) as u32;
    let raw = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
    let magnitude = (raw >> (128 - size)) as i128;
    if rng.next_u64() & 1 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn grid(rng: &mut SplitMix64) -> P2 {
    let x = (signed(rng, 40) as i64).clamp(-COORD_MAX, COORD_MAX);
    let y = (signed(rng, 40) as i64).clamp(-COORD_MAX, COORD_MAX);
    P2::new(x, y).unwrap()
}

fn point(rng: &mut SplitMix64) -> Rational2 {
    loop {
        let (x, y, w) = (
            signed(rng, POINT_BITS),
            signed(rng, POINT_BITS),
            signed(rng, WEIGHT_BITS),
        );
        if let Ok(p) = Rational2::new(x, y, w) {
            return p;
        }
    }
}

fn carrier(rng: &mut SplitMix64) -> Carrier {
    loop {
        let (a, b, c) = (
            signed(rng, NORMAL_BITS),
            signed(rng, NORMAL_BITS),
            signed(rng, OFFSET_BITS),
        );
        if let Ok(l) = Carrier::new(a, b, c) {
            return l;
        }
    }
}

fn ellipse(rng: &mut SplitMix64) -> Ellipse2 {
    loop {
        let centre = grid(rng);
        let major = [
            signed(rng, RADIUS_BITS) as i64,
            signed(rng, RADIUS_BITS) as i64,
        ];
        let q = 1 + signed(rng, RATIO_BITS).unsigned_abs() as i64;
        let p = 1 + signed(rng, RATIO_BITS).unsigned_abs() as i64 % q;
        if let Ok(e) = Ellipse2::new(centre, major, p, q) {
            return e;
        }
    }
}

/// One answer as one byte: a sign, or the error that stood in its place.
fn sign_byte(answer: Result<Sign, NumError>) -> u8 {
    match answer {
        Ok(Sign::Negative) => 0,
        Ok(Sign::Zero) => 1,
        Ok(Sign::Positive) => 2,
        Err(NumError::Degenerate) => 3,
        Err(_) => 4,
    }
}

fn contact_byte(answer: Result<Contact, NumError>) -> u8 {
    match answer {
        Ok(Contact::Apart) => 5,
        Ok(Contact::Tangent) => 6,
        Ok(Contact::Crossing) => 7,
        Err(_) => 8,
    }
}

/// A crossing's integers, or the error that stood in their place.
fn crossing_bytes(out: &mut Vec<u8>, p: &Result<Crossing2, NumError>) {
    let Ok(p) = p else {
        out.push(9);
        return;
    };
    let ([ax, ay], [bx, by], k, w) = p.parts();
    for word in [ax, ay, bx, by, k, w] {
        for limb in word.to_le_limbs() {
            out.extend_from_slice(&limb.to_le_bytes());
        }
    }
}

/// How a pair meets: its quartic's integers and each root's place and multiplicity.
fn meeting_bytes(
    out: &mut Vec<u8>,
    meeting: &Result<Meeting, NumError>,
) -> Option<(Quartic, RealRoot)> {
    let Meeting::At { quartic, roots } = *meeting.as_ref().unwrap() else {
        out.push(10);
        return None;
    };
    for c in quartic.coefficients() {
        for limb in c.to_le_limbs() {
            out.extend_from_slice(&limb.to_le_bytes());
        }
    }
    for root in roots.iter() {
        out.push(root.piece() as u8);
        out.push(root.multiplicity());
    }
    roots.iter().next().map(|r| (quartic, *r))
}

fn corpus() -> Vec<u8> {
    let mut rng = SplitMix64::new(Seed(SEED));
    let mut out = Vec::new();
    let mut last: Option<(Quartic, RealRoot)> = None;
    for _ in 0..300 {
        let (e, other) = (ellipse(&mut rng), ellipse(&mut rng));
        let (z, l) = (point(&mut rng), carrier(&mut rng));
        out.push(sign_byte(e.side(&z)));
        out.push(contact_byte(e.meets(&l)));
        let across = Carrier::through(e.centre(), grid(&mut rng));
        if let Ok(m) = across {
            let p = Crossing2::of_ellipse(&m, &e, Root::First);
            let q = Crossing2::of_ellipse(&m, &e, Root::Second);
            crossing_bytes(&mut out, &p);
            crossing_bytes(&mut out, &q);
            if let Ok(p) = p {
                out.push(sign_byte(p.against_ellipse(&other)));
            }
        }
        // On one centre, so that they meet.
        let (r, s) = other.ratio();
        let there = Ellipse2::new(e.centre(), other.major(), r, s).unwrap();
        let conic = Conic::of_ellipse(&there).unwrap();
        let found = meeting_bytes(&mut out, &e.meeting(&conic));
        if let (Some((a, p)), Some((b, q))) = (&found, &last) {
            out.push(sign_byte(compare(a, p, b, q)));
        }
        if found.is_some() {
            last = found;
        }
    }
    out
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn conic_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    let bytes = &checked.value;
    println!(
        "geom-num conic corpus: {} bytes, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(bytes.len() >= 5_000, "only {} bytes", bytes.len());
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target decides a sign differently (compare this \
             log's digest with another target's), or a definition changed in place, which chapter \
             26.2 forbids"
        )
    });
}
