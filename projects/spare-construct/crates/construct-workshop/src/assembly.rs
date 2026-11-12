//! An assembly posed by the geometry kernel.
//!
//! The assembly, as JSON: `{ parts: [each part's recipe, with its id], assembly: { instances,
//! joints } }`, the form `workshop.assembly` keeps (construct-model's FORMAT.md). Each instance is
//! placed as the page's prototype places it: a grounded part where it was put, and each joint's
//! second part from its first, through the joint's origin, turned (a hinge) or slid (a slider) by
//! the joint's value, held within its limits. Here the kernel does it: every body of every part is
//! moved by `transform.move` and turned by `transform.rotate`, and each instance's box, and the
//! assembly's, is the kernel's `query.bounding_box` of the bodies where they end up.
//!
//! A quarter turn keeps a body Exact. Any other angle is the rotation along the whole-number
//! direction nearest it (within a hundred-millionth of a degree), which rounds each vertex once,
//! and this would accept a bound up to the model's tolerance, 0.01 mm. Today the kernel's checker
//! can't certify a body turned so: rounded corners leave their faces' planes, and a tilted curved
//! face meets its caps where only exact data is followed. It says so
//! (`MakeDetail::NotCertifiedYet`, naming G134 for flat faces and G177 for curved ones), and that
//! instance and its joint wait for the kernel, naming the piece, as a feature does.
//!
//! The kernel runs its full checker after every operation only in debug and test builds; shipped
//! release builds, the browser's among them, run its cheap groups, which pass such a body. So that
//! every build answers the same, a body a turn rounded is checked again here on request, with
//! every group (`query.validity` at `Level::Full`), and waits on the same terms. What else the
//! kernel can't do yet stays out and says so where it is asked for: joints between faces and axes
//! picked on the parts wait for its assembly mates (G479) and persistent face names, and
//! interference for its booleans (G130 to G133).
//!
//! The answer: `{ engine, instances: [{ id, part, state, message, m, exactness, bound, bbox,
//! error }], joints: [{ id, state, value, message, error }], bbox, notes }`. `m` maps the part's own
//! space to where the instance is, 3 rows of 4, row-major, in millimetres, as the page draws it;
//! `state` is `built`, `waits` with the kernel piece it waits for, or `failed` with the kernel's
//! reason, and the page places an instance that isn't built itself. An instance or joint that
//! waits or fails carries its rebuild report (`error`, [`crate::report`]): a turn the checker can't
//! certify yet is the kernel's `NotCertifiedYet` in every build, with the piece as its remedy.
//! `bbox` holds only the built instances.

use std::collections::BTreeMap;

use engine_geometry::operations::{
    Level, Moved, bounding_box, exactness as exactness_of, move_by, rotate, validity,
};
use engine_geometry::value::Exactness;
use engine_geometry::value::body::Body;
use engine_geometry::value::error::{ErrorKind, MakeDetail, OpError, OpErrorDetail};
use engine_geometry::value::grid::{GridStep, Steps};
use engine_geometry::value::transform::{Axis, Rotation};
use serde::Serialize;
use serde_json::{Value, json};

use crate::build::{
    IDENTITY, Refusal, Res, bad, build_with_bodies, class, ctx, defect, fmt_mm, mm, point, refused,
    steps, then, translation,
};
use crate::report::{Report, change};

/// The largest bound accepted for a turned body: 0.01 mm, the model's tolerance, in grid steps.
const ACCEPT_STEPS: u64 = 1600;
/// The scale of a turn's whole-number direction: cos and sin times 2^30 fit `i32`.
const DIRECTION_SCALE: f64 = 1_073_741_824.0;

/// An instance, posed.
#[derive(Debug, Serialize)]
pub struct InstanceRow {
    pub id: String,
    pub part: String,
    /// `built`, or `waits` with the kernel's reason.
    pub state: &'static str,
    pub message: String,
    /// The map from the part's space to the instance's place, millimetres.
    pub m: [f64; 12],
    /// The least exact of its bodies: `Exact` or `Certified`.
    pub exactness: &'static str,
    /// A Certified pose's largest proven bound, in grid steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<u64>,
    /// The kernel's box round its posed bodies, millimetres; `None` when it isn't built.
    pub bbox: Option<[[f64; 3]; 2]>,
    /// Why it waits or fails, as data: the rebuild report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Report>,
}

