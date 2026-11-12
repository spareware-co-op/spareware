//! Fuzz target: `geom-topo`'s Euler operators (piece G81). On any sequence of operators, regions
//! and publishes, each operator changes the counts of vertices, edges, faces, loops and shells by
//! its row of the table, Euler's formula holds after every step, an operator on a dead row
//! refuses, a refusal changes nothing, and publishing the editor and editing the body again gives
//! the body back.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: up to [`MAX_STEPS`] steps of
//! four bytes each, an operator and the bytes that pick its rows. A row is picked from all the
//! rows of its arena, live and dead, so refusals come up as often as edits; the second fin of
//! `mef` and `mekr` is, by turns, the first fin itself, any fin, or a fin of the same face, in the
//! same loop or another. [`check`] starts an editor with the outside region, runs each step, and
//! holds the editor to the laws after it; a publish step, and the end of the case, publish the
//! editor, edit the body again and publish that. [`harness`] is `decode` then `check`.
//!
//! Guarantees:
//!
//! - **Any bytes are a case**, the empty string included; [`decode`] reads at most
//!   [`MAX_INPUT_BYTES`] bytes, and a unit test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case. The laws,
//!   the `euler` module header's: an operator that applies changes the live counts (V, E, F, loops,
//!   S) by its row of the table and nothing else; V − E + F − R = 2 (S − H) after every step, with
//!   R the loops less the faces and H the holes, which only `kfmrh` (one more) and `mfkrh` (one
//!   fewer) change; an operator given a dead row refuses it as dead; a refusal leaves the editor
//!   equal to what it was; and a published editor edits back to an editor with the same counts,
//!   which publishes to the same body.
//!
//! Method: Mäntylä and Sulonen's GWB operators as the module header of `geom_topo::builder::euler`
//! states them, with the counts of its table as the oracle and `tests/euler_model.rs`'s choice of
//! rows, driven by coverage-guided fuzzing (28.5).
//!
//! Cost: at most [`MAX_STEPS`] operators, each followed by a count of the live rows and a copy of
//! the editor; a publish, an edit and a second publish per publish step and at the end; at most
//! [`MAX_INPUT_BYTES`] bytes read.

use geom_curve::{CurveKind, Line};
use geom_math::Box3;
use geom_num::grid::P3;
use geom_run::{Budget, WorkMeter};
use geom_surface::{Plane, SurfaceKind};
use geom_test::{ByteSource, Entropy};
use geom_topo::order::NameKey;
use geom_topo::{
    Arena, BodyKind, Euler, FinIx, Ix, NewEdge, NewFace, NewVertex, Refusal, RegionKind, Sense,
    TopoError,
};

use crate::Finding;
use crate::cbor::hex;

/// The most steps a case runs.
pub const MAX_STEPS: usize = 32;

/// Bytes per step: the operator and three row picks.
const STEP_BYTES: usize = 4;

/// The most bytes [`decode`] reads: 32 · 4 = 128.
pub const MAX_INPUT_BYTES: usize = MAX_STEPS * STEP_BYTES;

const _: () = assert!(MAX_INPUT_BYTES == 128);

/// One case: its steps, each an operator byte and three bytes that pick rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The steps, in order.
    pub steps: Vec<[u8; 4]>,
}

/// The fuzzer's bytes as a case, and how many bytes were read (at most [`MAX_INPUT_BYTES`]).
#[must_use]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let count = (data.len() / STEP_BYTES).min(MAX_STEPS);
    let mut src = ByteSource::new(data);
    let steps = (0..count)
        .map(|_| [src.byte(), src.byte(), src.byte(), src.byte()])
        .collect();
    (Case { steps }, data.len().saturating_sub(src.remaining()))
}

/// The fuzz harness: the laws on `data` decoded as a case.
///
/// # Errors
///
/// A [`Finding`] naming the law `data` broke.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0).map_err(|f| Finding {
        law: f.law,
        detail: format!("{}: {}", hex(data), f.detail),
    })
}

