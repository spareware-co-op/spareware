//! The replay hash of piece G189: one-root numbers over a fixed corpus, hashed with SHA-256, must
//! give the same digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed seed: seeded circles and carriers near them at every scale up to each
//! budget, their crossings, every question of `geom_num::one_root` asked of them, and the extreme
//! values. Each answer and each error is one byte and each crossing its integers, so a target that
//! decides one sign differently changes the digest. The digest lives in the store; a change is a
//! bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic)]

use geom_num::arc::{Arc2, Circle2, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, NORMAL_BITS};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::one_root::{Crossing2, Root};
use geom_num::{NumError, Sign};
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

const GOLDEN: &str = "geom-num/one-root";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4731_3839_526f_6f74;

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

fn circle(rng: &mut SplitMix64) -> Circle2 {
    loop {
        let centre = grid(rng);
        if let Ok(c) = Circle2::new(centre, signed(rng, RADIUS_BITS)) {
            return c;
        }
    }
}

/// A carrier through a point of `c`'s neighbourhood, so that they usually meet.
fn near(rng: &mut SplitMix64, c: &Circle2) -> Carrier {
    loop {
        let centre = c.centre();
        let r = c.radius() as i64;
        let dx = (signed(rng, 40) as i64).clamp(-r, r);
        let dy = (signed(rng, 40) as i64).clamp(-r, r);
        let Ok(p) = P2::new(
            (centre.x.get() + dx / 2).clamp(-COORD_MAX, COORD_MAX),
            (centre.y.get() + dy / 2).clamp(-COORD_MAX, COORD_MAX),
        ) else {
            continue;
        };
        if let Ok(l) = Carrier::through(p, grid(rng)) {
            return l;
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

/// A crossing's integers, or the error that stood in their place.
fn crossing_bytes(out: &mut Vec<u8>, p: &Result<Crossing2, NumError>) {
    let Ok(p) = p else {
        out.push(5);
        return;
    };
    let ([ax, ay], [bx, by], k, w) = p.parts();
    for word in [ax, ay, k, w] {
        for limb in word.to_le_limbs() {
            out.extend_from_slice(&limb.to_le_bytes());
        }
    }
    out.extend_from_slice(&bx.to_le_bytes());
    out.extend_from_slice(&by.to_le_bytes());
}

fn corpus() -> Vec<u8> {
    let mut rng = SplitMix64::new(Seed(SEED));
    let mut out = Vec::new();
    for _ in 0..1_500 {
        let (c, d) = (circle(&mut rng), circle(&mut rng));
        let (l, m) = (near(&mut rng, &c), near(&mut rng, &c));
        let p = Crossing2::of_circle(&l, &c, Root::First);
        let q = Crossing2::of_circle(&m, &c, Root::Second);
        crossing_bytes(&mut out, &p);
        crossing_bytes(&mut out, &q);
        let (Ok(p), Ok(q)) = (p, q) else {
            continue;
        };
        out.push(sign_byte(p.cmp_x(&q)));
        out.push(sign_byte(p.cmp_y(&q)));
        out.push(u8::from(p.same(&q).unwrap()));
        out.push(sign_byte(p.side(&m)));
        out.push(sign_byte(p.against_circle(&d)));
        out.push(sign_byte(p.order(&l, &q)));
        out.push(sign_byte(
            p.against_round(&Round::new(grid(&mut rng), c.centre().into())),
        ));
        let (s, e) = (grid(&mut rng), grid(&mut rng));
        let arc = Arc2::bisector(s, e)
            .and_then(|b| b.meet(&m))
            .and_then(|centre| Arc2::new(s, e, centre));
        let Ok(arc) = arc else {
            out.push(6);
            continue;
        };
        let u = Crossing2::of_round(&l, &arc.round(), Root::First);
        let v = Crossing2::of_round(&m, &arc.round(), Root::Second);
        crossing_bytes(&mut out, &u);
        crossing_bytes(&mut out, &v);
        if let (Ok(u), Ok(v)) = (u, v) {
            out.push(u8::from(arc.holds_crossing(&u).unwrap()));
            out.push(sign_byte(arc.order_crossings(&u, &v)));
        }
    }
    let m = COORD_MAX;
    let n = (1_i128 << NORMAL_BITS) - 1;
    let r = (1_i128 << RADIUS_BITS) - 1;
    let wide = Circle2::new(P2::new(m, -m).unwrap(), r).unwrap();
    let l = Carrier::new(n, n - 1, 1).unwrap();
    let p = Crossing2::of_circle(&l, &wide, Root::First);
    let q = Crossing2::of_circle(&l, &wide, Root::Second);
    crossing_bytes(&mut out, &p);
    crossing_bytes(&mut out, &q);
    out
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn one_root_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    let bytes = &checked.value;
    println!(
        "geom-num one-root corpus: {} bytes, golden hash {}",
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
