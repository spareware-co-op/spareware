//! Generated sketch families with known diagnoses, and the differential check of degrees of freedom,
//! redundancy and conflicts (piece G231; chapters 15.8, 15.16 and 28.10).
//!
//! What it does: [`cases`] draws a night's sketches from six named families, each built so that its
//! diagnosis is known by construction, and each a [`super::sketches::Job`] every sketch oracle
//! already reads:
//!
//! - **chain**: a fixed point, then each next point a level or plumb line and a length from the last.
//!   Exactly determined.
//! - **loose**: a chain with the level or plumb line left off some points, each of which then swings
//!   freely about the last: as many degrees of freedom as points left loose.
//! - **ladder**: a grid of points held by levels, plumbs and the widths and heights of its first row
//!   and column. Exactly determined.
//! - **growth**: a sketch of the kinds of G222's differential runs, each point fixed by two rules.
//!   Exactly determined, but for the special configurations a drawing can fall into.
//! - **redundant**: a chain with one of its lengths stated twice. One redundant group, which holds.
//! - **conflicting**: a chain with one of its lengths stated again as another number. One conflict.
//!
//! [`check`] runs the kernel's own diagnosis (`geom_sketch::diagnose`) on a case and against its
//! expectation; [`judge`] holds an oracle's report of the same three counts to the expectation and to
//! the kernel's, grouped as the position judgements of `sketches` are.
//!
//! Guarantees:
//!
//! - **Known by construction.** The expected counts follow from how each family is built, not from
//!   running any solver, so a disagreement names the side that is off.
//! - **Consistent or not, said so.** A case whose rules cannot all hold ([`Expect::consistent`] is
//!   false) has a truth that does not meet them; the position judgements skip it, and the kernel's
//!   solve must refuse it.
//! - **Deterministic.** A seed gives the same cases on every target.
//!
//! Method: chapter 15.8's diagnosis and 15.16's differential runs
//! (`projects/engines/geometry/architecture/15-sketches.md`), and 28.10's rules for oracles
//! (`projects/engines/geometry/architecture/28-testing.md`). The expected counts are the ranks of the
//! families' equations, which the construction makes plain.
//!
//! Cost: linear in the cases; a check is one parse, one diagnosis and one solve.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "the indices of a chain and a grid, bounded by the sizes this module draws: a dozen points, six rows and columns"
)]

use core::fmt::Write;

use geom_run::{Budget, WorkMeter};
use geom_sketch::diagnose::diagnose;
use geom_sketch::solve::solve;
use geom_sketch::text::parse;
use serde::{Deserialize, Serialize};

use super::geomdl::Compared;
use super::sketches::{
    Job, Kinds, P, Rule, STEPS, below, between, draw, kind_name, missed, pairs, sign, sketch_text,
    text,
};
use super::triage::Signature;
use crate::SplitMix64;
use crate::seed::Seed;

/// How far a rule of a solved case may stray from holding, in units: one grid step.
pub const HOLDS: f64 = 1.0 / STEPS;

/// How far each start is moved from the truth, at most, in units on each axis.
const SHAKE: f64 = 0.05;

/// A named family of sketches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// A fixed point and a level or plumb line and a length to each next.
    Chain,
    /// A chain with the level or plumb line left off some points.
    Loose,
    /// A grid held by levels, plumbs and the widths and heights of its first row and column.
    Ladder,
    /// Each point fixed by two rules of the kinds of G222's differential runs.
    Growth,
    /// A chain with one length stated twice.
    Redundant,
    /// A chain with one length stated again as another number.
    Conflicting,
}

impl Family {
    /// Every family, in the order the cases cycle through them.
    pub const ALL: [Family; 6] = [
        Family::Chain,
        Family::Loose,
        Family::Ladder,
        Family::Growth,
        Family::Redundant,
        Family::Conflicting,
    ];

    /// Its name, as a job's family and a signature carry it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Family::Chain => "chain",
            Family::Loose => "loose",
            Family::Ladder => "ladder",
            Family::Growth => "growth",
            Family::Redundant => "redundant",
            Family::Conflicting => "conflicting",
        }
    }
}

