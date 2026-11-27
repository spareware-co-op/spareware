//! A layered wall in plan (job 4.1, CON-A4-F2-P1): where each of its layer faces lies on the
//! kernel's grid, read from the filled recipe of a `wall.layered`. Both builders ([`crate::kernel`]
//! and [`crate::stand_in`]) and hosting ([`crate::hosting`]) read a layered wall through this one
//! module, so they cannot disagree on where a face is.
//!
//! The wall's reference line runs from `start` to `end`, in grid steps of the element's frame
//! (the model's plan less its placement). Walking it that way, its exterior is on the right, as
//! for the old box. A face `o` steps to the right of the line (to the left when `o` is negative)
//! runs from `start + R(o)` to `end + R(o)`, where `R(o)` is the true offset
//! `o * (dy, -dx) / L` (`L` the line's length) with each coordinate rounded to the nearest grid
//! step, a half away from zero ([`Plan::across`]).
//!
//! That is the one place a corner is rounded, and each face is rounded once: both its ends move by
//! the same whole vector, so the face stays exactly parallel to the line, two layers that meet
//! share their face bit for bit, and every layer's strip is a parallelogram on grid points. A
//! parallelogram with an area is strictly convex, so the kernel's exact prism takes its extrusion
//! (geom-solidcore's `extrude`: a hole-free, level, straight-up, strictly convex profile goes to
//! `make.prism`). The bound: each coordinate of a corner is within half a step (78.125 nm) of the
//! true point; the tests pin it. A wall along an axis has no rounding at all.
//!
//! Numbers: exact integers throughout. The nearest step to `o * c / L` is found by comparing
//! squares, `(2m - 1)^2 * L^2` against `(2 * o * c)^2`, in 256-bit products of two 128-bit
//! numbers; no float is used. Every coordinate and offset is within the kernel's grid,
//! `2^40 - 1` steps of the origin ([`LIMIT`]), which keeps every product in range.
//!
//! The filled recipe ([`Layered::of`]): `start` and `end`, each `[x, y]` as construct-params'
//! stored lengths; `height`, a stored length (a whole number of millimetres also reads, as a
//! top tie the old box way); `layers`, from the exterior face to the interior face, each with its
//! `function`, `material`, and its two faces' offsets from the reference line, `exterior` and
//! `interior`, as stored lengths, positive toward the exterior side. The regenerator fills them
//! from the wall's type (construct-regen's `walls`). When the wall meets others, `joins` too
//! ([`Joints`], derived from construct-walls' join rule by construct-regen's `joins`, job 5.1):
//! each layer's pieces once its joints are made, which the kernel builder extrudes in place of the
//! layer's strip, and the whole wall's outline. Every corner there is the crossing of two layer
//! faces of walls along the plan's axes, so it is on the grid with no rounding.
//!
//! When it wraps (job 6.1), `wrap` too ([`Wrap`], filled by construct-regen's `walls` from the
//! wall type and the wall's own choice): its wrapping at free ends and at inserts, each layer's
//! wraps flag and the core's two boundaries. The free ends' turned layers are among the joined
//! pieces; the builder turns the layers at inserts.
//!
//! Depends on construct-model (recipes), construct-params (the stored length) and
//! construct-walls (the wrapping choices). Nothing here is interim: the rounding rule is the
//! kernel's grid's, and an exact kernel takes the same corners.

use construct_model::geometry::{GeometryError, Recipe};
use construct_params::{Length, Value};
use construct_walls::{AtEnds, AtInserts};
use serde_json::Value as Json;

/// The operation of a layered wall.
pub const OP: &str = "wall.layered";

/// The farthest a coordinate or an offset may be from the origin, in grid steps: the kernel's
/// grid, `2^40 - 1` steps (171.8 km on `Q0`).
pub const LIMIT: i64 = 1_099_511_627_775;

/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn too_big() -> GeometryError {
    fault("TooBig", "a length is beyond the kernel's grid")
}

