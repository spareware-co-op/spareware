//! The builder over the geometry kernel's engine seam: Spare Construct's recipes made by whatever
//! engine answers `engine_geometry`'s [`Engine`] trait, without this crate naming one.
//!
//! What it does: [`Kernel`] implements Construct's [`Geometry`] for any [`Engine`], the composition
//! root of the geometry join (chapter 25's facade on one side, Construct's recipes on the other):
//!
//! - `wall.straight`: the wall's plan, a rectangle as long and as thick as the recipe says along
//!   its axis (x, or y when `axis` is `y`), extruded straight up by its height through
//!   [`Engine::extrude`]. Its bounds are the stand-in's box, so placement means the same.
//! - `block`: its plan rectangle, width by depth, extruded the same way.
//! - `door.single`: a box in the wall's thickness, extruded the same way: a simple solid placed in
//!   its opening, until families give doors their real shape. [`Extrusion::door`] is the one place
//!   a door's shape is decided.
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
//! - Whether a hosted door cuts its wall is the caller's to say: Construct's [`Geometry`] builds a
//!   recipe alone, so the regenerator does not yet hand a wall its doors (the hosting job,
//!   CON-A4-F3-P1).
//! - A face within the engine's tolerance of another is refused, not built (named fault SC-4 of
//!   geom-solidcore, and the interim register's row for it), until lengths and meshes are on the
//!   kernel's grid and the engine is the kernel's exact one.

pub mod opening;

pub use opening::Hosted;

use construct_model::element::Placement;
use construct_model::geometry::{
    Bounds, Built, Display, Edition, EngineAnswer, EngineReport, EngineTolerance, FaceName,
    Geometry, GeometryError, Mesh, Recipe,
};
use engine_geometry::value::engine::{
    DisplayMesh, Edition as EngineEdition, Engine, EngineError, Profile,
};
use engine_geometry::value::grid::{GridStep, P3};
use engine_geometry::value::name::{Role, TopoName};
use engine_geometry::value::op::{OpId, OpPath};
use kit_ops::sha256_hex;
use opening::Cut;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The grid Spare Construct builds on: `Q0`, 156.25 nm, the building grid (chapter 6).
pub const STEP: GridStep = GridStep::Q0;

/// This builder's name in an [`Edition`].
pub const ENGINE: &str = "construct.kernel";

/// This builder's version: what it asks of the engine for each recipe. A change to that is a new
/// version, as a change to what the stand-in builds is.
const VERSION: u32 = 1;

/// The kernel's `NameScheme` the face names are made under (chapter 20.1).
const NAME_SCHEME: u32 = 1;

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

    /// How near two faces may come without being one, in steps: the model's tolerance (0.01 mm,
    /// main's ruling of 22 Nov 2026) or the engine's largest length tolerance, whichever is the
    /// larger, rounded up to a whole step.
    fn band(self, report: &EngineEdition) -> i64 {
        // A hundredth of a millimetre, rounded up to a whole step.
        let model = self.per_mm.saturating_add(99).checked_div(100).unwrap_or(1);
        report
            .tolerances
            .iter()
            .filter(|t| t.unit == "m" && t.value.is_finite() && t.value > 0.0)
            .map(|t| {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "steps per millimetre, far under 2^53, which an f64 holds exactly"
                )]
                let steps = (t.value * 1000.0 * self.per_mm as f64).ceil();
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
        self.millimetres(steps)
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

/// The builder over an engine: every shape is made by `E` through the kernel's engine seam.
pub struct Kernel<E> {
    engine: Mutex<E>,
    units: Units,
    edition: Edition,
    /// How near two faces may come without being one, in steps ([`Units::band`]).
    band: i64,
}

impl<E: Engine> Kernel<E> {
    /// A builder over `engine`, on the engine's own grid step.
    ///
    /// # Errors
    ///
    /// `UnitsUnavailable` when a millimetre is not a whole number of the engine's steps.
    pub fn new(engine: E) -> Result<Kernel<E>, GeometryError> {
        let report = engine.edition();
        let units = Units::on(report.step)?;
        Ok(Kernel {
            edition: edition_of(&report, units),
            band: units.band(&report),
            units,
            engine: Mutex::new(engine),
        })
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
            },
            Display {
                lod: 1,
                deviation: opening::deviation(&cuts, units),
                mesh: coarse,
                outline,
            },
        ];
        Ok(Built {
            body: serde_json::to_vec(&asked).unwrap_or_default(),
            bounds,
            names,
            displays,
        })
    }
}

impl<E: Engine + Send + 'static> Geometry for Kernel<E> {
    fn edition(&self) -> Edition {
        self.edition.clone()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        self.build_hosting(recipe, &[])
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