/// What a case's diagnosis must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expect {
    /// Degrees of freedom.
    pub freedom: usize,
    /// Redundant groups: dependent equations that hold.
    pub redundant: usize,
    /// Conflicting groups: dependent equations that cannot hold.
    pub conflicting: usize,
    /// Whether every rule can hold at once.
    pub consistent: bool,
}

/// One sketch with its expected diagnosis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    /// The family it was drawn from.
    pub family: Family,
    /// The sketch, as every harness reads it.
    pub job: Job,
    /// Its diagnosis.
    pub expect: Expect,
}

/// An oracle's report of a case's diagnosis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    /// The job's number.
    pub id: u64,
    /// Degrees of freedom; `None` if the oracle gave none.
    pub freedom: Option<usize>,
    /// Redundant constraints it named, as groups.
    pub redundant: Option<usize>,
    /// Conflicting constraints it named, as groups.
    pub conflicting: Option<usize>,
}

fn rule_of(a: usize, b: usize, along: f64, plumb: bool) -> [Rule; 2] {
    [
        if plumb {
            Rule::Plumb { a, b }
        } else {
            Rule::Level { a, b }
        },
        Rule::Distance {
            a,
            b,
            value: text(along.abs()),
        },
    ]
}

/// A chain of `count` points: the truth and the rules, the rules of each point from the second on
/// being a level or plumb line and a length unless `loose` leaves its level or plumb off.
fn chain(rng: &mut SplitMix64, count: usize, loose: &[bool]) -> (Vec<P>, Vec<Rule>) {
    let mut truth = vec![[between(rng, -20.0, 20.0), between(rng, -20.0, 20.0)]];
    let mut rules = vec![Rule::Fix { point: 0 }];
    for k in 1..count {
        let last = truth.get(k - 1).copied().unwrap_or([0.0, 0.0]);
        let along = between(rng, 3.0, 15.0) * sign(rng);
        let plumb = below(rng, 2) == 0;
        let p = if plumb {
            [last[0], last[1] + along]
        } else {
            [last[0] + along, last[1]]
        };
        truth.push(p);
        let [direction, length] = rule_of(k - 1, k, along, plumb);
        if !loose.get(k).copied().unwrap_or(false) {
            rules.push(direction);
        }
        rules.push(length);
    }
    (truth, rules)
}

/// A grid of `rows` by `columns` points held by levels, plumbs and the first row's and column's
/// widths and heights.
fn ladder(rng: &mut SplitMix64, rows: usize, columns: usize) -> (Vec<P>, Vec<Rule>) {
    let (x0, y0) = (between(rng, -20.0, 20.0), between(rng, -20.0, 20.0));
    let widths: Vec<f64> = (1..columns).map(|_| between(rng, 3.0, 12.0)).collect();
    let heights: Vec<f64> = (1..rows).map(|_| between(rng, 3.0, 12.0)).collect();
    let (mut xs, mut ys) = (vec![x0], vec![y0]);
    for w in &widths {
        xs.push(xs.last().copied().unwrap_or(0.0) + w);
    }
    for h in &heights {
        ys.push(ys.last().copied().unwrap_or(0.0) + h);
    }
    let at = |i: usize, j: usize| i * columns + j;
    let mut truth = Vec::new();
    for y in &ys {
        for x in &xs {
            truth.push([*x, *y]);
        }
    }
    let mut rules = vec![Rule::Fix { point: 0 }];
    for i in 0..rows {
        for j in 0..columns - 1 {
            rules.push(Rule::Level {
                a: at(i, j),
                b: at(i, j + 1),
            });
        }
    }
    for j in 0..columns {
        for i in 0..rows - 1 {
            rules.push(Rule::Plumb {
                a: at(i, j),
                b: at(i + 1, j),
            });
        }
    }
    for (j, w) in widths.iter().enumerate() {
        rules.push(Rule::Distance {
            a: at(0, j),
            b: at(0, j + 1),
            value: text(*w),
        });
    }
    for (i, h) in heights.iter().enumerate() {
        rules.push(Rule::Distance {
            a: at(i, 0),
            b: at(i + 1, 0),
            value: text(*h),
        });
    }
    (truth, rules)
}