/// `steps` of the building grid to the nearest whole millimetre, a half away from zero, and
/// whether that is exact: how a placement or a display in whole millimetres holds a corner of a
/// layered wall, which need not lie on one.
#[must_use]
pub fn nearest_mm(steps: i64) -> (i64, bool) {
    let per = Length::STEPS_PER_MM;
    let floor = steps.checked_div_euclid(per).unwrap_or(0);
    let rest = steps.checked_rem_euclid(per).unwrap_or(0);
    // 0 <= rest < 6,400, so twice it is well within an i64.
    let twice = rest.saturating_mul(2);
    let up = twice > per || (twice == per && floor >= 0);
    (if up { floor.saturating_add(1) } else { floor }, rest == 0)
}

/// Whether `recipe` is a layered wall's.
#[must_use]
pub fn is_layered(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// A layered wall's line as its record holds it, `start` and `end` in grid steps of its frame,
/// read without filling anything: how the walls that meet are found.
#[must_use]
pub fn line_of(recipe: &Recipe) -> Option<([i64; 2], [i64; 2])> {
    if !is_layered(recipe) {
        return None;
    }
    let args = &recipe.args;
    Some((
        point(args.get("start"), "start").ok()?,
        point(args.get("end"), "end").ok()?,
    ))
}

/// The 256-bit product of two 128-bit numbers, as its high and low halves.
fn wide(a: u128, b: u128) -> (u128, u128) {
    const LOW: u128 = u64::MAX as u128;
    let (a1, a0) = (a.wrapping_shr(64), a & LOW);
    let (b1, b0) = (b.wrapping_shr(64), b & LOW);
    // Each partial product is of two numbers under 2^64, so under 2^128; each sum below is of
    // three numbers under 2^64, or of a number under 2^128 less what was carried out of it.
    let (p00, p01, p10, p11) = (
        a0.wrapping_mul(b0),
        a0.wrapping_mul(b1),
        a1.wrapping_mul(b0),
        a1.wrapping_mul(b1),
    );
    let mid = p00
        .wrapping_shr(64)
        .wrapping_add(p01 & LOW)
        .wrapping_add(p10 & LOW);
    let low = (p00 & LOW) | (mid & LOW).wrapping_shl(64);
    let high = p11
        .wrapping_add(p01.wrapping_shr(64))
        .wrapping_add(p10.wrapping_shr(64))
        .wrapping_add(mid.wrapping_shr(64));
    (high, low)
}

/// Whether `m - 1/2 <= q / sqrt(dd)`, asked of the squares: `(2m - 1)^2 * dd <= (2q)^2`, where
/// `target` is `(2q)^2`. Every `m` here is at most `2^40 + 1`, so `(2m - 1)^2` is under `2^84`.
fn reached(m: u128, dd: u128, target: (u128, u128)) -> bool {
    if m == 0 {
        return true;
    }
    let x = m.wrapping_mul(2).wrapping_sub(1);
    wide(x.wrapping_mul(x), dd) <= target
}

/// The nearest whole number to `o * c / sqrt(dd)`, a half away from zero, where `c * c <= dd`
/// (so the answer is at most `|o|` in size) and `|o|`, `|c|` are within [`LIMIT`].
fn nearest(o: i64, c: i64, dd: u128) -> i64 {
    let q = u128::from(o.unsigned_abs()).wrapping_mul(u128::from(c.unsigned_abs()));
    let two_q = q.wrapping_mul(2);
    let target = wide(two_q, two_q);
    // `reached(lo)` holds and `reached(hi)` does not: the answer is at most |o|.
    let (mut lo, mut hi) = (0_u128, u128::from(o.unsigned_abs()).wrapping_add(1));
    while hi.wrapping_sub(lo) > 1 {
        let mid = lo.wrapping_add(hi.wrapping_sub(lo).wrapping_shr(1));
        if reached(mid, dd, target) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // At most |o|, which is within LIMIT, so it is an i64.
    let size = i64::try_from(lo).unwrap_or(i64::MAX);
    if (o < 0) == (c < 0) || size == 0 {
        size
    } else {
        size.wrapping_neg()
    }
}

fn within(value: i64) -> Result<i64, GeometryError> {
    if value.unsigned_abs() > LIMIT.unsigned_abs() {
        return Err(too_big());
    }
    Ok(value)
}

fn add2(a: [i64; 2], b: [i64; 2]) -> Result<[i64; 2], GeometryError> {
    let [ax, ay] = a;
    let [bx, by] = b;
    Ok([
        within(ax.checked_add(bx).ok_or_else(too_big)?)?,
        within(ay.checked_add(by).ok_or_else(too_big)?)?,
    ])
}

fn less2(a: [i64; 2], b: [i64; 2]) -> Result<[i64; 2], GeometryError> {
    let [ax, ay] = a;
    let [bx, by] = b;
    Ok([
        ax.checked_sub(bx).ok_or_else(too_big)?,
        ay.checked_sub(by).ok_or_else(too_big)?,
    ])
}

/// `a x b`, the cross product of two plan vectors, each within twice [`LIMIT`].
fn cross(a: [i64; 2], b: [i64; 2]) -> i128 {
    let [ax, ay] = a;
    let [bx, by] = b;
    // Each product is under 2^82 in size, so the difference is an i128.
    i128::from(ax)
        .wrapping_mul(i128::from(by))
        .wrapping_sub(i128::from(ay).wrapping_mul(i128::from(bx)))
}

/// Which way a wall along an axis runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Along +x.
    PlusX,
    /// Along -x.
    MinusX,
    /// Along +y.
    PlusY,
    /// Along -y.
    MinusY,
}

impl Axis {
    /// Its unit direction.
    #[must_use]
    pub fn unit(self) -> [i64; 2] {
        match self {
            Axis::PlusX => [1, 0],
            Axis::MinusX => [-1, 0],
            Axis::PlusY => [0, 1],
            Axis::MinusY => [0, -1],
        }
    }

    /// The plan's axis it runs along: 0 for x, 1 for y.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Axis::PlusX | Axis::MinusX => 0,
            Axis::PlusY | Axis::MinusY => 1,
        }
    }
}

