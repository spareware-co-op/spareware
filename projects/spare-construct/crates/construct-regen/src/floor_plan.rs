//! A floor (`floor`, job 4.3, CON-A4-F4-P1) as both builders read it: its filled recipe's loops
//! and layers, in the steps of construct-params' length grid (6,400 to the millimetre), checked
//! for what a builder needs and nothing more.
//!
//! What it reads: the recipe construct-slabs' `filled` writes, which the regenerator builds from
//! ([`crate::slabs`]): `loops`, the outer loop anticlockwise seen from above and then each hole
//! clockwise, each corner `{"id":<edge ID>,"at":[x,y]}`; and `layers`, from the top face down,
//! each `{"function","material","thickness"}`. The floor's top face is at `z = 0` in its own frame
//! and its layers hang below, so layer `k` runs from `z = -depth(k + 1)` up to `z = -depth(k)`
//! (construct-slabs' level rule: the top face sits at its level's elevation plus its offset, the
//! height its built row holds).
//!
//! A sloped floor or one with slab edges (job 5.2) is op version [`SHAPED_VERSION`], and its
//! filled recipe also holds `tops` (a sloped floor's: the top face's height at each corner of each
//! loop, in the loops' order, in grid steps; every corner 0 when it is left out) or `edges` (each
//! slab edge as `{"id":<edge ID>,"kind":<kind>,"top":[[x,y,z] x4],"foot":[[x,y,z] x4]}`, its plan
//! at the floor's underside and at its foot). construct-slabs' `Shape` worked both out and checked
//! them (every corner on the grid, every piece inside its floor and apart from the others).
//!
//! What it promises: a recipe it reads has a loop of at least three corners first, every corner
//! on the kernel's grid (within 2^40 - 1 steps of the origin), every edge ID once in the floor,
//! at least one layer, no layer thinner than zero, and a total above zero and at most 10 m; a
//! height for every corner when it has tops; every slab edge on an edge of the floor, of a known
//! kind, its foot below its top. A recipe whose layers or boundary are not filled yet is
//! `Unfilled`. Whether the loops are simple, closed and nested properly is construct-slabs' check,
//! made when the regenerator fills the recipe (its `Boundary`), and the engine's checker judges
//! every solid it returns.
//!
//! Depends on construct-model (the recipe) and construct-params (the grid's steps per
//! millimetre). Nothing here is interim: every length is exact in construct-params' unit. Arc
//! edges are refused before this (construct-slabs, `SlabError::Arc`: the interim register's row).

use construct_model::geometry::{GeometryError, Recipe};
use construct_params::Length;
use serde_json::Value;
use std::collections::BTreeSet;

/// A floor's recipe operation (construct-slabs' `FLOOR_OP`).
pub const OP: &str = "floor";
/// A ceiling's recipe operation (construct-slabs' `CEILING_OP`): the same flat layered slab, so
/// both builders read it as a floor (job A101).
pub const CEILING_OP: &str = "ceiling";
/// The version of a flat floor's recipe this module reads (construct-slabs' `OP_VERSION`).
pub const OP_VERSION: u32 = 1;
/// The version of a sloped floor's or one with slab edges (construct-slabs' `SHAPED_VERSION`).
pub const SHAPED_VERSION: u32 = 2;
/// The owner of Spare Construct's own operations.
const OWN: &str = "construct";
/// The most a floor's layers may add up to: 10 m in steps (construct-slabs' `MOST_TOTAL`).
const MOST_TOTAL: i64 = Length::from_mm(10_000).steps();
/// The farthest a corner may be from the origin along an axis: the kernel's grid, 2^40 - 1 steps.
const GRID_LIMIT: i64 = 1_099_511_627_775;

fn fault(code: &str, message: impl Into<String>) -> GeometryError {
    GeometryError {
        code: code.to_owned(),
        message: message.into(),
    }
}

/// Whether `recipe` is a floor's or a ceiling's, the one flat layered slab the builders read.
#[must_use]
pub fn is_floor(recipe: &Recipe) -> bool {
    recipe.owner == OWN && (recipe.op == OP || recipe.op == CEILING_OP)
}

/// One corner of a loop: the ID of the edge that starts there, and where it is in grid steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCorner {
    /// The ID of the edge from this corner to the next; it names the floor's side face there.
    pub id: String,
    /// Where it is, `[x, y]` in grid steps.
    pub at: [i64; 2],
}

