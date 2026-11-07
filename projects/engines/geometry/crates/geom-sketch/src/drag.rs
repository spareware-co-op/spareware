//! Dragging: a solve with a moving target, held targets, limits, and a release that replays the
//! logged path (piece G228; chapter 15.9).
//!
//! What it does: [`Dragger::begin`] takes a solved sketch and the points to drag, and holds each
//! dragged coordinate with a driving fix that only the drag owns. [`Dragger::frame`] then moves
//! those fixes to the cursor and follows the path from the last frame to it, the way
//! `solve::continue_to` follows a path (section 15.7), and returns the sketch with a [`Status`].
//! [`Dragger::release`] is the one logged operation: it replays the accepted targets from the
//! stored state and commits the result (section 15.10).
//!
//! Guarantees:
//!
//! - **Continuous.** A frame starts from the last frame's solution, not from the stored sketch.
//! - **Held, never weighted.** The dragged coordinates are fixed at the cursor, and the rest is
//!   projected back onto the constraints by the least-change step of section 15.6. No constraint is
//!   softened.
//! - **`AtLimit`.** If the constraints cannot hold with the dragged coordinates held (the step and
//!   its half both fail to converge), the drag is released for that frame and the whole sketch is
//!   projected from the cursor: the dragged geometry stops at the nearest position the constraints
//!   allow, and the frame names the constraints the cursor broke ([`Status::AtLimit`]).
//! - **`DragLimit`.** After a frame the stored root choices (the senses of section 15.7) are checked
//!   with interval arithmetic. A step that would flip one is halved; if it still flips, the frame is
//!   the last good one and the status names the rule ([`Status::DragLimit`]). Both are preview
//!   statuses, not errors.
//! - **A frame that runs out of budget shows the previous frame** ([`Status::Behind`]) and changes
//!   nothing. Frames are shown, never stored.
//! - **Replay is bit for bit.** Every frame that changed the state (`Held` or `AtLimit`) logs its
//!   target; the others leave the state as it was and are left out. The log keeps at most 256
//!   targets (above that, every k-th with k the smallest whole number that fits, and the last).
//!   Replaying a log of up to 256 targets from the stored sketch gives exactly the last frame's
//!   values, and replaying it twice gives the same bits, because nothing in a frame depends on
//!   anything but the stored data and the cursor path.
//! - **Release reports its drift.** If a committed point is more than the fit ceiling
//!   ([`crate::check::Checker::fit`], 64 steps) from the last preview, [`Released::moved`] names it
//!   and says by how much.
//! - **No recursion, no unsafe code.**
//!
//! Only points are dragged: a line or a circle is dragged by its points. A dragged point whose
//! coordinates other constraints already fix is a conflict at once, and its frames are `AtLimit`.
//!
//! Method: section 15.9 and section 15.7 of the kernel architecture
//! (`projects/engines/geometry/architecture/15-sketches.md`); the solver is `solve`'s.
//!
//! Cost: a frame is one pass over the stages from the last frame's values, with the held plan's
//! structure analysed once in [`Dragger::begin`]; two when a step is halved. A frame costs about
//! what a residual evaluation of the whole sketch costs, plus the stages whose inputs moved. No
//! recursion.

extern crate alloc;

use alloc::vec::Vec;

use geom_run::WorkMeter;

use crate::check::{Checker, coordinates};
use crate::commit::{Committed, commit};
use crate::equation::Sense;
use crate::model::{Constraint, ConstraintKind, ItemId, Sketch, SketchError, Standing};
use crate::solve::{Plan, Report, TOLERANCE, empty_report};

/// The most targets a release keeps.
pub const MOST_WAYPOINTS: usize = 256;

