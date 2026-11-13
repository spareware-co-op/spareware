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
//! A body that is not recorded Exact may have faces its rounded corners have bent, and on those
//! the exact volume and the exact side both read a loop from its first corner, which a body's
//! names choose and its geometry does not (`crate::planar`, on why a bent face is a star and not
//! a fan). So such a body is read another way:
//!
//! - **Its volume's sign from a bracket.** A face's term in six times the volume is its
//!   twice-area vector A dotted with a point of the face. On a flat face every corner gives the
//!   same number; on a bent one they differ, and the star's term is A dotted with the mean of the
//!   corners, the mean of theirs. [`volume_sign`] adds the least any corner gives, face by face,
//!   and the most: two exact integers with the star's volume, and every fan's, between them. Both
//!   of one sign is the shell's sign; otherwise the shell is thinner than its corners' rounding,
//!   and it is [`FaultKind::Unchecked`].
//! - **Its nesting through the stars.** Every pair of shells goes to `crate::planar::shell_side`,
//!   which reads each face as group 3 read it against the others, an outline or a star.
//!
//! Both depend on where the corners lie and on nothing else, so a body, its mirror, and every
//! exact move, quarter turn, scale by one factor and renamed copy of it get one verdict: a face's
//! area vector and its set of corners are the same wherever its ring starts; a move adds to each
//! face's term A dotted with the move, and a closed shell's area vectors add up to nothing; a turn
//! turns A and the corners together; a mirror turns each ring round as well, which gives A back
//! its sign; and a scale multiplies every term by one positive number.
//!
//! Guarantees:
//!
//! - **Exact.** Signs of exact integers and the exact predicates behind `shell_side`; no double
//!   decides anything here, but for the sign of a shell with a vertex where three planes meet, a
//!   certified interval measured from the shell's own corner. A sign that cannot decide is
//!   [`FaultKind::Unchecked`], never a pass.
//! - **An Exact body is read as before.** Its faces are flat to the last bit, so the exact volume
//!   and the winding number do not depend on a ring's start, and their cost is unchanged.
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

use geom_num::interval::Interval;
use geom_num::wide::I256;
use geom_num::{P3, Sign};
use geom_run::{Stopped, WorkMeter};
use geom_topo::entity::{Face, Region, RegionIx, RegionKind, Shell, ShellIx};
use geom_topo::ix::Arena;
use geom_topo::query::{
    QueryError, Side, certified_shell_volume, face_area, point, shell_side, shell_volume,
};
use geom_topo::{Arenas, Body, BodyKind};

use crate::combinatorial::{face_fins, fin_ends, rows};
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
        found => found.ok().and_then(Interval::certain_sign),
    };
    Ok(judged(sign, s, sink))
}

/// The word for shell `s` with the sign `sign` of its volume, if it has one. An enclosure of the
/// volume leaves a sign open only when its fan's terms cancel past their last place, and each is
/// exact to 1024 bits, so no fixture reaches that: a folded sheet thinner than a step would. A
/// bracket leaves one open for a bent shell thinner than its corners' rounding ([`volume_sign`]).
/// Either is a fault, never a pass.
fn judged(sign: Option<Sign>, s: ShellIx, sink: &mut Sink<'_, '_>) -> u32 {
    if sign.is_none() {
        sink.push(Fault::at(FaultKind::Unchecked, Entity::Shell(s)));
    }
    sign.map_or(UNJUDGED, word_of)
}

/// `area` · `p`, exactly: `None` past an `I256`, which no area vector of an arena reaches (each
/// coordinate is below 2^114, a point's below 2^40).
fn dotted(area: [i128; 3], p: P3) -> Option<I256> {
    let mut sum = Some(I256::from_i64(0));
    for (c, x) in area.into_iter().zip([p.x, p.y, p.z]) {
        let term = I256::from_i128(c).checked_mul(&I256::from_i64(x.get()));
        sum = sum.zip(term).and_then(|(s, t)| s.checked_add(&t));
    }
    sum
}

