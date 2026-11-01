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
//! what comes back: the body, its history and, from a transform, which one it was.
//!
//! Guarantees:
//!
//! - **Names, not positions.** Every index of the body follows its entities' names, and every name
//!   comes from the operation's path, a role and the names it is relative to: never from the
//!   order the rows were written in (8.2, 20.2).
//! - **Checked before returned.** A body that does not pass the checker at the build's level is
//!   never returned; the operation fails with [`MakeDetail::ResultInvalid`], a defect, instead.
//! - **Charged by what it builds.** One meter unit ([`PER_ROW`]) and 64 logical bytes per row built,
//!   before it is built (23.10), and the publishing charges its own rows to the same meter. The
//!   postcondition check runs on a meter of its own, cancellable and without a limit: its level
//!   differs between a test build and a shipped one, and a budget must stop an operation the same way
//!   in both.
//! - **Transactional.** Nothing the caller holds is changed; a failure, or a stop the operation
//!   did not report, returns no body (`OpContext::run`).
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
    Charge, MakeDetail, NameId, NameList, NameSet, OpContext, OpError, OpOutput, OpPath, Role,
    TopoName,
};
use geom_run::{LogicalSize, Phase, WorkMeter};
use geom_surface::SurfaceKind;
use geom_topo::order::{NameKey, Names};
use geom_topo::{
    Body, BodyKind, Builder, Edge, Face, Fin, Ix, Loop, Region, RegionKind, Sense, Shell,
    TopoError, Vertex,
};

/// The phase every Make and Transform operation charges in.
pub const PHASE: Phase = Phase::new("make");

/// The line of each Make and Transform operation's charge schedule (23.10): one unit per row
/// built. Publishing and the checker charge their own rows to the same meter.
pub const PER_ROW: Charge = Charge::new(PHASE, "row built", 1);

/// Which transform a body went through, for the attribute event `Transformed` (8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// A translation.
    Move,
    /// A rotation.
    Rotate,
    /// A mirror.
    Mirror,
    /// A scale.
    Scale,
}

