//! Assembling what a Make or Transform operation returns: a body's rows, each entity named by its
//! role, published in name order, checked, and the history that says where each entity came
//! from.
//!
//! What it does: a [`Part`] lists a solid's vertices, edges and faces, each with its geometry and
//! the role it is named by (20.2), and each face's loops as edges and senses. [`assemble`] names
//! every region, face, edge and vertex `Created { op, role, from }` (`geom_op::TopoName`), builds
//! the rows (one solid region and the void outside it, one shell, a ring of fins round each edge),
//! publishes them in name order (`geom_topo::Builder::publish_named`), runs the checker on the
//! result as the operation's postcondition (8.3) and records each entity's origin. [`Made`] is
//! what comes back: the body and its history.
//!
//! Guarantees:
//!
//! - **Names, not positions.** Every index of the body follows its entities' names, and every name
//!   comes from the operation's path, a role and the names it is relative to: never from the
//!   order the rows were written in (8.2, 20.2).
//! - **Checked before returned.** A body that does not pass the checker at the build's level is
//!   never returned; the operation fails with [`MakeDetail::ResultInvalid`], a defect, instead.
//! - **Charged by what it builds.** One meter unit per row built, before it is built (23.10), and
//!   the publishing and the checker charge their own rows to the same meter.
//! - **Transactional.** Nothing the caller holds is changed; a failure returns no body.
//!
//! Method: chapter 8's arenas and name order (8.2), chapter 20's names by role and history
//! records (20.2, 20.3), and the checker's postcondition (8.3)
//! (`projects/engines/geometry/architecture/08-topology.md`,
//! `projects/engines/geometry/architecture/20-naming-history.md`).
//!
//! Cost: linear in the part's rows, plus publishing (a sort by name) and the checker's own cost.
//! Recursion: none.

use alloc::vec::Vec;

use geom_check::{Level, check};
use geom_curve::CurveKind;
use geom_math::Box3;
use geom_num::{Exactness, P3};
use geom_op::history::{History, Origin, Record};
use geom_op::{
    MakeDetail, NameId, NameList, NameSet, OpContext, OpError, OpErrorDetail, OpOutput, OpPath,
    Role, TopoName,
};
use geom_run::{Phase, Stopped, WorkMeter};
use geom_surface::SurfaceKind;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense, Shell, Vertex,
};

/// The phase every Make and Transform operation charges in.
pub(crate) const PHASE: Phase = Phase::new("make");

/// What a Make or Transform operation returns: the body, and the history of its entities.
#[derive(Clone, Debug)]
pub struct Made {
    /// The body, published in name order.
    pub body: Body,
    /// Where each region, face, edge and vertex of the body came from (20.3).
    pub history: History,
}

/// A vertex of a part: its point and its role.
pub(crate) struct PartVertex {
    pub(crate) at: P3,
    pub(crate) role: Role,
}

/// An edge of a part: its two vertices (none for a closed edge), its curve and its role.
pub(crate) struct PartEdge {
    pub(crate) ends: Option<(u32, u32)>,
    pub(crate) curve: CurveKind<3>,
    pub(crate) role: Role,
}

/// A face of a part: its surface and sense, its loops as edges and senses, its box and its role.
pub(crate) struct PartFace {
    pub(crate) surface: SurfaceKind,
    pub(crate) sense: Sense,
    pub(crate) loops: Vec<Vec<(u32, Sense)>>,
    pub(crate) bound: Box3,
    pub(crate) role: Role,
}

/// A solid of one shell, as rows to be named and published.
pub(crate) struct Part {
    pub(crate) vertices: Vec<PartVertex>,
    pub(crate) edges: Vec<PartEdge>,
    pub(crate) faces: Vec<PartFace>,
}

/// The meter of an operation run with `ctx`: its budget and its cancellation.
pub(crate) fn meter<'c>(ctx: &OpContext<'c>) -> WorkMeter<'c> {
    let meter = WorkMeter::new(ctx.budget());
    match ctx.cancel() {
        Some(cancel) => meter.with_cancel(cancel),
        None => meter,
    }
}

/// The error for a stop of `meter`.
pub(crate) fn stopped(ctx: &OpContext<'_>, meter: &WorkMeter<'_>, stop: Stopped) -> OpError {
    OpError::new(
        ctx.path().clone(),
        OpErrorDetail::from(meter.error::<OpErrorDetail>(stop, PHASE)),
    )
}

/// The error for `detail`.
pub(crate) fn fail(ctx: &OpContext<'_>, detail: MakeDetail) -> OpError {
    OpError::new(ctx.path().clone(), detail)
}

