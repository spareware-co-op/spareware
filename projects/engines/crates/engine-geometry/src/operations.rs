//! Every operation the kernel offers, its schema held as data (chapter 22, "Every operation is
//! described as data").
//!
//! What it does: [`OPERATIONS`] lists one [`Operation`] per operation, in name order: its schema
//! and its tier. `geom-gen` writes the API documentation (`generated/api.md`) and the MCP tool
//! descriptions (`generated/mcp-tools.json`) from this list, and CI fails if a committed file has
//! not been regenerated after the list changed, so the interfaces never drift from the kernel and
//! no AI tool is written by hand (22; EXTENDING.md, section 2.2). The operations themselves are
//! re-exported beside the list, so a program reaches them here and never from a `geom-*` crate.
//!
//! Every operation piece adds its entry here as part of chapter 30.1's standard bar. The
//! operations of chapter 10.2's families come with phases 3 to 12; the first, the Make family's
//! primitives and the Transform family, came with piece G101, and the core queries with G102.
//!
//! Guarantees:
//!
//! - **One entry per operation, in name order.** `tests/attack_g100.rs` checks the names are
//!   unique, ascending, and valid MCP tool names, that each schema's version is at least 1, and
//!   that every preview's date is a real date.
//! - **The schemas tell the truth.** `tests/attack_g101.rs` runs each operation and fails on an
//!   error, a role, a class or a charge its schema does not list, and on a listed detail whose base
//!   kind is not the one the kernel gives it.
//!
//! Method: none; a list of constants (`projects/engines/geometry/architecture/22-errors-queries-ai.md`).
//!
//! Cost: none at run time.

pub use geom_make::{
    BoxInput, Copies, Instance, Made, Motion, Moved, PER_ROW, PrismInput, RoundInput, SphereInput,
    TorusInput, cone, cylinder, make_box, mirror, move_by, pattern, prism, rotate, scale, sphere,
    torus,
};

pub use geom_check::query::{ExactnessReport, exactness, validity};
pub use geom_check::{Fault, FaultKind, Group, Level, Report};
pub use geom_name::{NamedFace, faces};
pub use geom_topo::query_ops::{BoundingBox, bounding_box, six_volume, twice_area};

use crate::schema::{
    Charge, Class, ErrorSpec, Family, Field, OpSchema, Operation, Phase, RoleSpec, Tier, Unit,
};
use crate::value::error::ErrorKind;

/// The Make and Transform operations' tier: every one is new, and none has been through a release
/// (25).
const SINCE_G101: Tier = Tier::Preview {
    since: "2026-10-31",
};

/// The result of a primitive or a transform.
const BODY: Field = Field {
    name: "body",
    ty: "Body",
    unit: Unit::None,
    optional: false,
    doc: "The body, published in name order.",
};

/// The result's history.
const HISTORY: Field = Field {
    name: "history",
    ty: "History",
    unit: Unit::None,
    optional: false,
    doc: "Where each region, face, edge and vertex of the result came from.",
};

/// The result's record, or the input's.
const EXACTNESS: Field = Field {
    name: "exactness",
    ty: "Exactness",
    unit: Unit::None,
    optional: false,
    doc: "The body's exactness class and bound.",
};

/// Which transform a result went through.
const MOTION: Field = Field {
    name: "motion",
    ty: "Motion",
    unit: Unit::None,
    optional: false,
    doc: "Which transform this was, for the attribute event Transformed.",
};

/// The least exact result a transform's caller accepts.
const ACCEPT: Field = Field {
    name: "accept",
    ty: "Exactness",
    unit: Unit::None,
    optional: true,
    doc: "The class and bound accepted for a result less exact than the body; left out, none is. \
          An Exact body's record holds no grid step, so a rounded result counts its bound in the \
          step given here.",
};

/// A round primitive's grid step.
const STEP: Field = Field {
    name: "step",
    ty: "GridStep",
    unit: Unit::None,
    optional: false,
    doc: "The grid step the frame's coordinates count, which the result's bound counts too.",
};

/// A solid's outputs.
const MADE: [Field; 3] = [BODY, HISTORY, EXACTNESS];

