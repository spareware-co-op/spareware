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

use crate::file::{END_TAG, HEADER_TAG};
use crate::header::{Header, rung};
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