/// A joint, as posed.
#[derive(Debug, Serialize)]
pub struct JointRow {
    pub id: String,
    /// `built`, `waits` or `failed`.
    pub state: &'static str,
    /// Its value, held within its limits: degrees for a hinge, millimetres for a slider.
    pub value: f64,
    pub message: String,
    /// Why it waits or fails, as data: the rebuild report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Report>,
}

/// What posing an assembly gives.
#[derive(Debug, Serialize)]
pub struct Pose {
    pub engine: &'static str,
    pub instances: Vec<InstanceRow>,
    pub joints: Vec<JointRow>,
    /// The kernel's box round every posed body, millimetres.
    pub bbox: Option<[[f64; 3]; 2]>,
    pub notes: Vec<String>,
}

/// One step of an instance's placing, in the order it is done.
#[derive(Clone, Copy)]
enum Step {
    Move([f64; 3]),
    Turn {
        axis: Axis,
        index: usize,
        rotation: Rotation,
        /// A whole number of quarter turns, which rounds nothing.
        quarter: bool,
        cos: f64,
        sin: f64,
    },
}

fn axis_named(name: &str) -> Option<(Axis, usize)> {
    match name {
        "x" => Some((Axis::X, 0)),
        "y" => Some((Axis::Y, 1)),
        "z" => Some((Axis::Z, 2)),
        _ => None,
    }
}

fn numbers<const N: usize>(v: Option<&Value>) -> [f64; N] {
    let mut out = [0.0; N];
    for (o, x) in out
        .iter_mut()
        .zip(v.and_then(Value::as_array).into_iter().flatten())
    {
        *o = x.as_f64().unwrap_or(0.0);
    }
    out
}

fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// The turn by `degrees`: a quarter turn exactly (and `true`), or the nearest whole-number
/// direction.
fn turn(degrees: f64) -> Res<(Rotation, bool, f64, f64)> {
    let quarters = degrees / 90.0;
    if quarters == quarters.round() && quarters.abs() < 1e9 {
        // a whole number of quarter turns, well inside i32
        #[allow(clippy::cast_possible_truncation)]
        let q = (quarters.round() as i64).rem_euclid(4) as i32;
        let (c, s) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][q as usize];
        return Ok((Rotation::quarter_turns(q), true, c, s));
    }
    let r = degrees.to_radians();
    // |cos|, |sin| <= 1, so each scaled value is within i32
    #[allow(clippy::cast_possible_truncation)]
    let (c, s) = (
        (r.cos() * DIRECTION_SCALE).round() as i32,
        (r.sin() * DIRECTION_SCALE).round() as i32,
    );
    let Ok(rotation) = Rotation::direction(c, s) else {
        return bad(
            ErrorKind::Degenerate,
            "NoDirection",
            "the angle",
            "That angle has no direction.",
            "Change the angle",
        );
    };
    let (c, s) = (f64::from(c), f64::from(s));
    let n = c.hypot(s);
    Ok((rotation, false, c / n, s / n))
}

/// The map of one step, as the page draws it.
fn map_of(step: Step) -> [f64; 12] {
    match step {
        Step::Move(d) => translation(d),
        Step::Turn {
            index, cos, sin, ..
        } => {
            let (c, s) = (cos, sin);
            let r = match index {
                0 => [1., 0., 0., 0., c, -s, 0., s, c],
                1 => [c, 0., s, 0., 1., 0., -s, 0., c],
                _ => [c, -s, 0., s, c, 0., 0., 0., 1.],
            };
            [
                r[0], r[1], r[2], 0., r[3], r[4], r[5], 0., r[6], r[7], r[8], 0.,
            ]
        }
    }
}

/// A turned body the kernel's checker can't certify yet, waiting for `piece`, with the kernel's
/// error as its report.
fn waits_for_checker(piece: &str, error: &OpError) -> Refusal {
    let what = match piece {
        "G134" => "certifying a turned body's flat faces",
        "G177" => "certifying a turned body's curved faces",
        _ => "certifying a body turned so",
    };
    Refusal::waits(
        format!(
            "Turned to this angle, it waits for the kernel's checker: {what} ({piece}). A quarter \
             turn rounds nothing and is posed today."
        ),
        Report::kernel(error).with(change(
            "the angle",
            "Turn it by a whole number of quarter turns, which is posed today",
        )),
    )
}

/// What the kernel's refusal of a step means here: a body its checker can't certify yet waits for
/// the piece it names; anything else fails with the kernel's reason.
fn refused_pose(e: &OpError) -> Refusal {
    match e.detail() {
        OpErrorDetail::Make(MakeDetail::NotCertifiedYet { piece }) => waits_for_checker(piece, e),
        _ => refused(e),
    }
}

