//! The JavaScript's recorded answers (expected/<name>.json) with the results typed, read in file order.
#![allow(dead_code)] // each test file uses its own part of this module

use construct_transport::project::{OrderedMap, Project};
use construct_transport::results::{Res, Results};
use serde::Deserialize;
use serde_json::Value;

/// expected/<name>.json.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    /// Project name.
    pub project: String,
    /// Per scenario, in project order.
    pub scenarios: OrderedMap<ScenarioRec>,
    /// Option against its comparison scenario ("a|b"), by period.
    pub pairs: OrderedMap<OrderedMap<PairRec>>,
    /// Engine.sensitivity(first scenario, { period: 'AM' }).
    pub sensitivity: Value,
    /// Engine.modelStatus(P, period) by period.
    pub model_status: OrderedMap<Value>,
}

/// One scenario's answers.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioRec {
    /// Engine.yearOf(sc).
    pub year_of: Value,
    /// Engine.capital(sc).
    pub capital: Value,
    /// By period.
    pub periods: OrderedMap<PeriodRec>,
    /// Engine.devDistribution(sc, [AM, PM]).
    pub dev_distribution: Value,
}

/// One scenario and period's answers.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodRec {
    /// Engine.assign(sc, { period }).
    pub res: Res,
    /// Engine.travel(sc, res, first zone, second zone).
    #[serde(default)]
    pub travel: Option<Value>,
    /// Engine.isochrone(sc, res, first zone).
    #[serde(default)]
    pub isochrone: Option<Value>,
    /// The busiest arc.
    pub select_link_key: Option<String>,
    /// Engine.selectLink(res, busiest arc).
    pub select_link: Option<Value>,
    /// Engine.lts(L, res) by link id.
    pub lts: Value,
    /// Engine.exportReady(res).
    pub export_ready: Value,
    /// Engine.modelMeta(sc, period, S, res).
    pub model_meta: Value,
}

/// An option against its comparison scenario in one period.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRec {
    /// Engine.econ(A, B, period).
    pub econ: Value,
    /// Engine.difference(A.res, B.res).
    pub difference: Value,
    /// Engine.withWithout(A.res, B.res).
    pub with_without: Value,
    /// Engine.extentTest(B, A.res, B.res).
    pub extent_test: Value,
}

/// Reads expected/<name>.json.
pub fn load(name: &str) -> Recorded {
    let text = super::support::read_truth(&format!("expected/{name}.json"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("expected/{name}.json: {e}"))
}

impl Recorded {
    /// A scenario's results by period, as `sc.res` held them when gen-expected.mjs ran the queries.
    pub fn results(&self, scenario: &str) -> Results {
        self.scenarios[scenario]
            .periods
            .iter()
            .map(|(p, r)| (p.to_string(), r.res.clone()))
            .collect()
    }
}

/// The project and its recorded answers.
pub fn project_and_record(name: &str) -> (Project, Recorded) {
    (super::support::load_project(name), load(name))
}

/// serde_json value of an output.
pub fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("serializes")
}
