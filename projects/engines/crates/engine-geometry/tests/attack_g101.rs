//! Attacks on G101's schemas (issue #436): the facade's operation list says what the operations
//! really do. A schema is what the MCP tools and the documentation are generated from, so a schema
//! that drifts from the kernel misleads every program and every AI tool reading it.
//!
//! Each test tries to break one claim: the list holds the eleven operations, each reached through
//! the facade; every error each operation gives is listed in its schema, under the base kind the
//! kernel gives it; every listed `MakeDetail` has that base kind; every role a primitive names is
//! listed and every listed role is used; every class returned is listed; every input a refusal
//! names is an input of the schema; and a pattern declares its copies' role.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeSet;

use engine_geometry::operations::{
    BoxInput, Instance, Made, Moved, OPERATIONS, PER_ROW, PrismInput, RoundInput, SphereInput,
    TorusInput, cone, cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere,
    torus,
};
use engine_geometry::schema::{Class, Family, OpSchema, Tier};
use engine_geometry::value::Exactness;
use engine_geometry::value::error::{ErrorKind, MakeDetail, OpError, OpErrorDetail};
use engine_geometry::value::grid::{GridStep, P3, Steps};
use engine_geometry::value::name::Origin;
use engine_geometry::value::op::{Budget, OpContext, OpId, OpOutput, OpPath};
use engine_geometry::value::transform::{Axis, GridTransform, Rotation, Scale, TypedTransform};

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

/// A context whose budget stops any operation at once.
fn broke() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::new(1, 1))
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn schema(name: &str) -> &'static OpSchema {
    OPERATIONS
        .iter()
        .find(|op| op.schema.name == name)
        .unwrap_or_else(|| panic!("{name} is not listed"))
        .schema
}

/// The detail's name, as a schema lists it: the variant's, under `Make`, `Num` and `Run`.
fn detail_name(e: &OpError) -> String {
    let text = format!("{:?}", e.detail());
    let inner = text
        .strip_prefix("Make(")
        .or_else(|| text.strip_prefix("Num("))
        .or_else(|| text.strip_prefix("Run("))
        .unwrap_or(&text);
    inner
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect()
}

/// Fails unless `schema` lists `e`, under its kind.
fn listed(name: &str, e: &OpError) {
    let detail = detail_name(e);
    let spec = schema(name)
        .errors
        .iter()
        .find(|s| s.detail == detail)
        .unwrap_or_else(|| panic!("{name} does not list {detail}: {e:?}"));
    assert_eq!(spec.kind, e.kind(), "{name}: {detail}");
    if let OpErrorDetail::Make(MakeDetail::ZeroLength { input }) = e.detail() {
        let field = input.split('.').next().unwrap();
        assert!(
            schema(name).inputs.iter().any(|f| f.name == field),
            "{name}: {input}"
        );
    }
}

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

fn a_box() -> OpOutput<Made> {
    make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(10), Steps(20), Steps(30)],
        },
    )
    .unwrap()
}

fn a_cylinder() -> OpOutput<Made> {
    cylinder(
        &ctx(),
        &RoundInput {
            base: p(0, 0, 0),
            radius: Steps(5),
            height: Steps(10),
            step: GridStep::Q0,
        },
    )
    .unwrap()
}

/// Fails unless the classes `out` can come in are listed, and the roles it names are `schema`'s.
fn made(name: &str, out: &OpOutput<Made>) {
    let spec = schema(name);
    assert!(spec.classes.contains(&Class::of(out.exactness)), "{name}");
    let named: BTreeSet<String> = out
        .result
        .history
        .records()
        .iter()
        .map(|r| match &r.origin {
            Origin::Generated { role, .. } => role.name().to_string(),
            other => panic!("{name}: {other:?}"),
        })
        .collect();
    let listed: BTreeSet<String> = spec.roles.iter().map(|r| r.name.to_string()).collect();
    assert_eq!(named, listed, "{name}");
}

#[test]
fn the_list_holds_every_make_and_transform_operation() {
    let ours = || {
        OPERATIONS
            .iter()
            .filter(|op| !op.schema.name.starts_with("query."))
    };
    let names: Vec<&str> = ours().map(|op| op.schema.name).collect();
    assert_eq!(
        names,
        [
            "make.box",
            "make.cone",
            "make.cylinder",
            "make.prism",
            "make.sphere",
            "make.torus",
            "transform.mirror",
            "transform.move",
            "transform.pattern",
            "transform.rotate",
            "transform.scale",
        ]
    );
    for op in ours() {
        let s = op.schema;
        let family = if s.name.starts_with("make.") {
            Family::Make
        } else {
            Family::Transform
        };
        assert_eq!(s.family, family, "{}", s.name);
        assert_eq!(s.version, 1, "{}", s.name);
        assert_eq!(s.charges, [PER_ROW], "{}", s.name);
        assert_eq!(
            op.tier,
            Tier::Preview {
                since: "2026-10-31"
            },
            "{}",
            s.name
        );
        assert!(!s.batch, "{}", s.name);
        assert!(s.example.starts_with(s.name), "{}", s.name);
    }
}

