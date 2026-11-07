//! Sketch solvers as oracles: ezpz and fiksi, each out of process, against the block solver of
//! `geom-sketch` (piece G222's differential runs; chapters 15.16 and 28.10).
//!
//! What it does: [`jobs`] draws a night's sketches for one oracle from a true configuration: a fixed
//! first point, then each further point fixed by two equations of kinds that oracle has (two
//! distances, a midpoint, a mirror image, a distance from a line, a level or plumb line of a length,
//! an angle, a parallel or a right angle with a length, an equal length), every value measured on the
//! truth, and every point but the fixed one starting a little away from it. A job is the neutral description: each
//! harness in the oracle repository writes it in its own solver's form. [`judge`] writes the same job
//! in `sketch v1`, solves it with `geom_sketch::solve`, and holds both answers to the truth.
//!
//! Guarantees:
//!
//! - **Drawn from a truth.** Every value is measured on a known configuration, so a difference is
//!   settled on the spot: the truth says which side is off.
//! - **Each oracle sees only what it can state.** fiksi has no midpoint and no mirror rule, so its
//!   sketches are drawn from the other kinds; level and plumb lines its harness states as parallel to
//!   a fixed axis.
//! - **Grouped by what the oracle did.** Where the kernel is on the truth and the oracle is not,
//!   the difference's family says how: `another-root` when the oracle's answer meets every rule
//!   within its tolerance (a sketch has other solutions than the drawn one), `unmet` when it does
//!   not, `refused` when it gave none, `short` when it gave too few points. Where the kernel is off
//!   the truth, the family is the job's kinds of rule, the kernel's to fix.
//! - **Within a tolerance, never bits** (15.16): the kernel's points and ezpz's agree within a grid
//!   step of the truth. fiksi stops once its residuals' sum of squares is below 1e-8 in a system
//!   divided by the root mean square of its coordinates and lengths (`solve/lm.rs`,
//!   `assemble/mod.rs` at the pinned commit), so its points are only as near as that: its answer
//!   agrees within [`FIKSI_SLACK`] times 1e-4 of the job's scale ([`near`]), still far inside the
//!   two units that separate a sketch's branches.
//! - **Deterministic.** A seed gives the same jobs on every target.
//!
//! Method: chapter 15.16's verdict on ezpz and fiksi ("running both as differential test oracles,
//! solved positions compared within tolerance, not bits"), and 28.10
//! (`projects/engines/geometry/architecture/15-sketches.md`,
//! `projects/engines/geometry/architecture/28-testing.md`). Both run out of process: ezpz's numerics
//! bring in `paste`, which the `RustSec` database marks unmaintained and `deny.toml` refuses anywhere in the graph,
//! and fiksi is not on crates.io, whose registry is the only source `deny.toml` allows. The fuller
//! sketch corpus and the other solvers come with G231.
//!
//! Cost: linear in the jobs; a judgement is one parse and one solve.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "the names are the geometry's: a, b and c points, k the new one, u and v directions"
)]

use core::fmt::Write;
use core::num::NonZeroU64;

use geom_num::math;
use geom_num::real::Real;
use geom_num::scalar::Scalar;
use geom_run::{Budget, WorkMeter};
use geom_sketch::model::ItemId;
use geom_sketch::solve::solve;
use geom_sketch::text::parse;
use serde::{Deserialize, Serialize};

use super::exact::{float, read_float};
use super::geomdl::Compared;
use super::triage::Signature;
use crate::SplitMix64;
use crate::seed::Seed;

/// ezpz, Zoo's sketch solver, in the catalogue.
pub const EZPZ: &str = "ezpz";

/// fiksi, Linebender's sketch solver, in the catalogue.
pub const FIKSI: &str = "fiksi";

/// Grid steps to one unit of a job, as the stored ezpz problems are written.
pub const STEPS: f64 = 1024.0;

/// How far a solved point may be from the truth, in units: one grid step.
pub const NEAR: f64 = 1.0 / STEPS;

