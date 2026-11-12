//! Fuzz target: `geom-make`'s primitives and transforms, on any bytes, return only what the full
//! checker passes, keep their laws, and refuse with a typed error, never a defect; and the core
//! queries (piece G102) on every body they make keep the Query family's laws.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a primitive, its inputs
//! placed on the hard cases of chapter 28.5 (one-step sizes, coarse sub-grids, coordinates at the
//! grid's edge), and a short list of transforms and patterns, each with the class the caller
//! accepts and a work budget. [`check`] builds the primitive and runs the list, each step on the
//! last result, and holds every outcome to the laws below. On the primitive and on each
//! transform's result it asks all six queries ([`answers`]), twice and under the step's budget,
//! and holds the answers to [`answers_hold`]. [`harness`] is `decode` then `check`.
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
//!   input, or an error of a kind no schema lists. For the queries: an answer that states another
//!   class than its body's, differs when asked again, or changes under a budget instead of
//!   stopping; a refusal of a valid body other than for a curved face or edge; a validity report
//!   or exactness judgement that is not the checker's; an exactness report that miscounts the
//!   faces and edges by kind; faces not the body's by name and order; a box that misses a vertex
//!   or is wrongly tight; and, for a flat solid, area vectors that do not close or a volume that
//!   is not the one its loops enclose.
//!
//! Method: chapter 28.5's oracle stack and its generators aimed at the hard cases
//! (`projects/engines/geometry/architecture/28-testing.md`), with the volume of a flat solid
//! computed from its loops as an oracle the checker does not share.
//!
//! Cost: at most [`MAX_STEPS`] transforms, each of at most [`MAX_COPIES`] copies, on a body of at
//! most [`MAX_CORNERS`] base corners, each run twice and checked in full; the six queries three
//! times on at most `1 + MAX_STEPS` bodies, each time with the checker in full; at most
//! [`MAX_INPUT_BYTES`] bytes read.
//!
//! Covers: make.box, make.cone, make.cylinder, make.prism, make.sphere, make.torus,
//! query.area, query.bounding_box, query.exactness, query.faces, query.validity, query.volume,
//! transform.mirror, transform.move, transform.pattern, transform.rotate, transform.scale.

