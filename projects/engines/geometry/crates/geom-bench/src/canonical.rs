//! The canonical byte encoding of every benchmark input and result, and its SHA-256 digest.
//!
//! A benchmark's golden digests (chapter 23.13, "Output hash") are SHA-256 hashes of these bytes:
//! one over every input of its batch, one over every result. The encoding is lossless, so two
//! different values never hash alike by construction, only by a SHA-256 collision:
//!
//! - every field that takes part in a value's equality is written, at full width: integers as
//!   little-endian two's complement, doubles as their IEEE 754 bit patterns (so −0 and +0, and
//!   every NaN payload, are distinct), wide integers limb by limb;
//! - each type has a fixed layout, and the variable parts (`Option`, `Result`, enums) start with a
//!   tag byte, so a sequence of values of one type splits back into its values in exactly one way.
//!
//! Guarantees: two values of one type have the same bytes only if they are equal field for field
//! (bit for bit, for doubles); a sequence's bytes split back into its values one way only; and the
//! bytes are the same on every target and in every build profile, since every width and byte
//! order is fixed and nothing depends on memory layout, padding or addresses (the tests check
//! the documented layout).
//!
//! Method: the canonical-encoding rule of chapter 28.13 (hash a fixed, documented byte form of each
//! result, never a lossy summary), with SHA-256 as chapters 21 and 28.13 use it
//! (NIST FIPS 180-4, <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>).
//!
//! Cost: linear in the bytes encoded, from one byte (a sign) to a few hundred (three planes) per
//! value, each fed to SHA-256 once; nothing is allocated.

use core::cmp::Ordering;

use geom_num::grid::{Coord, GridStep, P2, P3, Rounded, Steps};
use geom_num::interval::Interval;
use geom_num::plane::{Plane, Tpi};
use geom_num::real::Real;
use geom_num::wide::Wide;
use geom_num::{Exactness, NumError, Sign};

use crate::sha256::{Digest, Sha256};

/// A value with a canonical byte encoding (see the module documentation).
pub trait Canonical {
    /// Feeds the value's canonical bytes to `h`.
    fn encode(&self, h: &mut Sha256);
}

/// The SHA-256 of the canonical encodings of `items`, in order.
#[must_use]
pub fn digest<T: Canonical>(items: &[T]) -> Digest {
    let mut h = Sha256::new();
    for item in items {
        item.encode(&mut h);
    }
    h.finish()
}

impl Canonical for bool {
    fn encode(&self, h: &mut Sha256) {
        h.update(&[u8::from(*self)]);
    }
}

impl Canonical for i32 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for i64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for u64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for i128 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_le_bytes());
    }
}

impl Canonical for f64 {
    fn encode(&self, h: &mut Sha256) {
        h.update(&self.to_bits().to_le_bytes());
    }
}

impl Canonical for Ordering {
    fn encode(&self, h: &mut Sha256) {
        let tag: u8 = match self {
            Ordering::Less => 0,
            Ordering::Equal => 1,
            Ordering::Greater => 2,
        };
        h.update(&[tag]);
    }
}

impl Canonical for Sign {
    fn encode(&self, h: &mut Sha256) {
        let tag: u8 = match self {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        };
        h.update(&[tag]);
    }
}

impl Canonical for Coord {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl Canonical for P2 {
    fn encode(&self, h: &mut Sha256) {
        self.x.encode(h);
        self.y.encode(h);
    }
}

impl Canonical for P3 {
    fn encode(&self, h: &mut Sha256) {
        self.x.encode(h);
        self.y.encode(h);
        self.z.encode(h);
    }
}

impl Canonical for GridStep {
    fn encode(&self, h: &mut Sha256) {
        // The rung is 0, 1 or 2, one per step.
        h.update(&self.rung().to_le_bytes());
    }
}

impl Canonical for Steps {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl<const L: usize> Canonical for Wide<L> {
    fn encode(&self, h: &mut Sha256) {
        for limb in self.to_le_limbs() {
            limb.encode(h);
        }
    }
}

impl Canonical for Plane {
    fn encode(&self, h: &mut Sha256) {
        for n in self.n() {
            n.encode(h);
        }
        self.d().encode(h);
    }
}

impl Canonical for Tpi {
    // A meeting point is exactly its three planes, in order (its equality is theirs).
    fn encode(&self, h: &mut Sha256) {
        for plane in self.planes() {
            plane.encode(h);
        }
    }
}

impl Canonical for Interval {
    fn encode(&self, h: &mut Sha256) {
        self.lo().encode(h);
        self.hi().encode(h);
    }
}

impl Canonical for Real {
    fn encode(&self, h: &mut Sha256) {
        self.get().encode(h);
    }
}

impl Canonical for Exactness {
    // A tag, then the bound and the grid step it counts (its equality is theirs).
    fn encode(&self, h: &mut Sha256) {
        match self {
            Exactness::Exact => h.update(&[0]),
            Exactness::Certified { bound, step } => {
                h.update(&[1]);
                bound.encode(h);
                step.encode(h);
            }
            Exactness::Approximate { deviation, step } => {
                h.update(&[2]);
                deviation.encode(h);
                step.encode(h);
            }
        }
    }
}

impl Canonical for NumError {
    fn encode(&self, h: &mut Sha256) {
        match self {
            NumError::OutOfRange { value } => {
                h.update(&[0]);
                value.encode(h);
            }
            NumError::Degenerate => h.update(&[1]),
            NumError::NotFinite => h.update(&[2]),
            NumError::WrongDirection => h.update(&[3]),
            NumError::DivisorContainsZero => h.update(&[4]),
            NumError::NegativeSqrt => h.update(&[5]),
            NumError::EmptyInterval => h.update(&[6]),
            NumError::OutsideDomain => h.update(&[7]),
            NumError::BudgetExceeded => h.update(&[8]),
            NumError::ToleranceExceeded => h.update(&[9]),
            NumError::UnprovenMaths => h.update(&[10]),
        }
    }
}

impl<T: Canonical + Copy> Canonical for Rounded<T> {
    // The value, then whether it is exact (the fields are private to geom-num; these are its
    // getters).
    fn encode(&self, h: &mut Sha256) {
        self.value().encode(h);
        self.is_exact().encode(h);
    }
}

impl<T: Canonical> Canonical for Option<T> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            None => h.update(&[0]),
            Some(v) => {
                h.update(&[1]);
                v.encode(h);
            }
        }
    }
}

