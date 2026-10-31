//! A body's faces with their names and roles (22: "list a body's faces with their names and
//! roles"; piece G102).
//!
//! What it does: [`faces`] lists every face of a body as `geom_topo::query::faces` does, with its
//! persistent name and the roles that name goes back to. A name does not hold its role in a form
//! the body keeps, so the roles are read from the histories of the operations that made the body,
//! oldest first, the same records attributes follow (20.3): a generated entity has its role, a
//! modified one the roles of what it carries on, a split piece those of what it was split from, and
//! a merged entity those of everything merged into it. [`roles_of`] is that walk for one name.
//!
//! Guarantees:
//!
//! - **Read from history, never guessed.** A name no history covers has no roles; nothing is
//!   inferred from a face's place, shape or position in a list (20.4).
//! - **One answer.** Roles are listed sorted and without repeats, so the answer does not depend on
//!   the order the walk met them.
//! - **Ends.** Each step moves to an earlier history or finishes, so the walk visits at most one
//!   entry per name and history, each charged one unit.
//!
//! Method: chapter 20.3's history records read backward, as `crate::resolve` reads them forward
//! (`projects/engines/geometry/architecture/20-naming-history.md`).
//!
//! Cost: for each face, linear in the entries its name reaches, each one lookup in a history (a
//! binary search); the lists allocate. Recursion: none; an explicit stack.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use geom_num::Exactness;
use geom_op::history::{History, Origin};
use geom_op::{NameId, OpContext, OpError, OpOutput, Role};
use geom_run::{Stopped, WorkMeter};
use geom_topo::Body as TopoBody;
use geom_topo::query::{self, FaceRow};
use geom_topo::query_ops::failed;

/// One face, as [`faces`] lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedFace {
    /// The face's row.
    pub row: FaceRow,
    /// Its persistent name, for a body published by name.
    pub name: Option<NameId>,
    /// The roles its name goes back to, sorted.
    pub roles: Vec<Role>,
}

/// The roles `name` goes back to through `histories`, oldest first, sorted.
///
/// # Errors
///
/// The meter's stop.
pub fn roles_of(
    name: NameId,
    histories: &[&History],
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Role>, Stopped> {
    let mut found: BTreeSet<Role> = BTreeSet::new();
    let mut seen: BTreeSet<(NameId, usize)> = BTreeSet::new();
    let mut stack = alloc::vec![(name, histories.len())];
    while let Some((id, upto)) = stack.pop() {
        if !seen.insert((id, upto)) {
            continue;
        }
        meter.charge(1)?;
        let earlier = histories.get(..upto).unwrap_or(&[]);
        // The latest history that says anything but "unchanged" about this name.
        let step = earlier.iter().enumerate().rev().find_map(|(at, h)| {
            h.origin(id)
                .filter(|o| !matches!(o, Origin::Unchanged))
                .map(|o| (at, o))
        });
        match step {
            None => {}
            Some((_, Origin::Generated { role, .. })) => {
                found.insert(role.clone());
            }
            Some((at, Origin::Modified { from })) => {
                stack.extend(from.names().iter().map(|&n| (n, at)));
            }
            Some((at, Origin::Split { of, .. })) => stack.push((*of, at)),
            Some((at, Origin::Merged { of })) => {
                stack.extend(of.names().iter().map(|&n| (n, at)));
            }
            Some((_, Origin::Unchanged)) => {}
        }
    }
    Ok(found.into_iter().collect())
}

/// `query.faces`: every face of `body`, recorded `exactness`, in arena order, with its name and the
/// roles `histories`, oldest first, give it.
///
/// # Errors
///
/// `QueryDetail::Malformed`, and the budget and cancellation errors of the context.
pub fn faces(
    ctx: &OpContext<'_>,
    body: &TopoBody,
    histories: &[&History],
    exactness: Exactness,
) -> Result<OpOutput<Vec<NamedFace>>, OpError> {
    ctx.run(|meter| {
        let rows = query::faces(body, meter).map_err(|e| failed(ctx, meter, e))?;
        let mut out = Vec::new();
        for row in rows {
            let name = row.name.map(|key| NameId::from_bytes(key.0));
            let roles = match name {
                Some(id) => roles_of(id, histories, meter)
                    .map_err(|s| failed(ctx, meter, query::QueryError::Stopped(s)))?,
                None => Vec::new(),
            };
            out.push(NamedFace { row, name, roles });
        }
        Ok(OpOutput::new(out, exactness))
    })
}
