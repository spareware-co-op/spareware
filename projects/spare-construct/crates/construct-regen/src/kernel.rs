//! The builder over the geometry kernel's engine seam: Spare Construct's recipes made by whatever
//! engine answers `engine_geometry`'s [`Engine`] trait, without this crate naming one.
//!
//! What it does: [`Kernel`] implements Construct's [`Geometry`] for any [`Engine`], the composition
//! root of the geometry join (chapter 25's facade on one side, Construct's recipes on the other):
//!
//! - `wall.straight`: the wall's plan, a rectangle as long and as thick as the recipe says along
//!   its axis (x, or y when `axis` is `y`), extruded straight up by its height through
//!   [`Engine::extrude`]. Its bounds are the stand-in's box, so placement means the same.
//! - `wall.layered`: one solid per layer of its type, each its strip in plan
//!   ([`crate::wall_plan`]) extruded straight up, with each opening cut through every layer, in
//!   the child module `layered` (job 4.1); its lengths are construct-params' stored steps, which
//!   [`Units::from_q0`] hands to the engine, and its displays are rounded to the nearest
//!   millimetre ([`Units::nearest_mm`]) where a face is not on one.
//! - `block`: its plan rectangle, width by depth, extruded the same way.
//! - `floor`: one solid per layer of its floor type, each its boundary with its holes extruded
//!   from the layer's bottom face up to its top, the floor's top face at `z = 0`, in the child
//!   module `floor` (job 4.3); its lengths are construct-params' steps ([`crate::floor_plan`]).
//! - Openings and shafts (job 4.2): a plain opening (`opening.wall`) is cut through every layer of
//!   its wall on the door's path below ([`opening`]); a floor's openings and the shafts passing
//!   through it are more holes in each layer's profile (the child module `floor`). The void itself
//!   is drawn by [`crate::voids`], as the stand-in draws it.
//! - `roof`: one solid per layer of its roof type, each the solid construct-roofs worked out for
//!   it (a flat layer extruded straight up, a sloped layer lofted from verge to verge), in the
//!   child module `roof` (job 5.3); its points are construct-params' steps ([`crate::roof_plan`]).
//! - `room`: one solid, its loop with its holes extruded from its base up to its upper limit, in
//!   the child module `room` (job 5.4); its lengths are construct-params' steps
//!   ([`crate::room_plan`]).
//! - `door.single`: a box in the wall's thickness, extruded the same way: a simple solid placed in
//!   its opening, the old door kept for the documents that hold it. [`Extrusion::door`] is the one
//!   place its shape is decided.
//! - `family.instance` (job 6.4, CON-A7-F3-P2): a placed family, its solids made by the recipe
//!   evaluator (`construct_families::evaluate_turned`) through this builder's engine, turned as
//!   the family stands, each meshed and drawn as a part with its material, in the child module
//!   `family`. What a type builds to is kept by the evaluator's key ([`CacheKey`]: the recipe
//!   version, the type's values and the turn), so a type is built once however many are placed
//!   and however often their walls move. Its recipe comes from the builder's [`FamilySource`]
//!   ([`Kernel::with_families`]; [`Kernel::new`] uses the library baked in at build time, interim
//!   until job 7.6, [`crate::families`]). The wall it is hosted in cuts its opening on the same
//!   path as a door's ([`opening`]), through every layer.
//! - A wall with doors hosted in it ([`Kernel::build_hosting`], [`Hosted`]): the wall's extrusion
//!   less each door's opening, through [`Engine::subtract`], one door after another in the order
//!   of their keys, so the same model gives the same bytes whatever order the doors come in. An
//!   opening is the door's width and height through the wall's thickness; where it is placed, and
//!   every case it refuses (an opening at or past the wall's ends, as tall as the wall or taller,
//!   below its base, overlapping or touching another, or with a face within the engine's
//!   tolerance of another face without being on it), is decided in [`opening`] before the engine
//!   is asked, each refusal a [`GeometryError`] naming the door and why.
//! - Two levels of detail, each from [`Engine::mesh`] in whole millimetres: level 0 the body
//!   itself, openings cut, straying from it by nothing; level 1 the extrusion before its openings
//!   were cut, straying by at most the bound [`opening`] gives (none for a wall without openings).
//! - For a drawing: [`Kernel::solid`] gives a recipe's solid made the same way, with the plane of
//!   each named face (its carrier, as [`Engine::section`] names a cut edge's face), and
//!   [`Kernel::section`] cuts it by a plane through the engine.
//! - Every face carries a persistent name for its role (chapter 20.2): a wall's and a door's
//!   `exterior`, `interior`, `top`, `base`, `end0` and `end1`; a block's `cap.start`, `cap.end` and
//!   `side.0` to `side.3`. A name is the kernel's `NameScheme v1` id of the face's `Created` name:
//!   the step is the recipe's operation (`owner/op`), the role is the face's, and nothing of the
//!   recipe's lengths goes in, so a wall rebuilt or lengthened keeps every name, and a door hosted
//!   on its exterior keeps its face. An opening keeps them too: a face it cuts into keeps its name
//!   (a base a door to the floor cuts in two is named once, as its carrier), and the faces it
//!   makes, its jambs, its head and, above the base, its sill, are named by the door that made
//!   them as chapter 11.9 names a tool's faces: `Created { op, role: tool(door), from: [the
//!   opening's own face] }`.
//! - [`Geometry::build_hosting`] is [`Kernel::build_hosting`]: the regenerator hands a wall the
//!   doors hosted in it (construct-regen's `hosting`), each where its record puts it in the wall's
//!   frame, in whole millimetres. [`fits`] asks the same rules of openings without the engine, so
//!   a command refuses a door this builder would refuse, in this builder's words; [`names`] gives
//!   a recipe's face names without building it, so a door names its wall's face before the wall
//!   is built again.
//! - Its [`Edition`] is `construct.kernel`, this builder's version, `NameScheme` 1, and the
//!   engine's own report ([`Engine::edition`]: its name, its version or pin, its grid step, its
//!   tolerances and which part answers each operation). A document built on another edition is
//!   not built here; the regenerator reports it (FORMAT.md, "A document").
//!
//! Units: [`Units`] is the one place whole millimetres become the engine's grid steps and come
//! back, exactly or not at all (chapter 6: on `Q0` one millimetre is 6,400 steps). A hosted door's
//! place in its wall is a grid point ([`Units::offset`] makes it from the two placements).
//!
//! Guarantees: pure and deterministic as the [`Geometry`] trait asks: the same recipe on the same
//! engine gives the same bytes, and the engine's results are the kernel's checked solids, as the
//! [`Engine`] trait's contract says. Nothing here reads inside a solid.
//!
//! Depends on: `construct-model` (the trait it implements) and `engine-geometry` (the engine seam,
//! the grid and the kernel's names), nothing else of the kernel's. The program picks the engine
//! and hands it to [`Kernel::new`]; the server does it in its own workspace.
//!
//! Interim (each sits here, so replacing it changes this file and no caller):
//!
//! - The engine behind [`Engine`] is not yet an exact engine of the kernel's own: the program's
//!   choice, and the interim register beside that engine's named faults, say which parts answer.
//! - A body's bytes are the engine's request (profile and vector in grid steps, and each opening's
//!   cutter by its door's key), not the body: the trait has no record of a body yet (chapter 24.4).
//! - A shape's names are written from its recipe and its doors, not read from the body, since the
//!   trait reports no face of a result; the pieces a door to the floor cuts the base into share
//!   the base's one name, where chapter 20.2 names each piece `Split` (CON-A3-F1-P2 with the body
//!   record).
//! - A face within the engine's tolerance of another is refused, not built (named fault SC-4 of
//!   geom-solidcore, and the interim register's row for it), until lengths and meshes are on the
//!   kernel's grid and the engine is the kernel's exact one.

