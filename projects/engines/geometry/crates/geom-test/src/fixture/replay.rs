//! The replay corpus: every fixture and a seeded operation log per family, the determinism matrix's
//! input (chapters 21 and 28.13; piece G98).
//!
//! What it does: [`seeded`] draws a family's logs from [`SEED`]: `box` is one box of known size,
//! `make` runs every primitive, `transform` builds a body and moves, mirrors and rotates it, and
//! `budget` runs out of its work and of its logical memory, so a stop by a budget is held to one hash
//! on every target. [`digest`]
//! replays the corpus and hashes what it gave: each case's id, its hash and its two counts, in order.
//! `tests/determinism.rs` holds that digest as the golden `geom-test/replay-corpus`, which
//! `golden run` then carries through every target and build mode, the release build as shipped
//! among them, whose checker runs only the cheap groups: one digest across the legs proves the
//! checker changes nothing (28.4).
//!
//! Guarantees:
//!
//! - **Fixed seeds.** The corpus is a pure function of [`SEED`] and the stored fixtures.
//! - **Work and logical bytes are in the digest**, so a count that moves is a changed golden.
//! - **Budget stops are errors like any other**: the two `budget` logs end in `BudgetExceeded`, and
//!   their hashes go into the digest.
//! - **A broken case stops the corpus**, so a digest never hides one.
//!
//! Method: chapter 28.13's replay corpus and budgets
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: one replay of every case.

use core::num::NonZeroU64;

use geom_format::content::ContentId;
use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_make::{BoxInput, PrismInput, RoundInput, SphereInput, TorusInput};
use geom_math::Axis;
use geom_num::grid::P3;
use geom_num::{Exactness, GridStep, Steps};
use geom_op::cbor::EncodeError;
use geom_op::{OpId, OpPath};
use geom_run::Budget;

use super::calls::{Call, Input, OPERATIONS, Turn};
use super::kernel::{Kernel, id_of};
use super::run::{Engine, Replayed, replay};
use super::{Fixture, OpLog};
use crate::golden::Sha256;
use crate::rng::SplitMix64;
use crate::seed::Seed;

/// The corpus's seed: "REPLAY01" in ASCII.
pub const SEED: Seed = Seed(0x5245_504c_4159_3031);

/// The families with a seeded log.
pub const FAMILIES: [&str; 4] = ["box", "make", "transform", "budget"];

/// More work and logical memory than any seeded operation asks for.
const PLENTY: Budget = Budget::new(1 << 40, 1 << 40);

/// How many operations the `make` family draws.
const DRAWS: u64 = 24;

const EXACT: Input = Input {
    exactness: Exactness::Exact,
    accept: None,
};

/// The operation record of `call`, as the `id`th of its log.
///
/// # Errors
///
/// The encoder's, which only a defect can cause.
pub fn operation_of(id: u64, call: &Call, budget: Budget) -> Result<Operation, EncodeError> {
    let mut bytes = [0_u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(id.to_be_bytes()) {
        *slot = byte;
    }
    Ok(Operation {
        op: OpPath::new(OpId::from_bytes(bytes)),
        name: call.name().to_owned(),
        version: OPERATIONS
            .iter()
            .find(|(name, _)| *name == call.name())
            .map_or(1, |(_, version)| *version),
        inputs: Vec::new(),
        params: call.params()?,
        budget,
    })
}

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    NonZeroU64::new(n).map_or(0, |n| rng.below(n))
}

/// A length from 1 to 2^20 steps.
fn size(rng: &mut SplitMix64) -> Steps {
    Steps(below(rng, 1 << 20).saturating_add(1))
}

fn coordinate(rng: &mut SplitMix64) -> i64 {
    i64::try_from(below(rng, 1 << 24))
        .unwrap_or(0)
        .saturating_sub(1 << 23)
}

fn point(rng: &mut SplitMix64) -> Option<P3> {
    let [x, y, z] = [coordinate(rng), coordinate(rng), coordinate(rng)];
    P3::new(x, y, z).ok()
}

/// Eight corners in convex position, counter-clockwise from above.
const RING: [(i64, i64); 8] = [
    (4, 0),
    (3, 3),
    (0, 4),
    (-3, 3),
    (-4, 0),
    (-3, -3),
    (0, -4),
    (3, -3),
];

/// A strictly convex polygon of three to eight corners on one level, counter-clockwise from above.
fn polygon(rng: &mut SplitMix64) -> Option<Vec<P3>> {
    let z = coordinate(rng);
    let scale = i64::try_from(below(rng, 1 << 12))
        .unwrap_or(0)
        .saturating_add(1);
    let keep = usize::try_from(below(rng, 6))
        .unwrap_or(0)
        .saturating_add(3);
    RING.iter()
        .take(keep)
        .map(|(x, y)| P3::new(x.saturating_mul(scale), y.saturating_mul(scale), z).ok())
        .collect()
}

fn grid(rng: &mut SplitMix64) -> GridStep {
    if below(rng, 2) == 0 {
        GridStep::Q0
    } else {
        GridStep::Q1
    }
}