/// A transformed solid's outputs.
const MOVED: [Field; 4] = [BODY, HISTORY, MOTION, EXACTNESS];

/// A coordinate past the grid's range.
const OUT_OF_RANGE: ErrorSpec = ErrorSpec {
    detail: "OutOfRange",
    kind: ErrorKind::FrameRangeExceeded,
    data: &[Field {
        name: "value",
        ty: "i128",
        unit: Unit::Length,
        optional: false,
        doc: "The coordinate, in grid steps.",
    }],
    doc: "A corner, a box or an offset would leave the grid's range.",
};

/// The work or memory budget passed.
const BUDGET_EXCEEDED: ErrorSpec = ErrorSpec {
    detail: "BudgetExceeded",
    kind: ErrorKind::BudgetExceeded,
    data: &[],
    doc: "The operation's budget ran out; nothing is returned.",
};

/// A zero extent, radius or height.
const ZERO_LENGTH: ErrorSpec = ErrorSpec {
    detail: "ZeroLength",
    kind: ErrorKind::Degenerate,
    data: &[Field {
        name: "input",
        ty: "&str",
        unit: Unit::None,
        optional: false,
        doc: "The input, by its name in this schema.",
    }],
    doc: "A length the shape needs to be positive is zero.",
};

/// A checker failure on a result that should be valid.
const RESULT_INVALID: ErrorSpec = ErrorSpec {
    detail: "ResultInvalid",
    kind: ErrorKind::Internal,
    data: &[],
    doc: "The result did not pass the checker. A defect: nothing is returned.",
};

/// Rows that could not be assembled.
const BUILD_FAILED: ErrorSpec = ErrorSpec {
    detail: "BuildFailed",
    kind: ErrorKind::Internal,
    data: &[],
    doc: "The body could not be assembled from its rows. A defect.",
};

/// A transform's input with no names.
const BODY_NOT_NAMED: ErrorSpec = ErrorSpec {
    detail: "BodyNotNamed",
    kind: ErrorKind::InvalidInput,
    data: &[],
    doc: "The body was not published in name order, so its entities cannot keep their names.",
};

/// A result less exact than the caller accepted.
const NOT_ACCEPTED: ErrorSpec = ErrorSpec {
    detail: "NotAccepted",
    kind: ErrorKind::ExactnessNotAccepted,
    data: &[Field {
        name: "would",
        ty: "Exactness",
        unit: Unit::None,
        optional: false,
        doc: "The class and bound the result would have.",
    }],
    doc: "The result would be less exact than the class the caller accepted.",
};

/// A kind the checker cannot certify yet.
const KIND_NOT_CERTIFIED: ErrorSpec = ErrorSpec {
    detail: "KindNotCertified",
    kind: ErrorKind::Unsupported,
    data: &[],
    doc: "A transform that does not keep circles would turn a curved face or edge into a kind the \
          checker cannot certify yet.",
};

/// A body the checker cannot certify yet.
const NOT_CERTIFIED_YET: ErrorSpec = ErrorSpec {
    detail: "NotCertifiedYet",
    kind: ErrorKind::Unsupported,
    data: &[Field {
        name: "piece",
        ty: "&str",
        unit: Unit::None,
        optional: false,
        doc: "The piece that widens what the checker certifies.",
    }],
    doc: "Rounded corners or curved faces moved by a certified deviation gave a body the checker \
          cannot certify yet.",
};

/// What every transform can return.
const TRANSFORM_ERRORS: [ErrorSpec; 8] = [
    OUT_OF_RANGE,
    BODY_NOT_NAMED,
    NOT_ACCEPTED,
    KIND_NOT_CERTIFIED,
    NOT_CERTIFIED_YET,
    BUDGET_EXCEEDED,
    RESULT_INVALID,
    BUILD_FAILED,
];

