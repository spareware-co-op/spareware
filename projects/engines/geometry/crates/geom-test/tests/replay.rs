//! The kernel replays a log: every operation's parameters come back as written and only as
//! written, each operation runs on the bodies the log made, and a log that names anything else is
//! refused (chapter 28.16; pieces G97, G101 and G116).

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_make::{BoxInput, PrismInput, RoundInput, SphereInput, TorusInput};
use geom_math::Axis;
use geom_num::grid::P3;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::{ErrorKind, OpId, OpPath};
use geom_run::Budget;
use geom_test::fixture::OpLog;
use geom_test::fixture::calls::{Call, CallError, Input, OPERATIONS, Place, Turn};
use geom_test::fixture::kernel::{Kernel, id_of};
use geom_test::fixture::run::{Engine, Ran, Replayed, replay};

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

const EXACT: Input = Input {
    exactness: Exactness::Exact,
    accept: None,
};

/// `Input` of a body whose class is certified, accepting a bound.
fn certified() -> Input {
    Input {
        exactness: Exactness::Certified {
            bound: Steps(3),
            step: GridStep::Q1,
        },
        accept: Some(Exactness::Approximate {
            deviation: Steps(9),
            step: GridStep::Q0,
        }),
    }
}

fn a_box() -> Call {
    Call::Box(BoxInput {
        min: p(-5, 0, 7),
        size: [Steps(10), Steps(20), Steps(30)],
    })
}

/// One call of every operation, in every form its parameters take.
fn every_call() -> Vec<Call> {
    let round = RoundInput {
        base: p(1, 2, 3),
        radius: Steps(4),
        height: Steps(5),
        step: GridStep::Q2,
    };
    vec![
        a_box(),
        Call::Prism(PrismInput {
            base: vec![p(0, 0, 0), p(4, 0, 0), p(0, 4, 0)],
            height: Steps(2),
        }),
        Call::Cylinder(round),
        Call::Cone(round),
        Call::Sphere(SphereInput {
            center: p(0, 0, -1),
            radius: Steps(6),
            step: GridStep::Q0,
        }),
        Call::Torus(TorusInput {
            center: p(0, 0, 0),
            major: Steps(10),
            minor: Steps(3),
            step: GridStep::Q1,
        }),
        Call::Move(EXACT, [1, -2, 3]),
        Call::Move(certified(), [0, 0, -1]),
        Call::Rotate(EXACT, Axis::X, Turn::Quarter(-1), p(1, 1, 1)),
        Call::Rotate(EXACT, Axis::Y, Turn::Direction(3, -4), p(0, 0, 0)),
        Call::Mirror(EXACT, Axis::Z, p(0, 0, 2)),
        Call::Scale(EXACT, [(2, 1), (3, 2), (1, 5)], p(0, 0, 0)),
        Call::Pattern(
            EXACT,
            vec![
                ([1; 16], Place::Move([40, 0, 0])),
                ([2; 16], Place::Rotate(Axis::Z, Turn::Quarter(1))),
                ([3; 16], Place::Mirror(Axis::X)),
                ([4; 16], Place::Scale(2, 1)),
            ],
        ),
    ]
}

#[test]
fn every_call_reads_back_as_written() {
    for call in every_call() {
        let params = call.params().unwrap();
        assert_eq!(Call::read(call.name(), 1, &params), Ok(call));
    }
}

#[test]
fn the_table_holds_every_call() {
    let names: Vec<&str> = OPERATIONS.iter().map(|o| o.0).collect();
    for call in every_call() {
        assert!(names.contains(&call.name()), "{}", call.name());
    }
    assert_eq!(names.len(), 11);
}

#[test]
fn an_unknown_operation_or_version_is_refused() {
    let params = a_box().params().unwrap();
    assert_eq!(
        Call::read("make.boxes", 1, &params),
        Err(CallError::Unknown)
    );
    assert_eq!(Call::read("make.box", 2, &params), Err(CallError::Unknown));
}

#[test]
fn parameters_with_anything_more_are_refused() {
    let mut params = a_box().params().unwrap();
    params.push(0);
    assert!(matches!(
        Call::read("make.box", 1, &params),
        Err(CallError::Shape(_))
    ));
}

