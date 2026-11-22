//! Shared by the tests: a fixed-seed number sequence, ids, and the plain scan every answer is
//! compared with. Written against this crate's public types only.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use construct_model::geometry::Bounds;
use construct_spatial::{Entry, Scope};
use kit_core::Id;

/// A fixed-seed sequence (splitmix64): the same numbers on every machine.
pub struct Seq(pub u64);

impl Seq {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A whole number in `low..high`.
    pub fn between(&mut self, low: i64, high: i64) -> i64 {
        low + i64::try_from(self.next() % u64::try_from(high - low).unwrap()).unwrap()
    }
}

pub fn id(n: u64) -> Id {
    Id::parse(&format!("e{n:08}")).unwrap()
}

/// A box with its low corner somewhere in `extent` and sides from 1 mm to `side` mm, a few of
/// them zero thick (a flat element) and a few much longer (a long wall).
pub fn random_box(seq: &mut Seq, extent: [i64; 3], side: i64) -> Bounds {
    let min = [
        seq.between(-extent[0], extent[0]),
        seq.between(-extent[1], extent[1]),
        seq.between(-extent[2], extent[2]),
    ];
    let long = seq.next().is_multiple_of(50);
    let length = |seq: &mut Seq| {
        if seq.next().is_multiple_of(20) {
            0
        } else if long {
            seq.between(1, side * 10)
        } else {
            seq.between(1, side)
        }
    };
    let max = [
        min[0] + length(seq),
        min[1] + length(seq),
        min[2] + length(seq),
    ];
    Bounds { min, max }
}

pub fn random_scope(seq: &mut Seq) -> Option<Scope> {
    match seq.next() % 4 {
        0 => None,
        1 => Some(Scope::Level(0)),
        2 => Some(Scope::Level(1)),
        _ => Some(Scope::Document(id(1_000_000))),
    }
}

/// The oracle: every entry looked at, one by one.
pub fn scan_meeting(all: &[Entry], q: &Bounds, scope: Option<&Scope>) -> Vec<Id> {
    let mut out: Vec<Id> = all
        .iter()
        .filter(|e| scope.is_none_or(|s| e.scope.as_ref() == Some(s)))
        .filter(|e| (0..3).all(|a| e.bounds.min[a] <= q.max[a] && e.bounds.max[a] >= q.min[a]))
        .map(|e| e.id.clone())
        .collect();
    out.sort();
    out
}

pub fn scan_inside(all: &[Entry], q: &Bounds, scope: Option<&Scope>) -> Vec<Id> {
    let mut out: Vec<Id> = all
        .iter()
        .filter(|e| scope.is_none_or(|s| e.scope.as_ref() == Some(s)))
        .filter(|e| (0..3).all(|a| e.bounds.min[a] >= q.min[a] && e.bounds.max[a] <= q.max[a]))
        .map(|e| e.id.clone())
        .collect();
    out.sort();
    out
}

pub fn scan_nearest(all: &[Entry], p: [i64; 3], count: usize, scope: Option<&Scope>) -> Vec<Id> {
    let d2 = |e: &Entry| -> i128 {
        (0..3)
            .map(|a| {
                let gap = (i128::from(e.bounds.min[a]) - i128::from(p[a]))
                    .max(i128::from(p[a]) - i128::from(e.bounds.max[a]))
                    .max(0);
                gap * gap
            })
            .sum()
    };
    let mut ranked: Vec<(i128, Id)> = all
        .iter()
        .filter(|e| scope.is_none_or(|s| e.scope.as_ref() == Some(s)))
        .map(|e| (d2(e), e.id.clone()))
        .collect();
    ranked.sort();
    ranked.truncate(count);
    let mut out: Vec<Id> = ranked.into_iter().map(|(_, id)| id).collect();
    out.sort();
    out
}

pub fn owned(ids: Vec<&Id>) -> Vec<Id> {
    ids.into_iter().cloned().collect()
}
