//! geom-format's recursive cycles at their stated bounds (chapter 28.8, `kinds`' Recursion field):
//! a surface `MAX_NESTING` offsets deep writes and reads back, and one nested past the decoder's
//! depth is refused before reading goes further. The recursion gate runs these on a small stack.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_format::cbor::Decoder;
use geom_format::kinds::{DEPTH, KindError, read_surface, surface, write_surface};
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};
use geom_surface::offset::MAX_NESTING;
use geom_surface::{Offset, Plane, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn plane() -> SurfaceKind {
    let domain = ((0.0, 2.0), (-1.0, 1.0));
    SurfaceKind::Plane(Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], domain).unwrap())
}

fn bytes(s: &SurfaceKind) -> Vec<u8> {
    let mut e = Encoder::new();
    write_surface(&mut e, s).unwrap();
    e.finish().unwrap().into_bytes()
}

#[test]
fn surfaces_write_and_read_at_their_bound() {
    let mut s = plane();
    for _ in 0..MAX_NESTING {
        s = SurfaceKind::Offset(Offset::new(s, 0.25).unwrap());
    }
    assert!(Offset::new(s.clone(), 0.25).is_err());
    let b = bytes(&s);
    let back = read_surface(&mut Decoder::new(&b, DEPTH), &mut meter()).unwrap();
    assert_eq!(back, s);
    assert_eq!(bytes(&back), b);

    // Offsets nested past the decoder's depth: refused by the decoder, not read further.
    let levels = DEPTH + 2;
    let mut e = Encoder::new();
    for _ in 0..levels {
        e.array(5).unwrap();
        e.uint(surface::OFFSET).unwrap();
    }
    write_surface(&mut e, &plane()).unwrap();
    for _ in 0..levels {
        e.float(0.25).unwrap();
        e.array(2).unwrap();
        for (lo, hi) in [(0.0, 2.0), (-1.0, 1.0)] {
            e.array(2).unwrap();
            e.float(lo).unwrap();
            e.float(hi).unwrap();
        }
        e.uint(0).unwrap();
    }
    let deep = e.finish().unwrap().into_bytes();
    assert!(matches!(
        read_surface(&mut Decoder::new(&deep, DEPTH), &mut meter()),
        Err(KindError::Cbor(_))
    ));
}
