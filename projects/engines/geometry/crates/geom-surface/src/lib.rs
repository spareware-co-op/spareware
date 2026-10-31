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
//! Modules today (pieces G60, G65, G66, G67 and G68): [`ParamSurface`], the behaviour every kind has, and
//! [`SurfaceKind`], the closed enum that stores every kind, in [`param`]; the NURBS kind
//! (`nurbs`); the analytic kinds ([`analytic`]) and the swept kinds ([`swept`]) on their shared
//! closed-form terms (`terms`); the offset kind ([`offset`]), with its two-variable interval
//! jets (`jet`) and its certified NURBS cache (`cache`); the [`ImplicitBound`] trait, with
//! `geom-num`'s exact plane as its first implementor, in [`implicit`]; and the kind contract any
//! new kind must pass ([`contract`]). Errors are `geom-curve`'s `CurveError`, shared by every
//! kind. The analytic kinds' bounds, derivative bounds and partials over a box are their
//! closed-form extents (`extent`, piece G67). Every kind's second partials and unit normal's
//! partials over a box, and chapter 19.3's chord constant, come from its partials there
//! ([`derivative`], piece G68); a rational NURBS patch's partials are the interval quotient rule
//! with the patch itself bounded by its control points.
//!
//! Status: the traits, the enum, the NURBS kind, `ImplicitBound` and the contract built (G60); the
//! analytic and swept kinds (G65); the offset kind, and every kind's partials of any order
//! (G66); the analytic kinds' closed-form extents (G67); certified second-derivative and
//! normal-derivative bounds for every kind (G68); the other kinds to come in G253, G254, G260,
//! G264, G315, G316, G317, G318 and G470.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod analytic;
mod cache;
pub mod contract;
pub mod derivative;
mod extent;
pub mod implicit;
mod jet;
mod nurbs;
pub mod offset;
pub mod param;
pub mod swept;
mod terms;

pub use analytic::{AffineSurface, Cone, Cylinder, Plane, Sphere, Torus, TorusForm};
pub use derivative::{SecondDerivativeBound, SecondPartials};
pub use implicit::{ImplicitBound, ImplicitEnclosure};
pub use offset::Offset;
pub use param::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind, partial_index};
pub use swept::{Extrusion, Revolution, Ruled};
pub use terms::Fun;
