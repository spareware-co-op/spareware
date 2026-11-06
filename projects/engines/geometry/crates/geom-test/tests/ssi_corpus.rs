//! The surface-surface intersection corpus run through `geom-intersect`: a benchmark, not a gate
//! (chapter 12.17; piece S6).
//!
//! Each of the corpus' 119 cases (`geom_test::ssi_corpus`) is placed on the grid, run through the
//! kernel's general surface-surface path, and put in one of five outcomes: a complete answer that
//! matches the corpus' exact topology (`right`), a complete answer that does not (`wrong`), an
//! answer with flagged regions (`flagged`), a typed refusal (`refused`), or a surface the adapter
//! has no kind for (`untranslated`). A panic or a run past its time is a sixth and seventh outcome,
//! and the only ones the tests below fail on. Each case is also set beside `SMLib`'s result in the
//! corpus: `agree` (both right), `differs` (ours answers and is wrong where `SMLib` passes), `ours
//! refuses` (ours flags or refuses where `SMLib` passes) and `theirs failed` (`SMLib` does not pass).
//!
//! Placing a case: every length of the corpus is multiplied by a power of two, the smallest from
//! 1024 up at which the case's coverage tolerance is at least 8 grid steps, so the multiplication
//! is exact, the tolerance is above the floor of subdivision, and the largest case stays inside the
//! grid. The floor of subdivision is 8 steps, and the ceiling on how far a branch may sit from its
//! chords is the case's own tolerance in steps. Angles go to half-turns.
//!
//! The runner: `subdivide` (G169), then `analyse` of what is flagged (G171), the shallow crossings
//! it settles joining the settled pairs, then `march` (G170) over all of them. There is no single
//! entry point yet, so this is the order the pieces' own tests use. Each stage runs under its own
//! work meter, so a case that is too hard ends in a refusal, never a hang.
//!
//! To print the report of the whole corpus, in release:
//! `cargo test -p geom-test --release --test ssi_corpus -- --ignored --nocapture report`. Setting
//! `SSI_CORPUS_REPORT` to a path writes the same report there; `SSI_CORPUS_WORK` sets the work
//! each stage may spend (default 2^24) and `SSI_CORPUS_SECONDS` how long a case may take (default
//! 120).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::panic,
    clippy::too_many_lines,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "test code: a failure should stop loudly"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use geom_curve::Frame;
use geom_intersect::IntersectError;
use geom_intersect::contact::{Reading, analyse};
use geom_intersect::general::{Subdivided, subdivide};
use geom_intersect::march::march;
use geom_intersect::result::{CurveContact, CurveEnd, PointContact};
use geom_nurbs::{Basis, Surface as Nurbs};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Cone, Cylinder, Plane, Sphere, SurfaceKind, Torus};
use geom_test::ssi_corpus::{self, Case, Cpp, Surface};

/// Subdivision stops at this many grid steps.
const FLOOR: f64 = 8.0;

/// The work each stage may spend, unless `SSI_CORPUS_WORK` says otherwise.
const WORK: u64 = 1 << 24;

/// How long a case may take, in seconds, unless `SSI_CORPUS_SECONDS` says otherwise.
const SECONDS: u64 = 120;

/// What the kernel returned for a case, counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Found {
    /// Branches traced.
    branches: usize,
    /// Of them, closed loops.
    closed: usize,
    /// Isolated tangent points.
    isolated: usize,
    /// Singular points.
    singular: usize,
    /// Other points: crossings and boundary touches.
    other_points: usize,
    /// Coincident regions.
    overlaps: usize,
    /// Tangent contacts along a curve, left as a band.
    bands: usize,
    /// Clusters the tests could not decide.
    undecided: usize,
    /// Settled pairs no step could be proven in.
    unproven: usize,
    /// How each branch ends, in the order traced.
    ends: Vec<[CurveEnd; 2]>,
}

impl Found {
    /// Whether anything is left flagged.
    fn flagged(&self) -> bool {
        self.bands + self.undecided + self.unproven > 0
    }
}

/// What a run of one case came to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ours {
    /// The kernel returned, with the counts of what it returned.
    Returned(Found),
    /// The kernel refused, with its error.
    Refused(IntersectError),
    /// A surface of the case has no kind here.
    Untranslated(String),
    /// The kernel panicked.
    Panicked(String),
    /// The case ran past its time.
    TimedOut,
}

