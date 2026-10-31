//! The boolean's entry: one arrangement, one body per selection (11.1).
//!
//! What it does: [`combine`] prepares its operands, arranges their polygons, and for each selection
//! rebuilds what it keeps and holds the body to the checker. [`unite`], [`subtract`] and
//! [`intersect`] are its three commonest calls. [`COMBINE`] is its schema.
//!
//! Guarantees:
//!
//! - **One arrangement, several results.** The selections share every clip, cut and trace (11.1).
//! - **One list per selection.** Most selections give one body, split one per cell, and each
//!   selection's list stands in its place.
//! - **Inputs are never changed, and nothing is returned unchecked.** A result that fails the
//!   checker is an error, never a body (11.10).
//! - **As exact as the least exact operand.** A flat arrangement adds no error of its own (11.3).
//! - **The same bytes on any threads, and in any order of the operands.** The order is the keys',
//!   and the work is deterministic (23.4).
//!
//! Method: chapter 11.1's selections on 11.3's exact polyhedral path
//! (`projects/engines/geometry/architecture/11-booleans.md`), EMBER (Trettner, Nehring-Wirxel and
//! Kobbelt 2022, <https://doi.org/10.1145/3528223.3530181>).
//!
//! Cost: what [`prepare`], [`arrange`] and [`rebuild`] cost, and the checker per result.

use alloc::vec::Vec;

use geom_check::{Level, check};
use geom_num::ladder::FIT;
use geom_num::{Exactness, NumError, P3, Sign};
use geom_op::{
    Charge, Class, CombineDetail, Diagnostic, DiagnosticSpec, ErrorKind, ErrorSpec, Family, Field,
    OpContext, OpError, OpOutput, OpSchema, RoleSpec, RunDetail, Unit,
};
use geom_poly::convex::Piece;
use geom_poly::merge::{
    Merge, MergeError, Merged, Outward, Rules, as_kept, both_ways, merge_with, section, select,
};
use geom_poly::subdivide::{LEAF_POLYGONS, around, arrange};
use geom_poly::trace::{Classified, TraceError};
use geom_poly::winding::{Winding, WindingTest};
use geom_run::{Exec, Phase, RunError, Stopped, WorkMeter};
use geom_topo::Body;

use crate::operand::{
    BooleanInput, Operand, OperandKey, PrepareError, Prepared, Selection, prepare,
};
use crate::rebuild::{RebuildError, rebuild, rebuild_with, wire};

/// The arrangement's phase.
const ARRANGEMENT: Phase = Phase::new("arrangement");

/// The rebuild's phase.
const REBUILD: Phase = Phase::new("rebuild");

/// The boolean's schema (10.1).
pub const COMBINE: OpSchema = OpSchema {
    name: "combine.boolean",
    version: 1,
    family: Family::Combine,
    doc: "Unites, subtracts or intersects bodies in one arrangement, one result per selection.",
    inputs: &[
        Field {
            name: "operands",
            ty: "[Operand]",
            unit: Unit::None,
            optional: false,
            doc: "The bodies, each with a stable key and a kind.",
        },
        Field {
            name: "select",
            ty: "[Selection]",
            unit: Unit::None,
            optional: false,
            doc: "What to keep: one body comes back for each.",
        },
        Field {
            name: "merge",
            ty: "Merge",
            unit: Unit::None,
            optional: true,
            doc: "Whether flush faces merge, or keep their seams.",
        },
    ],
    outputs: &[Field {
        name: "bodies",
        ty: "[Body]",
        unit: Unit::None,
        optional: false,
        doc: "One body per selection, in order.",
    }],
    roles: &[RoleSpec {
        name: "tool",
        doc: "A face a tool brought into the result.",
    }],
    errors: &[
        ErrorSpec {
            detail: "DuplicateOperand",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "Two operands share a key.",
        },
        ErrorSpec {
            detail: "InvalidOperand",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "An operand cannot enter the arrangement.",
        },
        ErrorSpec {
            detail: "UnboundedResult",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The selection would include the outside.",
        },
    ],
    diagnostics: &[
        DiagnosticSpec {
            code: "bool.cutter_does_not_separate",
            doc: "A split leaves its target whole: nothing cuts it right through.",
        },
        DiagnosticSpec {
            code: "bool.empty_result",
            doc: "A selection keeps nothing.",
        },
    ],
    classes: &[Class::Exact, Class::Certified],
    charges: &[Charge::new(ARRANGEMENT, "polygon a split clips", 1)],
    batch: false,
    example: "a 4 m cube less a 2 m cube at its corner: a cube with a corner notch",
};

