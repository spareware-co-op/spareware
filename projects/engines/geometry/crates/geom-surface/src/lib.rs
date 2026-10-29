//! Spare Kit geometry kernel: every surface kind.
//!
//! One job: Every surface kind, the subdivision limit surface included (the net, its local rules,
//! Stam evaluation and tables): evaluation, bounds, projection, conversion, extension, frame rules,
//! flat maps of developable kinds, Class-A maths, the `ImplicitBound` trait.
//!
//! Layer: 2, Geometry (chapter 27's proposed layer table).
//!
//! Invariants this crate keeps:
//!
//! - Each kind is stored as itself in a closed enum; conversion to NURBS is on demand and states
//!   whether it is exact and its maximum error (7).
//! - Every kind gives evaluation, derivatives and conservative bounds over any parameter box; a
//!   derivative bound may be `Unbounded`, and the kind then gives a normal cone (12.1).
//! - Trimming belongs to the topology's loops, never to a surface kind (7).
//! - The subdivision control net is exact, on the grid; its limit surface is certified (9, 16.1).
//! - `ImplicitBound` gives the interval value and gradient of F over a 3D box, so planes, quadrics,
//!   tori and fields take part in intersection without upward dependencies (12.1).
//!
//! Sections: 7, 12.1, 13 (introduction), 13.9, 16.1, 16.5, 16.6, 19.3 and 26.1, in the kernel
//! architecture (`projects/engines/geometry/architecture/`).
//!
//! Its direct dependencies are its row of chapter 27's crate table, checked by the
//! scaffold test in `engine-geometry` (`tests/scaffold`).
//!
//! Status: scaffold only; built by pieces G60, G65, G66, G67, G68, G253, G254, G260, G264, G315,
//! G316, G317, G318, G470.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
