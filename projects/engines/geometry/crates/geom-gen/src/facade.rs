//! Generators `facade-api` and `facade-mcp`: the facade's API documentation and its MCP tool
//! descriptions, from every operation's schema (piece G100; chapter 22, "Every operation is
//! described as data").
//!
//! What it does: [`api`] writes Markdown, one section per operation of
//! `engine_geometry::operations::OPERATIONS`: its name, version, family and tier, what it does,
//! its inputs and outputs (each with its type, unit and whether it may be left out), the roles it
//! names, the errors it can return (each detail with its base kind and its data), the warnings it
//! can give, the exactness classes it can return, whether it has a batch entry, and its example.
//! [`mcp`] writes JSON: the MCP protocol version the descriptions follow ([`MCP_VERSION`]) and one
//! `Tool` per operation, as MCP's `tools/list` describes a tool: its `name`, `title` and
//! `description`, an `inputSchema` and an `outputSchema` in JSON Schema 2020-12 (MCP's default
//! dialect) built from its fields, `annotations` (a kernel operation is pure, 10.1: it reads
//! nothing outside its inputs and changes nothing, so it is read-only, idempotent and closed-world
//! to the program that calls it), and the kernel's own data under `_meta`
//! (`org.spareware/kernel`: version, family, tier, exactness classes, errors with base kinds,
//! warnings, batch entry, roles). The command layer that serves the tools adds the program's side
//! (sign-in scopes, the program's units), as EXTENDING.md, section 2.2, sets out.
//!
//! Each field's JSON Schema comes from its type and unit ([`field_schema`]): booleans, whole
//! numbers (unsigned ones at least 0), real numbers, text, grid points as arrays of whole grid
//! steps, lists of any of these, and the facade's value types (bodies, curves, surfaces, names)
//! as objects named by their type, whose form the command layer gives. A type with no rule here
//! is an error naming it, so an operation cannot reach the interfaces with a field they cannot
//! describe.
//!
//! Guarantees:
//!
//! - **From the schemas, never by hand.** Both files hold only what the operation list says, so
//!   the documentation and the tools cannot drift from the kernel; CI fails until the committed
//!   files are regenerated after the list changes.
//! - **Valid tool descriptions.** Every tool's `name` is the operation's name, which
//!   `engine-geometry`'s tests hold to MCP's rule for tool names; every `inputSchema` is an object
//!   schema, which MCP requires, closed to fields the schema does not name.
//! - **Deterministic.** Operations in the list's order (name order); JSON objects with their keys
//!   sorted (`serde_json`'s ordered map); `\n` line ends; nothing read from the world.
//!
//! Method: MCP's tool description, version 2026-07-28
//! (<https://modelcontextprotocol.io/specification/2026-07-28/server/tools>; the `Tool` interface
//! in <https://github.com/modelcontextprotocol/modelcontextprotocol/blob/271ecc9accafdd9b83a3c869fa67c22953b2af80/schema/2026-07-28/schema.ts>),
//! with JSON Schema 2020-12 (<https://json-schema.org/draft/2020-12/json-schema-core>) for the
//! inputs and outputs, written with `serde_json`.
//!
//! Cost: linear in the schemas' fields.

use engine_geometry::operations::OPERATIONS;
use engine_geometry::schema::{Class, Family, Field, OpSchema, Operation, Tier, Unit};
use serde_json::{Map, Value, json};

use crate::generator::GenError;

/// The API documentation generator's name.
pub const API_NAME: &str = "facade-api";
/// The MCP tool descriptions generator's name.
pub const MCP_NAME: &str = "facade-mcp";
/// The folder both outputs are written into: the facade's own.
pub const OUTPUT_DIR: &str = "projects/engines/crates/engine-geometry/generated";
/// The API documentation.
pub const API_OUTPUT: &str = "projects/engines/crates/engine-geometry/generated/api.md";
/// The MCP tool descriptions.
pub const MCP_OUTPUT: &str = "projects/engines/crates/engine-geometry/generated/mcp-tools.json";
/// The MCP protocol version the descriptions follow.
pub const MCP_VERSION: &str = "2026-07-28";

