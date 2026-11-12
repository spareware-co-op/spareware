//! Chapter 28.2's contract on every operation the facade registers (piece G103).
//!
//! What this file proves: each operation in [`OPERATIONS`] has a [`Driver`], and each driver's calls
//! keep `geom_test::contract`'s laws. An operation added to the registry without a driver fails
//! `every_registered_operation_has_a_driver`, so the laws reach it from the day it lands; each
//! operation piece adds its row to [`drivers`]. No body holds a tolerance until chapter 6.5's
//! healing lands, so "No tolerance grows" joins here with the first operation that takes one; the
//! law itself is seeded in `geom-test/tests/contract_kit.rs`.
//!
//! The queries of piece G102 are held to the same laws, on bodies recorded at every class: a query
//! reads its body and changes nothing (each call watches its input), answers the same twice and
//! stops cleanly under every cancellation and budget, gives each error its schema lists, states the
//! class its body is recorded with, no more and no less, and runs its schema's example. A query
//! makes no coordinate, so for the range law it answers a body that fills the frame instead.
//!
//! Covers: `make.box`, `make.cone`, `make.cylinder`, `make.prism`, `make.sphere`, `make.torus`,
//! `query.area`, `query.bounding_box`, `query.exactness`, `query.faces`, `query.validity`,
//! `query.volume`, `transform.mirror`, `transform.move`, `transform.pattern`, `transform.rotate`,
//! `transform.scale`.

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
use geom_op::history::History;
use geom_op::{ErrorKind, Family, NameId, NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::Seed;
use geom_test::contract::{
    Done, Subject, Trigger, accept_holds, errors_tell_the_truth, exactness_holds, example_holds,
    range_holds, remedy_holds, transactional,
};
use geom_test::history::check_laws;
use geom_test::rng::SplitMix64;
use geom_topo::ix::Arena;
use geom_topo::{Body, BodyKind, Builder, Ix, SurfaceKind};

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
    /// The history of the operation that made it, which `query.faces` reads roles from.
    history: Arc<History>,
}

impl Held {
    fn new(out: OpOutput<Made>) -> Self {
        Self {
            body: Arc::new(out.result.body.clone()),
            first: Arc::new(out.result.body),
            exactness: out.exactness,
            history: Arc::new(out.result.history),
        }
    }

    /// The same body recorded with `exactness`: a body does not hold its own class (24.4), so a
    /// query is told it and must state it back.
    fn recorded(self, exactness: Exactness) -> Self {
        Self { exactness, ..self }
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
        history: Arc::new(b.result.history),
    }
}

/// The box with `edit` made to its rows, published as `kind` without its names: a body a query
/// must refuse. Publishing checks that every link stays inside its arena, not that a list ends or
/// a face has a loop, which is what the queries' own errors are for.
fn edited(kind: BodyKind, edit: impl FnOnce(&mut Builder)) -> Held {
    let b = a_box();
    let mut builder = Builder::edit(&b.result.body);
    edit(&mut builder);
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let body = builder
        .publish(kind, b.result.body.outside(), &mut meter)
        .unwrap();
    Held {
        body: Arc::new(body.clone()),
        first: Arc::new(body),
        exactness: Exactness::Exact,
        history: Arc::new(b.result.history),
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
    /// For a query, which makes no coordinate and so has no step past the frame: a call on a body
    /// that reaches the frame's edge on every side.
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
    let cyl = Held::new(cylinder(&ctx(), &round(5, 10)).unwrap());
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
            // A flat body's rounded corners are certified since G129's meeting points; curved
            // faces moved by a deviation still wait for the checker of piece G177.
            trigger("NotCertifiedYet", turned(&cyl, Some(certified(64)))),
        ],
        accept: Some(Box::new(move |class| turned(&accepting, class))),
        range: Some((quarter(&unit_box(), 0), quarter(&unit_box(), 1))),
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
            // A uniform scale keeps circles, and the cylinder's curved faces it moves by a
            // deviation wait for piece G177's checker (a flat body's rounded corners no longer do).
            trigger(
                "NotCertifiedYet",
                by(&cyl, [(1, 3); 3], Some(certified(64))),
            ),
        ],
        accept: Some(Box::new(move |class| by(&accepting, [(2, 1); 3], class))),
        range: Some((
            by(&unit_box(), wide(u64::try_from(EDGE).unwrap()), None),
            by(&unit_box(), wide(1 << 40), None),
        )),
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
            // Curved faces moved by a deviation wait for piece G177's checker; a flat body's
            // rounded corners are certified since G129's meeting points.
            trigger(
                "NotCertifiedYet",
                of(&cyl, Some(certified(64)), vec![tilted(1)]),
            ),
        ],
        accept: Some(Box::new(move |class| {
            of(&accepting, class, vec![tilted(1)])
        })),
        range: Some((
            of(&unit_box(), None, vec![shift(1, EDGE - 1)]),
            of(&unit_box(), None, vec![shift(1, EDGE)]),
        )),
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

