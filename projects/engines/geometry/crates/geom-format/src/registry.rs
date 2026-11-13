//! The registry of record types, as code: every version-1 record's tag, name, schema, flags, CDDL
//! and fields, from which `format/REGISTRY.md`'s table and `format/SPEC.md`'s field tables are
//! generated (24.2, 24.6).
//!
//! What it does: [`RECORDS`] lists every record type format 1 defines; [`ENVELOPE`] and each
//! type's `fields` describe the fields a reader meets, in order. [`registry_table`] and
//! [`field_table`] render them as the Markdown the specification holds, and the crate's
//! `tests/registry.rs` fails when a committed table differs from what they render (the drift
//! check). [`knows`] is the registry as a reader's `known` predicate.
//!
//! Guarantees:
//!
//! - **One list.** A tag appears once, tags are never reused, and the tables in the specification
//!   are exactly what this list renders.
//! - **The constants agree**: the header's and END's tags and schemas here are the ones
//!   `file` and `header` write (the tests check each).
//!
//! Method: chapter 24.2 and 24.6 of the kernel architecture,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`: "field tables are generated
//! from code annotations".
//!
//! Cost: constant data; rendering allocates the table's text.

use alloc::format;
use alloc::string::String;
use core::fmt::Write as _;

use geom_num::ladder;

use crate::attributes::{ATTRIBUTES_SCHEMA, ATTRIBUTES_TAG};
use crate::file::{END_TAG, HEADER_TAG};
use crate::header::{Header, rung};
use crate::history::{HISTORY_SCHEMA, HISTORY_TAG};
use crate::names::{NAMES_SCHEMA, NAMES_TAG};
use crate::operation::{OPERATION_SCHEMA, OPERATION_TAG};
use crate::pruned::{PRUNED_SCHEMA, PRUNED_TAG};
use crate::recipe::{RECIPE_SCHEMA, RECIPE_TAG};
use crate::record::REQUIRED;
use crate::{body, kinds, stamp};

/// One field of a record or payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// Its name, as the CDDL names it.
    pub name: &'static str,
    /// Its CDDL type.
    pub cddl: &'static str,
    /// What it holds.
    pub meaning: &'static str,
}

/// One record type of format 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordType {
    /// Its tag, never reused.
    pub tag: u64,
    /// Its name.
    pub name: &'static str,
    /// Its schema versions, oldest first.
    pub schemas: &'static [u64],
    /// The flags it is written with.
    pub flags: u64,
    /// Its payload's CDDL file, under `format/cddl/`.
    pub cddl: &'static str,
    /// The section of `format/SPEC.md` that defines it.
    pub section: &'static str,
    /// Its payload's fields, in order.
    pub fields: &'static [Field],
}

/// The record envelope every record shares (24.2).
pub const ENVELOPE: &[Field] = &[
    Field {
        name: "tag",
        cddl: "uint",
        meaning: "the record type, from the registry; never reused",
    },
    Field {
        name: "flags",
        cddl: "uint",
        meaning: "1: required; 2: copy-always; every other bit refused",
    },
    Field {
        name: "schema",
        cddl: "uint",
        meaning: "the layout version of this record type",
    },
    Field {
        name: "depends",
        cddl: "[* content-id]",
        meaning: "the records this one was derived from, a set sorted by the bytes of each encoding",
    },
    Field {
        name: "payload",
        cddl: "bstr",
        meaning: "one item of SKG's canonical CBOR, at most 64 MiB, nested at most 64 deep",
    },
    Field {
        name: "check",
        cddl: "uint",
        meaning: "the CRC-32C of the encoding of the five fields above",
    },
];

