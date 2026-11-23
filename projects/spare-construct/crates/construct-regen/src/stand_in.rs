//! The stand-in for the geometry kernel: every shape is a box. This is the only file that knows
//! that. It keeps the kernel's contract, so that the kernel can replace it without anything above
//! noticing: pure, deterministic, whole numbers, canonical bytes, named faces, and a mesh per level
//! of detail.
//!
//! The operations are Spare Construct's, named for what they make: `wall.straight`, not "box".
//! A floor (`floor`) is one box: the box around its outer loop in plan, each side rounded outward
//! to a whole millimetre, from its bottom face to its top face at `z = 0` ([`crate::floor_plan`]);
//! its outline is its outer loop, each corner the nearest millimetre, and its deviation is 0 only
//! when the box is the floor. Interim, as every stand-in shape: its layers and holes are the
//! kernel's to build (the interim register's row).

use crate::floor_plan::{self, FloorPlan};
use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Mesh, Recipe,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// The stand-in's edition. A change to what it builds is a new version, as with the kernel.
const VERSION: u32 = 1;

/// The stand-in: builds every recipe as a box, until the geometry kernel can build it.
pub struct Boxes;

/// The six sides of a box, in the kernel's role words (20.2).
const ROLES: [&str; 6] = ["base", "top", "exterior", "interior", "end0", "end1"];
const TRIANGLES: [u32; 36] = [
    0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 0, 4, 7, 0, 7, 3, 1, 2,
    6, 1, 6, 5,
];

/// A body, as the stand-in stores it.
#[derive(Serialize)]
struct BoxBody {
    kind: &'static str,
    size: [i64; 3],
}

fn whole(args: &BTreeMap<String, Value>, name: &str) -> Result<i64, GeometryError> {
    let value = args.get(name).and_then(Value::as_i64);
    let value = value.ok_or_else(|| GeometryError {
        code: "MissingArgument".to_owned(),
        message: format!("{name} must be a whole number of millimetres"),
    })?;
    if value <= 0 {
        return Err(GeometryError {
            code: "EmptyShape".to_owned(),
            message: format!("{name} must be above zero"),
        });
    }
    Ok(value)
}

/// Whether a linear element runs along x, or turns to run along y.
fn runs_along_y(args: &BTreeMap<String, Value>) -> bool {
    args.get("axis").and_then(Value::as_str) == Some("y")
}

/// The box an operation makes: its size along x, y and z.
fn size_of(recipe: &Recipe) -> Result<[i64; 3], GeometryError> {
    let args = &recipe.args;
    let linear = |long: &str, across: &str| -> Result<[i64; 3], GeometryError> {
        let (long, across, height) = (
            whole(args, long)?,
            whole(args, across)?,
            whole(args, "height")?,
        );
        Ok(if runs_along_y(args) {
            [across, long, height]
        } else {
            [long, across, height]
        })
    };
    match recipe.op.as_str() {
        "wall.straight" => linear("length", "thickness"),
        "door.single" => linear("width", "thickness"),
        "block" => Ok([
            whole(args, "width")?,
            whole(args, "depth")?,
            whole(args, "height")?,
        ]),
        other => Err(GeometryError {
            code: "UnknownOperation".to_owned(),
            message: format!("this edition can't build {other}"),
        }),
    }
}

fn mesh(size: [i64; 3]) -> Mesh {
    let [w, d, h] = size;
    let corners = [
        [0, 0, 0],
        [w, 0, 0],
        [w, d, 0],
        [0, d, 0],
        [0, 0, h],
        [w, 0, h],
        [w, d, h],
        [0, d, h],
    ];
    Mesh {
        positions: corners.concat(),
        indices: TRIANGLES.to_vec(),
    }
}

/// A floor's body, as the stand-in stores it: the box it is, in whole millimetres.
#[derive(Serialize)]
struct FloorBody {
    kind: &'static str,
    min: [i64; 3],
    max: [i64; 3],
}