/// fiksi's stopping residual, relative to its system's scale: the square root of the 1e-8 its
/// Levenberg–Marquardt loop stops below.
pub const FIKSI_STOP: f64 = 1e-4;

/// How many of fiksi's stopping residuals its points may stray: the sketches' conditioning turns a
/// residual into a distance. Over a thousand jobs the largest ratio measured was 22, with fiksi's
/// level and plumb lines held by axis lines as long as the sketch is wide.
pub const FIKSI_SLACK: f64 = 64.0;

/// How far each start is moved from the truth, at most, in units on each axis.
const SHAKE: f64 = 0.05;

/// One rule of a sketch, on the job's points by their places.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Rule {
    /// The point stays where it starts, on both axes.
    Fix {
        /// The point.
        point: usize,
    },
    /// Two points are `value` apart.
    Distance {
        /// One point.
        a: usize,
        /// The other.
        b: usize,
        /// The distance.
        value: String,
    },
    /// Two points have one y.
    Level {
        /// One point.
        a: usize,
        /// The other.
        b: usize,
    },
    /// Two points have one x.
    Plumb {
        /// One point.
        a: usize,
        /// The other.
        b: usize,
    },
    /// The line through the first two points is parallel to the one through the last two.
    Parallel {
        /// The lines' points.
        lines: [usize; 4],
    },
    /// The lines are at right angles.
    Perpendicular {
        /// The lines' points.
        lines: [usize; 4],
    },
    /// The second line is the first turned by `half_turns`, modulo half a turn.
    Angle {
        /// The lines' points.
        lines: [usize; 4],
        /// The angle, in half-turns.
        half_turns: String,
    },
    /// `point` is `signed` from the line through `line`, positive on its left.
    FromLine {
        /// The point.
        point: usize,
        /// The line's points.
        line: [usize; 2],
        /// The signed distance.
        signed: String,
    },
    /// `point` is halfway from `a` to `b`.
    Midpoint {
        /// The middle.
        point: usize,
        /// One end.
        a: usize,
        /// The other.
        b: usize,
    },
    /// `a` and `b` are mirror images in the line through `line`.
    Mirror {
        /// One point.
        a: usize,
        /// Its image.
        b: usize,
        /// The line's points.
        line: [usize; 2],
    },
    /// The two lines are as long as each other.
    EqualLength {
        /// The lines' points.
        lines: [usize; 4],
    },
}

/// One sketch, as every harness reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Its number.
    pub id: u64,
    /// The kinds it uses, in order: its family for triage.
    pub family: String,
    /// Where each point starts, in units.
    pub points: Vec<[String; 2]>,
    /// Its rules.
    pub rules: Vec<Rule>,
    /// Where each point truly is, in units; harnesses ignore it.
    pub truth: Vec<[String; 2]>,
}

/// An oracle's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// The job's number.
    pub id: u64,
    /// Each point solved, in units; `None` if the oracle refused or could not solve.
    pub points: Option<Vec<[String; 2]>>,
}

type P = [f64; 2];

