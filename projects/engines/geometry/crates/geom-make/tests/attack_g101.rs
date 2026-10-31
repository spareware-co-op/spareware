//! Attacks on piece G101's claims: every primitive and every transform passes all four checker
//! groups and the history laws; primitives name their entities by role, never by position;
//! transforms keep every name, stay exact where the transform is and refuse a less exact result
//! the caller did not accept; mirrors keep solids facing outward; patterns name their copies as
//! copies; bad input gets a typed error with its remedy.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_check::{Level, check};
use geom_make::{
    BoxInput, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, GridTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::history::Origin;
use geom_op::{
    Bound, ErrorKind, MakeDetail, NameId, NameList, NameSet, OpContext, OpErrorDetail, OpId,
    OpOutput, OpPath, Remedy, Role, RoleArg, TopoName, Value,
};
use geom_run::{Budget, WorkMeter};
use geom_topo::ix::Arena;
use geom_topo::{Body, Builder, Ix, Sense};
use proptest::prelude::*;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn certified(bound: u64) -> Exactness {
    Exactness::Certified {
        bound: Steps(bound),
        step: GridStep::Q0,
    }
}

/// Asserts `out` passes every checker group at its record, and returns it.
fn valid(name: &str, out: OpOutput<Made>) -> OpOutput<Made> {
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = check(&out.result.body, out.exactness, Level::Full, &mut meter).unwrap();
    assert!(report.passed(), "{name}: {:?}", report.faults());
    assert!(out.result.body.names().is_some(), "{name}");
    out
}

/// Every name a body carries.
fn names(body: &Body) -> Vec<NameId> {
    let n = body.names().unwrap();
    n.regions
        .iter()
        .chain(&n.faces)
        .chain(&n.edges)
        .chain(&n.vertices)
        .map(|k| NameId::from_bytes(k.0))
        .collect()
}

/// The name operation `ctx` gives what it creates by `role`.
fn created(role: &str, args: &[NameId]) -> NameId {
    let mut args = args.to_vec();
    args.sort_unstable();
    TopoName::Created {
        op: ctx().path().clone(),
        role: Role::new(role, args.into_iter().map(RoleArg::Name)).unwrap(),
        from: NameList::new([]),
    }
    .id()
    .unwrap()
}

/// Six times the signed volume of a flat solid, from its loops as published: positive when every
/// loop runs counter-clockwise seen from outside.
fn six_volume(body: &Body) -> i128 {
    let a = body.arenas();
    let mut total = 0i128;
    for l in 0..a.rows(Arena::Loop) {
        let first = a
            .loops
            .first_fin
            .get(Ix::new(l).unwrap())
            .copied()
            .flatten()
            .unwrap();
        let mut corners = Vec::new();
        let mut f = first;
        loop {
            let e = *a.fins.edge.get(f).unwrap();
            let (s, t) = (
                a.edges.start.get(e).copied().flatten().unwrap(),
                a.edges.end.get(e).copied().flatten().unwrap(),
            );
            let start = if *a.fins.sense.get(f).unwrap() == Sense::Forward {
                s
            } else {
                t
            };
            let at = *a.points.get(*a.vertices.point.get(start).unwrap()).unwrap();
            corners.push([at.x.get(), at.y.get(), at.z.get()].map(i128::from));
            f = *a.fins.next.get(f).unwrap();
            if f == first {
                break;
            }
        }
        let det = |[a, b, c]: [[i128; 3]; 3]| {
            a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0])
        };
        for i in 1..corners.len() - 1 {
            total += det([corners[0], corners[i], corners[i + 1]]);
        }
    }
    total
}

fn round(radius: u64, height: u64) -> RoundInput {
    RoundInput {
        base: p(0, 0, 0),
        radius: Steps(radius),
        height: Steps(height),
        step: GridStep::Q0,
    }
}

fn a_box() -> OpOutput<Made> {
    make_box(
        &ctx(),
        &BoxInput {
            min: p(-5, 0, 3),
            size: [Steps(10), Steps(20), Steps(30)],
        },
    )
    .unwrap()
}