/// One drawn primitive.
fn primitive(rng: &mut SplitMix64) -> Option<Call> {
    let at = point(rng)?;
    Some(match below(rng, 6) {
        0 => Call::Box(BoxInput {
            min: at,
            size: [size(rng), size(rng), size(rng)],
        }),
        1 => Call::Prism(PrismInput {
            base: polygon(rng)?,
            height: size(rng),
        }),
        2 => Call::Cylinder(RoundInput {
            base: at,
            radius: size(rng),
            height: size(rng),
            step: grid(rng),
        }),
        3 => Call::Cone(RoundInput {
            base: at,
            radius: size(rng),
            height: size(rng),
            step: grid(rng),
        }),
        4 => Call::Sphere(SphereInput {
            center: at,
            radius: size(rng),
            step: grid(rng),
        }),
        _ => {
            let minor = size(rng);
            Call::Torus(TorusInput {
                center: at,
                major: Steps(minor.0.saturating_add(size(rng).0)),
                minor,
                step: grid(rng),
            })
        }
    })
}

/// A box of 1 by 0.5 by 0.25 mm at the origin.
fn small_box() -> Option<Call> {
    Some(Call::Box(BoxInput {
        min: P3::new(0, 0, 0).ok()?,
        size: [Steps(6400), Steps(3200), Steps(1600)],
    }))
}

/// The content ID of what the log `made` leaves behind.
fn made_id(made: &Operation) -> Result<ContentId, String> {
    let mut kernel = Kernel::new();
    let log = OpLog::new(Vec::new(), vec![made.clone()]);
    match replay(&mut kernel, &log) {
        Replayed::Done { .. } => {}
        other => return Err(format!("{other:?}")),
    }
    let body = kernel
        .last()
        .first()
        .ok_or_else(|| "it made nothing".to_owned())?;
    id_of(body)
}

/// A body made and then moved, mirrored and rotated, each from the one made.
fn transforms() -> Result<OpLog, String> {
    let text = |e: EncodeError| e.to_string();
    let origin = P3::new(0, 0, 0).map_err(|e| e.to_string())?;
    let made =
        operation_of(1, &small_box().ok_or_else(|| "no box".to_owned())?, PLENTY).map_err(text)?;
    let id = made_id(&made)?;
    let calls = [
        Call::Move(EXACT, [1, -2, 3]),
        Call::Mirror(EXACT, Axis::Z, origin),
        Call::Rotate(EXACT, Axis::Z, Turn::Quarter(1), origin),
    ];
    let mut operations = vec![made];
    for (at, call) in calls.iter().enumerate() {
        let number = u64::try_from(at).unwrap_or(0).saturating_add(2);
        let mut operation = operation_of(number, call, PLENTY).map_err(text)?;
        operation.inputs = vec![RecipeInput::Body(id.clone())];
        operations.push(operation);
    }
    Ok(OpLog::new(Vec::new(), operations))
}

/// The seeded logs of `family`; `None` for a family the corpus does not have.
///
/// # Errors
///
/// Why a log cannot be drawn, which only a defect can cause.
pub fn seeded(family: &str) -> Result<Option<Vec<OpLog>>, String> {
    let text = |e: EncodeError| e.to_string();
    let single = |operation: Operation| OpLog::new(Vec::new(), vec![operation]);
    match family {
        // One log per primitive: each stands alone, since no operation joins them yet.
        "make" => {
            let mut rng = SplitMix64::new(SEED);
            let mut logs = Vec::new();
            for id in 0..DRAWS {
                if let Some(call) = primitive(&mut rng) {
                    logs.push(single(operation_of(id, &call, PLENTY).map_err(text)?));
                }
            }
            Ok(Some(logs))
        }
        "box" => {
            let call = small_box().ok_or_else(|| "no box".to_owned())?;
            Ok(Some(vec![single(
                operation_of(1, &call, PLENTY).map_err(text)?,
            )]))
        }
        "transform" => Ok(Some(vec![transforms()?])),
        "budget" => {
            let call = small_box().ok_or_else(|| "no box".to_owned())?;
            Ok(Some(vec![
                single(operation_of(1, &call, Budget::new(8, 1 << 40)).map_err(text)?),
                single(operation_of(2, &call, Budget::new(1 << 40, 64)).map_err(text)?),
            ]))
        }
        _ => Ok(None),
    }
}

/// Writes one case into the digest, or says why it is broken.
fn add(sha: &mut Sha256, id: &str, replayed: &Replayed) -> Result<(), String> {
    sha.write(id.as_bytes());
    sha.write(&[0]);
    match replayed {
        Replayed::Done { hash, work, .. } | Replayed::Failed { hash, work, .. } => {
            sha.write(&hash.0);
            sha.write(&work.work.to_be_bytes());
            sha.write(&work.logical_bytes.to_be_bytes());
            Ok(())
        }
        Replayed::Broken(why) => Err(format!("{id}: {why}")),
    }
}

/// Replays every case through `engine`, and hashes them.
///
/// # Errors
///
/// A broken case, or a log that cannot be drawn.
pub fn digest(fixtures: &[Fixture], engine: &mut dyn Engine) -> Result<[u8; 32], String> {
    let mut sha = Sha256::new();
    for fixture in fixtures {
        add(&mut sha, &fixture.id, &replay(engine, &fixture.log))?;
    }
    for family in FAMILIES {
        let logs = seeded(family)?.unwrap_or_default();
        for (at, log) in logs.iter().enumerate() {
            add(&mut sha, &format!("{family}/{at}"), &replay(engine, log))?;
        }
    }
    Ok(sha.finish())
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "test cases; the rules are for kernel code"
)]
mod tests {
    use super::RING;

    #[test]
    fn the_ring_turns_left_at_every_corner() {
        let next = RING.iter().cycle().skip(1);
        let after = RING.iter().cycle().skip(2);
        for ((a, b), c) in RING.iter().zip(next).zip(after) {
            let turn = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
            assert!(turn > 0, "{a:?} {b:?} {c:?}");
        }
    }
}