/// Ours against the corpus' exact topology.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
    /// A complete answer, with the topology of the truth.
    Right,
    /// A complete answer with another topology, and how it differs.
    Wrong(String),
    /// An answer with flagged regions.
    Flagged,
    /// A typed refusal.
    Refused(IntersectError),
    /// No kind for a surface.
    Untranslated,
    /// A panic.
    Panicked,
    /// Past its time.
    TimedOut,
}

impl Verdict {
    /// The word the report uses.
    fn word(&self) -> &'static str {
        match self {
            Verdict::Right => "right",
            Verdict::Wrong(_) => "wrong",
            Verdict::Flagged => "flagged",
            Verdict::Refused(_) => "refused",
            Verdict::Untranslated => "untranslated",
            Verdict::Panicked => "PANIC",
            Verdict::TimedOut => "TIMEOUT",
        }
    }
}

/// The unit vectors of a corpus frame: x, and the normal x × y with y made perpendicular.
fn frame(x: [f64; 3], y: [f64; 3]) -> Result<Frame<3>, String> {
    let unit = |v: [f64; 3]| {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    };
    let x = unit(x);
    let d = x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let y = unit([y[0] - d * x[0], y[1] - d * x[1], y[2] - d * x[2]]);
    let z = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    Frame::new(x, z).map_err(|e| format!("frame: {e:?}"))
}

