//! Attacks on piece G101's claims: every primitive and every transform passes all four checker
//! groups and the history laws; primitives name their entities by role, never by position;
//! transforms keep every name, stay exact where the transform is and refuse a less exact result
//! the caller did not accept; mirrors keep solids facing outward; patterns name their copies as
//! copies and read each copy's stretch from its transform; the class never rises; a coordinate past
//! the grid is the numbers layer's error, never a defect; a stop publishes nothing; bad input gets
//! a typed error with its remedy.
//!
//! Covers: make.box, make.cone, make.cylinder, make.prism, make.sphere, make.torus,
//! transform.mirror, transform.move, transform.pattern, transform.rotate, transform.scale.
//!
//! Family laws: Make, Transform.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "test code: a failure should stop loudly"
)]

use geom_check::{Level, check};
use geom_curve::{CurveKind, ParamCurve};
use geom_make::{
    BoxInput, Instance, Made, Motion, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere, torus,
};
use geom_math::{
    Axis, GridTransform, RationalTransform, Rotation, Scale, Transform, TypedTransform,
};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_nurbs::Limit;
use geom_op::history::Origin;
use geom_op::{
    Bound, ErrorKind, MakeDetail, NameId, NameList, NameSet, OpContext, OpErrorDetail, OpId,
    OpOutput, OpPath, Remedy, Role, RoleArg, TopoName, Value,
};
use geom_run::{Budget, CancelToken, WorkMeter};
use geom_surface::ParamSurface;
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
        // Three corners in a line, clockwise, a dent, and a star that winds twice.
        (
            vec![p(0, 0, 0), p(5, 0, 0), p(10, 0, 0), p(0, 9, 0)],
            MakeDetail::BaseNotConvex,
        ),
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

/// Whether every face of `body` looks away from `inside`, a point inside it: each face's normal,
/// with its sense, at the middle of its domain.
fn faces_outward(body: &Body, inside: [f64; 3]) -> bool {
    let a = body.arenas();
    (0..a.rows(Arena::Face)).all(|row| {
        let f = Ix::new(row).unwrap();
        let surface = a.surfaces.get(*a.faces.surface.get(f).unwrap()).unwrap();
        let ((u0, u1), (v0, v1)) = surface.domain();
        let sides = (Limit::FromRight, Limit::FromRight);
        let (u, v) = (u0 + (u1 - u0) * 0.3, v0 + (v1 - v0) * 0.4);
        let [point, du, dv] = surface.partials(u, v, sides).unwrap();
        let normal = [
            du[1] * dv[2] - du[2] * dv[1],
            du[2] * dv[0] - du[0] * dv[2],
            du[0] * dv[1] - du[1] * dv[0],
        ];
        let sign = match *a.faces.sense.get(f).unwrap() {
            Sense::Forward => 1.0,
            Sense::Reversed => -1.0,
        };
        let away = (0..3).fold(0.0, |d, i| d + (point[i] - inside[i]) * normal[i]);
        away * sign > 0.0
    })
}

#[test]
fn every_curved_solid_faces_outward_and_a_mirror_keeps_it_so() {
    let c = ctx();
    let bodies = [
        (
            "cylinder",
            cylinder(&c, &round(5, 10)).unwrap(),
            [0.0, 0.0, 5.0],
        ),
        ("cone", cone(&c, &round(6, 12)).unwrap(), [0.0, 0.0, 3.0]),
        ("sphere", primitives().swap_remove(4).1, [1.0, 2.0, 3.0]),
    ];
    for (name, out, inside) in bodies {
        assert!(faces_outward(&out.result.body, inside), "{name}");
        let moved = Moved {
            body: &out.result.body,
            exactness: out.exactness,
            accept: None,
        };
        for (axis, k) in [(Axis::X, 0), (Axis::Y, 1), (Axis::Z, 2)] {
            let image = mirror(&c, moved, axis, p(0, 0, 0)).unwrap();
            let mut there = inside;
            there[k] = -there[k];
            assert!(faces_outward(&image.result.body, there), "{name} {axis:?}");
        }
    }
}