/// The `count` cases of a night from `seed`, the families in turn.
#[must_use]
pub fn cases(seed: Seed, count: u64) -> Vec<Case> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::new();
    for id in 0..count {
        let family = Family::ALL
            .get(usize::try_from(id % 6).unwrap_or(0))
            .copied()
            .unwrap_or(Family::Chain);
        let points = 3_usize.saturating_add(below(&mut rng, 6));
        let mut expect = Expect {
            freedom: 0,
            redundant: 0,
            conflicting: 0,
            consistent: true,
        };
        let (truth, mut rules) = match family {
            Family::Chain | Family::Redundant | Family::Conflicting => chain(&mut rng, points, &[]),
            Family::Loose => {
                let mut loose = vec![false; points];
                for slot in loose.iter_mut().skip(1) {
                    *slot = below(&mut rng, 3) == 0;
                }
                if !loose.iter().any(|l| *l) {
                    if let Some(slot) = loose.get_mut(1) {
                        *slot = true;
                    }
                }
                expect.freedom = loose.iter().filter(|l| **l).count();
                chain(&mut rng, points, &loose)
            }
            Family::Ladder => {
                let rows = 2_usize.saturating_add(below(&mut rng, 3));
                let columns = 2_usize.saturating_add(below(&mut rng, 3));
                ladder(&mut rng, rows, columns)
            }
            Family::Growth => draw(
                &mut rng,
                points,
                Kinds {
                    midpoint: true,
                    mirror: true,
                },
            ),
        };
        if matches!(family, Family::Redundant | Family::Conflicting) {
            let lengths: Vec<usize> = rules
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r, Rule::Distance { .. }))
                .map(|(i, _)| i)
                .collect();
            let pick = lengths
                .get(below(&mut rng, lengths.len().max(1)))
                .copied()
                .unwrap_or(0);
            if let Some(Rule::Distance { a, b, value }) = rules.get(pick).cloned() {
                if family == Family::Redundant {
                    expect.redundant = 1;
                    rules.push(Rule::Distance { a, b, value });
                } else {
                    expect.conflicting = 1;
                    expect.consistent = false;
                    let bigger = super::exact::read_float(&value).unwrap_or(1.0) * 1.5;
                    rules.push(Rule::Distance {
                        a,
                        b,
                        value: text(bigger),
                    });
                }
            }
        }
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
        out.push(Case {
            family,
            job: Job {
                id,
                family: format!("{}:{}", family.name(), kinds.join("+")),
                points: starts,
                rules,
                truth: truth.iter().map(|p| [text(p[0]), text(p[1])]).collect(),
            },
            expect,
        });
    }
    out
}

/// What the kernel made of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked {
    /// Fewest degrees of freedom.
    pub freedom_least: usize,
    /// Most degrees of freedom.
    pub freedom_most: usize,
    /// Redundant groups.
    pub redundant: usize,
    /// Conflicting groups.
    pub conflicting: usize,
    /// Whether the block solver solved it.
    pub solved: bool,
}

/// The kernel's diagnosis and solve of `case`.
///
/// # Errors
///
/// A case that cannot be written as a sketch or read back, or one the diagnosis refuses: the
/// generator is broken, not the solver.
pub fn kernel(case: &Case) -> Result<Checked, String> {
    let sketch_text = sketch_text(&case.job)?;
    let sketch = parse(&sketch_text).map_err(|e| format!("case {}: {e:?}", case.job.id))?;
    let span = pairs(&case.job.points, case.job.id)?
        .iter()
        .flatten()
        .fold(1.0_f64, |a, b| if b.abs() > a { b.abs() } else { a });
    let report = diagnose(
        &sketch,
        span * STEPS,
        &mut WorkMeter::new(Budget::UNLIMITED),
    )
    .map_err(|e| format!("case {}: {e:?}", case.job.id))?;
    let solved = solve(&sketch, &mut WorkMeter::new(Budget::UNLIMITED)).is_ok();
    Ok(Checked {
        freedom_least: report.dof_least,
        freedom_most: report.dof_most,
        redundant: report.redundant.len(),
        conflicting: report.conflicting.len(),
        solved,
    })
}

