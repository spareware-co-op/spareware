//! The rebuild report: why a feature, a parameter, an instance or a joint was not built, as data.
//!
//! When the kernel refuses, its own structured error is kept whole (chapter 22's `OpError`): its
//! base kind, its typed detail, the operation and step, the entities and values it points at, its
//! words and what would make it succeed, a waiting piece named as the remedy. Workshop's own
//! refusals, made before the kernel is asked (a field that isn't a number, a sketch picked from later
//! in the history, a feature the kernel has no route for yet), take the same shape and the kernel's
//! closed set of kinds, so the page and an AI agent read one form whoever refused. A feature that
//! fails because an earlier one is not built names it as its `cause`.
//!
//! The form, as JSON: `{ source, kind, detail, op, entities, evidence: [{ what, value, mm }],
//! message, remedies: [{ action, text, piece, input }], cause }`, with chapter 22's field names.
//! `source` is `kernel` or `workshop`; `kind` is the kernel's `ErrorKind` name; `detail` the kernel's
//! detail as Rust writes it (`Make(BaseNotConvex)`) or Workshop's own name for it; `mm` gives a
//! length or a point in millimetres where the value is one; `action` is the kernel's name for the
//! remedy (`ChangeInput`, `AwaitPiece`, ...), with its `piece` or `input` where it has one.

use engine_geometry::value::error::{ErrorKind, Evidence, OpError, Remedy, Value};
use serde::Serialize;

use crate::build::STEPS_PER_MM;

/// Why something was not built, in the kernel's structured form.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    /// `kernel` when the kernel refused, `workshop` when Workshop did before asking it.
    pub source: &'static str,
    /// The base kind, one of the kernel's closed set (`ErrorKind::name`).
    pub kind: &'static str,
    /// The typed detail: the kernel's, as Rust writes it, or Workshop's own name for it.
    pub detail: String,
    /// The kernel operation and step that failed, as its path; `None` for Workshop's own.
    pub op: Option<String>,
    /// The faces, edges and vertices involved, by their names, in the kernel's order.
    pub entities: Vec<String>,
    /// The values that explain it.
    pub evidence: Vec<EvidenceRow>,
    /// In plain words.
    pub message: String,
    /// What would make it succeed, in order.
    pub remedies: Vec<RemedyRow>,
    /// The earlier feature, by its id, that this one needs and that is not built.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// One value an error points at.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvidenceRow {
    /// What it is, in plain words.
    pub what: String,
    /// The value, as the kernel writes it.
    pub value: String,
    /// A length (one number) or a point (two or three) in millimetres, where the value is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mm: Option<Vec<f64>>,
}

/// One thing that would make it succeed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemedyRow {
    /// The kernel's name for the remedy: `ChangeInput`, `AwaitPiece`, `Report` and the rest.
    pub action: &'static str,
    /// What to do, as an instruction.
    pub text: String,
    /// The kernel piece to wait for, for `AwaitPiece`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece: Option<String>,
    /// The input to change, for `ChangeInput`: the kernel's name for it, or the field as the page
    /// labels it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

/// Words that start a sentence and end with a full stop.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out = chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    });
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// Grid steps of the parts' step, `Q1`, in millimetres.
#[allow(clippy::cast_precision_loss)] // a coordinate is at most 2^40 - 1 steps, exact in f64
fn on_grid(steps: i128) -> f64 {
    steps as f64 / STEPS_PER_MM
}

fn evidence_row(e: &Evidence) -> EvidenceRow {
    let mm = match e.value {
        Value::Length { steps, step } => {
            // the step's size in quarter-nanometres; millimetres are 4e6 of them
            #[allow(clippy::cast_precision_loss)]
            let v = steps.0 as f64 * f64::from(step.quarter_nm()) / 4.0e6;
            Some(vec![v])
        }
        Value::Coordinate(c) => Some(vec![on_grid(c)]),
        Value::Point2(p) => Some(vec![
            on_grid(i128::from(p.x.get())),
            on_grid(i128::from(p.y.get())),
        ]),
        Value::Point3(p) => Some(vec![
            on_grid(i128::from(p.x.get())),
            on_grid(i128::from(p.y.get())),
            on_grid(i128::from(p.z.get())),
        ]),
        _ => None,
    };
    EvidenceRow {
        what: e.what.to_owned(),
        value: e.value.to_string(),
        mm,
    }
}