/// One layer, as the filled recipe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanLayer {
    /// Its function's stored name (construct-slabs' `Function`).
    pub function: String,
    /// Its material's ID.
    pub material: String,
    /// Its thickness in grid steps, at or above zero.
    pub thickness: i64,
}

/// One slab edge, as the filled recipe lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEdge {
    /// The ID of the edge of the floor it runs along.
    pub id: String,
    /// `thickened` or `downturn`.
    pub kind: String,
    /// Its plan at the floor's underside: four grid points at one height.
    pub top: [[i64; 3]; 4],
    /// Its plan at its foot: four grid points at one height below the top's.
    pub foot: [[i64; 3]; 4],
}

/// A floor read from its filled recipe ([`FloorPlan::of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorPlan {
    /// The outer loop, then each hole.
    pub loops: Vec<Vec<PlanCorner>>,
    /// The layers from the top face down.
    pub layers: Vec<PlanLayer>,
    /// A sloped floor's top face height at each corner of each loop; empty for a flat floor.
    pub tops: Vec<Vec<i64>>,
    /// Its slab edges, in order.
    pub edges: Vec<PlanEdge>,
}

/// A layer with a thickness, placed in the floor's frame: its index from the top, the layer, and
/// the heights of its bottom and its top in grid steps (at or below zero).
pub type Slab<'a> = (usize, &'a PlanLayer, i64, i64);

impl FloorPlan {
    /// Reads a floor's filled recipe.
    ///
    /// # Errors
    ///
    /// `NotAFloor` for a recipe of another operation or version; `Unfilled` for one whose layers
    /// or loops are not filled yet; `MissingArgument` for loops or layers not as the module
    /// header says; `EmptyShape` for a loop of fewer than three corners, no layers, or no
    /// thickness; `NegativeThickness`; `TooBig` for a corner beyond the grid or layers of more
    /// than 10 m; `DuplicateEdge` for an edge ID used twice; `MissingArgument` for tops or slab
    /// edges not as the module header says.
    pub fn of(recipe: &Recipe) -> Result<FloorPlan, GeometryError> {
        let known = recipe.op_version == OP_VERSION || recipe.op_version == SHAPED_VERSION;
        if !is_floor(recipe) || !known {
            return Err(fault(
                "NotAFloor",
                format!(
                    "a recipe of {}/{} version {} is not a floor's",
                    recipe.owner, recipe.op, recipe.op_version
                ),
            ));
        }
        let args = &recipe.args;
        let unfilled = |what: &str| {
            fault(
                "Unfilled",
                format!("its {what} are read from its records and were not filled in"),
            )
        };
        let layers = args
            .get("layers")
            .and_then(Value::as_array)
            .ok_or_else(|| unfilled("layers"))?;
        let loops = args
            .get("loops")
            .and_then(Value::as_array)
            .ok_or_else(|| unfilled("loops"))?;
        let mut seen = BTreeSet::new();
        let mut read_loops = Vec::with_capacity(loops.len());
        for (at, ring) in loops.iter().enumerate() {
            read_loops.push(read_loop(ring, at, &mut seen)?);
        }
        if read_loops.is_empty() {
            return Err(fault("EmptyShape", "a floor has an outer loop"));
        }
        let read_layers = layers
            .iter()
            .enumerate()
            .map(|(at, layer)| read_layer(layer, at))
            .collect::<Result<Vec<_>, _>>()?;
        let tops = match args.get("tops") {
            Some(tops) => read_tops(tops, &read_loops)?,
            None => Vec::new(),
        };
        let edges = match args.get("edges") {
            Some(edges) => read_edges(edges, &seen)?,
            None => Vec::new(),
        };
        let plan = FloorPlan {
            loops: read_loops,
            layers: read_layers,
            tops,
            edges,
        };
        if plan.layers.is_empty() {
            return Err(fault("EmptyShape", "a floor has at least one layer"));
        }
        match plan.total() {
            Some(0) => Err(fault("EmptyShape", "a floor's layers have no thickness")),
            Some(total) if total <= MOST_TOTAL => Ok(plan),
            _ => Err(fault("TooBig", "a floor's layers add up to more than 10 m")),
        }
    }