fn minus(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(u: P, v: P) -> f64 {
    u[0] * v[1] - u[1] * v[0]
}

fn dot(u: P, v: P) -> f64 {
    u[0] * v[0] + u[1] * v[1]
}

fn size(u: P) -> f64 {
    math::sqrt(dot(u, u)).map_or(0.0, Real::get)
}

/// A uniform double in [lo, hi), from 53 bits.
fn between(rng: &mut SplitMix64, lo: f64, hi: f64) -> f64 {
    let bits = rng.next_u64() >> 11;
    // A 53-bit whole number is exact as a double.
    #[allow(clippy::cast_precision_loss, reason = "53 bits, exact")]
    let unit = bits as f64 / 9_007_199_254_740_992.0;
    lo + (hi - lo) * unit
}

fn below(rng: &mut SplitMix64, n: usize) -> usize {
    let n = u64::try_from(n).ok().and_then(NonZeroU64::new);
    n.map_or(0, |n| usize::try_from(rng.below(n)).unwrap_or(0))
}

fn sign(rng: &mut SplitMix64) -> f64 {
    if below(rng, 2) == 0 { 1.0 } else { -1.0 }
}

fn text(x: f64) -> String {
    float(x).unwrap_or_default()
}

fn at(truth: &[P], i: usize) -> P {
    truth.get(i).copied().unwrap_or([0.0, 0.0])
}

/// Two distinct earlier points, well apart.
fn two(rng: &mut SplitMix64, truth: &[P]) -> (usize, usize) {
    loop {
        let (a, b) = (below(rng, truth.len()), below(rng, truth.len()));
        if a != b && size(minus(at(truth, a), at(truth, b))) > 2.0 {
            return (a, b);
        }
    }
}

/// Whether `p` is well clear of `a`, `b` and the line through them.
fn clear(p: P, a: P, b: P) -> bool {
    let (u, v, w) = (minus(b, a), minus(p, a), minus(p, b));
    size(v) > 2.0 && size(w) > 2.0 && cross(u, v).abs() > 0.3 * size(u) * size(v)
}

/// One sketch of `count` points, its rules of the kinds `fiksi` can state if it is set.
fn draw(rng: &mut SplitMix64, count: usize, fiksi: bool) -> (Vec<P>, Vec<Rule>) {
    let mut truth = vec![[between(rng, -20.0, 20.0), between(rng, -20.0, 20.0)]];
    let mut rules = vec![Rule::Fix { point: 0 }];
    let along = between(rng, 3.0, 20.0) * sign(rng);
    let plumb = below(rng, 2) == 0;
    let first = at(&truth, 0);
    truth.push(if plumb {
        [first[0], first[1] + along]
    } else {
        [first[0] + along, first[1]]
    });
    rules.push(if plumb {
        Rule::Plumb { a: 0, b: 1 }
    } else {
        Rule::Level { a: 0, b: 1 }
    });
    rules.push(Rule::Distance {
        a: 0,
        b: 1,
        value: text(along.abs()),
    });
    while truth.len() < count {
        let k = truth.len();
        let (a, b) = two(rng, &truth);
        let c = below(rng, k);
        let (pa, pb, pc) = (at(&truth, a), at(&truth, b), at(&truth, c));
        let free = [
            pc[0] + between(rng, -15.0, 15.0),
            pc[1] + between(rng, -15.0, 15.0),
        ];
        let distance = |p: P, q: P, a, b| Rule::Distance {
            a,
            b,
            value: text(size(minus(q, p))),
        };
        match below(rng, 10) {
            0 if clear(free, pa, pb) => {
                truth.push(free);
                rules.push(distance(pa, free, a, k));
                rules.push(distance(pb, free, b, k));
            }
            1 if !fiksi => {
                truth.push([f64::midpoint(pa[0], pb[0]), f64::midpoint(pa[1], pb[1])]);
                rules.push(Rule::Midpoint { point: k, a, b });
            }
            2 if !fiksi && c != a && c != b && clear(pc, pa, pb) => {
                let u = minus(pb, pa);
                let t = dot(minus(pc, pa), u) / dot(u, u);
                let foot = [pa[0] + t * u[0], pa[1] + t * u[1]];
                truth.push([2.0 * foot[0] - pc[0], 2.0 * foot[1] - pc[1]]);
                rules.push(Rule::Mirror {
                    a: c,
                    b: k,
                    line: [a, b],
                });
            }
            3 if c != a && c != b && clear(free, pa, pb) && clear(free, pc, pa) => {
                let u = minus(pb, pa);
                let (n, w) = ([-u[1], u[0]], minus(free, pc));
                if cross(n, w).abs() > 0.3 * size(n) * size(w) {
                    let signed = cross(u, minus(free, pa)) / size(u);
                    truth.push(free);
                    rules.push(Rule::FromLine {
                        point: k,
                        line: [a, b],
                        signed: text(signed),
                    });
                    rules.push(distance(pc, free, c, k));
                }
            }
            4 | 5 if size(minus(free, pc)) > 2.0 => {
                let plumb = below(rng, 2) == 0;
                let d = between(rng, 3.0, 15.0) * sign(rng);
                let p = if plumb {
                    [pc[0], pc[1] + d]
                } else {
                    [pc[0] + d, pc[1]]
                };
                truth.push(p);
                rules.push(if plumb {
                    Rule::Plumb { a: c, b: k }
                } else {
                    Rule::Level { a: c, b: k }
                });
                rules.push(distance(pc, p, c, k));
            }
            6 if size(minus(free, pc)) > 2.0 => {
                let (u, v) = (minus(pb, pa), minus(free, pc));
                let half_turns = math::atan2(cross(u, v), dot(u, v))
                    .and_then(|r| r.div(Real::pi()))
                    .map_or(0.0, Real::get);
                truth.push(free);
                rules.push(Rule::Angle {
                    lines: [a, b, c, k],
                    half_turns: text(half_turns),
                });
                rules.push(distance(pc, free, c, k));
            }
            7 | 8 => {
                let u = minus(pb, pa);
                let unit = [u[0] / size(u), u[1] / size(u)];
                let square = below(rng, 2) == 0;
                let dir = if square { [-unit[1], unit[0]] } else { unit };
                let d = between(rng, 3.0, 15.0) * sign(rng);
                let p = [pc[0] + d * dir[0], pc[1] + d * dir[1]];
                truth.push(p);
                let lines = [a, b, c, k];
                rules.push(if square {
                    Rule::Perpendicular { lines }
                } else {
                    Rule::Parallel { lines }
                });
                rules.push(distance(pc, p, c, k));
            }
            9 => {
                let d = size(minus(pb, pa)) * sign(rng);
                truth.push([pc[0] + d, pc[1]]);
                rules.push(Rule::EqualLength {
                    lines: [a, b, c, k],
                });
                rules.push(Rule::Level { a: c, b: k });
            }
            _ => {}
        }
    }
    (truth, rules)
}

fn kind_name(rule: &Rule) -> &'static str {
    match rule {
        Rule::Fix { .. } => "fix",
        Rule::Distance { .. } => "distance",
        Rule::Level { .. } => "level",
        Rule::Plumb { .. } => "plumb",
        Rule::Parallel { .. } => "parallel",
        Rule::Perpendicular { .. } => "perpendicular",
        Rule::Angle { .. } => "angle",
        Rule::FromLine { .. } => "from-line",
        Rule::Midpoint { .. } => "midpoint",
        Rule::Mirror { .. } => "mirror",
        Rule::EqualLength { .. } => "equal-length",
    }
}