fn primitives() -> Vec<(&'static str, OpOutput<Made>)> {
    let c = ctx();
    vec![
        ("box", a_box()),
        (
            "prism",
            prism(
                &c,
                &PrismInput {
                    base: vec![p(0, 0, 0), p(10, 0, 0), p(12, 7, 0), p(3, 11, 0)],
                    height: Steps(9),
                },
            )
            .unwrap(),
        ),
        ("cylinder", cylinder(&c, &round(5, 10)).unwrap()),
        ("cone", cone(&c, &round(6, 12)).unwrap()),
        (
            "sphere",
            sphere(
                &c,
                &SphereInput {
                    center: p(1, 2, 3),
                    radius: Steps(5),
                    step: GridStep::Q0,
                },
            )
            .unwrap(),
        ),
        (
            "torus",
            torus(
                &c,
                &TorusInput {
                    center: p(0, 0, 0),
                    major: Steps(10),
                    minor: Steps(3),
                    step: GridStep::Q0,
                },
            )
            .unwrap(),
        ),
    ]
}

#[test]
fn every_primitive_passes_all_four_groups_and_the_history_laws() {
    for (name, out) in primitives() {
        let out = valid(name, out);
        let made = &out.result;
        made.history.check(&NameSet::new([]).unwrap()).unwrap();
        // Every entity is generated from nothing, by its role.
        assert_eq!(
            made.history.records().len(),
            names(&made.body).len(),
            "{name}"
        );
        assert!(
            made.history.records().iter().all(
                |r| matches!(&r.origin, Origin::Generated { from, .. } if from.names().is_empty())
            ),
            "{name}"
        );
        let expected = if matches!(name, "box" | "prism") {
            Exactness::Exact
        } else {
            certified(1)
        };
        assert_eq!(out.exactness, expected, "{name}");
    }
}

#[test]
fn primitives_name_their_entities_by_role_never_by_position() {
    let faces = |out: &OpOutput<Made>| -> Vec<NameId> {
        let names = out.result.body.names().unwrap();
        let mut f: Vec<NameId> = names
            .faces
            .iter()
            .map(|k| NameId::from_bytes(k.0))
            .collect();
        f.sort_unstable();
        f
    };
    let roles = |rs: &[&str]| -> Vec<NameId> {
        let mut f: Vec<NameId> = rs.iter().map(|r| created(r, &[])).collect();
        f.sort_unstable();
        f
    };
    let all = primitives();
    assert_eq!(
        faces(&all[0].1),
        roles(&["min.x", "max.x", "min.y", "max.y", "min.z", "max.z"])
    );
    assert_eq!(faces(&all[2].1), roles(&["cap.start", "cap.end", "side"]));
    assert_eq!(faces(&all[3].1), roles(&["cap.start", "side"]));
    assert_eq!(faces(&all[4].1), roles(&["surface"]));
    // A box's edge is named by its two faces, a corner by its three.
    let box_names = names(&all[0].1.result.body);
    let (min_x, min_y, min_z) = (
        created("min.x", &[]),
        created("min.y", &[]),
        created("min.z", &[]),
    );
    assert!(box_names.contains(&created("edge", &[min_x, min_y])));
    assert!(box_names.contains(&created("corner", &[min_x, min_y, min_z])));
    // The same shape elsewhere, or another size, has the same names: they come from roles alone.
    let elsewhere = make_box(
        &ctx(),
        &BoxInput {
            min: p(100, -40, 7),
            size: [Steps(1), Steps(2), Steps(3)],
        },
    )
    .unwrap();
    assert_eq!(elsewhere.result.body.names(), all[0].1.result.body.names());
    // Another operation's path gives other names.
    let other = OpContext::new(OpPath::new(OpId::from_bytes([8; 16])), Budget::UNLIMITED);
    let renamed = make_box(
        &other,
        &BoxInput {
            min: p(-5, 0, 3),
            size: [Steps(10), Steps(20), Steps(30)],
        },
    )
    .unwrap();
    assert!(
        names(&renamed.result.body)
            .iter()
            .all(|n| !box_names.contains(n))
    );
}

#[test]
fn flat_solids_face_outward_and_mirrors_keep_them_so() {
    let b = a_box();
    assert_eq!(six_volume(&b.result.body), 6 * 10 * 20 * 30);
    let prism = &primitives()[1].1;
    assert!(six_volume(&prism.result.body) > 0);
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let m = mirror(&ctx(), moved, axis, p(3, -2, 1)).unwrap();
        assert_eq!(m.exactness, Exactness::Exact);
        assert_eq!(six_volume(&m.result.body), 6 * 10 * 20 * 30, "{axis:?}");
    }
    let q = rotate(
        &ctx(),
        moved,
        Axis::X,
        Rotation::quarter_turns(3),
        p(0, 0, 0),
    )
    .unwrap();
    assert_eq!(six_volume(&q.result.body), 6 * 10 * 20 * 30);
}