#[test]
fn another_shape_of_parameters_is_refused() {
    // Each of these is the array of another call's fields. (A cylinder's and a torus's have one
    // shape, so a log's name says which it is.)
    for call in every_call() {
        let params = call.params().unwrap();
        for name in ["make.box", "transform.scale"] {
            if name != call.name() {
                assert!(Call::read(name, 1, &params).is_err(), "{name} on {call:?}");
            }
        }
    }
}

#[test]
fn values_no_input_holds_are_refused() {
    // make.sphere's step: 3 is no grid step.
    let mut params = Call::Sphere(SphereInput {
        center: p(0, 0, 0),
        radius: Steps(6),
        step: GridStep::Q2,
    })
    .params()
    .unwrap();
    let last = params.len() - 1;
    params[last] = 3;
    assert_eq!(
        Call::read("make.sphere", 1, &params),
        Err(CallError::Value("a grid step"))
    );
    // transform.mirror's axis: 3 is no axis.
    let call = Call::Mirror(EXACT, Axis::Z, p(0, 0, 0));
    let mut params = call.params().unwrap();
    // [ [0], [], 2, [0, 0, 0] ]: the head, two items of two and one bytes, then the axis.
    params[4] = 3;
    assert_eq!(
        Call::read("transform.mirror", 1, &params),
        Err(CallError::Value("an axis"))
    );
}

fn path(byte: u8) -> OpPath {
    OpPath::new(OpId::from_bytes([byte; 16]))
}

fn operation(call: &Call, op: u8, inputs: Vec<RecipeInput>, budget: Budget) -> Operation {
    Operation {
        op: path(op),
        name: call.name().to_owned(),
        version: 1,
        inputs,
        params: call.params().unwrap(),
        budget,
    }
}

fn body_of(step: &Ran) -> RecipeInput {
    let Ran::Done(step) = step else {
        panic!("{step:?}");
    };
    RecipeInput::Body(geom_format::content::ContentId::new(1, &step.bodies[0]).unwrap())
}

fn log(operations: Vec<Operation>) -> OpLog {
    OpLog::new(Vec::new(), operations)
}

#[test]
fn a_log_replays_each_operation_on_the_body_before_it() {
    let mut kernel = Kernel::new();
    kernel.start(Steps(0), &[]).unwrap();
    let first = kernel.run(&operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED));
    let made = body_of(&first);
    let moved = Call::Move(EXACT, [1, 0, 0]);
    let m = operation(&moved, 2, vec![made.clone()], Budget::UNLIMITED);
    let second = kernel.run(&m);
    let moved_body = body_of(&second);
    let mirror = Call::Mirror(EXACT, Axis::Z, p(0, 0, 0));
    let n = operation(&mirror, 3, vec![moved_body], Budget::UNLIMITED);
    let third = kernel.run(&n);
    assert!(matches!(third, Ran::Done(_)), "{third:?}");
    let last = id_of(&kernel.last()[0]).unwrap();
    assert!(kernel.body(&last).is_some());

    let whole = log(vec![
        operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED),
        m,
        n,
    ]);
    let got = replay(&mut Kernel::new(), &whole);
    let Replayed::Done { hash, measures } = &got else {
        panic!("{got:?}");
    };
    assert_eq!(measures.counts.bodies, 1);
    assert_eq!(measures.counts.faces, 6);
    assert_eq!(measures.counts.edges, 12);
    assert_eq!(measures.counts.vertices, 8);
    assert_eq!(measures.bounds, Some([[-4, 0, -37], [6, 20, -7]]));
    assert_eq!(replay(&mut Kernel::new(), &whole), got);
    let moved_more = log(whole.operations[..2].to_vec());
    assert!(matches!(
        replay(&mut Kernel::new(), &moved_more),
        Replayed::Done { hash: other, .. } if other != *hash
    ));
}