/// Every record type of format 1, by tag.
pub const RECORDS: &[RecordType] = &[
    RecordType {
        tag: HEADER_TAG,
        name: "header",
        schemas: &[1],
        flags: REQUIRED,
        cddl: "header.cddl",
        section: "6",
        fields: &[
            Field {
                name: "format",
                cddl: "[major: uint, minor: uint]",
                meaning: "the format version; 1.0 while format 1 is a draft",
            },
            Field {
                name: "edition",
                cddl: "uint",
                meaning: "the kernel edition that wrote the file; 0 before the first release",
            },
            Field {
                name: "name-scheme",
                cddl: "uint",
                meaning: "the NameScheme of every NameId in the file; 1 (32.19)",
            },
            Field {
                name: "ladder",
                cddl: "{ * uint => int }",
                meaning: "the tolerance ladder: rung code to value (6.5); see the rung table",
            },
            Field {
                name: "unicode",
                cddl: "[uint, uint, uint]",
                meaning: "the Unicode version of hashed text; 18.0.0 (32.19)",
            },
            Field {
                name: "program",
                cddl: "tstr",
                meaning: "the program that wrote the file",
            },
            Field {
                name: "extensions",
                cddl: "{ * uint => any }",
                meaning: "an odd key must be understood; an even key may be skipped; none in 1.0",
            },
        ],
    },
    RecordType {
        tag: END_TAG,
        name: "end",
        schemas: &[1],
        flags: REQUIRED,
        cddl: "end.cddl",
        section: "7",
        fields: &[
            Field {
                name: "count",
                cddl: "uint",
                meaning: "the records before this one, the header included",
            },
            Field {
                name: "digest",
                cddl: "bstr .size 32",
                meaning: "the SHA-256 of every byte before this record, the label included",
            },
        ],
    },
    RecordType {
        tag: body::BODY_TAG,
        name: "body",
        schemas: &[body::SCHEMA],
        flags: REQUIRED,
        cddl: "body.cddl",
        section: "10",
        fields: &[
            Field {
                name: "kind",
                cddl: "0..4",
                meaning: "solid, sheet, wire, acorn or general (8.1)",
            },
            Field {
                name: "outside",
                cddl: "uint",
                meaning: "the infinite void region, as a reference: its row plus one",
            },
            Field {
                name: "tables",
                cddl: "[* [table: uint, rows: uint, chunks: [* content-id]]]",
                meaning: "each table by code, in order, with its rows and its chunks' content IDs in order",
            },
            Field {
                name: "named",
                cddl: "0 / 1",
                meaning: "1 if the body was published by name, when tables 12 to 15 hold its names",
            },
            Field {
                name: "extensions",
                cddl: "{}",
                meaning: "none in schema 1",
            },
        ],
    },
    RecordType {
        tag: body::CHUNK_TAG,
        name: "chunk",
        schemas: &[body::SCHEMA],
        flags: REQUIRED,
        cddl: "body.cddl",
        section: "10",
        fields: &[
            Field {
                name: "table",
                cddl: "uint",
                meaning: "the table's code",
            },
            Field {
                name: "position",
                cddl: "uint",
                meaning: "the chunk's place in its table, from 0",
            },
            Field {
                name: "rows",
                cddl: "1..4096",
                meaning: "its rows: 4,096 in every chunk but a table's last",
            },
            Field {
                name: "columns",
                cddl: "[+ refs / i64-column / bstr / [+ surface] / [+ curve]]",
                meaning: "the table's columns over those rows, in the table's order",
            },
        ],
    },
    RecordType {
        tag: stamp::STAMP_TAG,
        name: "stamp",
        schemas: &[stamp::SCHEMA],
        flags: stamp::FLAGS,
        cddl: "stamp.cddl",
        section: "17",
        fields: &[
            Field {
                name: "body",
                cddl: "content-id",
                meaning: "the body index the stamp is about, also its one dependency",
            },
            Field {
                name: "checker",
                cddl: "uint",
                meaning: "the version of the checker that passed the body",
            },
            Field {
                name: "groups",
                cddl: "[* uint]",
                meaning: "the groups of chapter 8.3 it passed, by number, strictly ascending",
            },
            Field {
                name: "edition",
                cddl: "uint",
                meaning: "the kernel edition the check ran under",
            },
        ],
    },
    RecordType {
        tag: NAMES_TAG,
        name: "names",
        schemas: &[NAMES_SCHEMA],
        flags: REQUIRED,
        cddl: "names.cddl",
        section: "12",
        fields: &[Field {
            name: "nodes",
            cddl: "[* [id: name-id, name: topo-name]]",
            meaning: "every name node the file keeps, in NameId order; each id is the first 16 bytes                       of the SHA-256 of the scheme's prefix and the name's canonical bytes (32.19)",
        }],
    },
    RecordType {
        tag: HISTORY_TAG,
        name: "history",
        schemas: &[HISTORY_SCHEMA],
        flags: REQUIRED,
        cddl: "history.cddl",
        section: "13",
        fields: &[
            Field {
                name: "op",
                cddl: "op-path",
                meaning: "the operation's path: its OpId and its labels",
            },
            Field {
                name: "records",
                cddl: "[* [output: name-id, origin]]",
                meaning: "one per output, in output-name order: unchanged, modified, generated, split                           or merged (20.3)",
            },
            Field {
                name: "deleted",
                cddl: "[* name-id]",
                meaning: "the inputs the operation deleted, a set",
            },
            Field {
                name: "changes",
                cddl: "[* topology-change]",
                meaning: "the topology changes it made (13.14), in the history's order",
            },
        ],
    },
    RecordType {
        tag: RECIPE_TAG,
        name: "recipe",
        schemas: &[RECIPE_SCHEMA],
        flags: REQUIRED,
        cddl: "recipe.cddl",
        section: "14",
        fields: &[
            Field {
                name: "owner",
                cddl: "tstr",
                meaning: "the owner's namespace, printable ASCII: `kit.geom` for kernel operations, \
                          `kit.geom.ifc`, a plugin's ID",
            },
            Field {
                name: "owner-schema",
                cddl: "uint",
                meaning: "the owner's schema version for the payload",
            },
            Field {
                name: "op",
                cddl: "op-path",
                meaning: "the operation's path",
            },
            Field {
                name: "version",
                cddl: "uint",
                meaning: "the operation's version (26.2)",
            },
            Field {
                name: "inputs",
                cddl: "[* ([0, content-id] / [1, name-id])]",
                meaning: "the inputs in the owner's order: a body by content ID, an entity by name",
            },
            Field {
                name: "payload",
                cddl: "bstr",
                meaning: "the owner's own encoding of the recipe, never decoded by the format",
            },
            Field {
                name: "outputs",
                cddl: "[* content-id]",
                meaning: "the content IDs of the bodies the recipe produced",
            },
        ],
    },
    RecordType {
        tag: ATTRIBUTES_TAG,
        name: "attributes",
        schemas: &[ATTRIBUTES_SCHEMA],
        flags: REQUIRED,
        cddl: "attributes.cddl",
        section: "15",
        fields: &[
            Field {
                name: "definitions",
                cddl: "[* definition]",
                meaning: "every attribute definition in attribute order: owner, name and version, the \
                          kinds that may carry it, its value schema and its behaviour at 8.4's events",
            },
            Field {
                name: "values",
                cddl: "[* [attr-id, [* [name-id, carried]]]]",
                meaning: "each attribute's values by name: the owner's opaque bytes, or the values a \
                          merge combined",
            },
        ],
    },
    RecordType {
        tag: OPERATION_TAG,
        name: "operation",
        schemas: &[OPERATION_SCHEMA],
        flags: REQUIRED,
        cddl: "operation.cddl",
        section: "16",
        fields: &[
            Field {
                name: "op",
                cddl: "op-path",
                meaning: "the operation's path: its permanent OpId from the log, and labels",
            },
            Field {
                name: "name",
                cddl: "tstr",
                meaning: "its name in the operation schemas, printable ASCII",
            },
            Field {
                name: "version",
                cddl: "uint",
                meaning: "its version, which fixes its results, charges and details (26.2)",
            },
            Field {
                name: "inputs",
                cddl: "[* ([0, content-id] / [1, name-id])]",
                meaning: "its inputs in its schema's order: a body by content ID, an entity by name",
            },
            Field {
                name: "params",
                cddl: "bstr",
                meaning: "its parameters: one item of SKG's canonical CBOR, as its schema writes them",
            },
            Field {
                name: "budget",
                cddl: "[work: uint, logical-bytes: uint]",
                meaning: "the budget it runs under (23.10)",
            },
        ],
    },
    RecordType {
        tag: PRUNED_TAG,
        name: "pruned",
        schemas: &[PRUNED_SCHEMA],
        flags: REQUIRED,
        cddl: "pruned.cddl",
        section: "20",
        fields: &[
            Field {
                name: "pruned-by",
                cddl: "op-path",
                meaning: "the `PruneHistory` operation's path",
            },
            Field {
                name: "held",
                cddl: "[* name-id]",
                meaning: "the names its references held, a set",
            },
            Field {
                name: "steps",
                cddl: "[* history]",
                meaning: "each step it cut, oldest first: its path and what it kept, as a history \
                          record's payload",
            },
        ],
    },
];

