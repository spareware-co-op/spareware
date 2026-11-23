//! A roof (`roof`, job 5.3, CON-A4-F4-P3) as both builders read it: its filled recipe's form, the
//! edges it rises from, its eaves outline and each layer with the solid it is, in the steps of
//! construct-params' length grid (6,400 to the millimetre), checked for what a builder needs and
//! nothing more.
//!
//! What it reads: the recipe construct-roofs' `filled` writes, which the regenerator builds from
//! ([`crate::roofs`]): `form` (`flat`, `mono` or `gable`); `slopes`, the edges the roof rises from;
//! `outline`, the eaves outline anticlockwise from above, each corner `{"id":<edge ID>,"at":[x,y]}`;
//! and `layers`, from the top face down, each `{"function","material","thickness"}` and, for a
//! layer with a thickness, `solid`: `{"extrude":[[x,y,z], ...],"by":[x,y,z]}` or
//! `{"loft":[[[x,y,z], ...],[[x,y,z], ...]]}`. The roof's base is `z = 0` in its own frame (its
//! level's elevation plus its offset, the height its built row holds).
//!
//! What it promises: a recipe it reads has an outline of at least three corners, every edge ID
//! once, every point on the kernel's grid (within 2^40 - 1 steps of the origin), at least one
//! layer with a solid, every loft two sections of one count of at least three corners, and a
//! sloping edge only among the outline's edges and only on a sloped form. A recipe whose layers are
//! not filled yet is `Unfilled`. Where each corner is, and that it is right, is construct-roofs'
//! (its `shape`), and the engine's checker judges every solid it returns.
//!
//! Faces' roles ([`RoofPlan::roles`]): a flat roof's `top`, `bottom`, `face.<k>` between layer
//! `k - 1` and layer `k`, and `side.<edge ID>` for each edge's eaves or verge; a sloped roof names
//! each slope by the edge it rises from, `top.<edge>`, `bottom.<edge>` and `face.<k>.<edge>`,
//! and `side.<edge ID>` for every edge.
//!
//! Depends on construct-model (the recipe) and [`crate::floor_plan`] (a corner of a loop). Nothing
//! here is interim.

use crate::floor_plan::PlanCorner;
use construct_model::geometry::{GeometryError, Recipe};
use serde_json::Value;
use std::collections::BTreeSet;

/// A roof's recipe operation (construct-roofs' `ROOF_OP`).
pub const OP: &str = "roof";
/// The version of a roof's recipe this module reads (construct-roofs' `OP_VERSION`).
pub const OP_VERSION: u32 = 1;
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The farthest a point may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
const GRID_LIMIT: i64 = 1_099_511_627_775;

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Whether `recipe` is a roof's.
#[must_use]
pub fn is_roof(recipe: &Recipe) -> bool {
    recipe.owner == OWN && recipe.op == OP
}

/// The solid of one layer, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanSolid {
    /// A level profile extruded by a vector.
    Extrude {
        /// The profile's corners.
        profile: Vec<[i64; 3]>,
        /// The vector.
        by: [i64; 3],
    },
    /// A loft from one section to the other, corner to corner.
    Loft {
        /// The two sections.
        sections: [Vec<[i64; 3]>; 2],
    },
}

impl PlanSolid {
    /// Every point it names.
    #[must_use]
    pub fn points(&self) -> Vec<[i64; 3]> {
        match self {
            Self::Extrude { profile, by } => {
                let mut out = profile.clone();
                out.extend(profile.iter().map(|p| {
                    [
                        p[0].saturating_add(by[0]),
                        p[1].saturating_add(by[1]),
                        p[2].saturating_add(by[2]),
                    ]
                }));
                out
            }
            Self::Loft { sections } => sections.concat(),
        }
    }
}

/// One layer, as the filled recipe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoofLayer {
    /// Its function's stored name (construct-slabs' `Function`).
    pub function: String,
    /// Its material's ID.
    pub material: String,
    /// Its thickness in grid steps, square to its faces, at or above zero.
    pub thickness: i64,
    /// Its solid; none for a layer of no thickness.
    pub solid: Option<PlanSolid>,
}

/// A roof read from its filled recipe ([`RoofPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoofPlan {
    /// `flat`, `mono` or `gable`.
    pub form: String,
    /// The edges it rises from, in the footprint's order.
    pub slopes: Vec<String>,
    /// The eaves outline, anticlockwise from above.
    pub outline: Vec<PlanCorner>,
    /// The layers from the top face down.
    pub layers: Vec<RoofLayer>,
}

