//! Fuzz target: `geom-make`'s primitives and transforms, on any bytes, return only what the full
//! checker passes, keep their laws, and refuse with a typed error, never a defect.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a primitive, its inputs
//! placed on the hard cases of chapter 28.5 (one-step sizes, coarse sub-grids, coordinates at the
//! grid's edge), and a short list of transforms and patterns, each with the class the caller
//! accepts and a work budget. [`check`] builds the primitive and runs the list, each step on the
//! last result, and holds every outcome to the laws below. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case: a result
//!   the full checker rejects, a result without names, a history that breaks its laws, a transform
//!   that loses a name, a class that rises, a flat solid that turns inside out, a rerun that gives
//!   another answer, a budget that changes a result instead of stopping it, a defect on valid
//!   input, or an error of a kind no schema lists.
//!
//! Method: chapter 28.5's oracle stack and its generators aimed at the hard cases
//! (`projects/engines/geometry/architecture/28-testing.md`), with the volume of a flat solid
//! computed from its loops as an oracle the checker does not share.
//!
//! Cost: at most [`MAX_STEPS`] transforms, each of at most [`MAX_COPIES`] copies, on a body of at
//! most [`MAX_CORNERS`] base corners, each run twice and checked in full; at most
//! [`MAX_INPUT_BYTES`] bytes read.
//!
//! Covers: make.box, make.cone, make.cylinder, make.prism, make.sphere, make.torus,
//! transform.mirror, transform.move, transform.pattern, transform.rotate, transform.scale.

use geom_check::{Level, check as full_check};
use geom_make::{
    BoxInput, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{ErrorKind, NameId, NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};
use geom_topo::ix::Arena;
use geom_topo::{Body, Ix, Sense};

use crate::Finding;
pub use crate::adversarial::MAX_CORNERS;
use crate::adversarial::{POINT_BYTES, VALUE_BYTES, base, length, point};

/// The most transforms a case runs.
pub const MAX_STEPS: u8 = 4;

/// The most copies a pattern of a case makes.
pub const MAX_COPIES: u8 = 3;

/// The bytes a primitive reads: a choice, a point, three lengths, and a prism's corners.
pub(crate) const SHAPE_BYTES: usize = 1 + POINT_BYTES + 3 * VALUE_BYTES + 1 + 8 * POINT_BYTES;

/// The bytes a motion reads: a choice, an axis and a point.
const MOTION_BYTES: usize = 1 + 1 + POINT_BYTES;

/// The bytes a step reads: a rule, a budget, a copy count and [`MAX_COPIES`] motions.
const STEP_BYTES: usize = 1 + 8 + 1 + 3 * MOTION_BYTES;

/// The most bytes [`decode`] reads from any input: 661.
///
/// A byte choosing the primitive, a point and three lengths. For a prism, a corner count and
/// [`MAX_CORNERS`] points. A step count and [`MAX_STEPS`] steps. So
/// 1 + 27 + 27 + 1 + 8 · 27 + 1 + 4 · 97 = 661. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds that input.
pub const MAX_INPUT_BYTES: usize =
    1 + POINT_BYTES + 3 * VALUE_BYTES + 1 + 8 * POINT_BYTES + 1 + 4 * STEP_BYTES;

const _: () = assert!(MAX_INPUT_BYTES == 661);

/// A primitive of a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A box.
    Box(BoxInput),
    /// A prism.
    Prism(PrismInput),
    /// A cylinder.
    Cylinder(RoundInput),
    /// A cone.
    Cone(RoundInput),
    /// A sphere.
    Sphere(SphereInput),
    /// A ring torus.
    Torus(TorusInput),
}

/// A motion of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// A move by whole steps.
    Move([i64; 3]),
    /// A rotation about a line along an axis: quarter turns, or a direction.
    Rotate(Axis, RotationBy, P3),
    /// A mirror across a plane.
    Mirror(Axis, P3),
    /// A scale about a point, each factor a numerator and a denominator.
    Scale([(u64, u64); 3], P3),
}

/// A rotation's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationBy {
    /// Quarter turns.
    Quarter(i32),
    /// A direction, cosine and sine in whole numbers.
    Direction(i32, i32),
}

/// One step of a case: a motion, or a pattern of motions, keyed by their place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// One motion, or several for a pattern.
    pub motions: Vec<Motion>,
    /// Whether it is a pattern, even of one copy.
    pub pattern: bool,
    /// The largest bound the caller accepts, in steps; `None` accepts none.
    pub accept: Option<u64>,
    /// The work budget of a second run, which must give the same result or stop.
    pub work: u64,
}

/// One case: a primitive and what to do to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The primitive.
    pub shape: Shape,
    /// The steps, in order.
    pub steps: Vec<Step>,
}

