//! Spare Construct: exchange formats.
//!
//! IFC, DXF, PDF, glTF and 3D Tiles, from published specifications only.
//! Map: "Interop".
//!
//! - [`ifc_spatial`] (job 12.1, CON-A18-F1-P2, first half): a model's project, site, building,
//!   storeys, layered walls and floors written as IFC 4.3 (ISO 16739-1) in the STEP text form
//!   (ISO 10303-21): [`ifc_spatial::write`] of an [`ifc_spatial::Export`].
//! - [`step`]: the STEP text form, written ([`step::Step`], [`step::Val`]).
//! - [`decimal`]: grid steps as exact decimal millimetres ([`decimal::mm`]), the one way a length
//!   reaches a file.
//! - [`guid`]: IFC `GlobalId`s from our permanent IDs ([`guid::global_id`]), the same on every export.
//! - [`error`]: [`IfcError`], every refusal as a value.
//!
//! Promises: the same model gives the same bytes (no clock, no random number, no hash-map order;
//! the header's stamp is the caller's), every length is exact (no float), nothing panics. Reading
//! IFC in, the other formats and the IFC mapping tables (job 12.3) are not here yet.

pub mod decimal;
pub mod error;
pub mod guid;
pub mod ifc_spatial;
pub mod step;

pub use error::IfcError;
