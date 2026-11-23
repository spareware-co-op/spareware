//! A hosted family's opening, its solids turned, and the boxes its solids lie in (CON-A7-F3-P2,
//! piece A64, job 6.4): on every type of the three door and window templates, the opening a
//! template declares is solved at the type's values to the outside of its frame, through the
//! wall's whole thickness; each refusal (no host, a host with no opening, an outline that is not a
//! loop, a sketch that does not solve, a sketch off the wall's plane) names its place and reason;
//! a family turned by a quarter turn is the family, turned, exactly, and kept apart in the cache;
//! and the boxes worked out without an engine are each solid's own. Judged through this crate's
//! types and the `Engine` trait only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_families::{
    Constraint, Depth, Evaluator, Extent, Host, Hosting, Key, Limits, Place, Reason, Recipe, Turn,
    TypeValues, extents, opening,
};
use construct_params::Length;
use engine_geometry::value::engine::{
    Cut, DisplayMesh, Edition, Engine, EngineError, GridPlane, Profile, Section, Volume,
};
use engine_geometry::value::grid::GridStep;
use std::collections::BTreeSet;

const DOOR: &str = include_str!("../../../library/doors-windows/single-swing-door.json");
const FIXED: &str = include_str!("../../../library/doors-windows/fixed-window.json");
const SLIDING: &str = include_str!("../../../library/doors-windows/sliding-window.json");

fn read(text: &str) -> Recipe {
    Recipe::from_json(text).unwrap()
}

fn key(text: &str) -> Key {
    Key::parse(text).unwrap()
}

fn mm(v: i64) -> Length {
    Length::checked_from_mm(v).unwrap()
}

/// A type key's width and height in millimetres: `d900x2100` is 900 by 2100.
fn size_of(ty: &str) -> (i64, i64) {
    let (w, h) = ty[1..].split_once('x').unwrap();
    (w.parse().unwrap(), h.parse().unwrap())
}

/// Every type key a recipe names.
fn types(recipe: &Recipe) -> Vec<String> {
    recipe
        .named_types()
        .iter()
        .map(|t| t.key().as_str().to_owned())
        .collect()
}

/// The corners of an upright rectangle, as a set.
fn rectangle(x: (i64, i64), z: (i64, i64)) -> BTreeSet<[Length; 2]> {
    BTreeSet::from([
        [mm(x.0), mm(z.0)],
        [mm(x.1), mm(z.0)],
        [mm(x.1), mm(z.1)],
        [mm(x.0), mm(z.1)],
    ])
}

/// Every type of the three templates declares a wall and an opening that is the outside of its
/// frame at the type's size: a door's from the floor, a window's from its sill (900 mm, the
/// instance parameter's default), centred on the family's origin along the wall, cut through the
/// wall's whole thickness. Each corner is once in the outline, a closed loop of four.
#[test]
fn every_template_type_opens_its_wall_to_the_outside_of_its_frame() {
    for (text, sill) in [(DOOR, 0), (FIXED, 900), (SLIDING, 900)] {
        let recipe = read(text);
        let all = types(&recipe);
        assert!(all.len() >= 6, "{}", recipe.name);
        for ty in all {
            let (w, h) = size_of(&ty);
            let values = TypeValues::of(&recipe, &key(&ty)).unwrap();
            let solved = opening(&recipe, &values).unwrap();
            assert_eq!(solved.host, Host::Wall, "{ty}");
            assert_eq!(solved.depth, Depth::ThroughHost, "{ty}");
            assert_eq!(solved.outline.len(), 4, "{ty}");
            let corners: BTreeSet<[Length; 2]> = solved.outline.iter().copied().collect();
            assert_eq!(
                corners,
                rectangle((-w / 2, w / 2), (sill, sill + h)),
                "{ty}"
            );
            // Each corner and the next differ along one axis only: the loop's sides are upright.
            for (i, a) in solved.outline.iter().enumerate() {
                let b = solved.outline[(i + 1) % 4];
                assert!((a[0] == b[0]) != (a[1] == b[1]), "{ty}: {a:?} to {b:?}");
            }
            assert_eq!(opening(&recipe, &values).unwrap(), solved, "{ty}: again");
        }
    }
}