#[test]
fn a_result_says_which_transform_made_it() {
    assert!(primitives().iter().all(|(_, o)| o.result.motion.is_none()));
    let b = a_box();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let c = ctx();
    let two = [Scale::new(2, 1).unwrap(); 3];
    let quarter = Rotation::quarter_turns(1);
    for (result, motion) in [
        (move_by(&c, moved, [1, 0, 0]), Motion::Move),
        (
            rotate(&c, moved, Axis::Z, quarter, p(0, 0, 0)),
            Motion::Rotate,
        ),
        (mirror(&c, moved, Axis::Z, p(0, 0, 0)), Motion::Mirror),
        (scale(&c, moved, two, p(0, 0, 0)), Motion::Scale),
    ] {
        assert_eq!(result.unwrap().result.motion, Some(motion));
    }
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::Exact(GridTransform::translation([1, 0, 0]).unwrap()),
    };
    let copies = pattern(&c, moved, &[at]).unwrap();
    assert_eq!(copies.result[0].1.motion, None);
}

#[test]
fn the_class_never_rises_and_a_more_exact_result_needs_no_acceptance() {
    let ball = |radius: u64| {
        sphere(
            &ctx(),
            &SphereInput {
                center: p(0, 0, 0),
                radius: Steps(radius),
                step: GridStep::Q0,
            },
        )
        .unwrap()
    };
    let half = [Scale::new(1, 2).unwrap(); 3];
    let b = ball(8);
    // Recorded looser than it is, then halved: the bound falls with no acceptance, to half of 8
    // plus the step the radius's division can certify at best.
    let loose = Moved {
        body: &b.result.body,
        exactness: certified(8),
        accept: None,
    };
    let halved = scale(&ctx(), loose, half, p(0, 0, 0)).unwrap();
    assert_eq!(halved.exactness, certified(5));
    // An Approximate body stays Approximate however its bound moves, and a ceiling of a stricter
    // class does not accept it.
    let measured = |deviation: u64| Exactness::Approximate {
        deviation: Steps(deviation),
        step: GridStep::Q0,
    };
    let rough = Moved {
        body: &b.result.body,
        exactness: measured(8),
        accept: None,
    };
    assert_eq!(
        scale(&ctx(), rough, half, p(0, 0, 0)).unwrap().exactness,
        measured(5)
    );
    let two = [Scale::new(2, 1).unwrap(); 3];
    assert_eq!(
        *scale(&ctx(), rough, two, p(0, 0, 0)).unwrap_err().detail(),
        OpErrorDetail::Make(MakeDetail::NotAccepted {
            would: measured(16)
        })
    );
    let strict = Moved {
        accept: Some(certified(64)),
        ..rough
    };
    assert_eq!(
        scale(&ctx(), strict, two, p(0, 0, 0)).unwrap_err().kind(),
        ErrorKind::ExactnessNotAccepted
    );
    let open = Moved {
        accept: Some(measured(16)),
        ..rough
    };
    assert_eq!(
        scale(&ctx(), open, two, p(0, 0, 0)).unwrap().exactness,
        measured(16)
    );
    // A ceiling in a coarser step is compared in the finer one.
    let coarse = Moved {
        accept: Some(Exactness::Certified {
            bound: Steps(1),
            step: GridStep::Q1,
        }),
        exactness: Exactness::Certified {
            bound: Steps(8),
            step: GridStep::Q2,
        },
        ..rough
    };
    let fine = scale(&ctx(), coarse, two, p(0, 0, 0));
    assert_eq!(
        fine.map(|o| o.exactness).map_err(|e| e.kind()),
        Ok(Exactness::Certified {
            bound: Steps(16),
            step: GridStep::Q2
        })
    );
}

