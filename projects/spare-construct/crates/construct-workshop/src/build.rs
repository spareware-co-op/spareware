//! A part's recipe, built by the geometry kernel.
//!
//! The recipe, as JSON: `{ params: [{ name, value, kind }], features: [{ id, type, name, ... }], end }`.
//! A value is a number of millimetres or counts, the name of a parameter, or a formula of parameters
//! with units, such as `width / 2 - 4 mm` (`construct-formulas`, shared with families). A parameter's
//! `kind` is `length` (the default), `angle` or `count`, and its formula may use the others in any
//! order; a loop of formulas is refused. Features before `end` are built in order; the rest are
//! rolled back. The answer:
//! `{ engine, params, features: [{ id, state, message, bodies }], bodies, volume, bbox }`, where a
//! feature's state is `built`, `waits`, `failed`, `suppressed` or `rolled_back`, and a body says how to
//! draw it (its profile, height and the map from the profile's place to where it is) and what the
//! kernel says of it (its exactness, volume, face count and box).
//!
//! The kernel decides: a profile goes to `make.prism` as it is, and if the kernel refuses it as not
//! convex, the feature waits for the kernel's extrude or booleans rather than being built some other
//! way. The page draws a body from its profile and map because the facade has no display meshes yet
//! (tessellation is kernel piece G288); the tests hold every map to the kernel's own box of the body.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use construct_formulas::{Entry, FormulaError, Kind, Quantity, evaluate, value_of};
use engine_geometry::operations::{
    Instance, Made, Moved, PrismInput, RoundInput, bounding_box, cylinder, faces, mirror, move_by,
    pattern, prism, six_volume,
};
use engine_geometry::value::Exactness;
use engine_geometry::value::body::Body;
use engine_geometry::value::error::{MakeDetail, OpError, OpErrorDetail};
use engine_geometry::value::grid::{GridStep, P3, Steps};
use engine_geometry::value::op::{Budget, OpContext, OpId, OpOutput, OpPath};
use engine_geometry::value::transform::{Axis, GridTransform, TypedTransform};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// Grid steps per millimetre: the kernel's step for parts and boards, `Q1`, is 6.25 nm.
pub const STEPS_PER_MM: f64 = 160_000.0;
/// The kernel's reach, in steps: `geom_num::grid::COORD_MAX`, 2^40 − 1 (about 6.87 km for parts).
const COORD_MAX: i64 = (1 << 40) - 1;

/// The features a part can hold that the kernel cannot build yet, and the kernel pieces each waits
/// for (`projects/engines/geometry/architecture/30-build-order.json`). The same words as the page's.
const WAITS: &[(&str, &str, &str)] = &[
    ("revolve", "Revolve", "the kernel's revolve (G255)"),
    ("sweep", "Sweep", "the kernel's sweeps (G256, G257)"),
    (
        "fillet",
        "Fillet",
        "the kernel's blends (G263 to G265) and persistent edge names",
    ),
    (
        "chamfer",
        "Chamfer",
        "the kernel's blends (G263) and persistent edge names",
    ),
    ("shell", "Shell", "the kernel's offsets (G275, G277)"),
    ("hole", "Hole", "the kernel's booleans (G130 to G133)"),
    ("join", "Join", "the kernel's booleans (G130 to G133)"),
    ("cut", "Cut", "the kernel's booleans (G130 to G133)"),
    (
        "intersect",
        "Intersect",
        "the kernel's booleans (G130 to G133)",
    ),
];
const OUTLINE_WAITS: &str = "the kernel's extrude (G255) or its booleans (G130, G131); today it extrudes convex outlines and circles";

/// What a builder answers for a part.
#[derive(Debug, Serialize)]
pub struct Answer {
    /// Which builder answered: `kernel`.
    pub engine: &'static str,
    /// Each parameter, its value and whether it was read.
    pub params: Vec<ParamRow>,
    /// Each feature, in the history's order.
    pub features: Vec<FeatureRow>,
    /// Every body the part has after the last built feature, in the order made.
    pub bodies: Vec<BodyOut>,
    /// The bodies' volumes added, in cubic millimetres.
    pub volume: f64,
    /// The box round every body, in millimetres; `None` with no bodies.
    pub bbox: Option<[[f64; 3]; 2]>,
}

/// A parameter, as read.
#[derive(Debug, Serialize)]
pub struct ParamRow {
    /// Its name.
    pub name: String,
    /// `built` or `failed`.
    pub state: &'static str,
    /// Why it failed, or nothing.
    pub message: String,
    /// Its value, once read, in millimetres, degrees or as a count.
    pub value: Option<f64>,
    /// `length`, `angle` or `count`.
    pub kind: &'static str,
    /// Whether it is a formula of other parameters rather than a number.
    pub formula: bool,
}

