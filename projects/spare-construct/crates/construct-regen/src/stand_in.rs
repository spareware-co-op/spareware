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
//!
//! A layered wall (`wall.layered`) is one box of its whole thickness: the prism on its outer
//! outline ([`crate::wall_plan`]), each corner the nearest whole millimetre, named as a box is;
//! its layers are the kernel's to build as solids. Its displays draw each layer as a part, so a
//! view shows the layers and their materials on either edition.
//!
//! Interim: it cuts no openings. A wall built with the doors hosted in it
//! ([`Geometry::build_hosting`]) is built alone, as it was before hosting, and says so; the
//! kernel's builder ([`crate::kernel`]) cuts them. The interim register has the row.

use crate::floor_plan::{self, FloorPlan};
use crate::wall_plan::{self, Layered};
use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Hosted, Mesh, Part, Recipe,
};
use construct_params::Length;
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
    let names = floor_names(&plan);
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
            parts: Vec::new(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds { min, max },
        names,
        displays,
    })
}

/// A floor's face names: `box.top`, `box.bottom` and `box.side.<edge ID>` for each edge of each
/// loop, in loop order.
fn floor_names(plan: &FloorPlan) -> Vec<FaceName> {
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
    names
}

/// The persistent names of a box's faces, one per role: the same six for every box.
fn box_names() -> Vec<FaceName> {
    ROLES
        .iter()
        .map(|role| FaceName {
            name: format!("box.{role}"),
            role: (*role).to_owned(),
        })
        .collect()
}

/// The persistent names of the faces `recipe` builds, without building it: what a hosted element
/// names its host's face by.
///
/// # Errors
///
/// As [`Geometry::build`]: a recipe it cannot build has no faces.
pub fn names(recipe: &Recipe) -> Result<Vec<FaceName>, GeometryError> {
    if floor_plan::is_floor(recipe) {
        return Ok(floor_names(&FloorPlan::of(recipe)?));
    }
    if wall_plan::is_layered(recipe) {
        Layered::of(recipe)?;
    } else {
        size_of(recipe)?;
    }
    Ok(box_names())
}

fn too_many() -> GeometryError {
    GeometryError {
        code: "TooBig".to_owned(),
        message: "a wall with more layers than a display can count".to_owned(),
    }
}

/// A count of a display's corners or triangles, as its mesh numbers them.
fn part_count(n: usize) -> Result<u32, GeometryError> {
    u32::try_from(n).map_err(|_| too_many())
}

/// A layered wall's body, as the stand-in stores it: its outer outline and its height, in whole
/// millimetres.
#[derive(Serialize)]
struct WallBody {
    kind: &'static str,
    outline: Vec<[i64; 2]>,
    height: i64,
}