/// The API documentation of the facade's operation list.
///
/// # Errors
///
/// None today; a generator's signature.
pub fn generate_api() -> Result<Vec<u8>, GenError> {
    api(OPERATIONS)
}

/// The MCP tool descriptions of the facade's operation list.
///
/// # Errors
///
/// A field whose type has no JSON Schema rule ([`field_schema`]).
pub fn generate_mcp() -> Result<Vec<u8>, GenError> {
    mcp(OPERATIONS)
}

/// A generator failure of [`MCP_NAME`].
fn fail(message: String) -> GenError {
    GenError::Generator {
        name: MCP_NAME.to_string(),
        message,
    }
}

/// A family's name, as chapter 10.2 writes it.
fn family(f: Family) -> &'static str {
    match f {
        Family::Make => "Make",
        Family::TwoD => "2D",
        Family::Sketch => "Sketch",
        Family::Sweep => "Sweep",
        Family::Combine => "Combine",
        Family::Blend => "Blend",
        Family::Offset => "Offset",
        Family::Transform => "Transform",
        Family::Repair => "Repair",
        Family::Query => "Query",
        Family::Draw => "Draw",
        Family::Mesh => "Mesh",
        Family::Manufacture => "Manufacture",
        Family::FreeForm => "Free-form",
        Family::Ground => "Ground",
    }
}

/// An exactness class's name, as chapter 9 writes it.
fn class(c: Class) -> &'static str {
    match c {
        Class::Exact => "Exact",
        Class::Certified => "Certified",
        Class::Approximate => "Approximate",
    }
}

/// A tier, as the documentation writes it.
fn tier(t: Tier) -> String {
    match t {
        Tier::Stable => "Stable".to_string(),
        Tier::Preview { since } => format!("Preview since {since}"),
    }
}

/// A unit in words, or `None` for no unit.
fn unit(u: Unit) -> Option<&'static str> {
    match u {
        Unit::None => None,
        Unit::Length => Some("a length, in grid steps of the frame"),
        Unit::Angle => Some("an angle"),
        Unit::Area => Some("an area"),
        Unit::Volume => Some("a volume"),
        Unit::Parameter => Some("a curve or surface parameter"),
        Unit::Count => Some("a count"),
        Unit::Ratio => Some("a ratio"),
    }
}

/// One Markdown table row, its cells with `|` escaped.
fn row(cells: &[&str]) -> String {
    let cells: Vec<String> = cells.iter().map(|c| c.replace('|', "\\|")).collect();
    format!("| {} |\n", cells.join(" | "))
}

/// A table of fields, or a line saying there are none.
fn fields(out: &mut String, title: &str, list: &[Field]) {
    out.push_str(&format!("**{title}**"));
    if list.is_empty() {
        out.push_str(": none.\n\n");
        return;
    }
    out.push_str("\n\n");
    out.push_str(&row(&["Name", "Type", "Unit", "Optional", "What it is"]));
    out.push_str(&row(&["---", "---", "---", "---", "---"]));
    for f in list {
        out.push_str(&row(&[
            &format!("`{}`", f.name),
            &format!("`{}`", f.ty),
            unit(f.unit).unwrap_or("-"),
            if f.optional { "yes" } else { "no" },
            f.doc,
        ]));
    }
    out.push('\n');
}

