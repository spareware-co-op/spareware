//! Placed instances (19.12, 23.9; piece G145): every query on an instance equals the same query on
//! its materialisation, for all 48 signed permutations and for rotations that round; a general
//! placement's body is made once, only when asked, and re-validated.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "test code: a failure should stop loudly"
)]

use geom_make::placed::{Library, Shared, materialise};
use geom_make::{BoxInput, Made, Moved, PrismInput, RoundInput, cylinder, make_box, prism};
use geom_math::{Axis, Box3, GridTransform, Rotation, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{MakeDetail, OpContext, OpErrorDetail, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::Ix;
use geom_topo::placed::{Definition, Placed, Placement, from_shared, to_shared};
use geom_topo::query::{self, Side};

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([11; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn a_box() -> OpOutput<Made> {
    let input = BoxInput {
        min: p(2, -3, 5),
        size: [Steps(10), Steps(20), Steps(30)],
    };
    make_box(&ctx(), &input).unwrap()
}

fn a_prism() -> OpOutput<Made> {
    let input = PrismInput {
        base: vec![p(0, 0, 1), p(30, 0, 1), p(40, 20, 1), p(10, 20, 1)],
        height: Steps(15),
    };
    prism(&ctx(), &input).unwrap()
}

/// The box of a body's vertices.
fn boxed(made: &OpOutput<Made>) -> Box3 {
    query::bounding_box(&made.result.body, &mut meter())
        .unwrap()
        .unwrap()
        .0
}

/// All 48 signed permutations, each with a shift.
fn permutations() -> Vec<GridTransform> {
    let mut out = Vec::new();
    for source in [
        [Axis::X, Axis::Y, Axis::Z],
        [Axis::X, Axis::Z, Axis::Y],
        [Axis::Y, Axis::X, Axis::Z],
        [Axis::Y, Axis::Z, Axis::X],
        [Axis::Z, Axis::X, Axis::Y],
        [Axis::Z, Axis::Y, Axis::X],
    ] {
        for bits in 0..8 {
            let negate = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
            out.push(GridTransform::from_parts(source, negate, [7, -11, 13]).unwrap());
        }
    }
    out
}

/// Every query of `instance` of `made` against the same query on `body`.
fn same_answers(made: &OpOutput<Made>, body: &geom_topo::Body, instance: &Placed, name: &str) {
    let shared = Shared {
        key: 1_u8,
        body: &made.result.body,
        exactness: made.exactness,
        accept: None,
    };
    let table: [TypedTransform; 0] = [];
    let mut library = Library::new(&table);
    assert_eq!(
        library.six_volume(&ctx(), &shared, instance).unwrap(),
        query::volume(body, &mut meter()).unwrap(),
        "{name}"
    );
    let rows = library.faces(&ctx(), &shared, instance).unwrap();
    assert_eq!(rows, query::faces(body, &mut meter()).unwrap(), "{name}");
    for row in &rows {
        assert_eq!(
            library
                .face_area(&ctx(), &shared, instance, row.face)
                .unwrap(),
            query::face_area(body, row.face, &mut meter()).unwrap(),
            "{name}: {row:?}"
        );
    }
    assert_eq!(
        *instance.bound(),
        query::bounding_box(body, &mut meter()).unwrap().unwrap().0,
        "{name}"
    );
    let b = instance.bound();
    let (lo, hi) = (b.min(), b.max());
    for x in [
        lo.x.get() - 2,
        lo.x.get() + 1,
        hi.x.get() - 1,
        hi.x.get() + 2,
    ] {
        for y in [
            lo.y.get() - 2,
            lo.y.get() + 1,
            hi.y.get() - 1,
            hi.y.get() + 2,
        ] {
            for z in [
                lo.z.get() - 2,
                lo.z.get() + 1,
                hi.z.get() - 1,
                hi.z.get() + 2,
            ] {
                let q = p(x, y, z);
                assert_eq!(
                    library
                        .side_of_shell(&ctx(), &shared, instance, Ix::new(0).unwrap(), q)
                        .unwrap(),
                    query::side_of_shell(body, Ix::new(0).unwrap(), q, &mut meter()).unwrap(),
                    "{name}: {q:?}"
                );
            }
        }
    }
    // An exact placement never made a body.
    assert!(library.is_empty());
}

#[test]
fn every_query_on_an_exact_instance_equals_its_materialisation() {
    for (name, made) in [("box", a_box()), ("prism", a_prism())] {
        for (n, t) in permutations().iter().enumerate() {
            let instance = Placed::exact(Definition(3), t, &boxed(&made)).unwrap();
            let moved = Moved {
                body: &made.result.body,
                exactness: made.exactness,
                accept: None,
            };
            let body = materialise(&ctx(), moved, &TypedTransform::Exact(*t)).unwrap();
            same_answers(&made, &body.result.body, &instance, &format!("{name} {n}"));
        }
    }
}

#[test]
fn a_point_crosses_a_placement_and_comes_back() {
    let made = a_box();
    for t in permutations() {
        let instance = Placed::exact(Definition(0), &t, &boxed(&made)).unwrap();
        let q = p(3, 4, 6);
        let there = from_shared(&instance, q).unwrap();
        assert_eq!(there, t.apply(q).unwrap());
        assert_eq!(to_shared(&instance, there).unwrap(), q);
    }
}

#[test]
fn a_rotation_goes_through_a_body_made_once_and_checked() {
    let made = a_box();
    let table = [TypedTransform::rotation(Axis::Z, Rotation::direction(4, 3).unwrap()).unwrap()];
    let accept = Some(Exactness::Certified {
        bound: Steps(64),
        step: GridStep::Q0,
    });
    let shared = Shared {
        key: 9_u8,
        body: &made.result.body,
        exactness: made.exactness,
        accept,
    };
    let instance = Placed::general(Definition(1), 0, &table[0], &boxed(&made)).unwrap();
    let mut library = Library::new(&table);
    assert!(library.is_empty());
    let six = library.six_volume(&ctx(), &shared, &instance).unwrap();
    assert_eq!(library.len(), 1);
    assert!(!library.is_empty());
    // The second question makes nothing.
    let side = library
        .side_of_shell(&ctx(), &shared, &instance, Ix::new(0).unwrap(), p(1, 3, 20))
        .unwrap();
    assert_eq!(library.len(), 1);
    let rows = library.faces(&ctx(), &shared, &instance).unwrap();
    assert_eq!(rows.len(), 6);
    let area = library
        .face_area(&ctx(), &shared, &instance, rows[0].face)
        .unwrap();
    assert_eq!(library.len(), 1);
    // The answers are the materialised body's, and its box lies inside the instance's.
    let moved = Moved {
        body: &made.result.body,
        exactness: made.exactness,
        accept,
    };
    let body = materialise(&ctx(), moved, &table[0]).unwrap();
    assert!(matches!(body.exactness, Exactness::Certified { .. }));
    let b = body.result.body;
    assert_eq!(six, query::volume(&b, &mut meter()).unwrap());
    assert_eq!(
        area,
        query::face_area(&b, rows[0].face, &mut meter()).unwrap()
    );
    assert_eq!(
        side,
        query::side_of_shell(&b, Ix::new(0).unwrap(), p(1, 3, 20), &mut meter()).unwrap()
    );
    let (inner, _) = query::bounding_box(&b, &mut meter()).unwrap().unwrap();
    assert!(instance.bound().contains(inner.min()));
    assert!(instance.bound().contains(inner.max()));
    // One definition at another key is another body.
    let other = Shared {
        key: 10_u8,
        ..shared
    };
    library.six_volume(&ctx(), &other, &instance).unwrap();
    assert_eq!(library.len(), 2);
}

#[test]
fn a_side_of_a_placed_shell_is_where_the_placed_body_is() {
    let made = a_box();
    let t = GridTransform::translation([100, 200, 300]).unwrap();
    let instance = Placed::exact(Definition(0), &t, &boxed(&made)).unwrap();
    let shared = Shared {
        key: 0_u8,
        body: &made.result.body,
        exactness: made.exactness,
        accept: None,
    };
    let table: [TypedTransform; 0] = [];
    let mut library = Library::new(&table);
    let s = Ix::new(0).unwrap();
    let side = |l: &mut Library<'_, u8>, q| l.side_of_shell(&ctx(), &shared, &instance, s, q);
    assert_eq!(
        side(&mut library, p(107, 205, 320)).unwrap(),
        Some(Side::Inside)
    );
    assert_eq!(
        side(&mut library, p(7, 5, 20)).unwrap(),
        Some(Side::Outside)
    );
    assert_eq!(side(&mut library, p(102, 197, 305)).unwrap(), None);
}

#[test]
fn a_placement_past_the_table_is_refused_and_nothing_is_kept() {
    let made = a_box();
    let table = [TypedTransform::rotation(Axis::Z, Rotation::direction(4, 3).unwrap()).unwrap()];
    let shared = Shared {
        key: 0_u8,
        body: &made.result.body,
        exactness: made.exactness,
        accept: None,
    };
    let mut library = Library::new(&table[..0]);
    let instance = Placed::general(Definition(1), 5, &table[0], &boxed(&made)).unwrap();
    let e = library.six_volume(&ctx(), &shared, &instance).unwrap_err();
    assert_eq!(
        *e.detail(),
        OpErrorDetail::Make(MakeDetail::NoSuchPlacement { index: 5 })
    );
    assert!(library.is_empty());
}

#[test]
fn a_rounded_body_that_is_not_accepted_is_refused_and_not_kept() {
    let made = a_box();
    let table = [TypedTransform::rotation(Axis::Z, Rotation::direction(4, 3).unwrap()).unwrap()];
    let shared = Shared {
        key: 0_u8,
        body: &made.result.body,
        exactness: made.exactness,
        accept: None,
    };
    let instance = Placed::general(Definition(1), 0, &table[0], &boxed(&made)).unwrap();
    let mut library = Library::new(&table);
    assert!(library.six_volume(&ctx(), &shared, &instance).is_err());
    assert_eq!(library.len(), 0);
}

#[test]
fn an_exact_placement_of_a_curved_body_answers_in_the_shared_frame() {
    let round = RoundInput {
        base: p(0, 0, 0),
        radius: Steps(400),
        height: Steps(900),
        step: GridStep::Q0,
    };
    let made = cylinder(&ctx(), &round).unwrap();
    let t = GridTransform::quarter_turns(Axis::X, 1);
    let (bound, _) = query::bounding_box(&made.result.body, &mut meter())
        .unwrap()
        .unwrap();
    let instance = Placed::exact(Definition(2), &t, &bound).unwrap();
    assert_eq!(instance.placement(), Placement::exact(&t));
    let want = Box3::from_points([t.apply(bound.min()).unwrap(), t.apply(bound.max()).unwrap()]);
    assert_eq!(Some(*instance.bound()), want);
}
