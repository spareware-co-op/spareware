//! The facade generators (piece G100) on seeded operation lists: the API documentation holds every
//! part of each schema, and the MCP tool descriptions are valid tools whose inputs and outputs are
//! closed object schemas in JSON Schema 2020-12, with every type rule and every refusal shown.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a failure should stop loudly"
)]

use engine_geometry::schema::{
    Class, DiagnosticSpec, ErrorSpec, Family, Field, OpSchema, Operation, RoleSpec, Tier, Unit,
};
use engine_geometry::value::error::ErrorKind;
use geom_gen::facade::{MCP_VERSION, api, field_schema, mcp};
use serde_json::{Value, json};

/// A field.
const fn field(name: &'static str, ty: &'static str, unit: Unit, optional: bool) -> Field {
    Field {
        name,
        ty,
        unit,
        optional,
        doc: "What it is.",
    }
}

static INPUTS: [Field; 3] = [
    field("corner", "P3", Unit::None, false),
    field("size", "u32", Unit::Length, false),
    field("label", "Label", Unit::None, true),
];
static OUTPUTS: [Field; 1] = [field("body", "Body", Unit::None, false)];
static ERRORS: [ErrorSpec; 1] = [ErrorSpec {
    detail: "TooSmall",
    kind: ErrorKind::InvalidInput,
    data: &[field("size", "u32", Unit::Length, false)],
    doc: "The size is 0 | or less.",
}];
static BOX: OpSchema = OpSchema {
    name: "make.box",
    version: 2,
    family: Family::Make,
    doc: "A box from a corner and a size.",
    inputs: &INPUTS,
    outputs: &OUTPUTS,
    roles: &[RoleSpec {
        name: "top",
        doc: "The face at the greatest z.",
    }],
    errors: &ERRORS,
    diagnostics: &[DiagnosticSpec {
        code: "box.thin",
        doc: "The box is thinner than a millimetre.",
    }],
    classes: &[Class::Exact],
    batch: true,
    example: "make.box { corner: [0, 0, 0], size: 10 }",
};
static PLAIN: OpSchema = OpSchema {
    name: "query.count",
    version: 1,
    family: Family::Query,
    doc: "Counts.",
    inputs: &[],
    outputs: &[],
    roles: &[],
    errors: &[],
    diagnostics: &[],
    classes: &[Class::Certified, Class::Approximate],
    batch: false,
    example: "query.count {}",
};

fn ops() -> [Operation; 2] {
    [
        Operation {
            schema: &BOX,
            tier: Tier::Stable,
        },
        Operation {
            schema: &PLAIN,
            tier: Tier::Preview {
                since: "2026-10-31",
            },
        },
    ]
}