/// A motion.
fn motion(src: &mut ByteSource<'_>) -> Motion {
    let choice = src.byte();
    let axis = match src.byte() % 3 {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    };
    let at = point(src);
    match choice % 5 {
        0 => {
            let [x, y, z] = [at.x.get(), at.y.get(), at.z.get()];
            Motion::Move([x, y, z])
        }
        1 => {
            let turns = i32::try_from(at.x.get() % 8).unwrap_or(1);
            Motion::Rotate(axis, RotationBy::Quarter(turns), at)
        }
        2 => {
            let c = i32::try_from(at.y.get() % 1024).unwrap_or(4);
            let s = i32::try_from(at.z.get() % 1024).unwrap_or(3);
            Motion::Rotate(axis, RotationBy::Direction(c, s), at)
        }
        3 => Motion::Mirror(axis, at),
        _ => {
            let factor = |v: i64| {
                let v = v.unsigned_abs();
                ((v % 7).saturating_add(1), (v % 3).saturating_add(1))
            };
            let f = factor(at.x.get());
            let even = at.y.get() % 2 == 0;
            let (y, z) = if even {
                (f, f)
            } else {
                (factor(at.y.get()), factor(at.z.get()))
            };
            Motion::Scale([f, y, z], at)
        }
    }
}

/// A primitive.
pub(crate) fn shape(src: &mut ByteSource<'_>) -> Shape {
    let choice = src.byte();
    let at = point(src);
    let (a, b, c) = (length(src), length(src), length(src));
    let step = GridStep::Q0;
    let round = RoundInput {
        base: at,
        radius: a,
        height: b,
        step,
    };
    match choice % 6 {
        0 => Shape::Box(BoxInput {
            min: at,
            size: [a, b, c],
        }),
        1 => Shape::Prism(PrismInput {
            base: base(src),
            height: a,
        }),
        2 => Shape::Cylinder(round),
        3 => Shape::Cone(round),
        4 => Shape::Sphere(SphereInput {
            center: at,
            radius: a,
            step,
        }),
        _ => Shape::Torus(TorusInput {
            center: at,
            major: a,
            minor: b,
            step,
        }),
    }
}

/// A step: a transform, or a pattern of up to [`MAX_COPIES`].
pub(crate) fn step(src: &mut ByteSource<'_>) -> Step {
    let rule = src.byte();
    let accept = match rule % 4 {
        0 => None,
        1 => Some(1),
        2 => Some(64),
        _ => Some(1 << 30),
    };
    let pattern = rule & 4 != 0;
    let work = src.word() % 2000;
    let copies = if pattern {
        usize::from(src.byte() % MAX_COPIES).saturating_add(1)
    } else {
        1
    };
    let motions = (0..copies).map(|_| motion(src)).collect();
    Step {
        motions,
        pattern,
        accept,
        work,
    }
}

/// The case `data` decodes to, and how many bytes it read.
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let shape = shape(&mut src);
    let count = src.byte() % (MAX_STEPS + 1);
    let mut steps = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        steps.push(step(&mut src));
    }
    let read = data.len().saturating_sub(src.remaining());
    (Case { shape, steps }, read)
}

/// `decode` then `check`.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}

/// Holds the case's every outcome to the laws.
///
/// # Errors
///
/// A [`Finding`] if a law is broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    check_with(case, |out| out)
}

/// [`check`] with `tamper` applied to each result before it is judged: how a test shows that a
/// broken operation is caught.
///
/// # Errors
///
/// As for [`check`].
pub fn check_with<T>(case: &Case, tamper: T) -> Result<(), Finding>
where
    T: Fn(OpOutput<Made>) -> OpOutput<Made>,
{
    let ctx = context(Budget::UNLIMITED);
    let first = build(&ctx, &case.shape);
    let mut body = match first {
        Ok(out) => tamper(out),
        Err(e) => return refused(case, &e),
    };
    judge(case, "primitive", &body)?;
    if body.exactness == Exactness::Exact && six_volume(&body.result.body) <= 0 {
        return Err(Finding::new("a flat solid faces outward", || {
            format!("{case:?}")
        }));
    }
    for step in &case.steps {
        match run(&ctx, &body, step) {
            Ok(mut results) => {
                if step.pattern {
                    for copy in results {
                        judge(case, "a pattern's copy", &copy)?;
                        history_holds(case, &body, &copy)?;
                    }
                } else if let Some(out) = results.pop() {
                    let out = tamper(out);
                    judge(case, "a transform", &out)?;
                    history_holds(case, &body, &out)?;
                    if out.result.body.names() != body.result.body.names() {
                        return Err(Finding::new("a transform keeps every name", || {
                            format!("{case:?}")
                        }));
                    }
                    if out.exactness.rank() < body.exactness.rank() {
                        return Err(Finding::new("the class never rises", || {
                            format!("{case:?}: {:?} from {:?}", out.exactness, body.exactness)
                        }));
                    }
                    rerun(case, &body, step, &out)?;
                    body = out;
                }
            }
            Err(e) => refused(case, &e)?,
        }
    }
    Ok(())
}