/// The name of what operation `op` creates by `role`, from nothing.
pub(crate) fn created(op: &OpPath, role: &Role) -> Option<NameId> {
    TopoName::Created {
        op: op.clone(),
        role: role.clone(),
        from: NameList::new([]),
    }
    .id()
    .ok()
}

/// The role `name` with the names in `args` as its arguments, sorted so the role does not depend
/// on the order they were found in.
pub(crate) fn role_of(name: &str, args: impl IntoIterator<Item = NameId>) -> Option<Role> {
    let mut args: Vec<NameId> = args.into_iter().collect();
    args.sort_unstable();
    Role::new(name, args.into_iter().map(geom_op::RoleArg::Name)).ok()
}

/// The role `name`, with no arguments.
pub(crate) fn plain(name: &str) -> Option<Role> {
    Role::new(name, []).ok()
}

/// The index of row `row`.
fn ix<K>(row: usize) -> Option<Ix<K>> {
    u32::try_from(row).ok().and_then(Ix::new)
}

/// The index of row `row` if there is one: `None` (a defect) only when a row has no index.
#[allow(
    clippy::option_option,
    reason = "the outer option is the defect of a row without an index, the inner one a link that is absent"
)]
fn maybe<K>(row: Option<usize>) -> Option<Option<Ix<K>>> {
    match row {
        Some(r) => ix(r).map(Some),
        None => Some(None),
    }
}

/// Names `part`'s entities by role, publishes it as a solid in name order, checks it, and records
/// its history, for the operation run with `ctx`, recorded `exactness`.
///
/// # Errors
///
/// The meter's stop, as the operation's budget or cancellation error; [`MakeDetail::BuildFailed`]
/// or [`MakeDetail::ResultInvalid`] for a defect.
pub(crate) fn assemble(
    ctx: &OpContext<'_>,
    part: Part,
    exactness: Exactness,
) -> Result<OpOutput<Made>, OpError> {
    let mut meter = meter(ctx);
    let defect = || fail(ctx, MakeDetail::BuildFailed);
    let op = ctx.path();
    let outside = plain("region.outside").ok_or_else(defect)?;
    let solid = plain("region.solid").ok_or_else(defect)?;
    let roles: Vec<(&Role, Kind)> = [(&outside, Kind::Region), (&solid, Kind::Region)]
        .into_iter()
        .chain(part.faces.iter().map(|f| (&f.role, Kind::Face)))
        .chain(part.edges.iter().map(|e| (&e.role, Kind::Edge)))
        .chain(part.vertices.iter().map(|v| (&v.role, Kind::Vertex)))
        .collect();
    let rows = part
        .faces
        .iter()
        .flat_map(|f| &f.loops)
        .try_fold(roles.len(), |n, l| n.checked_add(1)?.checked_add(l.len()))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(defect)?;
    meter
        .charge(rows)
        .map_err(|stop| stopped(ctx, &meter, stop))?;
    let mut names = Names {
        regions: Vec::new(),
        faces: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
    };
    let mut records = Vec::new();
    for (role, kind) in roles {
        let name = created(op, role).ok_or_else(defect)?;
        let key = NameKey(name.to_bytes());
        match kind {
            Kind::Region => names.regions.push(key),
            Kind::Face => names.faces.push(key),
            Kind::Edge => names.edges.push(key),
            Kind::Vertex => names.vertices.push(key),
        }
        records.push(Record {
            output: name,
            origin: Origin::Generated {
                from: NameSet::new([]).map_err(|_| defect())?,
                role: role.clone(),
            },
        });
    }
    let builder = rows_of(part).ok_or_else(defect)?;
    let outside_row = ix::<Region>(0).ok_or_else(defect)?;
    let body = builder
        .publish_named(&names, BodyKind::Solid, outside_row, &mut meter)
        .map_err(|_| defect())?;
    finish(
        ctx,
        &mut meter,
        body,
        records,
        NameSet::new([]).map_err(|_| defect())?,
        exactness,
    )
}

/// Which arena a name belongs to.
#[derive(Clone, Copy)]
enum Kind {
    Region,
    Face,
    Edge,
    Vertex,
}

