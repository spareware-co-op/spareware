//! Undo and redo: the editing steps of model.js `Store`.
//!
//! Before an edit the caller takes a [`History::snapshot`] of the active scenario's editable parts (the network, demand,
//! zones, bus routes, manual OD matrices and the assessment fields: year, junction sites, role, build-out and periods).
//! [`History::undo`] puts the last snapshot back, keeping what it replaced for [`History::redo`], makes that scenario the
//! active one and marks its results out of date, as `Store.undo` does. At most 60 steps are kept, and a new snapshot clears
//! the redo steps. [`mark_stale`] is `Store.edited`: any edit to a scenario makes its results out of date.

use crate::jsnum::{Maybe, Num};
use crate::project::{Link, Node, OdMatrix, OrderedMap, Project, Route, Scenario, Site, Zone};
use serde_json::Value;

/// The most steps kept (`Store.snapshot` drops the oldest past 60).
pub const MAX_STEPS: usize = 60;

/// `editable(sc)`: what undo restores.
#[derive(Clone, Debug, PartialEq)]
struct Editable {
    nodes: OrderedMap<Node>,
    links: OrderedMap<Link>,
    zones: OrderedMap<Zone>,
    routes: OrderedMap<Route>,
    // None when the scenario had no odManual key (undo then leaves it alone)
    od_manual: Option<Maybe<OrderedMap<Maybe<OdMatrix>>>>,
    year: Num,
    sites: Maybe<OrderedMap<Site>>,
    role: Maybe<String>,
    buildout: Num,
    periods: Maybe<Vec<String>>,
}

/// A saved step: the scenario and what it held.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    sid: String,
    data: Editable,
}

/// A missing or null value is dropped (undo deletes the key when it restores null).
fn norm<T: Clone>(m: &Maybe<T>) -> Maybe<T> {
    match m {
        Maybe::Set(v) => Maybe::Set(v.clone()),
        _ => Maybe::Missing,
    }
}

fn editable(sc: &Scenario) -> Editable {
    Editable {
        nodes: sc.nodes.clone(),
        links: sc.links.clone(),
        zones: sc.zones.clone(),
        routes: sc.routes.clone(),
        od_manual: if sc.od_manual.is_missing() {
            None
        } else {
            Some(sc.od_manual.clone())
        },
        year: norm(&sc.year),
        sites: norm(&sc.sites),
        role: norm(&sc.role),
        buildout: norm(&sc.buildout),
        periods: norm(&sc.periods),
    }
}

/// The undo and redo steps of one open project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

/// The index of `Store.sc`: the active scenario, otherwise the first.
fn active_index(p: &Project) -> Option<usize> {
    p.active
        .as_str()
        .and_then(|id| p.scenarios.iter().position(|s| s.id == id))
        .or(if p.scenarios.is_empty() {
            None
        } else {
            Some(0)
        })
}

impl History {
    /// No steps.
    pub fn new() -> History {
        History::default()
    }
    /// Steps that can be undone.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    /// Steps that can be redone.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
    /// Forgets every step (`Store.setProject` and opening a project do this).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
    /// `Store.snapshot()`: saves the active scenario's editable parts before an edit and drops the redo steps.
    pub fn snapshot(&mut self, p: &Project) {
        let Some(i) = active_index(p) else {
            return;
        };
        let sc = &p.scenarios[i];
        self.undo.push(Step {
            sid: sc.id.clone(),
            data: editable(sc),
        });
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    /// `Store.undo()`: puts the last snapshot back; false when there is none (or its scenario is gone, in which case the step is lost).
    pub fn undo(&mut self, p: &mut Project) -> bool {
        swap(&mut self.undo, &mut self.redo, p)
    }
    /// `Store.redo()`: puts back the step the last undo replaced; false when there is none.
    pub fn redo(&mut self, p: &mut Project) -> bool {
        swap(&mut self.redo, &mut self.undo, p)
    }
}

fn swap(from: &mut Vec<Step>, to: &mut Vec<Step>, p: &mut Project) -> bool {
    let Some(u) = from.pop() else {
        return false;
    };
    let Some(i) = p.scenarios.iter().position(|s| s.id == u.sid) else {
        return false;
    };
    let sc = &mut p.scenarios[i];
    to.push(Step {
        sid: sc.id.clone(),
        data: editable(sc),
    });
    let d = u.data;
    sc.nodes = d.nodes;
    sc.links = d.links;
    sc.zones = d.zones;
    sc.routes = d.routes;
    if let Some(od) = d.od_manual {
        sc.od_manual = od;
    }
    sc.year = d.year;
    sc.sites = d.sites;
    sc.role = d.role;
    sc.buildout = d.buildout;
    sc.periods = d.periods;
    let id = sc.id.clone();
    mark_stale(sc);
    p.active = Maybe::Set(id);
    true
}

/// `Store.edited(kind, sc)`: any edit to a scenario's network or demand makes its results out of date (`stale: true` on each).
pub fn mark_stale(sc: &mut Scenario) {
    if let Maybe::Set(Value::Object(res)) = &mut sc.res {
        for r in res.values_mut() {
            if let Value::Object(o) = r {
                o.insert("stale".to_string(), Value::Bool(true));
            }
        }
    }
}
