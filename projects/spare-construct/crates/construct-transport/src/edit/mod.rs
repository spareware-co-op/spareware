//! Building and editing a project: what a person does in the prototype's editor, as plain functions.
//!
//! A port of model.js (`Net`, `Sites`, `Register`, `Scenarios`, `migrate` and the undo steps of
//! `Store`), so the same edits can be made from the command line, by an AI agent over MCP or in the
//! browser as WebAssembly. Each function does what the JavaScript does, in the same order, on the typed
//! [`crate::project`] data: network edits ([`net`]), junction sites ([`sites`]), the committed-development
//! register ([`register`]), assessment scenarios ([`scenarios`]), opening an older project ([`migrate`])
//! and undo ([`history`]).
//!
//! Ids and the clock are injected ([`ids`]): every function that makes an id takes an [`IdSource`],
//! so a seeded generator gives the same ids as the JavaScript. Names are ordered with
//! [`collate::locale_compare`], which stands in for `localeCompare`.

pub mod collate;
pub mod history;
pub mod ids;
pub mod json;
pub mod migrate;
pub mod net;
pub mod register;
pub mod scenarios;
pub mod sites;

pub use ids::{IdSource, Ids};

use crate::project::{Project, Scenario};

use std::fmt;

/// An edit that cannot be made (the JavaScript throws a TypeError where this returns one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError(pub String);

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditError {}

/// model.js CONTROLS: junction controls and their names, in the prototype's order.
pub const CONTROLS: [(&str, &str); 5] = [
    ("none", "Uncontrolled"),
    ("priority", "Give way (minor road)"),
    ("stop", "Stop (minor road)"),
    ("roundabout", "Roundabout"),
    ("signals", "Traffic signals"),
];

/// model.js STATUS: link statuses and their names.
pub const STATUS: [(&str, &str); 3] = [
    ("existing", "Existing"),
    ("proposed", "Proposed"),
    ("closed", "Closed"),
];

/// model.js CYCLE: cycling provision and its names.
pub const CYCLE: [(&str, &str); 3] = [
    ("none", "None"),
    ("painted", "Painted cycle lane"),
    ("separated", "Separated cycleway"),
];

/// model.js SPEEDS: the speed limits the editor offers (km/h).
pub const SPEEDS: [f64; 11] = [
    10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0,
];

/// model.js ROUTE_COLOURS: the colours new bus routes take in turn.
pub const ROUTE_COLOURS: [&str; 7] = [
    "#7048e8", "#0c8599", "#e8590c", "#c2255c", "#2f9e44", "#1971c2", "#9c36b5",
];

/// A scenario role of model.js SCENARIO_ROLES.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenarioRole {
    /// Key ("baseline").
    pub key: &'static str,
    /// Full name.
    pub label: &'static str,
    /// Short name.
    pub short: &'static str,
    /// Scenario code ("S1").
    pub code: &'static str,
    /// Position in tables.
    pub order: f64,
    /// "without", "with" or "other" (which side of the proposal it is on).
    pub side: &'static str,
}

/// model.js SCENARIO_ROLES, in its order.
pub const SCENARIO_ROLES: [ScenarioRole; 6] = [
    ScenarioRole {
        key: "existing",
        label: "Existing (base year)",
        short: "Existing",
        code: "S0",
        order: 0.0,
        side: "without",
    },
    ScenarioRole {
        key: "baseline",
        label: "Future baseline (do-minimum)",
        short: "Baseline",
        code: "S1",
        order: 1.0,
        side: "without",
    },
    ScenarioRole {
        key: "proposal",
        label: "With proposal (option)",
        short: "With proposal",
        code: "S2",
        order: 2.0,
        side: "with",
    },
    ScenarioRole {
        key: "mitigation",
        label: "With proposal and mitigation (option)",
        short: "With mitigation",
        code: "S3",
        order: 3.0,
        side: "with",
    },
    ScenarioRole {
        key: "sensitivity",
        label: "Sensitivity test",
        short: "Sensitivity",
        code: "ST",
        order: 4.0,
        side: "other",
    },
    ScenarioRole {
        key: "permitted",
        label: "Permitted baseline (RMA s104(2): may disregard)",
        short: "Permitted baseline",
        code: "PB",
        order: 5.0,
        side: "without",
    },
];

/// `SCENARIO_ROLES[key]`.
pub fn scenario_role(key: &str) -> Option<&'static ScenarioRole> {
    SCENARIO_ROLES.iter().find(|r| r.key == key)
}

/// model.js ZONE_ROLES: zone roles and their names.
pub const ZONE_ROLES: [(&str, &str); 3] = [
    ("existing", "Existing land use"),
    (
        "committed",
        "Committed development (consented, not yet built)",
    ),
    ("development", "Proposed development"),
];

/// model.js LINK_STAGES: link stages and their names.
pub const LINK_STAGES: [(&str, &str); 5] = [
    ("existing", "Existing road"),
    ("committed", "Committed network project"),
    ("unfunded", "Unfunded network project"),
    ("proposal", "Part of the proposal"),
    ("mitigation", "Mitigation"),
];

/// model.js FUNDING: funding states of committed projects and their names.
pub const FUNDING: [(&str, &str); 3] = [
    ("funded", "Funded (NLTP, RLTP or LTP)"),
    ("programmed", "Programmed, not yet funded"),
    ("unfunded", "Unfunded"),
];

/// model.js PATHWAYS: consenting pathways and their names.
pub const PATHWAYS: [(&str, &str); 4] = [
    ("consent", "Resource consent"),
    ("planchange", "Plan change"),
    ("nor", "Notice of requirement or designation"),
    ("ftaa", "Fast-track application"),
];

/// model.js REGISTER_TYPES: committed-development register row types and their names.
pub const REGISTER_TYPES: [(&str, &str); 5] = [
    ("consent", "Granted resource consent"),
    ("ftaa", "Fast-track (FTAA) approval"),
    ("planchange", "Operative plan change"),
    ("precinct", "Precinct yield"),
    ("network", "Committed network project"),
];

/// The name for a key in one of the tables above.
pub fn name_of(table: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// `newScenario(name)`: an empty scenario with a new id (year left unset, so it follows the project year).
pub fn new_scenario(ids: &mut dyn IdSource, name: &str) -> Scenario {
    crate::project::new_scenario(&ids.uid("s"), name)
}

/// `newProject(name)`: a project with default settings and one "Base case" scenario. The scenario's id is made first, then the
/// project's, as the prototype's object literal does.
pub fn new_project(ids: &mut dyn IdSource, name: &str) -> Project {
    let scenario_id = ids.uid("s");
    let project_id = ids.uid("p");
    crate::project::new_project(&project_id, name, &scenario_id)
}
