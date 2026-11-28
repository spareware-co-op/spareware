//! A shape-edited floor (job 14.15): its top face divided into triangles by its points and split
//! lines, every height worked by hand; its layers' volumes with a variable layer, worked by hand;
//! its highest and lowest points; its stored form and a reset back to a plain floor's bytes; a
//! variable layer in a floor type's text; and every refusal by name.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{id, layer, mm, rectangle, slab};
use construct_materials::Materials;
use construct_params::{Length, Volume};
use construct_slabs::{
    Boundary, EDITED_VERSION, EdgeProfile, EditError, Edits, FACES_ARG, FORMAT_VERSION, Face,
    Function, Layers, OP_VERSION, Rise, Shape, SlabEdge, SlabError, SlopeArrow, SplitLine, Top,
    TopPoint, VARIABLE_VERSION, area, filled, floor_recipe, shape_layer_quantities, shape_of,
    shape_quantities, shape_read_only, shaped_recipe,
};
use construct_solver::Point;

/// Steps in a millimetre.
const MM: i64 = 6400;
/// Cubic steps in a cubic millimetre.
const MM3: i128 = 6400 * 6400 * 6400;

/// The 4 m by 3 m rectangle, edges `e0` to `e3` anticlockwise from the origin.
fn rect() -> Boundary {
    Boundary::from_corners(&rectangle("e", 0, 0, 4000, 3000), &[]).unwrap()
}

/// A 4 m square, edges `s0` to `s3`.
fn square() -> Boundary {
    Boundary::from_corners(&rectangle("s", 0, 0, 4000, 4000), &[]).unwrap()
}

/// A point at whole millimetres, `height` whole millimetres from the top face.
fn point(name: &str, x: i32, y: i32, height: i64) -> TopPoint {
    TopPoint {
        id: id(name),
        at: Point::mm(x, y),
        height: mm(height),
    }
}

fn split(name: &str, from: &str, to: &str) -> SplitLine {
    SplitLine {
        id: id(name),
        from: id(from),
        to: id(to),
    }
}

fn edits(points: Vec<TopPoint>, splits: Vec<SplitLine>) -> Edits {
    Edits::new(points, splits).unwrap()
}

/// The 50 mm screed over 150 mm of concrete, its screed the variable layer.
fn screed_to_falls() -> Layers {
    slab().with_variable(Some(0)).unwrap()
}

/// The waste: the 4 m by 3 m floor with one point in its middle 20 mm down.
fn waste() -> Shape {
    Shape::flat(rect())
        .with_edits(Some(edits(vec![point("w", 2000, 1500, -20)], vec![])))
        .unwrap()
}

/// The height at a plan point at whole millimetres, in steps, when it is whole.
fn height(top: &Top, x: i32, y: i32) -> i128 {
    let (num, den) = top.height_at(Point::mm(x, y)).unwrap();
    assert_eq!(num % den, 0, "the height at ({x}, {y}) is whole");
    num / den
}

/// A floor 4 m by 3 m lowered 20 mm at a waste in its middle: four triangles round the waste,
/// each through two corners at the top face and the waste 20 mm down. Half way from the waste to a
/// corner, or to the middle of an edge, the top is 10 mm down; at the corners it is at 0.
#[test]
fn a_waste_lowered_20_mm_makes_four_falls() {
    let shape = waste();
    let top = shape.top().unwrap();
    assert_eq!(top.faces().len(), 4);
    for face in top.faces() {
        assert!(face.twice_area() > 0, "every face anticlockwise");
        assert_eq!(
            face.corners.iter().filter(|c| c[2] == -20 * MM).count(),
            1,
            "every face reaches the waste"
        );
    }
    assert_eq!(top.range(), (-20 * MM, 0));
    assert_eq!(height(top, 2000, 1500), i128::from(-20 * MM));
    assert_eq!(height(top, 1000, 750), i128::from(-10 * MM));
    assert_eq!(height(top, 1000, 1500), i128::from(-10 * MM));
    assert_eq!(height(top, 3000, 2250), i128::from(-10 * MM));
    assert_eq!(height(top, 0, 0), 0);
    assert!(top.height_at(Point::mm(5000, 0)).is_none());
    // Each face falls 20 mm over its run to the waste: the face on the edge y = 0 falls 20 mm over
    // 1.5 m along y, so its gradient is (0, -20 / 1500) per step of plan.
    let low = top
        .faces()
        .iter()
        .find(|f| f.corners.iter().all(|c| c[1] <= 1500 * MM))
        .unwrap();
    let ([gx, gy], den) = low.gradient();
    assert_eq!(gx, 0);
    assert_eq!(gy * 1500, -20 * den);
}

