//! The recipe envelope: how a shape was made, kept for whoever owns the recipe, so a file opens and
//! draws where that owner is absent.
//!
//! What it does: a [`Recipe`] is chapter 24.4's envelope every reader knows: the owner's namespace
//! (`kit.geom` for kernel operations, `kit.geom.ifc`, a plugin's ID), the owner's schema version,
//! the operation's path and version, its inputs by content ID and `NameId`, an opaque payload only
//! the owner decodes, and the content IDs of the bodies it produced. [`recipe_record`] writes one
//! as a required record (tag [`RECIPE_TAG`]); [`read_recipe`] reads it back. [`Recipe::owner_in`]
//! finds the owner that can re-run it, or refuses with `RecipeOwnerMissing`.
//!
//! Guarantees:
//!
//! - **The payload is never decoded here** and is kept byte for byte: a reader without the owner
//!   still has the evaluated bodies, by content ID, and writes the recipe back unchanged.
//! - **Only re-running needs the owner**: [`Recipe::owner_in`] refuses with
//!   [`RecipeError::RecipeOwnerMissing`], naming the owner and its schema, and nothing else does.
//! - **One encoding**: a recipe is read and encoded again, and the bytes must be the stored ones.
//!
//! Method: chapter 24.4's recipe envelope and 26.1's promise that a file opens without the plugin
//! that made a shape (`projects/engines/geometry/architecture/24-file-format-exchange.md`,
//! `projects/engines/geometry/architecture/26-how-it-grows.md`).
//!
//! Cost: linear in the recipe; the payload is copied once.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use geom_op::cbor::{EncodeError, Encoder};
use geom_op::name::encode_op_path;
use geom_op::{NameError, NameId, OpPath};
use geom_run::WorkMeter;

use crate::cbor::Item;
use crate::content::ContentId;
use crate::names::{NamesError, read_op_path};
use crate::payload::{PayloadError, Reader};
use crate::record::{REQUIRED, Record};

/// The recipe record's tag (REGISTRY.md).
pub const RECIPE_TAG: u64 = 18;

/// The recipe record's schema.
pub const RECIPE_SCHEMA: u64 = 1;

/// The owner namespace of the kernel's own operations.
pub const KERNEL_OWNER: &str = "kit.geom";

/// One input of a recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeInput {
    /// A body, by its content ID.
    Body(ContentId),
    /// An entity, by its persistent name.
    Entity(NameId),
}

/// How a shape was made, in the envelope every reader knows (24.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The owner's namespace: printable ASCII, such as `kit.geom` or a plugin's ID.
    pub owner: String,
    /// The owner's schema version for `payload`.
    pub owner_schema: u64,
    /// The operation's path.
    pub op: OpPath,
    /// The operation's version (26.2).
    pub version: u64,
    /// The inputs, in the order the owner gave them.
    pub inputs: Vec<RecipeInput>,
    /// The owner's own encoding of the recipe, never decoded here.
    pub payload: Vec<u8>,
    /// The content IDs of the bodies the recipe produced, in the owner's order.
    pub outputs: Vec<ContentId>,
}

/// Something that can re-run recipes of one owner: the facade for `kit.geom`, `geom-ifc`, a plugin.
pub trait RecipeOwner {
    /// The owner's namespace.
    fn owner(&self) -> &str;
    /// Whether it reads payloads of `schema`.
    fn reads(&self, schema: u64) -> bool;
}

/// Why a recipe cannot be read, written or re-run.
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeError {
    /// The payload's shape or encoding is wrong.
    Payload(PayloadError),
    /// The path inside is wrong.
    Names(NamesError),
    /// An owner namespace that is empty or not printable ASCII.
    Owner,
    /// No owner present reads this recipe's schema, so it cannot be re-run (24.4). Its bodies
    /// still open and its payload is kept.
    RecipeOwnerMissing {
        /// The owner's namespace.
        owner: String,
        /// The owner's schema the recipe needs.
        schema: u64,
    },
    /// The payload is not the recipe's one canonical encoding.
    NotCanonical,
    /// The encoder failed; only a defect can cause it.
    Encode(EncodeError),
}

impl fmt::Display for RecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecipeError::Payload(e) => write!(f, "not a recipe: {e}"),
            RecipeError::Names(e) => write!(f, "not a recipe: {e}"),
            RecipeError::Owner => f.write_str("a recipe owner must be non-empty printable ASCII"),
            RecipeError::RecipeOwnerMissing { owner, schema } => write!(
                f,
                "no owner present re-runs `{owner}` recipes of schema {schema}; its bodies open \
                 and its payload is kept"
            ),
            RecipeError::NotCanonical => f.write_str("the recipe is not in its canonical encoding"),
            RecipeError::Encode(e) => write!(f, "the recipe could not be encoded: {e}"),
        }
    }
}

impl core::error::Error for RecipeError {}

impl From<PayloadError> for RecipeError {
    fn from(e: PayloadError) -> Self {
        RecipeError::Payload(e)
    }
}

impl From<NamesError> for RecipeError {
    fn from(e: NamesError) -> Self {
        RecipeError::Names(e)
    }
}

impl From<EncodeError> for RecipeError {
    fn from(e: EncodeError) -> Self {
        RecipeError::Encode(e)
    }
}

impl From<NameError> for RecipeError {
    fn from(e: NameError) -> Self {
        RecipeError::Names(NamesError::Name(e))
    }
}

