//! The one join between Spare Construct and whatever builds its shapes. Nothing above [`Geometry`]
//! knows what a shape is made of: an element holds a [`Recipe`], and everything drawn, stored or
//! compared comes back as a [`Built`]. Two implementations are in `construct-regen`: a stand-in
//! that makes boxes, and a builder over the geometry kernel's engine seam, whose [`Edition`] carries
//! the engine's own [`EngineReport`]. Swapping them changes one line where a program picks its
//! builder, not the data layout, the edit log or the app; the server's `tests/swap.rs` proves that.
//!
//! This follows the kernel's own contract (architecture chapters 10, 20 and 24):
//!
//! - A recipe is the kernel's envelope: who owns it, the operation and its version, and an opaque
//!   payload. A shape is never the source of truth: it is rebuilt from the recipe, and named by
//!   the content ID of its canonical bytes.
//! - Shapes are per body, and the bytes above this line are opaque. Display meshes are records of
//!   their own, one per level of detail, so a large body can go to file storage, linked by hash.
//! - Building reads no clock and no randomness. The same recipe on the same edition gives the same
//!   bytes on every machine. The server and the browser both build through this trait, and the
//!   orderer's checkpoints compare content IDs, so a difference is caught from the first day.

use kit_ops::sha256_hex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// What built a document's shapes, recorded per document (24.4). Everyone editing one document
/// runs this edition, and moving to another is an ordered operation that reports what changed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Edition {
    /// Which implementation built them, such as the stand-in or the kernel.
    pub engine: String,
    /// That implementation's version.
    pub version: u32,
    /// The kernel's `NameScheme`: how faces and edges are named.
    pub name_scheme: u32,
    /// The geometry engine's own report of itself, when a kernel engine builds the shapes; none
    /// for a builder with no engine behind it. Left out of the bytes when there is none, so an
    /// edition written before this field reads, and writes, exactly as it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<EngineReport>,
}

/// What a geometry engine says it is (the kernel's engine seam, `Engine::edition`), as a document
/// records it: every value as the engine gave it, numbers as text so the record compares exactly.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EngineReport {
    /// The engine's own name.
    pub engine: String,
    /// Its version: for an engine taken from elsewhere, the source and the commit it is pinned to.
    pub version: String,
    /// How many of the engine's grid steps make one millimetre.
    pub steps_per_mm: u64,
    /// The tolerances it works to.
    pub tolerances: Vec<EngineTolerance>,
    /// Which part of it answers each operation, for each case.
    pub answers: Vec<EngineAnswer>,
}

/// One tolerance an engine works to.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EngineTolerance {
    /// The engine's own name for it.
    pub name: String,
    /// Its value in `unit`, written as the shortest decimal that reads back to the engine's number.
    pub value: String,
    /// The unit of `value`.
    pub unit: String,
}

/// Which part of an engine answers one operation, for one case of its inputs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EngineAnswer {
    /// The operation.
    pub operation: String,
    /// The inputs this answer is for, in the engine's words.
    pub case: String,
    /// What answers it, in the engine's words.
    pub by: String,
}

/// How to build a shape: the kernel's recipe envelope (24.4). An element's input, never its result.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Who owns the operation: a namespace, as `kit.geom` will be for the kernel's.
    pub owner: String,
    /// The owner's schema version.
    pub schema: u32,
    /// The operation, by the name Spare Construct knows it: `wall.straight`, not how it is made.
    pub op: String,
    /// The operation's version, so an old recipe still builds as it did.
    pub op_version: u32,
    /// The payload. Its meaning is the operation's. Numbers in it are whole, in millimetres, so a
    /// hash never depends on how a float prints. Sorted by key, so the bytes are canonical.
    pub args: BTreeMap<String, Value>,
}

impl Recipe {
    /// Spare Construct's own operations, under this owner and schema.
    #[must_use]
    pub fn construct(op: &str, args: BTreeMap<String, Value>) -> Self {
        Self {
            owner: "construct".to_owned(),
            schema: 1,
            op: op.to_owned(),
            op_version: 1,
            args,
        }
    }
}

/// A box around a shape, in whole millimetres, in the shape's own frame.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// The lowest x, y and z.
    pub min: [i64; 3],
    /// The highest x, y and z.
    pub max: [i64; 3],
}