/// The flat bodies every query answers, each recorded at a class: a unit box, a box, a prism, and
/// the box recorded Certified and Approximate, as a body read back from a file may be.
fn flat() -> Vec<Held> {
    vec![
        unit_box(),
        Held::new(a_box()),
        Held::new(prism(&ctx(), &a_prism()).unwrap()),
        Held::new(a_box()).recorded(certified(3)),
        Held::new(a_box()).recorded(Exactness::Approximate {
            deviation: Steps(5),
            step: GridStep::Q0,
        }),
    ]
}

/// The curved primitives, which the queries of a body's whole answer and the flat ones refuse.
fn curved() -> Vec<Held> {
    vec![
        Held::new(cylinder(&ctx(), &round(5, 10)).unwrap()),
        Held::new(cone(&ctx(), &round(6, 12)).unwrap()),
        Held::new(sphere(&ctx(), &ball(p(1, 2, 3), 5)).unwrap()),
        Held::new(torus(&ctx(), &ring(p(0, 0, 0), 10, 3)).unwrap()),
    ]
}

/// A box from −(2^40 − 1) to 2^40 − 1 on every axis: every vertex on the frame's edge.
fn filling() -> Held {
    Held::new(
        make_box(
            &ctx(),
            &BoxInput {
                min: p(-EDGE, -EDGE, -EDGE),
                size: [Steps(2 * EDGE.unsigned_abs()); 3],
            },
        )
        .unwrap(),
    )
}

/// The box with a face list that comes back on itself.
fn ringed_faces() -> Held {
    edited(BodyKind::Solid, |b| {
        b.arenas
            .faces
            .next
            .set(Ix::new(1).unwrap(), Some(Ix::new(0).unwrap()))
            .unwrap();
    })
}

/// The box with a loop list that comes back on itself.
fn ringed_loops() -> Held {
    edited(BodyKind::Solid, |b| {
        b.arenas
            .loops
            .next
            .set(Ix::new(0).unwrap(), Some(Ix::new(0).unwrap()))
            .unwrap();
    })
}

/// The box with its first face's loops taken off.
fn loopless() -> Held {
    edited(BodyKind::Solid, |b| {
        b.arenas
            .faces
            .first_loop
            .set(Ix::new(0).unwrap(), None)
            .unwrap();
    })
}

/// The box's rows published as a sheet.
fn sheet() -> Held {
    edited(BodyKind::Sheet, |_| {})
}

/// The first face of `held` on a plane, or the first not on one.
fn face_of(held: &Held, plane: bool) -> u32 {
    let a = held.body.arenas();
    (0..a.rows(Arena::Face))
        .find(|&row| {
            let face = a.faces.get(Ix::new(row).unwrap()).unwrap();
            matches!(a.surfaces.get(face.surface), Some(SurfaceKind::Plane(_))) == plane
        })
        .unwrap()
}

/// `query.area` of face `face` of `held`.
fn area_of(held: &Held, face: u32) -> S {
    held.on(format!("query.area face {face}"), move |c, h| {
        twice_area(c, &h.body, Ix::new(face).unwrap(), h.exactness)
    })
}

/// Every flat body's every face, with the class its body is recorded with.
fn areas() -> Vec<(S, Vec<Exactness>)> {
    flat()
        .iter()
        .flat_map(|h| {
            let faces = h.body.arenas().rows(Arena::Face);
            (0..faces).map(move |f| (area_of(h, f), vec![h.exactness]))
        })
        .collect()
}