/// The ladder's rungs in the header, by code: the code, the rung (architecture 6.5) and its unit.
pub const RUNGS: &[(u64, &str, &str)] = &[
    (rung::FIT, "fit bound, ε_fit", "grid steps"),
    (
        rung::PER_OPERATION,
        "per-operation bound, ε_step",
        "grid steps",
    ),
    (
        rung::HEAL,
        "this document's import healing, ε_heal",
        "grid steps",
    ),
    (rung::HEAL_CAP, "the cap on import healing", "grid steps"),
    (rung::SHORT_EDGE_WARNING, "short-edge warning", "grid steps"),
    (
        rung::COMMIT_PER_AXIS,
        "commit rounding per axis",
        "half grid steps",
    ),
    (
        rung::ANGULAR_LOG2,
        "angular bound, ε_ang",
        "log2 of radians",
    ),
];

/// The rung table, with the values this kernel writes (import healing at its default).
#[must_use]
pub fn rung_table() -> String {
    let ladder = Header::v1("", ladder::HEAL_DEFAULT).ladder;
    let mut out = String::from("| Code | Rung | Unit | Value |\n| --- | --- | --- | --- |\n");
    for &(code, rung, unit) in RUNGS {
        let value = ladder.get(&code).copied().unwrap_or_default();
        let _ = writeln!(out, "| {code} | {rung} | {unit} | {value} |");
    }
    out
}