/// The night's `count` jobs for `oracle` from `seed`; `None` for an oracle with no sketch jobs.
#[must_use]
pub fn jobs(oracle: &str, seed: Seed, count: u64) -> Option<Vec<Job>> {
    let fiksi = match oracle {
        EZPZ => false,
        FIKSI => true,
        _ => return None,
    };
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::new();
    for id in 0..count {
        let points =
            3_usize.saturating_add(usize::try_from(id.checked_rem(10).unwrap_or(0)).unwrap_or(0));
        let (truth, rules) = draw(&mut rng, points, fiksi);
        // The fixed first point starts on its truth, where its rule holds it; the rest are shaken.
        let starts = truth
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let shake = if i == 0 { 0.0 } else { SHAKE };
                [
                    text(p[0] + between(&mut rng, -shake, shake)),
                    text(p[1] + between(&mut rng, -shake, shake)),
                ]
            })
            .collect();
        let mut kinds: Vec<&str> = rules.iter().map(kind_name).collect();
        kinds.sort_unstable();
        kinds.dedup();
        out.push(Job {
            id,
            family: kinds.join("+"),
            points: starts,
            rules,
            truth: truth.iter().map(|p| [text(p[0]), text(p[1])]).collect(),
        });
    }
    Some(out)
}

fn read(t: &str, id: u64) -> Result<f64, String> {
    read_float(t).ok_or_else(|| format!("job {id}: bad number {t:?}"))
}

