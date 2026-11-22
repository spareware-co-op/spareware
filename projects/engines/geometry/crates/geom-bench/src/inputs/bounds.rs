//! Inputs for error bounds per entity (piece G143, decision 32.21): a part's history of 200
//! operations for [`after`] (`Bounds::after`), and the bounds two such histories leave on one
//! start for [`join`] (`Bounds::join`).
//!
//! What it does: a part is made by one operation, six faces and twelve edges each with its own
//! term or none, then cut 199 times. Each cut keeps everything else unchanged, trims one face of
//! the part (now and then spending a step), makes a cut face from it and two edges where the cut
//! meets two faces of the part, each with its own term or none; every tenth cut splits a face of
//! the part by a tool in two, and every tenth merges the two edges the cut before made. The oldest
//! cut face beyond four, and the oldest edges beyond twelve, are deleted, so the part keeps at
//! most about forty entities. [`after`] gives one part's 200 steps per input; [`join`] the bounds
//! of two parts cut from one made start, which share faces at different bounds.
//!
//! Guarantees: every typical batch is full and every call succeeds (`tests/inputs.rs`): every
//! history is built, every bound stays inside the ladder's limits, and the two bounds of a join
//! share an entity at different bounds. Only the typical class exists: the refusals are tested in
//! `geom-op` itself.
//!
//! Method: the cases of `geom-op`'s own tests (`tests/bounds.rs`: a part cut 200 times, every
//! origin summing its own operands, operands joined), made one part's whole history; each is
//! measured as chapter 28.14 sets benchmarks out,
//! `projects/engines/geometry/architecture/28-testing.md`.
//!
//! Cost: 200 histories of about forty records built per input, and for a join both bounds worked
//! out, once per batch, outside every measured run.

use geom_num::{GridStep, Steps};
use geom_op::{
    Bounds, History, Limits, NameId, NameSet, OpError, OpId, OpPath, Origin, PieceKey, Record,
    Role, RoleArg, Side,
};

use super::{Class, batch_of, rng};
use crate::rng::SplitMix64;

/// How many operations a part's history holds: the one that makes it, and the cuts after it.
pub const OPERATIONS: usize = 200;

/// One operation of a part's history: its path, what it did, and its own terms.
#[derive(Clone, Debug)]
pub struct Step {
    /// The operation's path.
    pub op: OpPath,
    /// Its history.
    pub history: History,
    /// Its own terms, in the order drawn.
    pub own: Vec<(NameId, Steps)>,
}

/// A part's whole history, oldest step first.
#[derive(Clone, Debug)]
pub struct Part {
    /// The [`OPERATIONS`] steps.
    pub steps: Vec<Step>,
}

impl Part {
    /// The bounds after every step, from none, on the building step.
    ///
    /// # Errors
    ///
    /// The first step's refusal.
    pub fn bounds(&self) -> Result<Bounds, OpError> {
        self.steps
            .iter()
            .try_fold(Bounds::new(GridStep::Q0), |bounds, step| {
                bounds.after(&step.op, &step.history, &step.own, Limits::DEFAULT)
            })
    }
}

/// The part between operations.
#[derive(Clone, Debug)]
struct Body {
    /// The part's faces, each with how many times a trim spent a step on it.
    faces: Vec<(NameId, u64)>,
    /// The cut faces, oldest first.
    cut_faces: Vec<NameId>,
    /// The edges, oldest first.
    edges: Vec<NameId>,
    /// The two edges the last cut made.
    fresh: [NameId; 2],
}

/// A name drawn from `g`.
fn a_name(g: &mut SplitMix64) -> NameId {
    let (a, b) = (g.next_u64().to_le_bytes(), g.next_u64().to_le_bytes());
    let mut out = [0; 16];
    for (o, v) in out.iter_mut().zip(a.iter().chain(&b)) {
        *o = *v;
    }
    NameId::from_bytes(out)
}

