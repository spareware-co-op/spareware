//! A family seated in a host that runs at any angle (piece A83, CON-A4-F3-P1): the library's door
//! and windows seated along walls drawn at 30, 45 and 60 degrees and at a 3:4:5 slope, every
//! extrusion a level profile swept straight up, each of its corners the nearest grid step to where
//! the exact turn puts it (checked on whole numbers, by squaring); an upright profile made of its
//! box less its cutters, each reaching 10 mm past where it opens onto the outside; a level profile
//! seated corner for corner as it is; a sloped upright profile refused at an angle but built turned
//! by a quarter turn; and each seat kept apart in the cache. Judged through this crate's types and
//! the `Engine` trait only.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod evaluate_suite;

use construct_families::{
    CacheKey, Evaluator, Key, Limits, Reason, Recipe, Seat, Turn, TypeValues, evaluate_seated,
};
use engine_geometry::value::engine::{
    Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Volume,
};
use engine_geometry::value::grid::GridStep;
use serde_json::json;

const DOOR: &str = include_str!("../../../library/doors-windows/single-swing-door.json");
const FIXED: &str = include_str!("../../../library/doors-windows/fixed-window.json");
const SLIDING: &str = include_str!("../../../library/doors-windows/sliding-window.json");

/// How far a cutter reaches past the solid it cuts: 10 mm.
const PAST: i64 = 64_000;

fn read(text: &str) -> Recipe {
    Recipe::from_json(text).unwrap()
}

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

/// What a test engine was asked for a solid: an extrusion, or a subtraction of two.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asked {
    Extrude {
        corners: Vec<[i64; 3]>,
        holes: Vec<Vec<[i64; 3]>>,
        by: [i64; 3],
    },
    Subtract(Box<Asked>, Box<Asked>),
}

/// A test engine that keeps what it is asked, and answers nothing else.
struct Asks;

fn unanswered(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything but extrusions and subtractions".to_owned(),
    }
}

impl Engine for Asks {
    type Solid = Asked;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.asks".to_owned(),
            version: "this test file".to_owned(),
            step: GridStep::Q0,
            tolerances: Vec::new(),
            answers: Vec::new(),
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Asked, EngineError> {
        let steps = |ring: &[engine_geometry::value::grid::P3]| {
            ring.iter()
                .map(|p| [p.x.get(), p.y.get(), p.z.get()])
                .collect::<Vec<_>>()
        };
        Ok(Asked::Extrude {
            corners: steps(&profile.outer),
            holes: profile.holes.iter().map(|h| steps(h)).collect(),
            by,
        })
    }

    fn subtract(&mut self, target: &Asked, tool: &Asked) -> Result<Asked, EngineError> {
        Ok(Asked::Subtract(
            Box::new(target.clone()),
            Box::new(tool.clone()),
        ))
    }

    fn subtract_pieces(&mut self, _: &Asked, _: &Asked) -> Result<Vec<Asked>, EngineError> {
        Err(unanswered("subtract_pieces"))
    }

    fn loft(&mut self, _: &[Section]) -> Result<Asked, EngineError> {
        Err(unanswered("loft"))
    }

    fn unite(&mut self, _: &Asked, _: &Asked) -> Result<Asked, EngineError> {
        Err(unanswered("unite"))
    }

    fn volume(&mut self, _: &Asked) -> Result<Volume, EngineError> {
        Err(unanswered("volume"))
    }

    fn mesh(&mut self, _: &Asked) -> Result<DisplayMesh, EngineError> {
        Err(unanswered("mesh"))
    }

    fn section(&mut self, _: &Asked, _: &GridPlane) -> Result<Cut, EngineError> {
        Err(unanswered("section"))
    }
}

/// The ways of walls drawn 4 m long from their start, in whole millimetres, as grid steps: at 30,
/// 45 and 60 degrees (each end the nearest millimetre to it) and at a 3:4:5 slope.
const WAYS: [(&str, [i64; 2]); 4] = [
    ("30 degrees", [3464 * 6400, 2000 * 6400]),
    ("45 degrees", [2828 * 6400, 2828 * 6400]),
    ("60 degrees", [2000 * 6400, 3464 * 6400]),
    ("3:4:5", [2400 * 6400, 3200 * 6400]),
];