/// A floor as one box ([`crate::floor_plan`]): the box around its outer loop, its sides rounded
/// outward to whole millimetres, from its bottom face (rounded down) to its top face at 0. Named
/// `box.top`, `box.bottom` and `box.side.<edge ID>` for each edge of each loop, in loop order.
fn floor(recipe: &Recipe) -> Result<Built, GeometryError> {
    let plan = FloorPlan::of(recipe)?;
    let total = plan.total().unwrap_or_default();
    let ([x0, y0], [x1, y1]) = plan.extent();
    let min = [
        floor_plan::floor_mm(x0),
        floor_plan::floor_mm(y0),
        floor_plan::floor_mm(total.saturating_neg()),
    ];
    let max = [floor_plan::ceil_mm(x1), floor_plan::ceil_mm(y1), 0];
    let mut rounded = false;
    let mut outline = Vec::with_capacity(plan.outer().len());
    for corner in plan.outer() {
        let [x, y] = corner.at;
        let ((x, a), (y, b)) = (floor_plan::nearest_mm(x), floor_plan::nearest_mm(y));
        rounded |= !(a && b);
        outline.push([x, y]);
    }
    rounded |= !floor_plan::nearest_mm(total).1;
    // The box strays from the floor by nothing when it is the floor; otherwise by at most its
    // plan's diagonal (every point of one is within that of every point of the other), and by
    // one millimetre more where a corner or the thickness was rounded.
    let deviation = if plan.is_box() {
        i64::from(rounded)
    } else {
        let [w, d] = [
            i128::from(max[0].saturating_sub(min[0])),
            i128::from(max[1].saturating_sub(min[1])),
        ];
        let square = w.saturating_mul(w).saturating_add(d.saturating_mul(d));
        let mut root = square.isqrt();
        if root.saturating_mul(root) < square {
            root = root.saturating_add(1);
        }
        i64::try_from(root)
            .unwrap_or(i64::MAX)
            .saturating_add(i64::from(rounded))
    };
    let [lx, ly, lz] = min;
    let [hx, hy, hz] = max;
    let corners = [
        [lx, ly, lz],
        [hx, ly, lz],
        [hx, hy, lz],
        [lx, hy, lz],
        [lx, ly, hz],
        [hx, ly, hz],
        [hx, hy, hz],
        [lx, hy, hz],
    ];
    let mut names = vec![
        FaceName {
            name: "box.top".to_owned(),
            role: "top".to_owned(),
        },
        FaceName {
            name: "box.bottom".to_owned(),
            role: "bottom".to_owned(),
        },
    ];
    for corner in plan.loops.iter().flatten() {
        let role = format!("side.{}", corner.id);
        names.push(FaceName {
            name: format!("box.{role}"),
            role,
        });
    }
    let body = FloorBody {
        kind: "floor",
        min,
        max,
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: Mesh {
                positions: corners.concat(),
                indices: TRIANGLES.to_vec(),
            },
            outline: outline.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names,
        displays,
    })
}

impl Geometry for Boxes {
    fn edition(&self) -> Edition {
        Edition {
            engine: "construct.stand-in".to_owned(),
            version: VERSION,
            name_scheme: 0,
            report: None,
        }
    }