/// Whether this version knows record type `tag` at `schema`.
#[must_use]
pub fn knows(tag: u64, schema: u64) -> bool {
    RECORDS
        .iter()
        .any(|r| r.tag == tag && r.schemas.contains(&schema))
}

/// A body's tables (24.2): code, arena and columns, in the order a chunk holds them.
pub const TABLES: &[(u64, &str, &str)] = &[
    (
        body::table::REGIONS,
        "regions",
        "kind (0 solid, 1 void), first shell",
    ),
    (body::table::SHELLS, "shells", "region, next, first face"),
    (
        body::table::FACES,
        "faces",
        "shell, next, surface, sense, first loop, bound (an `i64` column of six per face)",
    ),
    (body::table::LOOPS, "loops", "face, next, first fin"),
    (
        body::table::FINS,
        "fins",
        "loop, next, previous, edge, sense, radial, pcurve",
    ),
    (body::table::EDGES, "edges", "first fin, start, end, curve"),
    (body::table::VERTICES, "vertices", "point"),
    (
        body::table::POINTS,
        "points",
        "x, y, z in grid steps (an `i64` column of three per point); in a chunk holding a \
         meeting, first a kind per row (0 grid point, 1 meeting), then x, y, z for the grid points \
         only, then three plane references per meeting",
    ),
    (
        body::table::SURFACES,
        "surfaces",
        "one surface kind per row",
    ),
    (body::table::CURVES, "curves", "one 3D curve kind per row"),
    (body::table::PCURVES, "pcurves", "one 2D curve kind per row"),
    (
        body::table::REGION_NAMES,
        "region names",
        "16 bytes per region, one byte string",
    ),
    (
        body::table::FACE_NAMES,
        "face names",
        "16 bytes per face, one byte string",
    ),
    (
        body::table::EDGE_NAMES,
        "edge names",
        "16 bytes per edge, one byte string",
    ),
    (
        body::table::VERTEX_NAMES,
        "vertex names",
        "16 bytes per vertex, one byte string",
    ),
    (
        body::table::PLANES,
        "planes",
        "n_x, n_y, n_z and d of each plane, reduced: four integers per plane, one array; listed \
         only when the body has a meeting",
    ),
];