#[test]
fn a_pattern_reads_each_copys_stretch_from_its_transform() {
    let b = primitives().swap_remove(4).1;
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: Some(certified(64)),
    };
    let copy = |transform: TypedTransform| {
        let at = Instance {
            key: [1; 16],
            transform,
        };
        pattern(&ctx(), moved, &[at]).map(|o| o.exactness)
    };
    // An exact uniform scale stretches the bound by exactly its factor.
    let doubled = RationalTransform::uniform_scale(Scale::new(2, 1).unwrap()).unwrap();
    assert_eq!(copy(TypedTransform::from(doubled)), Ok(certified(2)));
    let halved = RationalTransform::uniform_scale(Scale::new(1, 2).unwrap()).unwrap();
    assert_eq!(copy(TypedTransform::from(halved)), Ok(certified(2)));
    // A uniform scale held in doubles: 3 in 1/1024ths, plus the step a surface moved in doubles
    // can certify at best.
    let in_doubles = Transform::uniform_scale(3.0).unwrap();
    let tripled = copy(TypedTransform::from(in_doubles)).unwrap();
    assert_eq!(tripled, certified(4));
    // A rotation by a third of a turn, held in doubles: lengths kept within ε_ang, so the bound
    // takes at most that fraction of a step more, rounded up.
    let turned = Transform::rotation(Axis::Z, 2.0 / 3.0).unwrap();
    let turned = copy(TypedTransform::from(turned)).unwrap();
    let (bound, _) = turned.bound().unwrap();
    assert!(
        (2..=64).contains(&bound.get()),
        "a bound above the input's and below the ceiling: {turned:?}"
    );
    // A shear keeps no lengths: bounded by its largest row or column sum, 3 here.
    let shear = RationalTransform::new([[1, 2, 0], [0, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap();
    let flat = a_box();
    let sheared = Moved {
        body: &flat.result.body,
        exactness: certified(5),
        accept: Some(certified(15)),
    };
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::from(shear),
    };
    assert_eq!(
        pattern(&ctx(), sheared, &[at])
            .map(|o| o.exactness)
            .map_err(|e| e.kind()),
        Ok(certified(15))
    );
    let row = RationalTransform::new([[1, 0, 0], [2, 1, 0], [0, 0, 1]], [0; 3], 1).unwrap();
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::from(row),
    };
    assert_eq!(
        pattern(&ctx(), sheared, &[at])
            .map(|o| o.exactness)
            .map_err(|e| e.kind()),
        Ok(certified(15))
    );
    // Stretching a shear held in doubles reads its rows and columns too: 2.5 in the top row.
    let doubles = Transform::from_rows([
        [1.0, 1.5, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ])
    .unwrap();
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::from(doubles),
    };
    assert_eq!(
        pattern(
            &ctx(),
            Moved {
                accept: Some(certified(64)),
                ..sheared
            },
            &[at]
        )
        .map(|o| o.exactness.bound().map(|(b, _)| b.get()))
        .map_err(|e| e.kind()),
        Ok(Some(13))
    );
}

#[test]
fn a_pattern_refuses_what_it_cannot_make() {
    let cyl = cylinder(&ctx(), &round(5, 10)).unwrap();
    let moved = Moved {
        body: &cyl.result.body,
        exactness: cyl.exactness,
        accept: Some(certified(64)),
    };
    let copy = |m: [[i64; 3]; 3]| {
        let transform = TypedTransform::from(RationalTransform::new(m, [0; 3], 1).unwrap());
        let at = Instance {
            key: [1; 16],
            transform,
        };
        pattern(&ctx(), moved, &[at]).unwrap_err()
    };
    let flat = copy([[1, 0, 0], [0, 1, 0], [0, 0, 0]]);
    assert_eq!(
        *flat.detail(),
        OpErrorDetail::Make(MakeDetail::SingularTransform)
    );
    assert_eq!(flat.kind(), ErrorKind::InvalidInput);
    assert_eq!(
        flat.remedies(),
        [Remedy::ChangeInput {
            input: "instances",
            to: None
        }]
    );
    assert_eq!(
        *copy([[2, 0, 0], [0, 1, 0], [0, 0, 1]]).detail(),
        OpErrorDetail::Make(MakeDetail::KindNotCertified)
    );
}

