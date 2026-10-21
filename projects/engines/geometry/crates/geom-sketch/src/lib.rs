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
//! Its direct dependencies are its row of chapter 27's proposed crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//! Direct dependencies still to add when geom-run merges (G04): geom-run.
//! Direct dependencies still to add when geom-math merges (G03): geom-math.
//!
//! Status: scaffold only; built by pieces G216, G217, G218, G219, G220, G221, G222, G223, G224,
//! G225, G226, G227, G228, G230, G477, G478.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