/// Every group of the kernel's checker on a body a turn rounded, asked for here so that a release
/// build, which runs only the cheap groups after an operation, answers as a debug build does. As
/// the kernel's transforms do, a rounded body that doesn't pass waits for the piece that widens
/// the checker for its kind: G177 when a face or an edge is curved, G134 otherwise. The error is
/// the one the kernel's transform gives in a debug build, at the turn's step: its detail
/// (`NotCertifiedYet`), its remedy (that piece) and the checker's first fault as its words.
fn check_fully(key: usize, turned_at: u32, body: &Body, exactness: Exactness) -> Res<()> {
    let c = ctx(key, 997);
    let report = validity(&c, body, exactness, Level::Full)
        .map_err(|e| refused(&e))?
        .result;
    if report.passed() {
        return Ok(());
    }
    let kinds = exactness_of(&c, body, exactness)
        .map_err(|e| refused(&e))?
        .result;
    let curved = kinds.curved_faces > 0 || kinds.curved_edges > 0;
    let piece = if curved { "G177" } else { "G134" };
    let mut error = OpError::new(
        ctx(key, turned_at).path().clone(),
        MakeDetail::NotCertifiedYet { piece },
    );
    if let Some(first) = report.faults().first() {
        error = error.with_message(first.to_string());
    }
    Err(waits_for_checker(piece, &error))
}

/// Puts one body through an instance's steps with the kernel.
fn place(key: usize, body: &Body, exactness: Exactness, chain: &[Step]) -> Res<(Body, Exactness)> {
    let accept = Some(Exactness::Certified {
        bound: Steps(ACCEPT_STEPS),
        step: GridStep::Q1,
    });
    let mut now = (body.clone(), exactness);
    // the step of the first turn that rounded, if any
    let mut rounded = None;
    for (n, step) in chain.iter().enumerate() {
        let n = u32::try_from(n).unwrap_or(u32::MAX);
        let c = ctx(key, n);
        let moved = Moved {
            body: &now.0,
            exactness: now.1,
            accept,
        };
        let made = match *step {
            Step::Move(d) => {
                let d = [steps(d[0])?, steps(d[1])?, steps(d[2])?];
                move_by(&c, moved, d)
            }
            Step::Turn {
                axis,
                rotation,
                quarter,
                ..
            } => {
                if !quarter && rounded.is_none() {
                    rounded = Some(n);
                }
                rotate(&c, moved, axis, rotation, point([0, 0, 0])?)
            }
        }
        .map_err(|e| refused_pose(&e))?;
        now = (made.result.body, made.exactness);
    }
    if let Some(turned_at) = rounded {
        check_fully(key, turned_at, &now.0, now.1)?;
    }
    Ok(now)
}

fn union(a: Option<[[f64; 3]; 2]>, b: [[f64; 3]; 2]) -> [[f64; 3]; 2] {
    match a {
        None => b,
        Some(a) => [
            [
                a[0][0].min(b[0][0]),
                a[0][1].min(b[0][1]),
                a[0][2].min(b[0][2]),
            ],
            [
                a[1][0].max(b[1][0]),
                a[1][1].max(b[1][1]),
                a[1][2].max(b[1][2]),
            ],
        ],
    }
}