/// A persistent name for part of a body. Above this interface it is an opaque value: a hosted
/// element refers to a face by its name, never by its position in a list (chapter 20).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FaceName {
    /// The `NameId`: stays with the face through regeneration.
    pub name: String,
    /// What the operation calls it, such as `exterior`.
    pub role: String,
}

/// A triangle mesh in millimetres: `positions` holds x, y, z in turn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Mesh {
    /// The vertices: x, y and z of each in turn.
    pub positions: Vec<i64>,
    /// The triangles: three vertex numbers each.
    pub indices: Vec<u32>,
}

/// One level of detail of a body, for drawing. The kernel keeps these beside the body (24.4).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Display {
    /// 0 is the finest.
    pub lod: u32,
    /// How far this level strays from the body, in millimetres.
    pub deviation: i64,
    /// The triangles to draw.
    pub mesh: Mesh,
    /// The shape's outline seen from above, as the plan draws it.
    pub outline: Vec<[i64; 2]>,
}

impl Display {
    /// The record's canonical bytes.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        // A struct of whole numbers can't fail to serialise.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The content ID of those bytes.
    #[must_use]
    pub fn id(&self) -> String {
        sha256_hex(&self.bytes())
    }
}

/// What building a recipe gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The body's canonical bytes. Opaque: nothing above this interface reads inside.
    pub body: Vec<u8>,
    /// A box around the body.
    pub bounds: Bounds,
    /// The persistent names of the body's faces.
    pub names: Vec<FaceName>,
    /// Finest first.
    pub displays: Vec<Display>,
}

impl Built {
    /// The body's content ID: SHA-256 of its canonical bytes.
    #[must_use]
    pub fn body_id(&self) -> String {
        sha256_hex(&self.body)
    }
}

/// Why a recipe didn't build. It names what went wrong, as the kernel's typed errors do, and it is
/// model state, not a crash: the same on every machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryError {
    /// A short code that names what went wrong, for programs.
    pub code: String,
    /// What went wrong, in words for people.
    pub message: String,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GeometryError {}

/// Builds shapes. The only way shapes come into being, on the server and in the browser.
pub trait Geometry: Send + Sync + 'static {
    /// What this implementation is. A document records it, and only this edition edits it.
    fn edition(&self) -> Edition;

    /// Builds a recipe. A pure function: no clock, no randomness, no state.
    ///
    /// # Errors
    ///
    /// A [`GeometryError`] when the recipe can't be built, the same on every machine.
    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Edition {
        Edition {
            engine: "construct.stand-in".to_owned(),
            version: 1,
            name_scheme: 0,
            report: None,
        }
    }

    /// An edition with no engine report is written exactly as before the report existed, and an
    /// edition written then reads back the same: "we never stop reading it" (FORMAT.md).
    #[test]
    fn an_edition_without_a_report_keeps_its_old_bytes() {
        let old = br#"{"engine":"construct.stand-in","version":1,"name_scheme":0}"#;
        assert_eq!(serde_json::to_vec(&plain()).unwrap(), old);
        assert_eq!(serde_json::from_slice::<Edition>(old).unwrap(), plain());
    }

    /// An engine's report is part of the edition: two editions that differ only in the engine's
    /// pin or a tolerance are different editions, and a report reads back as it was written.
    #[test]
    fn an_engines_report_is_part_of_its_edition() {
        let report = EngineReport {
            engine: "an engine".to_owned(),
            version: "pinned at one commit".to_owned(),
            steps_per_mm: 6400,
            tolerances: vec![EngineTolerance {
                name: "zone".to_owned(),
                value: "0.00001".to_owned(),
                unit: "m".to_owned(),
            }],
            answers: vec![EngineAnswer {
                operation: "extrude".to_owned(),
                case: "every case".to_owned(),
                by: "its sweep".to_owned(),
            }],
        };
        let with = Edition {
            report: Some(report.clone()),
            ..plain()
        };
        let bytes = serde_json::to_vec(&with).unwrap();
        assert_eq!(serde_json::from_slice::<Edition>(&bytes).unwrap(), with);
        assert_ne!(with, plain());
        let mut moved = report;
        moved.version = "pinned at another commit".to_owned();
        assert_ne!(
            with,
            Edition {
                report: Some(moved),
                ..plain()
            }
        );
    }
}
