//! Group 3 of chapter 8.3 on a solid's flat shells: each faces the way its place says, and each
//! lies where its region allows (issue #438's flat half, with G177 for curved shells).
//!
//! What it does: [`check`] takes the sign of each shell's exact six-times volume
//! (`geom_topo::query::shell_volume`), or for a shell with a vertex where three planes meet its
//! certified one (`geom_topo::query::certified_shell_volume`): positive faces outward, and anything
//! else is a cavity's. A solid region must have exactly one shell facing outward. Then, on a
//! body with no fault so far, it compares shells by which side of each other they lie
//! (`geom_topo::query::shell_side`, and for such a shell `crate::planar::shell_side`): a cavity
//! lies inside its region's outer shell and outside the
//! region's other cavities, and a solid region's outer shell inside another's lies inside one of
//! that region's cavities. A shell with a curved face or edge is left for G177 and counted in the
//! report ([`crate::Report::deferred`]), and judges nothing else.
//!
//! Guarantees:
//!
//! - **Exact.** Signs of exact integers and the exact predicates behind `shell_side`; no double
//!   decides anything here, but for the sign of a shell with a vertex where three planes meet, a
//!   certified interval measured from the shell's own corner. A sign that cannot decide is
//!   [`FaultKind::Unchecked`], never a pass.
//! - **Runs on bodies that do not cross.** It runs only on a solid body that passed everything
//!   before it, so each shell is closed, its loops run true, and two shells lie wholly on one side
//!   of each other: one vertex decides.
//! - **Fails closed.** Two shells where every vertex of one is a vertex of the other have no vertex
//!   to decide by, and are [`FaultKind::PairUnchecked`], never a pass.
//! - **Charged.** One meter unit per shell and per ordered pair; the walks charge their own rows.
//!
//! Method: chapter 8.1's regions and shells and 8.3's rule that regions nest correctly
//! (`projects/engines/geometry/architecture/08-topology.md`), by signed volume and point-in-shell
//! classification as `geom_topo::query` computes them (Feito and Torres, "Inclusion test for general
//! polyhedra", doi:10.1016/S0097-8493(96)00028-8).
//!
//! Cost: linear in faces and fins for the volumes; quadratic in shells, each pair linear in the
//! second shell's fins, for the nesting; nothing allocated (one word per shell of the shared
//! scratch). Recursion: none.

use geom_num::Sign;
use geom_run::Stopped;
use geom_topo::entity::{Region, RegionIx, RegionKind, Shell, ShellIx};
use geom_topo::ix::Arena;
use geom_topo::query::{QueryError, Side, certified_shell_volume, shell_side, shell_volume};
use geom_topo::{Arenas, Body, BodyKind};

use crate::combinatorial::rows;
use crate::planar;
use crate::report::{Entity, Fault, FaultKind};
use crate::sink::Sink;

/// A shell's row in the scratch: unjudged (curved, or enclosing nothing).
const UNJUDGED: u32 = 0;
/// A shell's row in the scratch: facing outward.
const OUTER: u32 = 1;
/// A shell's row in the scratch: a cavity's, facing into it.
const CAVITY: u32 = 2;

/// The word `s` keeps in `roles`.
fn role(roles: &[u32], s: ShellIx) -> u32 {
    roles.get(s.to_usize()).copied().unwrap_or(UNJUDGED)
}

/// The region of `s`.
fn region(a: &Arenas, s: ShellIx) -> Option<RegionIx> {
    a.shells.region.get(s).copied()
}

/// The word for a shell of a sign.
fn word_of(sign: Sign) -> u32 {
    if sign == Sign::Positive {
        OUTER
    } else {
        CAVITY
    }
}

/// The word for shell `s`, which has a vertex where three planes meet, by its certified volume.
fn certified_word(body: &Body, s: ShellIx, sink: &mut Sink<'_, '_>) -> Result<u32, Stopped> {
    let sign = match certified_shell_volume(body, s, sink.meter()) {
        Err(QueryError::Stopped(stop)) => return Err(stop),
        found => found.ok().and_then(|v| v.certain_sign()),
    };
    Ok(sign.map_or_else(
        || {
            sink.push(Fault::at(FaultKind::Unchecked, Entity::Shell(s)));
            UNJUDGED
        },
        word_of,
    ))
}