#[test]
fn transforms_keep_names_and_pass_the_checker() {
    for (name, out) in primitives() {
        let moved = Moved {
            body: &out.result.body,
            exactness: out.exactness,
            accept: Some(certified(64)),
        };
        let c = ctx();
        let results = [
            ("move", move_by(&c, moved, [3, -4, 5])),
            (
                "quarter turn",
                rotate(&c, moved, Axis::Z, Rotation::quarter_turns(1), p(1, 1, 0)),
            ),
            ("mirror", mirror(&c, moved, Axis::X, p(2, 0, 0))),
            (
                "double",
                scale(&c, moved, [Scale::new(2, 1).unwrap(); 3], p(0, 0, 0)),
            ),
        ];
        let inputs = NameSet::new(names(&out.result.body)).unwrap();
        for (how, result) in results {
            let result = valid(&format!("{name} {how}"), result.unwrap());
            assert_eq!(
                result.result.body.names(),
                out.result.body.names(),
                "{name} {how}"
            );
            result.result.history.check(&inputs).unwrap();
            assert!(
                result.result.history.records().iter().all(|r| matches!(
                    &r.origin,
                    Origin::Modified { from } if from.names() == [r.output]
                )),
                "{name} {how}"
            );
        }
    }
}

#[test]
fn general_rotations_are_certified_where_the_checker_can_follow() {
    // A rotation by the direction (4, 3), its cosine 4/5 and its sine 3/5, accepted up to 64
    // steps: the box's corners, multiples of 5 apart, land on the grid; a sphere and a torus are
    // certified on their own; a prism's corners round off its faces' planes, and a cylinder's or
    // cone's caps tilt off the axis' exact data, which the checker follows only from G134 and
    // G177.
    for (name, out) in primitives() {
        for axis in [Axis::Z, Axis::Y] {
            let moved = Moved {
                body: &out.result.body,
                exactness: out.exactness,
                accept: Some(certified(64)),
            };
            let turned = rotate(
                &ctx(),
                moved,
                axis,
                Rotation::direction(4, 3).unwrap(),
                p(0, 0, 0),
            );
            match name {
                "box" | "sphere" | "torus" => {
                    let turned = valid(&format!("{name} {axis:?}"), turned.unwrap());
                    assert!(matches!(turned.exactness, Exactness::Certified { .. }));
                }
                _ => {
                    let e = turned.unwrap_err();
                    let piece = if name == "prism" { "G134" } else { "G177" };
                    assert_eq!(
                        *e.detail(),
                        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }),
                        "{name} {axis:?}"
                    );
                    assert_eq!(e.kind(), ErrorKind::Unsupported);
                    assert_eq!(e.remedies(), [Remedy::AwaitPiece { piece }]);
                }
            }
        }
    }
}

#[test]
fn exact_transforms_keep_an_exact_body_exact() {
    let b = a_box();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let c = ctx();
    for result in [
        move_by(&c, moved, [1, 2, 3]),
        rotate(&c, moved, Axis::Y, Rotation::quarter_turns(2), p(7, 7, 7)),
        mirror(&c, moved, Axis::Z, p(0, 0, 0)),
        scale(&c, moved, [Scale::new(3, 1).unwrap(); 3], p(1, 1, 1)),
        scale(
            &c,
            moved,
            [
                Scale::new(2, 1).unwrap(),
                Scale::ONE,
                Scale::new(5, 1).unwrap(),
            ],
            p(0, 0, 0),
        ),
    ] {
        assert_eq!(valid("exact", result.unwrap()).exactness, Exactness::Exact);
    }
    let tripled = scale(&c, moved, [Scale::new(3, 1).unwrap(); 3], p(0, 0, 0)).unwrap();
    assert_eq!(six_volume(&tripled.result.body), 27 * 6 * 10 * 20 * 30);
}