/// What a Make or Transform operation returns: the body, and the history of its entities.
#[derive(Clone, Debug)]
pub struct Made {
    /// The body, published in name order.
    pub body: Body,
    /// Where each region, face, edge and vertex of the body came from (20.3).
    pub history: History,
    /// Which transform it went through: `None` for a primitive and for a pattern's copy, whose
    /// role `copy` is its attribute event (8.4).
    pub motion: Option<Motion>,
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

/// The error for `detail`.
pub(crate) fn fail(ctx: &OpContext<'_>, detail: MakeDetail) -> OpError {
    OpError::new(ctx.path().clone(), detail)
}

/// The error for rows that did not reach a body: the meter's stop, an allocation refused, or a
/// defect.
pub(crate) fn unbuilt(ctx: &OpContext<'_>, meter: &mut WorkMeter<'_>, e: TopoError) -> OpError {
    match e.stop(meter) {
        Some(stop) => ctx.stopped(meter, stop, PHASE),
        None => fail(ctx, MakeDetail::BuildFailed),
    }
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
    ctx.run(|meter| built(ctx, meter, part, exactness))
}

/// One row of a body, for the logical-memory budget (23.10): the size of the widest record any
/// arena holds, fixed by the operation's version, whatever a row takes on this machine.
struct Row;

impl LogicalSize for Row {
    const LOGICAL_BYTES: u64 = 64;
}

/// Charges `rows` rows to `meter`: their work, then their logical memory, before they are built.
pub(crate) fn charge_rows(
    ctx: &OpContext<'_>,
    meter: &mut WorkMeter<'_>,
    rows: u64,
) -> Result<(), OpError> {
    PER_ROW
        .charge(meter, rows)
        .map_err(|stop| ctx.stopped(meter, stop, PHASE))?;
    meter
        .reserve_for::<Row>(rows)
        .map_err(|stop| ctx.stopped(meter, stop, PHASE))
}

/// [`assemble`]'s body, on the operation's meter.
fn built(
    ctx: &OpContext<'_>,
    meter: &mut WorkMeter<'_>,
    part: Part,
    exactness: Exactness,
) -> Result<OpOutput<Made>, OpError> {
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
    charge_rows(ctx, meter, rows)?;
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
    let rows = rows_of(&part).ok_or_else(defect)?;
    let outside_row = ix::<Region>(0).ok_or_else(defect)?;
    let body = filled(&rows, part)
        .and_then(|b| b.publish_named(&names, BodyKind::Solid, outside_row, meter))
        .map_err(|e| unbuilt(ctx, meter, e))?;
    finish(ctx, body, records, None, exactness)
}

/// A solid's topology, as rows.
struct Rows {
    regions: [Region; 2],
    shells: [Shell; 1],
    faces: Vec<Face>,
    loops: Vec<Loop>,
    fins: Vec<Fin>,
    edges: Vec<Edge>,
    vertices: Vec<Vertex>,
}

/// Which arena a name belongs to.
#[derive(Clone, Copy)]
enum Kind {
    Region,
    Face,
    Edge,
    Vertex,
}

/// Checks `body` as the postcondition, and returns it with the history of `records` (no input is
/// deleted: a Make operation has none, and a transform keeps every entity).
///
/// # Errors
///
/// `Cancelled`; [`MakeDetail::ResultInvalid`] if the body does not pass the checker;
/// [`MakeDetail::BuildFailed`] if the history cannot be formed.
pub(crate) fn finish(
    ctx: &OpContext<'_>,
    body: Body,
    records: Vec<Record>,
    motion: Option<Motion>,
    exactness: Exactness,
) -> Result<OpOutput<Made>, OpError> {
    // The check's level differs by build, so its work stays off the operation's budget.
    let mut checking = ctx.unbudgeted_meter();
    let report = check(&body, exactness, Level::of_build(), &mut checking)
        .map_err(|stop| ctx.stopped(&checking, stop, PHASE))?;
    if !report.passed() {
        let first = report.faults().first().map_or_else(
            || alloc::string::String::from("the checker reported faults"),
            alloc::string::ToString::to_string,
        );
        return Err(fail(ctx, MakeDetail::ResultInvalid).with_message(first));
    }
    let defect = || fail(ctx, MakeDetail::BuildFailed);
    let none = NameSet::new([]).map_err(|_| defect())?;
    let history = History::new(records, none, []).map_err(|_| defect())?;
    let made = Made {
        body,
        history,
        motion,
    };
    Ok(OpOutput::new(made, exactness))
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over the part writes every arena's rows; split, each piece would need the others' counts"
)]
/// The rows of `part`: the void outside (region 0), the solid (region 1), one shell, and the
/// faces, loops, fins, edges and vertices in the part's order, each edge's fins in one radial ring
/// in the order the loops use them. Lists are left unlinked: publishing in name order rebuilds
/// every list from its members' owners.
fn rows_of(part: &Part) -> Option<Rows> {
    let shell = ix::<Shell>(0)?;
    let regions = [
        Region {
            kind: RegionKind::Void,
            first_shell: None,
        },
        Region {
            kind: RegionKind::Solid,
            first_shell: None,
        },
    ];
    let shells = [Shell {
        region: ix(1)?,
        next: None,
        first_face: None,
    }];
    let mut faces = Vec::new();
    let mut loops = Vec::new();
    let mut fins: Vec<Fin> = Vec::new();
    let mut ring: Vec<Vec<usize>> = (0..part.edges.len()).map(|_| Vec::new()).collect();
    for (f, face) in part.faces.iter().enumerate() {
        faces.push(Face {
            shell,
            next: None,
            surface: ix(f)?,
            sense: face.sense,
            first_loop: None,
            bound: face.bound,
        });
        for fins_of in &face.loops {
            let here = loops.len();
            let first = fins.len();
            loops.push(Loop {
                face: ix(f)?,
                next: None,
                first_fin: None,
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
                first_fin: None,
                start: maybe(edge.ends.and_then(|(s, _)| usize::try_from(s).ok()))?,
                end: maybe(edge.ends.and_then(|(_, t)| usize::try_from(t).ok()))?,
                curve: ix(e)?,
            })
        })
        .collect::<Option<_>>()?;
    let vertices: Vec<Vertex> = (0..part.vertices.len())
        .map(|v| Some(Vertex { point: ix(v)? }))
        .collect::<Option<_>>()?;
    Some(Rows {
        regions,
        shells,
        faces,
        loops,
        fins,
        edges,
        vertices,
    })
}

/// The arenas of `rows` and `part`'s geometry.
fn filled(rows: &Rows, part: Part) -> Result<Builder, TopoError> {
    let mut b = Builder::new();
    let a = &mut b.arenas;
    a.regions.extend(&rows.regions)?;
    a.shells.extend(&rows.shells)?;
    a.faces.extend(&rows.faces)?;
    a.loops.extend(&rows.loops)?;
    a.fins.extend(&rows.fins)?;
    a.edges.extend(&rows.edges)?;
    a.vertices.extend(&rows.vertices)?;
    a.surfaces
        .extend(part.faces.into_iter().map(|f| f.surface))?;
    a.curves.extend(part.edges.into_iter().map(|e| e.curve))?;
    a.points.extend(part.vertices.into_iter().map(|v| v.at))?;
    Ok(b)
}