/// A drawn operation path with one step.
fn a_path(g: &mut SplitMix64, step: &str) -> Option<OpPath> {
    let id = a_name(g).to_bytes();
    OpPath::new(OpId::from_bytes(id)).step(step)
}

/// A record of `output` from `origin`.
fn record(output: NameId, origin: Origin) -> Record {
    Record { output, origin }
}

/// `output`, generated in `role` from `from`.
fn generated(output: NameId, from: &[NameId], role: &Role) -> Option<Record> {
    let from = NameSet::new(from.iter().copied()).ok()?;
    Some(record(
        output,
        Origin::Generated {
            from,
            role: role.clone(),
        },
    ))
}

/// An own term of up to `most` steps, zero (no term) included.
fn term(g: &mut SplitMix64, most: u64) -> Steps {
    Steps(g.below(most + 1))
}

/// The operation that makes a part: six faces and twelve edges, from nothing.
fn made(g: &mut SplitMix64) -> Option<(Step, Body)> {
    let op = a_path(g, "make")?;
    let side = Role::new("side", []).ok()?;
    let edge = Role::new("edge", []).ok()?;
    let faces: Vec<NameId> = (0..6).map(|_| a_name(g)).collect();
    let edges: Vec<NameId> = (0..12).map(|_| a_name(g)).collect();
    let mut records = Vec::with_capacity(18);
    let mut own = Vec::with_capacity(18);
    for face in &faces {
        records.push(generated(*face, &[], &side)?);
        own.push((*face, term(g, 4)));
    }
    for e in &edges {
        records.push(generated(*e, &[], &edge)?);
        own.push((*e, term(g, 2)));
    }
    let history = History::new(records, NameSet::default(), []).ok()?;
    let fresh = [*edges.get(10)?, *edges.get(11)?];
    let body = Body {
        faces: faces.into_iter().map(|f| (f, 0)).collect(),
        cut_faces: Vec::new(),
        edges,
        fresh,
    };
    Some((Step { op, history, own }, body))
}