#[test]
fn every_primitive_names_exactly_the_roles_its_schema_lists() {
    let c = ctx();
    made("make.box", &a_box());
    let round = RoundInput {
        base: p(0, 0, 0),
        radius: Steps(6),
        height: Steps(12),
        step: GridStep::Q0,
    };
    made("make.cylinder", &cylinder(&c, &round).unwrap());
    made("make.cone", &cone(&c, &round).unwrap());
    let base = vec![p(0, 0, 0), p(10, 0, 0), p(12, 7, 0), p(3, 11, 0)];
    let input = PrismInput {
        base,
        height: Steps(9),
    };
    made("make.prism", &prism(&c, &input).unwrap());
    let ball = SphereInput {
        center: p(1, 2, 3),
        radius: Steps(5),
        step: GridStep::Q0,
    };
    made("make.sphere", &sphere(&c, &ball).unwrap());
    let ring = TorusInput {
        center: p(0, 0, 0),
        major: Steps(10),
        minor: Steps(3),
        step: GridStep::Q0,
    };
    made("make.torus", &torus(&c, &ring).unwrap());
}

#[test]
fn every_refusal_of_a_primitive_is_listed() {
    let c = ctx();
    let far = 1_099_511_627_775;
    let cube = |size: [u64; 3], at: P3| BoxInput {
        min: at,
        size: size.map(Steps),
    };
    listed(
        "make.box",
        &make_box(&c, &cube([0, 1, 1], p(0, 0, 0))).unwrap_err(),
    );
    listed(
        "make.box",
        &make_box(&c, &cube([1, 1, 1], p(far, 0, 0))).unwrap_err(),
    );
    listed(
        "make.box",
        &make_box(&broke(), &cube([1, 1, 1], p(0, 0, 0))).unwrap_err(),
    );
    let round = |radius: u64, height: u64, base: P3| RoundInput {
        base,
        radius: Steps(radius),
        height: Steps(height),
        step: GridStep::Q0,
    };
    for name in ["make.cylinder", "make.cone"] {
        let make = |c: &OpContext<'_>, input: &RoundInput| {
            if name == "make.cone" {
                cone(c, input)
            } else {
                cylinder(c, input)
            }
        };
        listed(name, &make(&c, &round(0, 1, p(0, 0, 0))).unwrap_err());
        listed(name, &make(&c, &round(1, 0, p(0, 0, 0))).unwrap_err());
        listed(name, &make(&c, &round(1, 1, p(far, 0, 0))).unwrap_err());
        listed(name, &make(&broke(), &round(1, 1, p(0, 0, 0))).unwrap_err());
    }
    let prism_of = |base: Vec<P3>, height: u64| PrismInput {
        base,
        height: Steps(height),
    };
    for input in [
        prism_of(vec![p(0, 0, 0), p(1, 0, 0)], 1),
        prism_of(vec![p(0, 0, 0), p(9, 0, 0), p(0, 9, 1)], 1),
        prism_of(vec![p(0, 0, 0), p(0, 9, 0), p(9, 0, 0)], 1),
        prism_of(vec![p(0, 0, 0), p(9, 0, 0), p(0, 9, 0)], 0),
        prism_of(vec![p(0, 0, far), p(9, 0, far), p(0, 9, far)], 1),
    ] {
        listed("make.prism", &prism(&c, &input).unwrap_err());
    }
    let ball = |radius: u64, center: P3| SphereInput {
        center,
        radius: Steps(radius),
        step: GridStep::Q0,
    };
    listed(
        "make.sphere",
        &sphere(&c, &ball(0, p(0, 0, 0))).unwrap_err(),
    );
    listed(
        "make.sphere",
        &sphere(&c, &ball(1, p(far, 0, 0))).unwrap_err(),
    );
    let ring = |major: u64, minor: u64, center: P3| TorusInput {
        center,
        major: Steps(major),
        minor: Steps(minor),
        step: GridStep::Q0,
    };
    for input in [
        ring(0, 1, p(0, 0, 0)),
        ring(1, 0, p(0, 0, 0)),
        ring(3, 3, p(0, 0, 0)),
        ring(3, 1, p(far, 0, 0)),
    ] {
        listed("make.torus", &torus(&c, &input).unwrap_err());
    }
}