impl RoofPlan {
    /// Reads a roof's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotARoof` for a recipe of another operation or version; `Unfilled` for one whose layers
    /// are not filled yet; `MissingArgument` for arguments not as the module header says;
    /// `EmptyShape` for an outline of fewer than three corners, a loft section of fewer than three,
    /// or no layer with a solid; `TooBig` for a point beyond the grid; `DuplicateEdge` for an edge
    /// ID used twice.
    pub fn of(recipe: &Recipe) -> Result<RoofPlan, GeometryError> {
        if !is_roof(recipe) || recipe.op_version != OP_VERSION {
            return Err(fault(
                "NotARoof",
                format!(
                    "a recipe of {}/{} version {} is not a roof's",
                    recipe.owner, recipe.op, recipe.op_version
                ),
            ));
        }
        let args = &recipe.args;
        let layers = args
            .get("layers")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                fault(
                    "Unfilled",
                    "its layers are read from its type and were not filled in",
                )
            })?;
        let missing = |what: &str| fault("MissingArgument", format!("a roof's {what}"));
        let form = args
            .get("form")
            .and_then(Value::as_str)
            .filter(|f| ["flat", "mono", "gable"].contains(f))
            .ok_or_else(|| missing("form is flat, mono or gable"))?;
        let outline = read_outline(args.get("outline"))?;
        let ids: BTreeSet<&str> = outline.iter().map(|c| c.id.as_str()).collect();
        let slopes: Vec<String> = args
            .get("slopes")
            .and_then(Value::as_array)
            .ok_or_else(|| missing("slopes are a list of edges"))?
            .iter()
            .map(|s| s.as_str().filter(|s| ids.contains(s)).map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or_else(|| missing("slopes are edges of its outline"))?;
        let expected = match form {
            "flat" => 0,
            "mono" => 1,
            _ => 2,
        };
        if slopes.len() != expected {
            return Err(missing("slopes are as many as its form has"));
        }
        let layers = layers
            .iter()
            .enumerate()
            .map(|(at, layer)| read_layer(layer, at))
            .collect::<Result<Vec<_>, _>>()?;
        if !layers.iter().any(|layer| layer.solid.is_some()) {
            return Err(fault("EmptyShape", "a roof has a layer with a thickness"));
        }
        Ok(RoofPlan {
            form: form.to_owned(),
            slopes,
            outline,
            layers,
        })
    }

    /// The roles of the roof's faces, as the module header says, in order.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        let mut roles = Vec::new();
        if self.slopes.is_empty() {
            roles.push("top".to_owned());
            roles.push("bottom".to_owned());
            for k in 1..self.layers.len() {
                roles.push(format!("face.{k}"));
            }
        }
        for edge in &self.slopes {
            roles.push(format!("top.{edge}"));
            roles.push(format!("bottom.{edge}"));
            for k in 1..self.layers.len() {
                roles.push(format!("face.{k}.{edge}"));
            }
        }
        for corner in &self.outline {
            roles.push(format!("side.{}", corner.id));
        }
        roles
    }

    /// The roles of its outer faces only: its tops, bottoms and sides, as a stand-in box names
    /// them.
    #[must_use]
    pub fn outer_roles(&self) -> Vec<String> {
        self.roles()
            .into_iter()
            .filter(|role| !role.starts_with("face."))
            .collect()
    }

    /// The lowest and highest points of every layer's solid, in grid steps.
    #[must_use]
    pub fn extent(&self) -> ([i64; 3], [i64; 3]) {
        let mut lo = [i64::MAX; 3];
        let mut hi = [i64::MIN; 3];
        for point in self
            .layers
            .iter()
            .filter_map(|layer| layer.solid.as_ref())
            .flat_map(PlanSolid::points)
        {
            for ((l, h), c) in lo.iter_mut().zip(hi.iter_mut()).zip(point) {
                *l = (*l).min(c);
                *h = (*h).max(c);
            }
        }
        (lo, hi)
    }
}

/// A point on the kernel's grid.
fn on_grid(point: [i64; 3]) -> Result<[i64; 3], GeometryError> {
    if point
        .iter()
        .any(|c| c.unsigned_abs() > GRID_LIMIT.unsigned_abs())
    {
        return Err(fault(
            "TooBig",
            "a roof's point is beyond the kernel's grid",
        ));
    }
    Ok(point)
}