/// The live counts the formula reads: vertices, edges, faces, loops and shells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Counts([i64; 5]);

/// The live rows of `arena`.
fn live(e: &Euler, arena: Arena) -> Vec<u32> {
    (0..e.arenas().rows(arena))
        .filter(|&r| e.is_live(arena, r))
        .collect()
}

fn counts(e: &Euler) -> Counts {
    let n = |arena| i64::try_from(live(e, arena).len()).unwrap_or(i64::MAX);
    Counts([
        n(Arena::Vertex),
        n(Arena::Edge),
        n(Arena::Face),
        n(Arena::Loop),
        n(Arena::Shell),
    ])
}

/// The steps' editor, the names it has given and its holes.
struct Run {
    euler: Euler,
    names: u64,
    holes: i64,
}

/// The index of `row`, which every row number below 2^32 − 1 has.
fn ix<K>(row: u32) -> Option<Ix<K>> {
    Ix::new(row)
}

impl Run {
    fn name(&mut self) -> NameKey {
        self.names = self.names.saturating_add(1);
        let mut key = [0u8; 16];
        for (k, b) in key.iter_mut().zip(self.names.to_be_bytes()) {
            *k = b;
        }
        NameKey(key)
    }

    fn face(&mut self) -> Option<NewFace> {
        let plane = Plane::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], ((0.0, 1.0), (0.0, 1.0)));
        let corner = P3::new(0, 0, 0).ok()?;
        Some(NewFace {
            name: self.name(),
            surface: SurfaceKind::Plane(plane.ok()?),
            sense: Sense::Forward,
            bound: Box3::new(corner, P3::new(1, 1, 1).ok()?).ok()?,
        })
    }

    fn edge(&mut self) -> Option<NewEdge> {
        Some(NewEdge {
            name: self.name(),
            curve: CurveKind::Line(Line::new([0.0; 3], [1.0, 0.0, 0.0], (0.0, 1.0)).ok()?),
        })
    }

    fn vertex(&mut self) -> Option<NewVertex> {
        Some(NewVertex {
            name: self.name(),
            point: P3::new(0, 0, 0).ok()?,
        })
    }

    /// A row of `arena` picked by `byte` from all its rows, live and dead.
    fn pick(&self, arena: Arena, byte: u8) -> Option<u32> {
        u32::from(byte).checked_rem(self.euler.arenas().rows(arena))
    }

    /// A fin of `f`'s face, in `f`'s loop or in another, picked by `byte`.
    fn partner(&self, f: FinIx, same_loop: bool, byte: u8) -> Option<FinIx> {
        let a = self.euler.arenas();
        let me = a.fins.get(f)?;
        let face = a.loops.get(me.loop_)?.face;
        let fins: Vec<u32> = live(&self.euler, Arena::Fin)
            .into_iter()
            .filter(|&g| {
                ix(g)
                    .and_then(|g| a.fins.get(g))
                    .and_then(|x| Some((x.loop_, a.loops.get(x.loop_)?.face)))
                    .is_some_and(|(l, of)| of == face && (l == me.loop_) == same_loop)
            })
            .collect();
        let n = u32::try_from(fins.len()).ok()?;
        ix(*fins.get(usize::try_from(u32::from(byte).checked_rem(n)?).ok()?)?)
    }

    fn dead(&self, arena: Arena, row: u32) -> bool {
        !self.euler.is_live(arena, row)
    }

    /// Runs one step: the rows it was given (with whether each is dead), the counts its table row
    /// adds, and what the editor answered; `None` for a step with nothing to act on.
    fn step(&mut self, [op, x, y, z]: [u8; 4]) -> Option<(bool, [i64; 5], Result<(), TopoError>)> {
        let fin = |run: &Run, b| run.pick(Arena::Fin, b);
        Some(match op % 14 {
            0 => {
                let r = self.pick(Arena::Region, x)?;
                let (face, edge, v0, v1) = (self.face()?, self.edge()?, self.vertex()?, self.vertex()?);
                let dead = self.dead(Arena::Region, r);
                (dead, [2, 1, 1, 1, 1], self.euler.make_wire(ix(r)?, face, edge, v0, v1).map(|_| ()))
            }
            1 => {
                let f = fin(self, x)?;
                let (edge, vertex) = (self.edge()?, self.vertex()?);
                (self.dead(Arena::Fin, f), [1, 1, 0, 0, 0], self.euler.mev(ix(f)?, edge, vertex).map(|_| ()))
            }
            2 => {
                let e = self.pick(Arena::Edge, x)?;
                (self.dead(Arena::Edge, e), [-1, -1, 0, 0, 0], self.euler.kev(ix(e)?))
            }
            3 => {
                let f = fin(self, x)?;
                let to = match y % 4 {
                    0 => ix(f),
                    1 => ix(fin(self, z)?),
                    _ => self.partner(ix(f)?, true, z),
                }?;
                let dead = self.dead(Arena::Fin, f) || self.dead(Arena::Fin, to.row());
                let (edge, face) = (self.edge()?, self.face()?);
                (dead, [0, 1, 1, 1, 0], self.euler.mef(ix(f)?, to, edge, face).map(|_| ()))
            }
            4 => {
                let f = fin(self, x)?;
                (self.dead(Arena::Fin, f), [0, -1, -1, -1, 0], self.euler.kef(ix(f)?))
            }
            5 => {
                let f = fin(self, x)?;
                let to = if y % 3 == 0 {
                    ix(fin(self, z)?)
                } else {
                    self.partner(ix(f)?, false, z)
                }?;
                let dead = self.dead(Arena::Fin, f) || self.dead(Arena::Fin, to.row());
                let edge = self.edge()?;
                (dead, [0, 1, 0, -1, 0], self.euler.mekr(ix(f)?, to, edge).map(|_| ()))
            }
            6 => {
                let f = fin(self, x)?;
                (self.dead(Arena::Fin, f), [0, -1, 0, 1, 0], self.euler.kemr(ix(f)?).map(|_| ()))
            }
            7 => {
                let (k, l) = (self.pick(Arena::Face, x)?, self.pick(Arena::Loop, y)?);
                let dead = self.dead(Arena::Face, k) || self.dead(Arena::Loop, l);
                let r = self.euler.kfmrh(ix(k)?, ix(l)?);
                if r.is_ok() {
                    self.holes = self.holes.saturating_add(1);
                }
                (dead, [0, 0, -1, 0, 0], r)
            }
            8 => {
                let l = self.pick(Arena::Loop, x)?;
                let face = self.face()?;
                let r = self.euler.mfkrh(ix(l)?, face).map(|_| ());
                if r.is_ok() {
                    self.holes = self.holes.saturating_sub(1);
                }
                (self.dead(Arena::Loop, l), [0, 0, 1, 0, 0], r)
            }
            9 => {
                let e = self.pick(Arena::Edge, x)?;
                (self.dead(Arena::Edge, e), [-2, -1, -1, -1, -1], self.euler.kill_wire(ix(e)?))
            }
            10 => {
                let kind = if x % 2 == 0 { RegionKind::Solid } else { RegionKind::Void };
                let name = self.name();
                (false, [0; 5], self.euler.make_region(kind, name).map(|_| ()))
            }
            11 => {
                // Row 0 is the outside, which every publish needs.
                let r = self.pick(Arena::Region, x).filter(|&r| r != 0)?;
                (self.dead(Arena::Region, r), [0; 5], self.euler.kill_region(ix(r)?))
            }
            12 => {
                let (s, r) = (self.pick(Arena::Shell, x)?, self.pick(Arena::Region, y)?);
                let dead = self.dead(Arena::Shell, s) || self.dead(Arena::Region, r);
                (dead, [0; 5], self.euler.move_shell(ix(s)?, ix(r)?))
            }
            _ => return None,
        })
    }
}