/// The API documentation of `ops`.
///
/// # Errors
///
/// None today; a generator's signature.
pub fn api(ops: &[Operation]) -> Result<Vec<u8>, GenError> {
    let mut out = String::new();
    out.push_str(
        "# engine-geometry: the operations\n\n\
         Generated by `geom-gen` (`facade-api`) from `engine_geometry::operations::OPERATIONS`; do \
         not edit. Every operation of the geometry kernel, as its schema describes it (chapter 22 \
         of the kernel architecture). Its MCP tool descriptions are `mcp-tools.json`, beside this \
         file.\n\n",
    );
    if ops.is_empty() {
        out.push_str(
            "No operation is listed yet: each operation piece adds its schema (chapter 30.1, the \
             standard bar).\n",
        );
    }
    for op in ops {
        let s: &OpSchema = op.schema;
        out.push_str(&format!("## `{}`\n\n", s.name));
        out.push_str(&format!(
            "Version {}, family {}, {}.\n\n{}\n\n",
            s.version,
            family(s.family),
            tier(op.tier),
            s.doc
        ));
        fields(&mut out, "Inputs", s.inputs);
        fields(&mut out, "Outputs", s.outputs);
        out.push_str("**Roles**");
        if s.roles.is_empty() {
            out.push_str(": none.\n\n");
        } else {
            out.push_str("\n\n");
            for r in s.roles {
                out.push_str(&format!("- `{}`: {}\n", r.name, r.doc));
            }
            out.push('\n');
        }
        out.push_str("**Errors**");
        if s.errors.is_empty() {
            out.push_str(": none.\n\n");
        } else {
            out.push_str("\n\n");
            out.push_str(&row(&["Detail", "Base kind", "Data", "When"]));
            out.push_str(&row(&["---", "---", "---", "---"]));
            for e in s.errors {
                let data: Vec<String> = e
                    .data
                    .iter()
                    .map(|f| format!("`{}: {}`", f.name, f.ty))
                    .collect();
                let data = if data.is_empty() {
                    "-".to_string()
                } else {
                    data.join(", ")
                };
                out.push_str(&row(&[
                    &format!("`{}`", e.detail),
                    &format!("`{}`", e.kind.name()),
                    &data,
                    e.doc,
                ]));
            }
            out.push('\n');
        }
        out.push_str("**Warnings**");
        if s.diagnostics.is_empty() {
            out.push_str(": none.\n\n");
        } else {
            out.push_str("\n\n");
            for d in s.diagnostics {
                out.push_str(&format!("- `{}`: {}\n", d.code, d.doc));
            }
            out.push('\n');
        }
        let classes: Vec<&str> = s.classes.iter().map(|c| class(*c)).collect();
        out.push_str(&format!(
            "**Exactness**: {}. **Batch entry**: {}.\n\n",
            classes.join(", "),
            if s.batch { "yes" } else { "no" }
        ));
        out.push_str(&format!("**Example**\n\n```text\n{}\n```\n\n", s.example));
    }
    Ok(out.into_bytes())
}

/// JSON Schema 2020-12 for a value of type `ty`, or why there is none.
fn type_schema(ty: &str) -> Result<Value, GenError> {
    let ty = ty.trim();
    if let Some(inner) = ty
        .strip_prefix("&[")
        .or_else(|| ty.strip_prefix('['))
        .and_then(|t| t.strip_suffix(']'))
        .or_else(|| ty.strip_prefix("Vec<").and_then(|t| t.strip_suffix('>')))
    {
        return Ok(json!({ "type": "array", "items": type_schema(inner)? }));
    }
    let schema = match ty {
        "bool" => json!({ "type": "boolean" }),
        "u8" | "u16" | "u32" | "u64" | "usize" | "Steps" => {
            json!({ "type": "integer", "minimum": 0 })
        }
        "i8" | "i16" | "i32" | "i64" | "Coord" => json!({ "type": "integer" }),
        "f64" => json!({ "type": "number" }),
        "&str" | "String" | "Label" => json!({ "type": "string" }),
        "P2" => json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 2,
            "maxItems": 2,
            "description": "a grid point, x and y in whole grid steps"
        }),
        "P3" => json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 3,
            "maxItems": 3,
            "description": "a grid point, x, y and z in whole grid steps"
        }),
        "Body" | "CurveKind" | "SurfaceKind" | "NameId" | "TopoName" | "Role" | "PieceKey"
        | "Exactness" | "GridStep" => json!({
            "type": "object",
            "title": ty,
            "description": format!(
                "a `{ty}`, one of engine-geometry's value types; the command layer gives its form"
            )
        }),
        other => {
            return Err(fail(format!(
                "no JSON Schema for the type `{other}`: add its rule to geom-gen's facade module"
            )));
        }
    };
    Ok(schema)
}

