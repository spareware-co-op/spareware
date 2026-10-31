//! Attacks on G102's schemas (22, 26.1): every core query is listed, in the Query family, with a
//! charge schedule and a cost the tests can check; each schema's errors are the kernel's, each with
//! its base kind; and each example says what the query really gives.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use engine_geometry::operations::{
    BoxInput, Level, OPERATIONS, bounding_box, exactness, faces, make_box, six_volume, twice_area,
    validity,
};
use engine_geometry::schema::{Class, Family, OpSchema, Tier};
use engine_geometry::value::error::ErrorKind;
use engine_geometry::value::grid::{P3, Steps};
use engine_geometry::value::op::{Budget, OpContext, OpId, OpPath};
use geom_op::detail::QueryDetail;
use geom_run::WorkMeter;
use geom_topo::query;

/// The queries, by name.
const QUERIES: [&str; 6] = [
    "query.area",
    "query.bounding_box",
    "query.exactness",
    "query.faces",
    "query.validity",
    "query.volume",
];

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([9; 16])), Budget::UNLIMITED)
}

fn schema(name: &str) -> &'static OpSchema {
    OPERATIONS
        .iter()
        .find(|op| op.schema.name == name)
        .unwrap_or_else(|| panic!("{name} is not listed"))
        .schema
}

#[test]
fn every_query_is_listed_with_its_cost() {
    for name in QUERIES {
        let s = schema(name);
        assert_eq!(s.family, Family::Query, "{name}");
        assert_eq!(s.version, 1, "{name}");
        assert_eq!(
            s.classes,
            [Class::Exact, Class::Certified, Class::Approximate],
            "{name}"
        );
        assert!(!s.charges.is_empty(), "{name}");
        assert!(s.charges.iter().all(|c| c.units == 1), "{name}");
        assert!(s.charges.iter().all(|c| c.phase.name() == "query"), "{name}");
        assert!(s.example.starts_with(name), "{name}");
        assert!(!s.batch, "{name}");
        assert!(
            s.errors.iter().any(|e| e.kind == ErrorKind::BudgetExceeded),
            "{name}"
        );
    }
    let tier = OPERATIONS
        .iter()
        .filter(|op| op.schema.name.starts_with("query."))
        .map(|op| op.tier)
        .collect::<Vec<_>>();
    assert_eq!(tier.len(), QUERIES.len());
    assert!(tier.iter().all(|t| matches!(t, Tier::Preview { .. })));
}

#[test]
fn each_listed_detail_has_the_kind_the_kernel_gives_it() {
    let details = [
        ("Malformed", QueryDetail::Malformed),
        ("CurvedEdge", QueryDetail::CurvedEdge { edge: 0 }),
        ("CurvedFace", QueryDetail::CurvedFace { face: 0 }),
        ("NoLoops", QueryDetail::NoLoops { face: 0 }),
        ("NotSolid", QueryDetail::NotSolid),
        ("Overflow", QueryDetail::Overflow),
    ];
    let mut seen = 0;
    for name in QUERIES {
        for spec in schema(name).errors {
            if let Some((_, d)) = details.iter().find(|(n, _)| *n == spec.detail) {
                assert_eq!(spec.kind, d.kind(), "{name}: {}", spec.detail);
                seen += 1;
            }
        }
    }
    assert!(seen >= details.len());
    // Each detail is listed by the query that can give it.
    let lists = |name: &str, detail: &str| schema(name).errors.iter().any(|e| e.detail == detail);
    assert!(lists("query.volume", "NotSolid"));
    assert!(lists("query.volume", "CurvedEdge"));
    assert!(lists("query.area", "CurvedFace"));
    assert!(lists("query.area", "NoLoops"));
    assert!(!lists("query.area", "NotSolid"));
    assert!(lists("query.bounding_box", "Malformed"));
}

#[test]
fn each_example_says_what_the_query_gives() {
    let input = BoxInput {
        min: P3::new(0, 0, 0).unwrap(),
        size: [Steps(10), Steps(20), Steps(30)],
    };
    let made = make_box(&ctx(), &input).unwrap();
    let (body, record) = (&made.result.body, made.exactness);
    assert!(schema("query.volume").example.contains("36000"));
    let volume = six_volume(&ctx(), body, record).unwrap().result;
    assert_eq!(volume, geom_num::I256::from_i64(36000));
    let listed = faces(&ctx(), body, &[&made.result.history], record)
        .unwrap()
        .result;
    let min_z = listed
        .iter()
        .find(|f| f.roles.iter().any(|r| r.name() == "min.z"))
        .unwrap();
    assert_eq!(
        twice_area(&ctx(), body, min_z.row.face, record)
            .unwrap()
            .result,
        [0, 0, -400]
    );
    let boxed = bounding_box(&ctx(), body, record).unwrap().result.unwrap();
    assert!(boxed.tight);
    assert_eq!(boxed.bound.max(), P3::new(10, 20, 30).unwrap());
    let report = validity(&ctx(), body, record, Level::Full).unwrap().result;
    assert!(report.passed());
    assert_eq!(report.ran().count(), 4);
    assert_eq!(report.deferred(), 0);
    let e = exactness(&ctx(), body, record).unwrap().result;
    assert_eq!((e.flat_faces, e.straight_edges), (6, 12));
}

#[test]
fn the_cost_stated_is_the_cost_charged() {
    // One unit for each thing the schema's charges count, on the box: 6 faces, 6 loops, 24 fins,
    // 8 vertices.
    let input = BoxInput {
        min: P3::new(1, 2, 3).unwrap(),
        size: [Steps(1), Steps(1), Steps(1)],
    };
    let made = make_box(&ctx(), &input).unwrap();
    let body = &made.result.body;
    let mut m = WorkMeter::new(Budget::UNLIMITED);
    query::volume(body, &mut m).unwrap();
    assert_eq!(m.used().work, 6 + 6 + 24);
    let mut m = WorkMeter::new(Budget::UNLIMITED);
    query::bounding_box(body, &mut m).unwrap();
    assert_eq!(m.used().work, 8 + 6 + 24);
    let mut m = WorkMeter::new(Budget::UNLIMITED);
    query::face_area(body, geom_topo::Ix::new(0).unwrap(), &mut m).unwrap();
    assert_eq!(m.used().work, 1 + 4);
}
