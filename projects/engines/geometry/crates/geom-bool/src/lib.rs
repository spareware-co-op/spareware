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
//! - [`rebuild`]: merged pieces published as a body by canonical rules (G129).
//!
//! Status: the rebuild built (G129); the rest by pieces G130, G131, G132, G133, G137, G138, G139,
//! G144, G242, G243, G244, G245, G246, G247, G248, G249, G250, G324.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod rebuild;