/// Holds the kernel to `case`'s expected diagnosis.
///
/// # Errors
///
/// What [`kernel`] refuses, or the first count that differs, named with the case.
pub fn check(case: &Case) -> Result<Checked, String> {
    let got = kernel(case)?;
    let want = case.expect;
    let id = case.job.id;
    let name = case.family.name();
    if got.freedom_least != want.freedom || got.freedom_most != want.freedom {
        return Err(format!(
            "case {id} ({name}): {}..{} degrees of freedom, expected {}",
            got.freedom_least, got.freedom_most, want.freedom
        ));
    }
    if got.redundant != want.redundant || got.conflicting != want.conflicting {
        return Err(format!(
            "case {id} ({name}): {} redundant and {} conflicting, expected {} and {}",
            got.redundant, got.conflicting, want.redundant, want.conflicting
        ));
    }
    if got.solved != want.consistent {
        return Err(format!(
            "case {id} ({name}): solved is {}, expected {}",
            got.solved, want.consistent
        ));
    }
    Ok(got)
}

/// Whether the kernel's solve of a consistent `case` meets every rule of it.
///
/// # Errors
///
/// A case that cannot be read, or that the kernel does not solve.
pub fn holds(case: &Case) -> Result<bool, String> {
    let solved = super::sketches::kernel(&case.job)?
        .ok_or_else(|| format!("case {}: not solved", case.job.id))?;
    let starts = pairs(&case.job.points, case.job.id)?;
    Ok(case
        .job
        .rules
        .iter()
        .all(|r| missed(r, &solved, &starts) <= HOLDS))
}

/// Judges `oracle`'s report of `case` against the expected counts and the kernel's.
///
/// # Errors
///
/// A case or report that cannot be read: the harness or the generator is broken.
pub fn judge(oracle: &str, case: &Case, report: &Diagnosis) -> Result<Compared, String> {
    if case.job.id != report.id {
        return Err(format!("report {} for case {}", report.id, case.job.id));
    }
    let ours = kernel(case)?;
    let want = case.expect;
    let ours_right = ours.freedom_least == want.freedom
        && ours.freedom_most == want.freedom
        && ours.redundant == want.redundant
        && ours.conflicting == want.conflicting;
    let theirs_right = report.freedom == Some(want.freedom)
        && report.redundant.unwrap_or(0) == want.redundant
        && report.conflicting.unwrap_or(0) == want.conflicting;
    let (kind, settled) = match (ours_right, theirs_right) {
        (true, true) => return Ok(Compared::Agree),
        (true, false) => (
            "oracle",
            "the kernel's counts are the family's, and the oracle's are not",
        ),
        (false, true) => (
            "kernel",
            "the oracle's counts are the family's, and the kernel's are not",
        ),
        (false, false) => ("both", "neither's counts are the family's"),
    };
    let family = if report.freedom.is_none() {
        "none"
    } else if report.freedom != Some(want.freedom) {
        "freedom"
    } else if report.conflicting.unwrap_or(0) != want.conflicting {
        "conflicts"
    } else {
        "redundancy"
    };
    Ok(Compared::Differ {
        signature: Signature {
            operation: "diagnose".to_owned(),
            oracle: oracle.to_owned(),
            kind: kind.to_owned(),
            family: format!("{}:{family}", case.family.name()),
        },
        settled: settled.to_owned(),
    })
}

/// The cases' text, one for each, for replay and for the golden store: every case's sketch, its
/// expectation and what the kernel made of it, in the canonical lines the digest is taken over.
///
/// # Errors
///
/// What [`kernel`] refuses.
pub fn replay_text(cases: &[Case]) -> Result<String, String> {
    let mut out = String::new();
    for case in cases {
        let got = kernel(case)?;
        writeln!(
            out,
            "case {} {} expect {} {} {} {} got {} {} {} {} {}",
            case.job.id,
            case.family.name(),
            case.expect.freedom,
            case.expect.redundant,
            case.expect.conflicting,
            case.expect.consistent,
            got.freedom_least,
            got.freedom_most,
            got.redundant,
            got.conflicting,
            got.solved
        )
        .map_err(|e| e.to_string())?;
        out.push_str(&sketch_text(&case.job)?);
    }
    Ok(out)
}