mod family;
mod floor;
mod layered;
pub mod opening;
mod roof;
mod room;

pub use opening::Hosted;

use crate::families::{self, FamilySource};
use crate::wall_plan::{self, Layered};
use construct_families::CacheKey;

use construct_model::element::Placement;
use construct_model::geometry::{
    Bounds, Built, Display, Edition, EngineAnswer, EngineReport, EngineTolerance, FaceName,
    Geometry, GeometryError, Hosted as HostedAt, Mesh, Recipe,
};
use engine_geometry::value::engine::{
    Cut as EngineCut, DisplayMesh, Edition as EngineEdition, Engine, EngineError, GridPlane,
    Profile,
};
use engine_geometry::value::grid::{GridStep, P3};
use engine_geometry::value::name::{Role, TopoName};
use engine_geometry::value::op::{OpId, OpPath};
use kit_ops::sha256_hex;
use opening::Cut;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The grid Spare Construct builds on: `Q0`, 156.25 nm, the building grid (chapter 6).
pub const STEP: GridStep = GridStep::Q0;

/// This builder's name in an [`Edition`].
pub const ENGINE: &str = "construct.kernel";

/// This builder's version: what it asks of the engine for each recipe. A change to that is a new
/// version, as a change to what the stand-in builds is.
const VERSION: u32 = 1;

/// The kernel's `NameScheme` the face names are made under (chapter 20.1).
pub const NAME_SCHEME: u32 = 1;

/// One millimetre in quarter-nanometres, the unit [`GridStep::steps_per`] takes.
const MM_QUARTER_NM: u64 = 4_000_000;

/// The largest whole number an f64 holds exactly, and beyond every grid coordinate (2^40 steps).
const EXACT_F64: f64 = 9_007_199_254_740_992.0;

/// A wall's and a door's six faces, in the kernel's role words (chapter 20.2).
pub const WALL_ROLES: [&str; 6] = ["base", "top", "exterior", "interior", "end0", "end1"];

/// A block's six faces: an extrusion's two caps and its four sides, each side named for the edge
/// of the plan that sweeps it, counting from the plan's corner at its origin (chapter 20.2).
pub const BLOCK_ROLES: [&str; 6] = [
    "cap.start",
    "cap.end",
    "side.0",
    "side.1",
    "side.2",
    "side.3",
];

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn too_big() -> GeometryError {
    fault("TooBig", "a length is beyond the kernel's grid")
}

/// The engine's reason, kept, under a code of its own for each kind of answer.
fn from_engine(error: EngineError) -> GeometryError {
    match error {
        EngineError::Invalid { operation, what } => {
            fault("EngineInvalid", format!("{operation}: {what}"))
        }
        EngineError::Refused { operation, reason } => {
            fault("EngineRefused", format!("{operation}: {reason}"))
        }
        EngineError::Unsupported { operation, what } => {
            fault("EngineUnsupported", format!("{operation}: {what}"))
        }
    }
}

/// Whole millimetres and the engine's grid steps: the one place Spare Construct's lengths meet the
/// kernel's. Both ways are exact: a length that is not a whole number of the other unit is an
/// error, never rounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Units {
    per_mm: i64,
}

impl Units {
    /// The units of the grid step `step`.
    ///
    /// # Errors
    ///
    /// `UnitsUnavailable` when a millimetre is not a whole number of `step`'s steps.
    pub fn on(step: GridStep) -> Result<Units, GeometryError> {
        step.steps_per(MM_QUARTER_NM)
            .and_then(|per| i64::try_from(per).ok())
            .map(|per_mm| Units { per_mm })
            .ok_or_else(|| {
                fault(
                    "UnitsUnavailable",
                    "a millimetre is not a whole number of this engine's grid steps",
                )
            })
    }

    /// How many steps make one millimetre.
    #[must_use]
    pub fn steps_per_mm(self) -> u64 {
        self.per_mm.unsigned_abs()
    }

    /// `mm` whole millimetres in steps.
    ///
    /// # Errors
    ///
    /// `TooBig` when that is beyond a 64-bit integer.
    pub fn steps(self, mm: i64) -> Result<i64, GeometryError> {
        mm.checked_mul(self.per_mm).ok_or_else(too_big)
    }

    /// `steps` in whole millimetres.
    ///
    /// # Errors
    ///
    /// `NotWholeMillimetres` when `steps` is not a whole number of millimetres.
    pub fn millimetres(self, steps: i64) -> Result<i64, GeometryError> {
        match steps.checked_rem(self.per_mm) {
            Some(0) => steps.checked_div(self.per_mm),
            _ => None,
        }
        .ok_or_else(|| {
            fault(
                "NotWholeMillimetres",
                format!("{steps} steps is not a whole number of millimetres"),
            )
        })
    }

    /// The grid point at `x`, `y` and `z` whole millimetres.
    ///
    /// # Errors
    ///
    /// `TooBig` when it is beyond the kernel's grid (2^40 − 1 steps from the origin).
    pub fn point(self, [x, y, z]: [i64; 3]) -> Result<P3, GeometryError> {
        P3::new(self.steps(x)?, self.steps(y)?, self.steps(z)?).map_err(|_| too_big())
    }

    /// Where `hosted` sits in the frame of `host`, the element it is hosted in: the difference of
    /// their placements, as a grid point.
    ///
    /// # Errors
    ///
    /// `TooBig` when it is beyond the kernel's grid.
    pub fn offset(self, host: &Placement, hosted: &Placement) -> Result<P3, GeometryError> {
        let less = |a: i64, b: i64| a.checked_sub(b).ok_or_else(too_big);
        self.point([
            less(hosted.x, host.x)?,
            less(hosted.y, host.y)?,
            less(hosted.z, host.z)?,
        ])
    }

    /// `steps`, at or above zero, in millimetres rounded up: a bound in whole millimetres.
    fn millimetres_up(self, steps: i64) -> i64 {
        let whole = steps.checked_div(self.per_mm).unwrap_or(0);
        match steps.checked_rem(self.per_mm) {
            Some(0) | None => whole,
            Some(_) => whole.saturating_add(1),
        }
    }

    /// The units of a grid with `steps_per_mm` steps to the millimetre, as an edition records it.
    ///
    /// # Errors
    ///
    /// `UnitsUnavailable` when that is no step count a millimetre can have.
    fn of_steps_per_mm(steps_per_mm: u64) -> Result<Units, GeometryError> {
        i64::try_from(steps_per_mm)
            .ok()
            .filter(|&per_mm| per_mm > 0)
            .map(|per_mm| Units { per_mm })
            .ok_or_else(|| {
                fault(
                    "UnitsUnavailable",
                    "a millimetre is not a whole number of this engine's grid steps",
                )
            })
    }

