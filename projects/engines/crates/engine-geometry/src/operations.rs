//! Every operation the kernel offers, its schema held as data (chapter 22, "Every operation is
//! described as data").
//!
//! What it does: [`OPERATIONS`] lists one [`Operation`] per operation, in name order: its schema
//! and its tier. `geom-gen` writes the API documentation (`generated/api.md`) and the MCP tool
//! descriptions (`generated/mcp-tools.json`) from this list, and CI fails if a committed file has
//! not been regenerated after the list changed, so the interfaces never drift from the kernel and
//! no AI tool is written by hand (22; EXTENDING.md, section 2.2).
//!
//! Every operation piece adds its entry here as part of chapter 30.1's standard bar. The
//! operations of chapter 10.2's families come with phases 3 to 12: the core queries with piece
//! G102, primitives and transforms with piece G101.
//!
//! Guarantees:
//!
//! - **One entry per operation, in name order.** `tests/attack_g100.rs` checks the names are
//!   unique, ascending, and valid MCP tool names, that each schema's version is at least 1, and
//!   that every preview's date is a real date.
//!
//! Method: none; a list of constants (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: none at run time.

use crate::schema::{
    Charge, Class, ErrorKind, ErrorSpec, Family, Field, OpSchema, Operation, Phase, Tier, Unit,
};

/// The day the queries entered the facade.
const QUERIES: Tier = Tier::Preview {
    since: "2026-10-31",
};

/// The phase every query charges in (`geom_topo::query_ops::PHASE`).
const QUERY: Phase = Phase::new("query");

/// A body to ask about.
const BODY: Field = Field {
    name: "body",
    ty: "Body",
    unit: Unit::None,
    optional: false,
    doc: "The body asked about.",
};

/// What a body is recorded with: bodies do not hold it themselves (24.4).
const RECORDED: Field = Field {
    name: "exactness",
    ty: "Exactness",
    unit: Unit::None,
    optional: false,
    doc: "The exactness the body is recorded with; the answer states the same class.",
};

/// Every class: a query's class is its body's.
const ANY_CLASS: &[Class] = &[Class::Exact, Class::Certified, Class::Approximate];

/// A body that is not one.
const MALFORMED: ErrorSpec = ErrorSpec {
    detail: "Malformed",
    kind: ErrorKind::InvalidInput,
    data: &[],
    doc: "A link into a row its arena does not have, or a list that does not end.",
};

/// A deterministic budget ran out.
const BUDGET: ErrorSpec = ErrorSpec {
    detail: "BudgetExceeded",
    kind: ErrorKind::BudgetExceeded,
    data: &[
        Field {
            name: "budget",
            ty: "u64",
            unit: Unit::Count,
            optional: false,
            doc: "The limit that ran out.",
        },
        Field {
            name: "kind",
            ty: "&str",
            unit: Unit::None,
            optional: false,
            doc: "Which limit: work or logical bytes.",
        },
        Field {
            name: "phase",
            ty: "&str",
            unit: Unit::None,
            optional: false,
            doc: "The phase, `query`.",
        },
        Field {
            name: "used",
            ty: "u64",
            unit: Unit::Count,
            optional: false,
            doc: "What had been used, as a serial loop counts it.",
        },
    ],
    doc: "The context's work or logical-memory budget ran out; no answer is given.",
};

/// An edge a polygon cannot have.
const CURVED_EDGE: ErrorSpec = ErrorSpec {
    detail: "CurvedEdge",
    kind: ErrorKind::Unsupported,
    data: &[Field {
        name: "edge",
        ty: "u32",
        unit: Unit::None,
        optional: false,
        doc: "The edge's row.",
    }],
    doc: "An edge is not a straight line between two vertices; certified mass properties of \
          curved bodies come with piece G177.",
};

