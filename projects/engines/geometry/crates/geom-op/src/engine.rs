//! The engine seam: what a program asks of the engine that answers its geometry, and how that
//! engine names itself, declared once so the engine behind it can change and the program does not.
//!
//! What it does: [`Engine`] is the interface a program codes against for the first building
//! operations: the extrusion of a flat profile with holes along a straight vector
//! ([`Engine::extrude`]), one solid less another ([`Engine::subtract`]), a loft through flat
//! polygon sections ([`Engine::loft`]), two solids united ([`Engine::unite`]), a solid's volume
//! ([`Engine::volume`]), a display mesh of it ([`Engine::mesh`]) and the engine's own record of
//! what answers each of them ([`Engine::edition`]). Every argument and result is a kernel value or
//! a plain number: grid points and steps of the engine's [`GridStep`] (chapter 6), exactness
//! classes, and this module's [`Profile`], [`Section`], [`Volume`], [`DisplayMesh`], [`Edition`] and
//! [`EngineError`]. Each engine keeps its own solid type behind [`Engine::Solid`]. The first
//! engine is geom-solidcore's (decision 32.70), outside the kernel's workspace; an exact engine of
//! the kernel's own takes its place by implementing the same trait.
//!
//! Guarantees:
//!
//! - **Declarations only.** The module defines types and one trait whose methods have no bodies;
//!   nothing here computes, so every engine answers for its own results.
//! - **Only checked results.** [`Engine`]'s contract: every solid an implementation returns has a
//!   body the kernel's checker passes, and anything else is an [`EngineError`].
//! - **No engine's own type.** No type of an engine behind the trait appears in it: a program that
//!   names only these items and the kernel's values keeps working when the engine changes.
//! - **The engine names itself.** [`Edition`] carries the engine's name, its version, the grid step
//!   it works on, its tolerances and which part of it answers each operation, so a program can
//!   record what built a shape.
//! - **Lengths in grid steps.** A program converts its own units once, at its seam, with
//!   [`GridStep::steps_per`]: on `Q0` one millimetre is 6,400 steps exactly.
//!
//! Method: chapter 25's stable facade, every program reaching geometry through `engine-geometry`
//! and never through an engine's own crate, and decision 32.70's rule that solidcore sits behind an
//! interface of the kernel's own (`projects/engines/geometry/architecture/25-public-interface.md`,
//! `projects/engines/geometry/architecture/32-decisions.md`).
//!
//! Cost: none; declarations only. Each engine states the cost of its own answers.

use alloc::string::String;
use alloc::vec::Vec;

use geom_num::{Exactness, GridStep, I256, P3};

/// A flat region to extrude: an outer polygon and the polygon holes inside it, every corner a grid
/// point on the outer polygon's plane, each loop in order either way round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// The outer polygon's corners.
    pub outer: Vec<P3>,
    /// The holes, each a polygon of corners.
    pub holes: Vec<Vec<P3>>,
}

/// One section of a loft ([`Engine::loft`]): a closed flat polygon placed in space, every corner a
/// grid point on one plane, in order round it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// The polygon's corners, in order round it.
    pub corners: Vec<P3>,
}

/// A solid's volume, as the engine knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Volume {
    /// Six times the volume, in cubic grid steps, exactly as the body's rows give it, and the
    /// exactness the body carries.
    SixTimes {
        /// Six times the volume, in cubic steps.
        cubic_steps: I256,
        /// The body's exactness class.
        exactness: Exactness,
    },
}

/// A display mesh: drawn, never measured, and accepted by no operation (chapter 19.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayMesh {
    /// Vertex positions, in grid steps of the engine's step.
    pub positions: Vec<[f64; 3]>,
    /// One unit normal per position, pointing out of the solid.
    pub normals: Vec<[f64; 3]>,
    /// Triangles as indices into the positions, counter-clockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

/// One tolerance an engine works to.
#[derive(Clone, Debug, PartialEq)]
pub struct Tolerance {
    /// The engine's own name for it.
    pub name: String,
    /// Its value, in `unit`.
    pub value: f64,
    /// The unit of `value`.
    pub unit: String,
}