/// Each refusal names its place and why: a family that is not hosted, one hosted on a floor or a
/// face (no opening to cut), an outline that is not a closed loop, a sketch that cannot be solved
/// at its values (two fixed points held at another distance), a sketch that is missing, and an
/// opening drawn on a plan.
#[test]
fn an_opening_that_cannot_be_cut_says_what_and_why() {
    let door = read(DOOR);
    let values = TypeValues::of(&door, &key("d900x2100")).unwrap();
    let refused = |recipe: &Recipe| opening(recipe, &values).unwrap_err();

    let mut unhosted = door.clone();
    unhosted.hosting = Hosting::Unhosted;
    let e = refused(&unhosted);
    assert_eq!(
        (e.place(), e.reason()),
        (&Place::Hosting, &Reason::NotHosted)
    );
    assert_eq!(
        e.to_string(),
        "hosting: the family is not hosted, so it cuts no opening"
    );
    for (host, name) in [(Hosting::Floor, "floor"), (Hosting::Face, "face")] {
        let mut on = door.clone();
        on.hosting = host;
        let e = refused(&on);
        assert_eq!(e.reason(), &Reason::NoOpening { host: name });
        assert!(e.to_string().contains(name), "{e}");
    }

    let Hosting::Wall { opening: declared } = &door.hosting else {
        panic!("the door is wall-hosted");
    };
    let mut open = door.clone();
    if let Hosting::Wall { opening } = &mut open.hosting {
        opening.profile.pop();
    }
    let e = refused(&open);
    assert_eq!(
        (e.place(), e.reason()),
        (&Place::Hosting, &Reason::NotALoop)
    );

    let mut unsolved = door.clone();
    let sketch = unsolved
        .sketches
        .iter_mut()
        .find(|s| s.key == declared.sketch)
        .unwrap();
    sketch.constraints.push(Constraint::Distance {
        a: key("o"),
        b: key("e"),
        length: "500 mm".to_owned(),
    });
    let e = refused(&unsolved);
    assert!(matches!(e.reason(), Reason::NotSolved(_)), "{e}");
    assert!(
        matches!(e.place(), Place::Constraint { sketch, .. } | Place::Sketch(sketch) if *sketch == declared.sketch),
        "{e}"
    );

    let mut missing = door.clone();
    if let Hosting::Wall { opening } = &mut missing.hosting {
        opening.sketch = key("nowhere");
    }
    let e = refused(&missing);
    assert_eq!(e.place(), &Place::Sketch(key("nowhere")));

    let mut on_plan = door.clone();
    let plan = on_plan
        .sketches
        .iter()
        .find(|s| s.plane == key("floor"))
        .map(|s| s.key.clone())
        .unwrap();
    if let Hosting::Wall { opening } = &mut on_plan.hosting {
        opening.sketch = plan;
    }
    let e = refused(&on_plan);
    assert!(
        matches!(e.reason(), Reason::NotOnWallPlane | Reason::Missing { .. }),
        "{e}"
    );
}

/// A test engine keeping the box of each extrusion and the corners it was asked, so a turned
/// family can be compared corner for corner.
struct Corners;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Asked {
    corners: Vec<[i64; 3]>,
    by: [i64; 3],
}

fn unanswered(operation: &'static str) -> EngineError {
    EngineError::Unsupported {
        operation,
        what: "anything but extrusions".to_owned(),
    }
}

impl Engine for Corners {
    type Solid = Asked;

    fn edition(&self) -> Edition {
        Edition {
            engine: "test.corners".to_owned(),
            version: "this test file".to_owned(),
            step: GridStep::Q0,
            tolerances: Vec::new(),
            answers: Vec::new(),
        }
    }

    fn extrude(&mut self, profile: &Profile, by: [i64; 3]) -> Result<Asked, EngineError> {
        let corners = profile
            .outer
            .iter()
            .chain(profile.holes.iter().flatten())
            .map(|p| [p.x.get(), p.y.get(), p.z.get()])
            .collect();
        Ok(Asked { corners, by })
    }