/// A layered wall as one box of its whole thickness: the prism on its outer outline, the strip
/// from its exterior face to its interior face ([`crate::wall_plan`]), each corner the nearest
/// whole millimetre, from its base to its top. Its bounds are rounded outward. Its displays draw
/// each layer with a thickness as a part of its own, the prism on that layer's strip, so a view
/// shows the layers and their materials as the kernel's displays do; its body is the one box.
fn layered(recipe: &Recipe) -> Result<Built, GeometryError> {
    let wall = Layered::of(recipe)?;
    let (exterior, interior) = wall.outer();
    let (corners, _) = wall.plan.strip(exterior, interior)?;
    let mut rounded = false;
    let mut near = |steps: i64| {
        let (mm, exact) = wall_plan::nearest_mm(steps);
        rounded |= !exact;
        mm
    };
    let outline: Vec<[i64; 2]> = corners.iter().map(|&[x, y]| [near(x), near(y)]).collect();
    let height = near(wall.height);
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut parts = Vec::new();
    for layer in wall.layers.iter().filter(|layer| layer.thickness() > 0) {
        let (strip, _) = wall.plan.strip(layer.exterior, layer.interior)?;
        let shape: Vec<[i64; 2]> = strip.iter().map(|&[x, y]| [near(x), near(y)]).collect();
        let (first, base) = (
            part_count(indices.len() / 3)?,
            part_count(positions.len() / 3)?,
        );
        for z in [0, height] {
            for &[x, y] in &shape {
                positions.extend([x, y, z]);
            }
        }
        for index in TRIANGLES {
            indices.push(base.checked_add(index).ok_or_else(too_many)?);
        }
        parts.push(Part {
            material: layer.material.clone(),
            function: layer.function.clone(),
            triangles: [first, part_count(TRIANGLES.len() / 3)?],
            outline: shape,
        });
    }
    let deviation = i64::from(rounded);
    let (mut min, mut max) = ([0, 0, 0], [0, 0, wall.height]);
    for axis in 0..2 {
        let values = corners.iter().filter_map(|c| c.get(axis).copied());
        if let (Some(lo), Some(hi)) = (values.clone().min(), values.max())
            && let (Some(l), Some(h)) = (min.get_mut(axis), max.get_mut(axis))
        {
            (*l, *h) = (lo, hi);
        }
    }
    let per = Length::STEPS_PER_MM;
    let floor = |s: i64| s.checked_div_euclid(per).unwrap_or(0);
    let ceil = |s: i64| match s.checked_rem_euclid(per) {
        Some(0) => floor(s),
        _ => floor(s).saturating_add(1),
    };
    let body = WallBody {
        kind: "wall",
        outline: outline.clone(),
        height,
    };
    let displays = (0..2)
        .map(|lod| Display {
            lod,
            deviation,
            mesh: Mesh {
                positions: positions.clone(),
                indices: indices.clone(),
            },
            outline: outline.clone(),
            parts: parts.clone(),
        })
        .collect();
    Ok(Built {
        body: serde_json::to_vec(&body).unwrap_or_default(),
        bounds: Bounds {
            min: min.map(floor),
            max: max.map(ceil),
        },
        names: box_names(),
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
        if wall_plan::is_layered(recipe) {
            return layered(recipe);
        }
        let size = size_of(recipe)?;
        let [w, d, _] = size;
        let body = serde_json::to_vec(&BoxBody { kind: "box", size }).unwrap_or_default();
        let names = box_names();
        let outline = vec![[0, 0], [w, 0], [w, d], [0, d]];
        // A box has no finer or coarser form, so both levels are the same shape. The kernel's
        // levels will differ: their bytes do here too, so the records already stand apart.
        let displays = (0..2)
            .map(|lod| Display {
                lod,
                deviation: 0,
                mesh: mesh(size),
                outline: outline.clone(),
                parts: Vec::new(),
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

    /// Interim (the module header): the stand-in cuts no openings, so a host is built alone,
    /// whatever is hosted in it, exactly as [`Geometry::build`] builds it.
    fn build_hosting(&self, host: &Recipe, hosted: &[Hosted<'_>]) -> Result<Built, GeometryError> {
        let _ = hosted;
        self.build(host)
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

    /// The stand-in cuts no openings: a wall with a door hosted in it is the wall alone, byte for
    /// byte, and its faces are named without building it as building names them.
    #[test]
    fn a_wall_with_a_door_is_the_wall_alone() {
        let wall = recipe(
            "wall.straight",
            json!({ "length": 3000, "thickness": 200, "height": 2700 }),
        );
        let door = recipe(
            "door.single",
            json!({ "width": 900, "thickness": 200, "height": 2100 }),
        );
        let at = construct_model::element::Placement {
            x: 1000,
            y: 0,
            z: 0,
        };
        let hosted = [Hosted {
            key: "D-1",
            recipe: &door,
            at,
        }];
        let alone = Boxes.build(&wall).unwrap();
        assert_eq!(Boxes.build_hosting(&wall, &hosted).unwrap(), alone);
        assert_eq!(names(&wall).unwrap(), alone.names);
        assert_eq!(
            names(&recipe("stair.spiral", json!({}))).unwrap_err().code,
            "UnknownOperation"
        );
    }

    fn q0(steps: i64) -> String {
        construct_params::Value::Length(Length::from_steps(steps)).stored()
    }

    /// The default partition (12.5, 90, 12.5 mm) on a line from `start` to `end` in steps, 2700
    /// mm high, centred on its line.
    fn partition(start: [i64; 2], end: [i64; 2]) -> Recipe {
        let layer = |f: &str, a: i64, b: i64| json!({ "function": f, "material": "m", "exterior": q0(a), "interior": q0(b) });
        recipe(
            "wall.layered",
            json!({
                "start": [q0(start[0]), q0(start[1])],
                "end": [q0(end[0]), q0(end[1])],
                "height": q0(17_280_000),
                "layers": [
                    layer("finish", 368_000, 288_000),
                    layer("structure", 288_000, -288_000),
                    layer("finish", -288_000, -368_000),
                ],
            }),
        )
    }

    /// A layered wall is one box of its whole thickness: the prism on its outer outline, each
    /// corner the nearest millimetre (57.5 mm rounds away from zero), its bounds rounded outward,
    /// named as a box is; a layered wall at an angle is its outer parallelogram. Its recorded
    /// bytes never change within an edition.
    #[test]
    fn a_layered_wall_is_one_box_of_its_whole_thickness() {
        let wall = partition([0, 0], [25_600_000, 0]);
        let built = Boxes.build(&wall).unwrap();
        let outline = vec![[0, -58], [4000, -58], [4000, 58], [0, 58]];
        assert_eq!(built.displays[0].outline, outline);
        assert_eq!(built.displays[0].deviation, 1);
        assert_eq!(built.bounds.min, [0, -58, 0]);
        assert_eq!(built.bounds.max, [4000, 58, 2700]);
        assert_eq!(built.names, box_names());
        assert_eq!(names(&wall).unwrap(), built.names);
        assert_eq!(
            built.body,
            br#"{"kind":"wall","outline":[[0,-58],[4000,-58],[4000,58],[0,58]],"height":2700}"#
        );
        // Each layer is a part of the displays: its strip's prism, its material and function.
        let parts = &built.displays[0].parts;
        let outlines: Vec<&Vec<[i64; 2]>> = parts.iter().map(|part| &part.outline).collect();
        assert_eq!(
            outlines,
            [
                &vec![[0, -58], [4000, -58], [4000, -45], [0, -45]],
                &vec![[0, -45], [4000, -45], [4000, 45], [0, 45]],
                &vec![[0, 45], [4000, 45], [4000, 58], [0, 58]],
            ]
        );
        let triangles: Vec<[u32; 2]> = parts.iter().map(|part| part.triangles).collect();
        assert_eq!(triangles, [[0, 12], [12, 12], [24, 12]]);
        let functions: Vec<&str> = parts.iter().map(|part| part.function.as_str()).collect();
        assert_eq!(functions, ["finish", "structure", "finish"]);
        assert!(parts.iter().all(|part| part.material == "m"));
        assert_eq!(built.displays[0].mesh.positions.len(), 72);
        assert_eq!(built.displays[0].mesh.indices.len(), 108);
        assert_eq!(built.displays[0].mesh.indices[36], 8);
        assert_eq!(built.displays[1].mesh, built.displays[0].mesh);
        assert_eq!(built.displays[1].parts, built.displays[0].parts);
        // A 3-4-5 wall from the origin: its exterior face is (46, -34.5) mm off its line.
        let angled = Boxes
            .build(&partition([0, 0], [15_360_000, 20_480_000]))
            .unwrap();
        assert_eq!(
            angled.displays[0].outline,
            vec![[46, -35], [2446, 3166], [2354, 3235], [-46, 35]]
        );
        assert_eq!(angled.bounds.min, [-46, -35, 0]);
        assert_eq!(angled.bounds.max, [2446, 3235, 2700]);
        let mut unfilled = wall;
        unfilled
            .args
            .insert("layers".to_owned(), json!({ "wall": "type" }));
        assert_eq!(Boxes.build(&unfilled).unwrap_err().code, "Unfilled");
        assert_eq!(names(&unfilled).unwrap_err().code, "Unfilled");
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
        assert_eq!(names(&floor_of(false)).unwrap(), l.names);
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