#[test]
fn a_coordinate_past_the_grid_is_the_numbers_error_never_a_defect() {
    let edge = (1i64 << 40) - 2;
    let c = ctx();
    // A sphere has no vertices, so only its face's box can leave the grid.
    let ball = primitives().swap_remove(4).1;
    let moved = Moved {
        body: &ball.result.body,
        exactness: ball.exactness,
        accept: Some(certified(1024)),
    };
    let kinds = [
        move_by(&c, moved, [edge, 0, 0]),
        scale(&c, moved, [Scale::new(1 << 39, 1).unwrap(); 3], p(0, 0, 0)),
        scale(&c, moved, [Scale::new(u64::MAX, 1).unwrap(); 3], p(0, 0, 0)),
        scale(&c, moved, [Scale::new(1, u64::MAX).unwrap(); 3], p(0, 0, 0)),
        scale(
            &c,
            moved,
            [
                Scale::new(1, 1 << 32).unwrap(),
                Scale::new(1, 1 << 16).unwrap(),
                Scale::new(1, 1 << 16).unwrap(),
            ],
            p(0, 0, 0),
        ),
    ]
    .map(|r| r.unwrap_err().kind());
    assert_eq!(kinds, [ErrorKind::FrameRangeExceeded; 5]);
    // Rounding one step wider at the grid's edge leaves it too.
    let b = make_box(
        &c,
        &BoxInput {
            min: p(edge - 4, 0, 0),
            size: [Steps(5); 3],
        },
    )
    .unwrap();
    let turned = rotate(
        &c,
        Moved {
            body: &b.result.body,
            exactness: b.exactness,
            accept: Some(certified(64)),
        },
        Axis::Z,
        Rotation::quarter_turns(1),
        p(0, 0, 0),
    )
    .map(|o| o.exactness);
    assert_eq!(turned, Ok(Exactness::Exact));
}

#[test]
fn a_stop_publishes_nothing() {
    let token = CancelToken::new();
    token.cancel();
    let cancelled = OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
        .with_cancel(&token);
    assert_eq!(
        cylinder(&cancelled, &round(5, 10)).unwrap_err().kind(),
        ErrorKind::Cancelled
    );
    let b = a_box();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    assert_eq!(
        move_by(&cancelled, moved, [1, 0, 0]).unwrap_err().kind(),
        ErrorKind::Cancelled
    );
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::Exact(GridTransform::translation([1, 0, 0]).unwrap()),
    };
    assert_eq!(
        pattern(&cancelled, moved, &[at]).unwrap_err().kind(),
        ErrorKind::Cancelled
    );
}

/// `work` units of budget: what an operation may charge.
fn with_work(work: u64) -> OpContext<'static> {
    OpContext::new(
        OpPath::new(OpId::from_bytes([7; 16])),
        Budget::new(work, u64::MAX),
    )
}

#[test]
fn every_budget_passes_or_stops_cleanly_at_the_same_charge() {
    let input = BoxInput {
        min: p(0, 0, 0),
        size: [Steps(1); 3],
    };
    // One unit below the least budget that passes is a budget failure, never a defect.
    let least = (0..10_000)
        .find(|&work| make_box(&with_work(work), &input).is_ok())
        .unwrap();
    assert!(least >= 8 + 12 + 6 + 2 + 6 + 24);
    assert_eq!(
        make_box(&with_work(least - 1), &input).unwrap_err().kind(),
        ErrorKind::BudgetExceeded
    );
    for work in 0..least {
        assert_eq!(
            make_box(&with_work(work), &input).unwrap_err().kind(),
            ErrorKind::BudgetExceeded
        );
    }
    // Moving charges every row once, then what publishing and the checker charge.
    let b = make_box(&ctx(), &input).unwrap();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    for work in 0..50 {
        let e = move_by(&with_work(work), moved, [1, 0, 0]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::BudgetExceeded, "{work}");
    }
}

#[test]
fn a_stop_inside_a_curve_is_a_stop_not_a_refused_kind() {
    // A NURBS line in place of one of a box's edges: moving it charges per control point.
    let b = a_box();
    let mut builder = Builder::edit(&b.result.body);
    let a = &mut builder.arenas;
    let line = a.curves.get(Ix::new(0).unwrap()).unwrap().clone();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let nurbs = line.to_nurbs(&mut meter).unwrap().value.curve;
    a.curves
        .set(Ix::new(0).unwrap(), CurveKind::Nurbs(nurbs))
        .unwrap();
    let names = b.result.body.names().unwrap().clone();
    let body = builder
        .publish_named(
            &names,
            b.result.body.kind(),
            b.result.body.outside(),
            &mut meter,
        )
        .unwrap();
    let moved = Moved {
        body: &body,
        exactness: Exactness::Exact,
        accept: Some(certified(64)),
    };
    let mut stops = 0;
    for work in 0..400 {
        match move_by(&with_work(work), moved, [1, 0, 0]) {
            Err(e) => {
                assert_ne!(e.kind(), ErrorKind::Unsupported, "{work}: {e:?}");
                stops += 1;
            }
            Ok(_) => break,
        }
    }
    assert!(stops > 0);
}