    fn subtract(&mut self, _: &Asked, _: &Asked) -> Result<Asked, EngineError> {
        Err(unanswered("subtract"))
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

/// `[x, y, z]` turned by `turn`, by hand.
fn turned(turn: Turn, [x, y, z]: [i64; 3]) -> [i64; 3] {
    match turn {
        Turn::None => [x, y, z],
        Turn::Quarter => [-y, x, z],
        Turn::Half => [-x, -y, z],
        Turn::ThreeQuarters => [y, -x, z],
    }
}

/// A family built turned is the family turned, corner for corner and vector for vector, every
/// way; the evaluator keeps each turn apart and builds each once.
#[test]
fn a_family_built_turned_is_the_family_turned_exactly() {
    let window = read(FIXED);
    let values = TypeValues::of(&window, &key("w1200x1200")).unwrap();
    let mut evaluator = Evaluator::new(Corners);
    let plain = evaluator.evaluate(&window, &values).unwrap();
    for turn in Turn::ALL {
        let built = evaluator.evaluate_turned(&window, &values, turn).unwrap();
        assert_eq!(built.solids.len(), plain.solids.len());
        for (a, b) in plain.solids.iter().zip(&built.solids) {
            assert_eq!(a.step, b.step);
            assert_eq!(a.material, b.material);
            let by_hand: Vec<[i64; 3]> = a.solid.corners.iter().map(|&p| turned(turn, p)).collect();
            assert_eq!(b.solid.corners, by_hand, "{turn:?} {}", a.step);
            assert_eq!(b.solid.by, turned(turn, a.solid.by), "{turn:?}");
        }
    }
    assert_eq!(
        evaluator.builds(),
        4,
        "none, then the three turns; none again is kept"
    );
    assert_eq!(evaluator.kept(), 4);
    evaluator
        .evaluate_turned(&window, &values, Turn::Half)
        .unwrap();
    assert_eq!(evaluator.builds(), 4);
}

/// The boxes worked out without an engine are each solid's own box: the door's frame and leaf and
/// the window's frame and glass at their sizes (the templates' sections), and the same boxes
/// turned a quarter turn are the boxes turned.
#[test]
fn each_solid_lies_in_its_own_box_without_an_engine() {
    let door = read(DOOR);
    let values = TypeValues::of(&door, &key("d900x2100")).unwrap();
    let boxes = extents(&door, &values, Limits::DEFAULT, Turn::None).unwrap();
    let shown = |e: &Extent| {
        (
            e.step.as_str().to_owned(),
            e.material.as_str().to_owned(),
            e.low.map(|l| l.whole_mm().unwrap()),
            e.high.map(|l| l.whole_mm().unwrap()),
        )
    };
    let listed: Vec<_> = boxes.iter().map(shown).collect();
    assert_eq!(
        listed,
        [
            (
                "frame".to_owned(),
                "timber-douglas-fir".to_owned(),
                [-450, -50, 0],
                [450, 50, 2100]
            ),
            (
                "leaf".to_owned(),
                "timber-white-oak".to_owned(),
                [-410, 10, 0],
                [410, 50, 2060]
            ),
        ]
    );
    let quarter = extents(&door, &values, Limits::DEFAULT, Turn::Quarter).unwrap();
    let leaf = quarter.iter().find(|e| e.step == key("leaf")).unwrap();
    assert_eq!(leaf.low.map(|l| l.whole_mm().unwrap()), [-50, -410, 0]);
    assert_eq!(leaf.high.map(|l| l.whole_mm().unwrap()), [-10, 410, 2060]);
    let window = read(FIXED);
    let values = TypeValues::of(&window, &key("w1200x1200")).unwrap();
    let boxes = extents(&window, &values, Limits::DEFAULT, Turn::None).unwrap();
    let glass = boxes.iter().find(|e| e.step == key("glass")).unwrap();
    assert_eq!(glass.material.as_str(), "glass-plate");
    assert_eq!(glass.low.map(|l| l.whole_mm().unwrap()), [-550, -12, 950]);
    assert_eq!(glass.high.map(|l| l.whole_mm().unwrap()), [550, 12, 2050]);
}