/// Whether `m` is the nearest whole number to `n / sqrt(dd)`, a half away from zero, asked of
/// whole numbers by squaring: `(2|m| - 1)^2 dd <= (2n)^2 < (2|m| + 1)^2 dd`, `m` signed as `n`.
fn is_nearest(m: i64, n: i128, dd: i128) -> bool {
    let (m, n2) = (i128::from(m), (2 * n).pow(2));
    let signed = m == 0 || (m > 0) == (n > 0);
    let low = if m == 0 {
        0
    } else {
        (2 * m.abs() - 1).pow(2) * dd
    };
    signed && low <= n2 && n2 < (2 * m.abs() + 1).pow(2) * dd
}

/// The point `[x, y]` of the family's frame seated by `seat` lands at `got`: each coordinate the
/// nearest step to the exact turn's.
fn lands(seat: &Seat, [x, y]: [i64; 2], got: [i64; 2]) -> bool {
    let [wx, wy] = seat.way().map(i128::from);
    let [bx, by] = seat.base();
    let (along, left) = (i128::from(seat.at()[0] + x), i128::from(seat.at()[1] + y));
    let dd = wx * wx + wy * wy;
    is_nearest(got[0] - bx, along * wx - left * wy, dd)
        && is_nearest(got[1] - by, along * wy + left * wx, dd)
}

/// The extrusions and subtractions of `asked`, in the order asked: the box first, then each tool.
fn boxes(asked: &Asked) -> Vec<(Vec<[i64; 3]>, [i64; 3])> {
    match asked {
        Asked::Extrude { corners, holes, by } => {
            assert!(holes.is_empty(), "an upright box has no hole");
            vec![(corners.clone(), *by)]
        }
        Asked::Subtract(target, tool) => {
            let mut all = boxes(target);
            all.extend(boxes(tool));
            all
        }
    }
}