/// A wall's reference line in plan, from its start to its end, in grid steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    start: [i64; 2],
    end: [i64; 2],
    d: [i64; 2],
    dd: u128,
}

impl Plan {
    /// The line from `start` to `end`.
    ///
    /// # Errors
    ///
    /// `ZeroLength` when they are the same point; `TooBig` for a point beyond the grid.
    pub fn new(start: [i64; 2], end: [i64; 2]) -> Result<Plan, GeometryError> {
        for c in start.into_iter().chain(end) {
            within(c)?;
        }
        let d = less2(end, start)?;
        if d == [0, 0] {
            return Err(fault(
                "ZeroLength",
                "a wall's start and end are the same point: it has no length",
            ));
        }
        let [dx, dy] = d;
        let square =
            |v: i64| u128::from(v.unsigned_abs()).wrapping_mul(u128::from(v.unsigned_abs()));
        // Each square is under 2^82, so their sum is a u128.
        let dd = square(dx).wrapping_add(square(dy));
        Ok(Plan { start, end, d, dd })
    }

    /// Where it starts.
    #[must_use]
    pub fn start(&self) -> [i64; 2] {
        self.start
    }

    /// Where it ends.
    #[must_use]
    pub fn end(&self) -> [i64; 2] {
        self.end
    }

    /// The vector from its start to its end.
    #[must_use]
    pub fn direction(&self) -> [i64; 2] {
        self.d
    }

    /// The axis it runs along, when it runs along one.
    #[must_use]
    pub fn axis(&self) -> Option<Axis> {
        match self.d {
            [x, 0] if x > 0 => Some(Axis::PlusX),
            [x, 0] if x < 0 => Some(Axis::MinusX),
            [0, y] if y > 0 => Some(Axis::PlusY),
            [0, y] if y < 0 => Some(Axis::MinusY),
            _ => None,
        }
    }

    /// Its length in steps, when it runs along an axis (then it is whole).
    #[must_use]
    pub fn axis_length(&self) -> Option<i64> {
        self.axis().map(|_| {
            let [x, y] = self.d;
            x.unsigned_abs()
                .max(y.unsigned_abs())
                .try_into()
                .unwrap_or(i64::MAX)
        })
    }

    /// `R(o)`: the true offset of a face `offset` steps to the right of the line, each coordinate
    /// rounded to the nearest step, a half away from zero. The one place a corner is rounded.
    ///
    /// # Errors
    ///
    /// `TooBig` for an offset beyond the grid.
    pub fn across(&self, offset: i64) -> Result<[i64; 2], GeometryError> {
        within(offset)?;
        let [dx, dy] = self.d;
        Ok([
            nearest(offset, dy, self.dd),
            nearest(offset, dx.checked_neg().ok_or_else(too_big)?, self.dd),
        ])
    }