/// `v` times `s`.
fn times(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// A range of degrees as half-turns.
fn turns(r: [f64; 2]) -> (f64, f64) {
    (r[0] / 180.0, r[1] / 180.0)
}

/// A range of lengths in grid steps.
fn steps(r: [f64; 2], s: f64) -> (f64, f64) {
    (r[0] * s, r[1] * s)
}

/// The kernel's surface for a corpus surface, every length times `s`.
fn kernel_surface(surface: &Surface, s: f64) -> Result<SurfaceKind, String> {
    let bad = |e: geom_curve::CurveError| format!("{e:?}");
    match surface {
        Surface::Plane {
            origin,
            xaxis,
            yaxis,
            u,
            v,
        } => {
            let ([ax, ay, az], [bx, by, bz]) = plane_axes(*xaxis, *yaxis);
            Plane::new(
                times(*origin, s),
                [ax * s, ay * s, az * s],
                [bx * s, by * s, bz * s],
                ((u[0], u[1]), (v[0], v[1])),
            )
            .map(SurfaceKind::Plane)
            .map_err(bad)
        }
        Surface::Cylinder {
            origin,
            xaxis,
            yaxis,
            radius,
            angle,
            height,
        } => Cylinder::new(
            times(*origin, s),
            radius * s,
            frame(*xaxis, *yaxis)?,
            (turns(*angle), steps(*height, s)),
        )
        .map(SurfaceKind::Cylinder)
        .map_err(bad),
        Surface::Cone {
            apex,
            xaxis,
            yaxis,
            half_angle_deg,
            height,
        } => Cone::new(
            times(*apex, s),
            0.0,
            half_angle_deg.to_radians().tan(),
            frame(*xaxis, *yaxis)?,
            ((0.0, 2.0), steps(*height, s)),
        )
        .map(SurfaceKind::Cone)
        .map_err(bad),
        Surface::Sphere {
            center,
            xaxis,
            yaxis,
            radius,
            lon,
            lat,
        } => Sphere::new(
            times(*center, s),
            radius * s,
            frame(*xaxis, *yaxis)?,
            (turns(*lon), turns(*lat)),
        )
        .map(SurfaceKind::Sphere)
        .map_err(bad),
        Surface::Torus {
            center,
            xaxis,
            yaxis,
            major_radius,
            minor_radius,
            major,
            minor,
        } => Torus::new(
            times(*center, s),
            major_radius * s,
            minor_radius * s,
            frame(*xaxis, *yaxis)?,
            (turns(*major), turns(*minor)),
        )
        .map(SurfaceKind::Torus)
        .map_err(bad),
        Surface::Nurbs {
            degree,
            knots_u,
            knots_v,
            points,
            weights,
            ..
        } => {
            let net: Vec<[f64; 3]> = points.iter().map(|p| times(*p, s)).collect();
            let basis =
                |p: usize, k: &Vec<f64>| Basis::new(p, k.clone()).map_err(|e| format!("{e:?}"));
            let (bu, bv) = (basis(degree[0], knots_u)?, basis(degree[1], knots_v)?);
            let surface = match weights {
                Some(w) => Nurbs::new_rational(bu, bv, &net, w),
                None => Nurbs::new(bu, bv, &net),
            };
            surface
                .map(SurfaceKind::Nurbs)
                .map_err(|e| format!("{e:?}"))
        }
    }
}

/// The unit axes a plane's parameters run along: x, and y made perpendicular to it. (A corpus
/// plane's y axis is only a direction; its point is `origin + u·x + v·y` on the orthogonal pair.)
fn plane_axes(x: [f64; 3], y: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let unit = |v: [f64; 3]| {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    };
    let x = unit(x);
    let d = x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    (x, unit([y[0] - d * x[0], y[1] - d * x[1], y[2] - d * x[2]]))
}

/// The scale of a case: the smallest power of two from 1024 up that puts its tolerance at 8 steps
/// or more, no larger than 2^34.
fn scale(cov_tol: f64) -> f64 {
    let mut s = 1024.0;
    while cov_tol * s < FLOOR && s < 17_179_869_184.0 {
        s *= 2.0;
    }
    s
}

/// A meter of `work`.
fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

/// The kernel's answer for two surfaces, each stage under its own meter.
fn pipeline(
    a: &SurfaceKind,
    b: &SurfaceKind,
    ceiling: f64,
    work: u64,
) -> Result<Found, IntersectError> {
    let found = subdivide(a, b, FLOOR, &mut meter(work))?;
    let readings = if found.flagged.is_empty() {
        Vec::new()
    } else {
        analyse(a, b, &found, FLOOR, ceiling, &mut meter(work))?
    };
    let mut out = Found::default();
    let mut settled = found.settled.clone();
    for reading in readings {
        match reading {
            Reading::Crossing(pair) => settled.push(pair),
            Reading::Point { contact, .. } => match contact {
                PointContact::IsolatedTangent => out.isolated += 1,
                PointContact::Singular { .. } => out.singular += 1,
                PointContact::Crossing | PointContact::BoundaryTouch => out.other_points += 1,
            },
            Reading::Curve { contact, .. } => {
                if contact != CurveContact::Crossing {
                    out.bands += 1;
                }
            }
            Reading::Overlap { .. } => out.overlaps += 1,
            Reading::Undecided { .. } => out.undecided += 1,
        }
    }
    let all = Subdivided {
        settled,
        flagged: Vec::new(),
    };
    let traced = march(a, b, &all, FLOOR, ceiling, &mut meter(work))?;
    out.branches = traced.branches.len();
    out.closed = traced
        .branches
        .iter()
        .filter(|branch| branch.ends == [CurveEnd::Closed, CurveEnd::Closed])
        .count();
    out.ends = traced.branches.iter().map(|branch| branch.ends).collect();
    out.unproven = traced.unproven.len();
    Ok(out)
}

/// One case run, caught if it panics.
fn run_case(case: &Case, work: u64) -> Ours {
    let s = scale(case.cov_tol);
    let built: Result<Vec<SurfaceKind>, String> =
        case.surfaces.iter().map(|x| kernel_surface(x, s)).collect();
    let surfaces = match built {
        Ok(v) => v,
        Err(why) => return Ours::Untranslated(why),
    };
    let [a, b] = surfaces.as_slice() else {
        return Ours::Untranslated("not two surfaces".into());
    };
    match catch_unwind(AssertUnwindSafe(|| pipeline(a, b, case.cov_tol * s, work))) {
        Ok(Ok(found)) => Ours::Returned(found),
        Ok(Err(e)) => Ours::Refused(e),
        Err(payload) => {
            let text = payload
                .downcast_ref::<&str>()
                .map(|t| (*t).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Ours::Panicked(text)
        }
    }
}

/// One case run on a thread of its own, given up on after `seconds`.
fn run_timed(case: &Case, work: u64, seconds: u64) -> (Ours, Duration) {
    let (tx, rx) = mpsc::channel();
    let owned = case.clone();
    let start = Instant::now();
    let spawned = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let _ = tx.send(run_case(&owned, work));
        });
    if spawned.is_err() {
        return (Ours::Panicked("no thread".into()), start.elapsed());
    }
    match rx.recv_timeout(Duration::from_secs(seconds)) {
        Ok(ours) => (ours, start.elapsed()),
        Err(_) => (Ours::TimedOut, start.elapsed()),
    }
}