/// A face a polygon cannot lie on.
const CURVED_FACE: ErrorSpec = ErrorSpec {
    detail: "CurvedFace",
    kind: ErrorKind::Unsupported,
    data: &[Field {
        name: "face",
        ty: "u32",
        unit: Unit::None,
        optional: false,
        doc: "The face's row.",
    }],
    doc: "A face is not on a plane; certified mass properties of curved bodies come with piece \
          G177.",
};

/// A face with no polygon.
const NO_LOOPS: ErrorSpec = ErrorSpec {
    detail: "NoLoops",
    kind: ErrorKind::Unsupported,
    data: &[Field {
        name: "face",
        ty: "u32",
        unit: Unit::None,
        optional: false,
        doc: "The face's row.",
    }],
    doc: "A face has no loop, so no polygon to measure; such faces are curved today, and piece \
          G177 measures them.",
};

/// What cannot happen.
const OVERFLOW: ErrorSpec = ErrorSpec {
    detail: "Overflow",
    kind: ErrorKind::Internal,
    data: &[],
    doc: "A sum went past its integer's width: never for a body within its arenas' limits.",
};

/// `query.area`.
const AREA: OpSchema = OpSchema {
    name: "query.area",
    version: 1,
    family: Family::Query,
    doc: "Twice the area vector of a flat face, exact on its grid vertices: Σ pᵢ × pᵢ₊₁ over its \
          loops, holes subtracting. It is normal to the face and points the way its outer loop \
          runs counter-clockwise; half its length is the face's area.",
    inputs: &[
        BODY,
        Field {
            name: "face",
            ty: "u32",
            unit: Unit::None,
            optional: false,
            doc: "The face's row.",
        },
        RECORDED,
    ],
    outputs: &[Field {
        name: "twice_area",
        ty: "AreaVector",
        unit: Unit::Area,
        optional: false,
        doc: "Twice the area vector, in grid steps squared.",
    }],
    roles: &[],
    errors: &[MALFORMED, CURVED_EDGE, CURVED_FACE, NO_LOOPS, OVERFLOW, BUDGET],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "loop of the face", 1),
        Charge::new(QUERY, "fin of its loops", 1),
    ],
    batch: false,
    example: "query.area { body: the box of 10 by 20 by 30, face: 0, exactness: Exact } gives \
              [0, 0, -400], the bottom face, and Exact.",
};

/// `query.bounding_box`.
const BOUNDING_BOX: OpSchema = OpSchema {
    name: "query.bounding_box",
    version: 1,
    family: Family::Query,
    doc: "The box of a body's vertices, joined with the stored box of each face that is not a \
          polygon on a plane, and whether it is tight: every face such a polygon.",
    inputs: &[BODY, RECORDED],
    outputs: &[Field {
        name: "box",
        ty: "BoundingBox",
        unit: Unit::Length,
        optional: true,
        doc: "The box and whether it is tight; absent for a body with no vertex and no face.",
    }],
    roles: &[],
    errors: &[MALFORMED, BUDGET],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "vertex", 1),
        Charge::new(QUERY, "loop", 1),
        Charge::new(QUERY, "fin", 1),
    ],
    batch: false,
    example: "query.bounding_box { body: the box of 10 by 20 by 30 at the origin, exactness: \
              Exact } gives (0, 0, 0) to (10, 20, 30), tight, and Exact.",
};

/// `query.exactness`.
const EXACTNESS: OpSchema = OpSchema {
    name: "query.exactness",
    version: 1,
    family: Family::Query,
    doc: "A body's exactness report: its recorded class and bound, how many faces lie on planes \
          and on other surfaces, how many edges on lines and on other curves, and how the \
          checker's cheap groups judged the record against them.",
    inputs: &[BODY, RECORDED],
    outputs: &[Field {
        name: "report",
        ty: "ExactnessReport",
        unit: Unit::None,
        optional: false,
        doc: "The report.",
    }],
    roles: &[],
    errors: &[BUDGET],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "face", 1),
        Charge::new(QUERY, "edge", 1),
        Charge::new(QUERY, "row of the checker's cheap groups", 1),
    ],
    batch: false,
    example: "query.exactness { body: a cylinder, exactness: Certified } gives 2 flat faces, 1 \
              curved face, 0 straight edges, 2 curved edges, and a report that passed.",
};