#[test]
fn a_less_exact_result_must_be_accepted() {
    // A rotation by the direction (4, 3) rounds the box's corners once: Certified, and refused
    // unless the caller accepts the bound it names.
    let b = a_box();
    let mut moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let turn = |m: Moved<'_>| {
        rotate(
            &ctx(),
            m,
            Axis::Z,
            Rotation::direction(4, 3).unwrap(),
            p(0, 0, 0),
        )
    };
    let refused = turn(moved).unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::ExactnessNotAccepted);
    let OpErrorDetail::Make(MakeDetail::NotAccepted { would }) = *refused.detail() else {
        panic!("{refused:?}");
    };
    assert_eq!(refused.remedies(), [Remedy::Accept { class: would }]);
    // Accepting exactly the bound it named is enough; one step less is not.
    let (bound, _) = would.bound().unwrap();
    moved.accept = Some(would);
    let accepted = valid("rotated box", turn(moved).unwrap());
    assert_eq!(accepted.exactness, would);
    moved.accept = Some(certified(bound.get() - 1));
    assert_eq!(
        turn(moved).unwrap_err().kind(),
        ErrorKind::ExactnessNotAccepted
    );
    // A doubled sphere's bound doubles too.
    let ball = &primitives()[4].1;
    let doubled = scale(
        &ctx(),
        Moved {
            body: &ball.result.body,
            exactness: ball.exactness,
            accept: None,
        },
        [Scale::new(2, 1).unwrap(); 3],
        p(0, 0, 0),
    )
    .unwrap_err();
    assert_eq!(
        *doubled.detail(),
        OpErrorDetail::Make(MakeDetail::NotAccepted {
            would: certified(2)
        })
    );
}

#[test]
fn a_scale_that_is_not_uniform_waits_for_the_curved_checker() {
    let cyl = cylinder(&ctx(), &round(5, 10)).unwrap();
    let result = scale(
        &ctx(),
        Moved {
            body: &cyl.result.body,
            exactness: cyl.exactness,
            accept: Some(certified(64)),
        },
        [Scale::new(2, 1).unwrap(), Scale::ONE, Scale::ONE],
        p(0, 0, 0),
    )
    .unwrap_err();
    assert_eq!(result.kind(), ErrorKind::Unsupported);
    assert_eq!(result.remedies(), [Remedy::AwaitPiece { piece: "G177" }]);
}

#[test]
fn patterns_name_each_copy_as_a_copy() {
    let b = a_box();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let at = |k: u8, dx: i64| Instance {
        key: [k; 16],
        transform: TypedTransform::Exact(GridTransform::translation([dx, 0, 0]).unwrap()),
        stretch: (1, 1),
    };
    let out = pattern(&ctx(), moved, &[at(3, 60), at(1, 20), at(2, 40)]).unwrap();
    assert_eq!(out.exactness, Exactness::Exact);
    let keys: Vec<[u8; 16]> = out.result.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, [[1; 16], [2; 16], [3; 16]]);
    let sources = names(&b.result.body);
    let inputs = NameSet::new(sources.clone()).unwrap();
    let mut seen: Vec<NameId> = Vec::new();
    for (key, made) in &out.result {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let report = check(&made.body, Exactness::Exact, Level::Full, &mut meter).unwrap();
        assert!(report.passed());
        made.history.check(&inputs).unwrap();
        for record in made.history.records() {
            let Origin::Generated { from, role } = &record.origin else {
                panic!("{record:?}");
            };
            assert_eq!(from.names().len(), 1);
            let source = from.names()[0];
            assert!(sources.contains(&source));
            assert_eq!(role, &Role::new("copy", [RoleArg::Name(source)]).unwrap());
            let expected = TopoName::Created {
                op: ctx().path().item(*key),
                role: role.clone(),
                from: NameList::new([source]),
            };
            assert_eq!(record.output, expected.id().unwrap());
        }
        let these = names(&made.body);
        assert!(
            these
                .iter()
                .all(|n| !sources.contains(n) && !seen.contains(n))
        );
        seen.extend(these);
    }
    assert_eq!(
        pattern(&ctx(), moved, &[]).unwrap_err().detail(),
        &OpErrorDetail::Make(MakeDetail::EmptyPattern)
    );
    assert_eq!(
        pattern(&ctx(), moved, &[at(1, 20), at(1, 40)])
            .unwrap_err()
            .detail(),
        &OpErrorDetail::Make(MakeDetail::RepeatedKey)
    );
}