    /// The face `offset` steps to the right of the line: from its start's corner to its end's.
    ///
    /// # Errors
    ///
    /// `TooBig` for a corner beyond the grid.
    pub fn face(&self, offset: i64) -> Result<[[i64; 2]; 2], GeometryError> {
        let r = self.across(offset)?;
        Ok([add2(self.start, r)?, add2(self.end, r)?])
    }

    /// The strip between the faces `a` and `b` steps to the right of the line, its four corners
    /// counter-clockwise seen from above, and its area in square steps, exactly: a parallelogram's
    /// area is the size of the cross product of its sides.
    ///
    /// # Errors
    ///
    /// `Degenerate` for a strip with no area on the grid (its two faces round to one line), and
    /// `TooBig` for a corner beyond the grid.
    pub fn strip(&self, a: i64, b: i64) -> Result<([[i64; 2]; 4], u128), GeometryError> {
        let (ra, rb) = (self.across(a)?, self.across(b)?);
        let side = less2(rb, ra)?;
        let turn = cross(self.d, side);
        let corners = [
            add2(self.start, ra)?,
            add2(self.end, ra)?,
            add2(self.end, rb)?,
            add2(self.start, rb)?,
        ];
        let corners = match turn.signum() {
            1 => corners,
            -1 => {
                let [p0, p1, p2, p3] = corners;
                [p0, p3, p2, p1]
            }
            _ => {
                return Err(fault(
                    "Degenerate",
                    "a layer is too thin to have an area on the kernel's grid at this angle",
                ));
            }
        };
        Ok((corners, turn.unsigned_abs()))
    }
}

/// One layer of a layered wall, as its filled recipe gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strip {
    /// What it does, in construct-walls' words (`structure`, `finish`, ...).
    pub function: String,
    /// What it is made of, by material ID.
    pub material: String,
    /// Its exterior face's offset from the reference line, in steps, positive toward the exterior.
    pub exterior: i64,
    /// Its interior face's offset, the same way.
    pub interior: i64,
}

impl Strip {
    /// How thick it is, in steps.
    #[must_use]
    pub fn thickness(&self) -> u64 {
        self.exterior.abs_diff(self.interior)
    }
}

/// A layered wall, as its filled recipe gives it: its line, its height and its layers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layered {
    /// Its reference line.
    pub plan: Plan,
    /// How tall it is, from its base, in steps; above zero.
    pub height: i64,
    /// Its layers, from the exterior face to the interior face; one face of each is the next's.
    pub layers: Vec<Strip>,
    /// Its layers' shapes once its joints with the walls it meets are made, when any is
    /// ([`Joints`]); `None` builds each layer as its strip.
    pub joined: Option<Joints>,
    /// Its wrapping, when it wraps anything ([`Wrap`]).
    pub wrap: Option<Wrap>,
}

/// A layered wall's joints, as its filled recipe gives them (`joins`, which the regenerator fills
/// from construct-walls' join rule, job 5.1): each layer's pieces in plan and the whole wall's
/// outline, in grid steps of the element's frame. Each piece is strictly convex, counter-clockwise
/// seen from above, its corners on the grid; a layer with no thickness has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joints {
    /// Each layer's pieces, from the exterior.
    pub layers: Vec<Vec<Vec<[i64; 2]>>>,
    /// The whole wall's outline, counter-clockwise.
    pub outline: Vec<[i64; 2]>,
}

/// The argument a regenerator fills with a layered wall's joints. Derived, never stored.
pub const JOINS: &str = "joins";

/// The argument a regenerator fills with a layered wall's wrapping, when it wraps anything:
/// `{"core":[e,i],"ends":<choice>,"inserts":<choice>,"layers":[<wraps>...]}`. Derived, never
/// stored.
pub const WRAP: &str = "wrap";

/// A layered wall's wrapping, as its filled recipe gives it ([`WRAP`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wrap {
    /// At its free ends.
    pub ends: AtEnds,
    /// At its inserts.
    pub inserts: AtInserts,
    /// Each layer's flag: whether it wraps where its side wraps.
    pub layers: Vec<bool>,
    /// Its core: the layers `core.0..core.1`.
    pub core: (usize, usize),
}