fn drive_area() -> Driver {
    let round = curved();
    let cyl = &round[0];
    let example = Held::new(a_box_at_origin());
    let listed = faces(
        &ctx(),
        &example.body,
        &[&example.history],
        example.exactness,
    )
    .unwrap();
    let min_z = listed
        .result
        .iter()
        .find(|f| f.roles.iter().any(|r| r.name() == "min.z"))
        .unwrap()
        .row
        .face;
    let shown = twice_area(&ctx(), &example.body, min_z, example.exactness).unwrap();
    assert_eq!(shown.result, [0, 0, -400]);
    Driver {
        good: areas(),
        triggers: vec![
            trigger("Malformed", area_of(&unit_box(), 99)),
            trigger("Malformed", area_of(&ringed_loops(), 0)),
            trigger("CurvedFace", area_of(cyl, face_of(cyl, false))),
            trigger("CurvedEdge", area_of(cyl, face_of(cyl, true))),
            trigger("NoLoops", area_of(&loopless(), 0)),
        ],
        edge: Some(area_of(&filling(), 0)),
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

/// `query.volume` of `held`.
fn volume_of(held: &Held) -> S {
    held.on("query.volume", |c, h| six_volume(c, &h.body, h.exactness))
}

fn drive_volume() -> Driver {
    let round = curved();
    let (cyl, ball) = (&round[0], &round[2]);
    let shown = six_volume(&ctx(), &a_box_at_origin().result.body, Exactness::Exact).unwrap();
    assert_eq!(shown.result, geom_num::I256::from_i64(36_000));
    Driver {
        good: flat()
            .iter()
            .map(|h| (volume_of(h), vec![h.exactness]))
            .collect(),
        triggers: vec![
            trigger("Malformed", volume_of(&ringed_faces())),
            trigger("CurvedEdge", volume_of(cyl)),
            trigger("CurvedFace", volume_of(ball)),
            trigger("NoLoops", volume_of(&loopless())),
            trigger("NotSolid", volume_of(&sheet())),
        ],
        edge: Some(volume_of(&filling())),
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

/// The box of 10 by 20 by 30 at the origin, the queries' examples' body.
fn a_box_at_origin() -> OpOutput<Made> {
    make_box(
        &ctx(),
        &BoxInput {
            min: p(0, 0, 0),
            size: [Steps(10), Steps(20), Steps(30)],
        },
    )
    .unwrap()
}

/// Every flat and curved body, with the class each is recorded with.
fn every_body() -> Vec<Held> {
    let mut all = flat();
    all.extend(curved());
    all
}

/// `query.bounding_box` of `held`.
fn box_of(held: &Held) -> S {
    held.on("query.bounding_box", |c, h| {
        bounding_box(c, &h.body, h.exactness)
    })
}

fn drive_bounding_box() -> Driver {
    let shown = bounding_box(&ctx(), &a_box_at_origin().result.body, Exactness::Exact).unwrap();
    let boxed = shown.result.unwrap();
    assert!(boxed.tight);
    assert_eq!(
        (boxed.bound.min(), boxed.bound.max()),
        (p(0, 0, 0), p(10, 20, 30))
    );
    Driver {
        good: every_body()
            .iter()
            .map(|h| (box_of(h), vec![h.exactness]))
            .collect(),
        triggers: vec![trigger("Malformed", box_of(&ringed_loops()))],
        edge: Some(box_of(&filling())),
        example: Some((vec![], shown.exactness)),
        ..Driver::default()
    }
}

/// `query.exactness` of `held`.
fn report_of(held: &Held) -> S {
    held.on("query.exactness", |c, h| exactness(c, &h.body, h.exactness))
}

fn drive_exactness() -> Driver {
    let round = curved();
    let cyl = &round[0];
    let shown = exactness(&ctx(), &cyl.body, cyl.exactness).unwrap();
    let r = &shown.result;
    assert!(r.report.passed());
    assert_eq!((r.curved_faces, r.curved_edges), (1, 2));
    Driver {
        good: every_body()
            .iter()
            .map(|h| (report_of(h), vec![h.exactness]))
            .collect(),
        edge: Some(report_of(&filling())),
        example: Some((
            vec![
                ("flat", u64::from(r.flat_faces)),
                ("straight", u64::from(r.straight_edges)),
            ],
            shown.exactness,
        )),
        ..Driver::default()
    }
}

/// `query.faces` of `held`, with the history that made it.
fn faces_of(held: &Held) -> S {
    held.on("query.faces", |c, h| {
        faces(c, &h.body, &[&h.history], h.exactness)
    })
}

fn drive_faces() -> Driver {
    let made = a_box_at_origin();
    let shown = faces(
        &ctx(),
        &made.result.body,
        &[&made.result.history],
        made.exactness,
    )
    .unwrap();
    let roles: BTreeSet<String> = shown
        .result
        .iter()
        .flat_map(|f| f.roles.iter().map(|r| r.name().to_owned()))
        .collect();
    let said = ["min.x", "max.x", "min.y", "max.y", "min.z", "max.z"];
    assert_eq!(roles, said.map(str::to_owned).into_iter().collect());
    Driver {
        good: every_body()
            .iter()
            .map(|h| (faces_of(h), vec![h.exactness]))
            .collect(),
        triggers: vec![trigger("Malformed", faces_of(&ringed_loops()))],
        edge: Some(faces_of(&filling())),
        example: Some((
            vec![("row", u64::try_from(shown.result.len()).unwrap())],
            shown.exactness,
        )),
        ..Driver::default()
    }
}

/// `query.validity` of `held` at `level`.
fn validity_of(held: &Held, level: Level) -> S {
    held.on(format!("query.validity {level:?}"), move |c, h| {
        validity(c, &h.body, h.exactness, level)
    })
}

fn drive_validity() -> Driver {
    let shown = validity(
        &ctx(),
        &a_box_at_origin().result.body,
        Exactness::Exact,
        Level::Full,
    )
    .unwrap();
    let r = &shown.result;
    Driver {
        good: every_body()
            .iter()
            .flat_map(|h| {
                [Level::Cheap, Level::Full].map(|level| (validity_of(h, level), vec![h.exactness]))
            })
            .collect(),
        edge: Some(validity_of(&filling(), Level::Full)),
        example: Some((
            vec![
                ("fault", u64::try_from(r.faults().len()).unwrap()),
                ("group", u64::try_from(r.ran().count()).unwrap()),
                ("deferred", r.deferred()),
            ],
            shown.exactness,
        )),
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
        ("query.area", drive_area()),
        ("query.bounding_box", drive_bounding_box()),
        ("query.exactness", drive_exactness()),
        ("query.faces", drive_faces()),
        ("query.validity", drive_validity()),
        ("query.volume", drive_volume()),
        ("transform.mirror", drive_mirror()),
        ("transform.move", drive_move()),
        ("transform.pattern", drive_pattern()),
        ("transform.rotate", drive_rotate()),
        ("transform.scale", drive_scale()),
    ]
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
fn a_query_states_its_bodys_class() {
    // Principle 3 for a query: its answer is as exact as the record it was given, so it states
    // that record, never a class of its own (22; `geom_topo::query_ops`).
    each(|schema, driver| {
        if schema.family != Family::Query {
            return;
        }
        for (subject, inputs) in &driver.good {
            let done = subject.once().unwrap();
            assert_eq!(&[done.exactness], &inputs[..], "{}", subject.label());
        }
    });
}

#[test]
fn the_range_holds_at_the_frames_edge() {
    each(|schema, driver| {
        let ranged = schema
            .errors
            .iter()
            .any(|e| e.kind == ErrorKind::FrameRangeExceeded);
        if ranged {
            assert!(driver.edge.is_none(), "{}", schema.name);
            let (inside, outside) = driver.range.as_ref().unwrap();
            range_holds(inside, outside).unwrap();
        } else {
            // Only a query makes no coordinate, so only a query has no step past the frame: it
            // answers a body whose every vertex is on the frame's edge, exactly.
            assert_eq!(schema.family, Family::Query, "{}", schema.name);
            assert!(driver.range.is_none(), "{}", schema.name);
            let edge = driver.edge.as_ref().unwrap();
            let answered = edge.once().map(|_| ()).map_err(|e| e.kind());
            assert_eq!(answered, Ok(()), "{}", edge.label());
        }
    });
}

#[test]
fn every_schema_example_runs() {
    each(|schema, driver| {
        let (counts, class) = driver.example.as_ref().unwrap();
        example_holds(schema, counts, *class).unwrap();
    });
}