/// The waste floor's layers, by hand: the 12 m2 floor's concrete is 12 m2 x 150 mm = 1.8 m3
/// whatever the top does. With the screed variable it fills to the concrete's flat top: 12 m2 x
/// 50 mm = 0.6 m3 less the pyramid the waste takes out, a third of 12 m2 x 20 mm = 0.08 m3, so
/// 0.52 m3; the bottom stays flat 200 mm down. With no variable layer every layer follows the
/// top: the screed is 0.6 m3, and the bottom is 220 mm down at the waste.
#[test]
fn the_waste_floors_volumes_are_worked_by_hand() {
    let shape = waste();
    let library = Materials::default_metric();
    let q = shape_layer_quantities(&shape, &screed_to_falls(), &library).unwrap();
    assert_eq!(
        q.layers[0].volume,
        Volume::from_cubic_steps(520_000_000 * MM3)
    );
    assert_eq!(
        q.layers[1].volume,
        Volume::from_cubic_steps(1_800_000_000 * MM3)
    );
    assert_eq!(q.volume, Volume::from_cubic_steps(2_320_000_000 * MM3));
    let (lowest, highest) = shape.lowest_and_highest(&screed_to_falls()).unwrap();
    assert_eq!((lowest, highest), (mm(-200), mm(0)));

    let plain = shape_layer_quantities(&shape, &slab(), &library).unwrap();
    assert_eq!(
        plain.layers[0].volume,
        Volume::from_cubic_steps(600_000_000 * MM3)
    );
    assert_eq!(plain.volume, Volume::from_cubic_steps(2_400_000_000 * MM3));
    let (lowest, highest) = shape.lowest_and_highest(&slab()).unwrap();
    assert_eq!((lowest, highest), (mm(-220), mm(0)));

    // Six times the rise's integral: twice the 12 m2 times the waste's -20 mm (each face has one
    // corner there), so its sixth is a third of 12 m2 x -20 mm.
    let six = shape.top().unwrap().sixfold_rise().unwrap();
    assert_eq!(six, -80_000_000 * 6 * MM3);
    // The page lists the highest and lowest points of a shape-edited floor, and no top area.
    let rows = shape_read_only(&shape_quantities(&shape, &screed_to_falls()).unwrap()).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, ["floor.highest", "floor.lowest"]);
}

/// A split line holds a crease: a 4 m square with two opposite corners 20 mm down. Its four
/// corners are on one circle, so without a line either diagonal may divide it; with a split line
/// between the two lowered corners the top folds along it into a valley, and at (3 m, 0.5 m) the
/// top is on the face through (0, 0, -20), (4, 0, 0) and (4, 4, -20): -20 + (3 - 0.5) x 5 = -7.5 mm.
#[test]
fn a_split_line_makes_a_valley() {
    let made = edits(
        vec![point("a", 0, 0, -20), point("b", 4000, 4000, -20)],
        vec![split("v", "a", "b")],
    );
    let shape = Shape::flat(square()).with_edits(Some(made)).unwrap();
    let top = shape.top().unwrap();
    assert_eq!(top.faces().len(), 2);
    assert_eq!(height(top, 3000, 500), i128::from(-7 * MM - MM / 2));
    assert_eq!(height(top, 2000, 2000), i128::from(-20 * MM));
    assert_eq!(height(top, 4000, 0), 0);
    // The other diagonal made a ridge instead: -2.5 mm there. Raising the two other corners and
    // splitting between them gives that ridge.
    let ridge = edits(
        vec![
            point("a", 0, 0, -20),
            point("b", 4000, 4000, -20),
            point("c", 4000, 0, 0),
            point("d", 0, 4000, 0),
        ],
        vec![split("r", "c", "d")],
    );
    let shape = Shape::flat(square()).with_edits(Some(ridge)).unwrap();
    assert_eq!(
        height(shape.top().unwrap(), 3000, 500),
        i128::from(-2 * MM - MM / 2)
    );
}