/// The wrapping a filled `wrap` argument gives, checked against the wall's `count` layers.
fn wrap_of(value: Option<&Json>, count: usize) -> Result<Option<Wrap>, GeometryError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let bad = || {
        fault(
            "BadWrap",
            "its wrapping is not as its filled recipe writes it",
        )
    };
    let key = |name: &str| value.get(name).and_then(Json::as_str);
    let ends = key("ends").and_then(AtEnds::from_key).ok_or_else(bad)?;
    let inserts = key("inserts")
        .and_then(AtInserts::from_key)
        .ok_or_else(bad)?;
    let layers = value
        .get("layers")
        .and_then(Json::as_array)
        .ok_or_else(bad)?
        .iter()
        .map(Json::as_bool)
        .collect::<Option<Vec<bool>>>()
        .ok_or_else(bad)?;
    let core = match value
        .get("core")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
    {
        Some([a, b]) => (
            usize::try_from(a.as_u64().ok_or_else(bad)?).map_err(|_| bad())?,
            usize::try_from(b.as_u64().ok_or_else(bad)?).map_err(|_| bad())?,
        ),
        _ => return Err(bad()),
    };
    if layers.len() != count || core.0 >= core.1 || core.1 > count {
        return Err(bad());
    }
    Ok(Some(Wrap {
        ends,
        inserts,
        layers,
        core,
    }))
}

/// Whether every corner of `corners` turns left: a strictly convex polygon, counter-clockwise.
fn convex(corners: &[[i64; 2]]) -> bool {
    let n = corners.len();
    n >= 3
        && (0..n).all(|at| {
            let pick = |k: usize| {
                at.checked_add(k)
                    .and_then(|i| i.checked_rem(n))
                    .and_then(|i| corners.get(i))
                    .copied()
            };
            match (pick(0), pick(1), pick(2)) {
                (Some(a), Some(b), Some(c)) => match (less2(b, a), less2(c, b)) {
                    (Ok(ab), Ok(bc)) => cross(ab, bc) > 0,
                    _ => false,
                },
                _ => false,
            }
        })
}

/// A polygon, a list of `[x, y]` points as stored lengths.
fn polygon(value: &Json, what: &str) -> Result<Vec<[i64; 2]>, GeometryError> {
    let listed = value.as_array().ok_or_else(|| {
        fault(
            "MissingArgument",
            format!("{what} must list its corners, each [x, y] as two stored lengths"),
        )
    })?;
    listed.iter().map(|p| point(Some(p), what)).collect()
}

/// The joints a filled `joins` argument gives, checked: one list of pieces per layer, none for a
/// layer with no thickness, each piece strictly convex and counter-clockwise; `None` for a wall
/// whose joins only say why it is not joined.
fn joints(value: Option<&Json>, layers: &[Strip]) -> Result<Option<Joints>, GeometryError> {
    let Some(listed) = value.and_then(|v| v.get("layers")) else {
        return Ok(None);
    };
    let bad = |why: String| fault("BadJoins", why);
    let listed = listed
        .as_array()
        .ok_or_else(|| bad("its joined layers must be a list".to_owned()))?;
    if listed.len() != layers.len() {
        return Err(bad(format!(
            "it has {} layers and {} joined",
            layers.len(),
            listed.len()
        )));
    }
    let mut out = Vec::with_capacity(listed.len());
    for (at, (pieces, layer)) in listed.iter().zip(layers).enumerate() {
        let pieces = pieces
            .as_array()
            .ok_or_else(|| bad(format!("layer {at}'s pieces must be a list")))?;
        let pieces = pieces
            .iter()
            .map(|p| polygon(p, "a joined layer's piece"))
            .collect::<Result<Vec<_>, _>>()?;
        if (layer.thickness() == 0) != pieces.is_empty() || !pieces.iter().all(|p| convex(p)) {
            return Err(bad(format!(
                "layer {at}'s pieces are not strictly convex polygons of a layer with a thickness"
            )));
        }
        out.push(pieces);
    }
    let outline = value
        .and_then(|v| v.get("outline"))
        .ok_or_else(|| bad("its joins have no outline".to_owned()))
        .and_then(|v| polygon(v, "a joined wall's outline"))?;
    if outline.len() < 3 {
        return Err(bad("its outline has fewer than three corners".to_owned()));
    }
    Ok(Some(Joints {
        layers: out,
        outline,
    }))
}