    /// How near two faces may come without being one, in steps: the model's tolerance (0.01 mm,
    /// main's ruling of 22 Nov 2026) or the engine's largest length tolerance, whichever is the
    /// larger, rounded up to a whole step.
    fn band(self, report: &EngineEdition) -> i64 {
        self.band_over(report.tolerances.iter().map(|t| (t.unit.as_str(), t.value)))
    }

    /// [`Units::band`] over tolerances given as their unit and value.
    fn band_over<'a>(self, tolerances: impl Iterator<Item = (&'a str, f64)>) -> i64 {
        // A hundredth of a millimetre, rounded up to a whole step.
        let model = self.per_mm.saturating_add(99).checked_div(100).unwrap_or(1);
        tolerances
            .filter(|&(unit, value)| unit == "m" && value.is_finite() && value > 0.0)
            .map(|(_, value)| {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "steps per millimetre, far under 2^53, which an f64 holds exactly"
                )]
                let steps = (value * 1000.0 * self.per_mm as f64).ceil();
                if steps < EXACT_F64 {
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "a whole number under 2^53, checked above, which an i64 holds"
                    )]
                    let steps = steps as i64;
                    steps
                } else {
                    i64::MAX
                }
            })
            .fold(model, i64::max)
    }

    /// A mesh coordinate, in steps as the engine gives it, in whole millimetres.
    fn coordinate(self, steps: f64) -> Result<i64, GeometryError> {
        self.millimetres(Units::whole_steps(steps)?)
    }

    /// A mesh coordinate as the engine gives it, a whole number of steps held in an f64, as an
    /// integer.
    ///
    /// # Errors
    ///
    /// `NotOnTheGrid` for a coordinate that is not a whole number of steps.
    fn whole_steps(steps: f64) -> Result<i64, GeometryError> {
        if steps.fract() != 0.0 || steps.abs() >= EXACT_F64 {
            return Err(fault(
                "NotOnTheGrid",
                format!("a mesh coordinate of {steps} is not a whole number of steps"),
            ));
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a whole number under 2^53, checked above, which an i64 holds exactly"
        )]
        let steps = steps as i64;
        Ok(steps)
    }

    /// A length Spare Construct stores (construct-params' `Length`: steps of the building grid,
    /// [`STEP`], 6,400 to the millimetre) in this engine's steps, exactly: unchanged on `Q0`.
    ///
    /// # Errors
    ///
    /// `NotOnTheGrid` when it is not a whole number of this engine's steps; `TooBig` beyond a
    /// 64-bit integer.
    pub fn from_q0(self, q0: i64) -> Result<i64, GeometryError> {
        let per = i128::from(construct_params::Length::STEPS_PER_MM);
        let product = i128::from(q0)
            .checked_mul(i128::from(self.per_mm))
            .ok_or_else(too_big)?;
        match (product.checked_rem(per), product.checked_div(per)) {
            (Some(0), Some(steps)) => i64::try_from(steps).map_err(|_| too_big()),
            _ => Err(fault(
                "NotOnTheGrid",
                format!(
                    "a length of {q0} building-grid steps is not a whole number of this engine's steps"
                ),
            )),
        }
    }

    /// `steps` to the nearest whole millimetre, a half away from zero, and whether that is
    /// exact: how a display in whole millimetres holds a point that is not on one.
    #[must_use]
    pub fn nearest_mm(self, steps: i64) -> (i64, bool) {
        let (floor, rest) = self.split(steps);
        // 0 <= rest < per_mm, so twice it is well within an i64.
        let twice = rest.saturating_mul(2);
        let up = twice > self.per_mm || (twice == self.per_mm && floor >= 0);
        (if up { floor.saturating_add(1) } else { floor }, rest == 0)
    }

    /// `steps` in whole millimetres, rounded down: a bound's low corner.
    #[must_use]
    pub fn floor_mm(self, steps: i64) -> i64 {
        self.split(steps).0
    }

    /// `steps` in whole millimetres, rounded up: a bound's high corner.
    #[must_use]
    pub fn ceil_mm(self, steps: i64) -> i64 {
        match self.split(steps) {
            (floor, 0) => floor,
            (floor, _) => floor.saturating_add(1),
        }
    }

    /// `steps` as whole millimetres rounded down and the steps left over, at or above zero.
    fn split(self, steps: i64) -> (i64, i64) {
        // per_mm is above zero (`Units::on`, `Units::of_steps_per_mm`), so neither is `None`.
        (
            steps.checked_div_euclid(self.per_mm).unwrap_or(0),
            steps.checked_rem_euclid(self.per_mm).unwrap_or(0),
        )
    }
}

/// What a recipe is made of here: a plan rectangle at the floor extruded straight up, and the role
/// of each face it makes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Extrusion {
    /// The plan's far corner, in millimetres: the rectangle is from the origin to here.
    size: [i64; 2],
    /// How far it goes up, in millimetres.
    height: i64,
    /// The roles of the floor's face and the top's.
    caps: [&'static str; 2],
    /// The role of the side each edge of the plan sweeps, edge `i` from corner `i` to `i + 1`.
    sides: [&'static str; 4],
    /// The plan's axis a linear element runs along: 0 for x, 1 for y.
    along: usize,
}

fn whole(args: &BTreeMap<String, Value>, name: &str) -> Result<i64, GeometryError> {
    let value = args.get(name).and_then(Value::as_i64);
    let value = value.ok_or_else(|| {
        fault(
            "MissingArgument",
            format!("{name} must be a whole number of millimetres"),
        )
    })?;
    if value <= 0 {
        return Err(fault("EmptyShape", format!("{name} must be above zero")));
    }
    Ok(value)
}

impl Extrusion {
    /// What `recipe` makes.
    fn of(recipe: &Recipe) -> Result<Extrusion, GeometryError> {
        let args = &recipe.args;
        match recipe.op.as_str() {
            "wall.straight" => Extrusion::linear(args, "length", "thickness"),
            "door.single" => Extrusion::door(args),
            "block" => Ok(Extrusion {
                size: [whole(args, "width")?, whole(args, "depth")?],
                height: whole(args, "height")?,
                caps: ["cap.start", "cap.end"],
                sides: ["side.0", "side.1", "side.2", "side.3"],
                along: 0,
            }),
            other => Err(fault(
                "UnknownOperation",
                format!("this edition can't build {other}"),
            )),
        }
    }

    /// A door: a box in its wall's thickness with a wall's roles, a simple solid placed in its
    /// opening until families give it its real shape. The opening it cuts from its host is its
    /// width and height through the host's thickness ([`opening`]).
    fn door(args: &BTreeMap<String, Value>) -> Result<Extrusion, GeometryError> {
        Extrusion::linear(args, "width", "thickness")
    }

    /// A linear element: `long` along its axis, `across` through its thickness. Walking it from its
    /// start to its end, the exterior is the side on the right, the interior on the left, `end0`
    /// where it starts and `end1` where it ends.
    fn linear(
        args: &BTreeMap<String, Value>,
        long: &str,
        across: &str,
    ) -> Result<Extrusion, GeometryError> {
        let (long, across, height) = (
            whole(args, long)?,
            whole(args, across)?,
            whole(args, "height")?,
        );
        let caps = ["base", "top"];
        Ok(if args.get("axis").and_then(Value::as_str) == Some("y") {
            // Walking along +y, the right hand is +x: the side at x = thickness.
            Extrusion {
                size: [across, long],
                height,
                caps,
                sides: ["end0", "exterior", "end1", "interior"],
                along: 1,
            }
        } else {
            // Walking along +x, the right hand is −y: the side at y = 0.
            Extrusion {
                size: [long, across],
                height,
                caps,
                sides: ["exterior", "end1", "interior", "end0"],
                along: 0,
            }
        })
    }

    /// Its length along its axis and its thickness across it, in millimetres.
    fn long_and_across(&self) -> (i64, i64) {
        let [a, b] = self.size;
        if self.along == 1 { (b, a) } else { (a, b) }
    }

    /// The plan's corners, in millimetres, counter-clockwise from above, from the origin.
    fn corners(&self) -> [[i64; 2]; 4] {
        let [a, b] = self.size;
        [[0, 0], [a, 0], [a, b], [0, b]]
    }

    /// Every role it gives a face.
    fn roles(&self) -> impl Iterator<Item = &'static str> {
        self.caps.into_iter().chain(self.sides)
    }
}