/// Points on an edge divide it and points on a corner set its height; on an L-shaped floor with a
/// hole, the triangles cover the floor exactly, nothing over the hole.
#[test]
fn the_triangles_cover_the_floor_exactly() {
    let floor = common::l_with_hole();
    let made = edits(
        vec![
            point("edge", 4000, 0, -10),
            point("corner", 8000, 4000, 15),
            point("mid", 6000, 2000, -20),
            point("high", 2000, 6000, 5),
        ],
        vec![split("ridge", "edge", "corner")],
    );
    let shape = Shape::flat(floor.clone()).with_edits(Some(made)).unwrap();
    let top = shape.top().unwrap();
    let twice: i128 = top.faces().iter().map(Face::twice_area).sum();
    assert_eq!(twice, 2 * area(&floor).unwrap().square_steps());
    assert_eq!(top.range(), (-20 * MM, 15 * MM));
    assert_eq!(height(top, 8000, 4000), i128::from(15 * MM));
    assert_eq!(height(top, 4000, 0), i128::from(-10 * MM));
    assert_eq!(height(top, 2000, 6000), i128::from(5 * MM));
    // Over the hole there is no top.
    assert!(top.height_at(Point::mm(2000, 2000)).is_none());
    // Each face after the first shares an edge with one before it.
    let faces = top.faces();
    for (k, face) in faces.iter().enumerate().skip(1) {
        let shares = faces[..k].iter().any(|earlier| {
            face.corners
                .iter()
                .filter(|c| earlier.corners.contains(c))
                .count()
                == 2
        });
        assert!(shares, "face {k} shares an edge with one before it");
    }
}

/// The stored form: version 3 with its top, read back to the same shape and the same bytes; the
/// filled recipe holds the faces and marks the variable layer; a reset gives a plain floor's bytes
/// back; and a plain floor of a type with a variable layer is filled as before.
#[test]
fn the_stored_form_and_a_reset() {
    let shape = waste();
    let recipe = shaped_recipe(&shape);
    assert_eq!(recipe.op_version, EDITED_VERSION);
    assert_eq!(
        serde_json::to_string(&recipe.args["top"]).unwrap(),
        r#"{"points":[{"at":[12800000,9600000],"height":"length:q0:-128000","id":"w"}],"splits":[]}"#
    );
    assert_eq!(shape_of(&recipe).unwrap(), shape);
    let full = filled(&recipe, &screed_to_falls()).unwrap();
    assert_eq!(full.args[FACES_ARG].as_array().unwrap().len(), 4);
    assert_eq!(full.args["layers"][0]["variable"], true);
    assert!(full.args["layers"][1].get("variable").is_none());

    let reset = shape.with_edits(None).unwrap();
    assert!(reset.is_plain());
    let plain = shaped_recipe(&reset);
    assert_eq!(plain, floor_recipe(&rect()));
    assert_eq!(plain.op_version, OP_VERSION);
    let plain_filled = filled(&plain, &screed_to_falls()).unwrap();
    assert_eq!(plain_filled, filled(&plain, &slab()).unwrap());
    assert!(!plain_filled.args.contains_key(FACES_ARG));

    // A version 3 recipe without its top, or a version 1 recipe with one, is refused.
    let mut bare = recipe.clone();
    bare.args.remove("top");
    assert!(matches!(shape_of(&bare), Err(SlabError::NotAFloor(_))));
    let mut early = recipe;
    early.op_version = OP_VERSION;
    assert!(matches!(shape_of(&early), Err(SlabError::NotAFloor(_))));
}