/// The context of a case's operations.
fn context(budget: Budget) -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([3; 16])), budget)
}

/// The primitive of `shape`.
fn build(ctx: &OpContext<'_>, shape: &Shape) -> Result<OpOutput<Made>, OpError> {
    match shape {
        Shape::Box(input) => make_box(ctx, input),
        Shape::Prism(input) => prism(ctx, input),
        Shape::Cylinder(input) => cylinder(ctx, input),
        Shape::Cone(input) => cone(ctx, input),
        Shape::Sphere(input) => sphere(ctx, input),
        Shape::Torus(input) => torus(ctx, input),
    }
}

/// The transform of `motion`, for a pattern's instance.
fn typed(motion: Motion) -> Option<TypedTransform> {
    match motion {
        Motion::Move(offset) => GridTransform::translation(offset)
            .ok()
            .map(TypedTransform::Exact),
        Motion::Rotate(axis, by, _) => {
            TypedTransform::rotation(axis, rotation(by).unwrap_or(Rotation::quarter_turns(1))).ok()
        }
        Motion::Mirror(axis, _) => Some(TypedTransform::Exact(GridTransform::mirror(axis))),
        Motion::Scale([(n, d), ..], _) => RationalTransform::uniform_scale(Scale::new(n, d).ok()?)
            .ok()
            .map(TypedTransform::from),
    }
}

/// `by` as a rotation; `None` for a direction of length zero.
fn rotation(by: RotationBy) -> Option<Rotation> {
    match by {
        RotationBy::Quarter(turns) => Some(Rotation::quarter_turns(turns)),
        RotationBy::Direction(c, s) => Rotation::direction(c, s).ok(),
    }
}

/// `step` on `body`, under `ctx`: its results in order.
fn run(
    ctx: &OpContext<'_>,
    body: &OpOutput<Made>,
    step: &Step,
) -> Result<Vec<OpOutput<Made>>, OpError> {
    let input = Moved {
        body: &body.result.body,
        exactness: body.exactness,
        accept: step.accept.map(|bound| Exactness::Certified {
            bound: Steps(bound),
            step: GridStep::Q0,
        }),
    };
    if step.pattern {
        let instances: Vec<Instance> = step
            .motions
            .iter()
            .zip(0u8..)
            .filter_map(|(&m, k)| {
                Some(Instance {
                    key: [k; 16],
                    transform: typed(m)?,
                })
            })
            .collect();
        let out = pattern(ctx, input, &instances)?;
        let exactness = out.exactness;
        return Ok(out
            .result
            .into_iter()
            .map(|(_, made)| OpOutput::new(made, exactness))
            .collect());
    }
    let Some(&motion) = step.motions.first() else {
        return Ok(Vec::new());
    };
    let out = match motion {
        Motion::Move(offset) => move_by(ctx, input, offset)?,
        Motion::Rotate(axis, by, center) => match rotation(by) {
            Some(turn) => rotate(ctx, input, axis, turn, center)?,
            None => return Ok(Vec::new()),
        },
        Motion::Mirror(axis, at) => mirror(ctx, input, axis, at)?,
        Motion::Scale(factors, center) => {
            let [a, b, c] = factors.map(|(n, d)| Scale::new(n, d));
            match (a, b, c) {
                (Ok(a), Ok(b), Ok(c)) => scale(ctx, input, [a, b, c], center)?,
                _ => return Ok(Vec::new()),
            }
        }
    };
    Ok(vec![out])
}

/// A refusal is typed, and of a kind some schema lists; a defect is a finding.
fn refused(case: &Case, e: &OpError) -> Result<(), Finding> {
    let listed = matches!(
        e.kind(),
        ErrorKind::Degenerate
            | ErrorKind::InvalidInput
            | ErrorKind::FrameRangeExceeded
            | ErrorKind::ExactnessNotAccepted
            | ErrorKind::Unsupported
            | ErrorKind::BudgetExceeded
    );
    if listed {
        Ok(())
    } else {
        Err(Finding::new("a refusal is typed, never a defect", || {
            format!("{:?}: {}: {case:?}", e.detail(), e.message())
        }))
    }
}