/// `query.faces`.
const FACES: OpSchema = OpSchema {
    name: "query.faces",
    version: 1,
    family: Family::Query,
    doc: "Every face of a body in arena order, with its shell, surface, sense and loops, its \
          persistent name, and the roles the given histories trace that name back to. Roles come \
          from the histories alone, never from a face's place or shape.",
    inputs: &[
        BODY,
        Field {
            name: "histories",
            ty: "[History]",
            unit: Unit::None,
            optional: false,
            doc: "The histories of the operations that made the body, oldest first.",
        },
        RECORDED,
    ],
    outputs: &[Field {
        name: "faces",
        ty: "[NamedFace]",
        unit: Unit::None,
        optional: false,
        doc: "One row per face.",
    }],
    roles: &[],
    errors: &[MALFORMED, BUDGET],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "face", 1),
        Charge::new(QUERY, "loop", 1),
        Charge::new(QUERY, "history entry a face's name reaches", 1),
    ],
    batch: false,
    example: "query.faces { body: a box, histories: [its make.box history], exactness: Exact } \
              gives six rows with the roles min.x, max.x, min.y, max.y, min.z and max.z.",
};

/// `query.validity`.
const VALIDITY: OpSchema = OpSchema {
    name: "query.validity",
    version: 1,
    family: Family::Query,
    doc: "A body's validity report: chapter 8.3's checks at the level asked, each fault with the \
          entities it involves, the groups that ran, and how many curved faces and shells left \
          their orientation unjudged.",
    inputs: &[
        BODY,
        RECORDED,
        Field {
            name: "level",
            ty: "Level",
            unit: Unit::None,
            optional: false,
            doc: "Cheap, the groups run after every operation, or Full, every group.",
        },
    ],
    outputs: &[Field {
        name: "report",
        ty: "Report",
        unit: Unit::None,
        optional: false,
        doc: "The report.",
    }],
    roles: &[],
    errors: &[BUDGET],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[Charge::new(QUERY, "row the checker visits", 1)],
    batch: false,
    example: "query.validity { body: a box, exactness: Exact, level: Full } gives a report with \
              no faults, all four groups run and nothing deferred.",
};

/// `query.volume`.
const VOLUME: OpSchema = OpSchema {
    name: "query.volume",
    version: 1,
    family: Family::Query,
    doc: "Six times the volume of a flat solid, exact on its grid vertices: Σ over its faces of \
          the twice-area vector dotted with a vertex, a cavity's shell counting negative.",
    inputs: &[BODY, RECORDED],
    outputs: &[Field {
        name: "six_volume",
        ty: "I256",
        unit: Unit::Volume,
        optional: false,
        doc: "Six times the volume, in grid steps cubed.",
    }],
    roles: &[],
    errors: &[
        MALFORMED,
        CURVED_EDGE,
        CURVED_FACE,
        NO_LOOPS,
        ErrorSpec {
            detail: "NotSolid",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The body is not a solid.",
        },
        OVERFLOW,
        BUDGET,
    ],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "face", 1),
        Charge::new(QUERY, "loop", 1),
        Charge::new(QUERY, "fin", 1),
    ],
    batch: false,
    example: "query.volume { body: the box of 10 by 20 by 30, exactness: Exact } gives 36000 and \
              Exact.",
};

/// Every operation, in name order.
pub const OPERATIONS: &[Operation] = &[
    Operation {
        schema: &AREA,
        tier: QUERIES,
    },
    Operation {
        schema: &BOUNDING_BOX,
        tier: QUERIES,
    },
    Operation {
        schema: &EXACTNESS,
        tier: QUERIES,
    },
    Operation {
        schema: &FACES,
        tier: QUERIES,
    },
    Operation {
        schema: &VALIDITY,
        tier: QUERIES,
    },
    Operation {
        schema: &VOLUME,
        tier: QUERIES,
    },
];
