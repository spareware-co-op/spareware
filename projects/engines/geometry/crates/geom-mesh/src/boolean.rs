//! Mesh booleans (19.5): the arrangement's pieces a selection keeps, as one exact mesh per test.
//!
//! What it does: [`boolean`] hands the faces of closed meshes and their planes to `geom-poly`'s
//! arrangement, and for each [`WindingTest`] keeps the pieces its indicator keeps, merges them into
//! maximal faces with `geom-poly`'s merge, and builds an [`ExactMesh`] with the history of its runs.
//! Cutting 300 openings is one call, and several results share one arrangement. [`soup`] stops
//! after the selection, with the kept pieces as a polygon soup, the stage EMBER times.
//!
//! Guarantees:
//!
//! - **Closed inputs only.** The arrangement needs a whole winding number everywhere, so an open
//!   mesh is [`MeshError::NotClosed`], whose remedy is healing. Self-intersections are fine.
//! - **A result is a solid** ([`ExactMesh::is_solid`]): its winding is its test's, 0 or 1, and no
//!   two of its faces cross. A solid operand's boxes away from the others are solved without
//!   cuts or tracing (11.3 step 4): fewer pieces, the same vectors, the same mesh.
//! - **Bounded results only.** A test the outside passes, such as "not A", has no finite boundary:
//!   [`MeshError::UnboundedResult`] names which test, and the remedy is to intersect it with a box.
//! - **Conforming and exact.** The merge gives maximal faces and edges that end where other edges
//!   do, so no T-junction is left. Nothing is rounded, however often a mesh is cut: a corner off the
//!   grid stays three planes. Each result passes the mesh checker on the way out.
//! - **The same mesh however it was reached.** The boxes, the threads and the order of the faces do
//!   not show. A corner off the grid takes the lowest three planes around it, not the three that
//!   happened to cut a piece.
//! - **Run-level history.** Each output run is unchanged, modified, split or merged, with deleted
//!   runs listed (20.3). A split run's pieces are named by the runs that cut them and the side, as
//!   `Split { piece: ByTools { tools, side } }`, and pieces those leave the same take weak ordinals
//!   in the order of their faces (20.2). Faces inside a run have no names of their own, so
//!   references should target runs.
//!
//! Method: 19.5 and 11.3 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`,
//! `11-booleans.md`): EMBER's plane-based arrangement (Trettner, Nehring-Wirxel and Kobbelt 2022,
//! <https://doi.org/10.1145/3528223.3530181>), and hypermesh's many operands with a small expression
//! over their winding numbers, rewritten from 19.5's description.
//!
//! Cost: the arrangement's, in `geom-poly`, then the merge's O(n log n) exact comparisons for n
//! sides of kept pieces, for each test, and the mesh's build. G148's budgets, timed apart: the
//! soup within 3 times EMBER's 1.6 ms on `Thingi10K` pairs, and the merge and build within 0.5 µs
//! for each kept piece and each result face, two of G146's builds, whose done-when is a million
//! faces built and checked in 0.5 s (`projects/kit/notes/G148-stage-budgets.md`).

#![allow(
    clippy::many_single_char_names,
    reason = "planes, faces and the two ends of an edge, as chapter 19 names them"
)]

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use geom_num::Sign;
use geom_num::plane::{Plane, Tpi};
use geom_op::{
    History, NameId, NameSet, OpPath, Ordinal, Origin, PieceKey, Record, Side, TopoName,
};
use geom_poly::merge::{Merge, Merged, Outward, Rules, merge_on, select};
use geom_poly::order::normals_det;
use geom_poly::plane::canonical;
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange_with};
use geom_poly::trace::{Classified, Polygon, TraceError};
use geom_poly::winding::{Entry, Winding, WindingTest};
use geom_run::{Exec, RunError, Stop, WorkMeter};

use crate::classify::{pieces_of, trace_error};
use crate::exact::build::{Corner, Polygons, Source, build};
use crate::exact::{ExactMesh, MeshError, MeshFault, MeshKind, PropTable, Role, at, index};

/// One result.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The mesh.
    pub mesh: ExactMesh,
    /// What became of the inputs' runs.
    pub history: History,
}

/// A run of an input.
struct Input {
    name: NameId,
    role: Role,
    flags: u32,
    operand: u32,
    faces: u32,
}

