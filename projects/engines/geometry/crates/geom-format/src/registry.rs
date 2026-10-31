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
        tag: NAMES_TAG,
        name: "names",
        schemas: &[NAMES_SCHEMA],
        flags: REQUIRED,
        cddl: "names.cddl",
        section: "10",
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
        section: "11",
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
        section: "12",
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
        section: "13",
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
        section: "14",
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
        section: "15",
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

/// The registry's table, as `format/REGISTRY.md` holds it.
#[must_use]
pub fn registry_table() -> String {
    let mut out = String::from(
        "| Tag | Name | Schemas | Flags | CDDL | Defined in |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for r in RECORDS {
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
