//! Import and export reports as data: what a file declared, what the importer or exporter chose,
//! and every rounding, healing action, unsupported item and failed item, in the order they
//! happened.
//!
//! What it does: [`ImportReport`] is chapter 24.8's step 7: the source's SHA-256, the format, the
//! units and where each came from, the frame, the declared precision, the healing tolerance
//! proposed and used ([`HealTolerance`]), each body's class, Tolerant flag, bounds and import
//! deviation ([`BodyReport`]), and every rounding ([`Rounding`]), healing action ([`HealAction`]),
//! unsupported entity ([`Unsupported`]) and failed item ([`Failed`]). [`ExportReport`] is chapter
//! 24.10's: the target format, its units, the bound stated in the file, every value that did not
//! write exactly ([`Rounding`] again, from the written decimal back), and whether the caller
//! accepted an invalid rounding. A format crate fills a report as it goes, through `record_*`
//! methods that refuse rather than fail silently when memory runs out; the report is plain data,
//! compared with `==`, and the same file and options give the same report.
//!
//! **The healing tolerance.** Chapter 24.8: the default is 640 steps; the importer proposes the
//! file's declared precision instead, raised to 640 steps when smaller and capped at 6,400; the
//! proposal applies only when the caller accepts it, and any value from one step to the cap may be
//! used. [`HealTolerance::propose`] computes the proposal from the declared precision in the file's
//! unit, rounded once to steps ([`crate::grid::length`]); [`HealTolerance::with_used`] records the
//! value used, refusing one outside one step to the cap.
//!
//! Guarantees:
//!
//! - **Everything recorded, in order.** Each `record_*` appends; nothing is merged, sorted or
//!   dropped, so the report lists events in the order the reader met them, which is the file's
//!   order for a deterministic reader.
//! - **One failed element is one failed item.** A failed item is an entry, never the end of the
//!   report.
//! - **Refused, not lost.** An append the allocator cannot make is
//!   [`ExchangeError::OutOfMemory`], and the report is left as it was.
//! - **The healing tolerance stays in its range.** A proposal is within 640 to 6,400 steps; a used
//!   value outside one step to 6,400 is refused with [`ExchangeError::OutOfRange`].
//!
//! Method: chapter 24.8's import report (step 7, and `ε_heal`) and chapter 24.10's export
//! (`projects/engines/geometry/architecture/24-file-format-exchange.md`), with the tolerance ladder
//! of `geom_num::ladder` (`HEAL_DEFAULT`, `HEAL_CAP`; chapter 6.5,
//! `projects/engines/geometry/architecture/06-numbers.md`).
//!
//! Cost: each record is an amortised constant-time append; [`HealTolerance::propose`] is one
//! numbers-to-grid conversion. A report holds its entries and the text of their keys, nothing more.

use alloc::string::String;
use alloc::vec::Vec;

use geom_math::{Box3, Decimal, FrameRecord, LinearUnit};
use geom_num::grid::GridStep;
use geom_num::ladder::{HEAL_CAP, HEAL_DEFAULT};
use geom_num::{Exactness, Steps};

use crate::error::ExchangeError;
use crate::grid::length;
use crate::unit::AngleUnit;
use crate::write::Deviation;

/// A file format of chapter 24.11.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Format {
    /// IFC (4.3, `IFC4`, `IFC2x3`).
    Ifc,
    /// STEP (AP242, AP214, AP203).
    Step,
    /// DXF.
    Dxf,
    /// glTF 2.0.
    Gltf,
    /// Wavefront OBJ.
    Obj,
    /// STL.
    Stl,
    /// PLY.
    Ply,
    /// 3MF.
    ThreeMf,
    /// `LandXML` 1.2.
    LandXml,
    /// E57.
    E57,
    /// LAS (and LAZ).
    Las,
    /// PTS.
    Pts,
    /// 3dm.
    ThreeDm,
}

/// Where a unit came from: the file, or the caller, for a format that declares none (STL, OBJ,
/// PLY, PTS, XYZ, DXF with `$INSUNITS` 0); the importer never guesses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitSource {
    /// The file declared it.
    Declared,
    /// The file declared none, and the caller named it.
    Named,
}

/// The units a file was read or written in, and where each came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Units {
    /// The length unit.
    pub length: LinearUnit,
    /// Where the length unit came from.
    pub length_source: UnitSource,
    /// The plane angle unit.
    pub angle: AngleUnit,
    /// Where the angle unit came from.
    pub angle_source: UnitSource,
}

/// How a report names an item of the source: chapter 24.8's persistent key where the format has
/// one, else its instance number.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKey {
    /// A persistent identifier: a STEP persistent ID, an IFC `GlobalId` with the item's path, a 3dm
    /// object id, as the format writes it.
    Persistent(String),
    /// An instance number (`#12` in Part 21), marked weak: it may not survive a revision.
    Instance(u64),
}

