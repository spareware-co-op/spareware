//! What the simulation reads from an assignment result.

use crate::jsnum::{Maybe, truthy_number};
use crate::results::Res;
use serde_json::Value;
use std::collections::HashMap;

/// The timing the assignment chose for one signal controller (`res.nodes[id].C` and `.split`).
#[derive(Clone, Debug, PartialEq)]
pub struct SignalResult {
    /// Cycle (s); None away from signals.
    pub cycle: Option<f64>,
    /// Green split [phase 0, phase 1]; None away from signals.
    pub split: Option<Vec<f64>>,
}

/// One path of the assignment: arc indexes (into the result's arc keys) in order, and its flow.
#[derive(Clone, Debug, PartialEq)]
pub struct SimPath {
    /// Arc indexes, as `res.arcKeys` lists them.
    pub arcs: Vec<usize>,
    /// Flow (pcu/h).
    pub flow: f64,
}

/// The parts of an assignment result the simulation uses (`res` in sim.js).
///
/// The simulation builds its car routes from the stored paths, takes each signal's optimised
/// cycle and split, runs in the result's period, and compares its own flows with the model's
/// link flows. Build it from the typed result ([`SimResults::from_res`]) or from the result's
/// JSON as the prototype writes it ([`SimResults::from_value`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SimResults {
    /// Period the result is for ("AM", "PM").
    pub period: Option<String>,
    /// Signal timings by controller node id.
    pub signals: HashMap<String, SignalResult>,
    /// Every arc's key by arc index; None when the result has none (a saved project's).
    pub arc_keys: Option<Vec<String>>,
    /// Every path of every OD pair, in OD order; None when the result has none (over 300 zones, or saved).
    pub paths: Option<Vec<SimPath>>,
    /// Modelled flow (veh/h) by arc key.
    pub arc_flow: HashMap<String, f64>,
}

// A number as tools/transport-truth writes it: a JSON number, or the text NaN, Infinity or -Infinity
fn plain_num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        },
        _ => None,
    }
}

impl SimResults {
    /// Reads a typed result (`Engine.assign`'s answer) for the simulation.
    pub fn from_res(res: &Res) -> SimResults {
        SimResults {
            period: Some(res.period.clone()).filter(|p| !p.is_empty()),
            signals: res
                .nodes
                .iter()
                .map(|(id, n)| {
                    (
                        id.to_string(),
                        SignalResult {
                            cycle: n.c,
                            split: n.split.clone(),
                        },
                    )
                })
                .collect(),
            arc_keys: res.arc_keys.clone(),
            paths: match &res.paths {
                Maybe::Set(ods) => Some(
                    ods.iter()
                        .flat_map(|od| od.list.iter())
                        .map(|p| SimPath {
                            arcs: p.arcs.clone(),
                            flow: p.w,
                        })
                        .collect(),
                ),
                _ => None,
            },
            arc_flow: res.arcs.iter().map(|(k, a)| (k.to_string(), a.v)).collect(),
        }
    }

    /// Reads a result from its JSON as the prototype writes it (numbers, with NaN and infinities as text).
    pub fn from_value(res: &Value) -> SimResults {
        let signals = match res.get("nodes") {
            Some(Value::Object(nodes)) => nodes
                .iter()
                .map(|(id, n)| {
                    (
                        id.clone(),
                        SignalResult {
                            cycle: n.get("C").and_then(plain_num),
                            split: match n.get("split") {
                                Some(Value::Array(a)) => Some(
                                    a.iter().map(|x| plain_num(x).unwrap_or(f64::NAN)).collect(),
                                ),
                                _ => None,
                            },
                        },
                    )
                })
                .collect(),
            _ => HashMap::new(),
        };
        let arc_keys = match res.get("arcKeys") {
            Some(Value::Array(a)) => Some(
                a.iter()
                    .map(|k| k.as_str().unwrap_or_default().to_string())
                    .collect(),
            ),
            _ => None,
        };
        let paths = match res.get("paths") {
            Some(Value::Array(ods)) => Some(
                ods.iter()
                    .filter_map(|od| od.get("list").and_then(Value::as_array))
                    .flatten()
                    .map(|p| SimPath {
                        arcs: p["arcs"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .map(|x| x.as_f64().unwrap_or(f64::NAN) as usize)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        flow: plain_num(&p["w"]).unwrap_or(f64::NAN),
                    })
                    .collect(),
            ),
            _ => None,
        };
        let arc_flow = match res.get("arcs") {
            Some(Value::Object(arcs)) => arcs
                .iter()
                .filter_map(|(k, a)| Some((k.clone(), plain_num(a.get("v")?)?)))
                .collect(),
            _ => HashMap::new(),
        };
        SimResults {
            period: res
                .get("period")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
                .map(str::to_string),
            signals,
            arc_keys,
            paths,
            arc_flow,
        }
    }
}

impl SignalResult {
    /// True when the result carries a cycle the simulation uses (`nr.C` is truthy).
    pub(super) fn has_cycle(&self) -> bool {
        self.cycle.is_some_and(truthy_number)
    }
}
