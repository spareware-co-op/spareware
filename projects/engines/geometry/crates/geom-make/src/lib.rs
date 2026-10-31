//! Spare Kit geometry kernel: primitives and transforms.
//!
//! One job: The Make and Transform families of 10.2: primitives, transforms and patterns.
//!
//! Layer: 9, Shared operation services (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Every result passes all four checker groups before it is returned; inputs are never changed
//!   (principle 4, 8.3).
//! - Primitives name their faces by role (20.2).
//! - Transforms are typed so a change of kind is explicit: whole-step translations, quarter turns
//!   and mirrors stay exact; other rotations round to the grid and the result says so (7).
//! - Attribute events (Transformed, Copied) follow the history; a pattern declares its instances as
//!   copies in its schema (8.4).
//!
//! Sections: 7, 8.4, 10.2 and 30.5, in the kernel architecture
//! (`projects/engines/geometry/architecture/`).
//!
//! Family: Make and Transform.
//!
//! Chapter 30 places primitives here because no crate in chapter 27 owns them, and they cannot live
//! in `geom-topo`: every operation runs the checker before it returns, and `geom-check` sits above
//! `geom-topo`.
//!
//! Its direct dependencies are its row in chapter 30.5's amendment to chapter 27's crate table,
//! checked by the scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Modules (piece G101):
//!
//! - `build`: a solid's rows, named by role, published in name order, checked, with its history;
//!   [`Made`], what every operation here returns, and [`PER_ROW`], their charge schedule.
//! - [`primitive`]: box, prism, cylinder, cone, sphere and torus.
//! - [`transform`]: move, rotate, mirror and scale.
//! - [`pattern`]: copies of a body, one per transform, each keyed by the caller.
//!
//! Status: built by piece G101: every primitive and transform of 10.2 with its schema in the
//! facade; scales that are not uniform on curved bodies wait for G177's checker.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod build;
pub mod pattern;
pub mod primitive;
pub mod transform;

pub use build::{Made, Motion, PER_ROW, PHASE};
pub use pattern::{Copies, Instance, pattern};
pub use primitive::{
    BoxInput, PrismInput, RoundInput, SphereInput, TorusInput, cone, cylinder, make_box, prism,
    sphere, torus,
};
pub use transform::{Moved, mirror, move_by, rotate, scale};