/// A feature, as built.
#[derive(Debug, Serialize)]
pub struct FeatureRow {
    /// The feature's id.
    pub id: String,
    /// `built`, `waits`, `failed`, `suppressed` or `rolled_back`.
    pub state: &'static str,
    /// What happened, in words.
    pub message: String,
    /// The bodies it made or moved.
    pub bodies: Vec<String>,
    /// For a sketch: whether the kernel can extrude its profile today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub convex: Option<bool>,
}

/// A body, as the page draws it and as the kernel describes it.
#[derive(Clone, Debug, Serialize)]
pub struct BodyOut {
    /// The body's id: its feature's, and for a copy the copy's too.
    pub id: String,
    /// The feature that made it.
    pub by: String,
    /// `prism` or `cylinder`.
    pub kind: &'static str,
    /// A prism's profile, counter-clockwise from above, in millimetres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<[f64; 2]>>,
    /// A cylinder's centre, in millimetres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<[f64; 2]>,
    /// A cylinder's radius, in millimetres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// The profile's height, in millimetres.
    pub z: f64,
    /// How far it is lifted, in millimetres.
    pub height: f64,
    /// The map from the profile's place to where the body is: 3 rows of 4, row-major, millimetres.
    pub m: [f64; 12],
    /// The kernel's exactness class: `Exact` or `Certified`.
    pub exactness: &'static str,
    /// A Certified body's proven bound, in grid steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<u64>,
    /// Its volume, in cubic millimetres.
    pub volume: f64,
    /// Where the volume came from: `kernel` (`query.volume`, exact) or `recipe` (a cylinder's, whose
    /// certified volume waits for the kernel's mass properties, G247).
    pub volume_from: &'static str,
    /// How many faces the kernel gives it (`query.faces`).
    pub faces: usize,
    /// The kernel's box of it (`query.bounding_box`), in millimetres.
    pub bbox: [[f64; 3]; 2],
}

/// Why a feature was not built.
pub(crate) enum Refusal {
    Failed(String),
    Waits(String),
}
pub(crate) type Res<T> = Result<T, Refusal>;
pub(crate) fn fail<T>(m: impl Into<String>) -> Res<T> {
    Err(Refusal::Failed(m.into()))
}
fn waits<T>(m: impl Into<String>) -> Res<T> {
    Err(Refusal::Waits(m.into()))
}
pub(crate) fn refused(e: &OpError) -> Refusal {
    Refusal::Failed(format!("The kernel refused it: {}", e.message()))
}

/// A body the part has, with the kernel's body behind it.
struct Live {
    out: BodyOut,
    body: Body,
    exactness: Exactness,
}

/// A profile: an outline counter-clockwise in grid steps, or a circle, at a height.
#[derive(Clone)]
enum Profile {
    Outline {
        pts: Vec<[i64; 2]>,
        z: i64,
        convex: bool,
        turned: bool,
    },
    Circle {
        center: [i64; 2],
        radius: i64,
        z: i64,
    },
}

/// What a built feature left for later features to refer to.
#[derive(Clone)]
enum Left {
    Profile(Profile),
    Bodies(Vec<String>),
}

pub(crate) const IDENTITY: [f64; 12] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.];

/// `a` then `b`: the map that applies `a` first.
#[allow(clippy::needless_range_loop)] // the indices are the matrix's rows and columns
pub(crate) fn then(a: &[f64; 12], b: &[f64; 12]) -> [f64; 12] {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..4 {
            let mut s = if j == 3 { b[i * 4 + 3] } else { 0.0 };
            for k in 0..3 {
                s += b[i * 4 + k] * a[k * 4 + j];
            }
            r[i * 4 + j] = s;
        }
    }
    r
}
pub(crate) fn translation(d: [f64; 3]) -> [f64; 12] {
    [1., 0., 0., d[0], 0., 1., 0., d[1], 0., 0., 1., d[2]]
}
fn mirror_map(axis: usize, at: f64) -> [f64; 12] {
    let mut m = IDENTITY;
    m[axis * 4 + axis] = -1.0;
    m[axis * 4 + 3] = 2.0 * at;
    m
}