    /// The outer loop.
    #[must_use]
    pub fn outer(&self) -> &[PlanCorner] {
        self.loops.first().map_or(&[], Vec::as_slice)
    }

    /// The holes, in order.
    #[must_use]
    pub fn holes(&self) -> &[Vec<PlanCorner>] {
        self.loops.get(1..).unwrap_or_default()
    }

    /// The total thickness in grid steps; `None` beyond a 64-bit integer.
    #[must_use]
    pub fn total(&self) -> Option<i64> {
        self.layers
            .iter()
            .try_fold(0_i64, |sum, layer| sum.checked_add(layer.thickness))
    }

    /// Every layer with a thickness, top first, with the heights of its bottom and its top
    /// ([`Slab`]). A layer of no thickness (a membrane) is a face between its neighbours and has
    /// no solid.
    #[must_use]
    pub fn slabs(&self) -> Vec<Slab<'_>> {
        let mut out = Vec::with_capacity(self.layers.len());
        // The total is at most 10 m (`of`), so no depth here overflows.
        let mut depth = 0_i64;
        for (at, layer) in self.layers.iter().enumerate() {
            let below = depth.saturating_add(layer.thickness);
            if layer.thickness > 0 {
                out.push((at, layer, below.saturating_neg(), depth.saturating_neg()));
            }
            depth = below;
        }
        out
    }

    /// The lowest and highest corners of the outer loop in plan, in grid steps: every hole lies
    /// inside it.
    #[must_use]
    pub fn extent(&self) -> ([i64; 2], [i64; 2]) {
        let mut lo = [i64::MAX; 2];
        let mut hi = [i64::MIN; 2];
        for corner in self.outer() {
            for ((l, h), &c) in lo.iter_mut().zip(hi.iter_mut()).zip(&corner.at) {
                *l = (*l).min(c);
                *h = (*h).max(c);
            }
        }
        (lo, hi)
    }

    /// The top face's height at corner `k` of loop `ring`, in grid steps: 0 on a flat floor.
    #[must_use]
    pub fn top_at(&self, ring: usize, k: usize) -> i64 {
        self.tops
            .get(ring)
            .and_then(|heights| heights.get(k))
            .copied()
            .unwrap_or(0)
    }

    /// The top face's lowest and highest points, in grid steps: the outer loop's corners (a
    /// plane's highest and lowest over a polygon are corners, and every hole lies inside the
    /// outer loop); both 0 on a flat floor.
    #[must_use]
    pub fn top_range(&self) -> (i64, i64) {
        let outer = self.tops.first().map_or(&[][..], Vec::as_slice);
        (
            outer.iter().copied().min().unwrap_or(0),
            outer.iter().copied().max().unwrap_or(0),
        )
    }

    /// The floor's lowest point, in grid steps: the bottom face's lowest corner, or the deepest
    /// slab edge's foot.
    #[must_use]
    pub fn lowest(&self) -> i64 {
        let bottom = self
            .top_range()
            .0
            .saturating_sub(self.total().unwrap_or_default());
        self.edges
            .iter()
            .flat_map(|edge| edge.foot.iter().map(|p| p[2]))
            .fold(bottom, i64::min)
    }

    /// Whether the floor is exactly the box around its outer loop: one loop of four corners, each
    /// edge along x or y, no hole, no slope and no slab edges.
    #[must_use]
    pub fn is_box(&self) -> bool {
        let outer = self.outer();
        let flat = self.tops.iter().flatten().all(|&z| z == 0) && self.edges.is_empty();
        if !flat || !self.holes().is_empty() || outer.len() != 4 {
            return false;
        }
        outer
            .iter()
            .zip(outer.iter().cycle().skip(1))
            .all(|(a, b)| {
                let [ax, ay] = a.at;
                let [bx, by] = b.at;
                ax == bx || ay == by
            })
    }
}

