//! What is stored in the rows: people, hubs, projects, folders, and the model's pieces.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub email: String,
    pub home_hub: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hub {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub hub: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Team {
    pub id: String,
    pub name: String,
}

/// A model in a team's WIP folder. `package` names what publishing it makes, as in "Arch package".
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub team: String,
    pub package: String,
}

/// Where a consumed package came from.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Origin {
    pub hub: String,
    pub hub_name: String,
    pub project: String,
    pub team: String,
    pub package: String,
}

/// A numbered package in a Shared or Consumed folder. The bytes are a file in the object store.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub model: String,
    pub seq: u64,
    pub file: String,
    pub sha256: String,
    pub elements: usize,
    pub by: String,
    pub from: Option<Origin>,
    pub bridged_to: Vec<String>,
}

/// A package bridged to a hub, waiting for the team to take it into Consumed.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Incoming {
    pub id: String,
    pub to_team: String,
    pub item: Item,
    pub origin: Origin,
}

/// The three ways a model can link another.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Live,
    Shared,
    Consumed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Link {
    pub id: String,
    pub kind: LinkKind,
    pub name: String,
    /// The model a live link follows.
    pub model: Option<String>,
    /// The team and package id a fixed link points at.
    pub team: Option<String>,
    pub package: Option<String>,
}

/// One element's data. Lengths are whole millimetres so a hash never depends on rounding.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Element {
    pub id: String,
    pub model: String,
    pub kind: String,
    pub name: String,
    pub level: i32,
    pub x: i64,
    pub y: i64,
    pub shape: String,
    pub props: BTreeMap<String, String>,
}

/// A box. The geometry kernel's shapes will take this one's place.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Shape {
    pub w: i64,
    pub d: i64,
    pub h: i64,
}

/// One input change made by an edit.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub element: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Someone is working on an element. It runs out by itself if they stop.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Claim {
    pub person: String,
    pub name: String,
    pub until_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClaimChange {
    pub element: String,
    pub claim: Option<Claim>,
}

/// One entry of the edit log, with the fields note A05 lists: order, who, when, the intent, the
/// input changes and their content ID, and the result.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub seq: u64,
    pub client: String,
    pub counter: u64,
    pub op_id: String,
    pub author: String,
    pub base: u64,
    pub author_ms: u64,
    pub orderer_ms: u64,
    pub model: String,
    pub cmd: String,
    pub version: u32,
    pub args: Value,
    pub changes: Vec<Change>,
    pub claims: Vec<ClaimChange>,
    pub content_id: String,
    /// `None` when the edit applied; otherwise the reason it was turned back.
    pub failed: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The last edit taken from one browser tab.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientMark {
    pub counter: u64,
    pub seq: u64,
}

/// Cells are 5 m squares of a level of a model.
const CELL_MM: i64 = 5000;

pub fn cell_name(model: &str, level: i32, cx: i64, cy: i64) -> String {
    format!("{model}.L{level}.{cx}.{cy}")
}

/// Every cell a box touches.
pub fn cells(element: &Element, shape: &Shape) -> Vec<String> {
    let mut found = Vec::new();
    for cx in element.x.div_euclid(CELL_MM)..=(element.x + shape.w).div_euclid(CELL_MM) {
        for cy in element.y.div_euclid(CELL_MM)..=(element.y + shape.d).div_euclid(CELL_MM) {
            found.push(cell_name(&element.model, element.level, cx, cy));
        }
    }
    found
}

/// Every cell of a level inside a view, the same way round as [`cells`].
pub fn cells_in_view(model: &str, level: i32, x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<String> {
    let mut found = Vec::new();
    for cx in x0.div_euclid(CELL_MM)..=x1.div_euclid(CELL_MM) {
        for cy in y0.div_euclid(CELL_MM)..=y1.div_euclid(CELL_MM) {
            found.push(cell_name(model, level, cx, cy));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn door(x: i64, y: i64) -> Element {
        Element {
            id: "d".into(),
            model: "m".into(),
            kind: "door".into(),
            name: "D".into(),
            level: 2,
            x,
            y,
            shape: "s".into(),
            props: BTreeMap::new(),
        }
    }

    #[test]
    fn a_box_across_a_cell_line_is_in_both_cells() {
        let shape = Shape {
            w: 900,
            d: 100,
            h: 2100,
        };
        assert_eq!(cells(&door(100, 100), &shape), vec!["m.L2.0.0"]);
        assert_eq!(
            cells(&door(4500, 100), &shape),
            vec!["m.L2.0.0", "m.L2.1.0"]
        );
    }

    #[test]
    fn negative_positions_get_their_own_cells() {
        let shape = Shape {
            w: 100,
            d: 100,
            h: 100,
        };
        assert_eq!(cells(&door(-200, -200), &shape), vec!["m.L2.-1.-1"]);
    }

    #[test]
    fn the_same_element_always_has_the_same_hash() {
        let a = sha256_hex(&serde_json::to_vec(&door(1, 2)).unwrap());
        let b = sha256_hex(&serde_json::to_vec(&door(1, 2)).unwrap());
        assert_eq!(a, b);
    }
}