/// The kernel's operation id of a recipe's operation: the first 16 bytes of the SHA-256 of
/// `owner/op`, so every element built by one operation names its faces alike.
fn op_id(recipe: &Recipe) -> Result<OpId, GeometryError> {
    let hash = sha256_hex(format!("{}/{}", recipe.owner, recipe.op).as_bytes());
    let bytes = hash
        .get(..32)
        .and_then(|half| hex::decode(half).ok())
        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
        .ok_or_else(|| fault("Naming", "an operation's hash is not 32 bytes"))?;
    Ok(OpId::from_bytes(bytes))
}

/// The persistent name of the face `role` makes under `op`.
fn name(op: OpId, role: &str) -> Result<FaceName, GeometryError> {
    let naming = |e: &dyn std::fmt::Debug| fault("Naming", format!("{role}: {e:?}"));
    #[allow(
        clippy::default_trait_access,
        reason = "engine-geometry does not re-export the name list's type; no parents is its default"
    )]
    let created = TopoName::Created {
        op: OpPath::new(op),
        role: Role::new(role, std::iter::empty()).map_err(|e| naming(&e))?,
        from: Default::default(),
    };
    Ok(FaceName {
        name: created.id().map_err(|e| naming(&e))?.to_string(),
        role: role.to_owned(),
    })
}

/// A body's bytes: what the engine was asked, in grid steps. A shape with no openings writes no
/// `subtract`, so its bytes are as they were before openings were cut.
#[derive(Serialize)]
struct Asked {
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    subtract: Vec<AskedCut>,
}

/// One opening's cutter as the engine was asked for it, by its door's key.
#[derive(Serialize)]
struct AskedCut {
    key: String,
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
}

/// The grid points of `points`, as whole steps.
fn as_steps(points: &[P3]) -> Vec<[i64; 3]> {
    points
        .iter()
        .map(|p| [p.x.get(), p.y.get(), p.z.get()])
        .collect()
}

/// The engine's mesh in whole millimetres.
fn millimetres(mesh: &DisplayMesh, units: Units) -> Result<Mesh, GeometryError> {
    let mut positions = Vec::new();
    for point in &mesh.positions {
        for &c in point {
            positions.push(units.coordinate(c)?);
        }
    }
    let count = mesh.positions.len();
    let mut indices = Vec::new();
    for &index in mesh.triangles.iter().flatten() {
        if !usize::try_from(index).is_ok_and(|i| i < count) {
            return Err(fault("EngineRefused", "mesh: a triangle past its points"));
        }
        indices.push(index);
    }
    Ok(Mesh { positions, indices })
}

/// The box around a mesh's points.
fn bounds(mesh: &Mesh) -> Result<Bounds, GeometryError> {
    let mut points = mesh.positions.as_chunks::<3>().0.iter();
    let first = *points
        .next()
        .ok_or_else(|| fault("EmptyShape", "the engine's mesh has no points"))?;
    let (mut min, mut max) = (first, first);
    for p in points {
        for ((lo, hi), &c) in min.iter_mut().zip(max.iter_mut()).zip(p) {
            *lo = (*lo).min(c);
            *hi = (*hi).max(c);
        }
    }
    Ok(Bounds { min, max })
}

/// Construct's record of the engine's own report.
fn edition_of(report: &EngineEdition, units: Units) -> Edition {
    Edition {
        engine: ENGINE.to_owned(),
        version: VERSION,
        name_scheme: NAME_SCHEME,
        report: Some(EngineReport {
            engine: report.engine.clone(),
            version: report.version.clone(),
            steps_per_mm: units.steps_per_mm(),
            tolerances: report
                .tolerances
                .iter()
                .map(|t| EngineTolerance {
                    name: t.name.clone(),
                    value: t.value.to_string(),
                    unit: t.unit.clone(),
                })
                .collect(),
            answers: report
                .answers
                .iter()
                .map(|a| EngineAnswer {
                    operation: a.operation.clone(),
                    case: a.case.clone(),
                    by: a.by.clone(),
                })
                .collect(),
        }),
    }
}

/// The persistent names of the faces `recipe` builds, without building it, sorted by name as a
/// built shape lists them: each face's role name ([`WALL_ROLES`], [`BLOCK_ROLES`]). An opening's
/// faces are not among them: they are the wall's with its doors.
///
/// # Errors
///
/// As [`Geometry::build`] for a recipe it cannot build.
pub fn names(recipe: &Recipe) -> Result<Vec<FaceName>, GeometryError> {
    if construct_openings::is_void(recipe) || crate::flat::is_flat(recipe) {
        return Ok(Vec::new());
    }
    if families::is_instance(recipe) {
        // A placed family hosts nothing, so none of its faces is named for a hosted element yet.
        families::Instance::of(recipe)?;
        return Ok(Vec::new());
    }
    if crate::floor_plan::is_floor(recipe) {
        let plan = crate::floor_plan::FloorPlan::of(recipe)?;
        return floor::names(op_id(recipe)?, &plan);
    }
    if crate::roof_plan::is_roof(recipe) {
        let plan = crate::roof_plan::RoofPlan::of(recipe)?;
        return roof::names(op_id(recipe)?, &plan);
    }
    if crate::room_plan::is_room(recipe) {
        let plan = crate::room_plan::RoomPlan::of(recipe)?;
        return room::names(op_id(recipe)?, &plan);
    }
    if wall_plan::is_layered(recipe) {
        return layered::names(recipe, &Layered::of(recipe)?);
    }
    let shape = Extrusion::of(recipe)?;
    let op = op_id(recipe)?;
    let mut names = shape
        .roles()
        .map(|role| name(op, role))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(names)
}

/// The hosted elements on the grid of `units`, as the openings take them.
fn on_grid<'a>(units: Units, hosted: &[HostedAt<'a>]) -> Result<Vec<Hosted<'a>>, GeometryError> {
    hosted
        .iter()
        .map(|h| {
            Ok(Hosted {
                key: h.key,
                recipe: h.recipe,
                at: units.point([h.at.x, h.at.y, h.at.z])?,
            })
        })
        .collect()
}