/// A stored length's steps.
fn stored_length(value: Option<&Json>, what: &str) -> Result<i64, GeometryError> {
    let text = value.and_then(Json::as_str).ok_or_else(|| {
        fault(
            "MissingArgument",
            format!("{what} must be a stored length (length:q0:<steps>)"),
        )
    })?;
    match Value::from_stored(text) {
        Some(Value::Length(length)) => within(length.steps()),
        _ => Err(fault(
            "MissingArgument",
            format!("{what} is {text}, not a stored length"),
        )),
    }
}

/// A point, `[x, y]` as two stored lengths.
fn point(value: Option<&Json>, what: &str) -> Result<[i64; 2], GeometryError> {
    match value.and_then(Json::as_array).map(Vec::as_slice) {
        Some([x, y]) => Ok([stored_length(Some(x), what)?, stored_length(Some(y), what)?]),
        _ => Err(fault(
            "MissingArgument",
            format!("{what} must be a point: [x, y] as two stored lengths"),
        )),
    }
}

/// A height: a stored length, or a whole number of millimetres.
fn height(value: Option<&Json>) -> Result<i64, GeometryError> {
    let steps = match value {
        Some(Json::Number(mm)) => {
            let mm = mm.as_i64().ok_or_else(|| {
                fault(
                    "MissingArgument",
                    "height must be a whole number of millimetres",
                )
            })?;
            Length::checked_from_mm(mm).ok_or_else(too_big)?.steps()
        }
        other => stored_length(other, "height")?,
    };
    if steps <= 0 {
        return Err(fault(
            "TopBelowBase",
            "its top is at or below its base: a wall needs a height above zero",
        ));
    }
    within(steps)
}

fn text(
    object: &serde_json::Map<String, Json>,
    key: &str,
    at: usize,
) -> Result<String, GeometryError> {
    object
        .get(key)
        .and_then(Json::as_str)
        .map(str::to_owned)
        .ok_or_else(|| fault("MissingArgument", format!("layer {at} has no {key}")))
}

impl Layered {
    /// The layered wall a filled `wall.layered` recipe makes.
    ///
    /// # Errors
    ///
    /// `MissingArgument` for an argument that is not as the module header says, `Unfilled` for
    /// layers not yet read from the wall's type, `ZeroLength` for a line with no length,
    /// `TopBelowBase` for a height at or below zero, `BadLayers` for layers that do not run
    /// face to face one way across the wall or have no thickness between them, and `TooBig`
    /// beyond the grid.
    pub fn of(recipe: &Recipe) -> Result<Layered, GeometryError> {
        let args = &recipe.args;
        let plan = Plan::new(
            point(args.get("start"), "start")?,
            point(args.get("end"), "end")?,
        )?;
        let height = height(args.get("height"))?;
        let listed = match args.get("layers") {
            Some(Json::Array(listed)) => listed,
            Some(Json::Object(_)) => {
                return Err(fault(
                    "Unfilled",
                    "its layers are read from its wall type, which was not read before building",
                ));
            }
            _ => {
                return Err(fault(
                    "MissingArgument",
                    "layers must list the wall's layers",
                ));
            }
        };
        let mut layers = Vec::with_capacity(listed.len());
        for (at, layer) in listed.iter().enumerate() {
            let object = layer
                .as_object()
                .ok_or_else(|| fault("MissingArgument", format!("layer {at} is not an object")))?;
            layers.push(Strip {
                function: text(object, "function", at)?,
                material: text(object, "material", at)?,
                exterior: stored_length(object.get("exterior"), "a layer's exterior")?,
                interior: stored_length(object.get("interior"), "a layer's interior")?,
            });
        }
        let bad = |why: String| fault("BadLayers", why);
        let (Some(first), Some(last)) = (layers.first(), layers.last()) else {
            return Err(bad("a wall has at least one layer".to_owned()));
        };
        if first.exterior == last.interior {
            return Err(bad("its layers have no thickness between them".to_owned()));
        }
        let outward = first.exterior > last.interior;
        for (at, layer) in layers.iter().enumerate() {
            let one_way = if outward {
                layer.exterior >= layer.interior
            } else {
                layer.exterior <= layer.interior
            };
            if !one_way {
                return Err(bad(format!(
                    "layer {at} runs the other way across the wall"
                )));
            }
            if let Some(next) = layers.get(at.saturating_add(1))
                && next.exterior != layer.interior
            {
                return Err(bad(format!(
                    "layer {} does not start where layer {at} ends",
                    at.saturating_add(1)
                )));
            }
        }
        let joined = joints(args.get(JOINS), &layers)?;
        let wrap = wrap_of(args.get(WRAP), layers.len())?;
        Ok(Layered {
            plan,
            height,
            layers,
            joined,
            wrap,
        })
    }

