//! Overflow detection and the caller's policy (13.10, "Overflows", G262).
//!
//! What it does: a blend overflows when a contact curve would leave its support face. A [`Site`] is
//! one place where a contact curve nears the edge of its face: the face, the edge that bounds it
//! there, how far the face reaches (an enclosure), the half-angle tangent of the blend's edge, and
//! what lies beyond. [`detect`] finds the sites where r · tan(φ / 2) passes the reach. [`resolve`]
//! picks a type for each by the caller's ordered [`Policy`]: **smooth** (carry on across a smooth
//! edge onto the next face), **cliff** (across a sharp edge, one side stays on the face and the
//! other runs along the edge), **cliff-end** (where a cliff does not fit, the blend's end becomes a
//! cliff along the edges it meets) and **notch** (keep the blend's surface and trim it by the
//! faces it runs into). If no allowed type works the plan fails with
//! [`OverflowError::OverflowNotAllowed`], naming the boundary edge and the type that would.
//!
//! Guarantees:
//!
//! - **Certain or possible, never missed.** A site overflows for certain when the needed reach is
//!   proven past the face's reach, and possibly when the enclosures overlap; both are reported, so
//!   no overflow is missed, and [`Found::certain`] says which.
//! - **A type must be possible.** Smooth needs a smooth edge beyond, cliff a sharp edge and room
//!   for it, cliff-end a sharp edge and no room, notch any edge that is not smooth. A type the
//!   policy allows but the site cannot take is skipped, never forced.
//! - **The default order is Parasolid's published one**, smooth then cliff then notch, with
//!   cliff-end tried where a cliff does not fit (pending owner check, rule 11).
//! - **Every refusal names a way out.** [`OverflowError::OverflowNotAllowed`] carries the type that
//!   would work, so the caller can allow it.
//!
//! Method: chapter 13.10 of the kernel architecture, "Overflows"
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`), which takes the four
//! overflow types of the survey of Vida, Martin and Varady, CAD 26(5), 1994
//! (<https://doi.org/10.1016/0010-4485(94)90023-X>), in its own words.
//!
//! Cost: linear in the sites and the policy. No recursion.

use alloc::vec::Vec;
use core::fmt;

use geom_num::interval::Interval;

use crate::chain::EdgeId;
use crate::cluster::FaceId;

/// How a blend carries on past the edge of a face it overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowKind {
    /// Across a smooth edge onto the next face.
    Smooth,
    /// Across a sharp edge: one side stays on the face and the other runs along the edge.
    Cliff,
    /// Where a cliff does not fit: the blend's end becomes a cliff along the edges it meets.
    CliffEnd,
    /// The blend's surface is kept and trimmed by the faces it runs into.
    Notch,
}

/// What lies past the edge of the face at a site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beyond {
    /// A smooth edge to the next face.
    Smooth,
    /// A sharp edge.
    Sharp {
        /// Whether a cliff fits along it.
        cliff_fits: bool,
    },
    /// A vertex: the face ends in a corner.
    Corner,
}

/// A place where a contact curve nears the edge of its face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// The blended edge.
    pub edge: EdgeId,
    /// The face that carries the contact curve.
    pub face: FaceId,
    /// The edge that bounds the face there.
    pub boundary: EdgeId,
    /// How far the face reaches from the blended edge, perpendicular to it, as an enclosure.
    pub reach: Interval,
    /// The tangent of half the angle between the blended edge's faces.
    pub tan_half: Interval,
    /// What lies past the boundary.
    pub beyond: Beyond,
    /// Whether the site is inside the blend's run rather than at its end.
    pub internal: bool,
}

/// A site where the blend overflows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    /// The site.
    pub site: Site,
    /// Whether the overflow is certain: the reach needed is proven past the face's.
    pub certain: bool,
}

/// An overflow with the type chosen for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    /// The site.
    pub site: Site,
    /// The type chosen.
    pub kind: OverflowKind,
}

/// Which types are allowed, in what order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// The allowed types, most wanted first.
    pub order: Vec<OverflowKind>,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            order: alloc::vec![
                OverflowKind::Smooth,
                OverflowKind::Cliff,
                OverflowKind::CliffEnd,
                OverflowKind::Notch
            ],
        }
    }
}

/// Why an overflow could not be resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowError {
    /// No allowed type works at this site; the boundary edge and the type that would are named.
    OverflowNotAllowed {
        /// The boundary edge.
        boundary: EdgeId,
        /// A type that would work if it were allowed.
        would_work: OverflowKind,
    },
    /// No type can work at all: nothing past the boundary can carry the blend.
    NoOverflowPossible {
        /// The boundary edge.
        boundary: EdgeId,
    },
}

impl fmt::Display for OverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverflowError::OverflowNotAllowed { .. } => {
                f.write_str("no allowed overflow type works at this boundary")
            }
            OverflowError::NoOverflowPossible { .. } => {
                f.write_str("no overflow type can carry the blend past this boundary")
            }
        }
    }
}

impl core::error::Error for OverflowError {}

/// The sites where a blend of `radius` overflows, in the order given.
///
/// # Errors
///
/// [`geom_num::NumError`] from the interval product, as a refusal to decide.
pub fn detect(sites: &[Site], radius: f64) -> Result<Vec<Found>, geom_num::NumError> {
    let r = Interval::point(radius)?;
    let mut out = Vec::new();
    for site in sites {
        let needed = r.mul(site.tan_half)?;
        if needed.lo() > site.reach.hi() {
            out.push(Found {
                site: *site,
                certain: true,
            });
        } else if needed.hi() > site.reach.lo() {
            out.push(Found {
                site: *site,
                certain: false,
            });
        }
    }
    Ok(out)
}

/// Whether a type can be taken at a site.
const fn possible(kind: OverflowKind, beyond: Beyond) -> bool {
    match (kind, beyond) {
        (OverflowKind::Smooth, Beyond::Smooth)
        | (OverflowKind::Notch, Beyond::Sharp { .. } | Beyond::Corner) => true,
        (OverflowKind::Cliff, Beyond::Sharp { cliff_fits }) => cliff_fits,
        (OverflowKind::CliffEnd, Beyond::Sharp { cliff_fits }) => !cliff_fits,
        _ => false,
    }
}

/// The type for each overflow, by the policy.
///
/// # Errors
///
/// [`OverflowError::OverflowNotAllowed`] for a site where a type would work but the policy does
/// not allow it, [`OverflowError::NoOverflowPossible`] for one where none would; the first such
/// site in order.
pub fn resolve(found: &[Found], policy: &Policy) -> Result<Vec<Resolved>, OverflowError> {
    const ORDER: [OverflowKind; 4] = [
        OverflowKind::Smooth,
        OverflowKind::Cliff,
        OverflowKind::CliffEnd,
        OverflowKind::Notch,
    ];
    let mut out = Vec::new();
    for f in found {
        let chosen = policy
            .order
            .iter()
            .copied()
            .find(|k| possible(*k, f.site.beyond));
        if let Some(kind) = chosen {
            out.push(Resolved { site: f.site, kind });
        } else {
            let would = ORDER.iter().copied().find(|k| possible(*k, f.site.beyond));
            return Err(match would {
                Some(would_work) => OverflowError::OverflowNotAllowed {
                    boundary: f.site.boundary,
                    would_work,
                },
                None => OverflowError::NoOverflowPossible {
                    boundary: f.site.boundary,
                },
            });
        }
    }
    Ok(out)
}