/// Ours against the exact topology of `case`.
fn judge(case: &Case, ours: &Ours) -> Verdict {
    let found = match ours {
        Ours::Returned(found) => found,
        Ours::Refused(e) => return Verdict::Refused(*e),
        Ours::Untranslated(_) => return Verdict::Untranslated,
        Ours::Panicked(_) => return Verdict::Panicked,
        Ours::TimedOut => return Verdict::TimedOut,
    };
    if found.flagged() {
        return Verdict::Flagged;
    }
    if case.coincident_regions > 0 {
        return if found.overlaps > 0 {
            Verdict::Right
        } else {
            Verdict::Wrong("no overlap reported".into())
        };
    }
    let closed = case.branches.iter().filter(|b| b.closed).count();
    let mut differs = Vec::new();
    for (name, ours, truth) in [
        ("branches", found.branches, case.branches.len()),
        ("closed", found.closed, closed),
        (
            "isolated points",
            found.isolated,
            case.isolated_points.len(),
        ),
        (
            "singular points",
            found.singular,
            case.singular_points.len(),
        ),
    ] {
        if ours != truth {
            differs.push(format!("{name} {ours} for {truth}"));
        }
    }
    if found.overlaps > 0 {
        differs.push(format!("{} overlap(s) for none", found.overlaps));
    }
    if differs.is_empty() {
        Verdict::Right
    } else {
        Verdict::Wrong(differs.join(", "))
    }
}

/// How ours stands beside `SMLib`'s result on the case.
fn against(cpp: Cpp, verdict: &Verdict) -> &'static str {
    match (cpp, verdict) {
        (Cpp::Fail, _) => "theirs failed",
        (Cpp::Pass, Verdict::Right) => "agree",
        (Cpp::Pass, Verdict::Wrong(_)) => "differs",
        (Cpp::Pass, Verdict::Panicked | Verdict::TimedOut) => "ours broke",
        (Cpp::Pass, _) => "ours refuses",
    }
}

/// What a row says about ours, beyond the verdict's word.
fn detail(ours: &Ours, verdict: &Verdict) -> String {
    match (ours, verdict) {
        (Ours::Returned(f), Verdict::Wrong(d)) => format!("{d}; ends {:?}", f.ends),
        (_, Verdict::Wrong(d)) => d.clone(),
        (Ours::Refused(e), _) => e.to_string(),
        (Ours::Untranslated(w) | Ours::Panicked(w), _) => w.replace('|', "/"),
        (Ours::Returned(f), Verdict::Flagged) => format!(
            "bands {}, undecided {}, unproven {}; branches {}, points {}",
            f.bands,
            f.undecided,
            f.unproven,
            f.branches,
            f.isolated + f.singular + f.other_points
        ),
        _ => String::new(),
    }
}

/// The whole corpus run: the report in Markdown, and the verdicts by case id.
fn run_corpus(work: u64, seconds: u64) -> (String, Vec<(String, Verdict)>) {
    let cases = ssi_corpus::cases().expect("the corpus reads");
    let mut rows = String::new();
    let mut verdicts = Vec::new();
    let mut table: BTreeMap<(&'static str, &'static str), usize> = BTreeMap::new();
    let mut labels: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut classes: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    let mut total = Duration::ZERO;
    let only = std::env::var("SSI_CORPUS_ONLY").unwrap_or_default();
    for case in cases.iter().filter(|c| c.id.contains(&only)) {
        let (ours, took) = run_timed(case, work, seconds);
        total += took;
        let verdict = judge(case, &ours);
        let label = against(case.cpp, &verdict);
        let cpp = if case.cpp == Cpp::Pass {
            "pass"
        } else {
            "fail"
        };
        *table.entry((verdict.word(), cpp)).or_insert(0) += 1;
        *labels.entry(label).or_insert(0) += 1;
        let c = classes.entry(case.class.clone()).or_insert([0; 3]);
        c[0] += 1;
        c[1] += usize::from(verdict == Verdict::Right);
        c[2] += usize::from(case.cpp == Cpp::Pass);
        writeln!(
            rows,
            "| `{}` | {} | {} | {} | {} | {:.2} | {} |",
            case.id,
            case.class,
            verdict.word(),
            cpp,
            label,
            took.as_secs_f64(),
            detail(&ours, &verdict)
        )
        .unwrap();
        verdicts.push((case.id.clone(), verdict));
    }
    let mut report = String::new();
    writeln!(
        report,
        "SSI corpus, 119 cases, work 2^{} a stage\n",
        work.ilog2()
    )
    .unwrap();
    writeln!(
        report,
        "| ours \\ `SMLib` | pass | fail | all |\n| --- | --- | --- | --- |"
    )
    .unwrap();
    for word in [
        "right",
        "wrong",
        "flagged",
        "refused",
        "untranslated",
        "PANIC",
        "TIMEOUT",
    ] {
        let p = table.get(&(word, "pass")).copied().unwrap_or(0);
        let f = table.get(&(word, "fail")).copied().unwrap_or(0);
        writeln!(report, "| {word} | {p} | {f} | {} |", p + f).unwrap();
    }
    writeln!(report, "\nAgainst `SMLib`:").unwrap();
    for (label, n) in &labels {
        writeln!(report, "- {label}: {n}").unwrap();
    }
    writeln!(
        report,
        "\n| class | cases | ours right | `SMLib` passes |\n| --- | --- | --- | --- |"
    )
    .unwrap();
    for (class, [n, right, cpp]) in &classes {
        writeln!(report, "| {class} | {n} | {right} | {cpp} |").unwrap();
    }
    writeln!(report, "\ntotal time {:.1} s\n", total.as_secs_f64()).unwrap();
    writeln!(
        report,
        "| case | class | ours | `SMLib` | against `SMLib` | seconds | detail |\n| --- | --- | --- | --- | --- | --- | --- |"
    )
    .unwrap();
    report.push_str(&rows);
    (report, verdicts)
}