/// One loop of the recipe: its corners, each edge ID new to the floor.
fn read_loop(
    ring: &Value,
    at: usize,
    seen: &mut BTreeSet<String>,
) -> Result<Vec<PlanCorner>, GeometryError> {
    let which = if at == 0 {
        "the outer loop".to_owned()
    } else {
        format!("hole {}", at.saturating_sub(1))
    };
    let bad = || {
        fault(
            "MissingArgument",
            format!("{which} must be a list of corners {{\"id\",\"at\":[x,y]}} in grid steps"),
        )
    };
    let corners = ring.as_array().ok_or_else(bad)?;
    if corners.len() < 3 {
        return Err(fault(
            "EmptyShape",
            format!("{which} has fewer than three corners"),
        ));
    }
    let mut out = Vec::with_capacity(corners.len());
    for corner in corners {
        let object = corner.as_object().ok_or_else(bad)?;
        let id = object.get("id").and_then(Value::as_str).ok_or_else(bad)?;
        let place = object.get("at").and_then(Value::as_array).ok_or_else(bad)?;
        let (Some(x), Some(y), 2) = (
            place.first().and_then(Value::as_i64),
            place.get(1).and_then(Value::as_i64),
            place.len(),
        ) else {
            return Err(bad());
        };
        if x.unsigned_abs() > GRID_LIMIT.unsigned_abs()
            || y.unsigned_abs() > GRID_LIMIT.unsigned_abs()
        {
            return Err(fault(
                "TooBig",
                format!("{which}: corner {id} is beyond the kernel's grid"),
            ));
        }
        if !seen.insert(id.to_owned()) {
            return Err(fault(
                "DuplicateEdge",
                format!("{which}: edge {id} is used twice in one floor"),
            ));
        }
        out.push(PlanCorner {
            id: id.to_owned(),
            at: [x, y],
        });
    }
    Ok(out)
}

