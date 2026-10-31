//! The recursion bounds of `geom-topo`'s module headers, run at their bound (chapter 28.8; the
//! recursion check of piece G13): the rows that name each other through `Ix` hold no row of
//! another, so a row of each is cloned, compared, hashed, printed and dropped one level deep.

#![allow(clippy::unwrap_used, reason = "tests: a failure stops the test")]

use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use geom_math::Box3;
use geom_num::grid::P3;
use geom_topo::{Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense, Shell};

fn ix<K>(row: u32) -> Ix<K> {
    Ix::new(row).unwrap()
}

/// FNV-1a, a hasher the recursion check can read.
struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn hash<T: Hash>(t: &T) -> u64 {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    t.hash(&mut h);
    h.finish()
}

/// Clones, compares, hashes, prints and drops one row, and returns its printed form.
fn round<T: Clone + PartialEq + Hash + Debug>(row: &T) -> String {
    let copy = row.clone();
    assert_eq!(copy, *row);
    assert_eq!(hash(&copy), hash(row));
    let text = format!("{copy:?}");
    drop(copy);
    text
}

#[test]
fn rows_clone_compare_hash_and_print_one_level_deep() {
    let region = Region {
        kind: RegionKind::Solid,
        first_shell: Some(ix(0)),
    };
    let shell = Shell {
        region: ix(0),
        next: None,
        first_face: Some(ix(1)),
    };
    let face = Face {
        shell: ix(0),
        next: Some(ix(2)),
        surface: ix(0),
        sense: Sense::Forward,
        first_loop: Some(ix(0)),
        bound: Box3::new(P3::new(0, 0, 0).unwrap(), P3::new(1, 1, 1).unwrap()).unwrap(),
    };
    let loop_ = Loop {
        face: ix(1),
        next: None,
        first_fin: Some(ix(3)),
    };
    let fin = Fin {
        loop_: ix(0),
        next: ix(4),
        prev: ix(2),
        edge: ix(5),
        sense: Sense::Reversed,
        radial: ix(6),
        pcurve: None,
    };
    let edge = Edge {
        first_fin: Some(ix(3)),
        start: Some(ix(0)),
        end: None,
        curve: ix(0),
    };
    let texts = [
        round(&region),
        round(&shell),
        round(&face),
        round(&loop_),
        round(&fin),
        round(&edge),
    ];
    for (text, name) in texts
        .iter()
        .zip(["Region", "Shell", "Face", "Loop", "Fin", "Edge"])
    {
        assert!(text.starts_with(name), "{text}");
    }
}