/// The two regions of every solid.
const REGION_SOLID: RoleSpec = RoleSpec {
    name: "region.solid",
    doc: "The solid inside the body.",
};
/// The void around every solid.
const REGION_OUTSIDE: RoleSpec = RoleSpec {
    name: "region.outside",
    doc: "The void outside the body.",
};
/// An edge between two faces.
const EDGE: RoleSpec = RoleSpec {
    name: "edge",
    doc: "An edge, with the names of the two faces it joins.",
};
/// A corner.
const CORNER: RoleSpec = RoleSpec {
    name: "corner",
    doc: "A vertex, with the names of the faces that meet there.",
};
/// The start cap.
const CAP_START: RoleSpec = RoleSpec {
    name: "cap.start",
    doc: "The face at the base.",
};
/// The end cap.
const CAP_END: RoleSpec = RoleSpec {
    name: "cap.end",
    doc: "The face at the top.",
};
/// The whole of a closed face.
const SURFACE: RoleSpec = RoleSpec {
    name: "surface",
    doc: "The body's one face.",
};

/// A transform's body.
const INPUT_BODY: Field = Field {
    name: "body",
    ty: "Body",
    unit: Unit::None,
    optional: false,
    doc: "The body to transform, published in name order.",
};

/// A centre point.
const CENTER: Field = Field {
    name: "center",
    ty: "P3",
    unit: Unit::Length,
    optional: false,
    doc: "The point it stays about.",
};

/// A shape's centre.
const CENTER_OF_SHAPE: Field = Field {
    name: "center",
    ty: "P3",
    unit: Unit::Length,
    optional: false,
    doc: "The centre.",
};

/// A round primitive's base centre.
const BASE: Field = Field {
    name: "base",
    ty: "P3",
    unit: Unit::Length,
    optional: false,
    doc: "The centre of the base. The axis runs along +z from here.",
};

/// A round primitive's radius.
const RADIUS: Field = Field {
    name: "radius",
    ty: "Steps",
    unit: Unit::Length,
    optional: false,
    doc: "The radius of the base, at least one step.",
};

/// An axis.
const AXIS: Field = Field {
    name: "axis",
    ty: "Axis",
    unit: Unit::None,
    optional: false,
    doc: "The coordinate axis it acts along.",
};