/// The tops of the recipe: one height for each corner of each loop, each within the grid.
fn read_tops(tops: &Value, loops: &[Vec<PlanCorner>]) -> Result<Vec<Vec<i64>>, GeometryError> {
    let bad = || {
        fault(
            "MissingArgument",
            "tops must be the top face's height at each corner of each loop, in grid steps",
        )
    };
    let lists = tops.as_array().ok_or_else(bad)?;
    if lists.len() != loops.len() {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(lists.len());
    for (list, ring) in lists.iter().zip(loops) {
        let heights = list
            .as_array()
            .filter(|heights| heights.len() == ring.len())
            .ok_or_else(bad)?
            .iter()
            .map(|z| {
                z.as_i64()
                    .filter(|z| z.unsigned_abs() <= GRID_LIMIT.unsigned_abs())
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(bad)?;
        out.push(heights);
    }
    Ok(out)
}

/// Four grid points `[x, y, z]` at one height, each within the grid.
fn read_plan(value: Option<&Value>) -> Option<[[i64; 3]; 4]> {
    let list = value?.as_array()?;
    let mut out = [[0_i64; 3]; 4];
    if list.len() != out.len() {
        return None;
    }
    for (slot, point) in out.iter_mut().zip(list) {
        let point = point.as_array()?;
        if point.len() != 3 {
            return None;
        }
        for (c, v) in slot.iter_mut().zip(point) {
            *c = v
                .as_i64()
                .filter(|v| v.unsigned_abs() <= GRID_LIMIT.unsigned_abs())?;
        }
    }
    let z = out.first()?[2];
    out.iter().all(|p| p[2] == z).then_some(out)
}

/// The slab edges of the recipe, each on an edge of the floor (`edges`), its foot below its top.
fn read_edges(edges: &Value, ids: &BTreeSet<String>) -> Result<Vec<PlanEdge>, GeometryError> {
    let bad = |what: &str| {
        fault(
            "MissingArgument",
            format!(
                "{what}: a slab edge is {{\"id\",\"kind\",\"top\",\"foot\"}}, each plan four grid \
                 points at one height"
            ),
        )
    };
    let list = edges.as_array().ok_or_else(|| bad("edges"))?;
    let mut out = Vec::with_capacity(list.len());
    for edge in list {
        let id = edge
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| ids.contains(*id))
            .ok_or_else(|| bad("a slab edge on no edge of the floor"))?;
        let kind = edge
            .get("kind")
            .and_then(Value::as_str)
            .filter(|kind| ["thickened", "downturn"].contains(kind))
            .ok_or_else(|| bad(&format!("slab edge {id}")))?;
        let (Some(top), Some(foot)) = (read_plan(edge.get("top")), read_plan(edge.get("foot")))
        else {
            return Err(bad(&format!("slab edge {id}")));
        };
        if foot[0][2] >= top[0][2] {
            return Err(bad(&format!("slab edge {id}'s foot is not below its top")));
        }
        out.push(PlanEdge {
            id: id.to_owned(),
            kind: kind.to_owned(),
            top,
            foot,
        });
    }
    Ok(out)
}

/// One layer of the recipe.
fn read_layer(layer: &Value, at: usize) -> Result<PlanLayer, GeometryError> {
    let bad = || {
        fault(
            "MissingArgument",
            format!(
                "layer {at} must be {{\"function\",\"material\",\"thickness\"}}, its thickness in grid steps"
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
    Ok(PlanLayer {
        function: text("function")?.to_owned(),
        material: text("material")?.to_owned(),
        thickness,
    })
}

/// `steps` of the length grid to the nearest whole millimetre, a half away from zero, and whether
/// that is exact: the layered wall's rounding ([`crate::wall_plan::nearest_mm`]), one for both.
pub use crate::wall_plan::nearest_mm;

/// `steps` in whole millimetres, rounded down: a bound's low corner.
#[must_use]
pub fn floor_mm(steps: i64) -> i64 {
    split(steps).0
}

/// `steps` in whole millimetres, rounded up: a bound's high corner.
#[must_use]
pub fn ceil_mm(steps: i64) -> i64 {
    match split(steps) {
        (floor, 0) => floor,
        (floor, _) => floor.saturating_add(1),
    }
}

/// `steps` as whole millimetres rounded down and the steps left over, at or above zero.
fn split(steps: i64) -> (i64, i64) {
    let per = Length::STEPS_PER_MM;
    // per is 6,400, above zero, so neither is `None`.
    (
        steps.checked_div_euclid(per).unwrap_or(0),
        steps.checked_rem_euclid(per).unwrap_or(0),
    )
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

    fn floor(args: Value) -> Recipe {
        Recipe::construct(OP, serde_json::from_value(args).unwrap())
    }

    fn square() -> Value {
        json!([
            { "id": "a", "at": [0, 0] },
            { "id": "b", "at": [6400, 0] },
            { "id": "c", "at": [6400, 6400] },
            { "id": "d", "at": [0, 6400] },
        ])
    }

    fn layer(thickness: i64) -> Value {
        json!({ "function": "structure", "material": "concrete-regular", "thickness": thickness })
    }

    /// A filled floor reads as its loops and its layers; its layers with a thickness hang below
    /// its top face in order, a membrane between them making no solid.
    #[test]
    fn a_filled_floor_reads_as_its_loops_and_layers() {
        let membrane = json!({ "function": "membrane", "material": "m", "thickness": 0 });
        let plan = FloorPlan::of(&floor(json!({
            "loops": [square()],
            "layers": [layer(320_000), membrane, layer(960_000)],
        })))
        .unwrap();
        assert_eq!(plan.outer().len(), 4);
        assert_eq!(plan.holes(), &[] as &[Vec<PlanCorner>]);
        assert_eq!(plan.total(), Some(1_280_000));
        let slabs: Vec<(usize, i64, i64)> = plan.slabs().iter().map(|s| (s.0, s.2, s.3)).collect();
        assert_eq!(slabs, [(0, -320_000, 0), (2, -1_280_000, -320_000)]);
        assert_eq!(plan.extent(), ([0, 0], [6400, 6400]));
        assert!(plan.is_box());
    }

    /// What is not a filled floor is refused, each with its code.
    #[test]
    fn what_a_builder_cannot_take_is_refused() {
        let code = |args: Value| FloorPlan::of(&floor(args)).unwrap_err().code;
        let unfilled = json!({ "boundary": {}, "layers": { "type": "layers" } });
        assert_eq!(code(unfilled), "Unfilled");
        assert_eq!(code(json!({ "layers": [layer(1)] })), "Unfilled");
        let two = json!([{ "id": "a", "at": [0, 0] }, { "id": "b", "at": [1, 0] }]);
        assert_eq!(
            code(json!({ "loops": [two], "layers": [layer(1)] })),
            "EmptyShape"
        );
        assert_eq!(
            code(json!({ "loops": [], "layers": [layer(1)] })),
            "EmptyShape"
        );
        assert_eq!(
            code(json!({ "loops": [square()], "layers": [] })),
            "EmptyShape"
        );
        assert_eq!(
            code(json!({ "loops": [square()], "layers": [layer(0)] })),
            "EmptyShape"
        );
        assert_eq!(
            code(json!({ "loops": [square()], "layers": [layer(-1)] })),
            "NegativeThickness"
        );
        let deep = Length::from_mm(10_001).steps();
        assert_eq!(
            code(json!({ "loops": [square()], "layers": [layer(deep)] })),
            "TooBig"
        );
        let far = json!([
            { "id": "a", "at": [0, 0] },
            { "id": "b", "at": [1_i64 << 40, 0] },
            { "id": "c", "at": [0, 1] },
        ]);
        assert_eq!(
            code(json!({ "loops": [far], "layers": [layer(1)] })),
            "TooBig"
        );
        assert_eq!(
            code(json!({ "loops": [square(), square()], "layers": [layer(1)] })),
            "DuplicateEdge"
        );
        let bare = json!([[0, 0], [1, 0], [0, 1]]);
        assert_eq!(
            code(json!({ "loops": [bare], "layers": [layer(1)] })),
            "MissingArgument"
        );
        let mut other = floor(json!({ "loops": [square()], "layers": [layer(1)] }));
        other.op_version = 3;
        assert_eq!(FloorPlan::of(&other).unwrap_err().code, "NotAFloor");
    }

    /// A sloped floor's tops and a slab edge read as they are written: its top range, its lowest
    /// point (the edge's foot or the bottom face under the slope's lowest corner, whichever is
    /// lower), and no box; tops that miss a corner, an edge on no edge of the floor, an unknown
    /// kind and a foot not below its top are refused.
    #[test]
    fn a_shaped_floor_reads_its_tops_and_edges() {
        let shaped = |extra: Value| {
            let mut args = json!({ "loops": [square()], "layers": [layer(1000)] });
            for (k, v) in extra.as_object().unwrap() {
                args[k] = v.clone();
            }
            let mut recipe = floor(args);
            recipe.op_version = SHAPED_VERSION;
            FloorPlan::of(&recipe)
        };
        let plan = shaped(json!({ "tops": [[0, -50, -50, 0]] })).unwrap();
        assert_eq!(plan.top_range(), (-50, 0));
        assert_eq!(plan.top_at(0, 1), -50);
        assert_eq!(plan.lowest(), -1050);
        assert!(!plan.is_box());
        let edge = |id: &str, kind: &str, foot: i64| {
            let at = |z: i64| json!([[0, 0, z], [6400, 0, z], [6400, 100, z], [0, 100, z]]);
            json!({ "id": id, "kind": kind, "top": at(-1000), "foot": at(foot) })
        };
        let plan = shaped(json!({ "edges": [edge("a", "downturn", -3000)] })).unwrap();
        assert_eq!(plan.edges.len(), 1);
        assert_eq!(plan.lowest(), -3000);
        let code = |extra: Value| shaped(extra).unwrap_err().code;
        assert_eq!(code(json!({ "tops": [[0, 0, 0]] })), "MissingArgument");
        assert_eq!(
            code(json!({ "edges": [edge("z", "downturn", -3000)] })),
            "MissingArgument"
        );
        assert_eq!(
            code(json!({ "edges": [edge("a", "ledge", -3000)] })),
            "MissingArgument"
        );
        assert_eq!(
            code(json!({ "edges": [edge("a", "downturn", -1000)] })),
            "MissingArgument"
        );
    }

    /// Steps go to the nearest millimetre a half away from zero, saying when that is exact; a
    /// bound rounds outward; the grid's millimetre is construct-params'.
    #[test]
    fn steps_round_to_the_nearest_millimetre() {
        assert_eq!(nearest_mm(368_000), (58, false));
        assert_eq!(nearest_mm(-368_000), (-58, false));
        assert_eq!(nearest_mm(288_000), (45, true));
        assert_eq!(nearest_mm(3199), (0, false));
        assert_eq!(nearest_mm(-3201), (-1, false));
        assert_eq!(floor_mm(-368_000), -58);
        assert_eq!(ceil_mm(-368_000), -57);
        assert_eq!(ceil_mm(6400), 1);
        assert_eq!(Length::from_mm(1).steps(), 6400);
        assert_eq!(GRID_LIMIT, (1_i64 << 40) - 1);
        assert_eq!(MOST_TOTAL, 64_000_000);
    }
}