pub(crate) fn mm(steps: i64) -> f64 {
    // a coordinate is at most 2^40 - 1 steps, so it converts to f64 exactly
    #[allow(clippy::cast_precision_loss)]
    let v = steps as f64;
    v / STEPS_PER_MM
}
/// Millimetres to the nearest grid step, rounding a half step up, as the page's prototype rounds.
pub(crate) fn steps(v: f64) -> Res<i64> {
    let s = (v * STEPS_PER_MM + 0.5).floor();
    #[allow(clippy::cast_precision_loss)]
    let reach = COORD_MAX as f64;
    if !s.is_finite() || s.abs() > reach {
        return fail("A size lands past the kernel's reach for parts, 6.87 km from the origin.");
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok(s as i64)
}
/// Millimetres on the grid.
fn q(v: f64) -> Res<f64> {
    Ok(mm(steps(v)?))
}

/// The parameters that have values, exactly.
type Env = BTreeMap<String, Quantity>;

/// A parameter's value for a formula, or why it has none.
fn lookup(env: &Env, name: &str) -> Result<Quantity, FormulaError> {
    env.get(name).cloned().ok_or_else(|| FormulaError {
        code: "UnknownName",
        message: format!("there is no parameter called {name}, or it fails."),
    })
}

/// A field's value of `kind`: a number in the kind's own unit, a parameter's name or a formula. A
/// formula is worked out exactly and meets one rounding, to the nearest double, here.
fn value_as(env: &Env, v: Option<&Value>, what: &str, kind: Kind) -> Res<f64> {
    match v {
        Some(Value::Number(n)) => n
            .as_f64()
            .map_or_else(|| fail(format!("{what} is not a number.")), Ok),
        Some(Value::String(s)) if !s.trim().is_empty() => {
            match value_of(s, kind, &mut |name| lookup(env, name)) {
                Ok(x) => Ok(x.to_f64()),
                Err(e) => fail(format!("{what}: {}", lower(&e.message))),
            }
        }
        _ => fail(format!("{what} is not set.")),
    }
}
fn value(env: &Env, v: Option<&Value>, what: &str) -> Res<f64> {
    value_as(env, v, what, Kind::Length)
}
/// A message, to follow a colon.
fn lower(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_lowercase().collect::<String>() + chars.as_str()
    })
}
fn length(env: &Env, v: Option<&Value>, what: &str) -> Res<f64> {
    q(value(env, v, what)?)
}
fn positive(env: &Env, v: Option<&Value>, what: &str) -> Res<f64> {
    let x = length(env, v, what)?;
    if x < 1.0 / STEPS_PER_MM {
        return fail(format!("{what} must be more than zero."));
    }
    Ok(x)
}
fn whole(env: &Env, v: Option<&Value>, what: &str, lo: u32, hi: u32) -> Res<u32> {
    let x = value_as(env, v, what, Kind::Count)?;
    if x.fract() != 0.0 || x < f64::from(lo) || x > f64::from(hi) {
        return fail(format!("{what} must be a whole number from {lo} to {hi}."));
    }
    // checked against hi just above
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(x as u32)
}

fn cross(o: [i64; 2], a: [i64; 2], b: [i64; 2]) -> i128 {
    let d = |p: [i64; 2], q: [i64; 2], i: usize| i128::from(p[i]) - i128::from(q[i]);
    d(a, o, 0) * d(b, o, 1) - d(a, o, 1) * d(b, o, 0)
}
fn area2(pts: &[[i64; 2]]) -> i128 {
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            i128::from(a[0]) * i128::from(b[1]) - i128::from(b[0]) * i128::from(a[1])
        })
        .sum()
}
/// Strictly convex and counter-clockwise, as `make.prism` asks: every turn to the left, exactly, and
/// once round in all (a star turns left at every corner too). The kernel decides again when it builds.
fn convex(pts: &[[i64; 2]]) -> bool {
    let n = pts.len();
    if (0..n).any(|i| cross(pts[i], pts[(i + 1) % n], pts[(i + 2) % n]) <= 0) {
        return false;
    }
    let turn: f64 = (0..n)
        .map(|i| {
            let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
            let f = |p: [i64; 2]| [mm(p[0]), mm(p[1])];
            let (a, b, c) = (f(a), f(b), f(c));
            let cr = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            let dot = (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1]);
            cr.atan2(dot)
        })
        .sum();
    (turn - 2.0 * PI).abs() < 1e-6
}

