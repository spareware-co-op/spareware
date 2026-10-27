//! The steps of the sample that belong to other features: official data feeds, junction lane editing, compliance and the report.
//!
//! The prototype calls these through the optional modules `Feeds`, `Junctions`, `Compliance`, `ComplianceExtra` and `Report` (when the page has loaded them).
//! Here each call is a method of [`SampleHooks`]; [`super::Modules`] says which modules are loaded, and a step whose module is not loaded is never called.
//! Every method has a default that does nothing, so a host gives only the parts it has.

use super::SampleError;
use crate::census::Bx;
use crate::project::{Project, Scenario};
use serde_json::{Map, Value};

/// What the flood hazard feed answered (`Feeds.hazards.load`): the layers that came back and the errors of those that did not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hazards {
    /// The layers loaded (kept in the project with the lengths each road has in them).
    pub layers: Vec<Value>,
    /// Why layers did not load; the first is the failure text when none did.
    pub errors: Vec<String>,
}

/// An approach into a junction node (an entry of `Junctions.approaches(sc, nodeId)`): the road into it and the lanes it has there.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneApproach {
    /// The link id.
    pub link: String,
    /// The general (not bus) lanes into the node, at least 1.
    pub gl: f64,
}

/// The calls the sample makes into the feeds, junction tools, compliance and report features.
#[allow(unused_variables)]
pub trait SampleHooks {
    /// `Feeds.nslr.all(near, [src], status)`: the legal speed limits of the roads of the first scenario.
    fn nslr_all(&mut self, p: &mut Project, near: &Bx, scenario: usize, status: &mut dyn FnMut(&str)) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Feeds.tms.load(Feeds.tms.sites(), { status })`: the NZTA TMS daily counts of the count sites loaded.
    fn tms_load(&mut self, p: &mut Project, status: &mut dyn FnMut(&str)) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Feeds.hazards.load(near, { status })`: the flood plains and coastal inundation areas.
    fn hazards_load(&mut self, p: &mut Project, near: &Bx, status: &mut dyn FnMut(&str)) -> Result<Hazards, SampleError> {
        Ok(Hazards::default())
    }
    /// `Feeds.hazards.links(sc)`: the length of each road in each hazard layer, for one scenario.
    fn hazard_rows(&mut self, p: &Project, scenario: usize) -> Value {
        Value::Null
    }
    /// `Feeds.gtfs.catalogue(near)`: the first bus timetable that covers the area (None when none does).
    fn gtfs_first_feed(&mut self, near: &Bx) -> Result<Option<Value>, SampleError> {
        Ok(None)
    }
    /// `Feeds.gtfs.load`, `routes`, `apply` and `keep`: loads the timetable, puts its bus routes in the scenario and keeps what they use; gives the number of routes.
    fn gtfs_apply(&mut self, p: &mut Project, feed: &Value, near: &Bx, scenario: usize, status: &mut dyn FnMut(&str)) -> Result<usize, SampleError> {
        Ok(0)
    }
    /// `Junctions.approaches(sc, nodeId)`: the roads into a junction node that carry traffic into it (not connectors), with their lanes.
    fn approaches(&mut self, sc: &Scenario, node_id: &str) -> Vec<LaneApproach> {
        Vec::new()
    }
    /// `Compliance.state(P)` then `Object.assign(C.ctx, ctx)` and, when given, `Object.assign(C.site, site)`.
    fn compliance_context(&mut self, p: &mut Project, ctx: &Map<String, Value>, site: Option<&Map<String, Value>>) {}
    /// `Compliance.lookupSite(P)`: the site's zone, precinct and FDS area from Auckland Council.
    fn lookup_site(&mut self, p: &mut Project) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Compliance.loadSH(P, { sc })`: the NZTA state highway network, for one scenario.
    fn load_sh(&mut self, p: &mut Project, scenario: usize) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Compliance.loadOverlays(P, { sc })`: the Auckland Unitary Plan arterial and access controls, for one scenario.
    fn load_overlays(&mut self, p: &mut Project, scenario: usize) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Compliance.recordOutputs(P)`: writes what the project produced into the compliance record.
    fn record_outputs(&mut self, p: &mut Project) {}
    /// What the features' results listeners do after a model run of a scenario and period (`Registry.fire('results', ...)`), other than the calibration feature's, which the sample does itself.
    fn on_results(&mut self, p: &mut Project, scenario: usize, period: &str) {}
    /// `ComplianceExtra.data(P).precinct`: sets the precinct id, the register of its committed growth and the proposal.
    fn precinct(&mut self, p: &mut Project, id: &str, register: Vec<Value>, proposal: Value) {}
    /// `ComplianceExtra.seedMitigation(P)`: seeds the mitigation register from the modelled effects and the mitigation scenario.
    fn seed_mitigation(&mut self, p: &mut Project) -> Result<(), SampleError> {
        Ok(())
    }
    /// `ComplianceExtra.data(P).mitigation`: the mitigation register.
    fn mitigation(&mut self, p: &mut Project) -> Vec<Value> {
        Vec::new()
    }
    /// Replaces `ComplianceExtra.data(P).mitigation`.
    fn set_mitigation(&mut self, p: &mut Project, list: Vec<Value>) {}
    /// `ComplianceExtra.effects(P)`: the modelled effects, each with an `id` and a `dos` or `vc`.
    fn effects(&mut self, p: &mut Project) -> Result<Vec<Value>, SampleError> {
        Ok(Vec::new())
    }
    /// `ComplianceExtra.seedConditions(P)`: seeds the condition parameters.
    fn seed_conditions(&mut self, p: &mut Project) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Report.staging(P, { steps, onProgress })`: the proposal at each build-out step. `progress(k, n, text)` is called as each step finishes and gives an error when the build was cancelled.
    fn staging(&mut self, p: &mut Project, steps: &[f64], progress: &mut dyn FnMut(usize, usize, &str) -> Result<(), SampleError>) -> Result<(), SampleError> {
        Ok(())
    }
    /// `Report.sensitivity(P, { tests, onProgress })`: the sensitivity tests. `progress` is as for [`SampleHooks::staging`].
    fn sensitivity(&mut self, p: &mut Project, tests: &[&str], progress: &mut dyn FnMut(usize, usize, &str) -> Result<(), SampleError>) -> Result<(), SampleError> {
        Ok(())
    }
}

/// Hooks that do nothing: every optional step is left out or empty.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoHooks;

impl SampleHooks for NoHooks {}