/// Which part of an engine answers one operation, for one case of its inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// The operation, by the [`Engine`] method's name.
    pub operation: String,
    /// The inputs this answer is for, in plain words (`every case` when the operation has one).
    pub case: String,
    /// What answers it, in plain words.
    pub by: String,
}

/// What an engine is, so a program can record what built its shapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Edition {
    /// The engine's name.
    pub engine: String,
    /// Its version: for an engine taken from elsewhere, the source and the commit it is pinned to.
    pub version: String,
    /// The grid step every point and length is in.
    pub step: GridStep,
    /// The tolerances it works to.
    pub tolerances: Vec<Tolerance>,
    /// Which part of it answers each operation.
    pub answers: Vec<Answer>,
}

/// Why an engine gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// The input is not one the operation takes.
    Invalid {
        /// The operation, by the [`Engine`] method's name.
        operation: &'static str,
        /// What is wrong with the input.
        what: String,
    },
    /// The engine refused the operation, or its result failed a check the engine runs on every
    /// result.
    Refused {
        /// The operation, by the [`Engine`] method's name.
        operation: &'static str,
        /// The engine's reason.
        reason: String,
    },
    /// The engine does not answer this case yet.
    Unsupported {
        /// The operation, by the [`Engine`] method's name.
        operation: &'static str,
        /// What it does not answer.
        what: String,
    },
}

/// The engine a program asks for its geometry.
///
/// An implementation returns only solids whose bodies the kernel's checker (`geom-check`, at its
/// full level) passes, with the exactness they carry: a result that is not closed, or fails any
/// other check, is an [`EngineError`], never a solid (principle 4). One suite of tests is written
/// over this trait, so every engine, solidcore's today and the kernel's own exact ones later, is
/// judged by the same cases.
pub trait Engine {
    /// The engine's own solid, held by the program and handed back to the engine.
    type Solid;

    /// What the engine is and what answers each operation.
    fn edition(&self) -> Edition;

    /// The solid swept by `profile` along the vector `by` (whole steps along x, y and z), capped
    /// at both ends.
    ///
    /// # Errors
    ///
    /// [`EngineError::Invalid`] for a profile or vector the extrusion does not take, and the
    /// engine's refusals.
    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Self::Solid, EngineError>;

    /// `target` less `tool`, one solid; neither is changed.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unsupported`] for a result of more than one solid or operands the engine
    /// does not take yet, and the engine's refusals.
    fn subtract(
        &mut self,
        target: &Self::Solid,
        tool: &Self::Solid,
    ) -> Result<Self::Solid, EngineError>;

    /// The solid lofted through `sections` in their order: corner `i` of each section is joined to
    /// corner `i` of the next by a straight wall, and the first and last sections cap its ends.
    ///
    /// What the sections must be: at least two; each a polygon of at least three corners on one
    /// plane, with an area; every one with the same number of corners, listed so that corner `i`
    /// of each lines up with corner `i` of the next (the caller pairs the corners; the engine adds
    /// none and moves none); every section wholly ahead of the plane of the one before it, ahead
    /// being the side the second section lies on, so that the loft runs one way and does not fold
    /// back. The sections may all run either way round, all the same way. A loft that folds back
    /// is an invalid input, and one whose walls cross is a fault the engine reports as a refusal:
    /// neither is ever a solid.
    ///
    /// # Errors
    ///
    /// [`EngineError::Invalid`] for sections that are not as above, a folded loft among them,
    /// [`EngineError::Unsupported`] for walls the engine does not make yet, and the engine's
    /// refusals, a loft whose walls cross among them.
    fn loft(&mut self, sections: &[Section]) -> Result<Self::Solid, EngineError>;

    /// `a` united with `b`, one solid; neither is changed.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unsupported`] for a result of more than one solid or operands the engine
    /// does not take yet, and the engine's refusals.
    fn unite(&mut self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, EngineError>;

    /// The solid's volume.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unsupported`] for a solid whose volume the engine does not give yet.
    fn volume(&mut self, solid: &Self::Solid) -> Result<Volume, EngineError>;

    /// A display mesh of the solid's boundary.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unsupported`] for a face the engine does not mesh yet, and the engine's
    /// refusals.
    fn mesh(&mut self, solid: &Self::Solid) -> Result<DisplayMesh, EngineError>;
}