    /// The offsets of its two finish faces: the exterior face of its first layer and the interior
    /// face of its last.
    #[must_use]
    pub fn outer(&self) -> (i64, i64) {
        (
            self.layers.first().map_or(0, |l| l.exterior),
            self.layers.last().map_or(0, |l| l.interior),
        )
    }

    /// How thick it is, in steps.
    #[must_use]
    pub fn thickness(&self) -> u64 {
        let (a, b) = self.outer();
        a.abs_diff(b)
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "tests; f64 only to check a bound against, never to make a value"
)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A 3-4-5 wall's faces are exact: the offsets are whole steps.
    #[test]
    fn a_face_of_a_wall_on_whole_ratios_is_exact() {
        let plan = Plan::new([0, 0], [3000, 4000]).unwrap();
        // The right of (3, 4) is (4, -3) / 5.
        assert_eq!(plan.across(500).unwrap(), [400, -300]);
        assert_eq!(plan.across(-500).unwrap(), [-400, 300]);
        assert_eq!(plan.face(500).unwrap(), [[400, -300], [3400, 3700]]);
    }

    /// Along an axis, a face is the line moved by its offset exactly: to the right of +x is -y.
    #[test]
    fn a_face_of_a_wall_along_an_axis_is_its_offset() {
        let x = Plan::new([100, 200], [10_100, 200]).unwrap();
        assert_eq!(x.axis(), Some(Axis::PlusX));
        assert_eq!(x.axis_length(), Some(10_000));
        assert_eq!(x.across(80_000).unwrap(), [0, -80_000]);
        let minus_y = Plan::new([0, 0], [0, -7]).unwrap();
        assert_eq!(minus_y.axis(), Some(Axis::MinusY));
        assert_eq!(minus_y.across(3).unwrap(), [-3, 0]);
        assert_eq!(Plan::new([0, 0], [1, 1]).unwrap().axis(), None);
    }

    /// The bound: every corner is within half a step of the true point, on every direction tried,
    /// and a half rounds away from zero.
    #[test]
    fn a_corner_is_within_half_a_step_of_the_true_point() {
        let directions = [
            [1, 1],
            [7, 3],
            [-5, 11],
            [6_400_000, 1],
            [-1_234_567, -7_654_321],
            [123_456_789, 98_765],
            [1, -1_000_000_000],
        ];
        for d in directions {
            let plan = Plan::new([0, 0], d).unwrap();
            let length = ((d[0] as f64).powi(2) + (d[1] as f64).powi(2)).sqrt();
            for offset in [1_i64, 2, 3, 80_000, 576_000, -737_280, 64_000_000] {
                let [x, y] = plan.across(offset).unwrap();
                let tx = offset as f64 * d[1] as f64 / length;
                let ty = -(offset as f64) * d[0] as f64 / length;
                assert!(
                    (x as f64 - tx).abs() <= 0.5 + 1e-6,
                    "{d:?} {offset}: {x} vs {tx}"
                );
                assert!(
                    (y as f64 - ty).abs() <= 0.5 + 1e-6,
                    "{d:?} {offset}: {y} vs {ty}"
                );
            }
        }
        // On (6, 8), R(1) is (0.8, -0.6): each to its nearest step, (1, -1).
        let plan = Plan::new([0, 0], [6, 8]).unwrap();
        assert_eq!(plan.across(1).unwrap(), [1, -1]);
        assert_eq!(plan.across(-1).unwrap(), [-1, 1]);
        // (1, 1) at offset 1: (0.7071, -0.7071) -> (1, -1).
        let plan = Plan::new([0, 0], [1, 1]).unwrap();
        assert_eq!(plan.across(1).unwrap(), [1, -1]);
    }