/// The inputs as the arrangement takes them.
struct Gathered {
    polygons: Vec<Polygon>,
    /// The run of each polygon.
    run_of: Vec<u32>,
    inputs: Vec<Input>,
}

/// Every face of `operands`, split into convex polygons.
fn gather(operands: &[&ExactMesh], meter: &mut WorkMeter<'_>) -> Result<Gathered, MeshError> {
    let mut gathered = Gathered {
        polygons: Vec::new(),
        run_of: Vec::new(),
        inputs: Vec::new(),
    };
    for (&mesh, operand) in operands.iter().zip(0_u32..) {
        if mesh.kind() == MeshKind::Open {
            return Err(MeshError::NotClosed);
        }
        let parts = mesh.parts();
        for run in &parts.runs {
            let number = index(gathered.inputs.len())?;
            gathered.inputs.push(Input {
                name: run.source,
                role: run.role,
                flags: run.flags,
                operand,
                faces: run.faces.end.saturating_sub(run.faces.start),
            });
            for face in run.faces.clone() {
                for piece in pieces_of(parts, face, meter)? {
                    gathered.polygons.push(Polygon {
                        piece,
                        entry: Some(operand),
                    });
                    gathered.run_of.push(number);
                }
            }
        }
    }
    Ok(gathered)
}

/// The root of `at` in a disjoint-set forest, halving the path.
fn root(parents: &mut [u32], mut at: u32) -> u32 {
    loop {
        let up = parents.get(at as usize).copied().unwrap_or(at);
        if up == at {
            return at;
        }
        let top = parents.get(up as usize).copied().unwrap_or(up);
        if let Some(slot) = parents.get_mut(at as usize) {
            *slot = top;
        }
        at = top;
    }
}

/// Joins the sets of `a` and `b`, the lower root on top.
fn join(parents: &mut [u32], a: u32, b: u32) {
    let (a, b) = (root(parents, a), root(parents, b));
    if let Some(slot) = parents.get_mut(a.max(b) as usize) {
        *slot = a.min(b);
    }
}

/// Where each output face came from.
struct Traced {
    /// The input runs in each face.
    runs: Vec<BTreeSet<u32>>,
    /// Whether any piece of the face was turned over.
    turned: Vec<bool>,
    /// A piece of each face.
    piece: Vec<Option<usize>>,
}

/// The runs in each of `merged`'s faces, from the kept pieces.
fn trace(merged: &Merged, kept: &[Outward], run_of: &[u32]) -> Result<Traced, MeshFault> {
    let faces = merged.faces.len();
    let mut traced = Traced {
        runs: alloc::vec![BTreeSet::new(); faces],
        turned: alloc::vec![false; faces],
        piece: alloc::vec![None; faces],
    };
    for (p, (piece, &face)) in kept.iter().zip(&merged.piece_face).enumerate() {
        let run = run_of.get(piece.polygon).ok_or(MeshFault::Shape)?;
        let held = traced.runs.get_mut(face).ok_or(MeshFault::Shape)?;
        held.insert(*run);
        if piece.turned {
            *traced.turned.get_mut(face).ok_or(MeshFault::Shape)? = true;
        }
        traced
            .piece
            .get_mut(face)
            .ok_or(MeshFault::Shape)?
            .get_or_insert(p);
    }
    Ok(traced)
}

/// A piece of a run: faces joined by edges, all of that run alone.
#[derive(Default)]
struct Patch {
    faces: Vec<u32>,
    /// The runs across its edges.
    beyond: BTreeSet<u32>,
    /// Whether any of its faces was turned over.
    turned: bool,
}