impl<T: Canonical> Canonical for Result<T, NumError> {
    fn encode(&self, h: &mut Sha256) {
        match self {
            Ok(v) => {
                h.update(&[0]);
                v.encode(h);
            }
            Err(e) => {
                h.update(&[1]);
                e.encode(h);
            }
        }
    }
}

impl<T: Canonical, const N: usize> Canonical for [T; N] {
    fn encode(&self, h: &mut Sha256) {
        for item in self {
            item.encode(h);
        }
    }
}

impl<A: Canonical, B: Canonical> Canonical for (A, B) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
    }
}

impl<A: Canonical, B: Canonical, C: Canonical> Canonical for (A, B, C) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
        self.2.encode(h);
    }
}

impl<A: Canonical, B: Canonical, C: Canonical, D: Canonical> Canonical for (A, B, C, D) {
    fn encode(&self, h: &mut Sha256) {
        self.0.encode(h);
        self.1.encode(h);
        self.2.encode(h);
        self.3.encode(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom_num::grid;

    fn bytes<T: Canonical>(v: &T) -> Digest {
        digest(core::slice::from_ref(v))
    }

    #[test]
    fn values_that_differ_anywhere_encode_differently() {
        // The weaknesses of a folded 64-bit summary: bit 63 cancelling, words that alias across
        // fields, errors that alias results. None survives a lossless encoding.
        assert_ne!(bytes(&2.0f64), bytes(&-2.0f64));
        assert_ne!(bytes(&0.0f64), bytes(&-0.0f64));
        let w = |v: i128| Wide::<4>::from_i128(v);
        assert_ne!(bytes(&w(1 << 63)), bytes(&w(1 << 64)));
        let q = |x, y, z| P3::new(x, y, z).unwrap();
        assert_ne!(bytes(&q(65_536, 0, 0)), bytes(&q(0, 1, 0)));
        let ok: Result<Coord, NumError> = Ok(Coord::new(0x0040_0000).unwrap());
        let err: Result<Coord, NumError> = Err(NumError::WrongDirection);
        assert_ne!(bytes(&ok), bytes(&err));
        // One step of Q0 from 25 steps of Q1 (exact) and from 26 (rounded): the same value, and
        // only the flag differs.
        let coarse = |v| grid::coarsen(Coord::new(v).unwrap(), GridStep::Q1, GridStep::Q0).unwrap();
        let (exact, rounded) = (coarse(25), coarse(26));
        assert_eq!(exact.value(), rounded.value());
        assert!(exact.is_exact() && !rounded.is_exact());
        assert_ne!(bytes(&exact), bytes(&rounded));
        assert_ne!(bytes(&Some(0i64)), bytes(&None::<i64>));
        let (q0, q1) = (GridStep::Q0, GridStep::Q1);
        assert_ne!(
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q0
            }),
            bytes(&Exactness::Approximate {
                deviation: Steps(3),
                step: q0
            })
        );
        // A bound is a count and its step: 3 steps of Q0 are not 3 steps of Q1.
        assert_ne!(
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q0
            }),
            bytes(&Exactness::Certified {
                bound: Steps(3),
                step: q1
            })
        );
        // Every error has its own tag.
        let errors = [
            NumError::OutOfRange { value: 0 },
            NumError::Degenerate,
            NumError::NotFinite,
            NumError::WrongDirection,
            NumError::DivisorContainsZero,
            NumError::NegativeSqrt,
            NumError::EmptyInterval,
            NumError::OutsideDomain,
            NumError::BudgetExceeded,
            NumError::ToleranceExceeded,
            NumError::UnprovenMaths,
        ];
        let tags: std::collections::BTreeSet<Digest> = errors.iter().map(bytes).collect();
        assert_eq!(tags.len(), errors.len());
    }

    #[test]
    fn sequences_split_one_way() {
        // [Some(x), None] and [None, Some(x)] have the same bytes in a different order, never the
        // same bytes.
        let a = [Some(5i64), None];
        let b = [None, Some(5i64)];
        assert_ne!(digest(&a), digest(&b));
        // Two flips of the same bit in different results no longer cancel.
        assert_ne!(digest(&[1.5f64, 2.5]), digest(&[-1.5f64, -2.5]));
    }

    #[test]
    fn the_layout_is_the_documented_one() {
        // A coordinate is its i64, little-endian: the encoding is part of the golden digests.
        let mut h = Sha256::new();
        Coord::new(-2).unwrap().encode(&mut h);
        assert_eq!(h.finish(), Sha256::digest(&(-2i64).to_le_bytes()));
        let mut h = Sha256::new();
        Err::<Coord, NumError>(NumError::OutOfRange { value: 7 }).encode(&mut h);
        let mut expected = vec![1u8, 0];
        expected.extend_from_slice(&7i128.to_le_bytes());
        assert_eq!(h.finish(), Sha256::digest(&expected));
    }
}