/// Holds the case to every law of the module header.
///
/// # Errors
///
/// A [`Finding`] naming the law the case broke.
pub fn check(case: &Case) -> Result<(), Finding> {
    run_case(case).map(|_| ())
}

/// How many times each operator (by its number, 0 to 12) applied in `case`; all zero for a case
/// that breaks a law.
#[must_use]
pub fn applied(case: &Case) -> [u32; 13] {
    run_case(case).unwrap_or_default()
}

/// The laws on `case`, and how many times each operator applied.
fn run_case(case: &Case) -> Result<[u32; 13], Finding> {
    let law = |law: &str, what: String| Finding::new(law, || format!("{case:?}: {what}"));
    let mut tally = [0u32; 13];
    let mut run = Run {
        euler: Euler::new(),
        names: 0,
        holes: 0,
    };
    let outside = run.name();
    if run.euler.make_region(RegionKind::Void, outside).is_err() {
        return Err(law("an empty editor makes a region", String::new()));
    }
    for (at, &step) in case.steps.iter().enumerate() {
        let before = run.euler.clone();
        let was = counts(&before);
        let [op, ..] = step;
        if op % 14 == 13 {
            round_trip(&mut run, &law)?;
            continue;
        }
        let Some((dead, delta, got)) = run.step(step) else {
            continue;
        };
        let now = counts(&run.euler);
        if got.is_ok()
            && let Some(n) = tally.get_mut(usize::from(op % 14))
        {
            *n = n.saturating_add(1);
        }
        match &got {
            Err(_) if run.euler != before => {
                return Err(law("a refusal changes nothing", format!("step {at}: {got:?}")));
            }
            Ok(()) if dead => {
                return Err(law("an operator refuses a dead row", format!("step {at}")));
            }
            Ok(()) => {
                let expected = Counts(core::array::from_fn(|i| {
                    let (w, d) = (was.0.get(i).copied(), delta.get(i).copied());
                    w.zip(d).map_or(0, |(w, d)| w.saturating_add(d))
                }));
                if now != expected {
                    return Err(law(
                        "an operator changes the counts by its row of the table",
                        format!("step {at}: {was:?} to {now:?}, not {expected:?}"),
                    ));
                }
            }
            Err(e) if dead && !matches!(e, TopoError::Euler(Refusal::Dead { .. })) => {
                return Err(law("a dead row is refused as dead", format!("step {at}: {e:?}")));
            }
            Err(_) => {}
        }
        let [v, e, f, l, s] = now.0;
        let chi = v
            .saturating_sub(e)
            .saturating_add(f)
            .saturating_sub(l.saturating_sub(f));
        if chi != s.saturating_sub(run.holes).saturating_mul(2) {
            return Err(law(
                "V - E + F - R = 2 (S - H) after every step",
                format!("step {at}: {now:?}, {} holes", run.holes),
            ));
        }
    }
    round_trip(&mut run, &law)?;
    Ok(tally)
}

/// Publishes the editor, edits the body again, and publishes that: the same counts, the same body.
fn round_trip(run: &mut Run, law: &impl Fn(&str, String) -> Finding) -> Result<(), Finding> {
    let meter = || WorkMeter::new(Budget::UNLIMITED);
    let Some(outside) = ix(0) else {
        return Ok(());
    };
    let body = match run.euler.clone().publish(BodyKind::General, outside, &mut meter()) {
        Ok(body) => body,
        Err(e) => return Err(law("an editor publishes", format!("{e:?}"))),
    };
    let again = match Euler::edit(&body) {
        Ok(again) => again,
        Err(e) => return Err(law("a published body edits again", format!("{e:?}"))),
    };
    if counts(&again) != counts(&run.euler) {
        return Err(law(
            "a published editor edits back to the same counts",
            format!("{:?}, then {:?}", counts(&run.euler), counts(&again)),
        ));
    }
    let republished = again.clone().publish(BodyKind::General, body.outside(), &mut meter());
    if republished.as_ref().ok() != Some(&body) {
        return Err(law(
            "an edited body publishes to itself",
            format!("{republished:?}"),
        ));
    }
    run.euler = again;
    Ok(())
}
