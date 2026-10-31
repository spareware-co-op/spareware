//! The replay hash of piece G188: circles and arcs over a fixed corpus, hashed with SHA-256, must
//! give the same digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed seed: seeded circles about rational points and about grid points, at
//! every scale up to each budget, each question of `geom_num::arc` asked of them, seeded arcs on
//! their chords' bisectors, and the extreme values. Each answer and each error is one byte, so a
//! target that decides one sign differently changes the digest. The digest lives in the store; a
//! change is a bug, or a new definition with a golden of its own (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic)]

use geom_num::arc::{Arc2, Circle2, Contact, Pair, RADIUS_BITS, Round};
use geom_num::carrier::{Carrier, NORMAL_BITS, OFFSET_BITS, POINT_BITS, Rational2, WEIGHT_BITS};
use geom_num::grid::{COORD_MAX, P2};
use geom_num::{NumError, Sign};
use geom_test::SplitMix64;
use geom_test::golden::{self, hex, sha256};
use geom_test::seed::Seed;

const GOLDEN: &str = "geom-num/arc";

/// The corpus's fixed seed. Never `SPAREKIT_SEED`: a golden's inputs are part of its definition.
const SEED: u64 = 0x4731_3838_4172_6373;

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

fn circle(rng: &mut SplitMix64) -> Circle2 {
    loop {
        let centre = grid(rng);
        if let Ok(c) = Circle2::new(centre, signed(rng, RADIUS_BITS)) {
            return c;
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

fn pair_byte(answer: Result<Pair, NumError>) -> u8 {
    match answer {
        Ok(Pair::Apart) => 9,
        Ok(Pair::TouchingOutside) => 10,
        Ok(Pair::Crossing) => 11,
        Ok(Pair::TouchingInside) => 12,
        Ok(Pair::Nested) => 13,
        Ok(Pair::Same) => 14,
        Err(_) => 15,
    }
}

fn corpus() -> Vec<u8> {
    let mut rng = SplitMix64::new(Seed(SEED));
    let mut out = Vec::new();
    for _ in 0..1_500 {
        let round = Round::new(grid(&mut rng), point(&mut rng));
        let (z, p, l) = (grid(&mut rng), point(&mut rng), carrier(&mut rng));
        out.push(sign_byte(round.side_of_grid(z)));
        out.push(sign_byte(round.side(&p)));
        out.push(contact_byte(round.meets(&l)));
        let (c, d) = (circle(&mut rng), circle(&mut rng));
        out.push(sign_byte(c.side(&p)));
        out.push(contact_byte(c.meets(&l)));
        out.push(pair_byte(c.against(&d)));
        let (s, e) = (grid(&mut rng), grid(&mut rng));
        let centre = Arc2::bisector(s, e).and_then(|b| b.meet(&l));
        match centre.and_then(|c| Arc2::new(s, e, c)) {
            Ok(arc) => {
                out.push(u8::from(arc.holds(&p).unwrap()));
                out.push(sign_byte(arc.order(&p, &z.into())));
            }
            Err(e) => out.push(sign_byte(Err(e))),
        }
    }
    let m = COORD_MAX;
    let (p, r) = (P2::new(m, -m).unwrap(), (1_i128 << RADIUS_BITS) - 1);
    let top = (1_i128 << POINT_BITS) - 1;
    let weight = (1_i128 << WEIGHT_BITS) - 1;
    let q = Rational2::new(top, -top, weight).unwrap();
    let round = Round::new(p, q);
    out.push(sign_byte(round.side_of_grid(P2::new(-m, m).unwrap())));
    out.push(sign_byte(round.side(&q)));
    let l = Carrier::new((1 << NORMAL_BITS) - 1, 1, (1 << OFFSET_BITS) - 1).unwrap();
    out.push(contact_byte(round.meets(&l)));
    let c = Circle2::new(p, r).unwrap();
    out.push(sign_byte(c.side(&q)));
    out.push(contact_byte(c.meets(&l)));
    out
}

/// The golden hash. The same digest must hold on every target; a change is a bug (see the module
/// documentation).
#[test]
fn arc_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let bytes = corpus();
        (sha256(&bytes), bytes)
    });
    let bytes = &checked.value;
    println!(
        "geom-num arc corpus: {} results, golden hash {}",
        bytes.len(),
        hex(sha256(bytes))
    );
    assert!(bytes.len() >= 5_000, "only {} results", bytes.len());
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target decides a sign differently (compare this \
             log's digest with another target's), or a definition changed in place, which chapter \
             26.2 forbids"
        )
    });
}