/// One arrangement, for the selections to read.
struct Scene<'a> {
    /// The operands, ready.
    ready: &'a Prepared,
    /// Every piece, with its vectors.
    classified: &'a [Classified],
    /// What merges.
    merge: Merge,
}

/// Why a selection gave no body.
#[derive(Clone, Copy)]
enum Refused {
    /// The rebuild refused.
    Rebuild(RebuildError),
    /// The input did.
    Detail(CombineDetail),
}

impl From<RebuildError> for Refused {
    fn from(e: RebuildError) -> Self {
        Refused::Rebuild(e)
    }
}

impl From<MergeError> for Refused {
    fn from(e: MergeError) -> Self {
        Refused::Rebuild(e.into())
    }
}

impl From<NumError> for Refused {
    fn from(e: NumError) -> Self {
        Refused::Rebuild(e.into())
    }
}

/// The operation's error for a failure of the numbers layer.
fn numbers(ctx: &OpContext<'_>, e: NumError) -> OpError {
    OpError::new(ctx.path().clone(), e)
}

/// The operation's error for `detail`.
fn fail(ctx: &OpContext<'_>, detail: CombineDetail) -> OpError {
    OpError::new(ctx.path().clone(), detail)
}

/// `error` as the operation's.
fn prepared(ctx: &OpContext<'_>, error: PrepareError, meter: &WorkMeter<'_>) -> OpError {
    match error {
        PrepareError::Stopped(stopped) => ctx.stopped(meter, stopped, ARRANGEMENT),
        PrepareError::Num(e) => numbers(ctx, e),
        PrepareError::DuplicateOperand(_) => fail(ctx, CombineDetail::DuplicateOperand),
        PrepareError::UnknownOperand(_) => fail(ctx, CombineDetail::UnknownOperand),
        PrepareError::BadTest(_) => fail(ctx, CombineDetail::BadSelection),
        PrepareError::NotPlaneBased(_) => fail(ctx, CombineDetail::NotPlaneBased),
        PrepareError::Broken(_) => fail(ctx, CombineDetail::InvalidOperand),
    }
}

/// `error` as the operation's.
fn rebuilt(ctx: &OpContext<'_>, error: RebuildError, meter: &mut WorkMeter<'_>) -> OpError {
    match error {
        RebuildError::Merge(
            MergeError::Stopped(stopped) | MergeError::Trace(TraceError::Stopped(stopped)),
        ) => ctx.stopped(meter, stopped, REBUILD),
        RebuildError::Merge(MergeError::Inside) => fail(ctx, CombineDetail::UnboundedResult),
        RebuildError::Merge(MergeError::Num(e)) | RebuildError::Num(e) => numbers(ctx, e),
        RebuildError::Topo(e) => match e.stop(meter) {
            Some(stopped) => ctx.stopped(meter, stopped, REBUILD),
            None => fail(ctx, CombineDetail::ResultInvalid),
        },
        _ => fail(ctx, CombineDetail::ResultInvalid),
    }
}

/// `error` as the operation's.
fn refused(ctx: &OpContext<'_>, error: Refused, meter: &mut WorkMeter<'_>) -> OpError {
    match error {
        Refused::Rebuild(e) => rebuilt(ctx, e, meter),
        Refused::Detail(detail) => fail(ctx, detail),
    }
}

/// `error` as the operation's.
fn arranged(ctx: &OpContext<'_>, error: RunError<TraceError>, meter: &WorkMeter<'_>) -> OpError {
    match error {
        RunError::Failed(TraceError::Stopped(stopped)) => ctx.stopped(meter, stopped, ARRANGEMENT),
        RunError::Failed(TraceError::Num(e)) => numbers(ctx, e),
        RunError::Failed(TraceError::Winding(_)) => fail(ctx, CombineDetail::ResultInvalid),
        RunError::BudgetExceeded(exceeded) => {
            OpError::new(ctx.path().clone(), RunDetail::BudgetExceeded(exceeded))
        }
        RunError::Cancelled => OpError::new(ctx.path().clone(), RunDetail::Cancelled),
        RunError::OutOfMemory => OpError::new(ctx.path().clone(), RunDetail::OutOfMemory),
        RunError::Internal(what) => OpError::new(ctx.path().clone(), RunDetail::Internal { what }),
    }
}