use geom_check::query::{self, ExactnessReport};
use geom_check::{Level, Report, check as full_check};
use geom_make::{
    BoxInput, Instance, Made, Moved, PrismInput, RoundInput, SphereInput, TorusInput, cone,
    cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere, torus,
};
use geom_math::{Axis, Box3, GridTransform, RationalTransform, Rotation, Scale, TypedTransform};
use geom_name::NamedFace;
use geom_num::wide::I256;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{ErrorKind, NameId, NameSet, OpContext, OpError, OpId, OpOutput, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_test::{ByteSource, Entropy};
use geom_topo::ix::Arena;
use geom_topo::query_ops::{self, BoundingBox};
use geom_topo::{Body, CurveKind, Ix, Sense, SurfaceKind};

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
    let work = case.steps.first().map_or(0, |s| s.work);
    queries(case, "primitive", &body, work)?;
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
                    queries(case, "a transform", &out, step.work)?;
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

/// Every query of piece G102 on one body, as a program asks them: each face's twice-area vector,
/// six times the volume, the bounding box, the exactness report, the faces, and the validity
/// report at the cheap level and in full.
#[derive(Clone, Debug, PartialEq)]
pub struct Answers {
    /// `query.area` of each face, in arena order.
    pub areas: Vec<Result<OpOutput<[i128; 3]>, OpError>>,
    /// `query.volume`.
    pub volume: Result<OpOutput<I256>, OpError>,
    /// `query.bounding_box`.
    pub bounding_box: Result<OpOutput<Option<BoundingBox>>, OpError>,
    /// `query.exactness`.
    pub exactness: Result<OpOutput<ExactnessReport>, OpError>,
    /// `query.faces`, with the history of the operation that made the body.
    pub faces: Result<OpOutput<Vec<NamedFace>>, OpError>,
    /// `query.validity` at [`Level::Cheap`] and [`Level::Full`].
    pub validity: [Result<OpOutput<Report>, OpError>; 2],
}

/// The six queries on `out`, under `ctx`, the body recorded with the class it was made with.
#[must_use]
pub fn answers(ctx: &OpContext<'_>, out: &OpOutput<Made>) -> Answers {
    let (body, class) = (&out.result.body, out.exactness);
    Answers {
        areas: (0..body.arenas().rows(Arena::Face))
            .filter_map(Ix::new)
            .map(|f| query_ops::twice_area(ctx, body, f, class))
            .collect(),
        volume: query_ops::six_volume(ctx, body, class),
        bounding_box: query_ops::bounding_box(ctx, body, class),
        exactness: query::exactness(ctx, body, class),
        faces: geom_name::faces(ctx, body, &[&out.result.history], class),
        validity: [Level::Cheap, Level::Full].map(|level| query::validity(ctx, body, class, level)),
    }
}

/// Whether `got`, from a run under a budget, is `want` or the budget's stop.
fn same_or_stopped<T: PartialEq>(got: &Result<T, OpError>, want: &Result<T, OpError>) -> bool {
    got == want || matches!(got, Err(e) if e.kind() == ErrorKind::BudgetExceeded)
}

/// The queries on `out` keep chapter 22's laws: each answers the same twice, and under a budget of
/// `work` units the same or a stop; each holds to [`answers_hold`].
fn queries(case: &Case, what: &str, out: &OpOutput<Made>, work: u64) -> Result<(), Finding> {
    let first = answers(&context(Budget::UNLIMITED), out);
    answers_hold(out, &first)
        .map_err(|f| Finding::new(&f.law, || format!("{what}: {case:?}: {}", f.detail)))?;
    if answers(&context(Budget::UNLIMITED), out) != first {
        return Err(Finding::new("a query answers the same twice", || {
            format!("{what}: {case:?}")
        }));
    }
    let tight = answers(&context(Budget::new(work, u64::MAX)), out);
    let kept = tight
        .areas
        .iter()
        .zip(&first.areas)
        .all(|(g, w)| same_or_stopped(g, w))
        && tight.areas.len() == first.areas.len()
        && same_or_stopped(&tight.volume, &first.volume)
        && same_or_stopped(&tight.bounding_box, &first.bounding_box)
        && same_or_stopped(&tight.exactness, &first.exactness)
        && same_or_stopped(&tight.faces, &first.faces)
        && tight
            .validity
            .iter()
            .zip(&first.validity)
            .all(|(g, w)| same_or_stopped(g, w));
    if kept {
        Ok(())
    } else {
        Err(Finding::new(
            "a budget stops a query or leaves it alone",
            || format!("{what}: {case:?}: under {work}"),
        ))
    }
}

/// The answers of the queries on a valid body `out`, held to the laws and to oracles that do not
/// share their code: every answer states the body's class; a refusal is a curved face or edge
/// (`Unsupported`), and only of the area and volume; `query.validity` and `query.exactness`'s
/// judgement are the checker's own reports; the exactness report counts the body's faces and edges
/// by kind; the faces are the body's, by name, in arena order; the box holds every vertex, and is
/// tight exactly when every face is a polygon on a plane, and then the vertices' box; and then the
/// faces' area vectors sum to zero over the closed shell and six times the volume is the one
/// [`six_volume`] reads from the loops.
///
/// # Errors
///
/// A [`Finding`] naming the law the answers break.
#[allow(
    clippy::too_many_lines,
    reason = "one law after another, each a few lines, read in the order the header lists them"
)]
pub fn answers_hold(out: &OpOutput<Made>, answers: &Answers) -> Result<(), Finding> {
    let body = &out.result.body;
    let class = out.exactness;
    let law = |name: &'static str| Err(Finding::new(name, String::new));
    let curved = |e: &OpError| e.kind() == ErrorKind::Unsupported;
    let classes = answers
        .areas
        .iter()
        .map(|r| r.as_ref().map(|o| o.exactness))
        .chain([
            answers.volume.as_ref().map(|o| o.exactness),
            answers.bounding_box.as_ref().map(|o| o.exactness),
            answers.exactness.as_ref().map(|o| o.exactness),
            answers.faces.as_ref().map(|o| o.exactness),
        ])
        .chain(
            answers
                .validity
                .iter()
                .map(|r| r.as_ref().map(|o| o.exactness)),
        );
    let mut flat = true;
    for (at, answer) in classes.enumerate() {
        match answer {
            Ok(stated) if stated == class => {}
            Ok(_) => return law("a query states its body's class"),
            Err(e) if curved(e) && at <= answers.areas.len() => flat = false,
            Err(_) => return law("a query refuses a valid body only for a curved face or edge"),
        }
    }
    let mut meter = WorkMeter::new(Budget::UNLIMITED);
    let (Ok(cheap), Ok(full)) = (
        full_check(body, class, Level::Cheap, &mut meter),
        full_check(body, class, Level::Full, &mut meter),
    ) else {
        return law("an unlimited check finishes");
    };
    let [at_cheap, at_full] = &answers.validity;
    let (Ok(at_cheap), Ok(at_full), Ok(report), Ok(listed), Ok(boxed)) = (
        at_cheap,
        at_full,
        &answers.exactness,
        &answers.faces,
        &answers.bounding_box,
    ) else {
        return law("a query refuses a valid body only for a curved face or edge");
    };
    if at_cheap.result != cheap || at_full.result != full {
        return law("query.validity is the checker's report");
    }
    let r = &report.result;
    let a = body.arenas();
    let planes = (0..a.rows(Arena::Face))
        .filter_map(Ix::new)
        .filter(|&f| {
            let surface = a.faces.get(f).map(|face| face.surface);
            matches!(
                surface.and_then(|s| a.surfaces.get(s)),
                Some(SurfaceKind::Plane(_))
            )
        })
        .count();
    let lines = (0..a.rows(Arena::Edge))
        .filter_map(Ix::new)
        .filter(|&e| {
            let curve = a.edges.get(e).map(|edge| edge.curve);
            matches!(
                curve.and_then(|c| a.curves.get(c)),
                Some(CurveKind::Line(_))
            )
        })
        .count();
    let by_kind = [
        (r.flat_faces, planes),
        (r.straight_edges, lines),
        (
            r.flat_faces.saturating_add(r.curved_faces),
            usize::try_from(a.rows(Arena::Face)).unwrap_or(usize::MAX),
        ),
        (
            r.straight_edges.saturating_add(r.curved_edges),
            usize::try_from(a.rows(Arena::Edge)).unwrap_or(usize::MAX),
        ),
    ];
    let counted = by_kind
        .iter()
        .all(|&(said, is)| usize::try_from(said).is_ok_and(|s| s == is));
    if r.recorded != class || !counted || r.report != cheap {
        return law("query.exactness reports the record, the geometry and the checker's judgement");
    }
    let names: Vec<Option<NameId>> = body.names().map_or_else(Vec::new, |n| {
        n.faces
            .iter()
            .map(|k| Some(NameId::from_bytes(k.0)))
            .collect()
    });
    let rows: Vec<Option<NameId>> = listed.result.iter().map(|f| f.name).collect();
    let ordered = listed
        .result
        .iter()
        .zip(0u32..)
        .all(|(f, row)| f.row.face.row() == row);
    if rows != names || !ordered {
        return law("query.faces lists the body's faces by name, in arena order");
    }
    let corners: Vec<P3> = (0..a.rows(Arena::Vertex))
        .filter_map(Ix::new)
        .filter_map(|v| {
            a.vertices
                .get(v)
                .and_then(|vx| a.points.get(vx.point))
                .copied()
        })
        .collect();
    let Some(b) = boxed.result else {
        return law("a solid has a bounding box");
    };
    if !corners.iter().all(|q| b.bound.contains(*q)) || b.tight != flat {
        return law("the box holds every vertex, tight exactly when every face is flat");
    }
    if !flat {
        return if answers.volume.is_ok() {
            law("query.volume refuses a body with a curved face or edge")
        } else {
            Ok(())
        };
    }
    if Box3::from_points(corners) != Some(b.bound) {
        return law("a flat body's box is its vertices' box");
    }
    let mut sum = [0i128; 3];
    for area in answers.areas.iter().flatten() {
        for (s, v) in sum.iter_mut().zip(area.result) {
            *s = s.saturating_add(v);
        }
    }
    let Ok(volume) = &answers.volume else {
        return law("a query refuses a valid body only for a curved face or edge");
    };
    if sum != [0; 3] {
        return law("a closed shell's area vectors sum to zero");
    }
    if volume.result != I256::from_i128(six_volume(body)) {
        return law("query.volume is six times the volume its loops enclose");
    }
    Ok(())
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
            if let Some(at) = a.vertices.point.get(start).and_then(|p| a.points.get(*p)) {
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