/// The patches of each run, by run, in the order of their lowest faces.
fn patches(merged: &Merged, traced: &Traced) -> Result<BTreeMap<u32, Vec<Patch>>, MeshError> {
    let count = index(merged.faces.len())?;
    let mut forest: Vec<u32> = (0..count).collect();
    let alone = |f: usize| -> Option<u32> {
        match traced.runs.get(f) {
            Some(runs) if runs.len() == 1 => runs.first().copied(),
            _ => None,
        }
    };
    let face_of = |fin: usize| merged.fins.get(fin).map(|x| x.face);
    let mut crossings: Vec<(usize, usize)> = Vec::new();
    for edge in &merged.edges {
        let after = edge.fins.iter().cycle().skip(1);
        for (&x, &y) in edge.fins.iter().zip(after) {
            let (a, b) = (
                face_of(x).ok_or(MeshFault::Shape)?,
                face_of(y).ok_or(MeshFault::Shape)?,
            );
            if alone(a).is_some() && alone(a) == alone(b) {
                join(&mut forest, index(a)?, index(b)?);
            }
            crossings.push((a, b));
        }
    }
    let mut roots: BTreeMap<u32, Patch> = BTreeMap::new();
    for f in 0..count {
        let top = root(&mut forest, f);
        if alone(f as usize).is_some() {
            let patch = roots.entry(top).or_default();
            patch.faces.push(f);
            patch.turned |= traced.turned.get(f as usize) == Some(&true);
        }
    }
    for (a, b) in crossings {
        let top = root(&mut forest, index(a)?);
        if root(&mut forest, index(b)?) != top {
            let across = traced.runs.get(b).ok_or(MeshFault::Shape)?;
            if let Some(patch) = roots.get_mut(&top) {
                patch.beyond.extend(across.iter().copied());
            }
        }
    }
    let mut out: BTreeMap<u32, Vec<Patch>> = BTreeMap::new();
    for patch in roots.into_values() {
        let first = patch.faces.first().ok_or(MeshFault::Shape)?;
        let run = alone(*first as usize).ok_or(MeshFault::Shape)?;
        out.entry(run).or_default().push(patch);
    }
    Ok(out)
}

/// The names of `runs`.
fn names(inputs: &[Input], runs: impl IntoIterator<Item = u32>) -> Result<NameSet, MeshError> {
    let mut list = Vec::new();
    for run in runs {
        list.push(at(inputs, run)?.name);
    }
    NameSet::new(list).map_err(|_| MeshError::Defect)
}

/// The output runs and records, built up.
#[derive(Default)]
struct Ledger {
    records: Vec<Record>,
    runs: Vec<Source>,
    /// The output run of each output face.
    face_run: BTreeMap<u32, u32>,
}

impl Ledger {
    /// One output run of `faces`, named `output`, with `input`'s role and flags.
    fn add(
        &mut self,
        faces: &[u32],
        output: NameId,
        origin: Origin,
        input: &Input,
    ) -> Result<(), MeshError> {
        let number = index(self.runs.len())?;
        self.runs.push(Source {
            name: output,
            role: input.role,
            flags: input.flags,
        });
        self.records.push(Record { output, origin });
        for &f in faces {
            self.face_run.insert(f, number);
        }
        Ok(())
    }
}

/// Which side of its tools a kept piece of `patch` lies on: behind them if any holds it.
fn side_of(
    patch: &Patch,
    traced: &Traced,
    kept: &[Outward],
    gathered: &Gathered,
) -> Result<Side, MeshError> {
    let first = patch.faces.first().ok_or(MeshFault::Shape)?;
    let piece = at(&traced.piece, *first)?.ok_or(MeshFault::Shape)?;
    let piece = kept.get(piece).ok_or(MeshFault::Shape)?;
    for &tool in &patch.beyond {
        let operand = at(&gathered.inputs, tool)?.operand;
        if piece.back.count(operand) > 0 {
            return Ok(Side::Back);
        }
    }
    Ok(Side::Front)
}

/// The split pieces of a run: each named by what cut it and the side, weak ordinals telling apart
/// those that are still the same.
fn split_records(
    list: &[Patch],
    run: u32,
    (traced, kept, gathered): (&Traced, &[Outward], &Gathered),
    op: &OpPath,
    ledger: &mut Ledger,
) -> Result<(), MeshError> {
    let input = at(&gathered.inputs, run)?;
    let mut named: Vec<(NameId, PieceKey, &Patch)> = Vec::with_capacity(list.len());
    for patch in list {
        let tools = patch.beyond.iter().copied().filter(|r| *r != run);
        let piece = PieceKey::ByTools {
            tools: names(&gathered.inputs, tools)?,
            side: side_of(patch, traced, kept, gathered)?,
        };
        let name = TopoName::Split {
            op: op.clone(),
            of: input.name,
            piece: piece.clone(),
        };
        named.push((name.id().map_err(|_| MeshError::Defect)?, piece, patch));
    }
    for (place, (base, piece, patch)) in named.iter().enumerate() {
        let same: Vec<usize> = (0..named.len())
            .filter(|k| named.get(*k).is_some_and(|(b, _, _)| b == base))
            .collect();
        let mut output = *base;
        if same.len() > 1 {
            let n = same
                .iter()
                .position(|k| *k == place)
                .ok_or(MeshError::Defect)?;
            let ord = Ordinal::new(index(n)?, index(same.len())?).map_err(|_| MeshError::Defect)?;
            output = TopoName::Weak { base: *base, ord }
                .id()
                .map_err(|_| MeshError::Defect)?;
        }
        let origin = Origin::Split {
            of: input.name,
            piece: piece.clone(),
        };
        ledger.add(&patch.faces, output, origin, input)?;
    }
    Ok(())
}