/// The sign of six times the volume shell `s` encloses, read from where its corners lie and from
/// nothing else, for a body that is not recorded Exact: `None` if the corners leave it open.
///
/// Six times the volume is Σ over the faces of A · p, A the face's twice-area vector and p a point
/// of it. A is the same wherever the face's rings start. On a flat face every corner gives the
/// same A · p. On a face its rounded corners have bent they differ: the exact volume takes the
/// first corner of the first loop, which names choose, and the star group 3 reads the face as
/// takes the mean of the corners, whose A · p is the mean of theirs. So the sum of each face's
/// least A · p over its corners and the sum of each face's most bracket the star's volume and
/// every fan's, and are the same for every start, every numbering and both ways round. Both of
/// one sign is the sign. Otherwise the shell is thinner than its corners' rounding, and no sign is
/// given: on a shell of flat faces the two sums are one number, the exact volume.
///
/// Bit budget: a term is below 2^156 (above), and a shell has fewer than 2^32 faces, so each sum
/// is below 2^188, inside an `I256`; a sum that did not fit would be no sign.
///
/// # Errors
///
/// [`face_area`]'s, for a face that is not a polygon of grid points, and the meter's.
fn volume_sign(
    body: &Body,
    s: ShellIx,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<Sign>, QueryError> {
    let a = body.arenas();
    let zero = I256::from_i64(0);
    let mut sums = Some((zero, zero));
    for f in rows::<Face>(a.rows(Arena::Face)).filter(|&f| a.faces.shell.get(f) == Some(&s)) {
        let area = face_area(body, f, meter)?;
        // `Some(None)` before the first corner; `None` once a term does not fit.
        let mut span = Some(None::<(I256, I256)>);
        for n in face_fins(a, f) {
            meter.charge(1)?;
            // `face_area` has found a grid point where each fin starts.
            let term = fin_ends(a, n)
                .and_then(|(v, _)| point(body, v).ok())
                .and_then(|p| dotted(area, p));
            span = span.zip(term).map(|(held, t)| {
                Some(held.map_or((t, t), |(least, most)| (least.min(t), most.max(t))))
            });
        }
        sums = sums
            .zip(span.flatten())
            .and_then(|((least, most), (low, high))| {
                Some((least.checked_add(&low)?, most.checked_add(&high)?))
            });
    }
    Ok(sums.and_then(|(least, most)| (least.signum() == most.signum()).then(|| least.signum())))
}

/// Each shell's sign into `roles`, with its faults: for a body recorded Exact (`exact`) from its
/// exact volume, and otherwise from the bracket of [`volume_sign`].
fn signs(
    body: &Body,
    exact: bool,
    roles: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    let a = body.arenas();
    for s in rows::<Shell>(a.rows(Arena::Shell)) {
        sink.charge()?;
        let sign = if exact {
            // A closed shell that crosses nothing encloses something, so no sign here is zero.
            shell_volume(body, s, sink.meter()).map(|v| Some(v.signum()))
        } else {
            volume_sign(body, s, sink.meter())
        };
        let word = match sign {
            Ok(sign) => judged(sign, s, sink),
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
///
/// In a body recorded Exact (`exact`) it is the winding number of a vertex, and for a shell with
/// a vertex where three planes meet the parity of `crate::planar`'s ray. In any other body it is
/// that ray against each face as group 3 read it, an outline or a star: the winding number counts
/// a fan from each loop's first corner, which on a bent face is not where the star is.
fn side(
    body: &Body,
    exact: bool,
    (s, t): (ShellIx, ShellIx),
    sink: &mut Sink<'_, '_>,
) -> Result<Option<Side>, Stopped> {
    if exact {
        match shell_side(body, s, t, sink.meter()) {
            Err(QueryError::Stopped(stop)) => Err(stop),
            Err(QueryError::Implicit(_)) => planar::shell_side(body.arenas(), s, t, sink),
            found => Ok(found.ok().flatten()),
        }
    } else {
        planar::shell_side(body.arenas(), s, t, sink)
    }
}

/// Whether `s` lies inside a cavity of region `r`.
fn in_cavity(
    body: &Body,
    exact: bool,
    (s, r): (ShellIx, Option<RegionIx>),
    roles: &[u32],
    sink: &mut Sink<'_, '_>,
) -> Result<bool, Stopped> {
    let a = body.arenas();
    for c in rows::<Shell>(a.rows(Arena::Shell)) {
        sink.charge()?;
        if region(a, c) == r
            && role(roles, c) == CAVITY
            && side(body, exact, (s, c), sink)? == Some(Side::Inside)
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
fn nesting(
    body: &Body,
    exact: bool,
    roles: &[u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
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
            let found = side(body, exact, (s, t), sink)?;
            let fine = match found {
                None => {
                    sink.push(
                        Fault::at(FaultKind::PairUnchecked, Entity::Shell(s))
                            .with(Entity::Shell(t)),
                    );
                    continue;
                }
                Some(Side::Inside) if lumps => {
                    in_cavity(body, exact, (s, region(a, t)), roles, sink)?
                }
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

/// Shell orientation and nesting on a solid `body`, recorded Exact or not (`exact`); `scratch`
/// holds a word per shell.
pub(crate) fn check(
    body: &Body,
    exact: bool,
    scratch: &mut [u32],
    sink: &mut Sink<'_, '_>,
) -> Result<(), Stopped> {
    if body.kind() != BodyKind::Solid || sink.found() > 0 {
        return Ok(());
    }
    signs(body, exact, scratch, sink)?;
    outer_shells(body.arenas(), scratch, sink)?;
    if sink.found() == 0 {
        nesting(body, exact, scratch, sink)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a failure should stop loudly"
    )]

    use geom_run::{Budget, WorkMeter};
    use geom_topo::Ix;

    use super::*;

    #[test]
    fn a_volume_no_enclosure_can_sign_is_a_fault_never_a_pass() {
        let mut meter = WorkMeter::new(Budget::UNLIMITED);
        let mut sink = Sink::new(&mut meter).unwrap();
        let s = Ix::new(0).unwrap();
        assert_eq!(judged(None, s, &mut sink), UNJUDGED);
        assert_eq!(sink.faults.len(), 1);
        assert_eq!(sink.faults[0].kind, FaultKind::Unchecked);
        assert_eq!(
            judged(Some(Sign::Positive), s, &mut sink),
            word_of(Sign::Positive)
        );
        assert_eq!(
            judged(Some(Sign::Negative), s, &mut sink),
            word_of(Sign::Negative)
        );
        assert_eq!(sink.faults.len(), 1);
    }
}
