//! Repairing imported meshes (19.7): heal, within `ε_heal`, every step said.
//!
//! What it does: [`heal()`] turns a [`RawMesh`] into a closed [`ExactMesh`](crate::exact::ExactMesh)
//! by 19.7's five steps, each in a module of its own. [`quantise()`] rounds coordinates into the
//! frame, once (step 1). [`weld()`] joins each vertex to the lowest-keyed representative within ε
//! and cleans the faces that leaves (step 2), which closes every hole no wider than ε. [`holes`]
//! stitches the cracks of no width left and lists the boundaries still open (step 3). [`orient`]
//! floods one facing across each
//! patch and turns each closed patch outward by its exact volume (step 4). [`heal()`] then hands the
//! faces to the mesh boolean, which resolves self-intersections and overlaps by winding number
//! (step 5). Each step says what it did and where, in the [`HealLog`] the result carries, and a
//! failure lists what was left open. Rounding imported polyhedra (24.8) calls [`quantise()`] and
//! [`weld()`] with a weld distance of 0, never a copy of them.
//!
//! Choices 19.7 leaves open, made here:
//!
//! - **A face is made flat and simple in the weld.** Welding and rounding can leave a face off one
//!   plane, or crossing itself, and the mesh checker takes flat faces only, while the boolean cuts
//!   simple ones. Such a face is split into a fan from its lowest-keyed corner. A fan from any
//!   corner has the polygon's own winding number everywhere, so step 5 resolves it as the polygon
//!   was meant (24.8 makes the same split for rounded polyhedra).
//! - **Holes within ε are closed by the weld, cracks of no width by a stitch.** Representatives
//!   are more than ε apart, so after step 2 no loop narrower than ε is left for step 3's fan: the
//!   weld closed it by joining its corners, a move its class counts ([`holes`] has the argument).
//!   A vertex lying exactly on a boundary edge (a T-junction, or a sliver the weld removed) is put
//!   into every face along that edge. Nothing moves, and the mesh's half-edges can then pair.
//! - **A boundary is an edge used an odd number of times,** counted without direction, because
//!   step 3 comes before step 4 orients anything. Manifold counts directed uses that do not cancel,
//!   which is the same once a mesh is oriented.
//! - **A face repeated the same way round is kept once,** in step 2 and again once step 4 has
//!   turned faces, because the mesh checker refuses a repeat. A face and its reverse both stay: the
//!   winding number cancels them in step 5.
//!
//! Guarantees:
//!
//! - **Within ε, and said.** The result is [`Exact`](geom_num::Exactness::Exact) when no vertex
//!   moved. Otherwise it is [`Certified`](geom_num::Exactness::Certified) with the weld's largest
//!   move, at most ε, rounded up to whole steps: every point of a welded face is within that of the
//!   input face's matching point. The quantised input is the reference; [`quantise()`] reports its
//!   own rounding.
//! - **Exact decisions.** Every weld, flatness, crossing, collinearity and volume decision is on
//!   grid points in integers or an exact predicate, never a float.
//! - **The input's order does not show.** Vertices are keyed by their Morton codes and faces by
//!   their keys, so the vertices' and faces' order and where each loop starts change nothing.
//! - **Never quietly wrong.** A boundary wider than ε is never closed: heal fails with
//!   [`HealError::NotClosed`] or, past [`HEAL_CAP`], [`HealError::ToleranceExceeded`], each
//!   boundary listed with where it is and how wide. Nothing is retried with a larger fuzz and no
//!   cutter is dilated (19.7, after `IfcOpenShell`'s dilation, which 08, section A3, records).
//!
//! Method: chapter 19.7 (`projects/engines/geometry/architecture/19-meshes-spatial-clash.md`), with
//! 19.2's keys and 19.5's boolean; the steps of the stored analysis of mesh repair (weld, orient,
//! then winding numbers; `projects/kit/notes/G01-evidence/08-standards-coverage-testing.md`).
//!
//! Consulted: Manifold, Apache-2.0,
//!  <https://github.com/elalish/manifold/blob/d056b8fb70de8c8e6d6bee2e639b71c20aa08d01/src/sort.cpp>,
//!  for `MergeMeshGLP`: open edges as directed uses that do not cancel, and welding only within a
//!  tolerance; its method, not its code.
//!
//! Cost: each step's, in its module: sorts of the vertices and the edges, one cell-index search per
//! vertex and per boundary edge, and the boolean's arrangement of step 5.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use geom_num::grid::{GridStep, P3, Steps};
use geom_num::ladder::HEAL_CAP;
use geom_num::{Exactness, NumError};
use geom_op::{Bound, Remedy, Value};
use geom_run::Stopped;

use crate::exact::MeshError;
use crate::exact::raw::RawMesh;

pub mod heal;
pub mod holes;
mod loops;
pub mod orient;
pub mod quantise;
pub mod reconstruct;
pub mod weld;

pub use heal::{Heal, HealLog, Healed, heal};
pub use quantise::{Quantised, quantise};
pub use reconstruct::{Reconstruct, ReconstructError, Reconstructed, reconstruct};
pub use weld::{Welded, weld};

/// An open boundary: a loop of edges that only one face, or an odd number of faces, uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// Its lowest-keyed corner: where it is.
    pub at: P3,
    /// How many corners it has.
    pub corners: u32,
    /// Its width: the largest distance between two of its corners, rounded up to whole steps.
    pub width: Steps,
}