/// What became of the input runs, and the output run of each face.
fn account(
    merged: &Merged,
    kept: &[Outward],
    gathered: &Gathered,
    op: &OpPath,
) -> Result<(History, Vec<u32>, Vec<Source>), MeshError> {
    let traced = trace(merged, kept, &gathered.run_of)?;
    let patched = patches(merged, &traced)?;
    let mut ledger = Ledger::default();
    for (&run, list) in &patched {
        let input = at(&gathered.inputs, run)?;
        match list.as_slice() {
            [patch] => {
                let other = |r: &u32| {
                    gathered
                        .inputs
                        .get(*r as usize)
                        .is_some_and(|i| i.operand != input.operand)
                };
                let same = index(patch.faces.len())? == input.faces
                    && !patch.turned
                    && !patch.beyond.iter().any(other);
                let origin = if same {
                    Origin::Unchanged
                } else {
                    Origin::Modified {
                        from: names(&gathered.inputs, [run])?,
                    }
                };
                ledger.add(&patch.faces, input.name, origin, input)?;
            }
            _ => split_records(list, run, (&traced, kept, gathered), op, &mut ledger)?,
        }
    }
    let mut merging: BTreeMap<Vec<u32>, Vec<u32>> = BTreeMap::new();
    for (runs, f) in traced.runs.iter().zip(0_u32..) {
        if runs.len() > 1 {
            merging
                .entry(runs.iter().copied().collect())
                .or_default()
                .push(f);
        }
    }
    let mut used: BTreeSet<u32> = patched.keys().copied().collect();
    for (runs, faces) in &merging {
        used.extend(runs.iter().copied());
        let of = names(&gathered.inputs, runs.iter().copied())?;
        let name = TopoName::Merged {
            op: op.clone(),
            of: of.clone(),
        };
        let first = runs.first().ok_or(MeshFault::Shape)?;
        let output = name.id().map_err(|_| MeshError::Defect)?;
        let input = at(&gathered.inputs, *first)?;
        ledger.add(faces, output, Origin::Merged { of }, input)?;
    }
    let gone = (0..index(gathered.inputs.len())?).filter(|r| !used.contains(r));
    let deleted = names(&gathered.inputs, gone)?;
    let mut face_run = Vec::with_capacity(merged.faces.len());
    for f in 0..index(merged.faces.len())? {
        face_run.push(*ledger.face_run.get(&f).ok_or(MeshFault::Shape)?);
    }
    let history =
        History::new(ledger.records, deleted, Vec::new()).map_err(|_| MeshError::Defect)?;
    Ok((history, face_run, ledger.runs))
}

/// The lowest three of the planes through a point that meet only there, or `own`'s.
fn lowest_triple(mut planes: Vec<Plane>, own: &Tpi) -> Result<[Plane; 3], MeshError> {
    let mut reduced = Vec::with_capacity(planes.len());
    for plane in planes.drain(..) {
        let c = canonical(&plane)?;
        reduced.push((c.plane.n(), c.plane.d()));
    }
    reduced.sort_unstable();
    reduced.dedup();
    let mut rows = Vec::with_capacity(reduced.len());
    for (n, d) in reduced {
        rows.push(Plane::new(n, d)?);
    }
    for (i, a) in rows.iter().enumerate() {
        for (j, b) in rows.iter().enumerate().skip(i.saturating_add(1)) {
            for c in rows.iter().skip(j.saturating_add(1)) {
                if normals_det(a, b, c)? != Sign::Zero {
                    return Ok([*a, *b, *c]);
                }
            }
        }
    }
    Ok(*own.planes())
}

