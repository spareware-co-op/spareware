//! The replay hash of piece G187: the 2D carriers and rational points over a fixed corpus, hashed
//! with SHA-256, must give the same digest on every target (chapters 21 and 28.13; the golden store
//! of piece G19).
//!
//! The corpus, from a fixed seed: seeded carriers and rational points at every scale up to each
//! budget and the meetings of the carriers, each question of `geom_num::carrier` asked of them, and
//! the extreme values. Each answer and each error is one byte, and each point its three integers, so
//! a target that decides one sign differently changes the digest. The digest lives in the store; a
//! change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic)]

use geom_num::NumError;
use geom_num::Sign;
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

const GOLDEN: &str = "geom-num/carrier";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4731_3837_4361_7272;

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

fn push_point(out: &mut Vec<u8>, p: &Rational2) {
    let ([x, y], w) = p.coordinates();
    for v in [x, y, w] {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

struct Record {
    bytes: Vec<u8>,
    results: usize,
}

fn ask(rec: &mut Record, l: &Carrier, p: &Rational2, q: &Rational2, r: &Rational2) {
    for answer in [
        l.side(p),
        l.order(p, q),
        p.cmp_x(q),
        p.cmp_y(q),
        Rational2::orient(p, q, r),
    ] {
        rec.bytes.push(sign_byte(answer));
        rec.results += 1;
    }
    rec.bytes.push(u8::from(p.same(q).unwrap()));
    rec.results += 1;
}

fn corpus() -> Record {
    let mut rng = SplitMix64::new(Seed(SEED));
    let mut rec = Record {
        bytes: Vec::new(),
        results: 0,
    };
    let mut carriers = Vec::new();
    while carriers.len() < 600 {
        let (a, b, c) = (
            signed(&mut rng, NORMAL_BITS),
            signed(&mut rng, NORMAL_BITS),
            signed(&mut rng, OFFSET_BITS),
        );
        if let Ok(l) = Carrier::new(a, b, c) {
            carriers.push(l);
        }
    }
    let mut points = Vec::new();
    while points.len() < 600 {
        let (x, y, w) = (
            signed(&mut rng, POINT_BITS),
            signed(&mut rng, POINT_BITS),
            signed(&mut rng, WEIGHT_BITS),
        );
        if let Ok(p) = Rational2::new(x, y, w) {
            points.push(p);
        }
    }
    for pair in carriers.windows(2) {
        match pair[0].meet(&pair[1]) {
            Ok(p) => {
                push_point(&mut rec.bytes, &p);
                points.push(p);
            }
            Err(e) => rec.bytes.push(sign_byte(Err(e))),
        }
        rec.results += 1;
    }
    let (n, d) = ((1_i128 << NORMAL_BITS) - 1, (1_i128 << OFFSET_BITS) - 1);
    let (p, w) = ((1_i128 << POINT_BITS) - 1, (1_i128 << WEIGHT_BITS) - 1);
    carriers.push(Carrier::new(n, -n, d).unwrap());
    points.push(Rational2::new(p, -p, w).unwrap());
    points.push(Rational2::new(-p, -p, 1).unwrap());
    points.push(Rational2::new(p, p, w).unwrap());
    let count = points.len();
    for (k, l) in carriers.iter().cycle().take(count).enumerate() {
        let (p, q, r) = (
            &points[k],
            &points[(k + 1) % count],
            &points[(k + 2) % count],
        );
        ask(&mut rec, l, p, q, r);
    }
    rec
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn carrier_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-num carrier corpus: {} results, {} bytes, golden hash {}",
        rec.results,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.results >= 5_000, "only {} results", rec.results);
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target decides a sign differently (compare this \
             log's digest with another target's), or a definition changed in place, which chapter \
             26.2 forbids"
        )
    });
}
