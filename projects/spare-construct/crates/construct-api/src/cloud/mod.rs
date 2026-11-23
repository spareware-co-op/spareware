//! Spare Construct on its cloud server: who may do what, and the commands the kit's orderer runs
//! for this program (CON-A25, CON-A26). Turned on by the `cloud` feature, so the browser's build
//! stays free of the store. The rest of the server is in `projects/spare-construct/server`.
//!
//! - [`access`]: what Cedar is told, from the rows, fresh on every request.
//! - [`elements`]: the `element.*` commands. They change inputs only, and never touch a shape.
//! - [`params`]: parameters, types, and an element's parameter values and type, through the same
//!   log.
//! - [`grids`]: grid lines and reference planes, through the same log.
//! - [`families`]: placing a family of the library on a wall, a door or a window, through the same
//!   log.
//! - [`hosting`]: hosting an element on a face of another, a door on a wall, through the same log.
//! - [`levels`]: a document's levels, and tying elements and a wall's top to them, through the
//!   same log.
//! - [`walls`]: wall types and their layers, and drawing layered walls of them, through the same
//!   log.
//! - [`floors`]: floor types and their layers, and floors drawn from a boundary, through the same
//!   log.
//! - [`views`]: making and changing views (plans, sections, elevations), through the same log.
//! - [`undo`]: undo and redo, as new entries in the same log that reverse a person's own steps.
//! - [`transaction`]: every command fully applies or fully rolls back.
//! - [`spaces`]: models, packages, bridges and links.
//! - [`directory`]: hubs, projects and teams, in the directory's log.
//! - [`commands`]: a project's commands, each behind Cedar.

pub mod access;
pub mod commands;
pub mod directory;
pub mod elements;
pub mod families;
pub mod floors;
pub mod grids;
pub mod hosting;
pub mod levels;
pub mod params;
pub mod spaces;
pub mod transaction;
pub mod undo;
pub mod views;
pub mod walls;

pub use access::{Access, POLICY, SCHEMA, Target};
pub use commands::ProjectCommands;