/// Whether the elements `hosted` fit in `host` by this builder's rules for openings ([`opening`]),
/// asked without the engine: a door this builder would refuse is refused here in its words, the
/// same on every machine. A command asks it before it hosts a door, so a door that does not fit is
/// turned back at once.
///
/// The units and the band are those `edition` records: its engine's grid step and tolerances. An
/// edition with no engine behind it, the stand-in's, is asked on the kernel's grid ([`STEP`]) and
/// the model's tolerance alone, so a model drawn on the stand-in holds no door this builder would
/// refuse when the model moves to it.
///
/// # Errors
///
/// [`opening`]'s refusals, each naming the door and why; `UnitsUnavailable` for an edition whose
/// grid step is no step count; and [`Geometry::build`]'s errors for a host it cannot build.
pub fn fits(
    edition: &Edition,
    host: &Recipe,
    hosted: &[HostedAt<'_>],
) -> Result<(), GeometryError> {
    fits_with(edition, host, hosted, &*families::baked())
}

/// As [`fits`], a placed family's recipe found in `families`.
///
/// # Errors
///
/// As [`fits`]; and for a placed family, [`crate::families::opening_of`]'s refusals with its key
/// in front.
pub fn fits_with(
    edition: &Edition,
    host: &Recipe,
    hosted: &[HostedAt<'_>],
    families: &dyn FamilySource,
) -> Result<(), GeometryError> {
    let (units, band) = if let Some(report) = &edition.report {
        let units = Units::of_steps_per_mm(report.steps_per_mm)?;
        let tolerances = report
            .tolerances
            .iter()
            .filter_map(|t| t.value.parse::<f64>().ok().map(|v| (t.unit.as_str(), v)));
        (units, units.band_over(tolerances))
    } else {
        let units = Units::on(STEP)?;
        (units, units.band_over(std::iter::empty()))
    };
    if crate::floor_plan::is_floor(host) {
        return floor::fits(host, &on_grid(units, hosted)?, units);
    }
    if wall_plan::is_layered(host) {
        return layered::fits(
            &Layered::of(host)?,
            &on_grid(units, hosted)?,
            units,
            band,
            families,
        );
    }
    let shape = Extrusion::of(host)?;
    opening::plan(host, &shape, &on_grid(units, hosted)?, units, band).map(|_| ())
}

/// The builder over an engine: every shape is made by `E` through the kernel's engine seam.
pub struct Kernel<E> {
    engine: Mutex<E>,
    /// Where a placed family's recipe comes from.
    families: Arc<dyn FamilySource>,
    /// What each placed family's type built to, turned as it stands, by the evaluator's key, and
    /// how many were built (not found kept).
    family_shapes: Mutex<(BTreeMap<CacheKey, Built>, u64)>,
    units: Units,
    edition: Edition,
    /// How near two faces may come without being one, in steps ([`Units::band`]).
    band: i64,
}

impl<E: Engine> Kernel<E> {
    /// A builder over `engine`, on the engine's own grid step, placed families' recipes found in
    /// the library baked in at build time ([`families::baked`]).
    ///
    /// # Errors
    ///
    /// `UnitsUnavailable` when a millimetre is not a whole number of the engine's steps.
    pub fn new(engine: E) -> Result<Kernel<E>, GeometryError> {
        Kernel::with_families(engine, families::baked())
    }

    /// A builder over `engine`, placed families' recipes found in `families`.
    ///
    /// # Errors
    ///
    /// As [`Kernel::new`].
    pub fn with_families(
        engine: E,
        families: Arc<dyn FamilySource>,
    ) -> Result<Kernel<E>, GeometryError> {
        let report = engine.edition();
        let units = Units::on(report.step)?;
        Ok(Kernel {
            edition: edition_of(&report, units),
            band: units.band(&report),
            units,
            engine: Mutex::new(engine),
            families,
            family_shapes: Mutex::new((BTreeMap::new(), 0)),
        })
    }

    /// How many placed families' types it has built (not found kept): each type, turned as it
    /// stands, once.
    ///
    /// # Errors
    ///
    /// `EngineUnavailable` when an earlier build stopped part way.
    pub fn family_builds(&self) -> Result<u64, GeometryError> {
        self.family_shapes
            .lock()
            .map(|kept| kept.1)
            .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))
    }

    /// A placed family ([`family::build`]), as kept if its type was built before turned the same
    /// way; it hosts nothing.
    fn build_family(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        if let Some(first) = hosted.first() {
            return Err(fault(
                "NotAHost",
                format!("{}: a placed family hosts nothing", first.key),
            ));
        }
        let unavailable = || fault("EngineUnavailable", "an earlier build stopped part way");
        let instance = families::Instance::of(host)?;
        let resolved = families::resolve(&*self.families, &instance)?;
        let key = CacheKey::turned(&resolved.recipe, &resolved.values, instance.turn);
        let mut kept = self.family_shapes.lock().map_err(|_| unavailable())?;
        if let Some(built) = kept.0.get(&key) {
            return Ok(built.clone());
        }
        let built = {
            let mut engine = self.engine.lock().map_err(|_| unavailable())?;
            family::build(&mut *engine, &instance, &resolved, self.units)?
        };
        kept.0.insert(key, built.clone());
        kept.1 = kept.1.saturating_add(1);
        Ok(built)
    }

    /// Its units: whole millimetres to the engine's steps and back.
    #[must_use]
    pub fn units(&self) -> Units {
        self.units
    }

    /// How near two faces may come without being one, in the engine's grid steps: the larger of
    /// the model's tolerance (0.01 mm) and the engine's largest length tolerance. An opening with a
    /// face this near another face, and not on it, is refused (`OpeningNearFace`).
    #[must_use]
    pub fn band(&self) -> i64 {
        self.band
    }

    /// A floor, a roof or a placed family, each built by its own builder; `None` for any other
    /// recipe, which [`Kernel::build_hosting`] builds as a wall or a box.
    fn build_other_kind(
        &self,
        host: &Recipe,
        hosted: &[Hosted<'_>],
    ) -> Option<Result<Built, GeometryError>> {
        if crate::floor_plan::is_floor(host) {
            return Some(self.build_floor(host, hosted));
        }
        if crate::roof_plan::is_roof(host) {
            return Some(self.build_roof(host, hosted.len()));
        }
        if families::is_instance(host) {
            return Some(self.build_family(host, hosted));
        }
        None
    }

    /// Builds `host` with an opening cut through it for each element in `hosted`: the host's
    /// extrusion less each opening, through [`Engine::subtract`], in the order of the hosted
    /// elements' keys. With nothing hosted it is [`Geometry::build`].
    ///
    /// # Errors
    ///
    /// The errors of [`Geometry::build`]; for an opening, [`opening`]'s refusals, each naming the
    /// hosted element and why; and the engine's own, with the hosted element's key in front.
    pub fn build_hosting(
        &self,
        host: &Recipe,
        hosted: &[Hosted<'_>],
    ) -> Result<Built, GeometryError> {
        if let Some(built) = self.build_other_kind(host, hosted) {
            return built;
        }
        if crate::room_plan::is_room(host) {
            return self.build_room(host, hosted.len());
        }
        if wall_plan::is_layered(host) {
            let wall = Layered::of(host)?;
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let (units, band) = (self.units, self.band);
            let families = &*self.families;
            return layered::build(&mut *engine, host, &wall, hosted, units, band, families);
        }
        let shape = Extrusion::of(host)?;
        let units = self.units;
        let op = op_id(host)?;
        let cuts = opening::plan(host, &shape, hosted, units, self.band)?;
        let corners = shape.corners();
        let outer = corners
            .iter()
            .map(|&[x, y]| units.point([x, y, 0]))
            .collect::<Result<Vec<P3>, _>>()?;
        let by = [0, 0, units.steps(shape.height)?];
        let mut asked = Asked {
            extrude: as_steps(&outer),
            by,
            subtract: Vec::new(),
        };
        let profile = Profile {
            outer,
            holes: Vec::new(),
        };
        let tools = cuts
            .iter()
            .map(Cut::profile)
            .collect::<Result<Vec<_>, _>>()?;
        for (cut, (tool, by)) in cuts.iter().zip(&tools) {
            asked.subtract.push(AskedCut {
                key: cut.key().to_owned(),
                extrude: as_steps(&tool.outer),
                by: *by,
            });
        }
        let (coarse, fine) = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let mut solid = engine.extrude(&profile, by).map_err(from_engine)?;
            let coarse = engine.mesh(&solid).map_err(from_engine)?;
            for (cut, (tool, by)) in cuts.iter().zip(&tools) {
                let named = |e: EngineError| cut.engine_error(from_engine(e));
                let tool = engine.extrude(tool, *by).map_err(named)?;
                solid = engine.subtract(&solid, &tool).map_err(named)?;
            }
            let fine = if cuts.is_empty() {
                None
            } else {
                Some(engine.mesh(&solid).map_err(from_engine)?)
            };
            (coarse, fine)
        };
        let coarse = millimetres(&coarse, units)?;
        let fine = fine.map_or_else(|| Ok(coarse.clone()), |m| millimetres(&m, units))?;
        let bounds = bounds(&fine)?;
        let mut names = shape
            .roles()
            .map(|role| name(op, role))
            .collect::<Result<Vec<_>, _>>()?;
        for cut in &cuts {
            names.extend(cut.names(op)?);
        }
        names.sort_by(|a, b| a.name.cmp(&b.name));
        let outline = corners.to_vec();
        // Level 0 is the body itself; level 1 the extrusion before its openings were cut, which
        // strays from the body by at most the openings' bound. A shape with no openings has the
        // same mesh at both levels; their records still stand apart by their level.
        let displays = vec![
            Display {
                lod: 0,
                deviation: 0,
                mesh: fine,
                outline: outline.clone(),
                parts: Vec::new(),
            },
            Display {
                lod: 1,
                deviation: opening::deviation(&cuts, units),
                mesh: coarse,
                outline,
                parts: Vec::new(),
            },
        ];
        Ok(Built {
            body: serde_json::to_vec(&asked).unwrap_or_default(),
            bounds,
            names,
            displays,
        })
    }

    /// The engine's solid of `recipe`, made as [`Geometry::build`] makes it, with the plane of each
    /// named face, for a drawing to cut ([`Kernel::section`]).
    ///
    /// Each face's plane is its carrier as [`Engine::section`] names a cut edge's face: in grid
    /// steps, its normal pointing out of the solid, its coefficients with no common factor. A
    /// side's plane stands on the plan's edge that sweeps it; the caps' are the floor and the top.
    ///
    /// # Errors
    ///
    /// What [`Geometry::build`] gives for a recipe it cannot build, and the engine's errors.
    pub fn solid(&self, recipe: &Recipe) -> Result<KernelSolid<E::Solid>, GeometryError> {
        let shape = Extrusion::of(recipe)?;
        let units = self.units;
        let corners = shape.corners();
        let outer = corners
            .iter()
            .map(|&[x, y]| units.point([x, y, 0]))
            .collect::<Result<Vec<P3>, _>>()?;
        let height = units.steps(shape.height)?;
        let profile = Profile {
            outer,
            holes: Vec::new(),
        };
        let solid = self
            .engine
            .lock()
            .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?
            .extrude(&profile, [0, 0, height])
            .map_err(from_engine)?;
        let op = op_id(recipe)?;
        let mut faces = Vec::new();
        for (role, plane) in [
            (shape.caps[0], carrier([0, 0, -1], 0)),
            (shape.caps[1], carrier([0, 0, 1], i128::from(height))),
        ] {
            faces.push(NamedFace {
                plane: plane?,
                name: name(op, role)?,
            });
        }
        for (i, role) in shape.sides.into_iter().enumerate() {
            let start = corners.get(i).copied().ok_or_else(too_big)?;
            let next = i
                .checked_add(1)
                .and_then(|n| n.checked_rem(corners.len()))
                .ok_or_else(too_big)?;
            let end = corners.get(next).copied().ok_or_else(too_big)?;
            let [x0, y0] = start.map(|c| units.steps(c));
            let [x1, y1] = end.map(|c| units.steps(c));
            let (x0, y0, x1, y1) = (x0?, y0?, x1?, y1?);
            // The plan turns counter-clockwise, so the outside is on the right of each edge.
            let less = |a: i64, b: i64| i128::from(a).checked_sub(i128::from(b));
            let normal = [
                less(y1, y0).ok_or_else(too_big)?,
                less(x0, x1).ok_or_else(too_big)?,
                0,
            ];
            let offset = normal[0]
                .checked_mul(i128::from(x0))
                .and_then(|a| {
                    normal[1]
                        .checked_mul(i128::from(y0))
                        .and_then(|b| a.checked_add(b))
                })
                .ok_or_else(too_big)?;
            faces.push(NamedFace {
                plane: carrier(normal, offset)?,
                name: name(op, role)?,
            });
        }
        Ok(KernelSolid { solid, faces })
    }

    /// The solids of `recipe` for a drawing to cut: one for each layer of a layered wall (a layer
    /// of no thickness has none), each with its material and the plane of each named face, and
    /// for any other recipe the one [`Kernel::solid`] makes, with no material. A wall's openings
    /// are not cut in them.
    ///
    /// # Errors
    ///
    /// As [`Kernel::solid`], and the plan's refusals for a layered wall.
    pub fn layers(&self, recipe: &Recipe) -> Result<Vec<LayerSolid<E::Solid>>, GeometryError> {
        if wall_plan::is_layered(recipe) {
            let wall = Layered::of(recipe)?;
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            return layered::solids(&mut *engine, recipe, &wall, self.units);
        }
        Ok(vec![LayerSolid {
            function: String::new(),
            material: None,
            solid: self.solid(recipe)?,
        }])
    }

    /// Where `plane` cuts `solid`, by the engine ([`Engine::section`]).
    ///
    /// # Errors
    ///
    /// The engine's errors, under codes of their own (`EngineInvalid`, `EngineRefused`,
    /// `EngineUnsupported`).
    pub fn section(&self, solid: &E::Solid, plane: &GridPlane) -> Result<EngineCut, GeometryError> {
        self.engine
            .lock()
            .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?
            .section(solid, plane)
            .map_err(from_engine)
    }
}

/// One solid of an element for a drawing to cut ([`Kernel::layers`]): a layer of a layered wall,
/// or a whole element that has no layers.
#[derive(Debug)]
pub struct LayerSolid<S> {
    /// What the layer does (`structure`, `finish`, ...); empty for an element with no layers.
    pub function: String,
    /// What it is made of, by material ID; `None` for an element with no layers.
    pub material: Option<String>,
    /// Its solid and the plane of each named face.
    pub solid: KernelSolid<S>,
}

/// A recipe's solid as the engine made it ([`Kernel::solid`]), and its named faces' planes.
#[derive(Debug)]
pub struct KernelSolid<S> {
    /// The engine's solid.
    pub solid: S,
    /// Each named face and the plane it lies on.
    pub faces: Vec<NamedFace>,
}

/// A face's persistent name and the plane it lies on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedFace {
    /// The face's plane, as [`Engine::section`] names a cut edge's face.
    pub plane: GridPlane,
    /// Its persistent name and role.
    pub name: FaceName,
}