    fn build(&self, recipe: &Recipe) -> Result<Built, GeometryError> {
        if floor_plan::is_floor(recipe) {
            return floor(recipe);
        }
        let size = size_of(recipe)?;
        let [w, d, _] = size;
        let body = serde_json::to_vec(&BoxBody { kind: "box", size }).unwrap_or_default();
        let names = ROLES
            .iter()
            .map(|role| FaceName {
                name: format!("box.{role}"),
                role: (*role).to_owned(),
            })
            .collect();
        let outline = vec![[0, 0], [w, 0], [w, d], [0, d]];
        // A box has no finer or coarser form, so both levels are the same shape. The kernel's
        // levels will differ: their bytes do here too, so the records already stand apart.
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation: 0,
                mesh: mesh(size),
                outline: outline.clone(),
            })
            .collect();
        Ok(Built {
            body,
            bounds: Bounds {
                min: [0, 0, 0],
                max: size,
            },
            names,
            displays,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn recipe(op: &str, args: Value) -> Recipe {
        let args = serde_json::from_value(args).unwrap();
        Recipe::construct(op, args)
    }

    #[test]
    fn a_wall_is_as_long_as_it_says_along_its_axis() {
        let args = json!({ "length": 3000, "thickness": 200, "height": 2700 });
        let along_x = Boxes.build(&recipe("wall.straight", args)).unwrap();
        assert_eq!(along_x.bounds.max, [3000, 200, 2700]);
        let args = json!({ "length": 3000, "thickness": 200, "height": 2700, "axis": "y" });
        let along_y = Boxes.build(&recipe("wall.straight", args)).unwrap();
        assert_eq!(along_y.bounds.max, [200, 3000, 2700]);
    }

    #[test]
    fn the_same_recipe_always_gives_the_same_bytes() {
        let args = json!({ "width": 900, "thickness": 300, "height": 2100 });
        let one = Boxes.build(&recipe("door.single", args.clone())).unwrap();
        let two = Boxes.build(&recipe("door.single", args)).unwrap();
        assert_eq!(one.body_id(), two.body_id());
        assert_eq!(one.displays[0].id(), two.displays[0].id());
    }

    #[test]
    fn each_level_of_detail_is_a_record_of_its_own() {
        let args = json!({ "width": 900, "depth": 900, "height": 900 });
        let built = Boxes.build(&recipe("block", args)).unwrap();
        assert_ne!(built.displays[0].id(), built.displays[1].id());
    }

    #[test]
    fn a_recipe_that_cant_build_says_why() {
        let bad = Boxes.build(&recipe("stair.spiral", json!({}))).unwrap_err();
        assert_eq!(bad.code, "UnknownOperation");
        let bad = Boxes
            .build(&recipe("block", json!({ "width": 1 })))
            .unwrap_err();
        assert_eq!(bad.code, "MissingArgument");
        let args = json!({ "width": 0, "depth": 1, "height": 1 });
        assert_eq!(
            Boxes.build(&recipe("block", args)).unwrap_err().code,
            "EmptyShape"
        );
    }

    /// A filled floor: the L-shaped floor of 8 m by 8 m less its notch, with a 2 m square hole,
    /// its two layers 200 mm in all; or, with `rectangle`, a 4 m by 3 m rectangle.
    #[allow(clippy::arithmetic_side_effects, reason = "test code")]
    fn floor_of(rectangle: bool) -> Recipe {
        let mm = |v: i64| v * 6400;
        let corner = |id: &str, x: i64, y: i64| json!({ "id": id, "at": [mm(x), mm(y)] });
        let loops = if rectangle {
            json!([[
                corner("e0", 0, 0),
                corner("e1", 4000, 0),
                corner("e2", 4000, 3000),
                corner("e3", 0, 3000)
            ]])
        } else {
            json!([
                [
                    corner("e0", 0, 0),
                    corner("e1", 8000, 0),
                    corner("e2", 8000, 4000),
                    corner("e3", 4000, 4000),
                    corner("e4", 4000, 8000),
                    corner("e5", 0, 8000)
                ],
                [
                    corner("h3", 1000, 1000),
                    corner("h2", 1000, 3000),
                    corner("h1", 3000, 3000),
                    corner("h0", 3000, 1000)
                ]
            ])
        };
        let layer = |f: &str, t: i64| json!({ "function": f, "material": "m", "thickness": mm(t) });
        recipe(
            "floor",
            json!({ "loops": loops, "layers": [layer("finish", 50), layer("structure", 150)] }),
        )
    }

    /// A floor is one box: the box around its outer loop from its bottom face to its top face at
    /// 0, its outline its outer loop, named top, bottom and a side per edge; it strays by nothing
    /// when it is a rectangle, and by the box's plan diagonal otherwise. Its bytes are pinned, and
    /// a floor whose layers are not filled in does not build.
    #[test]
    fn a_floor_is_the_box_around_its_outer_loop() {
        let rectangle = Boxes.build(&floor_of(true)).unwrap();
        assert_eq!(rectangle.bounds.min, [0, 0, -200]);
        assert_eq!(rectangle.bounds.max, [4000, 3000, 0]);
        assert_eq!(rectangle.displays[0].deviation, 0);
        assert_eq!(
            rectangle.body,
            br#"{"kind":"floor","min":[0,0,-200],"max":[4000,3000,0]}"#
        );
        let l = Boxes.build(&floor_of(false)).unwrap();
        assert_eq!(l.bounds.min, [0, 0, -200]);
        assert_eq!(l.bounds.max, [8000, 8000, 0]);
        assert_eq!(
            l.displays[0].outline,
            vec![
                [0, 0],
                [8000, 0],
                [8000, 4000],
                [4000, 4000],
                [4000, 8000],
                [0, 8000]
            ]
        );
        // The diagonal of 8 m by 8 m is 11,313.7 mm.
        assert_eq!(l.displays[0].deviation, 11_314);
        let roles: Vec<&str> = l.names.iter().map(|n| n.role.as_str()).collect();
        assert_eq!(roles.len(), 2 + 6 + 4);
        assert_eq!(&roles[..3], ["top", "bottom", "side.e0"]);
        assert_eq!(l.names[2].name, "box.side.e0");
        let mut unfilled = floor_of(true);
        unfilled
            .args
            .insert("layers".to_owned(), json!({ "type": "layers" }));
        assert_eq!(Boxes.build(&unfilled).unwrap_err().code, "Unfilled");
    }

    /// Recorded: a document's stored shapes must keep their meaning. A change here is a new
    /// edition, never an edit.
    #[test]
    fn the_stand_ins_bytes_never_change_within_an_edition() {
        let args = json!({ "width": 900, "thickness": 300, "height": 2100 });
        let built = Boxes.build(&recipe("door.single", args)).unwrap();
        assert_eq!(built.body, br#"{"kind":"box","size":[900,300,2100]}"#);
    }
}