/// An upright profile's solid seated at an angle off the quarter turns is its box less its
/// cutters, each a level rectangle swept straight up, every corner the nearest step to the exact
/// turn of its point in the family's frame: the box around the profile through its depth, then a
/// cutter for what the profile leaves out of its box (a door frame's notch, reaching 10 mm under
/// its foot) and one for each hole (a window frame's light, between its faces), each reaching
/// 10 mm past both faces. Every template, every type, on every way; a 3:4:5 way lands exactly.
#[test]
fn an_upright_profile_seated_at_an_angle_is_its_box_less_its_cutters() {
    for text in [DOOR, FIXED, SLIDING] {
        let recipe = read(text);
        for ty in recipe.named_types() {
            let values = TypeValues::of(&recipe, ty.key()).unwrap();
            let plain = construct_families::evaluate(&mut Asks, &recipe, &values).unwrap();
            for (name, way) in WAYS {
                let seat = Seat::new(way, [123, -456], [7_000, -3_000]).unwrap();
                let seated =
                    evaluate_seated(&mut Asks, &recipe, &values, Limits::DEFAULT, &seat).unwrap();
                assert_eq!(seated.solids.len(), plain.solids.len());
                for (flat, turned) in plain.solids.iter().zip(&seated.solids) {
                    let Asked::Extrude { corners, holes, by } = &flat.solid else {
                        panic!("{}: a template's solid is an extrusion", flat.step);
                    };
                    // Every template's solid is on its centre plane: x and z, swept along y.
                    assert_eq!((by[0], by[2]), (0, 0));
                    let level = corners[0][1];
                    let (lo, hi) = (level.min(level + by[1]), level.max(level + by[1]));
                    let xs = corners.iter().map(|c| c[0]);
                    let zs = corners.iter().map(|c| c[2]);
                    let (x0, x1) = (xs.clone().min().unwrap(), xs.max().unwrap());
                    let (z0, z1) = (zs.clone().min().unwrap(), zs.max().unwrap());
                    let mut wanted = vec![([x0, x1, z0, z1], (lo, hi))];
                    let notch = corners.len() > 4;
                    if notch {
                        // The door's frame: its notch from its foot to its head, between its
                        // jambs, past the foot.
                        let inner: Vec<i64> = corners
                            .iter()
                            .map(|c| c[0])
                            .filter(|&x| x != x0 && x != x1)
                            .collect();
                        let head = corners
                            .iter()
                            .map(|c| c[2])
                            .filter(|&z| z != z0 && z != z1)
                            .max()
                            .unwrap();
                        let (a, b) = (*inner.iter().min().unwrap(), *inner.iter().max().unwrap());
                        wanted.push(([a, b, z0 - PAST, head], (lo - PAST, hi + PAST)));
                    }
                    for hole in holes {
                        let xs = hole.iter().map(|c| c[0]);
                        let zs = hole.iter().map(|c| c[2]);
                        wanted.push((
                            [
                                xs.clone().min().unwrap(),
                                xs.max().unwrap(),
                                zs.clone().min().unwrap(),
                                zs.max().unwrap(),
                            ],
                            (lo - PAST, hi + PAST),
                        ));
                    }
                    let asked = boxes(&turned.solid);
                    assert_eq!(
                        asked.len(),
                        wanted.len(),
                        "{name} {} {}",
                        ty.key(),
                        flat.step
                    );
                    for ((corners, up), ([a, b, v0, v1], (w0, w1))) in asked.iter().zip(&wanted) {
                        assert_eq!(*up, [0, 0, v1 - v0], "{name} {}", flat.step);
                        let frame = [[*a, *w0], [*b, *w0], [*b, *w1], [*a, *w1]];
                        for (got, point) in corners.iter().zip(frame) {
                            assert_eq!(got[2], *v0);
                            assert!(
                                lands(&seat, point, [got[0], got[1]]),
                                "{name} {} {}: {point:?} at {got:?}",
                                ty.key(),
                                flat.step
                            );
                            if name == "3:4:5" {
                                // A point on a multiple of five steps lands exactly.
                                let [x, y] = [point[0] + 7_000, point[1] - 3_000];
                                let [wx, wy] = way;
                                let length = 4000 * 6400;
                                assert_eq!(got[0] - 123, (x * wx - y * wy) / length);
                                assert_eq!((x * wx - y * wy) % length, 0);
                                assert_eq!((x * wy + y * wx) % length, 0);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A level profile seated at an angle is put through the seat corner for corner, holes and all,
/// and swept straight up as it is: one extrusion.
#[test]
fn a_level_profile_seated_at_an_angle_is_seated_corner_for_corner() {
    let recipe = evaluate_suite::block(&json!([evaluate_suite::extrude(
        "body",
        &evaluate_suite::BLOCK,
        &[&evaluate_suite::SQUARE]
    )]));
    let values = TypeValues::new();
    let plain = construct_families::evaluate(&mut Asks, &recipe, &values).unwrap();
    let seat = Seat::new(WAYS[0].1, [0, 0], [0, 0]).unwrap();
    let seated = evaluate_seated(&mut Asks, &recipe, &values, Limits::DEFAULT, &seat).unwrap();
    let (
        Asked::Extrude { corners, holes, by },
        Asked::Extrude {
            corners: put,
            holes: put_holes,
            by: put_by,
        },
    ) = (&plain.solids[0].solid, &seated.solids[0].solid)
    else {
        panic!("a level profile is one extrusion");
    };
    assert_eq!(put_by, by);
    assert_eq!(put.len(), corners.len());
    assert_eq!(put_holes.len(), holes.len());
    for (a, b) in corners
        .iter()
        .chain(holes.iter().flatten())
        .zip(put.iter().chain(put_holes.iter().flatten()))
    {
        assert_eq!(a[2], b[2]);
        assert!(lands(&seat, [a[0], a[1]], [b[0], b[1]]), "{a:?} at {b:?}");
    }
}

/// An upright profile with a sloped edge is refused seated at an angle off the quarter turns, in
/// words, and built as it always was turned by a quarter turn, or seated along an axis.
#[test]
fn a_sloped_upright_profile_is_refused_at_an_angle_and_built_square() {
    let mut text: serde_json::Value = serde_json::from_str(FIXED).unwrap();
    // Lift the frame's outer top-left corner: its top and left edges are no longer along an axis.
    let sketch = text["sketches"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["plane"] == "centre")
        .unwrap();
    sketch["constraints"] = json!([]);
    let recipe = Recipe::from_value(&text).unwrap();
    let values = TypeValues::of(&recipe, &key("w1200x1200")).unwrap();
    let mut sloped = text.clone();
    let centre = sloped["sketches"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["plane"] == "centre")
        .unwrap();
    // The outer top's start, 100 mm higher: the top and the side it meets slope.
    let corner = centre["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["key"] == "outer_top")
        .unwrap()["start"]
        .clone();
    let point = centre["points"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["key"] == corner)
        .unwrap();
    let Some(construct_params::Value::Length(z)) =
        construct_params::Value::from_stored(point["y"].as_str().unwrap())
    else {
        panic!("a point's second coordinate is a length");
    };
    let higher = z
        .checked_add(construct_params::Length::from_mm(100))
        .unwrap();
    point["y"] = json!(construct_params::Value::Length(higher).stored());
    let sloped = Recipe::from_value(&sloped).unwrap();
    let at_angle = Seat::new(WAYS[1].1, [0, 0], [0, 0]).unwrap();
    let refused =
        evaluate_seated(&mut Asks, &sloped, &values, Limits::DEFAULT, &at_angle).unwrap_err();
    assert_eq!(*refused.reason(), Reason::UprightNotSquare, "{refused}");
    assert!(refused.to_string().contains("upright plane"), "{refused}");
    let square = Seat::new([0, 9], [0, 0], [0, 0]).unwrap();
    evaluate_seated(&mut Asks, &sloped, &values, Limits::DEFAULT, &square).unwrap();
    construct_families::evaluate_turned(
        &mut Asks,
        &sloped,
        &values,
        Limits::DEFAULT,
        Turn::Quarter,
    )
    .unwrap();
    // The window as drawn is built at either.
    evaluate_seated(&mut Asks, &recipe, &values, Limits::DEFAULT, &at_angle).unwrap();
}

/// Seated along an axis a family is exactly the family turned and moved by whole steps, as a
/// quarter turn builds it: corners and vectors alike.
#[test]
fn seated_along_an_axis_a_family_is_turned_exactly() {
    let door = read(DOOR);
    let values = TypeValues::of(&door, &key("d900x2100")).unwrap();
    let quarter = construct_families::evaluate_turned(
        &mut Asks,
        &door,
        &values,
        Limits::DEFAULT,
        Turn::Quarter,
    )
    .unwrap();
    let seat = Seat::new([0, 5], [10, 20], [0, 0]).unwrap();
    let seated = evaluate_seated(&mut Asks, &door, &values, Limits::DEFAULT, &seat).unwrap();
    for (a, b) in quarter.solids.iter().zip(&seated.solids) {
        let (
            Asked::Extrude { corners, by, .. },
            Asked::Extrude {
                corners: moved,
                by: moved_by,
                ..
            },
        ) = (&a.solid, &b.solid)
        else {
            panic!("an extrusion each");
        };
        assert_eq!(by, moved_by);
        let shifted: Vec<[i64; 3]> = corners
            .iter()
            .map(|c| [c[0] + 10, c[1] + 20, c[2]])
            .collect();
        assert_eq!(*moved, shifted);
    }
}

/// The evaluator keeps a seated family under its seat: the same seat is found kept, another seat
/// is built, and a seat's key is not a turn's.
#[test]
fn each_seat_is_kept_apart() {
    let window = read(FIXED);
    let values = TypeValues::of(&window, &key("w1200x1200")).unwrap();
    let mut evaluator = Evaluator::new(Asks);
    let here = Seat::new(WAYS[0].1, [0, 0], [0, 0]).unwrap();
    let there = Seat::new(WAYS[0].1, [1, 0], [0, 0]).unwrap();
    evaluator.evaluate_seated(&window, &values, &here).unwrap();
    evaluator.evaluate_seated(&window, &values, &here).unwrap();
    assert_eq!(evaluator.builds(), 1);
    evaluator.evaluate_seated(&window, &values, &there).unwrap();
    assert_eq!(evaluator.builds(), 2);
    let key = CacheKey::seated(&window, &values, &here);
    assert!(evaluator.has(&key));
    assert_eq!(key.seat(), Some(&here));
    assert_ne!(key, CacheKey::turned(&window, &values, Turn::None));
}