/// A number as the file gave it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Given {
    /// Decimal text, exactly.
    Decimal(Decimal),
    /// A binary double, at its exact value.
    Double(f64),
}

/// One number that moved to the grid, or from it, by a rounding.
#[derive(Clone, Debug, PartialEq)]
pub struct Rounding {
    /// The item it belongs to.
    pub item: SourceKey,
    /// Which of the item's numbers, as the format names it (`Coordinates[2]`, `Radius`).
    pub attribute: String,
    /// The number in the file: as read on import, as written on export.
    pub given: Given,
    /// The grid count it stands for, in steps (or angle units for an angle).
    pub count: i64,
    /// The file's number, in steps, less the count, when known exactly (a decimal); `None` for a
    /// double, whose exact difference the importer may not have formed.
    pub deviation: Option<Deviation>,
}

/// What a healing step of chapter 24.9 did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HealKind {
    /// Step 0: the sender's 3D curve was bounded against both surfaces and became the certified
    /// primary curve.
    SenderCurveCertified,
    /// Step 1: a transversal edge was replaced by the certified intersection branch.
    TransversalEdge,
    /// Step 2: a tangent or near-tangent edge became the projection of the sender's curve.
    TangentEdge,
    /// Step 3: a vertex became the certified meeting of its edges.
    Vertex,
    /// Step 4: free edges were sewn.
    Sewn,
    /// Step 4: a group was not sewn because its match was not unique.
    Ambiguous,
    /// Step 5: an edge shorter than one step collapsed.
    ShortEdgeCollapsed,
    /// A face that failed to build was left out (the topology-first build).
    FaceLeftOut,
    /// Rounded polyhedron vertices landed on one grid point and merged.
    VerticesMerged,
    /// A face no longer flat after rounding was split.
    FaceSplit,
    /// A collapsed edge or face was removed after rounding.
    Collapsed,
}

/// One healing or rounding action on an item.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HealAction {
    /// The item it changed.
    pub item: SourceKey,
    /// What it did.
    pub kind: HealKind,
}

/// An entity of the file that no mapping covers (chapter 24.8's `Unsupported`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unsupported {
    /// The entity's type, as the file names it (`IFCFIXEDREFERENCESWEPTAREASOLID`).
    pub entity: String,
    /// The item.
    pub item: SourceKey,
    /// What the caller can do about it.
    pub remedy: String,
}

/// An element that failed: one failed item, never a failed file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Failed {
    /// The item.
    pub item: SourceKey,
    /// Why, in words.
    pub reason: String,
}

/// What became of one body.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyReport {
    /// The body's item.
    pub item: SourceKey,
    /// Its exactness class.
    pub class: Exactness,
    /// Whether it is still Tolerant after healing.
    pub tolerant: bool,
    /// Its bounding box on the grid.
    pub bounds: Box3,
    /// The import deviation: how far its stored numbers moved from the file's, the largest over
    /// its numbers, in steps (recorded apart from its class).
    pub deviation: Deviation,
}

/// The healing tolerance: what the importer proposed, and what was used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HealTolerance {
    proposed: Steps,
    used: Steps,
}

impl HealTolerance {
    /// The proposal from the file's declared precision in `unit` (IFC's `Precision`, STEP's
    /// uncertainty), rounded once to steps of `step` and kept within 640 to 6,400 steps; 640 when
    /// the file declares none. The value used starts as the default, 640 steps: the proposal
    /// applies only when the caller accepts it ([`HealTolerance::with_used`]).
    ///
    /// # Errors
    ///
    /// [`ExchangeError::NotPositive`] for a declared precision that is zero or negative; a
    /// precision beyond the grid's range is taken as the cap.
    pub fn propose(
        declared: Option<Decimal>,
        unit: LinearUnit,
        step: GridStep,
    ) -> Result<HealTolerance, ExchangeError> {
        let proposed = match declared {
            None => HEAL_DEFAULT,
            Some(d) if d.significand() <= 0 => return Err(ExchangeError::NotPositive),
            Some(d) => match length(d, unit, step) {
                Ok(g) => {
                    let steps = g.count().unsigned_abs();
                    Steps(steps.clamp(HEAL_DEFAULT.0, HEAL_CAP.0))
                }
                Err(ExchangeError::OutOfRange { .. }) => HEAL_CAP,
                Err(e) => return Err(e),
            },
        };
        Ok(HealTolerance {
            proposed,
            used: HEAL_DEFAULT,
        })
    }

    /// The same proposal with `used` as the value used: the proposal when the caller accepts it,
    /// or any value from one step to the cap.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfRange`] for zero or a value past 6,400 steps.
    pub fn with_used(self, used: Steps) -> Result<HealTolerance, ExchangeError> {
        if used.0 == 0 || used.0 > HEAL_CAP.0 {
            return Err(ExchangeError::OutOfRange {
                count: i128::from(used.0),
            });
        }
        Ok(HealTolerance { used, ..self })
    }

    /// The proposal.
    #[must_use]
    pub const fn proposed(self) -> Steps {
        self.proposed
    }

