//! Spare Kit geometry kernel: the constraint solver.
//!
//! One job: The constraint solver, in 2D and 3D.
//!
//! Layer: 10, Operations (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Never moves fixed, reference or model geometry (15.18).
//! - Never drops, relaxes or reorders a constraint silently, and never returns a partly solved
//!   sketch as a success (15.18).
//! - Constraints refer to entities by persistent name, never by position (15.14, 15.18).
//! - Convergence is measured in grid steps and budgets in work, not time; running out returns
//!   `BudgetExceeded` or `Cancelled`, never a best-so-far answer (15.6).
//! - Never flattens arcs, ellipses or splines to solve them, and never uses hand-typed or
//!   finite-difference derivatives or platform maths (15.18).
//!
//! Sections: 15, and 22 for its errors, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Sketch.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules today:
//!
//! - [`plane`]: sketch planes, their maps into 3D and the Gram metric (piece G216).
//! - [`model`]: entities, variables and constraints, in authoring order (piece G217).
//! - [`equation`]: each constraint's residual and singular set, and the Jacobian's assembly (pieces
//!   G217 and G218).
//! - [`author`]: the plain constraints a macro writes, for fixing an entity and for offset chains
//!   (piece G218).
//! - [`structure`]: substitution, components, matching and the Dulmage–Mendelsohn decomposition
//!   (piece G221).
//! - [`solve`]: Gauss–Newton steps with Moré's damping on each block, and continuation (piece G222).
//! - [`text`]: the `sketch v1` text form (piece G217).
//!
//! Status: built by pieces G216, G217, G218, G219, G220, G221, G222, G223, G224, G225, G226, G227,
//! G228, G230, G477, G478; G216, G217, G218, G221 and G222 are done.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod author;
pub mod equation;
pub mod model;
pub mod plane;
pub mod solve;
pub mod structure;
pub mod text;
