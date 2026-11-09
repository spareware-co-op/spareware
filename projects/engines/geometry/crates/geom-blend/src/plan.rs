//! The blend plan: feasibility first, and the preview query (13.10, G262).
//!
//! What it does: [`plan`] takes the facts about each chain (its radius limits and the sites where its
//! contact curves near a face's edge), the interaction clusters of the chains, the faces that
//! chains share, a request radius for each chain and the caller's overflow policy, and returns a
//! [`Plan`]: for each chain the largest feasible radius, the verdict on the request, and the
//! overflows resolved; for each cluster whether it succeeds as one. A cluster fails if any of its
//! chains does, or if the chains that share a narrow face ask for more of it than it has
//! ([`SharedFace`]). The plan is also the blend preview: what fits, asked of the facts without
//! building anything.
//!
//! Guarantees:
//!
//! - **A cluster succeeds or fails as one.** [`ClusterPlan::feasible`] is true only when every chain
//!   in it is feasible with its overflows resolved and every shared face has room for all of them.
//! - **The largest feasible radius is certified, per chain.** It is the lowest lower end among the
//!   chain's limits ([`crate::radius::largest`]), so a request at or below it passes that bound.
//! - **A refusal names a size.** An infeasible chain carries [`crate::radius::Largest`]; a shared
//!   face that is too narrow carries the factor by which every radius asking for it must be scaled
//!   to fit.
//! - **Nothing is built.** The plan reads facts and never calls a constructor, so it cannot leave a
//!   hole; an unresolved overflow is an error of the plan, not a trimmed result.
//!
//! Method: chapter 13.10 of the kernel architecture, "Feasibility first" and "Never a hole"
//! (`projects/engines/geometry/architecture/13-sweeps-blends-offsets.md`).
//!
//! Cost: linear in the chains, limits and sites. No recursion.

use alloc::vec::Vec;

use geom_num::NumError;
use geom_num::interval::Interval;

use crate::cluster::{Cluster, FaceId};
use crate::overflow::{Found, OverflowError, Policy, Resolved, Site, detect, resolve};
use crate::radius::{Largest, Limit, Verdict, judge, largest};

/// What is known about one chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainFacts {
    /// The bounds on its radius.
    pub limits: Vec<Limit>,
    /// The places its contact curves near the edge of their faces.
    pub sites: Vec<Site>,
}

/// A face that several chains ask for, and what each asks.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedFace {
    /// The face.
    pub face: FaceId,
    /// How much of it there is, perpendicular to the chains, as an enclosure.
    pub width: Interval,
    /// For each chain that asks for it, the chain's index and the tangent of half its angle there.
    pub demands: Vec<(usize, Interval)>,
}

/// What the plan finds for one chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainPlan {
    /// The largest radius that certainly builds, with its cause; `None` with no limits.
    pub largest: Option<Largest>,
    /// The verdict on the request.
    pub verdict: Verdict,
    /// The overflows at the requested radius, each with its type.
    pub overflows: Vec<Resolved>,
}

/// A shared face that is too narrow for what is asked of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crowded {
    /// The face.
    pub face: FaceId,
    /// The factor, below one, by which every radius asking for the face must be scaled to fit.
    pub scale: f64,
}

/// What the plan finds for one cluster.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterPlan {
    /// The chains in it, as indices.
    pub chains: Vec<usize>,
    /// Whether it succeeds as one.
    pub feasible: bool,
    /// The shared faces that are too narrow.
    pub crowded: Vec<Crowded>,
}

/// The whole plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// One per chain, in order.
    pub chains: Vec<ChainPlan>,
    /// One per cluster, in order.
    pub clusters: Vec<ClusterPlan>,
}

/// Why a plan could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// An overflow has no allowed type.
    Overflow(OverflowError),
    /// The numbers layer refused.
    Num(NumError),
    /// A request, a cluster or a shared face names a chain that is not there.
    UnknownChain(usize),
}

impl From<OverflowError> for PlanError {
    fn from(e: OverflowError) -> Self {
        PlanError::Overflow(e)
    }
}

impl From<NumError> for PlanError {
    fn from(e: NumError) -> Self {
        PlanError::Num(e)
    }
}

impl core::fmt::Display for PlanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PlanError::Overflow(e) => e.fmt(f),
            PlanError::Num(e) => e.fmt(f),
            PlanError::UnknownChain(_) => f.write_str("a plan names a chain that is not there"),
        }
    }
}

impl core::error::Error for PlanError {}

/// The factor, at most one, by which radii must be scaled so the demands on a face fit its width:
/// width / (Σ r · tan(φ / 2)), by the lower end of the width and the upper end of each demand.
fn crowding(
    shared: &SharedFace,
    radii: &[f64],
    chains: usize,
) -> Result<Option<Crowded>, PlanError> {
    let mut total = Interval::point(0.0)?;
    for (chain, tan_half) in &shared.demands {
        if *chain >= chains {
            return Err(PlanError::UnknownChain(*chain));
        }
        let r = radii
            .get(*chain)
            .copied()
            .ok_or(PlanError::UnknownChain(*chain))?;
        total = total.add(Interval::point(r)?.mul(*tan_half)?)?;
    }
    if total.hi() <= shared.width.lo() {
        return Ok(None);
    }
    let scale = if total.hi() > 0.0 {
        shared.width.lo() / total.hi()
    } else {
        1.0
    };
    Ok(Some(Crowded {
        face: shared.face,
        scale,
    }))
}

/// Plans the blends: `radii[i]` is the radius asked for chain `i`.
///
/// # Errors
///
/// [`PlanError::Overflow`] when an overflow has no allowed type, [`PlanError::UnknownChain`] for a
/// chain index that is not there, [`PlanError::Num`] when the numbers layer refuses.
pub fn plan(
    facts: &[ChainFacts],
    radii: &[f64],
    clusters: &[Cluster],
    shared: &[SharedFace],
    policy: &Policy,
) -> Result<Plan, PlanError> {
    let mut chains = Vec::with_capacity(facts.len());
    for (i, fact) in facts.iter().enumerate() {
        let radius = radii.get(i).copied().ok_or(PlanError::UnknownChain(i))?;
        let found: Vec<Found> = detect(&fact.sites, radius)?;
        let overflows = resolve(&found, policy)?;
        chains.push(ChainPlan {
            largest: largest(&fact.limits),
            verdict: judge(&fact.limits, radius),
            overflows,
        });
    }
    let mut out = Vec::with_capacity(clusters.len());
    for cluster in clusters {
        let mut feasible = true;
        for &i in &cluster.chains {
            let chain = chains.get(i).ok_or(PlanError::UnknownChain(i))?;
            feasible &= chain.verdict == Verdict::Feasible;
        }
        let mut crowded = Vec::new();
        for face in shared {
            let touches = face
                .demands
                .iter()
                .any(|(chain, _)| cluster.chains.contains(chain));
            if touches && let Some(c) = crowding(face, radii, facts.len())? {
                feasible = false;
                crowded.push(c);
            }
        }
        out.push(ClusterPlan {
            chains: cluster.chains.clone(),
            feasible,
            crowded,
        });
    }
    Ok(Plan {
        chains,
        clusters: out,
    })
}