#[test]
fn a_pattern_keeps_every_copy() {
    let mut kernel = Kernel::new();
    kernel.start(Steps(0), &[]).unwrap();
    let made = body_of(&kernel.run(&operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED)));
    let pattern = Call::Pattern(
        EXACT,
        vec![
            ([1; 16], Place::Move([40, 0, 0])),
            ([2; 16], Place::Move([80, 0, 0])),
        ],
    );
    let ran = kernel.run(&operation(&pattern, 2, vec![made], Budget::UNLIMITED));
    let Ran::Done(step) = ran else {
        panic!("{ran:?}");
    };
    assert_eq!(step.bodies.len(), 2);
    assert_eq!(kernel.last().len(), 2);
    assert_eq!(kernel.measures().counts.bodies, 2);
    // A pattern's copies have names of their own.
    assert_eq!(step.names.names().len(), 2 * (2 + 6 + 12 + 8));
}

#[test]
fn a_transform_makes_no_new_name() {
    let mut kernel = Kernel::new();
    kernel.start(Steps(0), &[]).unwrap();
    let first = kernel.run(&operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED));
    let Ran::Done(made) = &first else {
        panic!("{first:?}");
    };
    assert_eq!(made.names.names().len(), 2 + 6 + 12 + 8);
    let moved = Call::Move(EXACT, [1, 0, 0]);
    let ran = kernel.run(&operation(
        &moved,
        2,
        vec![body_of(&first)],
        Budget::UNLIMITED,
    ));
    let Ran::Done(step) = ran else {
        panic!("{ran:?}");
    };
    assert_eq!(step.names.names(), []);
    assert!(step.history.is_some());
}

#[test]
fn a_typed_error_ends_the_log() {
    let empty = Call::Box(BoxInput {
        min: p(0, 0, 0),
        size: [Steps(0), Steps(1), Steps(1)],
    });
    let got = replay(
        &mut Kernel::new(),
        &log(vec![operation(&empty, 1, Vec::new(), Budget::UNLIMITED)]),
    );
    assert!(
        matches!(
            got,
            Replayed::Failed {
                kind: ErrorKind::Degenerate,
                ..
            }
        ),
        "{got:?}"
    );
}

#[test]
fn the_records_budget_runs_the_operation() {
    let small = Budget {
        work: 3,
        logical_bytes: u64::MAX,
    };
    let got = replay(
        &mut Kernel::new(),
        &log(vec![operation(&a_box(), 1, Vec::new(), small)]),
    );
    assert!(
        matches!(
            got,
            Replayed::Failed {
                kind: ErrorKind::BudgetExceeded,
                ..
            }
        ),
        "{got:?}"
    );
}

#[test]
fn a_log_that_names_what_it_has_not_made_is_refused() {
    let moved = Call::Move(EXACT, [1, 0, 0]);
    let unknown = RecipeInput::Body(geom_format::content::ContentId::of_payload(b"no body"));
    let cases = [
        operation(&moved, 1, vec![unknown], Budget::UNLIMITED),
        operation(&moved, 1, Vec::new(), Budget::UNLIMITED),
        operation(
            &a_box(),
            1,
            vec![RecipeInput::Entity(geom_op::NameId::from_bytes([1; 16]))],
            Budget::UNLIMITED,
        ),
    ];
    for case in cases {
        let got = replay(&mut Kernel::new(), &log(vec![case]));
        assert!(matches!(got, Replayed::Broken(_)), "{got:?}");
    }
}

#[test]
fn a_call_with_no_such_input_is_refused() {
    let mut kernel = Kernel::new();
    kernel.start(Steps(0), &[]).unwrap();
    let made = body_of(&kernel.run(&operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED)));
    let flat = Call::Rotate(EXACT, Axis::Z, Turn::Direction(0, 0), p(0, 0, 0));
    assert!(matches!(
        kernel.run(&operation(&flat, 2, vec![made], Budget::UNLIMITED)),
        Ran::Refused(_)
    ));
}

#[test]
fn starting_again_forgets_the_bodies() {
    let mut kernel = Kernel::new();
    kernel.start(Steps(0), &[]).unwrap();
    let made = body_of(&kernel.run(&operation(&a_box(), 1, Vec::new(), Budget::UNLIMITED)));
    kernel.start(Steps(0), &[]).unwrap();
    assert_eq!(kernel.last().len(), 0);
    let moved = Call::Move(EXACT, [1, 0, 0]);
    assert!(matches!(
        kernel.run(&operation(&moved, 2, vec![made], Budget::UNLIMITED)),
        Ran::Refused(_)
    ));
}