/// The corners of `merged`: a grid point where a vertex is one, its lowest three planes where not.
fn corners_of(merged: &Merged) -> Result<Vec<Corner>, MeshError> {
    let mut near: Vec<Vec<Plane>> = alloc::vec![Vec::new(); merged.vertices.len()];
    for fin in &merged.fins {
        let face = merged.faces.get(fin.face).ok_or(MeshFault::Shape)?;
        let sheet = merged.sheets.get(face.sheet).ok_or(MeshFault::Shape)?;
        for v in [fin.from, fin.to] {
            let list = near.get_mut(v).ok_or(MeshFault::Shape)?;
            list.push(sheet.face);
            list.push(fin.bound);
        }
    }
    let mut out = Vec::with_capacity(merged.vertices.len());
    for (t, planes) in merged.vertices.iter().zip(near) {
        let rounded = t.to_grid()?;
        out.push(if rounded.is_exact() {
            Corner::Grid(rounded.value())
        } else {
            Corner::Planes(lowest_triple(planes, t)?)
        });
    }
    Ok(out)
}

/// `merged` as polygons of the runs given. Vertices are in lexicographic order, so the ring with a
/// face's lowest vertex is its outer ring, and it comes first.
fn polygons_of(
    merged: &Merged,
    face_run: Vec<u32>,
    runs: Vec<Source>,
) -> Result<Polygons, MeshError> {
    let vertices = corners_of(merged)?;
    let on_grid = |v: usize| matches!(vertices.get(v), Some(Corner::Grid(_)));
    let mut corners = Vec::new();
    let mut loop_start = alloc::vec![0_u32];
    let mut face_loops = alloc::vec![0_u32];
    let mut face_plane = Vec::with_capacity(merged.faces.len());
    let mut edge_plane = Vec::new();
    for face in &merged.faces {
        let sheet = merged.sheets.get(face.sheet).ok_or(MeshFault::Shape)?;
        face_plane.push(Some(sheet.face));
        let lowest = |ring: &Vec<usize>| {
            ring.iter()
                .filter_map(|f| merged.fins.get(*f))
                .map(|f| f.from)
                .min()
        };
        let mut rings: Vec<&Vec<usize>> = face.rings.iter().collect();
        rings.sort_by_key(|r| lowest(r));
        for ring in rings {
            for &number in ring {
                let fin = merged.fins.get(number).ok_or(MeshFault::Shape)?;
                corners.push(index(fin.from)?);
                let grid = on_grid(fin.from) && on_grid(fin.to);
                edge_plane.push(if grid { None } else { Some(fin.bound) });
            }
            loop_start.push(index(corners.len())?);
        }
        face_loops.push(index(loop_start.len().saturating_sub(1))?);
    }
    if !edge_plane.iter().any(Option::is_some) {
        edge_plane.clear();
    }
    Ok(Polygons {
        vertices,
        corners,
        loop_start,
        face_loops,
        face_run,
        runs,
        face_plane,
        edge_plane,
        corner_prop: Vec::new(),
        props: PropTable::default(),
        face_dev: Vec::new(),
        class: geom_num::Exactness::Exact,
    })
}

