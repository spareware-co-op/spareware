//! Spare Kit geometry kernel: the arrangement engine.
//!
//! One job: The arrangement engine: booleans, imprint, split, cells, batch cuts, the arrangement
//! step of local operations, `close_near_contacts`.
//!
//! Layer: 6, Arrangements (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - One arrangement, many selections: every result is a selection of cells by winding numbers (11,
//!   11.2).
//! - Nothing is returned until it passes the checker and the arrangement certificate; inputs are
//!   never changed, and a failure or cancellation leaves the model exactly as it was (11.10).
//! - Never wrong topology quietly: a near-tangent case gets the right topology within its stated
//!   bound or fails with `NearContact` and a remedy (11.13).
//! - Nothing falls back silently to another representation or path (9, 11.8).
//! - Work is charged per fixed item, never per predicate call, box visited or cache probe, so
//!   `BudgetExceeded` happens at the same stage on every machine (11.12).
//!
//! Sections: 10.2, 11, 13 (introduction), 13.14, 16.1, 22, 23.5 and 23.6, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Family: Combine.
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules:
//!
//! - [`operand`]: operands and selections, and an operand's faces as convex polygons (G130).
//! - [`broad`]: the broad phase: which pairs of entities may touch (G242).
//! - [`carrier`]: what the curved builder stores: points, curves and their carriers (G242).
//! - [`combine`]: the boolean's entry and its schema (G131).
//! - [`decide`]: the exact analytic decision table of the certified curved path (G242).
//! - [`distance`]: certified distances of points from carriers, and between points (G242).
//! - [`merge`]: touching entities become one, checked against their carriers (G242).
//! - [`half_space`]: everything on one side of a plane, as a solid operand (G130).
//! - [`rule`]: the contact rule at `ε_fit`, `NearContact` and `CoincidenceChain` (G242).
//! - [`rebuild`]: merged pieces published as a body by canonical rules (G129).
//!
//! Status: the rebuild (G129), operands and selections (G130) and the entry (G131) built; the rest
//! by pieces G132, G133, G137, G138, G139, G144, G242, G243, G244, G245, G246, G247, G248, G249,
//! G250, G324.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod broad;
pub mod carrier;
pub mod combine;
pub mod decide;
pub mod distance;
pub mod half_space;
pub mod merge;
pub mod operand;
pub mod rebuild;
pub mod rule;
mod wires;