fn field<'a>(f: &'a Map<String, Value>, k: &str) -> Option<&'a Value> {
    f.get(k)
}
fn text(f: &Map<String, Value>, k: &str) -> String {
    f.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn profile(env: &Env, f: &Map<String, Value>) -> Res<Profile> {
    let zero = json!(0);
    let or0 = |k: &str| field(f, k).filter(|v| !v.is_null()).unwrap_or(&zero);
    let z = length(env, Some(or0("z")), "The sketch height")?;
    let cx = length(env, Some(or0("cx")), "The centre x")?;
    let cy = length(env, Some(or0("cy")), "The centre y")?;
    let shape = text(f, "shape");
    if shape == "circle" {
        let r = positive(env, field(f, "r"), "The radius")?;
        return Ok(Profile::Circle {
            center: [steps(cx)?, steps(cy)?],
            radius: steps(r)?,
            z: steps(z)?,
        });
    }
    let raw: Vec<[f64; 2]> = match shape.as_str() {
        "rect" => {
            let w = positive(env, field(f, "w"), "The width")?;
            let h = positive(env, field(f, "h"), "The height")?;
            vec![
                [cx - w / 2.0, cy - h / 2.0],
                [cx + w / 2.0, cy - h / 2.0],
                [cx + w / 2.0, cy + h / 2.0],
                [cx - w / 2.0, cy + h / 2.0],
            ]
        }
        "polygon" => {
            let n = whole(env, field(f, "n"), "The number of sides", 3, 64)?;
            let r = positive(env, field(f, "r"), "The radius")?;
            let nf = f64::from(n);
            (0..n)
                .map(|k| {
                    let a = -PI / 2.0 + PI / nf + (2.0 * PI * f64::from(k)) / nf;
                    [cx + r * a.cos(), cy + r * a.sin()]
                })
                .collect()
        }
        "outline" => {
            let Some(pts) = field(f, "pts")
                .and_then(Value::as_array)
                .filter(|p| p.len() >= 3)
            else {
                return fail("An outline needs at least three corners.");
            };
            let mut out = Vec::with_capacity(pts.len());
            for (i, p) in pts.iter().enumerate() {
                let x = length(env, p.get(0), &format!("Corner {} x", i + 1))?;
                let y = length(env, p.get(1), &format!("Corner {} y", i + 1))?;
                out.push([cx + x, cy + y]);
            }
            out
        }
        other => return fail(format!("There is no sketch shape called {other}.")),
    };
    let mut pts = Vec::with_capacity(raw.len());
    for p in raw {
        pts.push([steps(p[0])?, steps(p[1])?]);
    }
    let n = pts.len();
    if let Some(i) = (0..n).find(|&i| pts[i] == pts[(i + 1) % n]) {
        return fail(format!(
            "Corners {} and {} are the same point.",
            i + 1,
            (i + 1) % n + 1
        ));
    }
    let s = area2(&pts);
    if s == 0 {
        return fail("The outline has no area.");
    }
    let turned = s < 0;
    if turned {
        pts.reverse();
    }
    let convex = convex(&pts);
    Ok(Profile::Outline {
        pts,
        z: steps(z)?,
        convex,
        turned,
    })
}

/// The context for one kernel operation: its path names the feature and the step within it, so every
/// face the part has gets a name of its own (kernel chapter 20).
pub(crate) fn ctx(feature: usize, step: u32) -> OpContext<'static> {
    let mut id = [0_u8; 16];
    id[..8].copy_from_slice(&(feature as u64).to_be_bytes());
    id[8..12].copy_from_slice(&step.to_be_bytes());
    OpContext::new(OpPath::new(OpId::from_bytes(id)), Budget::UNLIMITED)
}

pub(crate) fn class(e: Exactness) -> (&'static str, Option<u64>) {
    match e {
        Exactness::Certified { bound, .. } => ("Certified", Some(bound.get())),
        Exactness::Approximate { .. } => ("Approximate", None),
        _ => ("Exact", None),
    }
}

/// What the kernel says of a body: its box, its face count and, for a flat one, its volume.
fn describe(
    c: &OpContext<'_>,
    body: &Body,
    e: Exactness,
    flat: bool,
    recipe_volume: f64,
) -> Res<([[f64; 3]; 2], usize, f64, &'static str)> {
    let b = bounding_box(c, body, e).map_err(|e| refused(&e))?.result;
    let Some(b) = b else {
        return fail("The kernel built an empty body.");
    };
    let (lo, hi) = (b.bound.min(), b.bound.max());
    let bbox = [
        [mm(lo.x.get()), mm(lo.y.get()), mm(lo.z.get())],
        [mm(hi.x.get()), mm(hi.y.get()), mm(hi.z.get())],
    ];
    let n = faces(c, body, &[], e)
        .map_err(|e| refused(&e))?
        .result
        .len();
    if flat {
        let six = six_volume(c, body, e).map_err(|e| refused(&e))?.result;
        let Some(six) = six.to_i128() else {
            return fail("The volume does not fit.");
        };
        // six times the volume in cubic steps: at most about 2^122, so f64 keeps 53 bits of it
        #[allow(clippy::cast_precision_loss)]
        let v = (six.abs() as f64) / 6.0 / (STEPS_PER_MM * STEPS_PER_MM * STEPS_PER_MM);
        Ok((bbox, n, v, "kernel"))
    } else {
        Ok((bbox, n, recipe_volume, "recipe"))
    }
}

