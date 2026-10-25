//! What the report asks the other feature scripts for. The report reads the compliance, calibration, land-use, data-feed and
//! data-hub features only where they are installed; each method here has the answer for a feature that is not.

use crate::edit::json::JsonValue;
use crate::junction_tools::Features;
use crate::project::Project;
use std::collections::HashMap;

/// `Compliance.regime(P)`: the pathway, lodgement date and regime one source of truth gives every section.
#[derive(Clone, Debug, PartialEq)]
pub struct Regime {
    /// Pathway key.
    pub pathway: String,
    /// Pathway name.
    pub pathway_label: String,
    /// Lodgement date.
    pub date: String,
    /// The regime in words.
    pub text: String,
    /// The regime in short.
    pub short: String,
}

/// A report section another feature adds (`Registry.report`), already rendered.
#[derive(Clone, Debug, PartialEq)]
pub struct ForeignSection {
    /// Section id.
    pub id: String,
    /// Chapter: a number, a letter, a chapter title or a known topic.
    pub chapter: String,
    /// Order within the chapter.
    pub order: f64,
    /// Title, when it has one.
    pub title: Option<String>,
    /// Its HTML.
    pub html: String,
}

/// A feature's rows for "About the data" (`Registry.about`): title, order and rows of three cells.
#[derive(Clone, Debug, PartialEq)]
pub struct AboutEntry {
    /// The feature's title.
    pub title: String,
    /// Order among the features.
    pub order: f64,
    /// Rows: data, source and notes (HTML).
    pub rows: Vec<[String; 3]>,
}

/// An export another feature registers, tried by the data pack.
#[derive(Clone, Debug, PartialEq)]
pub enum ExportOutcome {
    /// Nothing to export yet; the text is the export's label.
    Disabled(String),
    /// The export failed with this message.
    Failed(String),
    /// The export's text.
    Text(String),
}

/// A junction's crash analysis (`DataHub.nodeRisk`): its name and the nodes in it.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRiskInfo {
    /// Name.
    pub name: String,
    /// Member node ids.
    pub nodes: Vec<String>,
}

/// A link's crash analysis (`DataHub.linkRisk`): the two-way daily volume it was measured against and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRiskInfo {
    /// Two-way AADT, when there is one.
    pub aadt: Option<f64>,
    /// Source of the volume.
    pub aadt_src: Option<String>,
}

/// The feature scripts the report reads. Every method has the answer for a feature that is not installed, so the default host
/// is a project with the report alone.
pub trait ReportHost: Features {
    /// `Compliance.regime(P)`: None when the compliance feature is not installed.
    fn regime(&self, _project: &Project) -> Option<Regime> {
        None
    }
    /// `Compliance.stamp(P).footer`: the version stamp, or an empty text.
    fn stamp_footer(&self, _project: &Project) -> String {
        String::new()
    }
    /// `ComplianceExtra.exportBlock(P)`: the scope linter's blocking finding, if any.
    fn export_block(&self, _project: &Project) -> Option<String> {
        None
    }
    /// `Compliance.conflate(sc, items)`: the vehicle access restriction type of each link, from the loaded overlay's items.
    fn conflate(&self, _sc: usize, _items: &JsonValue) -> Option<HashMap<String, String>> {
        None
    }
    /// `LandusePlus.tripTablesHtml(sc, { scope, parts })`: the trip generation tables; None when the land-use feature is not installed.
    fn landuse_trip_tables(&self, _sc: usize, _scope: Option<&JsonValue>, _parts: &[&str]) -> Option<String> {
        None
    }
    /// `LandusePlus.t5Html(sc)`: the gateway distribution table, when the feature has one.
    fn landuse_t5_html(&self, _sc: usize) -> Option<String> {
        None
    }
    /// `LandusePlus.wwHtml(sc, without)`: the junctions without and with the development, when the feature has one.
    fn landuse_ww_html(&self, _sc: usize, _without: usize) -> Option<String> {
        None
    }
    /// Another feature's report section by id (`Registry.get('report', id)` rendered): used for the sections the report prints inside its own.
    fn foreign_section(&self, _id: &str) -> Option<ForeignSection> {
        None
    }
    /// Every report section other features add (`Registry.list('report')` less the report's own and the junction tools').
    fn foreign_sections(&self) -> Vec<ForeignSection> {
        Vec::new()
    }
    /// The "About the data" rows other features register, beyond the junction tools' and the report's own.
    fn about_entries(&self) -> Vec<AboutEntry> {
        Vec::new()
    }
    /// An export another feature registers (`Registry.get('exports', id)` built for the assessed scenario); None when not installed.
    fn registered_export(&self, _id: &str) -> Option<ExportOutcome> {
        None
    }
    /// `Registry.get('modes', id)`: whether a map colour mode exists.
    fn has_mode(&self, _id: &str) -> bool {
        false
    }
    /// `DataHub.junctionOf(sc, nodeId)`: the key of the junction a node belongs to; None with no crashes loaded.
    fn junction_of(&self, _sc: usize, _node_id: &str) -> Option<String> {
        None
    }
    /// `DataHub.nodeRisk(sc)`: the crash analysis by junction, in order; None with no crashes loaded.
    fn node_risk(&self, _sc: usize) -> Option<Vec<(String, NodeRiskInfo)>> {
        None
    }
    /// `DataHub.linkRisk(sc)`: the crash analysis by link, in order; None with no crashes loaded.
    fn link_risk(&self, _sc: usize) -> Option<Vec<(String, LinkRiskInfo)>> {
        None
    }
    /// `DataHub.safetySummary(sc).onLinks`: how many crashes lie within 25 m of a modelled road.
    fn crashes_on_links(&self, _sc: usize) -> Option<f64> {
        None
    }
}

/// A host with none of the other features installed.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoHost;

impl Features for NoHost {}

impl ReportHost for NoHost {}