/// The plane `normal · p = offset` with its coefficients' common factor taken out, its normal's
/// way kept.
fn carrier(normal: [i128; 3], offset: i128) -> Result<GridPlane, GeometryError> {
    let mut g = offset.unsigned_abs();
    for c in normal {
        let mut b = c.unsigned_abs();
        while b != 0 {
            (g, b) = (b, g.checked_rem(b).unwrap_or(0));
        }
    }
    let g = i128::try_from(g)
        .ok()
        .filter(|g| *g > 0)
        .ok_or_else(|| fault("EmptyShape", "a face with no plane"))?;
    let part = |c: i128| c.checked_div(g).ok_or_else(too_big);
    Ok(GridPlane {
        normal: [part(normal[0])?, part(normal[1])?, part(normal[2])?],
        offset: part(offset)?,
    })
}

impl<E: Engine + Send + 'static> Geometry for Kernel<E> {
    fn edition(&self) -> Edition {
        self.edition.clone()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        if let Some(datum) = crate::datum::build(recipe) {
            return datum;
        }
        if let Some(void) = crate::voids::build(recipe) {
            return void;
        }
        if let Some(flat) = crate::flat::build(recipe) {
            return flat;
        }
        self.build_hosting(recipe, &[])
    }

    /// [`Kernel::build_hosting`], each hosted element where it is put, on this engine's grid.
    fn build_hosting(
        &self,
        host: &Recipe,
        hosted: &[HostedAt<'_>],
    ) -> Result<Built, GeometryError> {
        Kernel::build_hosting(self, host, &on_grid(self.units, hosted)?)
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "tests; the rules are for the crate's own code"
)]
mod tests {
    use super::*;
    use serde_json::json;