fn remedy_row(r: &Remedy) -> RemedyRow {
    let (action, piece, input) = match r {
        Remedy::ChangeInput { input, .. } => ("ChangeInput", None, Some((*input).to_owned())),
        Remedy::Choose { .. } => ("Choose", None, None),
        Remedy::UseRoute { .. } => ("UseRoute", None, None),
        Remedy::Accept { .. } => ("Accept", None, None),
        Remedy::Heal { .. } => ("Heal", None, None),
        Remedy::ChangeFrame => ("ChangeFrame", None, None),
        Remedy::CoarserStep { .. } => ("CoarserStep", None, None),
        Remedy::RaiseBudget { .. } => ("RaiseBudget", None, None),
        Remedy::SplitCall => ("SplitCall", None, None),
        Remedy::Preview => ("Preview", None, None),
        Remedy::AwaitPiece { piece } => ("AwaitPiece", Some((*piece).to_owned()), None),
        Remedy::Report => ("Report", None, None),
        _ => ("Other", None, None),
    };
    RemedyRow {
        action,
        text: sentence(&r.to_string()),
        piece,
        input,
    }
}

impl Report {
    /// The kernel's own error, kept whole.
    #[must_use]
    pub fn kernel(e: &OpError) -> Self {
        Self {
            source: "kernel",
            kind: e.kind().name(),
            detail: format!("{:?}", e.detail()),
            op: Some(e.op().to_string()),
            entities: e.entities().iter().map(ToString::to_string).collect(),
            evidence: e.evidence().iter().map(evidence_row).collect(),
            message: e.message().to_owned(),
            remedies: e.remedies().iter().map(remedy_row).collect(),
            cause: None,
        }
    }

    /// Workshop's own refusal, of the kernel's `kind`, with its own name for the detail.
    #[must_use]
    pub fn workshop(kind: ErrorKind, detail: &str, message: impl Into<String>) -> Self {
        Self {
            source: "workshop",
            kind: kind.name(),
            detail: detail.to_owned(),
            op: None,
            entities: Vec::new(),
            evidence: Vec::new(),
            message: message.into(),
            remedies: Vec::new(),
            cause: None,
        }
    }

    /// The same, with `remedy` after the remedies it has.
    #[must_use]
    pub fn with(mut self, remedy: RemedyRow) -> Self {
        self.remedies.push(remedy);
        self
    }

    /// The same, waiting for each of `pieces` too.
    #[must_use]
    pub fn awaiting(mut self, pieces: &[&str]) -> Self {
        self.remedies.extend(pieces.iter().map(|p| await_piece(p)));
        self
    }
}

/// Change `input` (as the page labels it): `text` says how.
#[must_use]
pub fn change(input: &str, text: impl Into<String>) -> RemedyRow {
    RemedyRow {
        action: "ChangeInput",
        text: sentence(&text.into()),
        piece: None,
        input: Some(input.to_owned()),
    }
}

/// Wait for the kernel piece `piece`, in the kernel's words.
#[must_use]
pub fn await_piece(piece: &str) -> RemedyRow {
    RemedyRow {
        action: "AwaitPiece",
        text: format!("Wait for piece {piece}, which adds a route."),
        piece: Some(piece.to_owned()),
        input: None,
    }
}

/// Report a defect, in the kernel's words.
#[must_use]
pub fn report_it() -> RemedyRow {
    RemedyRow {
        action: "Report",
        text: "Report it: this is a defect.".to_owned(),
        piece: None,
        input: None,
    }
}