fn pairs(list: &[[String; 2]], id: u64) -> Result<Vec<P>, String> {
    list.iter()
        .map(|[x, y]| Ok([read(x, id)?, read(y, id)?]))
        .collect()
}

/// `job` in `sketch v1`, its lengths in grid steps.
///
/// # Errors
///
/// A job that cannot be read, or a rule naming a point it does not have.
pub fn sketch_text(job: &Job) -> Result<String, String> {
    let fail = |e: core::fmt::Error| e.to_string();
    let starts = pairs(&job.points, job.id)?;
    let point = |i: usize| -> Result<usize, String> {
        if i < starts.len() {
            Ok(i)
        } else {
            Err(format!("job {}: no point {i}", job.id))
        }
    };
    let mut out = String::from("sketch v1\nplane lattice 0 0 0 1 0 0 0 1 0 q0\n");
    for (k, p) in starts.iter().enumerate() {
        writeln!(out, "entity 1.{k} point {} {}", p[0] * STEPS, p[1] * STEPS).map_err(fail)?;
    }
    let (mut lines, mut rules) = (String::new(), String::new());
    let mut line_count = 0_usize;
    let mut line = |a: usize, b: usize, lines: &mut String| -> Result<String, String> {
        let id = format!("2.{line_count}");
        writeln!(
            lines,
            "entity {id} line 1.{} 1.{} segment",
            point(a)?,
            point(b)?
        )
        .map_err(fail)?;
        line_count = line_count.saturating_add(1);
        Ok(id)
    };
    for (n, rule) in job.rules.iter().enumerate() {
        let body = match rule {
            Rule::Fix { point: p } => {
                let at = starts.get(point(*p)?).copied().unwrap_or([0.0, 0.0]);
                writeln!(
                    rules,
                    "constraint 3.{n} driving 0 fix 1.{p} 0 {}",
                    at[0] * STEPS
                )
                .map_err(fail)?;
                format!("fix 1.{p} 1 {}", at[1] * STEPS)
            }
            Rule::Distance { a, b, value } => format!(
                "distance 1.{} 1.{} {}",
                point(*a)?,
                point(*b)?,
                read(value, job.id)? * STEPS
            ),
            Rule::Level { a, b } => format!("horizontal {}", line(*a, *b, &mut lines)?),
            Rule::Plumb { a, b } => format!("vertical {}", line(*a, *b, &mut lines)?),
            Rule::Parallel { lines: l } | Rule::Perpendicular { lines: l } => {
                let kind = if matches!(rule, Rule::Parallel { .. }) {
                    "parallel"
                } else {
                    "perpendicular"
                };
                let (first, second) =
                    (line(l[0], l[1], &mut lines)?, line(l[2], l[3], &mut lines)?);
                format!("{kind} {first} {second}")
            }
            Rule::Angle {
                lines: l,
                half_turns,
            } => {
                let (first, second) =
                    (line(l[0], l[1], &mut lines)?, line(l[2], l[3], &mut lines)?);
                format!("turned {first} {second} {}", read(half_turns, job.id)?)
            }
            Rule::FromLine {
                point: p,
                line: l,
                signed,
            } => {
                let signed = read(signed, job.id)?;
                let id = line(l[0], l[1], &mut lines)?;
                let side = if signed > 0.0 { "left" } else { "right" };
                format!(
                    "point-line 1.{} {id} {} {side}",
                    point(*p)?,
                    signed.abs() * STEPS
                )
            }
            Rule::Midpoint { point: m, a, b } => {
                format!(
                    "midpoint 1.{} 1.{} 1.{}",
                    point(*m)?,
                    point(*a)?,
                    point(*b)?
                )
            }
            Rule::Mirror { a, b, line: l } => {
                let id = line(l[0], l[1], &mut lines)?;
                format!("symmetric 1.{} 1.{} {id}", point(*a)?, point(*b)?)
            }
            Rule::EqualLength { lines: l } => {
                let (first, second) =
                    (line(l[0], l[1], &mut lines)?, line(l[2], l[3], &mut lines)?);
                format!("equal-length {first} {second}")
            }
        };
        writeln!(rules, "constraint 4.{n} driving 0 {body}").map_err(fail)?;
    }
    out.push_str(&lines);
    out.push_str(&rules);
    Ok(out)
}

