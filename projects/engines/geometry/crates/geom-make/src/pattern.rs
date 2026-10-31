//! Patterns: copies of one body, each by its own transform and keyed by the caller, each copy's
//! entities named as copies of the body's (8.4's `Copied`, which a pattern's schema declares).
//!
//! What it does: [`pattern`] runs one copy per [`Instance`], in key order: the body moved by the
//! instance's transform as the Transform family moves it, each entity named
//! `Created { op: the operation's path with the instance's key, role: copy(source), from: source }`
//! and recorded as generated from its source with that role. It returns one [`Made`] per instance,
//! keyed as given, and the least exact of their records.
//!
//! Guarantees:
//!
//! - **One answer per key, whatever the order given.** Instances run in key order; two with one
//!   key are refused ([`MakeDetail::RepeatedKey`]), and so is an empty pattern.
//! - **Copies, not the body.** Every copy's entities have names of their own, distinct across
//!   instances (each instance's path differs), and trace to the body's entities as copies.
//! - **Each copy checked.** Every copy passes the checker before the pattern returns; one failure
//!   fails the whole pattern, which returns nothing.
//!
//! Method: chapter 10.1's batch rule (an item's path is the operation's path and its key) and
//! chapter 8.4's `Copied` event (`projects/engines/geometry/architecture/10-operations.md`,
//! `projects/engines/geometry/architecture/08-topology.md`).
//!
//! Cost: one transform of the body per instance. Recursion: none.

use alloc::vec::Vec;

use geom_math::TypedTransform;
use geom_num::Exactness;
use geom_op::history::Origin;
use geom_op::{
    MakeDetail, NameList, NameSet, OpContext, OpError, OpOutput, Role, RoleArg, TopoName,
};

use crate::build::{Made, fail, meter};
use crate::transform::{Moved, moved_copy, settle};

/// The copies a pattern returns: each instance's key and what it made, in key order.
pub type Copies = Vec<([u8; 16], Made)>;

/// One copy of a pattern: its key and its transform.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// The caller's key for the copy, which its entities' names carry.
    pub key: [u8; 16],
    /// Where the copy goes.
    pub transform: TypedTransform,
    /// The largest factor the transform scales by, as a numerator and a denominator: (1, 1) for
    /// moves, rotations and mirrors.
    pub stretch: (u64, u64),
}

/// Copies of `input.body`, one per instance, in key order.
///
/// # Errors
///
/// [`MakeDetail::EmptyPattern`], [`MakeDetail::RepeatedKey`], [`MakeDetail::BodyNotNamed`]; as
/// the Transform family for each copy.
pub fn pattern(
    ctx: &OpContext<'_>,
    input: Moved<'_>,
    instances: &[Instance],
) -> Result<OpOutput<Copies>, OpError> {
    if instances.is_empty() {
        return Err(fail(ctx, MakeDetail::EmptyPattern));
    }
    let mut order: Vec<&Instance> = instances.iter().collect();
    order.sort_unstable_by_key(|i| i.key);
    if order
        .windows(2)
        .any(|w| matches!(w, [a, b] if a.key == b.key))
    {
        return Err(fail(ctx, MakeDetail::RepeatedKey));
    }
    let names = input
        .body
        .names()
        .ok_or_else(|| fail(ctx, MakeDetail::BodyNotNamed))?
        .clone();
    let mut meter = meter(ctx);
    let mut out = Vec::with_capacity(order.len());
    let mut least = Exactness::Exact;
    for instance in order {
        let op = ctx.path().item(instance.key);
        let copy = |source| -> Option<_> {
            let role = Role::new("copy", [RoleArg::Name(source)]).ok()?;
            let name = TopoName::Created {
                op: op.clone(),
                role: role.clone(),
                from: NameList::new([source]),
            }
            .id()
            .ok()?;
            let from = NameSet::new([source]).ok()?;
            Some((name, Origin::Generated { from, role }))
        };
        let copied = moved_copy(
            ctx,
            &mut meter,
            input,
            &instance.transform,
            instance.stretch,
            &names,
            &copy,
        )?;
        let made = settle(ctx, &mut meter, copied)?;
        least = less_exact(least, made.exactness);
        out.push((instance.key, made.result));
    }
    Ok(OpOutput::new(out, least))
}

/// The less exact of `a` and `b`: the higher rank, then the larger bound.
fn less_exact(a: Exactness, b: Exactness) -> Exactness {
    let key = |e: Exactness| (e.rank(), e.bound().map(|(s, _)| s.get()));
    if key(b) > key(a) { b } else { a }
}