/// A list of grid points `[x, y, z]`.
fn read_points(value: &Value, what: &str) -> Result<Vec<[i64; 3]>, GeometryError> {
    let bad = || {
        fault(
            "MissingArgument",
            format!("{what} is a list of points [x,y,z]"),
        )
    };
    value
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|p| {
            let p = p.as_array().filter(|p| p.len() == 3).ok_or_else(bad)?;
            let c = |at: usize| p.get(at).and_then(Value::as_i64).ok_or_else(bad);
            on_grid([c(0)?, c(1)?, c(2)?])
        })
        .collect()
}

/// The eaves outline: its corners, each edge ID once.
fn read_outline(value: Option<&Value>) -> Result<Vec<PlanCorner>, GeometryError> {
    let bad = || {
        fault(
            "MissingArgument",
            "a roof's outline is a list of corners {\"id\",\"at\":[x,y]} in grid steps",
        )
    };
    let corners = value.and_then(Value::as_array).ok_or_else(bad)?;
    if corners.len() < 3 {
        return Err(fault(
            "EmptyShape",
            "a roof's outline has fewer than three corners",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(corners.len());
    for corner in corners {
        let id = corner.get("id").and_then(Value::as_str).ok_or_else(bad)?;
        let at = corner.get("at").and_then(Value::as_array).ok_or_else(bad)?;
        let (Some(x), Some(y), 2) = (
            at.first().and_then(Value::as_i64),
            at.get(1).and_then(Value::as_i64),
            at.len(),
        ) else {
            return Err(bad());
        };
        on_grid([x, y, 0])?;
        if !seen.insert(id.to_owned()) {
            return Err(fault(
                "DuplicateEdge",
                format!("edge {id} is used twice in one roof"),
            ));
        }
        out.push(PlanCorner {
            id: id.to_owned(),
            at: [x, y],
        });
    }
    Ok(out)
}

/// One layer of the recipe.
fn read_layer(layer: &Value, at: usize) -> Result<RoofLayer, GeometryError> {
    let bad = || {
        fault(
            "MissingArgument",
            format!(
                "layer {at} must be {{\"function\",\"material\",\"thickness\"}} and, with a thickness, its solid"
            ),
        )
    };
    let object = layer.as_object().ok_or_else(bad)?;
    let text = |key: &str| object.get(key).and_then(Value::as_str).ok_or_else(bad);
    let thickness = object
        .get("thickness")
        .and_then(Value::as_i64)
        .ok_or_else(bad)?;
    if thickness < 0 {
        return Err(fault(
            "NegativeThickness",
            format!("layer {at} is thinner than nothing"),
        ));
    }
    let solid = match object.get("solid") {
        None if thickness == 0 => None,
        None => return Err(bad()),
        Some(solid) => Some(read_solid(solid, at)?),
    };
    Ok(RoofLayer {
        function: text("function")?.to_owned(),
        material: text("material")?.to_owned(),
        thickness,
        solid,
    })
}

/// One layer's solid.
fn read_solid(solid: &Value, at: usize) -> Result<PlanSolid, GeometryError> {
    let what = format!("layer {at}'s solid");
    if let Some(profile) = solid.get("extrude") {
        let profile = read_points(profile, &what)?;
        let by = solid.get("by").cloned().unwrap_or_default();
        let by = read_points(&Value::Array(vec![by]), &what)?;
        let [by] = by.as_slice() else {
            return Err(fault("MissingArgument", format!("{what} has a vector")));
        };
        let by = *by;
        if profile.len() < 3 {
            return Err(fault(
                "EmptyShape",
                format!("{what} has fewer than three corners"),
            ));
        }
        return Ok(PlanSolid::Extrude { profile, by });
    }
    let sections = solid
        .get("loft")
        .and_then(Value::as_array)
        .filter(|s| s.len() == 2)
        .ok_or_else(|| {
            fault(
                "MissingArgument",
                format!("{what} is an extrusion or a loft of two sections"),
            )
        })?;
    let mut read = sections.iter().map(|s| read_points(s, &what));
    let (Some(first), Some(second)) = (read.next(), read.next()) else {
        return Err(fault("MissingArgument", format!("{what} has two sections")));
    };
    let (first, second) = (first?, second?);
    if first.len() < 3 || first.len() != second.len() {
        return Err(fault(
            "EmptyShape",
            format!("{what}'s sections are of one count of three corners or more"),
        ));
    }
    Ok(PlanSolid::Loft {
        sections: [first, second],
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roof(args: Value) -> Recipe {
        Recipe::construct(OP, serde_json::from_value(args).unwrap())
    }

    fn outline() -> Value {
        json!([
            { "id": "a", "at": [0, 0] },
            { "id": "b", "at": [6400, 0] },
            { "id": "c", "at": [6400, 6400] },
            { "id": "d", "at": [0, 6400] },
        ])
    }

    fn lofted() -> Value {
        let section = |x: i64| json!([[x, 0, 0], [x, 6400, 3200], [x, 6400, 4000], [x, 0, 800]]);
        json!({ "function": "structure", "material": "m", "thickness": 600,
                "solid": { "loft": [section(0), section(6400)] } })
    }

    /// A filled mono roof reads as its form, its slope, its outline and its layers; its roles name
    /// its slope by its edge; a membrane has no solid.
    #[test]
    fn a_filled_roof_reads_as_its_shape() {
        let membrane = json!({ "function": "membrane", "material": "m", "thickness": 0 });
        let plan = RoofPlan::of(&roof(json!({
            "form": "mono", "slopes": ["a"], "outline": outline(),
            "layers": [membrane, lofted()],
        })))
        .unwrap();
        assert_eq!(plan.form, "mono");
        assert_eq!(plan.layers[0].solid, None);
        assert_eq!(
            plan.roles(),
            [
                "top.a", "bottom.a", "face.1.a", "side.a", "side.b", "side.c", "side.d"
            ]
        );
        assert_eq!(
            plan.outer_roles(),
            ["top.a", "bottom.a", "side.a", "side.b", "side.c", "side.d"]
        );
        assert_eq!(plan.extent(), ([0, 0, 0], [6400, 6400, 4000]));
        let flat = json!({ "function": "structure", "material": "m", "thickness": 6400,
                           "solid": { "extrude": [[0, 0, 0], [6400, 0, 0], [6400, 6400, 0]], "by": [0, 0, 6400] } });
        let plan = RoofPlan::of(&roof(json!({
            "form": "flat", "slopes": [], "outline": outline(), "layers": [flat],
        })))
        .unwrap();
        assert_eq!(plan.roles()[..2], ["top", "bottom"]);
        assert_eq!(plan.extent().1, [6400, 6400, 6400]);
    }

    /// What is not a filled roof is refused, each with its code.
    #[test]
    fn what_a_builder_cannot_take_is_refused() {
        let code = |args: Value| RoofPlan::of(&roof(args)).unwrap_err().code;
        let base = || json!({ "form": "mono", "slopes": ["a"], "outline": outline(), "layers": [lofted()] });
        let with = |key: &str, value: Value| {
            let mut args = base();
            args[key] = value;
            args
        };
        assert_eq!(
            code(with("layers", json!({ "type": "layers" }))),
            "Unfilled"
        );
        assert_eq!(code(with("form", json!("hip"))), "MissingArgument");
        assert_eq!(code(with("slopes", json!(["z"]))), "MissingArgument");
        assert_eq!(code(with("slopes", json!([]))), "MissingArgument");
        assert_eq!(
            code(with("outline", json!([{ "id": "a", "at": [0, 0] }]))),
            "EmptyShape"
        );
        let twice = json!([{ "id": "a", "at": [0, 0] }, { "id": "a", "at": [1, 0] }, { "id": "b", "at": [0, 1] }]);
        assert_eq!(code(with("outline", twice)), "DuplicateEdge");
        let far = json!([{ "id": "a", "at": [0, 0] }, { "id": "b", "at": [1_i64 << 40, 0] }, { "id": "c", "at": [0, 1] }]);
        assert_eq!(code(with("outline", far)), "TooBig");
        let membrane = json!({ "function": "membrane", "material": "m", "thickness": 0 });
        assert_eq!(code(with("layers", json!([membrane]))), "EmptyShape");
        let solidless = json!({ "function": "structure", "material": "m", "thickness": 10 });
        assert_eq!(code(with("layers", json!([solidless]))), "MissingArgument");
        let uneven = json!({ "function": "structure", "material": "m", "thickness": 10,
                             "solid": { "loft": [[[0, 0, 0], [0, 1, 0], [0, 1, 1]], [[1, 0, 0], [1, 1, 0]]] } });
        assert_eq!(code(with("layers", json!([uneven]))), "EmptyShape");
        let thin = json!({ "function": "structure", "material": "m", "thickness": -1 });
        assert_eq!(code(with("layers", json!([thin]))), "NegativeThickness");
        let mut other = roof(base());
        other.op_version = 2;
        assert_eq!(RoofPlan::of(&other).unwrap_err().code, "NotARoof");
    }
}