fn live_from(
    c: &OpContext<'_>,
    made: &OpOutput<Made>,
    mut out: BodyOut,
    recipe_volume: f64,
) -> Res<Live> {
    let e = made.exactness;
    let (bbox, faces, volume, from) =
        describe(c, &made.result.body, e, out.kind == "prism", recipe_volume)?;
    let (cls, bound) = class(e);
    out.exactness = cls;
    out.bound = bound;
    out.volume = volume;
    out.volume_from = from;
    out.faces = faces;
    out.bbox = bbox;
    Ok(Live {
        out,
        body: made.result.body.clone(),
        exactness: e,
    })
}

fn axis_of(f: &Map<String, Value>) -> Res<(Axis, usize, char)> {
    match f.get("plane").and_then(Value::as_str).unwrap_or("x") {
        "x" => Ok((Axis::X, 0, 'x')),
        "y" => Ok((Axis::Y, 1, 'y')),
        "z" => Ok((Axis::Z, 2, 'z')),
        _ => fail("Mirror across x, y or z."),
    }
}

pub(crate) fn fmt_mm(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        format!("{r:.0}")
    } else {
        format!("{r}")
    }
}

/// A part being built: the parameters read, the features so far, and the bodies the part has.
struct Builder<'a> {
    env: Env,
    feats: &'a [Value],
    rows: Vec<FeatureRow>,
    left: BTreeMap<String, Left>,
    live: Vec<Live>,
}

/// A length that is zero when it is left out.
fn length0(env: &Env, f: &Map<String, Value>, k: &str, what: &str) -> Res<f64> {
    match f.get(k) {
        None | Some(Value::Null) => Ok(0.0),
        v => length(env, v, what),
    }
}
pub(crate) fn point(p: [i64; 3]) -> Res<P3> {
    P3::new(p[0], p[1], p[2])
        .map_err(|_| Refusal::Failed("A corner lands past the kernel's reach for parts.".into()))
}

