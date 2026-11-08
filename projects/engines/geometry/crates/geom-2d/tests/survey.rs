//! Piece G361's survey points (chapter 17.2): a dataset imported as one Acorn body, each point a
//! vertex named `Created { op, role: survey.point(key) }`, its ID an attribute, two points with one
//! ID an error naming both.
//!
//! Oracles: the key of a first import against SHA-256 over the prefix and the ID's CBOR text
//! string, both written out byte by byte here; every name against `geom-op`'s `TopoName::id` of the
//! name chapter 17.2 gives; the body's vertices against the records' positions, found by name; and
//! the name graph's parents of a point's name, which must be none, the key not being a name.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

use geom_2d::survey::{
    IdError, Imported, KEY_PREFIX, PointKey, Record, SURVEY_ID_CLASS, SurveyError, import,
    point_name, survey_id,
};
use geom_num::P3;
use geom_op::hash::sha256_128;
use geom_op::{NameId, NameList, OpId, OpPath, Role, TopoName};
use geom_run::{Budget, WorkMeter};
use geom_topo::attributes::{Carried, Value};
use geom_topo::order::NameKey;
use geom_topo::{BodyKind, Ix, RegionKind, TopoError};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn op(n: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([n; 16]))
}

/// The CBOR text string of `id`, written out: major type 3 and the length, then the UTF-8 bytes.
fn cbor_text(id: &str) -> Vec<u8> {
    let n = id.len();
    let mut out = if n < 24 {
        vec![0x60 | n as u8]
    } else if n < 256 {
        vec![0x78, n as u8]
    } else {
        vec![0x79, (n >> 8) as u8, n as u8]
    };
    out.extend_from_slice(id.as_bytes());
    out
}

fn records<'a>(rows: &[(&'a str, P3)]) -> Vec<Record<'a>> {
    rows.iter()
        .map(|&(id, at)| Record::first(id, at).unwrap())
        .collect()
}

/// The point the body's vertex named `name` lies at.
fn at_name(imported: &Imported, name: NameId) -> P3 {
    let names = imported.body.names().unwrap();
    let row = names
        .vertices
        .iter()
        .position(|k| *k == NameKey(name.to_bytes()))
        .unwrap();
    let arenas = imported.body.arenas();
    let point = *arenas
        .vertices
        .point
        .get(Ix::new(row as u32).unwrap())
        .unwrap();
    *arenas.points.get(point).unwrap()
}

/// The ID the attribute holds for the point named `name`.
fn id_of(imported: &Imported, name: NameId) -> Vec<u8> {
    match imported
        .attributes
        .get(&survey_id(), NameKey(name.to_bytes()))
        .unwrap()
    {
        Carried::One(Value(bytes)) => bytes.clone(),
        Carried::Combined(_) => panic!("one value per point"),
    }
}