#[test]
fn a_long_prism_keeps_every_face_edge_and_corner() {
    // Corners on a parabola, closed along its chord: strictly convex, counter-clockwise.
    let n = 200i64;
    let input = PrismInput {
        base: (0..n).map(|k| p(k, k * k, 0)).collect(),
        height: Steps(7),
    };
    let made = prism(&ctx(), &input).unwrap().result;
    let a = made.body.arenas();
    let n = n as u32;
    assert_eq!(a.rows(Arena::Face), n + 2);
    assert_eq!(a.rows(Arena::Edge), 3 * n);
    assert_eq!(a.rows(Arena::Vertex), 2 * n);
    assert_eq!(a.rows(Arena::Fin), 6 * n);
    assert_eq!(
        names(&made.body).len(),
        (n + 2 + 3 * n + 2 * n + 2) as usize
    );
}

#[test]
fn a_cones_side_narrows_to_its_apex() {
    // Nothing else pins the slope: the apex has no entity for the checker to measure.
    let input = RoundInput {
        base: p(1, 2, 3),
        radius: Steps(6),
        height: Steps(12),
        step: GridStep::Q0,
    };
    let out = cone(&ctx(), &input).unwrap();
    let a = out.result.body.arenas();
    let side = a
        .surfaces
        .iter()
        .find(|s| matches!(s, geom_surface::SurfaceKind::Cone(_)))
        .unwrap();
    let ((_, _), (_, top)) = side.domain();
    let sides = (Limit::FromRight, Limit::FromRight);
    for u in [0.0, 0.25, 0.5, 1.0, 1.5] {
        let [x, y, z] = side.point(u, top, sides).unwrap();
        let far = [x - 1.0, y - 2.0, z - 15.0].map(f64::abs);
        assert!(far.into_iter().all(|d| d < 1e-9), "{u}: {x} {y} {z}");
        let [x, y, z] = side.point(u, 0.0, sides).unwrap();
        let radius = ((x - 1.0) * (x - 1.0) + (y - 2.0) * (y - 2.0)).sqrt();
        assert!((radius - 6.0).abs() < 1e-9 && (z - 3.0).abs() < 1e-9, "{u}");
    }
}

#[test]
fn a_scale_that_is_not_uniform_waits_for_every_curved_kind() {
    let stretched = [Scale::new(2, 1).unwrap(), Scale::ONE, Scale::ONE];
    for (name, out) in primitives() {
        let moved = Moved {
            body: &out.result.body,
            exactness: out.exactness,
            accept: Some(certified(64)),
        };
        let result = scale(&ctx(), moved, stretched, p(0, 0, 0));
        if matches!(name, "box" | "prism") {
            assert!(result.is_ok(), "{name}");
        } else {
            assert_eq!(
                *result.unwrap_err().detail(),
                OpErrorDetail::Make(MakeDetail::KindNotCertified),
                "{name}"
            );
        }
    }
}

#[test]
fn a_rigid_motion_stretches_nothing_and_one_in_doubles_by_at_most_its_angle() {
    let ball = primitives().swap_remove(4).1;
    let moved = Moved {
        body: &ball.result.body,
        exactness: certified(100),
        accept: Some(certified(1000)),
    };
    let turn = Rotation::direction(4, 3).unwrap();
    let exact = rotate(&ctx(), moved, Axis::Z, turn, p(0, 0, 0)).unwrap();
    assert_eq!(exact.exactness, certified(101));
    let at = Instance {
        key: [1; 16],
        transform: TypedTransform::from(Transform::rotation(Axis::Z, 2.0 / 3.0).unwrap()),
    };
    let in_doubles = pattern(&ctx(), moved, &[at]).unwrap();
    assert_eq!(in_doubles.exactness, certified(102));
}

