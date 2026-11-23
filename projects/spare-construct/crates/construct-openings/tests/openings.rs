//! Openings and shafts as data: their records and recipes read back, their outlines checked, where
//! they cut a floor, whether a shaft passes a floor, and what a layer holds after the cut, with
//! exact numbers. Every refusal is checked by its code and its words.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_levels::{Tie, base_of, top_of};
use construct_openings::{
    Cutter, End, Named, OpeningError, Outline, Reach, Shaft, WallOpening, fit_in_floor,
    floor_layer, floor_opening_record, is_void, outline_of, reach, shaft_record, top_arg,
    wall_layer, wall_opening_record,
};
use construct_params::{Area, Length, Value as Stored, Volume};
use kit_core::Id;
use serde_json::json;

const MM: i64 = 6400;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<[i64; 2]> {
    [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
        .into_iter()
        .map(|c| c.map(|v| v * MM))
        .collect()
}

fn outline(corners: &[[i64; 2]]) -> Outline {
    Outline::from_mm(&Named::opening("O-1"), corners).unwrap()
}

/// A wall opening's recipe holds its width and height as stored lengths and its wall's axis, and
/// reads back as it was written; a recipe of anything else, or not of the stored form, is refused
/// naming the opening.
#[test]
fn a_wall_opening_reads_back_as_written() {
    let opening = WallOpening::new(&Named::opening("O-1"), mm(900), mm(2100), true).unwrap();
    let recipe = opening.recipe();
    assert_eq!(recipe.op, "opening.wall");
    assert_eq!(recipe.args["width"], json!("length:q0:5760000"));
    assert_eq!(recipe.args["height"], json!("length:q0:13440000"));
    assert_eq!(recipe.args["axis"], json!("y"));
    assert_eq!(WallOpening::of("O-1", &recipe).unwrap(), opening);
    let record = wall_opening_record(&i("O-1"), &i("doc"), "Hatch", &opening);
    assert_eq!(record.kind, "opening");
    assert_eq!(record.props["@category"], "1600");
    assert!(is_void(&record.recipe));

    let flat = WallOpening::new(
        &Named::opening("O-1"),
        mm(900),
        Length::from_steps(0),
        false,
    )
    .unwrap_err();
    assert_eq!(flat.code(), "EmptyShape");
    assert_eq!(flat.to_string(), "opening O-1: its height must be above 0");
    let mut odd = recipe.clone();
    odd.args.insert("width".to_owned(), json!(900));
    let malformed = WallOpening::of("O-1", &odd).unwrap_err();
    assert_eq!(
        malformed.to_string(),
        "opening O-1: its width is not a stored length"
    );
    let mut other = recipe;
    other.op = "door.single".to_owned();
    assert_eq!(
        WallOpening::of("O-1", &other).unwrap_err().code(),
        "NotAnOpening"
    );
}

/// An outline is a closed loop of straight edges: fewer than three corners, two corners in one
/// place, edges that cross, or a curved edge is refused naming the opening, in words.
#[test]
fn an_outline_is_one_simple_loop_of_straight_edges() {
    let o = Named::opening("O-1");
    let square = Outline::from_mm(&o, &[[0, 0], [1000, 0], [1000, 1000], [0, 1000]]).unwrap();
    assert_eq!(square.area(&o).unwrap(), Area::from_mm2(1_000_000));
    assert_eq!(Outline::from_value(&o, &square.to_value()).unwrap(), square);
    assert_eq!(
        square.to_value(),
        json!([
            [0, 0],
            [6_400_000, 0],
            [6_400_000, 6_400_000],
            [0, 6_400_000]
        ])
    );

    let two = Outline::from_mm(&o, &[[0, 0], [1000, 0]]).unwrap_err();
    assert_eq!(two.code(), "BadOutline");
    assert_eq!(
        two.to_string(),
        "opening O-1: its outline has 2 corners; an outline needs at least three"
    );
    let together = Outline::from_mm(&o, &[[0, 0], [1000, 0], [1000, 0], [0, 1000]]).unwrap_err();
    assert_eq!(
        together,
        OpeningError::CornersTogether { element: o.clone() }
    );
    let bow = Outline::from_mm(&o, &[[0, 0], [1000, 1000], [1000, 0], [0, 1000]]).unwrap_err();
    assert_eq!(bow.code(), "OutlineCrossesItself");
    assert_eq!(
        bow.to_string(),
        "opening O-1: its outline crosses or touches itself; draw it as one loop that does not"
    );
    let curved = Outline::from_value(
        &Named::shaft("S-1"),
        &json!([[0, 0], {"arc": [1, 1]}, [0, 6400]]),
    )
    .unwrap_err();
    assert_eq!(curved.code(), "CurvedOutline");
    assert_eq!(
        curved.to_string(),
        "shaft S-1: its outline has a curved edge, and a curved opening cannot be cut exactly \
         yet: draw it with straight edges"
    );
    let fraction = Outline::from_value(&o, &json!([[0, 0], [1.5, 0], [0, 1]])).unwrap_err();
    assert_eq!(fraction.code(), "BadOpening");
}

/// A shaft's record ties its base and its top to levels and reads its height from the top tie;
/// filled, it reads as its outline and its height, and a top below or at its base is refused.
#[test]
fn a_shaft_reads_its_height_from_its_ties() {
    let square = outline(&[[0, 0], [1200, 0], [1200, 1500], [0, 1500]]);
    let base = Tie {
        level: i("L1"),
        offset: Length::from_steps(0),
    };
    let top = Tie {
        level: i("L3"),
        offset: mm(-100),
    };
    let record = shaft_record(&i("S-1"), &i("doc"), "Lift", &square, &base, &top);
    assert_eq!(record.kind, "shaft");
    assert_eq!(record.recipe.args["height"], top_arg());
    assert_eq!(base_of(&record).unwrap(), Some(base));
    assert_eq!(top_of(&record).unwrap(), Some(top));
    assert_eq!(outline_of("S-1", &record.recipe).unwrap(), square);

    let filled = |height: Length| {
        let mut recipe = record.recipe.clone();
        recipe
            .args
            .insert("height".to_owned(), json!(Stored::Length(height).stored()));
        Shaft::of("S-1", &recipe)
    };
    let shaft = filled(mm(6300)).unwrap();
    assert_eq!(shaft.height, mm(6300));
    assert_eq!(shaft.outline, square);
    let below = filled(mm(-10)).unwrap_err();
    assert_eq!(below.code(), "ShaftTopBelowBase");
    assert_eq!(
        below.to_string(),
        "shaft S-1: its top is below its base; tie its top to a level above its base"
    );
    assert_eq!(filled(mm(0)).unwrap_err().code(), "ShaftNoHeight");
    assert_eq!(
        Shaft::of("S-1", &record.recipe).unwrap_err().code(),
        "BadOpening"
    );
    let opening = floor_opening_record(&i("O-1"), &i("doc"), "Hatch", &square);
    assert_eq!(opening.kind, "opening");
    assert_eq!(outline_of("O-1", &opening.recipe).unwrap(), square);
}

fn cut(element: Named, corners: Vec<[i64; 2]>) -> Cutter {
    Cutter { element, corners }
}

/// A floor of 10 m by 6 m with a 1 m square hole of its own: an opening inside it cuts it, and a
/// shaft over it does; a shaft beside it passes it by. Each case the floor cannot take is refused
/// naming the element and the floor.
#[test]
fn openings_and_shafts_fit_inside_their_floor_or_are_refused() {
    let outer = rect(0, 0, 10_000, 6000);
    let holes = vec![rect(8000, 4000, 9000, 5000)];
    let fit = |cutters: &[Cutter]| fit_in_floor("floor F-1", &outer, &holes, cutters);
    let (o1, o2, s1) = (
        Named::opening("O-1"),
        Named::opening("O-2"),
        Named::shaft("S-1"),
    );
    let inside = cut(o1.clone(), rect(1000, 1000, 3000, 2500));
    let shaft = cut(s1.clone(), rect(5000, 1000, 6200, 2500));
    let beside = cut(Named::shaft("S-0"), rect(12_000, 0, 13_000, 1000));
    assert_eq!(
        fit(&[shaft.clone(), inside.clone(), beside]).unwrap(),
        [1, 0]
    );

    let outside = fit(&[cut(o2.clone(), rect(12_000, 0, 13_000, 1000))]).unwrap_err();
    assert_eq!(outside.code(), "OpeningOutsideHost");
    assert_eq!(
        outside.to_string(),
        "opening O-2 lies outside floor F-1: an opening is cut wholly inside the floor it is in"
    );
    let across = fit(&[cut(s1.clone(), rect(9500, 1000, 10_500, 2000))]).unwrap_err();
    assert_eq!(
        across,
        OpeningError::AcrossEdge {
            element: s1.clone(),
            host: "floor F-1".to_owned()
        }
    );
    assert_eq!(
        across.to_string(),
        "shaft S-1 crosses or touches the edge of floor F-1: it must lie wholly inside the \
         floor, clear of its edge"
    );
    let touching = fit(&[cut(o2.clone(), rect(0, 1000, 500, 2000))]).unwrap_err();
    assert_eq!(touching.code(), "OpeningAcrossEdge");
    let overlap = fit(&[
        inside.clone(),
        cut(o2.clone(), rect(2500, 2000, 4000, 4000)),
    ])
    .unwrap_err();
    assert_eq!(overlap.code(), "OpeningsOverlap");
    assert_eq!(
        overlap.to_string(),
        "opening O-1 and opening O-2 overlap or touch; move one clear of the other"
    );
    let meeting = fit(&[
        inside.clone(),
        cut(s1.clone(), rect(3000, 1000, 4000, 2000)),
    ])
    .unwrap_err();
    assert_eq!(
        meeting,
        OpeningError::Overlap {
            element: o1.clone(),
            other: s1.clone()
        }
    );
    let over = fit(&[cut(o2, rect(8500, 4500, 9500, 5500))]).unwrap_err();
    assert_eq!(over.code(), "OpeningOverHole");
    let round = fit(&[cut(s1.clone(), rect(-1000, -1000, 11_000, 7000))]).unwrap_err();
    assert_eq!(round.code(), "ShaftTakesFloor");
    assert_eq!(
        round.to_string(),
        "shaft S-1 takes the whole of floor F-1: a shaft cuts a hole in a floor, not the floor away"
    );
    let twice = fit(&[inside.clone(), inside]).unwrap_err();
    assert_eq!(twice.code(), "DuplicateOpening");
}

/// A shaft cuts a floor only when it passes wholly through its thickness; one that only touches a
/// face misses it, and one that ends inside it is named by the end.
#[test]
fn a_shaft_passes_through_a_floor_or_misses_it() {
    let (bottom, top) = (mm(2850), mm(3000));
    let r = |base: i64, up: i64| reach(mm(base), mm(up), bottom, top);
    assert_eq!(r(0, 9000), Reach::Through);
    assert_eq!(r(2850, 3000), Reach::Through);
    assert_eq!(r(0, 2850), Reach::Misses);
    assert_eq!(r(3000, 6000), Reach::Misses);
    assert_eq!(r(4000, 6000), Reach::Misses);
    assert_eq!(r(2900, 6000), Reach::EndsInside(End::Base));
    assert_eq!(r(0, 2900), Reach::EndsInside(End::Top));
    let ends = OpeningError::EndsInFloor {
        element: Named::shaft("S-1"),
        floor: "floor F-2".to_owned(),
        end: End::Top,
    };
    assert_eq!(ends.code(), "ShaftEndsInFloor");
    assert_eq!(
        ends.to_string(),
        "shaft S-1: its top lies inside the thickness of floor F-2; a shaft passes wholly \
         through a floor or misses it, so move its top to the floor's top or bottom face or \
         past it"
    );
}

/// What a layer holds after the cut, exactly: a wall layer 4000 by 2700 by 100 mm less a door's
/// 900 by 2100 and a window's 1200 by 1000; a floor layer 10 m by 6 m and 150 mm thick less its
/// 1 m square hole and a 2 m by 1.5 m opening.
#[test]
fn a_layer_holds_its_shape_less_its_openings_exactly() {
    let wall = wall_layer(
        mm(4000),
        mm(2700),
        mm(100),
        &[(mm(900), mm(2100)), (mm(1200), mm(1000))],
    )
    .unwrap();
    assert_eq!(wall.face, Area::from_mm2(7_710_000));
    assert_eq!(wall.volume, Volume::from_mm3(771_000_000));
    let floor = floor_layer(
        &rect(0, 0, 10_000, 6000),
        &[rect(8000, 4000, 9000, 5000)],
        &[rect(1000, 1000, 3000, 2500)],
        mm(150),
    )
    .unwrap();
    assert_eq!(floor.face, Area::from_mm2(56_000_000));
    assert_eq!(floor.volume, Volume::from_mm3(8_400_000_000));
    let odd = floor_layer(&[[0, 0], [1, 0], [0, 1]], &[], &[], mm(1));
    assert_eq!(odd, None);
}