/// Whether `owner` is a namespace: non-empty printable ASCII.
fn valid_owner(owner: &str) -> bool {
    !owner.is_empty() && owner.bytes().all(|b| matches!(b, b'!'..=b'~'))
}

impl Recipe {
    /// The owner among `owners` that re-runs this recipe: its namespace, reading its schema.
    ///
    /// # Errors
    ///
    /// [`RecipeError::RecipeOwnerMissing`] when none does.
    pub fn owner_in<'a>(
        &self,
        owners: &[&'a dyn RecipeOwner],
    ) -> Result<&'a dyn RecipeOwner, RecipeError> {
        owners
            .iter()
            .copied()
            .find(|o| o.owner() == self.owner && o.reads(self.owner_schema))
            .ok_or_else(|| RecipeError::RecipeOwnerMissing {
                owner: self.owner.clone(),
                schema: self.owner_schema,
            })
    }

    /// The recipe's canonical encoding: `[owner, owner-schema, op, version, inputs, payload,
    /// outputs]`, an input `[0, content-id]` for a body or `[1, name-id]` for an entity.
    ///
    /// # Errors
    ///
    /// [`RecipeError::Owner`] for an owner that is not a namespace; otherwise only a defect.
    pub fn encode(&self) -> Result<Vec<u8>, RecipeError> {
        if !valid_owner(&self.owner) {
            return Err(RecipeError::Owner);
        }
        let count = |n: usize| u64::try_from(n).map_err(|_| EncodeError::TooLong);
        let mut e = Encoder::new();
        e.array(7)?;
        e.text(&self.owner)?;
        e.uint(self.owner_schema)?;
        e.item(&encode_op_path(&self.op)?)?;
        e.uint(self.version)?;
        e.array(count(self.inputs.len())?)?;
        for input in &self.inputs {
            e.array(2)?;
            match input {
                RecipeInput::Body(id) => {
                    e.uint(0)?;
                    e.item(&id.item()?)?;
                }
                RecipeInput::Entity(name) => {
                    e.uint(1)?;
                    e.bytes(&name.to_bytes())?;
                }
            }
        }
        e.bytes(&self.payload)?;
        e.array(count(self.outputs.len())?)?;
        for id in &self.outputs {
            e.item(&id.item()?)?;
        }
        Ok(e.finish()?.into_bytes())
    }
}

/// The recipe record of `recipe`.
///
/// # Errors
///
/// As [`Recipe::encode`].
pub fn recipe_record(recipe: &Recipe) -> Result<Record, RecipeError> {
    Ok(Record {
        tag: RECIPE_TAG,
        flags: REQUIRED,
        schema: RECIPE_SCHEMA,
        depends: Vec::new(),
        payload: recipe.encode()?,
    })
}

/// Reads a content ID: `[alg, digest]`.
fn read_content_id(
    r: &mut Reader<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ContentId, RecipeError> {
    let offset = r.offset();
    let shape = PayloadError::Shape {
        offset,
        expected: "a content ID [alg, digest]",
    };
    if r.array(meter, "a content ID")? != 2 {
        return Err(shape.into());
    }
    let alg = r.uint(meter, "a content ID's algorithm")?;
    let digest = r.bytes(meter, "a content ID's digest")?;
    ContentId::new(alg, digest).ok_or_else(|| shape.into())
}

/// Reads a recipe record's payload. The owner's payload is copied, never decoded.
///
/// # Errors
///
/// [`RecipeError`] for a broken shape, an owner that is not a namespace, or bytes that are not the
/// recipe's canonical encoding.
pub fn read_recipe(payload: &[u8], meter: &mut WorkMeter<'_>) -> Result<Recipe, RecipeError> {
    let mut r = Reader::new(payload);
    let offset = r.offset();
    if r.array(meter, "a recipe array")? != 7 {
        return Err(PayloadError::Shape {
            offset,
            expected: "a recipe of seven items",
        }
        .into());
    }
    let owner = String::from(r.text(meter, "the owner's namespace")?);
    let owner_schema = r.uint(meter, "the owner's schema")?;
    let op = read_op_path(&mut r, meter)?;
    let version = r.uint(meter, "the operation's version")?;
    let count = r.array(meter, "the inputs")?;
    let mut inputs = Vec::new();
    for _ in 0..count {
        let at = r.offset();
        let shape = PayloadError::Shape {
            offset: at,
            expected: "an input [0, content-id] or [1, name-id]",
        };
        if r.array(meter, "an input")? != 2 {
            return Err(shape.into());
        }
        inputs.push(match r.next(meter)? {
            Item::Uint(0) => RecipeInput::Body(read_content_id(&mut r, meter)?),
            Item::Uint(1) => RecipeInput::Entity(r.name(meter, "an input's NameId")?),
            _ => return Err(shape.into()),
        });
    }
    let owner_payload = r.bytes(meter, "the owner's payload")?.to_vec();
    let count = r.array(meter, "the outputs")?;
    let mut outputs = Vec::new();
    for _ in 0..count {
        outputs.push(read_content_id(&mut r, meter)?);
    }
    r.end()?;
    let recipe = Recipe {
        owner,
        owner_schema,
        op,
        version,
        inputs,
        payload: owner_payload,
        outputs,
    };
    if recipe.encode()? != payload {
        return Err(RecipeError::NotCanonical);
    }
    Ok(recipe)
}