/// Four points of a site: two at one position (a check shot), and an ID with an accent.
fn site() -> [(&'static str, P3); 4] {
    [
        ("1001", p(1_000_000, 2_000_000, 64_000)),
        ("1002", p(1_064_000, 2_000_000, 64_640)),
        ("CP7", p(-5, 7, 0)),
        ("kerb \u{e9}", p(1_064_000, 2_000_000, 64_640)),
    ]
}

#[test]
fn a_first_imports_key_is_the_hash_of_its_id() {
    for id in [
        "1001",
        "CP7",
        "kerb é",
        "a point with a long ID of thirty",
        "x",
    ] {
        let mut bytes = KEY_PREFIX.to_vec();
        bytes.extend(cbor_text(id));
        assert_eq!(
            PointKey::first_import(id).unwrap(),
            PointKey::from_bytes(sha256_128(&bytes)),
            "{id}"
        );
    }
    assert_eq!(KEY_PREFIX, b"SKG survey point key v1\0");
    let k = PointKey::first_import("1001").unwrap();
    assert_eq!(PointKey::from_bytes(k.to_bytes()), k);
    assert_ne!(k, PointKey::first_import("1001 ").unwrap());
}

#[test]
fn every_point_is_a_named_vertex_of_one_acorn_body() {
    let recs = records(&site());
    let imported = import(&op(3), &recs, &mut meter()).unwrap();
    let body = &imported.body;
    assert_eq!(body.kind(), BodyKind::Acorn);
    let arenas = body.arenas();
    assert_eq!(arenas.vertices.len(), 4);
    assert_eq!(arenas.points.len(), 4);
    assert_eq!(arenas.regions.len(), 1);
    assert_eq!(
        arenas.regions.get(body.outside()).unwrap().kind,
        RegionKind::Void
    );
    assert_eq!(
        arenas.edges.len() + arenas.faces.len() + arenas.shells.len(),
        0
    );
    // The names: the outside region, then each point in record order.
    let outside = TopoName::Created {
        op: op(3),
        role: Role::new("region.outside", []).unwrap(),
        from: NameList::new([]),
    };
    assert_eq!(imported.names[0], outside);
    assert_eq!(imported.names.len(), 5);
    assert_eq!(imported.points.len(), 4);
    for (i, r) in recs.iter().enumerate() {
        let name = point_name(&op(3), r.key).unwrap();
        assert_eq!(imported.names[i + 1], name);
        assert_eq!(imported.points[i], name.id().unwrap());
        let TopoName::Created { role, from, .. } = &name else {
            panic!()
        };
        assert_eq!(role.name(), "survey.point");
        assert!(role.is_keyed());
        assert_eq!(from.names(), []);
        // The key is not a name: a name graph finds no parent.
        assert_eq!(geom_name::graph::parents(&name), []);
        assert_eq!(at_name(&imported, imported.points[i]), r.at);
        assert_eq!(id_of(&imported, imported.points[i]), cbor_text(r.id));
    }
    // The vertices are in name order.
    let names = body.names().unwrap();
    assert!(names.vertices.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(names.regions, [NameKey(outside.id().unwrap().to_bytes())]);
    assert_eq!(
        imported.attributes.class(&survey_id()),
        Some(SURVEY_ID_CLASS)
    );
}

#[test]
fn the_body_does_not_depend_on_the_order_of_the_records() {
    let recs = records(&site());
    let a = import(&op(3), &recs, &mut meter()).unwrap();
    let mut reversed = recs.clone();
    reversed.reverse();
    let b = import(&op(3), &reversed, &mut meter()).unwrap();
    assert_eq!(a.body, b.body);
    assert_eq!(a.attributes, b.attributes);
    let mut back = b.points.clone();
    back.reverse();
    assert_eq!(a.points, back);
}

#[test]
fn names_hold_across_imports_and_renames() {
    let recs = records(&site());
    let first = import(&op(3), &recs, &mut meter()).unwrap();
    // The dataset updated: point 1002 renamed (its key kept), CP7 moved, a new point added.
    let mut update = recs.clone();
    update[1].id = "1002A";
    update[2].at = p(-6, 7, 1);
    update.push(Record::first("1005", p(9, 9, 9)).unwrap());
    let second = import(&op(3), &update, &mut meter()).unwrap();
    assert_eq!(&second.points[..4], &first.points[..]);
    assert_eq!(id_of(&second, second.points[1]), cbor_text("1002A"));
    assert_eq!(at_name(&second, second.points[2]), p(-6, 7, 1));
    // Another import operation names the same points differently.
    let other = import(&op(4), &recs, &mut meter()).unwrap();
    for (x, y) in other.points.iter().zip(&first.points) {
        assert_ne!(x, y);
    }
    // An import step's path takes part in the name too.
    let step = op(3).step("points").unwrap();
    let stepped = import(&step, &recs, &mut meter()).unwrap();
    assert_ne!(stepped.points[0], first.points[0]);
}

#[test]
fn two_points_with_one_id_are_an_error_naming_both() {
    let rows = [
        ("A", p(0, 0, 0)),
        ("B", p(1, 0, 0)),
        ("C", p(2, 0, 0)),
        ("B", p(3, 0, 0)),
        ("A", p(4, 0, 0)),
        ("B", p(5, 0, 0)),
    ];
    let e = import(&op(1), &records(&rows), &mut meter()).unwrap_err();
    // The first ID in byte order, and its two lowest records.
    assert_eq!(
        e,
        SurveyError::DuplicateId {
            id: "A".into(),
            first: 0,
            second: 4
        }
    );
    assert_eq!(e.to_string(), "points 0 and 4 both have the ID \"A\"");
    let e = import(&op(1), &records(&rows[1..]), &mut meter()).unwrap_err();
    assert_eq!(
        e,
        SurveyError::DuplicateId {
            id: "B".into(),
            first: 0,
            second: 2
        }
    );
    // Two IDs under one key would give two points one name.
    let mut recs = records(&rows[..3]);
    recs[2].key = recs[0].key;
    let e = import(&op(1), &recs, &mut meter()).unwrap_err();
    assert_eq!(
        e,
        SurveyError::DuplicateKey {
            key: recs[0].key,
            first: 0,
            second: 2
        }
    );
    assert!(
        e.to_string()
            .starts_with("points 0 and 2 both have the key ")
    );
}

#[test]
fn an_id_must_be_text_a_name_can_hold() {
    assert_eq!(PointKey::first_import(""), Err(IdError::Empty));
    // "é" as e and a combining acute accent is not NFC; as one code point it is.
    assert_eq!(PointKey::first_import("e\u{301}"), Err(IdError::NotNfc));
    assert!(PointKey::first_import("\u{e9}").is_ok());
    let good = Record::first("1", p(0, 0, 0)).unwrap();
    let bad = Record {
        id: "e\u{301}",
        ..good
    };
    assert_eq!(
        import(&op(1), &[good, bad], &mut meter()),
        Err(SurveyError::Id {
            index: 1,
            reason: IdError::NotNfc
        })
    );
    let empty = Record { id: "", ..good };
    assert_eq!(
        import(&op(1), &[empty], &mut meter()),
        Err(SurveyError::Id {
            index: 0,
            reason: IdError::Empty
        })
    );
    for e in [IdError::Empty, IdError::NotNfc, IdError::Encode] {
        assert_ne!(e.to_string(), "");
    }
    assert_ne!(SurveyError::Defect.to_string(), "");
}

#[test]
fn points_may_share_a_position_and_a_dataset_may_be_empty() {
    // Two IDs at one position (a check shot) are two points.
    let recs = records(&site());
    let imported = import(&op(2), &recs, &mut meter()).unwrap();
    assert_eq!(
        at_name(&imported, imported.points[1]),
        at_name(&imported, imported.points[3])
    );
    let empty = import(&op(2), &[], &mut meter()).unwrap();
    assert_eq!(empty.body.kind(), BodyKind::Acorn);
    assert_eq!(empty.body.arenas().vertices.len(), 0);
    assert_eq!(empty.names.len(), 1);
    assert_eq!(empty.points, []);
}

#[test]
fn a_stopped_meter_stops_the_import() {
    let recs = records(&site());
    let mut m = WorkMeter::new(Budget::of_work(3));
    assert!(matches!(
        import(&op(2), &recs, &mut m),
        Err(SurveyError::Body(TopoError::Stopped(_)))
    ));
}

#[test]
fn many_points_import_in_name_order() {
    let ids: Vec<String> = (0..2_000).map(|i| format!("P{i}")).collect();
    let recs: Vec<Record<'_>> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let i = i as i64;
            Record::first(id, p(i * 640, (i * 37) % 1_000 * 640, i)).unwrap()
        })
        .collect();
    let imported = import(&op(9), &recs, &mut meter()).unwrap();
    assert_eq!(imported.body.arenas().vertices.len(), 2_000);
    let names = imported.body.names().unwrap();
    assert!(names.vertices.windows(2).all(|w| w[0] < w[1]));
    for (r, &n) in recs.iter().zip(&imported.points).step_by(97) {
        assert_eq!(at_name(&imported, n), r.at);
    }
}