/// Whether the caller takes a result of class `would`: anything Exact, and otherwise a class at
/// least as loose and a bound at least its own, `ε_fit` unless the caller said (11.1).
fn accepted(would: Exactness, accept: Option<Exactness>) -> bool {
    match (would.bound(), accept) {
        (None, _) => true,
        (Some((_, step)), None) => would.within(FIT, step).is_ok(),
        (Some(_), Some(accept)) => {
            accept.rank() >= would.rank()
                && accept
                    .bound()
                    .is_some_and(|(most, step)| would.within(most, step).is_ok())
        }
    }
}

impl Scene<'_> {
    /// `pieces`, each knowing the face it came from.
    fn sourced(&self, mut pieces: Vec<Outward>) -> Vec<Outward> {
        for piece in &mut pieces {
            let source = self.ready.sources.get(piece.polygon);
            piece.source = source.copied().unwrap_or(piece.polygon);
        }
        pieces
    }

    /// Whether `polygon` belongs to the operand at `owner`.
    fn owned(&self, polygon: usize, owner: Option<usize>) -> bool {
        self.ready.owners.get(polygon).copied() == owner
    }

    /// Every piece of the operands other than the one at `target`.
    fn others(&self, target: Option<usize>) -> Vec<Piece> {
        let others = self
            .classified
            .iter()
            .filter(|c| !self.owned(c.polygon, target));
        others.map(|c| c.piece.clone()).collect()
    }

    /// What `test` keeps, its seams held where `cuts` lie.
    fn kept(
        &self,
        test: &WindingTest,
        cuts: &[Piece],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Body>, Refused> {
        let kept = self.sourced(select(test, self.classified)?);
        let rules = Rules { cuts, open: false };
        let body = rebuild_with(&kept, &self.ready.carriers, self.merge, rules, meter)?;
        Ok(alloc::vec![body])
    }

    /// The target of `test` cut into one body per cell by the pieces inside it, and whether any
    /// wall stood.
    fn split(
        &self,
        test: &WindingTest,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Vec<Body>, bool), Refused> {
        let outer = self.sourced(select(test, self.classified)?);
        let mut walls: Vec<&Classified> = self
            .classified
            .iter()
            .filter(|c| c.first && test.holds(&c.front) && test.holds(&c.back))
            .collect();
        let (mut pieces, mut merged) = self.cells(&outer, &walls, meter)?;
        // A wall with one cell on both sides separates nothing. Taking those away changes no cell,
        // so a second pass has nothing more to find.
        let base = outer.len();
        let shell_of = |merged: &Merged, at: usize| {
            let face = merged.piece_faces.get(at)?;
            merged.faces.get(*face).map(|f| f.shell)
        };
        let mut standing = Vec::with_capacity(walls.len());
        for (at, wall) in walls.iter().enumerate() {
            let front = base.saturating_add(at.saturating_mul(2));
            let separates = shell_of(&merged, front) != shell_of(&merged, front.saturating_add(1));
            if separates {
                standing.push(*wall);
            }
        }
        if standing.len() != walls.len() {
            walls = standing;
            (pieces, merged) = self.cells(&outer, &walls, meter)?;
        }
        let mut lists: alloc::collections::BTreeMap<usize, Vec<Outward>> =
            alloc::collections::BTreeMap::new();
        for (at, piece) in pieces.into_iter().enumerate() {
            let shell = shell_of(&merged, at).ok_or(RebuildError::Broken)?;
            let held = merged.shells.get(shell).ok_or(RebuildError::Broken)?;
            lists
                .entry(held.holder.unwrap_or(shell))
                .or_default()
                .push(piece);
        }
        let mut bodies = Vec::with_capacity(lists.len());
        for list in lists.values() {
            let carriers = &self.ready.carriers;
            bodies.push(rebuild(list, carriers, Merge::RemoveRedundant, meter)?);
        }
        Ok((bodies, !walls.is_empty()))
    }

    /// `outer` and `walls` merged, each wall twice, back to back, a seam held along it.
    fn cells(
        &self,
        outer: &[Outward],
        walls: &[&Classified],
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Vec<Outward>, Merged), Refused> {
        let mut pieces = outer.to_vec();
        for wall in walls {
            pieces.extend(self.sourced(both_ways(wall)?.to_vec()));
        }
        let cuts: Vec<Piece> = walls.iter().map(|wall| wall.piece.clone()).collect();
        let rules = Rules {
            cuts: &cuts,
            open: false,
        };
        let merged = merge_with(&pieces, Merge::RemoveRedundant, rules, meter)?;
        Ok((pieces, merged))
    }

    /// The pieces of the sheet at `target`, either side of each passing `test`.
    fn keep(
        &self,
        test: &WindingTest,
        target: Option<usize>,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Body>, Refused> {
        let pieces: Vec<Outward> = self
            .classified
            .iter()
            .filter(|c| self.owned(c.polygon, target))
            .filter(|c| test.holds(&c.front) || test.holds(&c.back))
            .map(as_kept)
            .collect();
        let pieces = self.sourced(pieces);
        let rules = Rules {
            cuts: &[],
            open: true,
        };
        let body = rebuild_with(&pieces, &self.ready.carriers, self.merge, rules, meter)?;
        Ok(alloc::vec![body])
    }

    /// The pieces of the sheet at `target`, cut by the others, that hold `seeds`.
    fn trim(
        &self,
        target: Option<usize>,
        seeds: &[P3],
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<Body>, Refused> {
        let of_target = self
            .classified
            .iter()
            .filter(|c| self.owned(c.polygon, target));
        let pieces = self.sourced(of_target.map(as_kept).collect());
        let cuts = self.others(target);
        let rules = Rules {
            cuts: &cuts,
            open: true,
        };
        let merged = merge_with(&pieces, self.merge, rules, meter)?;
        let mut chosen: Vec<usize> = Vec::with_capacity(seeds.len());
        for seed in seeds {
            let mut faces: Vec<usize> = Vec::new();
            for (piece, face) in pieces.iter().zip(&merged.piece_faces) {
                if holds(&piece.piece, *seed)? {
                    faces.push(*face);
                }
            }
            faces.sort_unstable();
            faces.dedup();
            match faces.as_slice() {
                [] => return Err(Refused::Detail(CombineDetail::SeedOutside)),
                [face] => chosen.push(*face),
                _ => return Err(Refused::Detail(CombineDetail::SeedOnBoundary)),
            }
        }
        let kept: Vec<Outward> = pieces
            .iter()
            .zip(&merged.piece_faces)
            .filter(|(_, face)| chosen.contains(face))
            .map(|(piece, _)| piece.clone())
            .collect();
        let rules = Rules {
            cuts: &[],
            open: true,
        };
        let body = rebuild_with(&kept, &self.ready.carriers, self.merge, rules, meter)?;
        Ok(alloc::vec![body])
    }

    /// The curves where operands meet.
    fn section(&self, meter: &mut WorkMeter<'_>) -> Result<Vec<Body>, Refused> {
        let mut pieces = Vec::with_capacity(self.classified.len());
        for c in self.classified {
            let owner = self.ready.owners.get(c.polygon);
            pieces.push((c.piece.clone(), owner.copied().ok_or(RebuildError::Broken)?));
        }
        let curves = section(&pieces, meter)?;
        Ok(alloc::vec![wire(&curves, meter)?])
    }
}

/// Whether `seed` lies on `piece`, its boundary included.
fn holds(piece: &Piece, seed: P3) -> Result<bool, NumError> {
    if piece.face.side(seed)? != Sign::Zero {
        return Ok(false);
    }
    for bound in &piece.bounds {
        if bound.side(seed)? == Sign::Negative {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One list of bodies for each of `input`'s selections.
///
/// # Errors
///
/// An [`OpError`]: operands that cannot enter, a selection that includes the outside, a stopped
/// meter, or a result the checker refuses, which is a defect.
pub fn combine(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    input: &BooleanInput<'_>,
) -> Result<OpOutput<Vec<Vec<Body>>>, OpError> {
    let mut meter = ctx.meter();
    let ready = prepare(input, &mut meter).map_err(|e| prepared(ctx, e, &meter))?;
    let mut exactness = Exactness::Exact;
    for operand in &input.operands {
        exactness = exactness
            .combine(operand.exactness)
            .ok_or_else(|| numbers(ctx, NumError::BudgetExceeded))?;
    }
    if !accepted(exactness, input.accept) {
        return Err(fail(ctx, CombineDetail::NotAccepted { would: exactness }));
    }
    let cell = around(&ready.polygons).map_err(|e| numbers(ctx, e))?;
    let classified = arrange(
        exec,
        cell,
        Winding::new(),
        &ready.polygons,
        LEAF_POLYGONS,
        &ready.tests,
        &mut meter,
    )
    .map_err(|e| arranged(ctx, e, &meter))?;
    let scene = Scene {
        ready: &ready,
        classified: &classified,
        merge: input.merge,
    };
    let mut lists = Vec::with_capacity(ready.tests.len());
    let mut notes = Vec::new();
    let jobs = input.select.iter().zip(&ready.tests).zip(&ready.targets);
    for ((selection, test), target) in jobs {
        let found = match selection {
            Selection::Imprint { .. } => scene.kept(test, &scene.others(*target), &mut meter),
            Selection::Split { .. } => scene.split(test, &mut meter).map(|(bodies, walls)| {
                if !walls {
                    notes.extend(Diagnostic::new(
                        "bool.cutter_does_not_separate",
                        "nothing cuts the target right through",
                    ));
                }
                bodies
            }),
            Selection::Keep { .. } => scene.keep(test, *target, &mut meter),
            Selection::Trim { seeds, .. } => scene.trim(*target, seeds, &mut meter),
            Selection::Section => scene.section(&mut meter),
            Selection::Unite
            | Selection::Subtract { .. }
            | Selection::Intersect
            | Selection::Where(_) => scene.kept(test, &[], &mut meter),
        };
        let bodies = found.map_err(|e| refused(ctx, e, &mut meter))?;
        for body in &bodies {
            let report = check(body, exactness, Level::Cheap, &mut meter)
                .map_err(|stopped: Stopped| ctx.stopped(&meter, stopped, REBUILD))?;
            if !report.passed() {
                return Err(fail(ctx, CombineDetail::ResultInvalid));
            }
        }
        if bodies.iter().all(|body| body.arenas().edges.is_empty()) {
            notes.extend(Diagnostic::new(
                "bool.empty_result",
                "the selection keeps nothing",
            ));
        }
        lists.push(bodies);
    }
    let mut out = OpOutput::new(lists, exactness);
    out.diagnostics = notes;
    Ok(out)
}

/// The one body of `operands` under `selection`.
fn one(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
    selection: Selection,
) -> Result<OpOutput<Body>, OpError> {
    let input = BooleanInput {
        operands: operands.to_vec(),
        select: alloc::vec![selection],
        merge: Merge::RemoveRedundant,
        accept: None,
    };
    let OpOutput {
        mut result,
        exactness,
        diagnostics,
        ..
    } = combine(ctx, exec, &input)?;
    let body = result
        .pop()
        .and_then(|mut list| list.pop())
        .ok_or_else(|| fail(ctx, CombineDetail::ResultInvalid))?;
    let mut out = OpOutput::new(body, exactness);
    out.diagnostics = diagnostics;
    Ok(out)
}

/// Every cell inside any of `operands`.
///
/// # Errors
///
/// As [`combine`].
pub fn unite(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Unite)
}

/// Every cell inside `target` and outside every other operand, however many tools there are, in
/// one arrangement (11.6).
///
/// # Errors
///
/// As [`combine`].
pub fn subtract(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    target: OperandKey,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Subtract { target })
}

/// Every cell inside all of `operands`.
///
/// # Errors
///
/// As [`combine`].
pub fn intersect(
    ctx: &OpContext<'_>,
    exec: &Exec<'_>,
    operands: &[Operand<'_>],
) -> Result<OpOutput<Body>, OpError> {
    one(ctx, exec, operands, Selection::Intersect)
}
