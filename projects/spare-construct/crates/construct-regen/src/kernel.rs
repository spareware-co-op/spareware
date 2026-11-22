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
//! - `door.single`: a box in the wall's thickness, extruded the same way. Its opening is not cut
//!   from the host wall here: that is the next job's (CON-A3-F1-P2, P6), in this crate's `mesh.rs`
//!   to come, and [`Extrusion::door`] is the one place a door's shape is decided.
//! - Every display mesh comes from [`Engine::mesh`], in whole millimetres.
//! - Every face carries a persistent name for its role (chapter 20.2): a wall's and a door's
//!   `exterior`, `interior`, `top`, `base`, `end0` and `end1`; a block's `cap.start`, `cap.end` and
//!   `side.0` to `side.3`. A name is the kernel's `NameScheme v1` id of the face's `Created` name:
//!   the step is the recipe's operation (`owner/op`), the role is the face's, and nothing of the
//!   recipe's lengths goes in, so a wall rebuilt or lengthened keeps every name, and a door hosted
//!   on its exterior keeps its face.
//! - Its [`Edition`] is `construct.kernel`, this builder's version, `NameScheme` 1, and the
//!   engine's own report ([`Engine::edition`]: its name, its version or pin, its grid step, its
//!   tolerances and which part answers each operation). A document built on another edition is
//!   not built here; the regenerator reports it (FORMAT.md, "A document").
//!
//! Units: [`Units`] is the one place whole millimetres become the engine's grid steps and come
//! back, exactly or not at all (chapter 6: on `Q0` one millimetre is 6,400 steps).
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
//! - A body's bytes are the engine's request (profile and vector in grid steps), not the body: the
//!   trait has no record of a body yet (chapter 24.4).
//! - Both levels of detail are the engine's one mesh, until the next job meshes per detail level
//!   (CON-A3-F1-P2, P6).

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
            }),
            other => Err(fault(
                "UnknownOperation",
                format!("this edition can't build {other}"),
            )),
        }
    }

    /// A door: today a box in its wall's thickness with a wall's roles. The opening it makes in its
    /// host is the next job's (CON-A3-F1-P2, P6).
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
            }
        } else {
            // Walking along +x, the right hand is −y: the side at y = 0.
            Extrusion {
                size: [long, across],
                height,
                caps,
                sides: ["exterior", "end1", "interior", "end0"],
            }
        })
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

/// A body's bytes: what the engine was asked, in grid steps.
#[derive(Serialize)]
struct Asked {
    extrude: Vec<[i64; 3]>,
    by: [i64; 3],
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
            units,
            engine: Mutex::new(engine),
        })
    }

    /// Its units: whole millimetres to the engine's steps and back.
    #[must_use]
    pub fn units(&self) -> Units {
        self.units
    }
}

impl<E: Engine + Send + 'static> Geometry for Kernel<E> {
    fn edition(&self) -> Edition {
        self.edition.clone()
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        let shape = Extrusion::of(recipe)?;
        let units = self.units;
        let corners = shape.corners();
        let outer = corners
            .iter()
            .map(|&[x, y]| units.point([x, y, 0]))
            .collect::<Result<Vec<P3>, _>>()?;
        let by = [0, 0, units.steps(shape.height)?];
        let asked = Asked {
            extrude: outer
                .iter()
                .map(|p| [p.x.get(), p.y.get(), p.z.get()])
                .collect(),
            by,
        };
        let profile = Profile {
            outer,
            holes: Vec::new(),
        };
        let mesh = {
            let mut engine = self
                .engine
                .lock()
                .map_err(|_| fault("EngineUnavailable", "an earlier build stopped part way"))?;
            let solid = engine.extrude(&profile, by).map_err(from_engine)?;
            engine.mesh(&solid).map_err(from_engine)?
        };
        let mesh = millimetres(&mesh, units)?;
        let bounds = bounds(&mesh)?;
        let op = op_id(recipe)?;
        let mut names = shape
            .roles()
            .map(|role| name(op, role))
            .collect::<Result<Vec<_>, _>>()?;
        names.sort_by(|a, b| a.name.cmp(&b.name));
        let outline = corners.to_vec();
        // One mesh for both levels until the next job meshes per detail level; their records still
        // stand apart by their level.
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation: 0,
                mesh: mesh.clone(),
                outline: outline.clone(),
            })
            .collect();
        Ok(Built {
            body: serde_json::to_vec(&asked).unwrap_or_default(),
            bounds,
            names,
            displays,
        })
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