impl Builder<'_> {
    /// What an earlier feature left, by its id.
    fn refer(&self, i: usize, rid: &str, what: &str) -> Res<Left> {
        let k = self
            .feats
            .iter()
            .position(|g| g.get("id").and_then(Value::as_str) == Some(rid));
        let Some(k) = k.filter(|_| !rid.is_empty()) else {
            return fail(format!("Pick the {what}."));
        };
        if k >= i {
            return fail(format!("The {what} comes later in the history."));
        }
        if let Some(l) = self.left.get(rid) {
            return Ok(l.clone());
        }
        let name = self.feats[k]
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let state = self
            .rows
            .get(k)
            .map_or("not built", |r| r.state)
            .replace('_', " ");
        fail(format!("The {what}, {name}, is not built ({state})."))
    }

    fn feature(&mut self, i: usize, f: &Map<String, Value>, row: &mut FeatureRow) -> Res<()> {
        let kind = text(f, "type");
        if let Some((_, _, what)) = WAITS.iter().find(|w| w.0 == kind) {
            return waits(format!("Waits for {what}."));
        }
        match kind.as_str() {
            "sketch" => self.sketch(f, row),
            "extrude" => self.extrude(i, f, row),
            "mirror" | "pattern" | "move" => self.copy(i, f, &kind, row),
            other => fail(format!("There is no feature called {other}.")),
        }
    }

    fn sketch(&mut self, f: &Map<String, Value>, row: &mut FeatureRow) -> Res<()> {
        let p = profile(&self.env, f)?;
        row.message = match &p {
            Profile::Circle { .. } => "A circle: extrudes to a cylinder.".into(),
            Profile::Outline {
                pts,
                convex: true,
                turned,
                ..
            } => format!(
                "Convex, {} corners: extrudes to a prism.{}",
                pts.len(),
                if *turned {
                    " Drawn clockwise, taken anticlockwise."
                } else {
                    ""
                }
            ),
            Profile::Outline { .. } => {
                format!("Not convex. Extruding it waits for {OUTLINE_WAITS}.")
            }
        };
        row.convex = Some(matches!(
            p,
            Profile::Circle { .. } | Profile::Outline { convex: true, .. }
        ));
        self.left.insert(row.id.clone(), Left::Profile(p));
        Ok(())
    }

    fn extrude(&mut self, i: usize, f: &Map<String, Value>, row: &mut FeatureRow) -> Res<()> {
        let Left::Profile(p) = self.refer(i, &text(f, "sketch"), "sketch")? else {
            return fail("Extrude a sketch, not a body.");
        };
        let op = f.get("op").and_then(Value::as_str).unwrap_or("new");
        if op != "new" {
            let label = WAITS.iter().find(|w| w.0 == op).map_or(op, |w| w.1);
            return waits(format!(
                "{label} waits for the kernel's booleans (G130 to G133). A new body extrudes today."
            ));
        }
        let height = positive(&self.env, f.get("depth"), "The distance")?;
        let h = Steps(steps(height)?.unsigned_abs());
        let c = ctx(i, 0);
        let id = row.id.clone();
        let blank = |kind| BodyOut {
            id: id.clone(),
            by: id.clone(),
            kind,
            outline: None,
            center: None,
            radius: None,
            z: 0.0,
            height,
            m: IDENTITY,
            exactness: "Exact",
            bound: None,
            volume: 0.0,
            volume_from: "kernel",
            faces: 0,
            bbox: [[0.0; 3]; 2],
        };
        let lv = match p {
            Profile::Circle { center, radius, z } => {
                let input = RoundInput {
                    base: point([center[0], center[1], z])?,
                    radius: Steps(radius.unsigned_abs()),
                    height: h,
                    step: GridStep::Q1,
                };
                let out = cylinder(&c, &input).map_err(|e| refused(&e))?;
                let r = mm(radius);
                let mut shape = blank("cylinder");
                shape.center = Some([mm(center[0]), mm(center[1])]);
                shape.radius = Some(r);
                shape.z = mm(z);
                let lv = live_from(&c, &out, shape, PI * r * r * height)?;
                row.message = format!(
                    "A cylinder of {} faces, {} within one grid step.",
                    lv.out.faces, lv.out.exactness
                );
                lv
            }
            Profile::Outline { pts, z, convex, .. } => {
                if !convex {
                    return waits(format!(
                        "Extruding a shape that is not convex waits for {OUTLINE_WAITS}."
                    ));
                }
                let base = pts
                    .iter()
                    .map(|p| point([p[0], p[1], z]))
                    .collect::<Res<Vec<_>>>()?;
                let out = match prism(&c, &PrismInput { base, height: h }) {
                    Ok(out) => out,
                    Err(e)
                        if matches!(e.detail(), OpErrorDetail::Make(MakeDetail::BaseNotConvex)) =>
                    {
                        return waits(format!(
                            "The kernel finds this outline not convex. Extruding it waits for {OUTLINE_WAITS}."
                        ));
                    }
                    Err(e) => return Err(refused(&e)),
                };
                let mut shape = blank("prism");
                shape.outline = Some(pts.iter().map(|p| [mm(p[0]), mm(p[1])]).collect());
                shape.z = mm(z);
                let lv = live_from(&c, &out, shape, 0.0)?;
                row.message = format!("A prism of {} faces, {}.", lv.out.faces, lv.out.exactness);
                lv
            }
        };
        row.bodies = vec![id.clone()];
        self.live.push(lv);
        self.left.insert(id.clone(), Left::Bodies(vec![id]));
        Ok(())
    }

    fn copy(
        &mut self,
        i: usize,
        f: &Map<String, Value>,
        kind: &str,
        row: &mut FeatureRow,
    ) -> Res<()> {
        let Left::Bodies(src_ids) = self.refer(i, &text(f, "of"), "feature to copy")? else {
            return fail("Pick a feature that made a body.");
        };
        let src: Vec<usize> = src_ids
            .iter()
            .filter_map(|s| self.live.iter().position(|l| &l.out.id == s))
            .collect();
        if src.is_empty() {
            return fail("That feature has no bodies left.");
        }
        let id = row.id.clone();
        let c = ctx(i, 0);
        let mut ids = Vec::new();
        match kind {
            "mirror" => {
                let (axis, ai, an) = axis_of(f)?;
                let at = length0(&self.env, f, "at", "The mirror plane")?;
                let mut p = [0_i64; 3];
                p[ai] = steps(at)?;
                let at_p = point(p)?;
                let mut fresh = Vec::new();
                for &k in &src {
                    let l = &self.live[k];
                    let moved = Moved {
                        body: &l.body,
                        exactness: l.exactness,
                        accept: None,
                    };
                    let out = mirror(&c, moved, axis, at_p).map_err(|e| refused(&e))?;
                    let mut shape = l.out.clone();
                    shape.id = format!("{id}:{}", l.out.id);
                    shape.by.clone_from(&id);
                    shape.m = then(&l.out.m, &mirror_map(ai, at));
                    fresh.push(live_from(&c, &out, shape, l.out.volume)?);
                }
                for l in fresh {
                    ids.push(l.out.id.clone());
                    self.live.push(l);
                }
                row.message = format!("Mirrored across the plane {an} = {} mm, Exact.", fmt_mm(at));
            }
            "pattern" => {
                let env = &self.env;
                let n = whole(env, f.get("count"), "The number in the row", 2, 100)?;
                let d = [
                    length0(env, f, "dx", "The spacing")?,
                    length0(env, f, "dy", "The spacing")?,
                    length0(env, f, "dz", "The spacing")?,
                ];
                let n2 = match f.get("count2").filter(|v| !v.is_null()) {
                    None => 1,
                    Some(v) => whole(env, Some(v), "The number of rows", 1, 100)?,
                };
                let d2 = [
                    length0(env, f, "dx2", "The row spacing")?,
                    length0(env, f, "dy2", "The row spacing")?,
                    length0(env, f, "dz2", "The row spacing")?,
                ];
                if d.iter().all(|v| *v == 0.0) {
                    return fail("The spacing is zero.");
                }
                if n2 > 1 && d2.iter().all(|v| *v == 0.0) {
                    return fail("The row spacing is zero.");
                }
                let ds = [steps(d[0])?, steps(d[1])?, steps(d[2])?];
                let d2s = [steps(d2[0])?, steps(d2[1])?, steps(d2[2])?];
                let mut instances = Vec::new();
                let mut offsets = BTreeMap::new();
                for j in 0..n2 {
                    for k in 0..n {
                        if j == 0 && k == 0 {
                            continue;
                        }
                        let mut off = [0_i64; 3];
                        for (a, o) in off.iter_mut().enumerate() {
                            let v = i64::from(k)
                                .checked_mul(ds[a])
                                .zip(i64::from(j).checked_mul(d2s[a]))
                                .and_then(|(x, y)| x.checked_add(y));
                            let Some(v) = v.filter(|v| v.abs() <= COORD_MAX) else {
                                return fail("A copy lands past the kernel's reach for parts.");
                            };
                            *o = v;
                        }
                        let t = GridTransform::translation(off).map_err(|_| {
                            Refusal::Failed(
                                "A copy lands past the kernel's reach for parts.".into(),
                            )
                        })?;
                        let mut key = [0_u8; 16];
                        key[..4].copy_from_slice(&j.to_be_bytes());
                        key[4..8].copy_from_slice(&k.to_be_bytes());
                        instances.push(Instance {
                            key,
                            transform: TypedTransform::Exact(t),
                        });
                        offsets.insert(key, (j, k, off));
                    }
                }
                let mut fresh = Vec::new();
                for (s, &si) in src.iter().enumerate() {
                    let l = &self.live[si];
                    let c = ctx(i, u32::try_from(s).unwrap_or(u32::MAX));
                    let moved = Moved {
                        body: &l.body,
                        exactness: l.exactness,
                        accept: None,
                    };
                    let copies = pattern(&c, moved, &instances).map_err(|e| refused(&e))?;
                    for (key, made) in &copies.result {
                        let Some(&(j, k, off)) = offsets.get(key) else {
                            return fail("The kernel gave back a copy it was not asked for.");
                        };
                        let mut shape = l.out.clone();
                        shape.id = format!("{id}:{}:{j}.{k}", l.out.id);
                        shape.by.clone_from(&id);
                        shape.m =
                            then(&l.out.m, &translation([mm(off[0]), mm(off[1]), mm(off[2])]));
                        let out = OpOutput::new(made.clone(), copies.exactness);
                        fresh.push(((j, k, s), live_from(&c, &out, shape, l.out.volume)?));
                    }
                }
                // in the page's order: row by row, copy by copy, each source in turn
                fresh.sort_by_key(|(order, _)| *order);
                for (_, l) in fresh {
                    ids.push(l.out.id.clone());
                    self.live.push(l);
                }
                let copies = n * n2 - 1;
                row.message = format!(
                    "{copies} cop{}, each moved by whole grid steps, Exact.",
                    if copies == 1 { "y" } else { "ies" }
                );
            }
            _ => {
                let env = &self.env;
                let d = [
                    length0(env, f, "dx", "The distance")?,
                    length0(env, f, "dy", "The distance")?,
                    length0(env, f, "dz", "The distance")?,
                ];
                let off = [steps(d[0])?, steps(d[1])?, steps(d[2])?];
                for &k in &src {
                    let l = &self.live[k];
                    let moved = Moved {
                        body: &l.body,
                        exactness: l.exactness,
                        accept: None,
                    };
                    let out = move_by(&c, moved, off).map_err(|e| refused(&e))?;
                    let mut shape = l.out.clone();
                    shape.m = then(&l.out.m, &translation(d));
                    let nl = live_from(&c, &out, shape, l.out.volume)?;
                    ids.push(nl.out.id.clone());
                    self.live[k] = nl;
                }
                row.message = format!(
                    "Moved {} mm, Exact.",
                    d.iter().map(|v| fmt_mm(*v)).collect::<Vec<_>>().join(", ")
                );
            }
        }
        row.bodies.clone_from(&ids);
        self.left.insert(id, Left::Bodies(ids));
        Ok(())
    }
}