/// `make.box`.
const MAKE_BOX: OpSchema = OpSchema {
    name: "make.box",
    version: 1,
    family: Family::Make,
    doc: "A box with faces along the axes, from its least corner and its sizes. Other orientations \
          come from the Transform family.",
    inputs: &[
        Field {
            name: "min",
            ty: "P3",
            unit: Unit::Length,
            optional: false,
            doc: "The corner with the least x, y and z.",
        },
        Field {
            name: "size",
            ty: "[Steps; 3]",
            unit: Unit::Length,
            optional: false,
            doc: "The sizes along x, y and z, each at least one step.",
        },
    ],
    outputs: &MADE,
    roles: &[
        RoleSpec {
            name: "min.x",
            doc: "The face at the least x.",
        },
        RoleSpec {
            name: "max.x",
            doc: "The face at the greatest x.",
        },
        RoleSpec {
            name: "min.y",
            doc: "The face at the least y.",
        },
        RoleSpec {
            name: "max.y",
            doc: "The face at the greatest y.",
        },
        RoleSpec {
            name: "min.z",
            doc: "The face at the least z.",
        },
        RoleSpec {
            name: "max.z",
            doc: "The face at the greatest z.",
        },
        EDGE,
        CORNER,
        REGION_SOLID,
        REGION_OUTSIDE,
    ],
    errors: &[
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[PER_ROW],
    batch: false,
    example: "make.box { min: [0, 0, 0], size: [4000, 200, 2700] } gives a solid of 6 faces, 12 \
              edges and 8 vertices, Exact.",
};

/// `make.cone`.
const MAKE_CONE: OpSchema = OpSchema {
    name: "make.cone",
    version: 1,
    family: Family::Make,
    doc: "A cone on the z axis through its base centre, narrowing to its apex: a base and a side \
          joined by one full circle; the apex has no entity.",
    inputs: &[
        BASE,
        RADIUS,
        Field {
            name: "height",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "The height of the apex above the base, at least one step.",
        },
        STEP,
    ],
    outputs: &MADE,
    roles: &[
        CAP_START,
        RoleSpec {
            name: "side",
            doc: "The curved face.",
        },
        EDGE,
        REGION_SOLID,
        REGION_OUTSIDE,
    ],
    errors: &[
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Certified],
    charges: &[PER_ROW],
    batch: false,
    example: "make.cone { base: [0, 0, 0], radius: 6, height: 12, step: Q0 } gives a solid of 2 \
              faces and 1 edge, Certified within 1 step.",
};

/// `make.cylinder`.
const MAKE_CYLINDER: OpSchema = OpSchema {
    name: "make.cylinder",
    version: 1,
    family: Family::Make,
    doc: "A cylinder on the z axis through its base centre: two caps and a side joined by two full \
          circles without vertices.",
    inputs: &[
        BASE,
        RADIUS,
        Field {
            name: "height",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "The height of the top above the base, at least one step.",
        },
        STEP,
    ],
    outputs: &MADE,
    roles: &[
        CAP_START,
        CAP_END,
        RoleSpec {
            name: "side",
            doc: "The curved face.",
        },
        EDGE,
        REGION_SOLID,
        REGION_OUTSIDE,
    ],
    errors: &[
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Certified],
    charges: &[PER_ROW],
    batch: false,
    example: "make.cylinder { base: [0, 0, 0], radius: 5, height: 10, step: Q0 } gives a solid of 3 \
              faces and 2 edges, Certified within 1 step.",
};

/// `make.prism`.
const MAKE_PRISM: OpSchema = OpSchema {
    name: "make.prism",
    version: 1,
    family: Family::Make,
    doc: "A prism over a strictly convex base polygon, level and counter-clockwise from above, \
          lifted along +z.",
    inputs: &[
        Field {
            name: "base",
            ty: "[P3]",
            unit: Unit::Length,
            optional: false,
            doc: "The base's corners, in order; at least three.",
        },
        Field {
            name: "height",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "How far the top is above the base, at least one step.",
        },
    ],
    outputs: &MADE,
    roles: &[
        CAP_START,
        CAP_END,
        RoleSpec {
            name: "side",
            doc: "A side face, with the index of its base edge as a count.",
        },
        EDGE,
        CORNER,
        REGION_SOLID,
        REGION_OUTSIDE,
    ],
    errors: &[
        ErrorSpec {
            detail: "TooFewCorners",
            kind: ErrorKind::InvalidInput,
            data: &[Field {
                name: "corners",
                ty: "u64",
                unit: Unit::Count,
                optional: false,
                doc: "How many it has.",
            }],
            doc: "The base has fewer than three corners.",
        },
        ErrorSpec {
            detail: "BaseNotLevel",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The base's corners do not all lie at one z.",
        },
        ErrorSpec {
            detail: "BaseNotConvex",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The base is not strictly convex and counter-clockwise from above.",
        },
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Exact],
    charges: &[PER_ROW],
    batch: false,
    example: "make.prism { base: [[0, 0, 0], [10, 0, 0], [12, 7, 0], [3, 11, 0]], height: 9 } gives \
              a solid of 6 faces, Exact.",
};

/// `make.sphere`.
const MAKE_SPHERE: OpSchema = OpSchema {
    name: "make.sphere",
    version: 1,
    family: Family::Make,
    doc: "A sphere: one face with no loops, no edges and no vertices.",
    inputs: &[
        CENTER_OF_SHAPE,
        Field {
            name: "radius",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "The radius, at least one step.",
        },
        STEP,
    ],
    outputs: &MADE,
    roles: &[SURFACE, REGION_SOLID, REGION_OUTSIDE],
    errors: &[
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Certified],
    charges: &[PER_ROW],
    batch: false,
    example: "make.sphere { center: [1, 2, 3], radius: 5, step: Q0 } gives a solid of 1 face, \
              Certified within 1 step.",
};

/// `make.torus`.
const MAKE_TORUS: OpSchema = OpSchema {
    name: "make.torus",
    version: 1,
    family: Family::Make,
    doc: "A ring torus about the z axis: one face, one vertex on its outer equator and two full \
          circles through it. The minor radius is below the major.",
    inputs: &[
        CENTER_OF_SHAPE,
        Field {
            name: "major",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "The radius of the circle through the tube's centres.",
        },
        Field {
            name: "minor",
            ty: "Steps",
            unit: Unit::Length,
            optional: false,
            doc: "The tube's radius, at least one step and below the major radius.",
        },
        STEP,
    ],
    outputs: &MADE,
    roles: &[
        SURFACE,
        RoleSpec {
            name: "vertex",
            doc: "The vertex on the outer equator.",
        },
        RoleSpec {
            name: "parallel",
            doc: "The circle round the axis through the vertex.",
        },
        RoleSpec {
            name: "meridian",
            doc: "The circle in the plane of the axis and the vertex.",
        },
        REGION_SOLID,
        REGION_OUTSIDE,
    ],
    errors: &[
        ErrorSpec {
            detail: "MinorNotBelowMajor",
            kind: ErrorKind::InvalidInput,
            data: &[Field {
                name: "major",
                ty: "u64",
                unit: Unit::Length,
                optional: false,
                doc: "The major radius; the minor radius must be below it.",
            }],
            doc: "It would be a horn or spindle torus, which is not a primitive.",
        },
        ZERO_LENGTH,
        OUT_OF_RANGE,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Certified],
    charges: &[PER_ROW],
    batch: false,
    example: "make.torus { center: [0, 0, 0], major: 10, minor: 3, step: Q0 } gives a solid of 1 \
              face, 2 edges and 1 vertex, Certified within 1 step.",
};

/// `transform.mirror`.
const TRANSFORM_MIRROR: OpSchema = OpSchema {
    name: "transform.mirror",
    version: 1,
    family: Family::Transform,
    doc: "A body mirrored across the plane through a point, perpendicular to an axis. Every entity \
          keeps its name, and every face and loop reverses with it, so solids stay facing outward.",
    inputs: &[
        INPUT_BODY,
        EXACTNESS,
        ACCEPT,
        AXIS,
        Field {
            name: "at",
            ty: "P3",
            unit: Unit::Length,
            optional: false,
            doc: "A point of the mirror's plane.",
        },
    ],
    outputs: &MOVED,
    roles: &[],
    errors: &TRANSFORM_ERRORS,
    diagnostics: &[],
    classes: &[Class::Exact, Class::Certified, Class::Approximate],
    charges: &[PER_ROW],
    batch: false,
    example: "transform.mirror { body, exactness: Exact, axis: x, at: [3, -2, 1] } gives the body \
              mirrored, Exact.",
};

/// `transform.move`.
const TRANSFORM_MOVE: OpSchema = OpSchema {
    name: "transform.move",
    version: 1,
    family: Family::Transform,
    doc: "A body moved by whole grid steps. Every entity keeps its name.",
    inputs: &[
        INPUT_BODY,
        EXACTNESS,
        ACCEPT,
        Field {
            name: "offset",
            ty: "[i64; 3]",
            unit: Unit::Length,
            optional: false,
            doc: "How far it moves along x, y and z.",
        },
    ],
    outputs: &MOVED,
    roles: &[],
    errors: &TRANSFORM_ERRORS,
    diagnostics: &[],
    classes: &[Class::Exact, Class::Certified, Class::Approximate],
    charges: &[PER_ROW],
    batch: false,
    example: "transform.move { body, exactness: Exact, offset: [1, 2, 3] } gives the body moved, \
              Exact.",
};

/// `transform.pattern`.
const TRANSFORM_PATTERN: OpSchema = OpSchema {
    name: "transform.pattern",
    version: 1,
    family: Family::Transform,
    doc: "Copies of one body, one per instance, each by its own transform and keyed by the caller. \
          Every copy's entities have names of their own and trace to the body's as copies. One \
          failure fails the whole pattern.",
    inputs: &[
        INPUT_BODY,
        EXACTNESS,
        ACCEPT,
        Field {
            name: "instances",
            ty: "[Instance]",
            unit: Unit::None,
            optional: false,
            doc: "Each copy's key and transform; at least one, with no key twice.",
        },
    ],
    outputs: &[
        Field {
            name: "copies",
            ty: "Copies",
            unit: Unit::None,
            optional: false,
            doc: "Each instance's key and its body and history, in key order.",
        },
        Field {
            name: "exactness",
            ty: "Exactness",
            unit: Unit::None,
            optional: false,
            doc: "The least exact of the copies' records.",
        },
    ],
    roles: &[RoleSpec {
        name: "copy",
        doc: "A copy of its one source entity, whose name is the role's argument (the attribute \
              event Copied).",
    }],
    errors: &[
        ErrorSpec {
            detail: "EmptyPattern",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The pattern has no instance.",
        },
        ErrorSpec {
            detail: "RepeatedKey",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "Two instances have one key.",
        },
        ErrorSpec {
            detail: "SingularTransform",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "A transform has no inverse.",
        },
        OUT_OF_RANGE,
        BODY_NOT_NAMED,
        NOT_ACCEPTED,
        KIND_NOT_CERTIFIED,
        NOT_CERTIFIED_YET,
        BUDGET_EXCEEDED,
        RESULT_INVALID,
        BUILD_FAILED,
    ],
    diagnostics: &[],
    classes: &[Class::Exact, Class::Certified, Class::Approximate],
    charges: &[PER_ROW],
    batch: false,
    example: "transform.pattern { body, exactness: Exact, instances: [{ key, move by 20 }, { key, \
              move by 40 }] } gives two copies, Exact.",
};

/// `transform.rotate`.
const TRANSFORM_ROTATE: OpSchema = OpSchema {
    name: "transform.rotate",
    version: 1,
    family: Family::Transform,
    doc: "A body rotated about a line along an axis. Quarter turns keep an Exact body Exact; any \
          other rotation rounds each vertex once, and the caller must accept the result's bound.",
    inputs: &[
        INPUT_BODY,
        EXACTNESS,
        ACCEPT,
        AXIS,
        Field {
            name: "rotation",
            ty: "Rotation",
            unit: Unit::Angle,
            optional: false,
            doc: "The rotation: quarter turns, or a direction in whole numbers.",
        },
        CENTER,
    ],
    outputs: &MOVED,
    roles: &[],
    errors: &TRANSFORM_ERRORS,
    diagnostics: &[],
    classes: &[Class::Exact, Class::Certified, Class::Approximate],
    charges: &[PER_ROW],
    batch: false,
    example: "transform.rotate { body, exactness: Exact, axis: z, rotation: 1 quarter turn, center: \
              [1, 1, 0] } gives the body rotated, Exact.",
};

/// `transform.scale`.
const TRANSFORM_SCALE: OpSchema = OpSchema {
    name: "transform.scale",
    version: 1,
    family: Family::Transform,
    doc: "A body scaled along x, y and z about a point. The result's bound grows with the largest \
          factor. Factors that are not all equal on a body with curved geometry wait for the \
          curved checker.",
    inputs: &[
        INPUT_BODY,
        EXACTNESS,
        ACCEPT,
        Field {
            name: "factors",
            ty: "[Scale; 3]",
            unit: Unit::Ratio,
            optional: false,
            doc: "The factors along x, y and z, each a positive fraction.",
        },
        CENTER,
    ],
    outputs: &MOVED,
    roles: &[],
    errors: &TRANSFORM_ERRORS,
    diagnostics: &[],
    classes: &[Class::Exact, Class::Certified, Class::Approximate],
    charges: &[PER_ROW],
    batch: false,
    example: "transform.scale { body, exactness: Exact, factors: [3, 3, 3], center: [1, 1, 1] } \
              gives the body three times the size, Exact.",
};

/// The day the queries entered the facade.
const QUERIES: Tier = Tier::Preview {
    since: "2026-10-31",
};

/// The phase every query charges in (`geom_topo::query_ops::PHASE`).
const QUERY: Phase = Phase::new("query");

/// A body to ask about.
const ASKED: Field = Field {
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

/// A vertex with no grid point for the exact sums.
const IMPLICIT_VERTEX: ErrorSpec = ErrorSpec {
    detail: "ImplicitVertex",
    kind: ErrorKind::Unsupported,
    data: &[Field {
        name: "vertex",
        ty: "u32",
        unit: Unit::None,
        optional: false,
        doc: "The vertex's row.",
    }],
    doc: "A vertex is where three planes meet, so the exact sums have no grid point for it; \
          the rebuild that first makes such a body brings its mass properties with piece G129.",
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
        ASKED,
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
    errors: &[
        MALFORMED,
        CURVED_EDGE,
        IMPLICIT_VERTEX,
        CURVED_FACE,
        NO_LOOPS,
        OVERFLOW,
        BUDGET_EXCEEDED,
    ],
    diagnostics: &[],
    classes: ANY_CLASS,
    charges: &[
        Charge::new(QUERY, "loop of the face", 1),
        Charge::new(QUERY, "fin of its loops", 1),
    ],
    batch: false,
    example: "query.area { body: the box of 10 by 20 by 30, face: its min.z face, exactness: \
              Exact } gives [0, 0, -400] and Exact.",
};

/// `query.bounding_box`.
const BOUNDING_BOX: OpSchema = OpSchema {
    name: "query.bounding_box",
    version: 1,
    family: Family::Query,
    doc: "The box of a body's vertices, joined with the stored box of each face that is not a \
          polygon on a plane, and whether it is tight: every face such a polygon.",
    inputs: &[ASKED, RECORDED],
    outputs: &[Field {
        name: "box",
        ty: "BoundingBox",
        unit: Unit::Length,
        optional: true,
        doc: "The box and whether it is tight; absent for a body with no vertex and no face.",
    }],
    roles: &[],
    errors: &[MALFORMED, BUDGET_EXCEEDED],
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
const QUERY_EXACTNESS: OpSchema = OpSchema {
    name: "query.exactness",
    version: 1,
    family: Family::Query,
    doc: "A body's exactness report: its recorded class and bound, how many faces lie on planes \
          and on other surfaces, how many edges on lines and on other curves, and how the \
          checker's cheap groups judged the record against them.",
    inputs: &[ASKED, RECORDED],
    outputs: &[Field {
        name: "report",
        ty: "ExactnessReport",
        unit: Unit::None,
        optional: false,
        doc: "The report.",
    }],
    roles: &[],
    errors: &[BUDGET_EXCEEDED],
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
        ASKED,
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
    errors: &[MALFORMED, BUDGET_EXCEEDED],
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
        ASKED,
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
    errors: &[BUDGET_EXCEEDED],
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
    inputs: &[ASKED, RECORDED],
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
        IMPLICIT_VERTEX,
        CURVED_FACE,
        NO_LOOPS,
        ErrorSpec {
            detail: "NotSolid",
            kind: ErrorKind::InvalidInput,
            data: &[],
            doc: "The body is not a solid.",
        },
        OVERFLOW,
        BUDGET_EXCEEDED,
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
        schema: &MAKE_BOX,
        tier: SINCE_G101,
    },
    Operation {
        schema: &MAKE_CONE,
        tier: SINCE_G101,
    },
    Operation {
        schema: &MAKE_CYLINDER,
        tier: SINCE_G101,
    },
    Operation {
        schema: &MAKE_PRISM,
        tier: SINCE_G101,
    },
    Operation {
        schema: &MAKE_SPHERE,
        tier: SINCE_G101,
    },
    Operation {
        schema: &MAKE_TORUS,
        tier: SINCE_G101,
    },
    Operation {
        schema: &AREA,
        tier: QUERIES,
    },
    Operation {
        schema: &BOUNDING_BOX,
        tier: QUERIES,
    },
    Operation {
        schema: &QUERY_EXACTNESS,
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
    Operation {
        schema: &TRANSFORM_MIRROR,
        tier: SINCE_G101,
    },
    Operation {
        schema: &TRANSFORM_MOVE,
        tier: SINCE_G101,
    },
    Operation {
        schema: &TRANSFORM_PATTERN,
        tier: SINCE_G101,
    },
    Operation {
        schema: &TRANSFORM_ROTATE,
        tier: SINCE_G101,
    },
    Operation {
        schema: &TRANSFORM_SCALE,
        tier: SINCE_G101,
    },
];