    /// Neighbouring strips share their face exactly, each is a parallelogram counter-clockwise
    /// from above, and the areas add up to the whole strip's.
    #[test]
    fn strips_share_their_faces_and_their_areas_add_up() {
        let plan = Plan::new([1000, -2000], [64_001_000, 25_598_000]).unwrap();
        let faces = [368_000_i64, 288_000, 32_000, -288_000, -368_000];
        let mut sum = 0_u128;
        for pair in faces.windows(2) {
            let (corners, area) = plan.strip(pair[0], pair[1]).unwrap();
            let [p0, p1, p2, p3] = corners;
            assert_eq!(less2(p1, p0).unwrap(), less2(p2, p3).unwrap());
            assert!(cross(less2(p1, p0).unwrap(), less2(p2, p1).unwrap()) > 0);
            sum += area;
        }
        let (_, whole) = plan.strip(faces[0], faces[4]).unwrap();
        assert_eq!(sum, whole);
        let (corners, _) = plan.strip(288_000, 32_000).unwrap();
        let (next, _) = plan.strip(32_000, -288_000).unwrap();
        let shared = plan.face(32_000).unwrap();
        assert!(corners.contains(&shared[0]) && corners.contains(&shared[1]));
        assert!(next.contains(&shared[0]) && next.contains(&shared[1]));
    }

    #[test]
    fn a_line_with_no_length_or_off_the_grid_is_refused() {
        assert_eq!(Plan::new([5, 5], [5, 5]).unwrap_err().code, "ZeroLength");
        assert_eq!(
            Plan::new([0, 0], [LIMIT + 1, 0]).unwrap_err().code,
            "TooBig"
        );
        let plan = Plan::new([0, 0], [LIMIT, LIMIT]).unwrap();
        assert_eq!(plan.across(LIMIT + 1).unwrap_err().code, "TooBig");
        assert!(plan.across(LIMIT).is_ok());
        let thin = Plan::new([0, 0], [1, 1]).unwrap();
        assert_eq!(thin.strip(1, 1).unwrap_err().code, "Degenerate");
    }

    fn recipe(args: serde_json::Value) -> Recipe {
        Recipe::construct(OP, serde_json::from_value(args).unwrap())
    }

    fn q0(steps: i64) -> String {
        Value::Length(Length::from_steps(steps)).stored()
    }

    /// A filled recipe reads as written; layers must run face to face, one way across.
    #[test]
    fn a_filled_recipe_reads_its_line_height_and_layers() {
        let layer = |f: &str, a: i64, b: i64| json!({ "function": f, "material": "m", "exterior": q0(a), "interior": q0(b) });
        let args = json!({
            "start": [q0(0), q0(0)],
            "end": [q0(25_600_000), q0(0)],
            "height": q0(17_280_000),
            "layers": [layer("finish", 368_000, 288_000), layer("structure", 288_000, -368_000)],
        });
        let wall = Layered::of(&recipe(args.clone())).unwrap();
        assert_eq!(wall.height, 17_280_000);
        assert_eq!(wall.layers.len(), 2);
        assert_eq!(wall.outer(), (368_000, -368_000));
        assert_eq!(wall.thickness(), 736_000);
        let mut mm = args.clone();
        mm["height"] = json!(2700);
        assert_eq!(Layered::of(&recipe(mm)).unwrap().height, 17_280_000);
        let mut low = args.clone();
        low["height"] = json!(q0(0));
        assert_eq!(Layered::of(&recipe(low)).unwrap_err().code, "TopBelowBase");
        let mut unfilled = args.clone();
        unfilled["layers"] = json!({ "wall": "type" });
        assert_eq!(Layered::of(&recipe(unfilled)).unwrap_err().code, "Unfilled");
        let mut gap = args.clone();
        gap["layers"] = json!([
            layer("finish", 368_000, 288_000),
            layer("structure", 280_000, 0)
        ]);
        assert_eq!(Layered::of(&recipe(gap)).unwrap_err().code, "BadLayers");
        let mut back = args;
        back["layers"] = json!([layer("finish", 0, 10), layer("structure", 10, -20)]);
        assert_eq!(Layered::of(&recipe(back)).unwrap_err().code, "BadLayers");
    }
}