#[test]
fn a_rounded_corner_adds_a_step() {
    // Corners at odd coordinates halved: each rounds once, the planes stay true.
    let b = make_box(
        &ctx(),
        &BoxInput {
            min: p(1, 1, 1),
            size: [Steps(2); 3],
        },
    )
    .unwrap();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: None,
    };
    let half = [Scale::new(1, 2).unwrap(); 3];
    assert_eq!(
        *scale(&ctx(), moved, half, p(0, 0, 0)).unwrap_err().detail(),
        OpErrorDetail::Make(MakeDetail::NotAccepted {
            would: certified(2)
        })
    );
}

#[test]
fn an_invalid_body_moved_exactly_is_a_defect_not_a_gap_in_the_checker() {
    let b = a_box();
    let mut builder = Builder::edit(&b.result.body);
    let f = Ix::new(0).unwrap();
    let sense = *builder.arenas.fins.sense.get(f).unwrap();
    let flipped = match sense {
        Sense::Forward => Sense::Reversed,
        Sense::Reversed => Sense::Forward,
    };
    builder.arenas.fins.sense.set(f, flipped).unwrap();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = builder
        .publish_named(
            b.result.body.names().unwrap(),
            b.result.body.kind(),
            b.result.body.outside(),
            &mut meter,
        )
        .unwrap();
    let moved = Moved {
        body: &body,
        exactness: Exactness::Exact,
        accept: None,
    };
    let e = move_by(&ctx(), moved, [1, 0, 0]).unwrap_err();
    assert_eq!(*e.detail(), OpErrorDetail::Make(MakeDetail::ResultInvalid));
    assert_eq!(e.kind(), ErrorKind::Internal);
}

/// Every vertex of every face of `body`, with the face's box.
fn face_corners(body: &Body) -> Vec<(geom_math::Box3, Vec<P3>)> {
    let a = body.arenas();
    (0..a.rows(Arena::Face))
        .map(|row| {
            let f = Ix::new(row).unwrap();
            let mut corners = Vec::new();
            for l in 0..a.rows(Arena::Loop) {
                let l = Ix::new(l).unwrap();
                if *a.loops.face.get(l).unwrap() != f {
                    continue;
                }
                let first = a.loops.first_fin.get(l).copied().flatten().unwrap();
                let mut at = first;
                loop {
                    let e = *a.fins.edge.get(at).unwrap();
                    let ends = if *a.fins.sense.get(at).unwrap() == Sense::Forward {
                        &a.edges.start
                    } else {
                        &a.edges.end
                    };
                    let v = ends.get(e).copied().flatten().unwrap();
                    corners.push(*a.points.get(*a.vertices.point.get(v).unwrap()).unwrap());
                    at = *a.fins.next.get(at).unwrap();
                    if at == first {
                        break;
                    }
                }
            }
            (*a.faces.bound.get(f).unwrap(), corners)
        })
        .collect()
}

#[test]
fn a_moved_faces_box_holds_its_corners_and_no_more() {
    let b = a_box();
    let moved = Moved {
        body: &b.result.body,
        exactness: b.exactness,
        accept: Some(certified(64)),
    };
    let c = ctx();
    let turn = Rotation::direction(4, 3).unwrap();
    for (how, out) in [
        (
            "quarter",
            rotate(&c, moved, Axis::Z, Rotation::quarter_turns(1), p(3, 4, 5)),
        ),
        ("rotated", rotate(&c, moved, Axis::Y, turn, p(0, 0, 0))),
        ("mirrored", mirror(&c, moved, Axis::X, p(7, 0, 0))),
    ] {
        for (bound, corners) in face_corners(&out.unwrap().result.body) {
            let (lo, hi) = (bound.min(), bound.max());
            for (low, high, get) in [
                (lo.x, hi.x, (|q: &P3| q.x) as fn(&P3) -> geom_num::Coord),
                (lo.y, hi.y, |q: &P3| q.y),
                (lo.z, hi.z, |q: &P3| q.z),
            ] {
                let values: Vec<i64> = corners.iter().map(|q| get(q).get()).collect();
                let (least, most) = (*values.iter().min().unwrap(), *values.iter().max().unwrap());
                assert!(low.get() <= least && most <= high.get(), "{how}");
                assert!(least - low.get() <= 1 && high.get() - most <= 1, "{how}");
            }
        }
    }
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