/// Why healing failed.
#[derive(Clone, Debug, PartialEq)]
pub enum HealError {
    /// This vertex has a coordinate that is not a finite number.
    NotFinite {
        /// The vertex.
        vertex: u32,
    },
    /// This vertex rounds outside the frame's ±(2^40 − 1) steps.
    OutOfFrame {
        /// The vertex.
        vertex: u32,
    },
    /// The unit is zero steps.
    NoUnit,
    /// ε is above the cap of 6,400 steps, [`HEAL_CAP`].
    AboveCap {
        /// The ε asked for.
        asked: Steps,
    },
    /// These boundaries are still open, each narrower than the cap: a larger ε may close them.
    NotClosed {
        /// The boundaries, the lowest-keyed first.
        boundaries: Vec<Boundary>,
    },
    /// These boundaries are wider than the cap, which healing never passes.
    ToleranceExceeded {
        /// The boundaries over the cap, the lowest-keyed first.
        boundaries: Vec<Boundary>,
    },
    /// Every edge is used an even number of times, but these are used more often one way than the
    /// other once each patch faces one way: a non-orientable piece, or sheets that do not pair.
    Unpaired {
        /// Each edge's two ends, the lower-keyed first.
        edges: Vec<[P3; 2]>,
        /// For each patch that cannot be oriented, an edge where its flood met itself turned.
        non_orientable: Vec<[P3; 2]>,
    },
    /// A failure of the mesh, its numbers or its budget.
    Mesh(MeshError),
}

impl HealError {
    /// What a caller can do to make healing succeed (chapter 22). Boundaries under the cap name
    /// the ε that may close them, the widest's width, at which the weld joins its corners, and the
    /// class that would give, Certified by that ε; every open boundary also names `reconstruct`,
    /// whose result is Approximate, its deviation measured both ways (19.7). `step` is the frame's
    /// grid step.
    #[must_use]
    pub fn remedies(&self, step: GridStep) -> Vec<Remedy> {
        let reconstruct = Remedy::UseRoute {
            route: "reconstruct",
        };
        match self {
            HealError::NotClosed { boundaries } => {
                let widest = boundaries
                    .iter()
                    .map(|b| b.width)
                    .max_by_key(|w| w.get())
                    .unwrap_or(Steps::ZERO);
                let mut out = vec![Remedy::ChangeInput {
                    input: "epsilon",
                    to: Some((
                        Bound::AtLeast,
                        Value::Length {
                            steps: widest,
                            step,
                        },
                    )),
                }];
                out.push(Remedy::Accept {
                    class: Exactness::Certified {
                        bound: widest,
                        step,
                    },
                });
                out.push(reconstruct);
                out
            }
            HealError::ToleranceExceeded { .. } | HealError::Unpaired { .. } => vec![reconstruct],
            HealError::AboveCap { .. } => vec![Remedy::ChangeInput {
                input: "epsilon",
                to: Some((
                    Bound::AtMost,
                    Value::Length {
                        steps: HEAL_CAP,
                        step,
                    },
                )),
            }],
            HealError::NoUnit | HealError::NotFinite { .. } | HealError::OutOfFrame { .. } => {
                Vec::new()
            }
            HealError::Mesh(_) => Vec::new(),
        }
    }
}

impl fmt::Display for HealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealError::NotFinite { vertex } => {
                write!(f, "vertex {vertex} has a coordinate that is not a number")
            }
            HealError::OutOfFrame { vertex } => {
                write!(f, "vertex {vertex} rounds outside the frame")
            }
            HealError::NoUnit => f.write_str("a unit of zero steps"),
            HealError::AboveCap { asked } => write!(
                f,
                "a heal distance of {} steps is over the cap of {} steps",
                asked.get(),
                HEAL_CAP.get()
            ),
            HealError::NotClosed { boundaries } => write!(
                f,
                "{} open boundaries are wider than the heal distance: heal with a larger one, or reconstruct (Approximate)",
                boundaries.len()
            ),
            HealError::ToleranceExceeded { boundaries } => write!(
                f,
                "{} open boundaries are wider than healing's cap of {} steps: reconstruct (Approximate)",
                boundaries.len(),
                HEAL_CAP.get()
            ),
            HealError::Unpaired { edges, .. } => write!(
                f,
                "{} edges cannot be paired one way against the other: reconstruct (Approximate)",
                edges.len()
            ),
            HealError::Mesh(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for HealError {}

impl From<MeshError> for HealError {
    fn from(e: MeshError) -> Self {
        HealError::Mesh(e)
    }
}

impl From<NumError> for HealError {
    fn from(e: NumError) -> Self {
        HealError::Mesh(MeshError::Num(e))
    }
}

impl From<Stopped> for HealError {
    fn from(e: Stopped) -> Self {
        HealError::Mesh(e.into())
    }
}

impl From<geom_bvh::BvhError> for HealError {
    fn from(e: geom_bvh::BvhError) -> Self {
        HealError::Mesh(e.into())
    }
}

impl From<crate::exact::MeshFault> for HealError {
    fn from(e: crate::exact::MeshFault) -> Self {
        HealError::Mesh(e.into())
    }
}