/// The kernel's solution of `job`, in units; `None` if the block solver refused it.
///
/// # Errors
///
/// A job that cannot be written as a sketch, or one the sketch reader refuses: the job generator is
/// broken, not the solver.
pub fn kernel(job: &Job) -> Result<Option<Vec<P>>, String> {
    let text = sketch_text(job)?;
    let sketch = parse(&text).map_err(|e| format!("job {}: {e:?}", job.id))?;
    let Ok(solved) = solve(&sketch, &mut WorkMeter::new(Budget::UNLIMITED)) else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for k in 0..job.points.len() {
        let key = u64::try_from(k).map_err(|e| e.to_string())?;
        let [x, y] = solved
            .sketch
            .point(ItemId::new(1, key))
            .map_err(|e| format!("job {}: {e:?}", job.id))?;
        out.push([x / STEPS, y / STEPS]);
    }
    Ok(Some(out))
}

/// How near `oracle`'s answer to `job` must be to the truth, in units: [`NEAR`], or for fiksi
/// [`FIKSI_SLACK`] times its stopping residual at the job's scale, the root mean square of its
/// starts' coordinates and its lengths, as fiksi measures its own, if that is wider.
///
/// # Errors
///
/// A job that cannot be read.
pub fn near(oracle: &str, job: &Job) -> Result<f64, String> {
    if oracle != FIKSI {
        return Ok(NEAR);
    }
    let mut values: Vec<f64> = pairs(&job.points, job.id)?.into_iter().flatten().collect();
    for rule in &job.rules {
        match rule {
            Rule::Distance { value: v, .. } | Rule::FromLine { signed: v, .. } => {
                values.push(read(v, job.id)?);
            }
            _ => {}
        }
    }
    let mut squares = 0.0;
    for v in &values {
        squares += v * v;
    }
    #[allow(clippy::cast_precision_loss, reason = "a job holds a few dozen values")]
    let mean = squares / values.len().max(1) as f64;
    let scale = math::sqrt(mean).map_or(0.0, Real::get);
    let wide = FIKSI_SLACK * FIKSI_STOP * scale;
    Ok(if wide > NEAR { wide } else { NEAR })
}

/// Whether every point of `solved` is within `near` of `truth`.
fn on_truth(solved: Option<&[P]>, truth: &[P], near: f64) -> bool {
    solved.is_some_and(|s| {
        s.len() == truth.len()
            && s.iter()
                .zip(truth)
                .all(|(p, t)| size(minus(*p, *t)) <= near)
    })
}