/// Checks `body` as the postcondition, and returns it with the history of `records` and the
/// `deleted` inputs.
///
/// # Errors
///
/// The meter's stop; [`MakeDetail::ResultInvalid`] if the body does not pass the checker;
/// [`MakeDetail::BuildFailed`] if the history cannot be formed.
pub(crate) fn finish(
    ctx: &OpContext<'_>,
    meter: &mut WorkMeter<'_>,
    body: Body,
    records: Vec<Record>,
    deleted: NameSet,
    exactness: Exactness,
) -> Result<OpOutput<Made>, OpError> {
    let report = check(&body, exactness, Level::of_build(), meter)
        .map_err(|stop| stopped(ctx, meter, stop))?;
    if !report.passed() {
        let first = report.faults().first().map_or_else(
            || alloc::string::String::from("the checker reported faults"),
            alloc::string::ToString::to_string,
        );
        return Err(fail(ctx, MakeDetail::ResultInvalid).with_message(first));
    }
    let history =
        History::new(records, deleted, []).map_err(|_| fail(ctx, MakeDetail::BuildFailed))?;
    Ok(OpOutput::new(Made { body, history }, exactness))
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over the part writes every arena's rows; split, each piece would need the others' counts"
)]
/// The rows of `part`: the void outside (region 0), the solid (region 1), one shell, and the
/// faces, loops, fins, edges and vertices in the part's order, each edge's fins in one radial ring
/// in the order the loops use them.
fn rows_of(part: Part) -> Option<Builder> {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    let shell = ix::<Shell>(0)?;
    let first_face = if part.faces.is_empty() {
        None
    } else {
        Some(ix::<Face>(0)?)
    };
    a.regions
        .extend(&[
            Region {
                kind: RegionKind::Void,
                first_shell: None,
            },
            Region {
                kind: RegionKind::Solid,
                first_shell: Some(shell),
            },
        ])
        .ok()?;
    a.shells
        .extend(&[Shell {
            region: ix(1)?,
            next: None,
            first_face,
        }])
        .ok()?;
    let mut faces = Vec::new();
    let mut loops = Vec::new();
    let mut fins: Vec<Fin> = Vec::new();
    let mut ring: Vec<Vec<usize>> = (0..part.edges.len()).map(|_| Vec::new()).collect();
    let face_count = part.faces.len();
    for (f, face) in part.faces.iter().enumerate() {
        let next_face = maybe(f.checked_add(1).filter(|&n| n < face_count))?;
        let first_loop = if face.loops.is_empty() {
            None
        } else {
            Some(ix(loops.len())?)
        };
        faces.push(Face {
            shell,
            next: next_face,
            surface: ix(f)?,
            sense: face.sense,
            first_loop,
            bound: face.bound,
        });
        let loop_count = face.loops.len();
        for (l, fins_of) in face.loops.iter().enumerate() {
            let here = loops.len();
            let first = fins.len();
            let more = l.checked_add(1).is_some_and(|n| n < loop_count);
            loops.push(Loop {
                face: ix(f)?,
                next: if more {
                    Some(ix(here.checked_add(1)?)?)
                } else {
                    None
                },
                first_fin: Some(ix(first)?),
            });
            let n = fins_of.len();
            for (k, &(e, sense)) in fins_of.iter().enumerate() {
                let at = first.checked_add(k)?;
                let next = first.checked_add(k.checked_add(1)?.checked_rem(n)?)?;
                let prev = first.checked_add(k.checked_add(n)?.checked_sub(1)?.checked_rem(n)?)?;
                let edge = usize::try_from(e).ok()?;
                ring.get_mut(edge)?.push(at);
                fins.push(Fin {
                    loop_: ix(here)?,
                    next: ix(next)?,
                    prev: ix(prev)?,
                    edge: ix(edge)?,
                    sense,
                    radial: ix(at)?,
                    pcurve: None,
                });
            }
        }
    }
    // Each edge's fins in one radial ring, in the order the loops use them.
    for members in &ring {
        for (k, &fin) in members.iter().enumerate() {
            let after = *members.get(k.checked_add(1)?.checked_rem(members.len())?)?;
            fins.get_mut(fin)?.radial = ix(after)?;
        }
    }
    let edges: Vec<Edge> = part
        .edges
        .iter()
        .enumerate()
        .map(|(e, edge)| {
            Some(Edge {
                first_fin: maybe(ring.get(e)?.first().copied())?,
                start: maybe(edge.ends.and_then(|(s, _)| usize::try_from(s).ok()))?,
                end: maybe(edge.ends.and_then(|(_, t)| usize::try_from(t).ok()))?,
                curve: ix(e)?,
            })
        })
        .collect::<Option<_>>()?;
    let vertices: Vec<Vertex> = (0..part.vertices.len())
        .map(|v| Some(Vertex { point: ix(v)? }))
        .collect::<Option<_>>()?;
    a.faces.extend(&faces).ok()?;
    a.loops.extend(&loops).ok()?;
    a.fins.extend(&fins).ok()?;
    a.edges.extend(&edges).ok()?;
    a.vertices.extend(&vertices).ok()?;
    a.surfaces
        .extend(part.faces.into_iter().map(|f| f.surface))
        .ok()?;
    a.curves
        .extend(part.edges.into_iter().map(|e| e.curve))
        .ok()?;
    a.points
        .extend(part.vertices.into_iter().map(|v| v.at))
        .ok()?;
    Some(b)
}