/// The work a stage may spend, from the environment or [`WORK`].
fn work_from_env() -> u64 {
    std::env::var("SSI_CORPUS_WORK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(WORK)
}

/// How long a case may take, from the environment or [`SECONDS`].
fn seconds_from_env() -> u64 {
    std::env::var("SSI_CORPUS_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SECONDS)
}

#[test]
fn the_corpus_reads_and_every_surface_has_a_kind() {
    let cases = ssi_corpus::cases().unwrap();
    assert_eq!(cases.len(), ssi_corpus::CASES);
    for case in &cases {
        let s = scale(case.cov_tol);
        assert!(s >= 1024.0, "{}", case.id);
        assert!(
            case.cov_tol * s >= FLOOR || s >= 17_179_869_184.0,
            "{}",
            case.id
        );
        for surface in &case.surfaces {
            kernel_surface(surface, s).unwrap_or_else(|e| panic!("{}: {e}", case.id));
        }
    }
}

#[test]
fn a_plane_meets_a_sphere_in_one_circle() {
    // The corpus' own first sphere case, run alone: one closed branch.
    let cases = ssi_corpus::cases().unwrap();
    let case = cases
        .iter()
        .find(|c| c.id == "c1_plane_sphere_circle")
        .unwrap();
    let (ours, _) = run_timed(case, work_from_env(), seconds_from_env());
    let verdict = judge(case, &ours);
    assert!(
        !matches!(verdict, Verdict::Panicked | Verdict::TimedOut),
        "{ours:?}"
    );
}

#[test]
fn judging_reads_the_counts_against_the_truth() {
    let cases = ssi_corpus::cases().unwrap();
    let case = cases
        .iter()
        .find(|c| c.id == "c1_plane_sphere_circle")
        .unwrap();
    let one_loop = Found {
        branches: 1,
        closed: 1,
        ..Found::default()
    };
    assert_eq!(
        judge(case, &Ours::Returned(one_loop.clone())),
        Verdict::Right
    );
    let open = Found {
        closed: 0,
        ..one_loop.clone()
    };
    assert_eq!(
        judge(case, &Ours::Returned(open)),
        Verdict::Wrong("closed 0 for 1".into())
    );
    let flagged = Found {
        undecided: 1,
        ..one_loop
    };
    assert_eq!(judge(case, &Ours::Returned(flagged)), Verdict::Flagged);
    assert_eq!(judge(case, &Ours::TimedOut), Verdict::TimedOut);
    assert_eq!(against(Cpp::Fail, &Verdict::Flagged), "theirs failed");
    assert_eq!(against(Cpp::Pass, &Verdict::Right), "agree");
    assert_eq!(against(Cpp::Pass, &Verdict::Flagged), "ours refuses");
    assert_eq!(
        against(Cpp::Pass, &Verdict::Wrong(String::new())),
        "differs"
    );
}

/// The corpus, all of it, as a benchmark: it reports every difference and fails on none, only on a
/// panic or a case that runs past its time.
#[test]
#[ignore = "the whole corpus: run in release for the report"]
fn report() {
    let (report, verdicts) = run_corpus(work_from_env(), seconds_from_env());
    println!("{report}");
    if let Ok(path) = std::env::var("SSI_CORPUS_REPORT") {
        std::fs::write(path, &report).unwrap();
    }
    for (id, verdict) in &verdicts {
        assert!(
            !matches!(verdict, Verdict::Panicked | Verdict::TimedOut),
            "{id}: {}",
            verdict.word()
        );
    }
}