/// The curve kinds (7.1): code, kind and fields after the code.
pub const CURVES: &[(u64, &str, &str)] = &[
    (
        kinds::curve::NURBS,
        "NURBS",
        "degree, knots, homogeneous points, weights",
    ),
    (
        kinds::curve::LINE,
        "line, ray or segment",
        "origin, direction, domain",
    ),
    (
        kinds::curve::CIRCLE,
        "circle",
        "centre, radius, frame, domain",
    ),
    (kinds::curve::ELLIPSE, "ellipse", "centre, axes, domain"),
    (kinds::curve::HYPERBOLA, "hyperbola", "centre, axes, domain"),
    (kinds::curve::PARABOLA, "parabola", "vertex, axes, domain"),
    (kinds::curve::HELIX, "helix", "centre, helix axes, domain"),
    (
        kinds::curve::POLYNOMIAL,
        "polynomial",
        "coefficients, domain",
    ),
    (kinds::curve::INVOLUTE, "involute", "centre, axes, domain"),
    (
        kinds::curve::BULGE,
        "bulge polyline",
        "vertices, bulges, widths, closed, start",
    ),
    (
        kinds::curve::TRIMMED,
        "trimmed",
        "a leaf kind (codes 1 to 10 and 15), interval",
    ),
    (
        kinds::curve::COMPOSITE,
        "composite",
        "trimmed segments, starts",
    ),
    (
        kinds::curve::OFFSET,
        "offset",
        "trimmed basis, offset law, cached",
    ),
    (
        kinds::curve::ON_SURFACE,
        "on a surface",
        "trimmed pcurve, surface, 3D curve (NURBS fields), error",
    ),
    (
        kinds::curve::TRANSITION,
        "transition",
        "start, direction, length, curvature law, window, reversed",
    ),
];

/// The surface kinds (7.2): code, kind and fields after the code.
pub const SURFACES: &[(u64, &str, &str)] = &[
    (
        kinds::surface::NURBS,
        "NURBS",
        "u degree, u knots, v degree, v knots, homogeneous points, weights",
    ),
    (kinds::surface::PLANE, "plane", "origin, a, b, domain"),
    (
        kinds::surface::CYLINDER,
        "cylinder",
        "centre, radius, frame, domain, axial sign",
    ),
    (
        kinds::surface::CONE,
        "cone",
        "centre, radius, slope, frame, domain, axial sign",
    ),
    (
        kinds::surface::SPHERE,
        "sphere",
        "centre, radius, frame, domain, axial sign",
    ),
    (
        kinds::surface::TORUS,
        "torus",
        "centre, major radius, minor radius, frame, domain, axial sign",
    ),
    (
        kinds::surface::AFFINE,
        "affine image",
        "centre, terms, domain, angular u, angular v",
    ),
    (
        kinds::surface::REVOLUTION,
        "revolution",
        "profile, axis, frame, angles, placement",
    ),
    (
        kinds::surface::EXTRUSION,
        "extrusion",
        "profile, direction, extent, placement",
    ),
    (
        kinds::surface::RULED,
        "ruled",
        "first profile, second profile, extent, sigma, placement",
    ),
    (
        kinds::surface::OFFSET,
        "offset",
        "base, distance, domain, cached",
    ),
];

/// A table of codes, as `format/SPEC.md` holds it: `head` names the second and third columns.
#[must_use]
pub fn code_table(head: (&str, &str), rows: &[(u64, &str, &str)]) -> String {
    let mut out = format!("| Code | {} | {} |\n| --- | --- | --- |\n", head.0, head.1);
    for &(code, name, what) in rows {
        let _ = writeln!(out, "| {code} | {name} | {what} |");
    }
    out
}

/// The registry's table, as `format/REGISTRY.md` holds it.
#[must_use]
pub fn registry_table() -> String {
    table_of(RECORDS)
}

/// The registry's table of `records`, as [`registry_table`] renders [`RECORDS`].
#[must_use]
pub fn table_of(records: &[RecordType]) -> String {
    let mut out = String::from(
        "| Tag | Name | Schemas | Flags | CDDL | Defined in |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for r in records {
        let schemas: String = r
            .schemas
            .iter()
            .map(|s| format!("{s}"))
            .collect::<alloc::vec::Vec<_>>()
            .join(", ");
        let flags = if r.flags & REQUIRED == 0 {
            "optional"
        } else {
            "required"
        };
        let _ = writeln!(
            out,
            "| {} | `{}` | {schemas} | {flags} | `cddl/{}` | SPEC.md, section {} |",
            r.tag, r.name, r.cddl, r.section
        );
    }
    out
}

/// A field table, as `format/SPEC.md` holds it.
#[must_use]
pub fn field_table(fields: &[Field]) -> String {
    let mut out = String::from("| Field | CDDL | Holds |\n| --- | --- | --- |\n");
    for f in fields {
        let _ = writeln!(out, "| `{}` | `{}` | {} |", f.name, f.cddl, f.meaning);
    }
    out
}
