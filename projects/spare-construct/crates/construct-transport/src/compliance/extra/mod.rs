//! Compliance+ (port of features/compliance-extra.js): the scope and wording linter, precinct staging, mitigation and conditions,
//! options and alternatives, the Fast-track package, plan change, speed limits, ANOP, parking, subdivision, policy alignment,
//! time-of-use screening and the hand-over packs.
//!
//! [`ctx`] and [`shared`] hold what the parts share; each other module is one part with its checks, its report sections (`report`) and its
//! exports (`export_build`). [`report`] and [`export_build`] here ask each part in turn.

pub mod ctx;
pub mod shared;

pub mod lint;
pub mod precinct;
pub mod mitigation;
pub mod options;
pub mod ftaa;
pub mod speed;
pub mod anop;
pub mod parking;
pub mod subdivision;
pub mod policy;
pub mod gps;
pub mod tou;
pub mod climate;
pub mod packs;

use crate::compliance::html::ReportCtx;
use crate::compliance::jv::V;

/// The HTML of a Compliance+ report section (`REPORT[id].render(P, ctx)`), None when no part has a section with that id.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if let Some(h) = lint::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = precinct::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = mitigation::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = options::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = ftaa::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = speed::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = anop::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = parking::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = subdivision::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = policy::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = gps::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = tou::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = climate::report(id, p, ctx) {
        return Some(h);
    }
    if let Some(h) = packs::report(id, p, ctx) {
        return Some(h);
    }
    None
}

/// The text of a Compliance+ export (`EXPORTS`, id "compliance-extra-<id>"), None when no part has an export with that id.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    if let Some(t) = lint::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = precinct::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = mitigation::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = options::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = ftaa::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = speed::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = anop::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = parking::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = subdivision::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = policy::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = gps::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = tou::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = climate::export_build(id, p) {
        return Some(t);
    }
    if let Some(t) = packs::export_build(id, p) {
        return Some(t);
    }
    None
}

/// Whether a Compliance+ export is offered for the project, None when no part has an export with that id.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    if let Some(b) = lint::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = precinct::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = mitigation::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = options::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = ftaa::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = speed::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = anop::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = parking::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = subdivision::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = policy::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = gps::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = tou::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = climate::export_enabled(id, p) {
        return Some(b);
    }
    if let Some(b) = packs::export_enabled(id, p) {
        return Some(b);
    }
    None
}
