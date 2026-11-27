//! A sloped floor (job 5.2): its top face's height at every corner, exactly on the grid; its plan
//! area and volume as a flat floor's (its layers are plumb); its sloped top area, exact where
//! the root is whole and the nearest square step where it is not; its highest and lowest
//! points; its stored form; and every refusal by name.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{id, mm, rectangle, slab};
use construct_materials::Materials;
use construct_params::{Area, Length, Slope, Volume};
use construct_slabs::{
    Boundary, EdgeProfile, Ring, Rise, SHAPED_VERSION, Shape, SlabEdge, SlabError, SlopeArrow,
    TOPS_ARG, filled, floor_recipe, quantities, shape_of, shape_quantities, shape_read_only,
    shaped_recipe,
};
use construct_solver::Point;
use serde_json::json;

/// The 4 m by 3 m rectangle, edges `e0` to `e3` anticlockwise from the origin.
fn rect() -> Boundary {
    Boundary::from_corners(&rectangle("e", 0, 0, 4000, 3000), &[]).unwrap()
}

/// An arrow along x from the origin to (4 m, 0), the top 0 at its tail.
fn along_x(rise: Rise) -> SlopeArrow {
    SlopeArrow::new(
        Point::mm(0, 0),
        Point::mm(4000, 0),
        Length::from_mm(0),
        rise,
    )
    .unwrap()
}

/// A floor falling 40 mm over 4 m towards x: its corners stand at 0 and -40 mm, exactly; its
/// plan area and volume are a flat floor's (12 m2 by 200 mm); its highest point is 0 and its
/// lowest 240 mm down; its top area is the nearest square step to 12 m2 times the root of
/// 1.0001, not exact.
#[test]
fn a_floor_falling_to_one_side() {
    let fall = along_x(Rise::Heights {
        at_head: Length::from_mm(-40),
    });
    let shape = Shape::new(rect(), Some(fall), Vec::new()).unwrap();
    let down = Length::from_mm(-40).steps();
    assert_eq!(shape.tops(), [vec![0, down, down, 0]]);
    let layers = slab();
    let flat = quantities(shape.boundary(), &layers, &Materials::default_metric()).unwrap();
    assert_eq!(flat.area, Area::from_mm2(12_000_000));
    assert_eq!(flat.volume, Volume::from_mm3(12_000_000 * 200));
    let q = shape_quantities(&shape, &layers).unwrap();
    assert_eq!(q.highest, Length::from_mm(0));
    assert_eq!(q.lowest, Length::from_mm(-240));
    assert!(!q.top_area.exact);
    let plan = flat.area.square_steps();
    let top = q.top_area.area.square_steps();
    // sqrt(1.0001) = 1.0000499987500625...: the nearest whole number to the plan area times it.
    let expected = (plan as f64) * 1.000_049_998_750_062_5;
    assert!(
        (top as f64 - expected).abs() <= 1.0,
        "{top} against {expected}"
    );
    // The same fall given as 1 in 100 along the same arrow is the same floor.
    let as_gradient = along_x(Rise::Gradient {
        slope: Slope::new(-1, 100).unwrap(),
    });
    let again = Shape::new(rect(), Some(as_gradient), Vec::new()).unwrap();
    assert_eq!(again.tops(), shape.tops());
    assert_eq!(
        shape_quantities(&again, &layers).unwrap().top_area,
        q.top_area
    );
    // Its read-only rows: top area, highest, lowest.
    let rows = shape_read_only(&q).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, ["floor.top-area", "floor.highest", "floor.lowest"]);
}

/// Rising 3 m over 4 m, the root is whole: the top face is 5/4 of the plan, 15 m2 exactly; the
/// highest point is the far side's 3 m.
#[test]
fn a_slope_of_three_in_four_has_an_exact_top_area() {
    for rise in [
        Rise::Heights {
            at_head: Length::from_mm(3000),
        },
        Rise::Gradient {
            slope: Slope::new(3, 4).unwrap(),
        },
    ] {
        let shape = Shape::new(rect(), Some(along_x(rise)), Vec::new()).unwrap();
        let q = shape_quantities(&shape, &slab()).unwrap();
        assert!(q.top_area.exact);
        assert_eq!(q.top_area.area, Area::from_mm2(15_000_000));
        assert_eq!(q.highest, Length::from_mm(3000));
        assert_eq!(q.lowest, Length::from_mm(-200));
    }
}

/// A diagonal arrow with heights at both ends: every corner's height is the ratio worked
/// exactly, holes included; the highest and lowest are the outer loop's corners.
#[test]
fn a_diagonal_slope_lands_every_corner_on_the_grid() {
    let floor = common::l_with_hole();
    let arrow = SlopeArrow::new(
        Point::mm(0, 0),
        Point::mm(8000, 8000),
        Length::from_mm(100),
        Rise::Heights {
            at_head: Length::from_mm(-60),
        },
    )
    .unwrap();
    let shape = Shape::new(floor, Some(arrow), Vec::new()).unwrap();
    // z = 100 - 160 (x + y) / 16000 mm.
    let z = |x: i64, y: i64| {
        Length::from_mm(0).steps() + mm(100).steps() - 6400 * 160 * (x + y) / 16000
    };
    let outer: Vec<i64> = [
        (0, 0),
        (8000, 0),
        (8000, 4000),
        (4000, 4000),
        (4000, 8000),
        (0, 8000),
    ]
    .iter()
    .map(|&(x, y)| z(x, y))
    .collect();
    assert_eq!(shape.tops()[0], outer);
    assert_eq!(shape.tops().len(), 2);
    let q = shape_quantities(&shape, &slab()).unwrap();
    assert_eq!(q.highest, Length::from_mm(100));
    assert_eq!(q.lowest, Length::from_mm(100 - 120 - 200));
}