/// The parameter table, worked out: every parameter's value of its kind, in any order, with loops
/// of formulas refused (`construct_formulas::evaluate`).
fn read_params(part: &Value) -> (Env, Vec<ParamRow>) {
    let list: &[Value] = part
        .get("params")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    let kind_of = |p: &Value| -> (Kind, &'static str) {
        match p.get("kind").and_then(Value::as_str) {
            Some("angle") => (Kind::Angle, "angle"),
            Some("count") => (Kind::Count, "count"),
            _ => (Kind::Length, "length"),
        }
    };
    let entries: Vec<Entry> = list
        .iter()
        .map(|p| Entry {
            name: p
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            text: match p.get("value") {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            },
            kind: kind_of(p).0,
        })
        .collect();
    let mut env = Env::new();
    let mut rows = Vec::new();
    for ((p, entry), result) in list.iter().zip(&entries).zip(evaluate(&entries)) {
        let formula = matches!(p.get("value"), Some(Value::String(s))
            if s.trim().parse::<f64>().is_err());
        let kind = kind_of(p).1;
        match result {
            Ok(value) => {
                rows.push(ParamRow {
                    name: entry.name.clone(),
                    state: "built",
                    message: String::new(),
                    value: Some(value.to_f64()),
                    kind,
                    formula,
                });
                env.insert(entry.name.clone(), value);
            }
            Err(e) => rows.push(ParamRow {
                name: entry.name.clone(),
                state: "failed",
                message: if entry.text.trim().is_empty() {
                    format!("{} is not set.", entry.name)
                } else {
                    e.message
                },
                value: None,
                kind,
                formula,
            }),
        }
    }
    (env, rows)
}

