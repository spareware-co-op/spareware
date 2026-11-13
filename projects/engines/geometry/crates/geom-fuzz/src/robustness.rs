//! Robustness, counted: of a fixed, seeded set of generated designs per operation family, how many
//! succeed (chapter 28.5).
//!
//! What it does: a typed error is legal, but an error where a person would expect success is a
//! gap, and a crash hunt does not count gaps. [`tally`] runs [`DESIGNS`] designs per family, each
//! the same on every run and target, through the kernel's operations, and counts the successes.
//! `src/robustness.txt` records them; `tests/robustness.rs` fails if a count falls.
//!
//! Guarantees:
//!
//! - **The same designs every time.** Each design comes from a fixed seed through
//!   `geom_test::SplitMix64`, so a count moves only when an operation's answer moves.
//! - **Plain designs, not the fuzzer's.** A design is what a person would draw: sizes in whole
//!   millimetres, near the origin, on the sub-grids. The fuzzer's hard cases are mostly refused
//!   for good reason; counting those would measure the generator. A design accepts no loss of
//!   exactness, so a scale of a curved body, which would widen its bound, counts as refused, as it
//!   is until the caller accepts it.
//!
//! Method: chapter 28.5's "Robustness is counted, not only crashes"
//! (`projects/engines/geometry/architecture/28-testing.md`).
//!
//! Cost: linear in [`DESIGNS`]: one or two operations a design.

use std::collections::BTreeMap;

use geom_format::operation::Operation;
use geom_format::recipe::RecipeInput;
use geom_make::{BoxInput, PrismInput, RoundInput, SphereInput, TorusInput};
use geom_math::Axis;
use geom_num::{Exactness, GridStep, P3, Steps};
use geom_op::{OpId, OpPath};
use geom_run::Budget;
use geom_test::fixture::calls::{Call, Input, Turn};
use geom_test::fixture::kernel::{Kernel, id_of};
use geom_test::fixture::run::{Engine, Ran};
use geom_test::{Seed, SplitMix64};

use crate::adversarial::{MILLIMETRE, ORIGIN};

/// The designs of each family.
pub const DESIGNS: u32 = 400;

/// The recorded counts: `family successes designs`, one a line.
pub const RECORDED: &str = include_str!("robustness.txt");

/// A whole number of millimetres from 1 to 1,000.
fn millimetres(g: &mut SplitMix64) -> u64 {
    (g.next_u64() % 1_000)
        .saturating_add(1)
        .saturating_mul(MILLIMETRE.unsigned_abs())
}

/// A coordinate within a metre of the origin, in whole millimetres.
fn near(g: &mut SplitMix64) -> i64 {
    i64::try_from(g.next_u64() % 2_001)
        .unwrap_or(0)
        .saturating_sub(1_000)
        .saturating_mul(MILLIMETRE)
}

fn point(g: &mut SplitMix64) -> P3 {
    let (x, y, z) = (near(g), near(g), near(g));
    P3::new(x, y, z).unwrap_or(ORIGIN)
}

/// A plain primitive.
#[allow(
    clippy::many_single_char_names,
    reason = "a and b the sizes, x, y and z the corner, g the generator"
)]
fn primitive(g: &mut SplitMix64) -> Call {
    let at = point(g);
    let (a, b) = (Steps(millimetres(g)), Steps(millimetres(g)));
    let step = GridStep::Q0;
    match g.next_u64() % 6 {
        0 => Call::Box(BoxInput {
            min: at,
            size: [a, b, Steps(millimetres(g))],
        }),
        1 => {
            let z = at.z.get();
            let (x, y) = (at.x.get(), at.y.get());
            let span = i64::try_from(a.get()).unwrap_or(MILLIMETRE);
            let corner = |dx: i64, dy: i64| P3::new(x.saturating_add(dx), y.saturating_add(dy), z);
            let base = [corner(0, 0), corner(span, 0), corner(0, span)];
            Call::Prism(PrismInput {
                base: base.into_iter().filter_map(Result::ok).collect(),
                height: b,
            })
        }
        2 => Call::Cylinder(RoundInput {
            base: at,
            radius: a,
            height: b,
            step,
        }),
        3 => Call::Cone(RoundInput {
            base: at,
            radius: a,
            height: b,
            step,
        }),
        4 => Call::Sphere(SphereInput {
            center: at,
            radius: a,
            step,
        }),
        _ => Call::Torus(TorusInput {
            center: at,
            major: Steps(a.get().saturating_add(b.get())),
            minor: Steps(a.get().min(b.get())),
            step,
        }),
    }
}

/// A plain transform of a body whose record is `exactness`.
fn transform(g: &mut SplitMix64, exactness: Exactness) -> Call {
    let input = Input {
        exactness,
        accept: None,
    };
    let axis = match g.next_u64() % 3 {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    };
    match g.next_u64() % 4 {
        0 => Call::Move(input, [near(g), near(g), near(g)]),
        1 => {
            let turns = i32::try_from(g.next_u64() % 4).unwrap_or(1);
            Call::Rotate(input, axis, Turn::Quarter(turns), point(g))
        }
        2 => Call::Mirror(input, axis, point(g)),
        _ => {
            let f = (g.next_u64() % 4).saturating_add(1);
            Call::Scale(input, [(f, 1); 3], point(g))
        }
    }
}

fn path(at: u64) -> OpPath {
    let mut id = [0u8; 16];
    id.iter_mut()
        .zip(at.to_be_bytes())
        .for_each(|(slot, b)| *slot = b);
    OpPath::new(OpId::from_bytes(id))
}

fn operation(at: u64, call: &Call, input: Vec<RecipeInput>) -> Option<Operation> {
    Some(Operation {
        op: path(at),
        name: call.name().to_owned(),
        version: 1,
        inputs: input,
        params: call.params().ok()?,
        budget: Budget::UNLIMITED,
    })
}

/// Each family's successes and designs, in family order.
#[must_use]
pub fn tally() -> BTreeMap<&'static str, (u32, u32)> {
    let mut counts = BTreeMap::new();
    let mut g = SplitMix64::new(Seed::DEFAULT);
    let mut kernel = Kernel::new();
    let mut make = 0;
    let mut moved = 0;
    for design in 0..u64::from(DESIGNS) {
        if kernel.start(Steps(0), &[]).is_err() {
            continue;
        }
        let call = primitive(&mut g);
        let Some(first) = operation(design, &call, Vec::new()) else {
            continue;
        };
        if let Ran::Done(step) = kernel.run(&first) {
            make = u32::saturating_add(make, 1);
            let Some(id) = kernel.last().first().and_then(|b| id_of(b).ok()) else {
                continue;
            };
            let then = transform(&mut g, step.exactness);
            let Some(second) = operation(design, &then, vec![RecipeInput::Body(id)]) else {
                continue;
            };
            if matches!(kernel.run(&second), Ran::Done(_)) {
                moved = u32::saturating_add(moved, 1);
            }
        }
    }
    counts.insert("make", (make, DESIGNS));
    counts.insert("transform", (moved, make));
    counts
}

/// The counts as `src/robustness.txt` writes them.
#[must_use]
pub fn render(counts: &BTreeMap<&'static str, (u32, u32)>) -> String {
    let mut out = String::new();
    for (family, (done, designs)) in counts {
        for field in [(*family).to_owned(), done.to_string(), designs.to_string()] {
            out.push_str(&field);
            out.push(' ');
        }
        out.pop();
        out.push('\n');
    }
    out
}