/// Each shell's sign into `roles`, with its faults.
fn signs(body: &Body, roles: &mut [u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let a = body.arenas();
    for s in rows::<Shell>(a.rows(Arena::Shell)) {
        sink.charge()?;
        let word = match shell_volume(body, s, sink.meter()) {
            // A closed shell that crosses nothing encloses something, so no sign here is zero.
            Ok(v) => word_of(v.signum()),
            Err(QueryError::Stopped(stop)) => return Err(stop),
            Err(QueryError::Implicit(_)) => certified_word(body, s, sink)?,
            Err(_) => {
                sink.defer();
                UNJUDGED
            }
        };
        roles
            .get_mut(s.to_usize())
            .into_iter()
            .for_each(|slot| *slot = word);
    }
    Ok(())
}

/// Exactly one shell of each solid region faces outward.
fn outer_shells(a: &Arenas, roles: &[u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    for r in rows::<Region>(a.rows(Arena::Region)) {
        if a.regions.kind.get(r).copied() != Some(RegionKind::Solid) {
            continue;
        }
        let mut outer = None;
        let mut first = None;
        let mut unjudged = false;
        for s in rows::<Shell>(a.rows(Arena::Shell)).filter(|&s| region(a, s) == Some(r)) {
            sink.charge()?;
            first = first.or(Some(s));
            match (role(roles, s), outer) {
                (OUTER, None) => outer = Some(s),
                (OUTER, Some(o)) => sink.push(
                    Fault::at(FaultKind::ShellOutward, Entity::Shell(s)).with(Entity::Shell(o)),
                ),
                (UNJUDGED, _) => unjudged = true,
                _ => {}
            }
        }
        if let (None, false, Some(s)) = (outer, unjudged, first) {
            sink.push(Fault::at(FaultKind::ShellInward, Entity::Region(r)).with(Entity::Shell(s)));
        }
    }
    Ok(())
}

/// Which side of `t` shell `s` lies on; `None` if no vertex decides, or the walk could not.
fn side(
    body: &Body,
    s: ShellIx,
    t: ShellIx,
    sink: &mut Sink<'_, '_>,
) -> Result<Option<Side>, Stopped> {
    match shell_side(body, s, t, sink.meter()) {
        Err(QueryError::Stopped(stop)) => Err(stop),
        Err(QueryError::Implicit(_)) => planar::shell_side(body.arenas(), s, t, sink),
        found => Ok(found.ok().flatten()),
    }
}

/// Whether `s` lies inside a cavity of region `r`.
fn in_cavity(
    body: &Body,
    s: ShellIx,
    r: Option<RegionIx>,
    roles: &[u32],
    sink: &mut Sink<'_, '_>,
) -> Result<bool, Stopped> {
    let a = body.arenas();
    for c in rows::<Shell>(a.rows(Arena::Shell)) {
        sink.charge()?;
        if region(a, c) == r
            && role(roles, c) == CAVITY
            && side(body, s, c, sink)? == Some(Side::Inside)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// What one ordered pair of shells must be: `Some` when its roles and regions fix a side.
fn wanted(same_region: bool, s: u32, t: u32) -> Option<Side> {
    match (same_region, s, t) {
        (true, CAVITY, OUTER) => Some(Side::Inside),
        (true, CAVITY, CAVITY) => Some(Side::Outside),
        _ => None,
    }
}

/// Every ordered pair of judged shells against what its roles require.
fn nesting(body: &Body, roles: &[u32], sink: &mut Sink<'_, '_>) -> Result<(), Stopped> {
    let a = body.arenas();
    for s in rows::<Shell>(a.rows(Arena::Shell)) {
        for t in rows::<Shell>(a.rows(Arena::Shell)).filter(|&t| t != s) {
            sink.charge()?;
            let (first, second) = (role(roles, s), role(roles, t));
            let same = region(a, s) == region(a, t);
            let lumps = !same && first == OUTER && second == OUTER;
            let want = wanted(same, first, second);
            if want.is_none() && !lumps {
                continue;
            }
            let found = side(body, s, t, sink)?;
            let fine = match found {
                None => {
                    sink.push(
                        Fault::at(FaultKind::PairUnchecked, Entity::Shell(s))
                            .with(Entity::Shell(t)),
                    );
                    continue;
                }
                Some(Side::Inside) if lumps => in_cavity(body, s, region(a, t), roles, sink)?,
                Some(_) if lumps => true,
                side_found => side_found == want,
            };
            if !fine {
                sink.push(
                    Fault::at(FaultKind::ShellNesting, Entity::Shell(s)).with(Entity::Shell(t)),
                );
            }
        }
    }
    Ok(())
}

/// Shell orientation and nesting on a solid `body`; `scratch` holds a word per shell.
pub(crate) fn check(
    body: &Body,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    if body.kind() != BodyKind::Solid || sink.found() > 0 {
        return Ok(());
    }
    signs(body, scratch, sink)?;
    outer_shells(body.arenas(), scratch, sink)?;
    if sink.found() == 0 {
        nesting(body, scratch, sink)?;
    }
    Ok(())
}