/// Poses an assembly, given as a JSON value (see the module's words).
#[must_use]
pub fn pose_assembly(input: &Value) -> Pose {
    let parts: Vec<&Value> = input
        .get("parts")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let asm = input.get("assembly").cloned().unwrap_or(Value::Null);
    let instances: Vec<&Value> = asm
        .get("instances")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let joints: Vec<&Value> = asm
        .get("joints")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    // Each instance's steps: a grounded one where it was put, then each joint's second part from
    // its first, round and round until nothing more can be placed, as the prototype does.
    let mut chains: BTreeMap<String, Vec<Step>> = BTreeMap::new();
    let mut joint_rows: Vec<Option<JointRow>> = joints.iter().map(|_| None).collect();
    for inst in &instances {
        if inst.get("grounded").and_then(Value::as_bool) == Some(true) {
            chains.insert(
                text(inst, "id").to_owned(),
                vec![Step::Move(numbers(inst.get("at")))],
            );
        }
    }
    let mut placed_any = true;
    while placed_any {
        placed_any = false;
        for (k, j) in joints.iter().enumerate() {
            let (a, b) = (text(j, "a"), text(j, "b"));
            if joint_rows[k].is_some() || !chains.contains_key(a) || chains.contains_key(b) {
                continue;
            }
            let Some(inst) = instances.iter().find(|i| text(i, "id") == b) else {
                continue;
            };
            let (lo, hi) = (number(j.get("min")), number(j.get("max")));
            let wanted = number(j.get("value"));
            let value = wanted.clamp(lo.min(hi), hi.max(lo));
            let held = if value == wanted {
                String::new()
            } else {
                format!(" Held at its limit: it was set to {}.", fmt_mm(wanted))
            };
            let axis = axis_named(text(j, "axis"));
            let origin: [f64; 3] = numbers(j.get("origin"));
            let (motion, said) = match (text(j, "type"), axis) {
                ("hinge", Some((axis, index))) => match turn(value) {
                    Ok((rotation, quarter, cos, sin)) => (
                        Some(Step::Turn {
                            axis,
                            index,
                            rotation,
                            quarter,
                            cos,
                            sin,
                        }),
                        format!(
                            "Turned {}° about {} by the kernel's transform.rotate",
                            fmt_mm(value),
                            text(j, "axis")
                        ),
                    ),
                    Err(r) => (None, r.said),
                },
                ("slider", Some((_, index))) => {
                    let mut d = [0.0; 3];
                    d[index] = value;
                    (
                        Some(Step::Move(d)),
                        format!(
                            "Slid {} mm along {} by the kernel's transform.move",
                            fmt_mm(value),
                            text(j, "axis")
                        ),
                    )
                }
                ("fixed", _) => (None, "Held by the kernel's transform.move".to_owned()),
                ("hinge" | "slider", None) => {
                    let message = "Pick the axis it moves about: x, y or z.".to_owned();
                    joint_rows[k] = Some(JointRow {
                        id: text(j, "id").to_owned(),
                        state: "failed",
                        value,
                        error: Some(
                            Report::workshop(ErrorKind::InvalidInput, "NoAxis", message.clone())
                                .with(change("the axis", "Pick x, y or z")),
                        ),
                        message,
                    });
                    continue;
                }
                (other, _) => {
                    let message = format!(
                        "A {other} joint waits for the kernel's assembly mates (G479) and \
                         persistent face names."
                    );
                    joint_rows[k] = Some(JointRow {
                        id: text(j, "id").to_owned(),
                        state: "waits",
                        value,
                        error: Some(
                            Report::workshop(
                                ErrorKind::Unsupported,
                                "AwaitsKernel",
                                message.clone(),
                            )
                            .with(change(
                                "the joint",
                                "Make it a fixed, hinge or slider joint on a typed-in axis, \
                                     which is posed today",
                            ))
                            .awaiting(&["G479"]),
                        ),
                        message,
                    });
                    continue;
                }
            };
            let mut chain = vec![Step::Move(numbers(inst.get("at")))];
            chain.extend(motion);
            chain.push(Step::Move(origin));
            chain.extend(chains[a].iter().copied());
            chains.insert(b.to_owned(), chain);
            joint_rows[k] = Some(JointRow {
                id: text(j, "id").to_owned(),
                state: "built",
                value,
                message: format!("{said}.{held}"),
                error: None,
            });
            placed_any = true;
        }
    }
    let mut notes = Vec::new();
    for inst in &instances {
        let id = text(inst, "id");
        if !chains.contains_key(id) {
            chains.insert(id.to_owned(), vec![Step::Move(numbers(inst.get("at")))]);
            notes.push(format!(
                "{} is not joined to a grounded part, so it stays where it was put.",
                text(inst, "name")
            ));
        }
    }

    // Build each part once, then put its bodies through each instance's steps.
    let mut built = BTreeMap::new();
    let mut rows = Vec::new();
    let mut whole = None;
    for (n, inst) in instances.iter().enumerate() {
        let id = text(inst, "id");
        let part_id = text(inst, "part");
        let chain = &chains[id];
        let m = chain
            .iter()
            .fold(IDENTITY, |acc, s| then(&acc, &map_of(*s)));
        let mut row = InstanceRow {
            id: id.to_owned(),
            part: part_id.to_owned(),
            state: "built",
            message: String::new(),
            m,
            exactness: "Exact",
            bound: None,
            bbox: None,
            error: None,
        };
        let Some(part) = parts.iter().find(|p| text(p, "id") == part_id) else {
            row.state = "failed";
            row.message = "Its part isn't in the project.".into();
            row.error = Some(
                Report::workshop(ErrorKind::InvalidInput, "NoSuchPart", row.message.clone())
                    .with(change("the part", "Pick a part that is in the project")),
            );
            rows.push(row);
            continue;
        };
        let bodies = built
            .entry(part_id.to_owned())
            .or_insert_with(|| build_with_bodies(part).1);
        let mut bbox = None;
        // the least exact class among its bodies, and the largest bound
        let mut worst: (&'static str, Option<u64>) = ("Exact", None);
        let mut failed = None;
        for (k, (body, e)) in bodies.iter().enumerate() {
            let key = 1_000_000 + n * 1000 + k;
            let posed = place(key, body, *e, chain).and_then(|(b, e)| {
                let c = ctx(key, 999);
                let found = bounding_box(&c, &b, e).map_err(|e| refused(&e))?.result;
                let Some(found) = found else {
                    return defect("The kernel posed an empty body.");
                };
                let (lo, hi) = (found.bound.min(), found.bound.max());
                Ok((
                    e,
                    [
                        [mm(lo.x.get()), mm(lo.y.get()), mm(lo.z.get())],
                        [mm(hi.x.get()), mm(hi.y.get()), mm(hi.z.get())],
                    ],
                ))
            });
            match posed {
                Ok((e, b)) => {
                    bbox = Some(union(bbox, b));
                    let (cls, bound) = class(e);
                    if cls != "Exact" && worst.0 != "Approximate" {
                        worst.0 = cls;
                    }
                    worst.1 = worst.1.max(bound);
                }
                Err(r) => {
                    failed = Some(r);
                    break;
                }
            }
        }
        if let Some(r) = failed {
            (row.state, row.message) = if r.waits {
                (
                    "waits",
                    format!("{} Until the kernel can, the page places it.", r.said),
                )
            } else {
                ("failed", format!("{} The page places it instead.", r.said))
            };
            row.error = Some(*r.report);
        } else {
            let (cls, bound) = worst;
            row.exactness = cls;
            row.bound = bound;
            row.message = match bound {
                Some(b) => format!(
                    "Posed by the kernel, Certified within {b} grid steps ({} nm).",
                    fmt_mm(6.25 * f64::from(u32::try_from(b).unwrap_or(u32::MAX)))
                ),
                None => "Posed by the kernel, Exact.".into(),
            };
            row.bbox = bbox;
            if let Some(b) = bbox {
                whole = Some(union(whole, b));
            }
        }
        rows.push(row);
    }
    // a joint the kernel couldn't pose its moving part through says why, as that part does
    let joints = joint_rows
        .into_iter()
        .zip(&joints)
        .map(|(row, j)| {
            let mut row = row?;
            if row.state == "built"
                && let Some(moving) = rows.iter().find(|r| r.id == text(j, "b"))
                && moving.state != "built"
            {
                let held = row
                    .message
                    .find(" Held at")
                    .map_or("", |k| &row.message[k..]);
                row.message = format!("{}{held}", moving.message);
                row.state = moving.state;
                row.error.clone_from(&moving.error);
            }
            Some(row)
        })
        .zip(&joints)
        .map(|(row, j)| {
            row.unwrap_or_else(|| {
                let message =
                    "Its first part isn't placed, or its second is placed by another joint.";
                JointRow {
                    id: text(j, "id").to_owned(),
                    state: "failed",
                    value: number(j.get("value")),
                    message: message.into(),
                    error: Some(
                        Report::workshop(ErrorKind::InvalidInput, "NotPlaced", message).with(
                            change(
                                "the parts it joins",
                                "Join it from a part that is placed, to one no other joint places",
                            ),
                        ),
                    ),
                }
            })
        })
        .collect();
    Pose {
        engine: "kernel",
        instances: rows,
        joints,
        bbox: whole,
        notes,
    }
}

/// Poses an assembly given as JSON text, and gives the answer as JSON text, or
/// `{"failed": {code, message}}` for text that is not an assembly.
#[must_use]
pub fn pose_json(input: &str) -> String {
    match serde_json::from_str::<Value>(input) {
        Ok(v) if v.is_object() => serde_json::to_string(&pose_assembly(&v)).unwrap_or_else(|e| {
            json!({ "failed": { "code": "Unwritable", "message": e.to_string() } }).to_string()
        }),
        Ok(_) => json!({ "failed": { "code": "BadAssembly", "message": "An assembly is a JSON object." } })
            .to_string(),
        Err(e) => json!({ "failed": { "code": "BadAssembly", "message": e.to_string() } }).to_string(),
    }
}
