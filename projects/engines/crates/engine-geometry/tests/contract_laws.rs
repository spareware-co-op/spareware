//! Chapter 28.2's contract on every operation the facade registers (piece G103).
//!
//! What this file proves: each operation in [`OPERATIONS`] has a [`Driver`], and each driver's calls
//! keep `geom_test::contract`'s laws. An operation added to the registry without a driver fails
//! `every_registered_operation_has_a_driver`, so the laws reach it from the day it lands; each
//! operation piece adds its row to [`drivers`]. No body holds a tolerance until chapter 6.5's
//! healing lands, so "No tolerance grows" joins here with the first operation that takes one; the
//! law itself is seeded in `geom-test/tests/contract_kit.rs`.
//!
//! Covers: `make.box`, `make.cone`, `make.cylinder`, `make.prism`, `make.sphere`, `make.torus`,
//! `transform.mirror`, `transform.move`, `transform.pattern`, `transform.rotate`,
//! `transform.scale`, `query.area`, `query.bounding_box`, `query.exactness`, `query.faces`,
//! `query.validity`, `query.volume`.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "test code: a failure should stop loudly"
)]

use core::fmt::Debug;
use std::collections::BTreeSet;
use std::sync::Arc;

use engine_geometry::operations::{
    BoxInput, Copies, Instance, Level, Made, Moved, OPERATIONS, PrismInput, RoundInput,
    SphereInput, TorusInput, bounding_box, cone, cylinder, exactness, faces, make_box, mirror,
    move_by, pattern, prism, rotate, scale, six_volume, sphere, torus, twice_area, validity,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{NameId, NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::Seed;
use geom_test::contract::{
    Done, Subject, Trigger, accept_holds, errors_tell_the_truth, exactness_holds, example_holds,
    range_holds, remedy_holds, transactional,
};
use geom_test::history::check_laws;
use geom_test::rng::SplitMix64;
use geom_topo::ix::Arena;
use geom_topo::{Body, Builder};

/// A call, its answer the text of its whole result.
type S = Subject<'static, String>;

/// The largest coordinate on the grid (chapter 6.2): results reach it and no further.
const EDGE: i64 = (1 << 40) - 1;

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

/// Every name a body carries.
fn names(body: &Body) -> NameSet {
    let n = body.names().unwrap();
    NameSet::new(
        n.regions
            .iter()
            .chain(&n.faces)
            .chain(&n.edges)
            .chain(&n.vertices)
            .map(|k| NameId::from_bytes(k.0)),
    )
    .unwrap()
}

fn call<R: Debug>(
    label: impl Into<String>,
    run: impl Fn(&OpContext<'_>) -> Result<OpOutput<R>, OpError> + 'static,
) -> S {
    Subject::new(label, move |c| {
        run(c).map(|o| Done {
            answer: format!("{:?} {:?}", o.result, o.diagnostics),
            exactness: o.exactness,
        })
    })
}

/// A primitive: besides its contract, its history keeps the laws over no inputs.
fn made(
    label: impl Into<String>,
    run: impl Fn(&OpContext<'_>) -> Result<OpOutput<Made>, OpError> + 'static,
) -> S {
    call(label, move |c| {
        let out = run(c)?;
        check_laws(&out.result.history, &NameSet::new([]).unwrap()).unwrap();
        Ok(out)
    })
}

/// A body a transform takes, shared by the calls on it, with a copy to hold it to.
#[derive(Clone)]
struct Held {
    body: Arc<Body>,
    first: Arc<Body>,
    exactness: Exactness,
}

impl Held {
    fn new(out: OpOutput<Made>) -> Self {
        Self {
            body: Arc::new(out.result.body.clone()),
            first: Arc::new(out.result.body),
            exactness: out.exactness,
        }
    }

    fn moved(&self, accept: Option<Exactness>) -> Moved<'_> {
        Moved {
            body: &self.body,
            exactness: self.exactness,
            accept,
        }
    }

    /// A call on this body, whose input must be the same after it.
    fn on<R: Debug>(
        &self,
        label: impl Into<String>,
        run: impl Fn(&OpContext<'_>, &Held) -> Result<OpOutput<R>, OpError> + 'static,
    ) -> S {
        let (held, watched) = (self.clone(), self.clone());
        call(label, move |c| run(c, &held)).watching(move || *watched.body == *watched.first)
    }

    /// A transform of this body: its history also keeps the laws over the body's names.
    fn transform(
        &self,
        label: impl Into<String>,
        run: impl Fn(&OpContext<'_>, &Held) -> Result<OpOutput<Made>, OpError> + 'static,
    ) -> S {
        let inputs = names(&self.body);
        self.on(label, move |c, held| {
            let out = run(c, held)?;
            check_laws(&out.result.history, &inputs).unwrap();
            Ok(out)
        })
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

fn unit_box() -> Held {
    Held::new(
        make_box(
            &ctx(),
            &BoxInput {
                min: p(0, 0, 0),
                size: [Steps(1); 3],
            },
        )
        .unwrap(),
    )
}

fn a_prism() -> PrismInput {
    PrismInput {
        base: vec![p(0, 0, 0), p(10, 0, 0), p(12, 7, 0), p(3, 11, 0)],
        height: Steps(9),
    }
}

fn round(radius: u64, height: u64) -> RoundInput {
    RoundInput {
        base: p(0, 0, 0),
        radius: Steps(radius),
        height: Steps(height),
        step: GridStep::Q0,
    }
}

fn ball(center: P3, radius: u64) -> SphereInput {
    SphereInput {
        center,
        radius: Steps(radius),
        step: GridStep::Q0,
    }
}

fn ring(center: P3, major: u64, minor: u64) -> TorusInput {
    TorusInput {
        center,
        major: Steps(major),
        minor: Steps(minor),
        step: GridStep::Q0,
    }
}

/// The body with its names taken off: an operation that needs them refuses it.
fn unnamed() -> Held {
    let b = a_box();
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = Builder::edit(&b.result.body)
        .publish(b.result.body.kind(), b.result.body.outside(), &mut meter)
        .unwrap();
    Held {
        body: Arc::new(body.clone()),
        first: Arc::new(body),
        exactness: Exactness::Exact,
    }
}

/// A rotation that rounds, as a pattern's transform.
fn tilted(key: u8) -> Instance {
    Instance {
        key: [key; 16],
        transform: TypedTransform::rotation(Axis::Z, Rotation::direction(4, 3).unwrap()).unwrap(),
    }
}

fn shift(key: u8, x: i64) -> Instance {
    Instance {
        key: [key; 16],
        transform: TypedTransform::Exact(GridTransform::translation([x, 0, 0]).unwrap()),
    }
}

/// A pattern's transform from a matrix.
fn matrix(m: [[i64; 3]; 3]) -> Instance {
    Instance {
        key: [1; 16],
        transform: TypedTransform::from(RationalTransform::new(m, [0; 3], 1).unwrap()),
    }
}

/// The counts an example may state, from a result.
fn counts(body: &Body) -> Vec<(&'static str, u64)> {
    let a = body.arenas();
    let rows = |arena| u64::from(a.rows(arena));
    vec![
        ("face", rows(Arena::Face)),
        ("edge", rows(Arena::Edge)),
        ("vertex", rows(Arena::Vertex)),
    ]
}

/// A remedy that gives a number: the input, a value that fails, and the call at a value.
type Remedial = (&'static str, u64, Box<dyn Fn(u64) -> S>);

/// What one operation's laws are run on.
#[derive(Default)]
struct Driver {
    /// Calls that succeed, each with the exactness of its inputs.
    good: Vec<(S, Vec<Exactness>)>,
    /// Calls that fail, one or more per error the schema lists.
    triggers: Vec<Trigger<'static, String>>,
    /// Remedies that give a number: the input, a value that fails, and the call at a value.
    remedies: Vec<Remedial>,
    /// The call under an accepted class, where a less exact result needs one.
    accept: Option<Box<dyn Fn(Option<Exactness>) -> S>>,
    /// A call reaching the frame's edge, and one a step past it.
    range: Option<(S, S)>,
    /// For a query, which makes no geometry to pass the frame: a call on a body that reaches its
    /// edge, which must answer.
    edge: Option<S>,
    /// The schema's example: what its call gave.
    example: Option<(Vec<(&'static str, u64)>, Exactness)>,
}

fn trigger(detail: &'static str, subject: S) -> Trigger<'static, String> {
    Trigger { detail, subject }
}

fn drive_box() -> Driver {
    let sized = |size: [u64; 3]| {
        made(format!("make.box {size:?}"), move |c| {
            make_box(
                c,
                &BoxInput {
                    min: p(0, 0, 0),
                    size: size.map(Steps),
                },
            )
        })
    };
    let at = |x: i64| {
        made(format!("make.box at {x}"), move |c| {
            make_box(
                c,
                &BoxInput {
                    min: p(x, 0, 0),
                    size: [Steps(1); 3],
                },
            )
        })
    };
    let example = make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(4000), Steps(200), Steps(2700)],
        },
    )
    .unwrap();
    Driver {
        good: vec![(sized([10, 20, 30]), vec![]), (sized([1, 1, 1]), vec![])],
        triggers: vec![
            trigger("ZeroLength", sized([1, 0, 1])),
            trigger("OutOfRange", at(EDGE)),
        ],
        remedies: vec![
            ("size.x", 0, Box::new(move |n| sized([n, 1, 1]))),
            ("size.y", 0, Box::new(move |n| sized([1, n, 1]))),
            ("size.z", 0, Box::new(move |n| sized([1, 1, n]))),
        ],
        range: Some((at(EDGE - 1), at(EDGE))),
        edge: None,
        example: Some((counts(&example.result.body), example.exactness)),
        ..Driver::default()
    }
}

fn drive_prism() -> Driver {
    let on = |base: Vec<P3>, height: u64| {
        made(format!("make.prism {} corners", base.len()), move |c| {
            prism(
                c,
                &PrismInput {
                    base: base.clone(),
                    height: Steps(height),
                },
            )
        })
    };
    let high = move |height: u64| on(a_prism().base, height);
    let reaching = |x: i64| on(vec![p(x - 10, 0, 0), p(x, 0, 0), p(x, 10, 0)], 1);
    let corners = move |n: u64| {
        let all = [p(0, 0, 0), p(10, 0, 0), p(10, 10, 0), p(0, 10, 0)];
        on(all[..usize::try_from(n).unwrap()].to_vec(), 1)
    };
    let example = prism(&ctx(), &a_prism()).unwrap();
    Driver {
        good: vec![(high(9), vec![]), (high(1), vec![])],
        triggers: vec![
            trigger("TooFewCorners", corners(2)),
            trigger(
                "BaseNotLevel",
                on(vec![p(0, 0, 0), p(9, 0, 0), p(0, 9, 1)], 1),
            ),
            trigger(
                "BaseNotConvex",
                on(vec![p(0, 0, 0), p(0, 9, 0), p(9, 0, 0)], 1),
            ),
            trigger("ZeroLength", high(0)),
            trigger(
                "OutOfRange",
                on(vec![p(0, 0, EDGE), p(9, 0, EDGE), p(0, 9, EDGE)], 1),
            ),
        ],
        remedies: vec![
            ("height", 0, Box::new(high)),
            ("base", 2, Box::new(corners)),
        ],
        range: Some((reaching(EDGE), {
            let base = vec![p(0, 0, EDGE), p(9, 0, EDGE), p(0, 9, EDGE)];
            on(base, 1)
        })),
        edge: None,
        example: Some((counts(&example.result.body), example.exactness)),
        ..Driver::default()
    }
}

/// A cylinder or a cone: the two take one input and refuse alike.
fn drive_round(
    name: &'static str,
    op: fn(&OpContext<'_>, &RoundInput) -> Result<OpOutput<Made>, OpError>,
    example: (u64, u64),
) -> Driver {
    let sized = move |radius: u64, height: u64| {
        made(format!("{name} {radius} {height}"), move |c| {
            op(c, &round(radius, height))
        })
    };
    let at = move |x: i64, radius: u64| {
        made(format!("{name} at {x}"), move |c| {
            op(
                c,
                &RoundInput {
                    base: p(x, 0, 0),
                    ..round(radius, 1)
                },
            )
        })
    };
    let shown = op(&ctx(), &round(example.0, example.1)).unwrap();
    Driver {
        good: vec![(sized(5, 10), vec![]), (sized(1, 1), vec![])],
        triggers: vec![
            trigger("ZeroLength", sized(0, 1)),
            trigger("OutOfRange", at(EDGE, 1)),
        ],
        remedies: vec![
            ("radius", 0, Box::new(move |n| sized(n, 1))),
            ("height", 0, Box::new(move |n| sized(1, n))),
        ],
        range: Some((at(EDGE - 5, 5), at(EDGE - 4, 5))),
        edge: None,
        example: Some((counts(&shown.result.body), shown.exactness)),
        ..Driver::default()
    }
}

fn drive_sphere() -> Driver {
    let on = |center: P3, radius: u64| {
        made(format!("make.sphere {radius}"), move |c| {
            sphere(c, &ball(center, radius))
        })
    };
    let shown = sphere(&ctx(), &ball(p(1, 2, 3), 5)).unwrap();
    Driver {
        good: vec![(on(p(1, 2, 3), 5), vec![]), (on(p(0, 0, 0), 1), vec![])],
        triggers: vec![
            trigger("ZeroLength", on(p(0, 0, 0), 0)),
            trigger("OutOfRange", on(p(EDGE, 0, 0), 1)),
        ],
        remedies: vec![("radius", 0, Box::new(move |n| on(p(0, 0, 0), n)))],
        range: Some((on(p(EDGE - 5, 0, 0), 5), on(p(EDGE - 4, 0, 0), 5))),
        edge: None,
        example: Some((counts(&shown.result.body), shown.exactness)),
        ..Driver::default()
    }
}

fn drive_torus() -> Driver {
    let on = |center: P3, major: u64, minor: u64| {
        made(format!("make.torus {major} {minor}"), move |c| {
            torus(c, &ring(center, major, minor))
        })
    };
    let shown = torus(&ctx(), &ring(p(0, 0, 0), 10, 3)).unwrap();
    Driver {
        good: vec![
            (on(p(0, 0, 0), 10, 3), vec![]),
            (on(p(0, 0, 0), 2, 1), vec![]),
        ],
        triggers: vec![
            trigger("MinorNotBelowMajor", on(p(0, 0, 0), 3, 3)),
            trigger("ZeroLength", on(p(0, 0, 0), 3, 0)),
            trigger("OutOfRange", on(p(EDGE, 0, 0), 2, 1)),
        ],
        remedies: vec![
            ("minor", 10, Box::new(move |n| on(p(0, 0, 0), 10, n))),
            ("minor", 0, Box::new(move |n| on(p(0, 0, 0), 10, n))),
        ],
        range: Some((on(p(EDGE - 13, 0, 0), 10, 3), on(p(EDGE - 12, 0, 0), 10, 3))),
        edge: None,
        example: Some((counts(&shown.result.body), shown.exactness)),
        ..Driver::default()
    }
}

/// The bodies a transform's good calls run on.
fn bodies() -> Vec<Held> {
    vec![
        Held::new(a_box()),
        Held::new(cylinder(&ctx(), &round(5, 10)).unwrap()),
        Held::new(sphere(&ctx(), &ball(p(1, 2, 3), 5)).unwrap()),
    ]
}

fn drive_move() -> Driver {
    let by = |held: &Held, x: i64| {
        held.transform(format!("transform.move {x}"), move |c, h| {
            move_by(c, h.moved(None), [x, 0, 0])
        })
    };
    let shown = move_by(&ctx(), unit_box().moved(None), [1, 2, 3]).unwrap();
    let edge = unit_box();
    Driver {
        good: bodies()
            .iter()
            .map(|h| (by(h, 7), vec![h.exactness]))
            .collect(),
        triggers: vec![
            trigger("OutOfRange", by(&edge, EDGE)),
            trigger(
                "BodyNotNamed",
                unnamed().on("transform.move unnamed", |c, h| {
                    move_by(c, h.moved(None), [1, 0, 0])
                }),
            ),
        ],
        range: Some((by(&edge, EDGE - 1), by(&edge, EDGE))),
        edge: None,
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

fn drive_mirror() -> Driver {
    let across = |held: &Held, x: i64| {
        held.transform(format!("transform.mirror {x}"), move |c, h| {
            mirror(c, h.moved(None), Axis::X, p(x, 0, 0))
        })
    };
    let shown = mirror(&ctx(), unit_box().moved(None), Axis::X, p(3, -2, 1)).unwrap();
    // A unit box from x = 1 mirrors to 2c − 2 and 2c − 1; one from x = 0 to 2c − 1 and 2c.
    let far = Held::new(
        make_box(
            &ctx(),
            &BoxInput {
                min: p(1, 0, 0),
                size: [Steps(1); 3],
            },
        )
        .unwrap(),
    );
    let half = 1i64 << 39;
    Driver {
        good: bodies()
            .iter()
            .map(|h| (across(h, 3), vec![h.exactness]))
            .collect(),
        triggers: vec![
            trigger("OutOfRange", across(&unit_box(), half)),
            trigger(
                "BodyNotNamed",
                unnamed().on("transform.mirror unnamed", |c, h| {
                    mirror(c, h.moved(None), Axis::X, p(0, 0, 0))
                }),
            ),
        ],
        range: Some((across(&far, half), across(&unit_box(), half))),
        edge: None,
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

fn drive_rotate() -> Driver {
    let quarter = |held: &Held, cy: i64| {
        held.transform(format!("transform.rotate about {cy}"), move |c, h| {
            rotate(
                c,
                h.moved(None),
                Axis::Z,
                Rotation::quarter_turns(1),
                p(EDGE, cy, 0),
            )
        })
    };
    let turned = |held: &Held, accept: Option<Exactness>| {
        held.transform("transform.rotate by (4, 3)", move |c, h| {
            rotate(
                c,
                h.moved(accept),
                Axis::Z,
                Rotation::direction(4, 3).unwrap(),
                p(0, 0, 0),
            )
        })
    };
    let bx = Held::new(a_box());
    let flat = Held::new(prism(&ctx(), &a_prism()).unwrap());
    let shown = rotate(
        &ctx(),
        unit_box().moved(None),
        Axis::Z,
        Rotation::quarter_turns(1),
        p(1, 1, 0),
    )
    .unwrap();
    let mut good: Vec<(S, Vec<Exactness>)> = bodies()
        .iter()
        .map(|h| {
            (
                h.transform("transform.rotate quarter", |c, h| {
                    rotate(
                        c,
                        h.moved(None),
                        Axis::Z,
                        Rotation::quarter_turns(1),
                        p(1, 1, 0),
                    )
                }),
                vec![h.exactness],
            )
        })
        .collect();
    good.push((turned(&bx, Some(certified(64))), vec![Exactness::Exact]));
    let accepting = bx.clone();
    Driver {
        good,
        triggers: vec![
            trigger("OutOfRange", quarter(&unit_box(), 1)),
            trigger(
                "BodyNotNamed",
                unnamed().on("transform.rotate unnamed", |c, h| {
                    rotate(
                        c,
                        h.moved(None),
                        Axis::Z,
                        Rotation::quarter_turns(1),
                        p(0, 0, 0),
                    )
                }),
            ),
            trigger("NotAccepted", turned(&bx, None)),
            trigger("NotCertifiedYet", turned(&flat, Some(certified(64)))),
        ],
        accept: Some(Box::new(move |class| turned(&accepting, class))),
        range: Some((quarter(&unit_box(), 0), quarter(&unit_box(), 1))),
        edge: None,
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

fn drive_scale() -> Driver {
    let by = |held: &Held, factors: [(u64, u64); 3], accept: Option<Exactness>| {
        held.transform(format!("transform.scale {factors:?}"), move |c, h| {
            scale(
                c,
                h.moved(accept),
                factors.map(|(n, d)| Scale::new(n, d).unwrap()),
                p(0, 0, 0),
            )
        })
    };
    let wide = |n: u64| [(n, 1), (1, 1), (1, 1)];
    let bx = Held::new(a_box());
    let ballish = Held::new(sphere(&ctx(), &ball(p(1, 2, 3), 5)).unwrap());
    let cyl = Held::new(cylinder(&ctx(), &round(5, 10)).unwrap());
    let flat = Held::new(prism(&ctx(), &a_prism()).unwrap());
    let shown = scale(
        &ctx(),
        unit_box().moved(None),
        [Scale::new(3, 1).unwrap(); 3],
        p(1, 1, 1),
    )
    .unwrap();
    let accepting = ballish.clone();
    Driver {
        good: vec![
            (by(&bx, [(2, 1); 3], None), vec![bx.exactness]),
            (by(&bx, wide(3), None), vec![bx.exactness]),
            (
                by(&ballish, [(2, 1); 3], Some(certified(2))),
                vec![ballish.exactness],
            ),
        ],
        triggers: vec![
            trigger("OutOfRange", by(&unit_box(), wide(1 << 40), None)),
            trigger(
                "BodyNotNamed",
                unnamed().on("transform.scale unnamed", |c, h| {
                    scale(c, h.moved(None), [Scale::ONE; 3], p(0, 0, 0))
                }),
            ),
            trigger("NotAccepted", by(&ballish, [(2, 1); 3], None)),
            trigger("KindNotCertified", by(&cyl, wide(2), Some(certified(64)))),
            trigger(
                "NotCertifiedYet",
                by(&flat, [(1, 3), (1, 7), (1, 1)], Some(certified(64))),
            ),
        ],
        accept: Some(Box::new(move |class| by(&accepting, [(2, 1); 3], class))),
        range: Some((
            by(&unit_box(), wide(u64::try_from(EDGE).unwrap()), None),
            by(&unit_box(), wide(1 << 40), None),
        )),
        edge: None,
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

fn drive_pattern() -> Driver {
    let of = |held: &Held, accept: Option<Exactness>, instances: Vec<Instance>| {
        held.on(
            "transform.pattern",
            move |c, h| -> Result<OpOutput<Copies>, OpError> {
                pattern(c, h.moved(accept), &instances)
            },
        )
    };
    let bx = Held::new(a_box());
    let cyl = Held::new(cylinder(&ctx(), &round(5, 10)).unwrap());
    let flat = Held::new(prism(&ctx(), &a_prism()).unwrap());
    let two = || vec![shift(1, 20), shift(2, 40)];
    let shown = pattern(&ctx(), unit_box().moved(None), &two()).unwrap();
    let accepting = bx.clone();
    Driver {
        good: bodies()
            .iter()
            .map(|h| (of(h, None, two()), vec![h.exactness]))
            .collect(),
        triggers: vec![
            trigger("EmptyPattern", of(&bx, None, vec![])),
            trigger("RepeatedKey", of(&bx, None, vec![shift(1, 1), shift(1, 2)])),
            trigger(
                "SingularTransform",
                of(&cyl, None, vec![matrix([[1, 0, 0], [0, 1, 0], [0, 0, 0]])]),
            ),
            trigger("OutOfRange", of(&unit_box(), None, vec![shift(1, EDGE)])),
            trigger("BodyNotNamed", of(&unnamed(), None, vec![shift(1, 1)])),
            trigger("NotAccepted", of(&bx, None, vec![tilted(1)])),
            trigger(
                "KindNotCertified",
                of(
                    &cyl,
                    Some(certified(64)),
                    vec![matrix([[2, 0, 0], [0, 1, 0], [0, 0, 1]])],
                ),
            ),
            trigger(
                "NotCertifiedYet",
                of(&flat, Some(certified(64)), vec![tilted(1)]),
            ),
        ],
        accept: Some(Box::new(move |class| {
            of(&accepting, class, vec![tilted(1)])
        })),
        range: Some((
            of(&unit_box(), None, vec![shift(1, EDGE - 1)]),
            of(&unit_box(), None, vec![shift(1, EDGE)]),
        )),
        edge: None,
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

/// Every driver, by the name its operation registers.
fn drivers() -> Vec<(&'static str, Driver)> {
    vec![
        ("make.box", drive_box()),
        ("make.cone", drive_round("make.cone", cone, (6, 12))),
        (
            "make.cylinder",
            drive_round("make.cylinder", cylinder, (5, 10)),
        ),
        ("make.prism", drive_prism()),
        ("make.sphere", drive_sphere()),
        ("make.torus", drive_torus()),
        ("transform.mirror", drive_mirror()),
        ("transform.move", drive_move()),
        ("transform.pattern", drive_pattern()),
        ("transform.rotate", drive_rotate()),
        ("transform.scale", drive_scale()),
        ("query.area", drive_area()),
        ("query.bounding_box", drive_bounding_box()),
        ("query.exactness", drive_exactness()),
        ("query.faces", drive_faces()),
        ("query.validity", drive_validity()),
        ("query.volume", drive_volume()),
    ]
}

/// The box of 10 by 20 by 30 at the origin, the query schemas' examples' body.
fn origin_box() -> Held {
    Held::new(
        make_box(
            &ctx(),
            &BoxInput {
                min: p(0, 0, 0),
                size: [Steps(10), Steps(20), Steps(30)],
            },
        )
        .unwrap(),
    )
}

/// A box of 10 by 20 by 30 whose far corner is the frame's edge.
fn edge_box() -> Held {
    Held::new(
        make_box(
            &ctx(),
            &BoxInput {
                min: p(EDGE - 10, EDGE - 20, EDGE - 30),
                size: [Steps(10), Steps(20), Steps(30)],
            },
        )
        .unwrap(),
    )
}

/// The box with its rows edited and published again, as a file or a broken operation could hand
/// a query; `kind` the kind it is published as.
fn edited(kind: geom_topo::BodyKind, edit: impl FnOnce(&mut geom_topo::Arenas)) -> Arc<Body> {
    let body = origin_box().body;
    let mut builder = Builder::edit(&body);
    edit(&mut builder.arenas);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    Arc::new(builder.publish(kind, body.outside(), &mut meter).unwrap())
}

fn face(row: u32) -> geom_topo::FaceIx {
    geom_topo::Ix::new(row).unwrap()
}

/// A face list that comes back on itself: the second face's next is the first.
fn ringed() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        a.faces.next.set(face(1), Some(face(0))).unwrap();
    })
}

/// A loop whose fins never come back round: the first fin's next is a fin of another face.
fn fin_ring() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        let fin = |row: u32| -> geom_topo::FinIx { geom_topo::Ix::new(row).unwrap() };
        let last = a.rows(Arena::Fin) - 1;
        a.fins.next.set(fin(0), fin(last)).unwrap();
    })
}

/// A face's list of loops that comes back on itself: its loop's next is that loop.
fn loop_ring() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        let first: geom_topo::LoopIx = geom_topo::Ix::new(0).unwrap();
        a.loops.next.set(first, Some(first)).unwrap();
    })
}

/// Every edge on a circle in place of its line.
fn circled() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        let frame = geom_curve::Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        for row in 0..a.rows(Arena::Curve) {
            let c = geom_curve::Circle::new([0.0; 3], 5.0, frame, (0.0, 1.0)).unwrap();
            let ix = geom_topo::Ix::new(row).unwrap();
            a.curves.set(ix, geom_curve::CurveKind::Circle(c)).unwrap();
        }
    })
}

/// Every face on a sphere in place of its plane, its edges still straight.
fn rounded() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        let frame = geom_curve::Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let ball =
            geom_surface::Sphere::new([0.0; 3], 50.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
        for row in 0..a.rows(Arena::Surface) {
            let ix = geom_topo::Ix::new(row).unwrap();
            a.surfaces
                .set(ix, geom_surface::SurfaceKind::Sphere(ball.clone()))
                .unwrap();
        }
    })
}

/// Every face without its loops.
fn loopless() -> Arc<Body> {
    edited(geom_topo::BodyKind::Solid, |a| {
        for row in 0..a.rows(Arena::Face) {
            a.faces.first_loop.set(face(row), None).unwrap();
        }
    })
}

/// The box published as a sheet.
fn sheet() -> Arc<Body> {
    edited(geom_topo::BodyKind::Sheet, |_| {})
}

/// A query on a body handed in, whose input must be the same after it.
fn asked<R: Debug>(
    label: &str,
    body: Arc<Body>,
    run: impl Fn(&OpContext<'_>, &Body) -> Result<OpOutput<R>, OpError> + 'static,
) -> S {
    let watched = (Arc::clone(&body), Arc::new((*body).clone()));
    call(label.to_owned(), move |c| run(c, &body)).watching(move || *watched.0 == *watched.1)
}

/// The row of the box's face on z = 0, its min.z face.
fn min_z(body: &Body) -> geom_topo::FaceIx {
    let a = body.arenas();
    (0..a.rows(Arena::Face))
        .map(face)
        .find(|&f| {
            let b = a.faces.bound.get(f).unwrap();
            b.min().z.get() == 0 && b.max().z.get() == 0
        })
        .unwrap()
}

/// What a query's driver shares: no remedy gives a number, no class needs accepting, and nothing
/// a query gives is geometry that could pass the frame, so its half of the range law is a call
/// on a body that reaches the edge.
fn query(
    good: Vec<(S, Vec<Exactness>)>,
    triggers: Vec<Trigger<'static, String>>,
    edge: S,
    example: Exactness,
) -> Driver {
    Driver {
        good,
        triggers,
        remedies: Vec::new(),
        accept: None,
        range: None,
        edge: Some(edge),
        example: Some((Vec::new(), example)),
    }
}

fn drive_area() -> Driver {
    let held = origin_box();
    let at = min_z(&held.body);
    let example = held.on("query.area of min.z, Exact", move |c, h| {
        let out = twice_area(c, &h.body, at, Exactness::Exact)?;
        assert_eq!(out.result, [0, 0, -400]);
        Ok(out)
    });
    let recorded = held.on("query.area of min.z, Certified", move |c, h| {
        twice_area(c, &h.body, at, certified(2))
    });
    let edge = edge_box().on("query.area at the edge", |c, h| {
        twice_area(c, &h.body, face(0), Exactness::Exact)
    });
    let area = |label: &str, body: Arc<Body>, row: u32| {
        asked(label, body, move |c, b| {
            twice_area(c, b, face(row), Exactness::Exact)
        })
    };
    query(
        vec![
            (example, vec![Exactness::Exact]),
            (recorded, vec![certified(2)]),
        ],
        vec![
            trigger(
                "Malformed",
                area("query.area of no such face", Arc::clone(&held.body), 99),
            ),
            trigger(
                "CurvedEdge",
                area("query.area across circles", circled(), 0),
            ),
            trigger("CurvedFace", area("query.area on a sphere", rounded(), 0)),
            trigger(
                "NoLoops",
                area("query.area of a face with no loop", loopless(), 0),
            ),
        ],
        edge,
        Exactness::Exact,
    )
}

fn drive_bounding_box() -> Driver {
    let held = origin_box();
    let example = held.on("query.bounding_box, Exact", |c, h| {
        let out = bounding_box(c, &h.body, Exactness::Exact)?;
        let b = out.result.as_ref().unwrap();
        assert!(b.tight);
        assert_eq!((b.bound.min(), b.bound.max()), (p(0, 0, 0), p(10, 20, 30)));
        Ok(out)
    });
    let recorded = held.on("query.bounding_box, Certified", |c, h| {
        bounding_box(c, &h.body, certified(2))
    });
    let edge = edge_box().on("query.bounding_box at the edge", |c, h| {
        bounding_box(c, &h.body, Exactness::Exact)
    });
    query(
        vec![
            (example, vec![Exactness::Exact]),
            (recorded, vec![certified(2)]),
        ],
        vec![trigger(
            "Malformed",
            asked(
                "query.bounding_box of a loop that never closes",
                fin_ring(),
                |c, b| bounding_box(c, b, Exactness::Exact),
            ),
        )],
        edge,
        Exactness::Exact,
    )
}

fn drive_volume() -> Driver {
    let held = origin_box();
    let example = held.on("query.volume, Exact", |c, h| {
        let out = six_volume(c, &h.body, Exactness::Exact)?;
        assert_eq!(out.result, geom_num::wide::I256::from_i128(36_000));
        Ok(out)
    });
    let recorded = held.on("query.volume, Certified", |c, h| {
        six_volume(c, &h.body, certified(2))
    });
    let edge = edge_box().on("query.volume at the edge", |c, h| {
        six_volume(c, &h.body, Exactness::Exact)
    });
    let volume = |label: &str, body: Arc<Body>| {
        asked(label, body, |c, b| six_volume(c, b, Exactness::Exact))
    };
    query(
        vec![
            (example, vec![Exactness::Exact]),
            (recorded, vec![certified(2)]),
        ],
        vec![
            trigger(
                "Malformed",
                volume("query.volume of a ring of faces", ringed()),
            ),
            trigger(
                "CurvedEdge",
                volume("query.volume across circles", circled()),
            ),
            trigger("CurvedFace", volume("query.volume on spheres", rounded())),
            trigger(
                "NoLoops",
                volume("query.volume of faces with no loop", loopless()),
            ),
            trigger("NotSolid", volume("query.volume of a sheet", sheet())),
        ],
        edge,
        Exactness::Exact,
    )
}

fn drive_exactness() -> Driver {
    let held = Held::new(cylinder(&ctx(), &round(5, 10)).unwrap());
    let recorded = held.exactness;
    let example = held.on("query.exactness of a cylinder", move |c, h| {
        let out = exactness(c, &h.body, recorded)?;
        let r = &out.result;
        assert_eq!(
            (
                r.flat_faces,
                r.curved_faces,
                r.straight_edges,
                r.curved_edges
            ),
            (2, 1, 0, 2)
        );
        assert!(r.report.passed());
        Ok(out)
    });
    let flat = origin_box().on("query.exactness of a box", |c, h| {
        exactness(c, &h.body, Exactness::Exact)
    });
    let edge = edge_box().on("query.exactness at the edge", |c, h| {
        exactness(c, &h.body, Exactness::Exact)
    });
    query(
        vec![(example, vec![recorded]), (flat, vec![Exactness::Exact])],
        Vec::new(),
        edge,
        recorded,
    )
}

fn drive_faces() -> Driver {
    let made = origin_box_made();
    let history = Arc::new(made.result.history.clone());
    let held = Held::new(made);
    let roles = Arc::clone(&history);
    let example = held.on("query.faces of a box", move |c, h| {
        let out = faces(c, &h.body, &[&roles], Exactness::Exact)?;
        let mut named: Vec<String> = out
            .result
            .iter()
            .flat_map(|f| f.roles.iter().map(|r| format!("{r:?}")))
            .collect();
        named.sort();
        assert_eq!(out.result.len(), 6);
        for side in ["min.x", "max.x", "min.y", "max.y", "min.z", "max.z"] {
            assert!(named.iter().any(|r| r.contains(side)), "{side}: {named:?}");
        }
        Ok(out)
    });
    let bare = held.on("query.faces with no history", |c, h| {
        faces(c, &h.body, &[], certified(2))
    });
    let edge = edge_box().on("query.faces at the edge", |c, h| {
        faces(c, &h.body, &[], Exactness::Exact)
    });
    query(
        vec![
            (example, vec![Exactness::Exact]),
            (bare, vec![certified(2)]),
        ],
        vec![trigger(
            "Malformed",
            asked("query.faces of a ring of loops", loop_ring(), |c, b| {
                faces(c, b, &[], Exactness::Exact)
            }),
        )],
        edge,
        Exactness::Exact,
    )
}

fn drive_validity() -> Driver {
    let held = origin_box();
    let example = held.on("query.validity of a box, Full", |c, h| {
        let out = validity(c, &h.body, Exactness::Exact, Level::Full)?;
        assert!(out.result.passed());
        Ok(out)
    });
    let cheap = held.on("query.validity of a box, Cheap", |c, h| {
        validity(c, &h.body, certified(2), Level::Cheap)
    });
    let edge = edge_box().on("query.validity at the edge", |c, h| {
        validity(c, &h.body, Exactness::Exact, Level::Full)
    });
    query(
        vec![
            (example, vec![Exactness::Exact]),
            (cheap, vec![certified(2)]),
        ],
        Vec::new(),
        edge,
        Exactness::Exact,
    )
}

/// The examples' box as made, with its history.
fn origin_box_made() -> OpOutput<Made> {
    make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(10), Steps(20), Steps(30)],
        },
    )
    .unwrap()
}

#[test]
fn every_registered_operation_has_a_driver() {
    let registered: BTreeSet<&str> = OPERATIONS.iter().map(|o| o.schema.name).collect();
    let driven: BTreeSet<&str> = drivers().iter().map(|(name, _)| *name).collect();
    assert_eq!(registered, driven);
}

/// Each driver with its operation's schema.
fn each(mut law: impl FnMut(&'static geom_op::OpSchema, &Driver)) {
    for (name, driver) in &drivers() {
        let op = OPERATIONS.iter().find(|o| o.schema.name == *name).unwrap();
        law(op.schema, driver);
    }
}

#[test]
fn every_operation_is_transactional_under_cancel_and_budget_injection() {
    let seed = geom_test::seed::resolve("contract_laws", Seed(0x4731_3033)).unwrap();
    let mut rng = SplitMix64::new(seed);
    each(|schema, driver| {
        assert_ne!(driver.good.len(), 0, "{}", schema.name);
        for (subject, _) in &driver.good {
            transactional(subject, &mut rng).unwrap();
        }
        for t in &driver.triggers {
            transactional(&t.subject, &mut rng).unwrap();
        }
    });
}

#[test]
fn every_error_a_schema_lists_is_triggered_and_no_other() {
    each(|schema, driver| errors_tell_the_truth(schema, &driver.triggers).unwrap());
}

#[test]
fn every_remedy_works_at_its_number_and_fails_one_step_beyond_it() {
    each(|_, driver| {
        for (input, failing, at) in &driver.remedies {
            remedy_holds(input, *failing, &**at).unwrap();
        }
        if let Some(at) = &driver.accept {
            accept_holds(&**at).unwrap();
        }
    });
}

#[test]
fn exactness_is_never_lost_unasked() {
    each(|schema, driver| {
        for (subject, inputs) in &driver.good {
            let done = subject.once().unwrap();
            exactness_holds(subject.label(), inputs, done.exactness).unwrap();
            let class = schema.classes.iter().any(|c| {
                matches!(
                    (c, done.exactness),
                    (geom_op::Class::Exact, Exactness::Exact)
                        | (geom_op::Class::Certified, Exactness::Certified { .. })
                        | (geom_op::Class::Approximate, Exactness::Approximate { .. })
                )
            });
            assert!(class, "{}: {:?}", subject.label(), done.exactness);
        }
    });
}

#[test]
fn the_range_holds_at_the_frames_edge() {
    each(|schema, driver| match (&driver.range, &driver.edge) {
        (Some((inside, outside)), None) => range_holds(inside, outside).unwrap(),
        // A query's answer is a number or a report, never geometry past the frame: its half of
        // the law is that it answers on a body reaching the frame's edge.
        (None, Some(edge)) if schema.family == geom_op::Family::Query => {
            edge.once().unwrap();
        }
        _ => panic!("{}: no range to hold", schema.name),
    });
}

#[test]
fn every_schema_example_runs() {
    each(|schema, driver| {
        let (counts, class) = driver.example.as_ref().unwrap();
        example_holds(schema, counts, *class).unwrap();
    });
}
