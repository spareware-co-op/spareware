//! Tests of the operation record (piece G96), through the public interface.
//!
//! What this file proves: an operation written as a record, through the record layer, and read
//! back is the same; a name that is not printable ASCII, parameters that are not canonical CBOR
//! (on writing and on reading) and a second spelling are refused; the payload matches its CDDL.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken case must stop the test loudly.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use geom_format::content::ContentId;
use geom_format::operation::{Operation, OperationError, operation_record, read_operation};
use geom_format::recipe::RecipeInput;
use geom_format::record::Record;
use geom_op::{NameId, OpId, OpPath};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn operation() -> Operation {
    Operation {
        op: OpPath::new(OpId::from_bytes([0x96; 16]))
            .step("cut")
            .unwrap(),
        name: "combine.subtract".to_owned(),
        version: 3,
        inputs: vec![
            RecipeInput::Body(ContentId::of_payload(b"a wall")),
            RecipeInput::Entity(NameId::from_bytes([4; 16])),
        ],
        // [1, "deep"]: one canonical item.
        params: vec![0x82, 0x01, 0x64, b'd', b'e', b'e', b'p'],
        budget: Budget::new(10_000, 1 << 20),
    }
}

#[test]
fn an_operation_round_trips() {
    let operation = operation();
    let record = operation_record(&operation, &mut meter()).unwrap();
    assert_eq!((record.tag, record.flags, record.schema), (20, 1, 1));
    let bytes = record.encode(&mut meter()).unwrap();
    let (back, _) = Record::decode(&bytes, &mut meter()).unwrap();
    assert_eq!(
        read_operation(&back.payload, &mut meter()).unwrap(),
        operation
    );
}

#[test]
fn a_bad_operation_is_refused() {
    for name in ["", "combine subtract", "comb\u{ed}ne"] {
        let mut o = operation();
        o.name = name.to_owned();
        assert_eq!(
            o.encode(&mut meter()).unwrap_err(),
            OperationError::Name,
            "{name:?}"
        );
    }
    // Parameters that are not one canonical item: two items, a long head, nothing.
    for params in [vec![0x01, 0x02], vec![0x18, 0x01], vec![]] {
        let mut o = operation();
        o.params = params.clone();
        assert!(
            matches!(
                o.encode(&mut meter()).unwrap_err(),
                OperationError::Params(_)
            ),
            "{params:?}"
        );
    }
    let payload = operation().encode(&mut meter()).unwrap();
    // On reading too: the parameters' long head `18 01` in place of `01`, lengths adjusted.
    let at = payload
        .windows(7)
        .position(|w| w == [0x82, 0x01, 0x64, b'd', b'e', b'e', b'p'])
        .unwrap();
    let mut long = payload[..at - 1].to_vec();
    long.push(0x48);
    long.extend([0x82, 0x18, 0x01, 0x64, b'd', b'e', b'e', b'p']);
    long.extend(&payload[at + 7..]);
    assert!(matches!(
        read_operation(&long, &mut meter()).unwrap_err(),
        OperationError::Params(_)
    ));
    let mut longer = payload;
    longer.push(0);
    assert!(read_operation(&longer, &mut meter()).is_err());
}

#[test]
fn the_operation_payload_matches_its_cddl() {
    let schema = [
        include_str!("../../../format/cddl/operation.cddl"),
        include_str!("../../../format/cddl/names.cddl"),
        include_str!("../../../format/cddl/record.cddl"),
    ]
    .join("\n");
    let payload = operation().encode(&mut meter()).unwrap();
    cddl_cat::validate_cbor_bytes("operation", &schema, &payload)
        .unwrap_or_else(|e| panic!("{e:?}"));
}