#[test]
fn the_documentation_holds_every_part_of_each_schema() {
    let text = String::from_utf8(api(&ops()).unwrap()).unwrap();
    for expected in [
        "## `make.box`",
        "Version 2, family Make, Stable.",
        "A box from a corner and a size.",
        "| `corner` | `P3` | - | no | What it is. |",
        "| `size` | `u32` | a length, in grid steps of the frame | no | What it is. |",
        "| `label` | `Label` | - | yes | What it is. |",
        "| `body` | `Body` | - | no | What it is. |",
        "- `top`: The face at the greatest z.",
        "| `TooSmall` | `InvalidInput` | `size: u32` | The size is 0 \\| or less. |",
        "- `box.thin`: The box is thinner than a millimetre.",
        "**Exactness**: Exact. **Batch entry**: yes.",
        "make.box { corner: [0, 0, 0], size: 10 }",
        "## `query.count`",
        "Version 1, family Query, Preview since 2026-10-31.",
        "**Inputs**: none.",
        "**Outputs**: none.",
        "**Roles**: none.",
        "**Errors**: none.",
        "**Warnings**: none.",
        "**Exactness**: Certified, Approximate. **Batch entry**: no.",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    assert!(!text.contains('\r'));
    let empty = String::from_utf8(api(&[]).unwrap()).unwrap();
    assert!(empty.contains("No operation is listed yet"));
    assert!(!text.contains("No operation is listed yet"));
}

#[test]
fn every_tool_is_a_valid_mcp_tool() {
    let bytes = mcp(&ops()).unwrap();
    assert_eq!(bytes, mcp(&ops()).unwrap(), "the same bytes every time");
    assert!(bytes.ends_with(b"}\n"));
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(file["protocolVersion"], MCP_VERSION);
    let tools = file["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    let tool = &tools[0];
    assert_eq!(tool["name"], "make.box");
    assert_eq!(tool["title"], "make.box");
    assert_eq!(
        tool["description"],
        "A box from a corner and a size. (version 2, Make family, Stable; exactness: Exact.)"
    );
    let input = &tool["inputSchema"];
    assert_eq!(input["type"], "object");
    assert_eq!(input["additionalProperties"], false);
    assert_eq!(input["required"], json!(["corner", "size"]));
    assert_eq!(
        input["properties"]["corner"],
        json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 3,
            "maxItems": 3,
            "description": "What it is."
        })
    );
    assert_eq!(
        input["properties"]["size"],
        json!({
            "type": "integer",
            "minimum": 0,
            "description": "What it is. (a length, in grid steps of the frame)"
        })
    );
    assert_eq!(input["properties"]["label"]["type"], "string");
    assert_eq!(tool["outputSchema"]["properties"]["body"]["title"], "Body");
    assert_eq!(
        tool["annotations"],
        json!({ "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false })
    );
    let meta = &tool["_meta"]["org.spareware/kernel"];
    assert_eq!(meta["version"], 2);
    assert_eq!(meta["family"], "Make");
    assert_eq!(meta["tier"], "Stable");
    assert_eq!(meta["exactness"], json!(["Exact"]));
    assert_eq!(meta["errors"][0]["kind"], "InvalidInput");
    assert_eq!(meta["errors"][0]["detail"], "TooSmall");
    assert_eq!(meta["warnings"][0]["code"], "box.thin");
    assert_eq!(meta["roles"][0]["name"], "top");
    assert_eq!(meta["batch"], true);
    // An operation with no inputs has the closed empty object MCP recommends.
    let plain = &tools[1]["inputSchema"];
    assert_eq!(plain["properties"], json!({}));
    assert_eq!(plain["additionalProperties"], false);
    assert_eq!(
        tools[1]["_meta"]["org.spareware/kernel"]["tier"],
        "Preview since 2026-10-31"
    );
    let empty: Value = serde_json::from_slice(&mcp(&[]).unwrap()).unwrap();
    assert_eq!(empty["tools"], json!([]));
}

#[test]
fn every_type_has_its_rule_and_others_are_refused() {
    let schema = |ty: &'static str| field_schema(&field("x", ty, Unit::None, false));
    assert_eq!(schema("bool").unwrap()["type"], "boolean");
    for unsigned in ["u8", "u16", "u32", "u64", "usize", "Steps"] {
        assert_eq!(schema(unsigned).unwrap()["minimum"], 0, "{unsigned}");
    }
    for signed in ["i8", "i16", "i32", "i64", "Coord"] {
        let s = schema(signed).unwrap();
        assert_eq!(
            (s["type"].clone(), s.get("minimum")),
            (json!("integer"), None),
            "{signed}"
        );
    }
    assert_eq!(schema("f64").unwrap()["type"], "number");
    for text in ["&str", "String", "Label"] {
        assert_eq!(schema(text).unwrap()["type"], "string");
    }
    assert_eq!(schema("P2").unwrap()["maxItems"], 2);
    for list in ["&[P3]", "[P3]", "Vec<P3>"] {
        let s = schema(list).unwrap();
        assert_eq!(s["type"], "array", "{list}");
        assert_eq!(s["items"]["maxItems"], 3, "{list}");
    }
    assert_eq!(
        schema("Vec<&[bool]>").unwrap()["items"]["items"]["type"],
        "boolean"
    );
    for value in [
        "Body",
        "CurveKind",
        "SurfaceKind",
        "NameId",
        "TopoName",
        "Role",
        "PieceKey",
        "Exactness",
        "GridStep",
    ] {
        let s = schema(value).unwrap();
        assert_eq!(
            (s["type"].clone(), s["title"].clone()),
            (json!("object"), json!(value))
        );
    }
    let units = [
        (Unit::Angle, "an angle"),
        (Unit::Area, "an area"),
        (Unit::Volume, "a volume"),
        (Unit::Parameter, "a curve or surface parameter"),
        (Unit::Count, "a count"),
        (Unit::Ratio, "a ratio"),
    ];
    for (unit, words) in units {
        let s = field_schema(&field("x", "f64", unit, false)).unwrap();
        assert_eq!(s["description"], format!("What it is. ({words})"));
    }
    let refused = format!("{:?}", schema("Mystery").unwrap_err());
    assert!(refused.contains("Mystery"), "{refused}");
}

static TWICE: [Field; 2] = [
    field("size", "u32", Unit::Length, false),
    field("size", "u32", Unit::Length, false),
];
static DOUBLE: OpSchema = OpSchema {
    inputs: &TWICE,
    ..PLAIN
};
static UNKNOWN_FIELD: [Field; 1] = [field("x", "Mystery", Unit::None, false)];
static UNKNOWN: OpSchema = OpSchema {
    outputs: &UNKNOWN_FIELD,
    ..PLAIN
};

#[test]
fn a_schema_the_tools_cannot_describe_is_refused() {
    let one = |schema: &'static OpSchema| {
        [Operation {
            schema,
            tier: Tier::Stable,
        }]
    };
    let twice = format!("{:?}", mcp(&one(&DOUBLE)).unwrap_err());
    assert!(twice.contains("named twice"), "{twice}");
    let unknown = format!("{:?}", mcp(&one(&UNKNOWN)).unwrap_err());
    assert!(unknown.contains("Mystery"), "{unknown}");
}