#[test]
fn bad_input_is_refused_with_a_typed_error_and_its_remedy() {
    let c = ctx();
    let zero = make_box(
        &c,
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(1), Steps(0), Steps(1)],
        },
    )
    .unwrap_err();
    assert_eq!(zero.kind(), ErrorKind::Degenerate);
    assert_eq!(
        zero.remedies(),
        [Remedy::ChangeInput {
            input: "size.y",
            to: Some((Bound::AtLeast, Value::Count(1)))
        }]
    );
    for (base, expected) in [
        (
            vec![p(0, 0, 0), p(1, 0, 0)],
            MakeDetail::TooFewCorners { corners: 2 },
        ),
        (
            vec![p(0, 0, 0), p(9, 0, 0), p(0, 9, 1)],
            MakeDetail::BaseNotLevel,
        ),
        // Clockwise, a dent, and a star that winds twice.
        (
            vec![p(0, 0, 0), p(0, 9, 0), p(9, 0, 0)],
            MakeDetail::BaseNotConvex,
        ),
        (
            vec![
                p(0, 0, 0),
                p(10, 0, 0),
                p(5, 2, 0),
                p(10, 10, 0),
                p(0, 10, 0),
            ],
            MakeDetail::BaseNotConvex,
        ),
        (
            vec![
                p(0, -10, 0),
                p(6, 8, 0),
                p(-9, -3, 0),
                p(9, -3, 0),
                p(-6, 8, 0),
            ],
            MakeDetail::BaseNotConvex,
        ),
    ] {
        let e = prism(
            &c,
            &PrismInput {
                base,
                height: Steps(1),
            },
        )
        .unwrap_err();
        assert_eq!(*e.detail(), OpErrorDetail::Make(expected));
    }
    let spindle = torus(
        &c,
        &TorusInput {
            center: p(0, 0, 0),
            major: Steps(3),
            minor: Steps(3),
            step: GridStep::Q0,
        },
    )
    .unwrap_err();
    assert_eq!(spindle.kind(), ErrorKind::InvalidInput);
    assert_eq!(
        spindle.remedies(),
        [Remedy::ChangeInput {
            input: "minor",
            to: Some((Bound::AtMost, Value::Count(2)))
        }]
    );
    // A corner past the grid's range.
    let far = make_box(
        &c,
        &BoxInput {
            min: p(1_099_511_627_775, 0, 0),
            size: [Steps(1); 3],
        },
    )
    .unwrap_err();
    assert_eq!(far.kind(), ErrorKind::FrameRangeExceeded);
    // A body without names cannot keep them.
    let b = a_box();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let unnamed = Builder::edit(&b.result.body)
        .publish(b.result.body.kind(), b.result.body.outside(), &mut meter)
        .unwrap();
    let e = move_by(
        &c,
        Moved {
            body: &unnamed,
            exactness: Exactness::Exact,
            accept: None,
        },
        [1, 0, 0],
    )
    .unwrap_err();
    assert_eq!(*e.detail(), OpErrorDetail::Make(MakeDetail::BodyNotNamed));
    // A budget too small to build the rows.
    let tight = OpContext::new(
        OpPath::new(OpId::from_bytes([7; 16])),
        Budget::new(10, u64::MAX),
    );
    let stopped = cylinder(&tight, &round(5, 10)).unwrap_err();
    assert_eq!(stopped.kind(), ErrorKind::BudgetExceeded);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn random_primitives_pass_all_four_groups(
        x in -1000i64..1000, y in -1000i64..1000, z in -1000i64..1000,
        a in 1u64..500, b in 1u64..500, h in 1u64..500,
    ) {
        let c = ctx();
        let size = [Steps(a), Steps(b), Steps(h)];
        let made = valid("box", make_box(&c, &BoxInput { min: p(x, y, z), size }).unwrap());
        let six = 6 * i128::from(a) * i128::from(b) * i128::from(h);
        prop_assert_eq!(six_volume(&made.result.body), six);
        let input = RoundInput { base: p(x, y, z), radius: Steps(a), height: Steps(h), step: GridStep::Q0 };
        valid("cylinder", cylinder(&c, &input).unwrap());
        valid("cone", cone(&c, &input).unwrap());
        let ball = SphereInput { center: p(x, y, z), radius: Steps(a), step: GridStep::Q0 };
        valid("sphere", sphere(&c, &ball).unwrap());
        let (major, minor) = (a.max(b) + 1, a.min(b));
        let ring = TorusInput { center: p(x, y, z), major: Steps(major), minor: Steps(minor), step: GridStep::Q0 };
        valid("torus", torus(&c, &ring).unwrap());
    }
}