/// Builds a part's recipe, given as a JSON value.
#[must_use]
pub fn build_part(part: &Value) -> Answer {
    build_with_bodies(part).0
}

/// Builds a part, and keeps the kernel's bodies behind the answer's, in the same order, with their
/// exactness, for an assembly to place.
pub(crate) fn build_with_bodies(part: &Value) -> (Answer, Vec<(Body, Exactness)>) {
    let (env, params) = read_params(part);
    let feats: &[Value] = part
        .get("features")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    let end = part
        .get("end")
        .and_then(Value::as_u64)
        .map_or(feats.len(), |e| {
            usize::try_from(e).unwrap_or(usize::MAX).min(feats.len())
        });
    let mut b = Builder {
        env,
        feats,
        rows: Vec::new(),
        left: BTreeMap::new(),
        live: Vec::new(),
    };
    let no_fields = Map::new();
    for (i, fv) in feats.iter().enumerate() {
        let f = fv.as_object().unwrap_or(&no_fields);
        let mut row = FeatureRow {
            id: text(f, "id"),
            state: "built",
            message: String::new(),
            bodies: Vec::new(),
            convex: None,
        };
        if i >= end {
            row.state = "rolled_back";
            row.message = "After the end of the history.".into();
        } else if f.get("suppressed").and_then(Value::as_bool) == Some(true) {
            row.state = "suppressed";
            row.message = "Suppressed: kept in the history, not built.".into();
        } else {
            match b.feature(i, f, &mut row) {
                Ok(()) => {}
                Err(Refusal::Failed(m)) => {
                    (row.state, row.message, row.bodies) = ("failed", m, Vec::new())
                }
                Err(Refusal::Waits(m)) => {
                    (row.state, row.message, row.bodies) = ("waits", m, Vec::new())
                }
            }
        }
        b.rows.push(row);
    }
    let (bodies, kept): (Vec<BodyOut>, Vec<(Body, Exactness)>) = b
        .live
        .into_iter()
        .map(|l| (l.out, (l.body, l.exactness)))
        .unzip();
    let volume = bodies.iter().map(|x| x.volume).sum();
    let bbox = bodies.iter().map(|x| x.bbox).reduce(|a, x| {
        [
            [
                a[0][0].min(x[0][0]),
                a[0][1].min(x[0][1]),
                a[0][2].min(x[0][2]),
            ],
            [
                a[1][0].max(x[1][0]),
                a[1][1].max(x[1][1]),
                a[1][2].max(x[1][2]),
            ],
        ]
    });
    let answer = Answer {
        engine: "kernel",
        params,
        features: b.rows,
        bodies,
        volume,
        bbox,
    };
    (answer, kept)
}
/// Builds a part's recipe given as JSON text, and gives the answer as JSON text, or
/// `{"failed": {code, message}}` for text that is not a recipe.
#[must_use]
pub fn build_json(recipe: &str) -> String {
    match serde_json::from_str::<Value>(recipe) {
        Ok(v) if v.is_object() => serde_json::to_string(&build_part(&v)).unwrap_or_else(|e| {
            json!({ "failed": { "code": "Unwritable", "message": e.to_string() } }).to_string()
        }),
        Ok(_) => json!({ "failed": { "code": "BadRecipe", "message": "A part's recipe is a JSON object." } }).to_string(),
        Err(e) => json!({ "failed": { "code": "BadRecipe", "message": e.to_string() } }).to_string(),
    }
}