    /// The value used.
    #[must_use]
    pub const fn used(self) -> Steps {
        self.used
    }
}

/// Appends to a list, or refuses without changing it.
fn push<T>(list: &mut Vec<T>, value: T) -> Result<(), ExchangeError> {
    list.try_reserve(1)
        .map_err(|_| ExchangeError::OutOfMemory)?;
    list.push(value);
    Ok(())
}

/// Chapter 24.8's import report.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportReport {
    /// The SHA-256 of the source's bytes, as the caller computed it.
    pub source_sha256: [u8; 32],
    /// The format read.
    pub format: Format,
    /// The units, and where each came from.
    pub units: Units,
    /// The frame the model was placed in.
    pub frame: FrameRecord,
    /// The precision the file declares, in its length unit, as written.
    pub declared_precision: Option<Decimal>,
    /// The healing tolerance proposed and used.
    pub heal: HealTolerance,
    bodies: Vec<BodyReport>,
    roundings: Vec<Rounding>,
    healing: Vec<HealAction>,
    unsupported: Vec<Unsupported>,
    failed: Vec<Failed>,
}

impl ImportReport {
    /// An empty report of an import.
    #[must_use]
    pub const fn new(
        source_sha256: [u8; 32],
        format: Format,
        units: Units,
        frame: FrameRecord,
        declared_precision: Option<Decimal>,
        heal: HealTolerance,
    ) -> ImportReport {
        ImportReport {
            source_sha256,
            format,
            units,
            frame,
            declared_precision,
            heal,
            bodies: Vec::new(),
            roundings: Vec::new(),
            healing: Vec::new(),
            unsupported: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Records a body.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_body(&mut self, body: BodyReport) -> Result<(), ExchangeError> {
        push(&mut self.bodies, body)
    }

    /// Records a rounding.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_rounding(&mut self, rounding: Rounding) -> Result<(), ExchangeError> {
        push(&mut self.roundings, rounding)
    }

    /// Records a healing action.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_healing(&mut self, action: HealAction) -> Result<(), ExchangeError> {
        push(&mut self.healing, action)
    }

    /// Records an unsupported entity.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_unsupported(&mut self, item: Unsupported) -> Result<(), ExchangeError> {
        push(&mut self.unsupported, item)
    }

    /// Records a failed item.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_failed(&mut self, item: Failed) -> Result<(), ExchangeError> {
        push(&mut self.failed, item)
    }

    /// The bodies, in the order recorded.
    #[must_use]
    pub fn bodies(&self) -> &[BodyReport] {
        &self.bodies
    }

    /// The roundings, in the order recorded.
    #[must_use]
    pub fn roundings(&self) -> &[Rounding] {
        &self.roundings
    }

    /// The healing actions, in the order recorded.
    #[must_use]
    pub fn healing(&self) -> &[HealAction] {
        &self.healing
    }

    /// The unsupported entities, in the order recorded.
    #[must_use]
    pub fn unsupported(&self) -> &[Unsupported] {
        &self.unsupported
    }

    /// The failed items, in the order recorded.
    #[must_use]
    pub fn failed(&self) -> &[Failed] {
        &self.failed
    }
}

/// Chapter 24.10's export report.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportReport {
    /// The format written.
    pub format: Format,
    /// The units written in.
    pub units: Units,
    /// The frame the model was written from.
    pub frame: FrameRecord,
    /// The bound stated in the file (IFC `Precision`, STEP uncertainty), in its length unit.
    pub stated_bound: Option<Decimal>,
    /// Whether the caller accepted an invalid rounding (`accept_invalid_rounding`), which the file's
    /// metadata also records.
    pub accepted_invalid_rounding: bool,
    roundings: Vec<Rounding>,
    failed: Vec<Failed>,
}

impl ExportReport {
    /// An empty report of an export.
    #[must_use]
    pub const fn new(
        format: Format,
        units: Units,
        frame: FrameRecord,
        stated_bound: Option<Decimal>,
        accepted_invalid_rounding: bool,
    ) -> ExportReport {
        ExportReport {
            format,
            units,
            frame,
            stated_bound,
            accepted_invalid_rounding,
            roundings: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Records a value that did not write exactly.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_rounding(&mut self, rounding: Rounding) -> Result<(), ExchangeError> {
        push(&mut self.roundings, rounding)
    }

    /// Records an item that could not be written.
    ///
    /// # Errors
    ///
    /// [`ExchangeError::OutOfMemory`], with the report unchanged.
    pub fn record_failed(&mut self, item: Failed) -> Result<(), ExchangeError> {
        push(&mut self.failed, item)
    }

    /// The roundings, in the order recorded.
    #[must_use]
    pub fn roundings(&self) -> &[Rounding] {
        &self.roundings
    }

    /// The failed items, in the order recorded.
    #[must_use]
    pub fn failed(&self) -> &[Failed] {
        &self.failed
    }
}