/// A variable layer is version 2 of a floor type's text, that layer marked; a type without one is
/// version 1, byte for byte as before; at most one layer, never a membrane, and one that exists.
#[test]
fn a_variable_layer_in_the_types_text() {
    let text = screed_to_falls().to_text().unwrap();
    assert!(text.starts_with(&format!("{{\"version\":{VARIABLE_VERSION},")));
    assert!(text.contains(r#""material":"screed-cement","variable":true}"#));
    assert_eq!(Layers::from_text(&text).unwrap(), screed_to_falls());
    let plain = slab().to_text().unwrap();
    assert!(plain.starts_with(&format!("{{\"version\":{FORMAT_VERSION},")));
    assert!(!plain.contains("variable"));
    // Version 1 with a variable layer, or version 2 without one, is refused.
    let wrong = text.replace("\"version\":2", "\"version\":1");
    assert!(matches!(
        Layers::from_text(&wrong),
        Err(SlabError::Malformed(_))
    ));
    let none = plain.replace("\"version\":1", "\"version\":2");
    assert!(matches!(
        Layers::from_text(&none),
        Err(SlabError::Malformed(_))
    ));
    let membrane = Layers::new(vec![
        layer(Function::Finish, 50, "screed-cement"),
        layer(Function::Membrane, 0, "screed-cement"),
        layer(Function::Structure, 150, "concrete-regular"),
    ])
    .unwrap();
    assert_eq!(
        membrane.clone().with_variable(Some(1)),
        Err(SlabError::VariableLayer {
            layer: 1,
            why: "a membrane keeps its thickness"
        })
    );
    assert!(membrane.with_variable(Some(3)).is_err());
}

/// Every refusal, by name.
#[test]
fn what_is_refused() {
    let refused = |e: Result<Shape, SlabError>| match e {
        Err(SlabError::Edit(error)) => error,
        other => panic!("not an edit's refusal: {other:?}"),
    };
    let on = |points, splits| Shape::flat(rect()).with_edits(Some(edits(points, splits)));
    assert_eq!(
        refused(on(vec![point("p", 5000, 1000, 0)], vec![])),
        EditError::PointOff { point: id("p") }
    );
    let holed = common::l_with_hole();
    assert_eq!(
        refused(
            Shape::flat(holed).with_edits(Some(edits(vec![point("p", 2000, 2000, 0)], vec![])))
        ),
        EditError::PointOff { point: id("p") }
    );
    // Two lines across the middle cross.
    let four = vec![
        point("a", 1000, 500, 0),
        point("b", 3000, 2500, 0),
        point("c", 1000, 2500, 0),
        point("d", 3000, 500, 0),
    ];
    assert_eq!(
        refused(on(four, vec![split("ab", "a", "b"), split("cd", "c", "d")])),
        EditError::SplitsCross {
            first: id("ab"),
            second: id("cd")
        }
    );
    // A line along the edge y = 0, from a corner to a point on it.
    assert_eq!(
        refused(on(
            vec![point("a", 0, 0, 0), point("b", 2000, 0, -5)],
            vec![split("s", "a", "b")]
        )),
        EditError::SplitAlongEdge { split: id("s") }
    );
    // On the L, a line from the inner corner's neighbourhood out across the notch.
    let l = Boundary::from_corners(&common::l_corners(), &[]).unwrap();
    let across = edits(
        vec![point("a", 6000, 3000, 0), point("b", 3000, 6000, 0)],
        vec![split("s", "a", "b")],
    );
    assert_eq!(
        refused(Shape::flat(l).with_edits(Some(across))),
        EditError::SplitOff { split: id("s") }
    );
    // Checked without a boundary.
    let twice = Edits::new(vec![point("p", 1, 1, 0), point("p", 2, 2, 0)], vec![]);
    assert_eq!(
        twice,
        Err(SlabError::Edit(EditError::IdTwice { id: id("p") }))
    );
    let meet = Edits::new(vec![point("p", 1, 1, 0), point("q", 1, 1, 5)], vec![]);
    assert_eq!(
        meet,
        Err(SlabError::Edit(EditError::PointsMeet {
            first: id("p"),
            second: id("q")
        }))
    );
    let far = Edits::new(vec![point("p", 1, 1, -10_001)], vec![]);
    assert_eq!(
        far,
        Err(SlabError::Edit(EditError::PointTooFar { point: id("p") }))
    );
    let nowhere = Edits::new(vec![point("p", 1, 1, 0)], vec![split("s", "p", "q")]);
    assert_eq!(
        nowhere,
        Err(SlabError::Edit(EditError::SplitEnd {
            split: id("s"),
            end: id("q")
        }))
    );
}

/// A slope and shape edits are one or the other, slab edges are not shape-edited yet, and a point
/// may not leave the variable layer no thickness.
#[test]
fn what_is_kept_apart() {
    let refused = |e: Result<Shape, SlabError>| match e {
        Err(SlabError::Edit(error)) => error,
        other => panic!("not an edit's refusal: {other:?}"),
    };
    let arrow = SlopeArrow::new(
        Point::mm(0, 0),
        Point::mm(4000, 0),
        Length::from_mm(0),
        Rise::Heights { at_head: mm(-40) },
    )
    .unwrap();
    let sloped = Shape::new(rect(), Some(arrow), vec![]).unwrap();
    assert_eq!(
        refused(sloped.with_edits(Some(edits(vec![point("w", 2000, 1500, -20)], vec![])))),
        EditError::WithSlope
    );
    let edged = Shape::new(
        rect(),
        None,
        vec![SlabEdge {
            edge: id("e0"),
            profile: EdgeProfile::Downturn {
                width: mm(300),
                depth: mm(400),
            },
        }],
    )
    .unwrap();
    assert_eq!(
        refused(edged.with_edits(Some(edits(vec![point("w", 2000, 1500, -20)], vec![])))),
        EditError::WithEdges
    );
    // A waste 60 mm down leaves a 50 mm variable screed no thickness there.
    let deep = Shape::flat(rect())
        .with_edits(Some(edits(vec![point("w", 2000, 1500, -60)], vec![])))
        .unwrap();
    assert_eq!(
        filled(&shaped_recipe(&deep), &screed_to_falls()),
        Err(SlabError::Edit(EditError::VariableTooThin {
            point: id("w"),
            layer: 0
        }))
    );
    // With every layer following the top, it is built.
    assert!(filled(&shaped_recipe(&deep), &slab()).is_ok());
}