#[test]
fn every_refusal_of_a_transform_is_listed() {
    let c = ctx();
    let b = a_box();
    let exact = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let cyl = a_cylinder();
    let round = Moved {
        body: &cyl.result.body,
        exactness: cyl.exactness,
        accept: Some(certified(64)),
    };
    let far = 1 << 41;
    listed(
        "transform.move",
        &move_by(&c, exact, [far, 0, 0]).unwrap_err(),
    );
    listed(
        "transform.move",
        &move_by(&broke(), exact, [1, 0, 0]).unwrap_err(),
    );
    let turn = Rotation::direction(4, 3).unwrap();
    listed(
        "transform.rotate",
        &rotate(&c, exact, Axis::Z, turn, p(0, 0, 0)).unwrap_err(),
    );
    listed(
        "transform.rotate",
        &rotate(&c, round, Axis::Y, turn, p(0, 0, 0)).unwrap_err(),
    );
    let two = Scale::new(2, 1).unwrap();
    listed(
        "transform.scale",
        &scale(&c, round, [two, Scale::ONE, Scale::ONE], p(0, 0, 0)).unwrap_err(),
    );
    listed(
        "transform.mirror",
        &mirror(&broke(), exact, Axis::X, p(0, 0, 0)).unwrap_err(),
    );
    let at = |k: u8, t: GridTransform| Instance {
        key: [k; 16],
        transform: TypedTransform::Exact(t),
    };
    let right = GridTransform::translation([20, 0, 0]).unwrap();
    listed("transform.pattern", &pattern(&c, exact, &[]).unwrap_err());
    listed(
        "transform.pattern",
        &pattern(&c, exact, &[at(1, right), at(1, right)]).unwrap_err(),
    );
}

#[test]
fn every_listed_make_detail_has_its_kernel_kind() {
    let details = [
        MakeDetail::ZeroLength { input: "radius" },
        MakeDetail::TooFewCorners { corners: 2 },
        MakeDetail::BaseNotLevel,
        MakeDetail::BaseNotConvex,
        MakeDetail::MinorNotBelowMajor { major: 3 },
        MakeDetail::NotAccepted {
            would: Exactness::Exact,
        },
        MakeDetail::KindNotCertified,
        MakeDetail::NotCertifiedYet { piece: "G177" },
        MakeDetail::BodyNotNamed,
        MakeDetail::EmptyPattern,
        MakeDetail::RepeatedKey,
        MakeDetail::SingularTransform,
        MakeDetail::ResultInvalid,
        MakeDetail::BuildFailed,
    ];
    let mut used = BTreeSet::new();
    for op in OPERATIONS {
        for spec in op.schema.errors {
            if let Some(d) = details
                .iter()
                .find(|d| format!("{d:?}").starts_with(spec.detail))
            {
                assert_eq!(spec.kind, d.kind(), "{}: {}", op.schema.name, spec.detail);
                used.insert(spec.detail);
            }
        }
    }
    assert_eq!(used.len(), details.len(), "{used:?}");
    for op in OPERATIONS {
        let s = op.schema;
        assert_eq!(
            s.errors
                .iter()
                .map(|e| e.detail)
                .collect::<BTreeSet<_>>()
                .len(),
            s.errors.len(),
            "{}",
            s.name
        );
    }
}

#[test]
fn a_pattern_declares_its_copies() {
    let roles: Vec<&str> = schema("transform.pattern")
        .roles
        .iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(roles, ["copy"]);
    let b = a_box();
    let exact = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::Exact(GridTransform::translation([20, 0, 0]).unwrap()),
    };
    let out = pattern(&ctx(), exact, &[at]).unwrap();
    for (_, copy) in &out.result {
        for record in copy.history.records() {
            let Origin::Generated { role, .. } = &record.origin else {
                panic!("{record:?}");
            };
            assert_eq!(role.name(), "copy");
        }
    }
    assert_eq!(out.exactness, Exactness::Exact);
}

#[test]
fn the_errors_are_named_for_kinds_programs_can_read() {
    // A transform's refusal of an accepted class is the one error whose remedy is a class.
    let c = ctx();
    let b = a_box();
    let exact = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let e = rotate(
        &c,
        exact,
        Axis::Z,
        Rotation::direction(4, 3).unwrap(),
        p(0, 0, 0),
    )
    .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::ExactnessNotAccepted);
}