/// `out` passes the full checker at its record, in name order.
fn judge(case: &Case, what: &str, out: &OpOutput<Made>) -> Result<(), Finding> {
    let body = &out.result.body;
    if body.names().is_none() {
        return Err(Finding::new("a result carries its names", || {
            format!("{what}: {case:?}")
        }));
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let report = full_check(body, out.exactness, Level::Full, &mut meter)
        .map_err(|_| Finding::new("an unlimited check finishes", || format!("{case:?}")))?;
    if !report.passed() {
        return Err(Finding::new("every result passes the full checker", || {
            format!("{what}: {case:?}: {:?}", report.faults())
        }));
    }
    Ok(())
}

/// Every name of `body`.
fn names_of(body: &Body) -> Vec<NameId> {
    body.names().map_or_else(Vec::new, |n| {
        n.regions
            .iter()
            .chain(&n.faces)
            .chain(&n.edges)
            .chain(&n.vertices)
            .map(|k| NameId::from_bytes(k.0))
            .collect()
    })
}

/// `out`'s history keeps its laws over `input`'s names.
fn history_holds(case: &Case, input: &OpOutput<Made>, out: &OpOutput<Made>) -> Result<(), Finding> {
    let names = NameSet::new(names_of(&input.result.body))
        .map_err(|_| Finding::new("a body's names are distinct", || format!("{case:?}")))?;
    out.result.history.check(&names).map_err(|e| {
        Finding::new("a transform's history keeps its laws", || {
            format!("{case:?}: {e:?}")
        })
    })?;
    // The laws leave an entity without a record to mean unchanged; a transform or a copy changes
    // every one, so every one has a record.
    let recorded: Vec<NameId> = out
        .result
        .history
        .records()
        .iter()
        .map(|r| r.output)
        .collect();
    if names_of(&out.result.body)
        .iter()
        .all(|n| recorded.contains(n))
    {
        Ok(())
    } else {
        Err(Finding::new("a transform records every entity", || {
            format!("{case:?}")
        }))
    }
}

/// A rerun gives the same answer, and one under `step.work` gives that answer or stops.
fn rerun(
    case: &Case,
    body: &OpOutput<Made>,
    step: &Step,
    out: &OpOutput<Made>,
) -> Result<(), Finding> {
    let same = |other: &OpOutput<Made>| {
        other.exactness == out.exactness && other.result.body == out.result.body
    };
    let again = run(&context(Budget::UNLIMITED), body, step);
    if !matches!(&again, Ok(r) if r.first().is_some_and(same)) {
        return Err(Finding::new("a rerun gives the same answer", || {
            format!("{case:?}")
        }));
    }
    match run(&context(Budget::new(step.work, u64::MAX)), body, step) {
        Ok(r) if r.first().is_some_and(same) => Ok(()),
        Err(e) if e.kind() == ErrorKind::BudgetExceeded => Ok(()),
        _ => Err(Finding::new(
            "a budget stops a result or leaves it alone",
            || format!("{case:?}"),
        )),
    }
}

/// Six times the signed volume of a flat solid, from its loops as published: positive when every
/// loop runs counter-clockwise seen from outside.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "grid coordinates are below 2^40, so a determinant is below 2^123, and a case's few hundred of them fit an i128; the names are a fin's and an edge's rows"
)]
pub fn six_volume(body: &Body) -> i128 {
    let a = body.arenas();
    let mut total = 0i128;
    for l in (0..a.rows(Arena::Loop)).filter_map(Ix::new) {
        let Some(first) = a.loops.first_fin.get(l).copied().flatten() else {
            continue;
        };
        let mut corners = Vec::new();
        let mut f = first;
        while let Some(&e) = a.fins.edge.get(f) {
            let (Some(s), Some(t)) = (
                a.edges.start.get(e).copied().flatten(),
                a.edges.end.get(e).copied().flatten(),
            ) else {
                break;
            };
            let forward = a.fins.sense.get(f) == Some(&Sense::Forward);
            let start = if forward { s } else { t };
            if let Some(at) = a
                .vertices
                .point
                .get(start)
                .and_then(|p| a.points.get(*p))
                .and_then(|p| p.grid())
            {
                corners.push([at.x.get(), at.y.get(), at.z.get()].map(i128::from));
            }
            match a.fins.next.get(f) {
                Some(&next) if next != first => f = next,
                _ => break,
            }
        }
        let det = |[ax, ay, az]: [i128; 3], [bx, by, bz]: [i128; 3], [cx, cy, cz]: [i128; 3]| {
            ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx)
        };
        if let Some((&o, rest)) = corners.split_first() {
            for w in rest.windows(2) {
                if let [b, c] = w {
                    total += det(o, *b, *c);
                }
            }
        }
    }
    total
}