/// How far `points` are from meeting `rule`, in units: a distance in every case, so that one
/// tolerance reads them all. An angle is the distance the second line's far end is from where the
/// angle puts it.
fn missed(rule: &Rule, points: &[P], starts: &[P]) -> f64 {
    let p = |i: usize| at(points, i);
    let turn = |u: P, half_turns: f64| {
        let (c, s) = (
            math::cospi(half_turns).map_or(1.0, Real::get),
            math::sinpi(half_turns).map_or(0.0, Real::get),
        );
        [u[0] * c - u[1] * s, u[0] * s + u[1] * c]
    };
    let across = |l: [usize; 4], half_turns: f64| {
        let (u, v) = (minus(p(l[1]), p(l[0])), minus(p(l[3]), p(l[2])));
        let w = turn(u, half_turns);
        let length = size(w);
        if length == 0.0 {
            size(v)
        } else {
            cross(w, v).abs() / length
        }
    };
    match rule {
        Rule::Fix { point } => size(minus(p(*point), at(starts, *point))),
        Rule::Distance { a, b, value } => {
            (size(minus(p(*b), p(*a))) - read_float(value).unwrap_or(f64::NAN)).abs()
        }
        Rule::Level { a, b } => (p(*b)[1] - p(*a)[1]).abs(),
        Rule::Plumb { a, b } => (p(*b)[0] - p(*a)[0]).abs(),
        Rule::Parallel { lines } => across(*lines, 0.0),
        Rule::Perpendicular { lines } => across(*lines, 0.5),
        Rule::Angle { lines, half_turns } => {
            across(*lines, read_float(half_turns).unwrap_or(f64::NAN))
        }
        Rule::FromLine {
            point,
            line,
            signed,
        } => {
            let u = minus(p(line[1]), p(line[0]));
            let length = size(u);
            let off = if length == 0.0 {
                f64::NAN
            } else {
                cross(u, minus(p(*point), p(line[0]))) / length
            };
            (off - read_float(signed).unwrap_or(f64::NAN)).abs()
        }
        Rule::Midpoint { point, a, b } => {
            let (pa, pb) = (p(*a), p(*b));
            size(minus(
                p(*point),
                [f64::midpoint(pa[0], pb[0]), f64::midpoint(pa[1], pb[1])],
            ))
        }
        Rule::Mirror { a, b, line } => {
            let (o, u) = (p(line[0]), minus(p(line[1]), p(line[0])));
            let (pa, uu) = (p(*a), dot(u, u));
            if uu == 0.0 {
                return f64::NAN;
            }
            let t = dot(minus(pa, o), u) / uu;
            let foot = [o[0] + t * u[0], o[1] + t * u[1]];
            size(minus(p(*b), [2.0 * foot[0] - pa[0], 2.0 * foot[1] - pa[1]]))
        }
        Rule::EqualLength { lines } => {
            (size(minus(p(lines[1]), p(lines[0]))) - size(minus(p(lines[3]), p(lines[2])))).abs()
        }
    }
}

/// What an oracle that is off the truth did: [`judge`]'s family for it.
fn how_off(job: &Job, theirs: Option<&[P]>, starts: &[P], near: f64) -> &'static str {
    match theirs {
        None => "refused",
        Some(points) if points.len() != starts.len() => "short",
        // A NaN miss (a line of no length) is not met.
        Some(points) if job.rules.iter().all(|r| missed(r, points, starts) <= near) => {
            "another-root"
        }
        Some(_) => "unmet",
    }
}

/// Judges `oracle`'s answer to `job` against the kernel's and the truth.
///
/// # Errors
///
/// A job or answer that cannot be read: the harness or the generator is broken.
pub fn judge(oracle: &str, job: &Job, answer: &Answer) -> Result<Compared, String> {
    if job.id != answer.id {
        return Err(format!("answer {} for job {}", answer.id, job.id));
    }
    let truth = pairs(&job.truth, job.id)?;
    let ours = kernel(job)?;
    let theirs = answer
        .points
        .as_ref()
        .map(|p| pairs(p, job.id))
        .transpose()?;
    let reach = near(oracle, job)?;
    let (good_ours, good_theirs) = (
        on_truth(ours.as_deref(), &truth, NEAR),
        on_truth(theirs.as_deref(), &truth, reach),
    );
    let starts = pairs(&job.points, job.id)?;
    let family = if good_ours {
        how_off(job, theirs.as_deref(), &starts, reach).to_owned()
    } else {
        job.family.clone()
    };
    let (kind, settled) = match (good_ours, good_theirs) {
        (true, true) => return Ok(Compared::Agree),
        (true, false) => (
            "oracle",
            "the kernel is on the truth, and the oracle is not",
        ),
        (false, true) => (
            "kernel",
            "the oracle is on the truth, and the kernel is not",
        ),
        (false, false) => ("both", "neither is on the truth"),
    };
    Ok(Compared::Differ {
        signature: Signature {
            operation: "solve".to_owned(),
            oracle: oracle.to_owned(),
            kind: kind.to_owned(),
            family,
        },
        settled: settled.to_owned(),
    })
}