/// The `k`-th cut of `body` (see the module documentation), and the part after it.
fn cut(g: &mut SplitMix64, body: Body, k: u64) -> Option<(Step, Body)> {
    let op = a_path(g, "cut")?;
    let Body {
        mut faces,
        mut cut_faces,
        mut edges,
        fresh,
    } = body;
    let mut records = Vec::new();
    let mut own = Vec::new();
    let mut deleted = Vec::new();
    // The face trimmed, which spends a step now and then, at most four times.
    let at = g.index(faces.len());
    let (trimmed, spent) = *faces.get(at)?;
    if spent < 4 && g.below(8) == 0 {
        own.push((trimmed, Steps(1)));
        *faces.get_mut(at)? = (trimmed, spent + 1);
    }
    records.push(record(
        trimmed,
        Origin::Modified {
            from: NameSet::new([trimmed]).ok()?,
        },
    ));
    // The cut face, from the face trimmed, and the edges where it meets that face and another.
    let cut_face = a_name(g);
    records.push(generated(
        cut_face,
        &[trimmed],
        &Role::new("cut", [RoleArg::Count(k)]).ok()?,
    )?);
    own.push((cut_face, Steps(1 + g.below(4))));
    let other = faces.get(g.index(faces.len()))?.0;
    let edge = Role::new("edge", [RoleArg::Count(k)]).ok()?;
    let new_edges = [a_name(g), a_name(g)];
    for (e, from) in new_edges.iter().zip([trimmed, other]) {
        records.push(generated(*e, &[from], &edge)?);
        own.push((*e, term(g, 2)));
    }
    // Every tenth cut, from the fifth, splits another face of the part by a tool in two.
    let mut split = None;
    if k % 10 == 5 {
        let at = g.index(faces.len());
        let (of, spent) = *faces.get(at)?;
        if of != trimmed {
            let tools = NameSet::new([a_name(g)]).ok()?;
            let pieces = [a_name(g), a_name(g)];
            for (p, side) in pieces.iter().zip([Side::Back, Side::Front]) {
                let piece = PieceKey::ByTools {
                    tools: tools.clone(),
                    side,
                };
                records.push(record(*p, Origin::Split { of, piece }));
            }
            faces.remove(at);
            faces.extend(pieces.map(|p| (p, spent)));
            split = Some(of);
        }
    }
    // Every tenth cut merges the two edges the cut before made.
    let mut merged = None;
    if k.is_multiple_of(10) {
        let into = a_name(g);
        records.push(record(
            into,
            Origin::Merged {
                of: NameSet::new(fresh).ok()?,
            },
        ));
        edges.retain(|e| !fresh.contains(e));
        edges.push(into);
        merged = Some(fresh);
    }
    // The oldest cut face beyond four, and the oldest edges beyond twelve, go.
    if cut_faces.len() >= 4 {
        deleted.push(cut_faces.remove(0));
    }
    while edges.len() > 10 {
        deleted.push(edges.remove(0));
    }
    // Everything else is unchanged.
    let touched = |name: &NameId| {
        *name == trimmed
            || split == Some(*name)
            || merged.is_some_and(|m| m.contains(name))
            || deleted.contains(name)
    };
    let kept: Vec<NameId> = faces
        .iter()
        .map(|f| f.0)
        .chain(cut_faces.iter().copied())
        .chain(edges.iter().copied())
        .filter(|name| !touched(name) && !records.iter().any(|r| r.output == *name))
        .collect();
    records.extend(kept.into_iter().map(|name| record(name, Origin::Unchanged)));
    cut_faces.push(cut_face);
    edges.extend(new_edges);
    let history = History::new(records, NameSet::new(deleted).ok()?, []).ok()?;
    let body = Body {
        faces,
        cut_faces,
        edges,
        fresh: new_edges,
    };
    Some((Step { op, history, own }, body))
}

/// `body` cut `n` times, the first cut numbered 1.
fn cuts(g: &mut SplitMix64, mut body: Body, n: usize) -> Option<Vec<Step>> {
    let mut steps = Vec::with_capacity(n);
    for k in (1..).take(n) {
        let (step, after) = cut(g, body, k)?;
        steps.push(step);
        body = after;
    }
    Some(steps)
}

/// A part made and cut, [`OPERATIONS`] steps in all, from `g`.
fn a_part(g: &mut SplitMix64) -> Option<Part> {
    let (first, body) = made(g)?;
    let mut steps = vec![first];
    steps.extend(cuts(g, body, OPERATIONS - 1)?);
    let part = Part { steps };
    part.bounds().is_ok().then_some(part)
}

/// One part's history per call of `Bounds::after`, each step in turn on the bounds before it.
#[must_use]
pub fn after(class: Class) -> Vec<Part> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("history::bounds_after", class);
    batch_of(|_| a_part(&mut g))
}

/// The bounds two parts leave, each made by one start and cut [`OPERATIONS`] less one times its own
/// way, per call of `Bounds::join`: they share the faces of the start that neither split, at the
/// bounds each one's trims gave.
#[must_use]
pub fn join(class: Class) -> Vec<(Bounds, Bounds)> {
    if class != Class::Typical {
        return Vec::new();
    }
    let mut g = rng("history::bounds_join", class);
    batch_of(|_| {
        let (first, body) = made(&mut g)?;
        let one = |g: &mut SplitMix64| -> Option<Bounds> {
            let mut steps = vec![first.clone()];
            steps.extend(cuts(g, body.clone(), OPERATIONS - 1)?);
            Part { steps }.bounds().ok()
        };
        let a = one(&mut g)?;
        let b = one(&mut g)?;
        let differ = a.entities().any(|e| {
            let (x, y) = (a.of(e), b.of(e));
            y.0 > 0 && x != y
        });
        differ.then_some((a, b))
    })
}