    fn recipe(op: &str, args: Value) -> Recipe {
        Recipe::construct(op, serde_json::from_value(args).unwrap())
    }

    /// Whole millimetres go to steps and back exactly, on every grid step; a number of steps that
    /// is not whole millimetres, or millimetres beyond the grid, is an error, never rounded.
    #[test]
    fn whole_millimetres_map_to_the_grid_both_ways() {
        let q0 = Units::on(GridStep::Q0).unwrap();
        assert_eq!(q0.steps_per_mm(), 6400);
        assert_eq!(
            q0.steps_per_mm(),
            u64::try_from(construct_params::Length::STEPS_PER_MM).unwrap()
        );
        assert_eq!(q0.steps(1).unwrap(), 6400);
        assert_eq!(q0.steps(-2700).unwrap(), -17_280_000);
        assert_eq!(q0.millimetres(17_280_000).unwrap(), 2700);
        assert_eq!(q0.millimetres(-6400).unwrap(), -1);
        for mm in [0, 1, 200, 2700, 4000, 171_798] {
            assert_eq!(q0.millimetres(q0.steps(mm).unwrap()).unwrap(), mm);
        }
        assert_eq!(
            q0.millimetres(6401).unwrap_err().code,
            "NotWholeMillimetres"
        );
        assert_eq!(q0.steps(i64::MAX).unwrap_err().code, "TooBig");
        assert_eq!(q0.point([200_000_000, 0, 0]).unwrap_err().code, "TooBig");
        assert_eq!(q0.coordinate(6400.0).unwrap(), 1);
        assert_eq!(q0.coordinate(6400.5).unwrap_err().code, "NotOnTheGrid");
        assert_eq!(q0.coordinate(f64::NAN).unwrap_err().code, "NotOnTheGrid");
        let q1 = Units::on(GridStep::Q1).unwrap();
        assert_eq!(q1.steps_per_mm(), 160_000);
        assert_eq!(q1.millimetres(q1.steps(3).unwrap()).unwrap(), 3);
        assert_eq!(Units::on(STEP).unwrap(), q0);
    }

    /// A stored length goes to the engine's steps exactly (unchanged on `Q0`, 25 times on `Q1`);
    /// a point not on a millimetre goes to its nearest, a half away from zero, saying it was
    /// rounded; a bound rounds outward.
    #[test]
    fn stored_lengths_reach_the_engine_and_come_back_to_the_nearest_millimetre() {
        let q0 = Units::on(GridStep::Q0).unwrap();
        let q1 = Units::on(GridStep::Q1).unwrap();
        assert_eq!(q0.from_q0(80_000).unwrap(), 80_000);
        assert_eq!(q1.from_q0(80_000).unwrap(), 2_000_000);
        assert_eq!(q1.from_q0(-1).unwrap(), -25);
        assert_eq!(q0.from_q0(i64::MAX).unwrap(), i64::MAX);
        assert_eq!(q1.from_q0(i64::MAX).unwrap_err().code, "TooBig");
        let coarse = Units::of_steps_per_mm(100).unwrap();
        assert_eq!(coarse.from_q0(64).unwrap(), 1);
        assert_eq!(coarse.from_q0(1).unwrap_err().code, "NotOnTheGrid");
        assert_eq!(q0.nearest_mm(368_000), (58, false));
        assert_eq!(q0.nearest_mm(-368_000), (-58, false));
        assert_eq!(q0.nearest_mm(288_000), (45, true));
        assert_eq!(q0.nearest_mm(3199), (0, false));
        assert_eq!(q0.nearest_mm(-3199), (0, false));
        assert_eq!(q0.nearest_mm(-3201), (-1, false));
        assert_eq!(q0.floor_mm(-368_000), -58);
        assert_eq!(q0.ceil_mm(-368_000), -57);
        assert_eq!(q0.floor_mm(368_000), 57);
        assert_eq!(q0.ceil_mm(368_000), 58);
        assert_eq!(q0.ceil_mm(6400), 1);
        assert_eq!(Units::whole_steps(6400.0).unwrap(), 6400);
        assert_eq!(Units::whole_steps(0.5).unwrap_err().code, "NotOnTheGrid");
    }