/// JSON Schema 2020-12 for one field: its type's schema, with what the field is and its unit.
///
/// # Errors
///
/// A type with no rule here.
pub fn field_schema(f: &Field) -> Result<Value, GenError> {
    let mut schema = type_schema(f.ty)?;
    let words = match unit(f.unit) {
        Some(u) => format!("{} ({u})", f.doc),
        None => f.doc.to_string(),
    };
    if let Some(object) = schema.as_object_mut() {
        object.insert("description".to_string(), Value::String(words));
    }
    Ok(schema)
}

/// An object schema with one property per field, every field that is not optional required, and
/// no other property.
fn object_schema(list: &[Field]) -> Result<Value, GenError> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for f in list {
        if properties
            .insert(f.name.to_string(), field_schema(f)?)
            .is_some()
        {
            return Err(fail(format!("the field `{}` is named twice", f.name)));
        }
        if !f.optional {
            required.push(Value::String(f.name.to_string()));
        }
    }
    Ok(json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    }))
}

/// One operation's MCP tool description.
fn tool(op: &Operation) -> Result<Value, GenError> {
    let s = op.schema;
    let errors: Vec<Value> = s
        .errors
        .iter()
        .map(|e| json!({ "detail": e.detail, "kind": e.kind.name(), "doc": e.doc }))
        .collect();
    let warnings: Vec<Value> = s
        .diagnostics
        .iter()
        .map(|d| json!({ "code": d.code, "doc": d.doc }))
        .collect();
    let roles: Vec<Value> = s
        .roles
        .iter()
        .map(|r| json!({ "name": r.name, "doc": r.doc }))
        .collect();
    let classes: Vec<&str> = s.classes.iter().map(|c| class(*c)).collect();
    Ok(json!({
        "name": s.name,
        "title": s.name,
        "description": format!(
            "{} (version {}, {} family, {}; exactness: {}.)",
            s.doc,
            s.version,
            family(s.family),
            tier(op.tier),
            classes.join(", ")
        ),
        "inputSchema": object_schema(s.inputs)?,
        "outputSchema": object_schema(s.outputs)?,
        "annotations": {
            "readOnlyHint": true,
            "idempotentHint": true,
            "openWorldHint": false
        },
        "_meta": {
            "org.spareware/kernel": {
                "version": s.version,
                "family": family(s.family),
                "tier": tier(op.tier),
                "exactness": classes,
                "errors": errors,
                "warnings": warnings,
                "roles": roles,
                "batch": s.batch,
                "example": s.example
            }
        }
    }))
}

/// The MCP tool descriptions of `ops`.
///
/// # Errors
///
/// A field whose type has no JSON Schema rule, or a schema that names one field twice.
pub fn mcp(ops: &[Operation]) -> Result<Vec<u8>, GenError> {
    let mut tools = Vec::with_capacity(ops.len());
    for op in ops {
        tools.push(tool(op)?);
    }
    let file = json!({
        "about": "Generated by geom-gen (facade-mcp) from engine_geometry::operations::OPERATIONS; \
                  do not edit. One MCP tool description per operation of the geometry kernel.",
        "protocolVersion": MCP_VERSION,
        "tools": tools
    });
    let mut text = serde_json::to_string_pretty(&file).map_err(|e| fail(e.to_string()))?;
    text.push('\n');
    Ok(text.into_bytes())
}
