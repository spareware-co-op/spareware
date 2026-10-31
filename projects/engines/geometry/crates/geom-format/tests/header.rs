//! The header record through its public interface: round trips, the ladder from `geom_num`, and
//! every refusal.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure stops the test"
)]

use geom_format::header::{FORMAT, Header, HeaderError, NAME_SCHEME, PRE_EDITION, UNICODE, rung};
use geom_num::grid::Steps;
use geom_num::ladder;
use geom_op::cbor::Encoder;
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// A header payload written field by field, the format and extensions as given.
fn raw(major: u64, extensions: &[(u64, u64)]) -> Vec<u8> {
    let mut e = Encoder::new();
    e.array(7).unwrap();
    e.array(2).unwrap();
    e.uint(major).unwrap();
    e.uint(3).unwrap();
    e.uint(9).unwrap();
    e.uint(1).unwrap();
    e.map(2).unwrap();
    e.key(1).unwrap();
    e.int(64).unwrap();
    e.key(7).unwrap();
    e.int(-34).unwrap();
    e.array(3).unwrap();
    for p in UNICODE {
        e.uint(p).unwrap();
    }
    e.text("p").unwrap();
    e.map(extensions.len() as u64).unwrap();
    for &(k, v) in extensions {
        e.key(k).unwrap();
        e.uint(v).unwrap();
    }
    e.finish().unwrap().into_bytes()
}

#[test]
fn v1_round_trips_with_the_kernels_ladder() {
    let h = Header::v1("geom-format test", Steps(1_000));
    assert_eq!(h.format, FORMAT);
    assert_eq!(
        (h.edition, h.name_scheme, h.unicode),
        (PRE_EDITION, NAME_SCHEME, UNICODE)
    );
    assert_eq!(h.ladder[&rung::FIT], 64);
    assert_eq!(h.ladder[&rung::PER_OPERATION], 16);
    assert_eq!(h.ladder[&rung::HEAL], 1_000);
    assert_eq!(h.ladder[&rung::HEAL_CAP], 6_400);
    assert_eq!(h.ladder[&rung::SHORT_EDGE_WARNING], 3_200);
    assert_eq!(h.ladder[&rung::COMMIT_PER_AXIS], 1);
    assert_eq!(
        h.ladder[&rung::ANGULAR_LOG2],
        i64::from(ladder::ANGULAR_LOG2)
    );
    let payload = h.payload().unwrap();
    assert_eq!(Header::read(&payload, &mut meter()).unwrap(), h);
    let r = h.record().unwrap();
    assert_eq!(r.payload, payload);
}

#[test]
fn a_newer_minor_and_skippable_extensions_are_read() {
    let h = Header::read(&raw(1, &[(2, 5), (4, 6)]), &mut meter()).unwrap();
    assert_eq!(h.format, (1, 3));
    assert_eq!(h.edition, 9);
    assert_eq!(h.ladder.len(), 2);
    assert_eq!(h.ladder[&7], -34);
    assert_eq!(h.ignored, vec![2, 4]);
    assert_eq!(h.program, "p");
}

#[test]
fn a_header_refuses_what_it_cannot_honour() {
    let mut m = meter();
    assert_eq!(
        Header::read(&raw(2, &[]), &mut m),
        Err(HeaderError::Format { major: 2, minor: 3 })
    );
    assert_eq!(
        Header::read(&raw(1, &[(2, 0), (3, 0)]), &mut m),
        Err(HeaderError::MustUnderstand(3))
    );
    // Not canonical, not a header, bytes after it.
    assert!(matches!(
        Header::read(&[0x18, 0x01], &mut m),
        Err(HeaderError::Cbor(_))
    ));
    assert!(matches!(
        Header::read(&[0x86, 0, 0, 0, 0, 0, 0], &mut m),
        Err(HeaderError::Shape { offset: 0, .. })
    ));
    let mut long = raw(1, &[]);
    long.push(0);
    assert!(matches!(
        Header::read(&long, &mut m),
        Err(HeaderError::Cbor(_))
    ));
    for e in [
        HeaderError::Format { major: 2, minor: 0 },
        HeaderError::MustUnderstand(3),
        HeaderError::Shape {
            offset: 1,
            expected: "x",
        },
    ] {
        assert!(e.to_string().len() > 10);
    }
}