/// How a frame ended. `AtLimit`, `DragLimit` and `Behind` are preview statuses, not errors.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// The dragged points are at the cursor.
    Held,
    /// The constraints cannot hold with the points at the cursor: the sketch is projected from it,
    /// and these constraints are the ones it broke.
    AtLimit {
        /// The constraints, in id order.
        blocking: Vec<ItemId>,
    },
    /// A stored root choice (or a step that cannot be told) stops the drag: the frame is the last
    /// good one.
    DragLimit {
        /// The rule that blocks it.
        blocking: Vec<ItemId>,
    },
    /// The frame ran out of its budget: the frame is the previous one.
    Behind,
}

/// One frame of a drag.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// How it ended.
    pub status: Status,
    /// The sketch at the frame.
    pub sketch: Sketch,
    /// What the solve did.
    pub report: Report,
}

/// The data of one release: the stored sketch, the points and the accepted targets.
#[derive(Clone, Debug, PartialEq)]
pub struct DragLog {
    /// The stored state the drag started from.
    pub stored: Sketch,
    /// The dragged points.
    pub dragged: Vec<ItemId>,
    /// The accepted targets, one position for each dragged point, at most [`MOST_WAYPOINTS`].
    pub path: Vec<Vec<[f64; 2]>>,
    /// How many accepted frames the drag had.
    pub frames: usize,
    /// Every how many of them the path keeps (1 when none were dropped).
    pub every: usize,
}

/// What a release gave.
#[derive(Clone, Debug)]
pub struct Released {
    /// The replayed sketch, at final precision.
    pub sketch: Sketch,
    /// Its commit (section 15.10).
    pub committed: Committed,
    /// The points that moved more than the fit ceiling between the last preview and the commit, and
    /// by how much (the larger coordinate difference, in steps).
    pub moved: Vec<(ItemId, f64)>,
    /// The largest such difference over every point, in steps.
    pub largest: f64,
    /// The log that was replayed.
    pub log: DragLog,
}

/// A drag in progress.
pub struct Dragger {
    stored: Sketch,
    dragged: Vec<ItemId>,
    /// The plan with a fix on each dragged coordinate.
    held: Plan,
    /// The plan of the sketch as stored.
    free: Plan,
    /// Each dragged coordinate's column, x then y for each point.
    columns: Vec<usize>,
    /// Each dragged coordinate's fix, where its constant is among the held plan's inputs.
    places: Vec<usize>,
    senses: Vec<(usize, Sense)>,
    x: Vec<f64>,
    accepted: Vec<Vec<[f64; 2]>>,
    frames: usize,
}

fn column(sketch: &Sketch, merged: &[usize], id: ItemId, slot: u8) -> Result<usize, SketchError> {
    let variables = sketch.variables()?;
    let at = variables
        .binary_search_by_key(&(id, slot), |v| (v.owner, v.slot))
        .map_err(|_| SketchError::WrongKind(id))?;
    merged.get(at).copied().ok_or(SketchError::Internal)
}

fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), SketchError> {
    list.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

impl Dragger {
    /// Starts a drag of `dragged`, which must be points of `sketch`.
    ///
    /// # Errors
    ///
    /// [`SketchError::WrongKind`] for an id that is not a point; what a solve of `sketch` refuses;
    /// [`SketchError::Stopped`].
    pub fn begin(
        sketch: &Sketch,
        dragged: &[ItemId],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Dragger, SketchError> {
        let mut free = Plan::new(sketch, meter)?;
        let mut x = free.assembly.start()?;
        let mut report = empty_report();
        free.run(&mut x, &mut report, meter)?;
        // The fixes sort after every id the sketch has, so they add rules at the end and change no
        // other rule's place.
        let mut last = 0_u64;
        for id in sketch
            .entities()
            .iter()
            .map(|e| e.id)
            .chain(sketch.constraints().iter().map(|c| c.id))
        {
            last = last.max(id.sequence);
        }
        let sequence = last.checked_add(1).ok_or(SketchError::Internal)?;
        let mut with_fixes = sketch.clone();
        let mut fixes = Vec::new();
        let mut columns = Vec::new();
        let mut key = 0_u64;
        for &id in dragged {
            for slot in 0..2_u8 {
                let at = column(sketch, &free.merged, id, slot)?;
                let value = *x.get(at).ok_or(SketchError::Internal)?;
                let fix = ItemId::new(sequence, key);
                key = key.checked_add(1).ok_or(SketchError::Internal)?;
                with_fixes.add_constraint(Constraint {
                    id: fix,
                    kind: ConstraintKind::Fix {
                        entity: id,
                        slot,
                        value,
                    },
                    standing: Standing::Driving,
                    priority: 0,
                })?;
                push(&mut fixes, fix)?;
                push(&mut columns, at)?;
            }
        }
        let mut held = Plan::new(&with_fixes, meter)?;
        if held.merged != free.merged {
            return Err(SketchError::Internal);
        }
        let mut places = Vec::new();
        for &fix in &fixes {
            push(&mut places, held.assembly.input_of(fix)?)?;
        }
        held.assembly.rescale(&x)?;
        let senses = free.assembly.senses(&x)?;
        Ok(Dragger {
            stored: sketch.clone(),
            dragged: dragged.to_vec(),
            held,
            free,
            columns,
            places,
            senses,
            x,
            accepted: Vec::new(),
            frames: 0,
        })
    }

    /// The dragged coordinates as they are now.
    fn at(&self) -> Result<Vec<f64>, SketchError> {
        let mut out = Vec::new();
        for &c in &self.columns {
            push(&mut out, *self.x.get(c).ok_or(SketchError::Internal)?)?;
        }
        Ok(out)
    }

    /// Puts the fixes at `from` + `along` · (`to` − `from`) and rescales at `x`.
    fn place(
        &mut self,
        from: &[f64],
        to: &[f64],
        along: f64,
        x: &[f64],
    ) -> Result<(), SketchError> {
        for ((&at, &a), &b) in self.places.iter().zip(from).zip(to) {
            let value = if along == 1.0 { b } else { a + along * (b - a) };
            self.held.assembly.set_constant(at, value)?;
        }
        self.held.assembly.rescale(x)
    }

    /// One try at `along` of the way: `Ok(Ok(values))` if it converged with every sense kept.
    fn attempt(
        &mut self,
        from: &[f64],
        to: &[f64],
        along: f64,
        start: &[f64],
        report: &mut Report,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Result<Vec<f64>, Blocked>, SketchError> {
        self.place(from, to, along, start)?;
        let mut y = start.to_vec();
        match self.held.run(&mut y, report, meter) {
            Ok(()) => {}
            Err(SketchError::Unsolved(id) | SketchError::Degenerate(id)) => {
                return Ok(Err(Blocked::Conflict(id)));
            }
            Err(e) => return Err(e),
        }
        Ok(match self.held.at_risk(&y, &self.senses, &[], meter)? {
            Some(id) => Err(Blocked::Flipped(id)),
            None => Ok(y),
        })
    }

    /// The sketch at `x`.
    fn sketch_at(&mut self, x: &[f64], report: Report) -> Result<(Sketch, Report), SketchError> {
        self.free.assembly.rescale(x)?;
        let solved = self.free.finish(&self.stored, x, report)?;
        Ok((solved.sketch, solved.report))
    }

    /// The sketch as it is, as a frame with `status`.
    fn current(&mut self, status: Status, report: Report) -> Result<Frame, SketchError> {
        let x = self.x.clone();
        let (sketch, report) = self.sketch_at(&x, report)?;
        Ok(Frame {
            status,
            sketch,
            report,
        })
    }

    /// One frame: the dragged points go to `cursor`, one position for each, in the order given to
    /// [`Dragger::begin`].
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for a cursor of another length; what the arithmetic and the
    /// allocations refuse. Running out of budget is not an error here: it is [`Status::Behind`].
    pub fn frame(
        &mut self,
        cursor: &[[f64; 2]],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Frame, SketchError> {
        if cursor.len() != self.dragged.len() {
            return Err(SketchError::Internal);
        }
        let mut to = Vec::new();
        for p in cursor {
            push(&mut to, p[0])?;
            push(&mut to, p[1])?;
        }
        let from = self.at()?;
        let start = self.x.clone();
        let mut report = empty_report();
        report.steps = 0;
        let outcome = self.reach(&from, &to, &start, &mut report, meter);
        let reached = match outcome {
            Err(SketchError::Stopped) => {
                self.place(&from, &from, 1.0, &start)?;
                return self.current(Status::Behind, report);
            }
            other => other?,
        };
        match reached {
            Ok(y) => {
                self.x = y;
                self.log(cursor)?;
                self.current(Status::Held, report)
            }
            Err(Blocked::Flipped(id)) => {
                self.place(&from, &from, 1.0, &start)?;
                self.current(
                    Status::DragLimit {
                        blocking: alloc::vec![id],
                    },
                    report,
                )
            }
            Err(Blocked::Conflict(id)) => self.release_to(cursor, id, &from, &start, report, meter),
        }
    }

    /// The step from `from` to `to`, then two halves of it if that fails.
    fn reach(
        &mut self,
        from: &[f64],
        to: &[f64],
        start: &[f64],
        report: &mut Report,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Result<Vec<f64>, Blocked>, SketchError> {
        let full = self.attempt(from, to, 1.0, start, report, meter)?;
        let first = match full {
            Ok(y) => return Ok(Ok(y)),
            Err(blocked) => blocked,
        };
        // Halved once: a step that would flip a choice or fail to converge is tried in two halves.
        self.place(from, from, 1.0, start)?;
        let half = self.attempt(from, to, 0.5, start, report, meter)?;
        let Ok(middle) = half else {
            self.place(from, from, 1.0, start)?;
            return Ok(Err(first));
        };
        let rest = self.attempt(from, to, 1.0, &middle, report, meter)?;
        if rest.is_err() {
            self.place(from, from, 1.0, start)?;
        }
        Ok(rest)
    }

    /// The constraints cannot hold with the points at the cursor: project the whole sketch from it.
    fn release_to(
        &mut self,
        cursor: &[[f64; 2]],
        first: ItemId,
        from: &[f64],
        start: &[f64],
        mut report: Report,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Frame, SketchError> {
        self.place(from, from, 1.0, start)?;
        let mut z = start.to_vec();
        for (i, p) in cursor.iter().enumerate() {
            for (k, v) in p.iter().enumerate() {
                let at = i
                    .checked_mul(2)
                    .and_then(|q| q.checked_add(k))
                    .and_then(|q| self.columns.get(q))
                    .ok_or(SketchError::Internal)?;
                *z.get_mut(*at).ok_or(SketchError::Internal)? = *v;
            }
        }
        self.free.assembly.rescale(&z)?;
        let mut blocking = Vec::new();
        match self.free.assembly.evaluate_at(&z) {
            Ok(()) => {
                let mut seen: Option<ItemId> = None;
                for (row, r) in self.free.assembly.residuals().iter().enumerate() {
                    if r.abs() > TOLERANCE {
                        let id = self.free.assembly.name_of_row(row)?;
                        if seen != Some(id) {
                            push(&mut blocking, id)?;
                            seen = Some(id);
                        }
                    }
                }
            }
            Err(SketchError::Degenerate(id)) => push(&mut blocking, id)?,
            Err(e) => return Err(e),
        }
        if blocking.is_empty() {
            push(&mut blocking, first)?;
        }
        let ran = match self.free.run(&mut z, &mut report, meter) {
            Ok(()) => self
                .free
                .at_risk(&z, &self.senses, &[], meter)?
                .map_or(Ok(()), Err),
            Err(SketchError::Unsolved(id) | SketchError::Degenerate(id)) => Err(id),
            Err(e) => return Err(e),
        };
        match ran {
            Ok(()) => {}
            Err(id) => {
                return self.current(
                    Status::DragLimit {
                        blocking: alloc::vec![id],
                    },
                    report,
                );
            }
        }
        // The fixes follow the points to where the constraints let them go.
        let mut here = Vec::new();
        for &c in &self.columns {
            push(&mut here, *z.get(c).ok_or(SketchError::Internal)?)?;
        }
        self.place(&here, &here, 1.0, &z)?;
        self.x = z;
        self.log(cursor)?;
        self.current(Status::AtLimit { blocking }, report)
    }

    fn log(&mut self, cursor: &[[f64; 2]]) -> Result<(), SketchError> {
        self.frames = self.frames.checked_add(1).ok_or(SketchError::Internal)?;
        push(&mut self.accepted, cursor.to_vec())
    }

    /// The release's data: the targets of the frames that changed the sketch, thinned to at most
    /// [`MOST_WAYPOINTS`].
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`].
    pub fn log_of(&self) -> Result<DragLog, SketchError> {
        let m = self.accepted.len();
        // The smallest k with ceil(m / k) targets, and the last if it is not one of them, within the
        // most.
        let mut every = 1_usize;
        while kept(m, every) > MOST_WAYPOINTS {
            every = every.checked_add(1).ok_or(SketchError::Internal)?;
        }
        let mut path = Vec::new();
        for (i, p) in self.accepted.iter().enumerate() {
            let last = i.checked_add(1) == Some(m);
            if i.is_multiple_of(every) || last {
                push(&mut path, p.clone())?;
            }
        }
        Ok(DragLog {
            stored: self.stored.clone(),
            dragged: self.dragged.clone(),
            path,
            frames: m,
            every,
        })
    }

    /// Releases the drag: replays the log from the stored sketch and commits the result.
    ///
    /// # Errors
    ///
    /// What [`replay`] and [`commit`] refuse.
    pub fn release(mut self, meter: &mut WorkMeter<'_>) -> Result<Released, SketchError> {
        let log = self.log_of()?;
        let preview = self.current(Status::Held, empty_report())?.sketch;
        let sketch = replay(&log, meter)?;
        let committed = commit(&sketch, meter)?;
        let mut moved = Vec::new();
        let mut largest = 0.0_f64;
        let fit = Checker::fit();
        for (id, stored) in &committed.points {
            let Ok(shown) = preview.point(*id) else {
                continue;
            };
            let [cx, cy] = coordinates(stored)?;
            let dx = (f64::midpoint(cx.lo(), cx.hi()) - shown[0]).abs();
            let dy = (f64::midpoint(cy.lo(), cy.hi()) - shown[1]).abs();
            let d = if dx > dy { dx } else { dy };
            if d > largest {
                largest = d;
            }
            if d > fit {
                push(&mut moved, (*id, d))?;
            }
        }
        Ok(Released {
            sketch,
            committed,
            moved,
            largest,
            log,
        })
    }
}

/// How many targets a path of `m` keeps at every `every`-th, with the last.
fn kept(m: usize, every: usize) -> usize {
    if m == 0 {
        return 0;
    }
    let first = m.div_ceil(every);
    // The last is already kept when m − 1 is a multiple of `every`.
    if m.saturating_sub(1).is_multiple_of(every) {
        first
    } else {
        first.saturating_add(1)
    }
}

/// Why a try did not come out.
#[derive(Clone, Copy, Debug)]
enum Blocked {
    /// A stored sense flipped or could not be told.
    Flipped(ItemId),
    /// A block did not converge.
    Conflict(ItemId),
}

/// Replays `log` from its stored sketch: the same frames, in the same order, so the result is the
/// last live frame's values bit for bit when no target was dropped.
///
/// # Errors
///
/// What [`Dragger::begin`] and [`Dragger::frame`] refuse.
pub fn replay(log: &DragLog, meter: &mut WorkMeter<'_>) -> Result<Sketch, SketchError> {
    let mut dragger = Dragger::begin(&log.stored, &log.dragged, meter)?;
    for target in &log.path {
        dragger.frame(target, meter)?;
    }
    let x = dragger.x.clone();
    Ok(dragger.sketch_at(&x, empty_report())?.0)
}