/// A slope is refused by name: an arrow of no direction; a gradient along an arrow whose length
/// is a root; a corner whose height falls between steps; a slope with slab edges.
#[test]
fn what_a_slope_cannot_be_is_refused() {
    let none = SlopeArrow::new(
        Point::mm(10, 10),
        Point::mm(10, 10),
        Length::from_mm(0),
        Rise::Heights {
            at_head: Length::from_mm(5),
        },
    );
    assert_eq!(none, Err(SlabError::SlopeNoDirection));
    let diagonal = SlopeArrow::new(
        Point::mm(0, 0),
        Point::mm(1, 1),
        Length::from_mm(0),
        Rise::Gradient {
            slope: Slope::new(1, 50).unwrap(),
        },
    );
    assert_eq!(diagonal, Err(SlabError::SlopeArrowNotWhole));
    // 1 mm over a 3 mm arrow puts (4 m, 0) at 4000/3 mm.
    let thirds = SlopeArrow::new(
        Point::mm(0, 0),
        Point::mm(3, 0),
        Length::from_mm(0),
        Rise::Heights {
            at_head: Length::from_mm(1),
        },
    )
    .unwrap();
    assert_eq!(
        Shape::new(rect(), Some(thirds), Vec::new()),
        Err(SlabError::SlopeOffGrid {
            ring: Ring::Outer,
            segment: id("e1"),
        })
    );
    let edge = SlabEdge {
        edge: id("e0"),
        profile: EdgeProfile::Downturn {
            width: mm(300),
            depth: mm(400),
        },
    };
    let fall = along_x(Rise::Heights {
        at_head: Length::from_mm(-40),
    });
    assert_eq!(
        Shape::new(rect(), Some(fall), vec![edge]),
        Err(SlabError::SlopeWithEdges)
    );
    assert!(
        SlabError::SlopeOffGrid {
            ring: Ring::Outer,
            segment: id("e1")
        }
        .to_string()
        .contains("edge e1 of the outer loop")
    );
}

/// A sloped floor's recipe is version 2 with its stored arrow, reading back the same floor; a
/// flat floor's stays version 1 byte for byte; the builder's recipe holds each corner's top;
/// a version 1 recipe with a slope, a version 2 with neither, and a slope not in its one
/// spelling are refused; a new boundary keeps the slope, checked again.
#[test]
fn a_sloped_floor_is_stored_and_filled() {
    let fall = along_x(Rise::Heights {
        at_head: Length::from_mm(-40),
    });
    let shape = Shape::new(rect(), Some(fall), Vec::new()).unwrap();
    let recipe = shaped_recipe(&shape);
    assert_eq!(recipe.op_version, SHAPED_VERSION);
    assert_eq!(
        recipe.args["slope"],
        json!({
            "at_head": "length:q0:-256000",
            "at_tail": "length:q0:0",
            "head": [25_600_000, 0],
            "tail": [0, 0],
        })
    );
    assert_eq!(shape_of(&recipe).unwrap(), shape);
    assert_eq!(shaped_recipe(&Shape::flat(rect())), floor_recipe(&rect()));
    let gradient = along_x(Rise::Gradient {
        slope: Slope::new(-1, 100).unwrap(),
    });
    let stored = gradient.to_value();
    assert_eq!(stored["slope"], json!("slope:-1/100"));
    assert_eq!(SlopeArrow::from_value(&stored).unwrap(), gradient);

    let built = filled(&recipe, &slab()).unwrap();
    assert_eq!(built.args[TOPS_ARG], json!([[0, -256_000, -256_000, 0]]));
    assert_eq!(built.op_version, SHAPED_VERSION);

    let mut old = recipe.clone();
    old.op_version = 1;
    assert!(matches!(shape_of(&old), Err(SlabError::NotAFloor(_))));
    let mut bare = floor_recipe(&rect());
    bare.op_version = SHAPED_VERSION;
    assert!(matches!(shape_of(&bare), Err(SlabError::NotAFloor(_))));
    let mut extra = recipe.clone();
    extra.args.get_mut("slope").unwrap()["x"] = json!(1);
    assert!(matches!(shape_of(&extra), Err(SlabError::Malformed(_))));

    let record = construct_slabs::floor_record(
        &id("F-1"),
        &id("DOC-1"),
        "Deck",
        &id("floor-slab"),
        &construct_levels::Tie {
            level: id("L-1"),
            offset: Length::from_mm(0),
        },
        &rect(),
    );
    let sloped = construct_slabs::with_slope(record, Some(fall)).unwrap();
    let wider = Boundary::from_corners(&rectangle("e", 0, 0, 8000, 3000), &[]).unwrap();
    let moved = construct_slabs::with_boundary(sloped.clone(), &wider).unwrap();
    let kept = shape_of(&moved.recipe).unwrap();
    assert_eq!(kept.slope(), Some(&fall));
    assert_eq!(kept.tops()[0][1], Length::from_mm(-80).steps());
    let flat_again = construct_slabs::with_slope(sloped, None).unwrap();
    assert_eq!(flat_again.recipe, floor_recipe(&rect()));
}