    /// A hosted element's place in its host is the difference of their placements, on the grid;
    /// a bound in millimetres rounds up; the band two faces may not come within is the model's
    /// 0.01 mm or the engine's largest length tolerance, whichever is larger, in whole steps.
    #[test]
    fn a_hosted_place_a_bound_and_the_band_are_on_the_grid() {
        let q0 = Units::on(GridStep::Q0).unwrap();
        let wall = Placement {
            x: 100,
            y: -55,
            z: 0,
        };
        let door = Placement {
            x: 1100,
            y: -150,
            z: 900,
        };
        assert_eq!(
            q0.offset(&wall, &door).unwrap(),
            P3::new(6_400_000, -608_000, 5_760_000).unwrap()
        );
        let far = Placement {
            x: i64::MIN,
            y: 0,
            z: 0,
        };
        assert_eq!(q0.offset(&door, &far).unwrap_err().code, "TooBig");
        assert_eq!(q0.millimetres_up(0), 0);
        assert_eq!(q0.millimetres_up(6400), 1);
        assert_eq!(q0.millimetres_up(6401), 2);
        assert_eq!(q0.millimetres_up(3200), 1);
        let report = |value: f64, unit: &str| EngineEdition {
            engine: String::new(),
            version: String::new(),
            step: GridStep::Q0,
            tolerances: vec![engine_geometry::value::engine::Tolerance {
                name: "t".to_owned(),
                value,
                unit: unit.to_owned(),
            }],
            answers: Vec::new(),
        };
        assert_eq!(q0.band(&report(0.0, "m")), 64);
        assert_eq!(q0.band(&report(1.0e-6, "m")), 64);
        assert_eq!(q0.band(&report(1.0, "degree")), 64);
        assert_eq!(q0.band(&report(0.25, "m")), 1_600_000);
        assert_eq!(q0.band(&report(f64::NAN, "m")), 64);
    }

    /// A wall's exterior is the side on the right walking it from start to end: y = 0 along x,
    /// x = thickness along y. Its ends are where it starts and where it ends.
    #[test]
    fn a_walls_exterior_is_on_its_right() {
        let along_x = json!({ "length": 4000, "thickness": 200, "height": 2700 });
        let shape = Extrusion::of(&recipe("wall.straight", along_x)).unwrap();
        let corners = shape.corners();
        let edge = |role: &str| {
            let i = shape.sides.iter().position(|r| *r == role).unwrap();
            (corners[i], corners[(i + 1) % 4])
        };
        assert_eq!(edge("exterior"), ([0, 0], [4000, 0]));
        assert_eq!(edge("interior"), ([4000, 200], [0, 200]));
        assert_eq!(edge("end0"), ([0, 200], [0, 0]));
        assert_eq!(edge("end1"), ([4000, 0], [4000, 200]));
        let along_y = json!({ "length": 4000, "thickness": 200, "height": 2700, "axis": "y" });
        let shape = Extrusion::of(&recipe("wall.straight", along_y)).unwrap();
        let corners = shape.corners();
        let edge = |role: &str| {
            let i = shape.sides.iter().position(|r| *r == role).unwrap();
            (corners[i], corners[(i + 1) % 4])
        };
        assert_eq!(edge("exterior"), ([200, 0], [200, 4000]));
        assert_eq!(edge("interior"), ([0, 4000], [0, 0]));
        assert_eq!(edge("end0"), ([0, 0], [200, 0]));
        assert_eq!(edge("end1"), ([200, 4000], [0, 4000]));
        assert_eq!(shape.caps, ["base", "top"]);
    }

    /// A recipe's faces are named without building it as building names them: one name per role,
    /// sorted by name, and none for a recipe it cannot build.
    #[test]
    fn a_recipes_faces_are_named_without_building_it() {
        let wall = recipe(
            "wall.straight",
            json!({ "length": 4000, "thickness": 200, "height": 2700 }),
        );
        let named = names(&wall).unwrap();
        let op = op_id(&wall).unwrap();
        let mut expected: Vec<FaceName> = WALL_ROLES.iter().map(|r| name(op, r).unwrap()).collect();
        expected.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(named, expected);
        let longer = recipe(
            "wall.straight",
            json!({ "length": 9000, "thickness": 300, "height": 3000, "axis": "y" }),
        );
        assert_eq!(names(&longer).unwrap(), named);
        assert_eq!(
            names(&recipe("stair.spiral", json!({}))).unwrap_err().code,
            "UnknownOperation"
        );
    }

    /// A door is asked to fit its wall by the builder's own rules without the engine: on an
    /// edition's recorded grid and tolerances, or on the kernel's grid and the model's tolerance
    /// for an edition with no engine. Each refusal is the builder's, naming the door.
    #[test]
    fn a_door_fits_its_wall_by_the_builders_own_rules() {
        let wall = recipe(
            "wall.straight",
            json!({ "length": 4000, "thickness": 200, "height": 2700 }),
        );
        let door = recipe(
            "door.single",
            json!({ "width": 900, "thickness": 200, "height": 2100 }),
        );
        let at = |x: i64, z: i64| Placement { x, y: 0, z };
        let one = |x: i64, z: i64| {
            [HostedAt {
                key: "D-1",
                recipe: &door,
                at: at(x, z),
            }]
        };
        let stand_in = Edition {
            engine: "construct.stand-in".to_owned(),
            version: 1,
            name_scheme: 0,
            report: None,
        };
        let engine = Edition {
            report: Some(EngineReport {
                engine: "an engine".to_owned(),
                version: "a pin".to_owned(),
                steps_per_mm: 6400,
                tolerances: vec![EngineTolerance {
                    name: "length".to_owned(),
                    value: "0.001".to_owned(),
                    unit: "m".to_owned(),
                }],
                answers: Vec::new(),
            }),
            ..stand_in.clone()
        };
        for edition in [&stand_in, &engine] {
            assert_eq!(fits(edition, &wall, &one(1000, 0)), Ok(()));
            assert_eq!(fits(edition, &wall, &[]), Ok(()));
            let past = fits(edition, &wall, &one(3500, 0)).unwrap_err();
            assert_eq!(past.code, "OpeningPastEnd");
            assert_eq!(past.message, "door D-1: it crosses the wall's end");
            let tall = fits(edition, &wall, &one(1000, 700)).unwrap_err();
            assert_eq!(tall.code, "OpeningTooTall");
        }
        // A millimetre from the end is inside the model's 0.01 mm but within this engine's 1 mm.
        assert_eq!(fits(&stand_in, &wall, &one(1, 0)), Ok(()));
        assert_eq!(
            fits(&engine, &wall, &one(1, 0)).unwrap_err().code,
            "OpeningNearFace"
        );
        let block = recipe(
            "block",
            json!({ "width": 4000, "depth": 200, "height": 2700 }),
        );
        assert_eq!(
            fits(&stand_in, &block, &one(1000, 0)).unwrap_err().code,
            "NotAHost"
        );
        let mut odd = engine;
        if let Some(report) = &mut odd.report {
            report.steps_per_mm = 0;
        }
        assert_eq!(
            fits(&odd, &wall, &one(1000, 0)).unwrap_err().code,
            "UnitsUnavailable"
        );
    }

    /// Every role an operation gives is one of its list, each once.
    #[test]
    fn each_operation_names_its_six_faces_once() {
        let mut wall: Vec<&str> = Extrusion::of(&recipe(
            "wall.straight",
            json!({ "length": 1, "thickness": 1, "height": 1 }),
        ))
        .unwrap()
        .roles()
        .collect();
        wall.sort_unstable();
        let mut expected = WALL_ROLES;
        expected.sort_unstable();
        assert_eq!(wall, expected);
        let block: Vec<&str> = Extrusion::of(&recipe(
            "block",
            json!({ "width": 1, "depth": 1, "height": 1 }),
        ))
        .unwrap()
        .roles()
        .collect();
        assert_eq!(block, BLOCK_ROLES);
    }
}
