//! What the tests share: the three example recipes, the default library they are checked
//! against, a way to edit a recipe as JSON, and a recipe holding one of every shape the format
//! has (every constraint, step and symbol form), for the schema agreement test.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; each test file uses part of this fixture"
)]

use construct_categories::CategoryTable;
use construct_families::{Library, Problem, ProblemKind, Recipe};
use construct_materials::Materials;
use serde_json::{Value, json};

/// The single-swing door.
pub const DOOR: &str = include_str!("../../recipes/single-swing-door.json");
/// The fixed window.
pub const WINDOW: &str = include_str!("../../recipes/fixed-window.json");
/// The simple table.
pub const TABLE: &str = include_str!("../../recipes/simple-table.json");
/// Every example, by its file name.
pub const EXAMPLES: [(&str, &str); 3] = [
    ("single-swing-door", DOOR),
    ("fixed-window", WINDOW),
    ("simple-table", TABLE),
];

/// The built-in categories and the default metric materials.
pub struct Fixture {
    pub categories: CategoryTable,
    pub materials: Materials,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            categories: CategoryTable::new(),
            materials: Materials::default_metric(),
        }
    }

    pub fn library(&self) -> Library<'_> {
        Library {
            categories: &self.categories,
            materials: &self.materials,
        }
    }
}

/// A recipe read from its text.
pub fn read(text: &str) -> Recipe {
    Recipe::from_json(text).unwrap()
}

/// The recipe text after an edit to its JSON.
pub fn edit(text: &str, change: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(text).unwrap();
    change(&mut value);
    serde_json::to_string(&value).unwrap()
}

/// What the checker says about a recipe that reads.
pub fn problems(text: &str) -> Vec<Problem> {
    let fixture = Fixture::new();
    match read(text).check(&fixture.library()) {
        Ok(()) => Vec::new(),
        Err(problems) => problems,
    }
}

/// Whether one of the problems is at `at` and of the kind `want` says.
pub fn has(problems: &[Problem], at: &str, want: impl Fn(&ProblemKind) -> bool) -> bool {
    problems
        .iter()
        .any(|problem| problem.at() == at && want(problem.kind()))
}

/// Fails, listing the problems, unless one is at `at` and of the kind `want` says.
pub fn assert_has(problems: &[Problem], at: &str, want: impl Fn(&ProblemKind) -> bool) {
    assert!(
        has(problems, at, want),
        "no such problem at {at}; the problems are: {problems:#?}"
    );
}

/// A recipe in format 1's shape with one of every form the shape has: both kinds of parameter
/// value, a choice, every constraint, every step, a symbol with and without a condition, and a
/// wall opening. It is for the schema and the reader to agree on; it need not mean anything.
#[allow(clippy::too_many_lines, reason = "one document, written out whole")]
pub fn every_shape() -> Value {
    let key2 = |kind: &str| json!({"kind": kind, "a": "p", "b": "q"});
    json!({
        "format": 1,
        "id": "F-1",
        "name": "Every shape",
        "category": 2600,
        "parameters": [
            {
                "key": "size",
                "definition": {
                    "id": "F-1.size", "name": "Size", "kind": "length", "holder": "type",
                    "choices": []
                },
                "default": "length:q0:640000"
            },
            {
                "key": "hand",
                "definition": {
                    "id": "F-1.hand", "name": "Hand", "kind": "choice", "holder": "instance",
                    "choices": [{"key": "left", "label": "Left"}]
                },
                "default": "choice:left"
            },
            {
                "key": "half",
                "definition": {
                    "id": "F-1.half", "name": "Half", "kind": "length", "holder": "type",
                    "choices": []
                },
                "formula": "size / 2"
            }
        ],
        "planes": [{"key": "base", "base": "xy", "offset": "0 mm"}],
        "sketches": [{
            "key": "s",
            "plane": "base",
            "points": [
                {"key": "p", "x": "length:q0:0", "y": "length:q0:0"},
                {"key": "q", "x": "length:q0:640000", "y": "length:q0:0"}
            ],
            "lines": [{"key": "l", "start": "p", "end": "q"}],
            "arcs": [
                {"key": "a", "centre": "p", "start": "q", "end": "q", "radius": "length:q0:640000"}
            ],
            "circles": [{"key": "c", "centre": "p", "radius": "length:q0:320000"}],
            "constraints": [
                key2("coincident"),
                {"kind": "horizontal", "line": "l"},
                {"kind": "vertical", "line": "l"},
                key2("parallel"),
                key2("perpendicular"),
                key2("equal_length"),
                {"kind": "distance", "a": "p", "b": "q", "length": "size"},
                {
                    "kind": "point_line_distance", "point": "p", "line": "l",
                    "length": "half", "side": "left"
                },
                {"kind": "fixed", "point": "p"},
                {"kind": "point_on_line", "point": "p", "line": "l"},
                {"kind": "point_on_curve", "point": "p", "curve": "c"},
                key2("concentric"),
                key2("equal_radius"),
                {"kind": "radius", "curve": "c", "length": "half"},
                {"kind": "diameter", "curve": "c", "length": "size"},
                {"kind": "tangent_line", "line": "l", "curve": "c", "side": "right"},
                {"kind": "tangent_curves", "a": "a", "b": "c", "touch": "first_inside"}
            ]
        }],
        "steps": [
            {
                "key": "one", "op": "extrude", "sketch": "s", "profile": ["l"],
                "holes": [["l"]], "start": "0 mm", "end": "size", "subcategory": 2600,
                "material": {"fixed": "glass-plate"},
                "visibility": {"coarse": true, "medium": true, "fine": false}
            },
            {
                "key": "two", "op": "subtract", "target": "one", "tool": "one",
                "subcategory": 2600, "material": {"parameter": "size"},
                "visibility": {"coarse": false, "medium": true, "fine": true}
            },
            {
                "key": "three", "op": "unite", "a": "one", "b": "two", "subcategory": 2600,
                "material": {"fixed": "glass-plate"},
                "visibility": {"coarse": true, "medium": false, "fine": true}
            }
        ],
        "symbols": [
            {
                "key": "plain", "view": "plan", "sketch": "s", "edges": ["l", "a"],
                "subcategory": 2600,
                "visibility": {"coarse": true, "medium": true, "fine": true}
            },
            {
                "key": "only_left", "view": "elevation", "sketch": "s", "edges": ["c"],
                "subcategory": 2600,
                "visibility": {"coarse": false, "medium": false, "fine": true},
                "when": {"parameter": "hand", "is": "choice:left"}
            }
        ],
        "hosting": {"host": "wall", "opening": {"sketch": "s", "profile": ["l"]}},
        "types": [
            {"key": "small", "name": "Small", "values": [{"parameter": "size", "value": "length:q0:320000"}]}
        ]
    })
}
