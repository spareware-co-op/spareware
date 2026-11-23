//! The stand-in for the geometry kernel: every shape is a box. This is the only file that knows
//! that. It keeps the kernel's contract, so that the kernel can replace it without anything above
//! noticing: pure, deterministic, whole numbers, canonical bytes, named faces, and a mesh per level
//! of detail.
//!
//! The operations are Spare Construct's, named for what they make: `wall.straight`, not "box".
//!
//! Interim: it cuts no openings. A wall built with the doors hosted in it
//! ([`Geometry::build_hosting`]) is built alone, as it was before hosting, and says so; the
//! kernel's builder ([`crate::kernel`]) cuts them. The interim register has the row.

use construct_model::geometry::{
    Bounds, Built, Display, Edition, FaceName, Geometry, GeometryError, Hosted, Mesh, Recipe,
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
    size_of(recipe)?;
    Ok(box_names())
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

    /// Recorded: a document's stored shapes must keep their meaning. A change here is a new
    /// edition, never an edit.
    #[test]
    fn the_stand_ins_bytes_never_change_within_an_edition() {
        let args = json!({ "width": 900, "thickness": 300, "height": 2100 });
        let built = Boxes.build(&recipe("door.single", args)).unwrap();
        assert_eq!(built.body, br#"{"kind":"box","size":[900,300,2100]}"#);
    }
}