/// The result of `test`.
fn outcome(
    gathered: &Gathered,
    classified: &[Classified],
    test: &WindingTest,
    (op, exec): (&OpPath, &Exec<'_>),
    meter: &mut WorkMeter<'_>,
) -> Result<Outcome, MeshError> {
    let kept = select(test, classified)?;
    let merged = merge_on(
        exec,
        &kept,
        (Merge::RemoveRedundant, Rules::default()),
        meter,
    )?;
    let (history, face_run, runs) = account(&merged, &kept, gathered, op)?;
    let polygons = polygons_of(&merged, face_run, runs)?;
    Ok(Outcome {
        // A result's winding is its test's, 0 or 1, and no two of its faces cross.
        mesh: build(&polygons, exec, meter)?.proven_solid(),
        history,
    })
}

/// Every piece of `operands` for `tests`, from one arrangement.
fn arranged(
    (gathered, solids): (&Gathered, &[Entry]),
    tests: &[WindingTest],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Classified>, MeshError> {
    if gathered.polygons.is_empty() {
        return Ok(Vec::new());
    }
    let cell = around(&gathered.polygons)?;
    arrange_with(
        exec,
        (cell, Winding::new()),
        &gathered.polygons,
        (LEAF_POLYGONS, tests),
        solids,
        meter,
    )
    .map_err(run_error)
}

/// What passes `test` as one run, `source`, with no history: for a result whose operands are
/// steps of one operation, whose names mean nothing outside it.
pub(crate) fn boolean_into(
    operands: &[&ExactMesh],
    test: &WindingTest,
    source: Source,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactMesh, MeshError> {
    let (_, classified) = classified(operands, core::slice::from_ref(test), exec, meter)?;
    let kept = select(test, &classified)?;
    let merged = merge_on(
        exec,
        &kept,
        (Merge::RemoveRedundant, Rules::default()),
        meter,
    )?;
    let face_run = alloc::vec![0; merged.faces.len()];
    let polygons = polygons_of(&merged, face_run, alloc::vec![source])?;
    // A boolean's result: its winding is its test's, 0 or 1, and no two of its faces cross.
    Ok(build(&polygons, exec, meter)?.proven_solid())
}

/// The arrangement's errors as a mesh's.
fn run_error(e: RunError<TraceError>) -> MeshError {
    match e {
        RunError::Failed(e) => trace_error(e),
        RunError::BudgetExceeded(exceeded) => MeshError::BudgetExceeded(exceeded),
        RunError::Cancelled => MeshError::Stopped(Stop::Cancelled),
        RunError::OutOfMemory => MeshError::OutOfMemory,
        RunError::Internal(_) => MeshError::Defect,
    }
}

/// One result for each of `tests`, over one arrangement of `operands`. The result of a test is what
/// passes it: entry `n` of a [`Winding`] is `operands[n]`. `op` names this step in the names of new
/// runs.
///
/// # Errors
///
/// [`MeshError::NotClosed`] for an operand with boundary edges, to heal first;
/// [`MeshError::UnboundedResult`] for a test the outside passes, to intersect with a box first; and
/// a stopped meter.
pub fn boolean(
    operands: &[&ExactMesh],
    tests: &[WindingTest],
    op: &OpPath,
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Outcome>, MeshError> {
    let (gathered, classified) = classified(operands, tests, exec, meter)?;
    let mut out = Vec::with_capacity(tests.len());
    for test in tests {
        out.push(outcome(&gathered, &classified, test, (op, exec), meter)?);
    }
    Ok(out)
}

/// The operands gathered, and every piece of one arrangement for `tests`.
fn classified(
    operands: &[&ExactMesh],
    tests: &[WindingTest],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<(Gathered, Vec<Classified>), MeshError> {
    let outside = Winding::new();
    if let Some(test) = tests.iter().position(|t| t.holds(&outside)) {
        return Err(MeshError::UnboundedResult { test: index(test)? });
    }
    let gathered = gather(operands, meter)?;
    // A solid operand's leaves away from the others need no cuts and no tracing (11.3 step 4).
    let solids: Vec<Entry> = (0..)
        .zip(operands)
        .filter(|(_, mesh)| mesh.is_solid())
        .map(|(entry, _)| entry)
        .collect();
    let classified = arranged((&gathered, &solids), tests, exec, meter)?;
    Ok((gathered, classified))
}

/// [`boolean`]'s first stage alone: for each of `tests`, the pieces it keeps, each a convex part
/// of an input face turned to face out of the result, with nothing merged and no mesh built. This
/// is the output EMBER gives, a polygon soup whose every polygon is part of an input polygon, so
/// timing it against EMBER's 1.6 ms compares like with like (G148's stage budgets,
/// `projects/kit/notes/G148-stage-budgets.md`). [`boolean`] merges the same pieces into maximal
/// faces and builds each result.
///
/// # Errors
///
/// As [`boolean`], short of the merge's and the mesh build's.
pub fn soup(
    operands: &[&ExactMesh],
    tests: &[WindingTest],
    exec: &Exec<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<Vec<Outward>>, MeshError> {
    let (_, classified) = classified(operands, tests, exec, meter)?;
    let mut out = Vec::with_capacity(tests.len());
    for test in tests {
        out.push(select(test, &classified)?);
    }
    Ok(out)
}
