//! Roofs solved from a slope per edge (job 6.2): a hip roof on a rectangle, a hip at one end and a
//! verge at the other, an uneven gable, a rectangle turned off the axes, an L with its valley, and
//! the L with its overhangs. Every pitch is 3 in 4, or 4 in 3 for the uneven gable's far slope, so
//! a slope's true length is 5/4 (or 5/3) of its run and every figure below is a whole millimetre,
//! worked by hand from the rules in solve.rs's and shape.rs's headers: a 200 mm layer stands
//! 250 mm upright on 3 in 4, a 300 mm layer 375 mm (and 500 mm on 4 in 3).

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, at, edges, id, l_shape, layer, rectangle, slope};
use construct_materials::Materials;
use construct_params::{Area, Length, Volume};
use construct_roofs::{
    Form, Line, LineKind, Pitch, Roof, Shape, Solid, filled, quantities, roof_recipe, shape,
};
use construct_slabs::{Boundary, Function, Layers};

/// One structural layer `mm` thick.
fn one(mm: i64) -> Layers {
    Layers::new(vec![layer(Function::Structure, mm, "timber-douglas-fir")]).unwrap()
}

/// A point in whole millimetres, as grid steps.
fn p(x: i64, y: i64, z: i64) -> [i64; 3] {
    [x * MM, y * MM, z * MM]
}

/// A line of `kind` between two points in whole millimetres.
fn l(kind: LineKind, a: [i64; 3], b: [i64; 3]) -> Line {
    let (a, b) = (
        [a[0] * MM, a[1] * MM, a[2] * MM],
        [b[0] * MM, b[1] * MM, b[2] * MM],
    );
    let (from, to) = if a <= b { (a, b) } else { (b, a) };
    Line { kind, from, to }
}

/// The lines of one kind.
fn of_kind(built: &Shape, kind: LineKind) -> Vec<Line> {
    built
        .lines
        .iter()
        .filter(|line| line.kind == kind)
        .cloned()
        .collect()
}

/// Each plane's edge and plan area in square millimetres.
fn plane_areas(built: &Shape) -> Vec<(String, i128)> {
    built
        .planes
        .iter()
        .map(|plane| {
            (
                plane.edge.as_ref().unwrap().as_str().to_owned(),
                plane.plan_area.square_steps() / i128::from(MM * MM),
            )
        })
        .collect()
}

/// The planes' plan areas and slope areas sum to the whole's.
fn sums_to_the_whole(built: &Shape) {
    let plan: i128 = built
        .planes
        .iter()
        .map(|p| p.plan_area.square_steps())
        .sum();
    let laid: i128 = built
        .planes
        .iter()
        .map(|p| p.slope_area.square_steps())
        .sum();
    assert_eq!(plan, built.plan_area.square_steps());
    assert_eq!(laid, built.slope_area.square_steps());
}

/// A piece's corners, sorted, in whole millimetres.
fn sorted_mm(piece: &[[i64; 3]]) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = piece.iter().map(|c| c.map(|v| v / MM)).collect();
    out.sort_unstable();
    out
}

const THREE_FOUR: (i64, i64) = (3, 4);

fn pitch() -> Pitch {
    slope(THREE_FOUR.0, THREE_FOUR.1)
}

/// A hip roof on the 6 m by 4 m rectangle, every edge at 3 in 4, no overhang, one 200 mm layer:
/// the ridge along y = 2 m from x = 2 m to 4 m, 1,500 mm up on the bottom face (half the 4 m span
/// at 3 in 4) and 1,750 mm on the top face; two trapezoids of 8 m2 and two triangles of 4 m2 in
/// plan, 10 and 5 m2 as laid; four hips from the corners; one solid, the union of the four pieces
/// extruded 250 mm, 24 m2 by 250 mm.
#[test]
fn a_hip_roof_on_a_rectangle() {
    let roof = Roof::new(
        rectangle(),
        &edges(&[
            ("o0", 0, Some(pitch())),
            ("o1", 0, Some(pitch())),
            ("o2", 0, Some(pitch())),
            ("o3", 0, Some(pitch())),
        ]),
    )
    .unwrap();
    let built = shape(&roof, &one(200)).unwrap();
    assert_eq!(
        built.faces,
        [Length::from_steps(250 * MM), Length::from_steps(0)]
    );
    assert_eq!(built.plan_area, Area::from_mm2(24_000_000));
    assert_eq!(built.slope_area, Area::from_mm2(30_000_000));
    assert_eq!(built.height, Length::from_steps(1750 * MM));
    assert_eq!(
        plane_areas(&built),
        [
            ("o0".to_owned(), 8_000_000),
            ("o1".to_owned(), 4_000_000),
            ("o2".to_owned(), 8_000_000),
            ("o3".to_owned(), 4_000_000)
        ]
    );
    assert_eq!(built.planes[0].slope_area, Area::from_mm2(10_000_000));
    assert_eq!(built.planes[1].slope_area, Area::from_mm2(5_000_000));
    sums_to_the_whole(&built);
    assert_eq!(
        sorted_mm(&built.planes[0].pieces[0]),
        [
            [0, 0, 250],
            [2000, 2000, 1750],
            [4000, 2000, 1750],
            [6000, 0, 250]
        ]
    );
    assert_eq!(
        of_kind(&built, LineKind::Ridge),
        [l(LineKind::Ridge, [2000, 2000, 1750], [4000, 2000, 1750])]
    );
    assert_eq!(
        of_kind(&built, LineKind::Hip),
        [
            l(LineKind::Hip, [0, 0, 250], [2000, 2000, 1750]),
            l(LineKind::Hip, [0, 4000, 250], [2000, 2000, 1750]),
            l(LineKind::Hip, [4000, 2000, 1750], [6000, 0, 250]),
            l(LineKind::Hip, [4000, 2000, 1750], [6000, 4000, 250]),
        ]
    );
    assert_eq!(of_kind(&built, LineKind::Eave).len(), 4);
    assert_eq!(of_kind(&built, LineKind::Valley), []);
    assert_eq!(of_kind(&built, LineKind::Verge), []);
    assert_eq!(built.solids.len(), 1);
    let Solid::Unite { profiles, by } = &built.solids[0].solid else {
        panic!("a solved layer is its pieces united");
    };
    assert_eq!(*by, [0, 0, 250 * MM]);
    assert_eq!(profiles.len(), 4);
    assert_eq!(
        sorted_mm(&profiles[0]),
        [
            [0, 0, 0],
            [2000, 2000, 1500],
            [4000, 2000, 1500],
            [6000, 0, 0]
        ]
    );
    assert_eq!(built.solids[0].volume, Volume::from_mm3(6_000_000_000));
    // The ridge height from the pitch and the span: 4 m / 2 x 3 / 4, and the layer's 250 mm.
    let q = quantities(&roof, &one(200), &Materials::default_metric()).unwrap();
    assert_eq!(
        q.ridge_height,
        Some(Length::from_steps((2000 * 3 / 4 + 250) * MM))
    );
    assert_eq!(q.volume, Volume::from_mm3(6_000_000_000));
}

/// A hip at the right-hand end (o1) and a verge at the left (o3): the ridge runs from the verge
/// at x = 0 to x = 4 m; two trapezoids of 10 m2 and the hip end's triangle of 4 m2; two hips, and
/// the verge in two lines, rising to the ridge and falling from it.
#[test]
fn a_hip_at_one_end_and_a_verge_at_the_other() {
    let roof = Roof::new(
        rectangle(),
        &edges(&[
            ("o0", 0, Some(pitch())),
            ("o1", 0, Some(pitch())),
            ("o2", 0, Some(pitch())),
        ]),
    )
    .unwrap();
    assert_eq!(
        *roof.form(),
        Form::Solved {
            eaves: vec![id("o0"), id("o1"), id("o2")]
        }
    );
    let built = shape(&roof, &one(200)).unwrap();
    assert_eq!(
        plane_areas(&built),
        [
            ("o0".to_owned(), 10_000_000),
            ("o1".to_owned(), 4_000_000),
            ("o2".to_owned(), 10_000_000)
        ]
    );
    sums_to_the_whole(&built);
    assert_eq!(
        of_kind(&built, LineKind::Ridge),
        [l(LineKind::Ridge, [0, 2000, 1750], [4000, 2000, 1750])]
    );
    assert_eq!(of_kind(&built, LineKind::Hip).len(), 2);
    assert_eq!(
        of_kind(&built, LineKind::Verge),
        [
            l(LineKind::Verge, [0, 0, 250], [0, 2000, 1750]),
            l(LineKind::Verge, [0, 2000, 1750], [0, 4000, 250]),
        ]
    );
    assert_eq!(of_kind(&built, LineKind::Eave).len(), 3);
    assert_eq!(built.solids[0].volume, Volume::from_mm3(6_000_000_000));
}

/// An uneven gable: o0 at 3 in 4 and o2 at 4 in 3 on the 6 m by 4 m rectangle, one 300 mm layer.
/// On the bottom face the slopes meet where 3/4 a = 4/3 (4 m - a): 2,560 mm across, 1,920 mm up.
/// The layer stands 375 mm upright on o0's slope and 500 mm on o2's, so on the top face they meet
/// at (s' W + u' - u) / (s + s') = 2,620 mm across, 2,340 mm up. Its section is the six-sided
/// chevron between the two, 1,676,250 mm2 (by the shoelace rule), lofted 6 m: 10.0575 m3. Its
/// planes: 6 m by 2,620 mm (19.65 m2 as laid, x 5/4) and 6 m by 1,380 mm (13.8 m2, x 5/3).
#[test]
fn an_uneven_gable() {
    let roof = Roof::new(
        rectangle(),
        &edges(&[("o0", 0, Some(slope(3, 4))), ("o2", 0, Some(slope(4, 3)))]),
    )
    .unwrap();
    let built = shape(&roof, &one(300)).unwrap();
    let ridge = built.ridge.clone().unwrap();
    assert_eq!(ridge.from, [0, 2620 * MM]);
    assert_eq!(ridge.to, [6000 * MM, 2620 * MM]);
    assert_eq!(ridge.height, Length::from_steps(2340 * MM));
    let Solid::Loft { sections } = &built.solids[0].solid else {
        panic!("a gable's layer is lofted");
    };
    assert_eq!(
        sections[0],
        [
            p(0, 0, 0),
            p(0, 2560, 1920),
            p(0, 4000, 0),
            p(0, 4000, 500),
            p(0, 2620, 2340),
            p(0, 0, 375),
        ]
    );
    assert_eq!(sections[1][1], p(6000, 2560, 1920));
    assert_eq!(built.solids[0].volume, Volume::from_mm3(10_057_500_000));
    assert_eq!(
        plane_areas(&built),
        [("o0".to_owned(), 15_720_000), ("o2".to_owned(), 8_280_000)]
    );
    assert_eq!(built.planes[0].slope_area, Area::from_mm2(19_650_000));
    assert_eq!(built.planes[1].slope_area, Area::from_mm2(13_800_000));
    sums_to_the_whole(&built);
    assert_eq!(
        of_kind(&built, LineKind::Ridge),
        [l(LineKind::Ridge, [0, 2620, 2340], [6000, 2620, 2340])]
    );
    assert_eq!(of_kind(&built, LineKind::Eave).len(), 2);
    assert_eq!(of_kind(&built, LineKind::Verge).len(), 4);
}

/// The 10 m by 5 m rectangle turned so its long edges run along (4, 3): every corner and every
/// meeting point a whole millimetre. Hipped: the ridge from (500, 3500) to (4500, 6500), 1,875 mm
/// up on the bottom face (2.5 m at 3 in 4), 2,125 mm on the top; trapezoids of 18.75 m2 and
/// triangles of 6.25 m2. Rising from o0 alone: one plane over all 50 m2, an eave and three verges.
#[test]
fn a_rectangle_turned_off_the_axes() {
    let turned = || {
        Boundary::from_corners(
            &[at(0, 0), at(8000, 6000), at(5000, 10000), at(-3000, 4000)],
            &[],
        )
        .unwrap()
    };
    let roof = Roof::new(
        turned(),
        &edges(&[
            ("o0", 0, Some(pitch())),
            ("o1", 0, Some(pitch())),
            ("o2", 0, Some(pitch())),
            ("o3", 0, Some(pitch())),
        ]),
    )
    .unwrap();
    let built = shape(&roof, &one(200)).unwrap();
    assert_eq!(built.plan_area, Area::from_mm2(50_000_000));
    assert_eq!(
        plane_areas(&built),
        [
            ("o0".to_owned(), 18_750_000),
            ("o1".to_owned(), 6_250_000),
            ("o2".to_owned(), 18_750_000),
            ("o3".to_owned(), 6_250_000)
        ]
    );
    sums_to_the_whole(&built);
    assert_eq!(
        of_kind(&built, LineKind::Ridge),
        [l(LineKind::Ridge, [500, 3500, 2125], [4500, 6500, 2125])]
    );
    assert_eq!(
        of_kind(&built, LineKind::Hip),
        [
            l(LineKind::Hip, [-3000, 4000, 250], [500, 3500, 2125]),
            l(LineKind::Hip, [0, 0, 250], [500, 3500, 2125]),
            l(LineKind::Hip, [4500, 6500, 2125], [5000, 10000, 250]),
            l(LineKind::Hip, [4500, 6500, 2125], [8000, 6000, 250]),
        ]
    );
    assert_eq!(built.height, Length::from_steps(2125 * MM));
    assert_eq!(built.solids[0].volume, Volume::from_mm3(12_500_000_000));

    let mono = Roof::new(turned(), &edges(&[("o0", 0, Some(pitch()))])).unwrap();
    let built = shape(&mono, &one(200)).unwrap();
    assert_eq!(plane_areas(&built), [("o0".to_owned(), 50_000_000)]);
    assert_eq!(of_kind(&built, LineKind::Eave).len(), 1);
    assert_eq!(of_kind(&built, LineKind::Verge).len(), 3);
    // The high side, 5 m in from o0 at 3 in 4: 3,750 mm up, and 250 more on the top face.
    assert_eq!(built.height, Length::from_steps(4000 * MM));
}

/// The L (8 m by 4 m, and 4 m by 4 m above its left half), every edge at 3 in 4, one 300 mm
/// layer: two ridges meeting at (2, 2) m, 1,500 mm up on the bottom face; five hips; one valley
/// from the inside corner (4, 4) m down from the ridges' meeting; planes of 12, 4, 8, 8, 4 and
/// 12 m2, 48 m2 in all; 48 m2 by 375 mm of layer.
#[test]
fn an_l_with_its_valley() {
    let all = |o: i64| {
        edges(&[
            ("o0", o, Some(pitch())),
            ("o1", o, Some(pitch())),
            ("o2", o, Some(pitch())),
            ("o3", o, Some(pitch())),
            ("o4", o, Some(pitch())),
            ("o5", o, Some(pitch())),
        ])
    };
    let roof = Roof::new(l_shape(), &all(0)).unwrap();
    let built = shape(&roof, &one(300)).unwrap();
    assert_eq!(built.plan_area, Area::from_mm2(48_000_000));
    assert_eq!(
        plane_areas(&built),
        [
            ("o0".to_owned(), 12_000_000),
            ("o1".to_owned(), 4_000_000),
            ("o2".to_owned(), 8_000_000),
            ("o3".to_owned(), 8_000_000),
            ("o4".to_owned(), 4_000_000),
            ("o5".to_owned(), 12_000_000)
        ]
    );
    sums_to_the_whole(&built);
    assert_eq!(built.slope_area, Area::from_mm2(60_000_000));
    assert_eq!(
        of_kind(&built, LineKind::Ridge),
        [
            l(LineKind::Ridge, [2000, 2000, 1875], [2000, 6000, 1875]),
            l(LineKind::Ridge, [2000, 2000, 1875], [6000, 2000, 1875]),
        ]
    );
    assert_eq!(
        of_kind(&built, LineKind::Valley),
        [l(LineKind::Valley, [2000, 2000, 1875], [4000, 4000, 375])]
    );
    assert_eq!(of_kind(&built, LineKind::Hip).len(), 5);
    assert_eq!(of_kind(&built, LineKind::Eave).len(), 6);
    assert_eq!(built.height, Length::from_steps(1875 * MM));
    assert_eq!(built.solids[0].volume, Volume::from_mm3(18_000_000_000));

    // With 500 mm over every edge: the eaves 375 mm below the base on the bottom face, the outline
    // the L moved out (65 m2), the same ridges and valley.
    let over = Roof::new(l_shape(), &all(500)).unwrap();
    let built = shape(&over, &one(300)).unwrap();
    assert_eq!(built.plan_area, Area::from_mm2(65_000_000));
    sums_to_the_whole(&built);
    let corners: Vec<[i64; 2]> = built.outline.iter().map(|(_, at)| *at).collect();
    assert_eq!(corners[3], [4500 * MM, 4500 * MM]);
    assert_eq!(
        of_kind(&built, LineKind::Valley),
        [l(LineKind::Valley, [2000, 2000, 1875], [4500, 4500, 0])]
    );
    assert!(
        of_kind(&built, LineKind::Eave)
            .iter()
            .all(|line| line.from[2] == 0 && line.to[2] == 0)
    );
    assert_eq!(built.solids[0].volume, Volume::from_mm3(24_375_000_000));
}

/// A solved roof's filled recipe names its form `solved`, its slopes, and each layer's pieces to
/// unite, as the shape gives them.
#[test]
fn a_solved_roof_is_filled_with_its_pieces() {
    let roof = Roof::new(
        rectangle(),
        &edges(&[("o0", 0, Some(pitch())), ("o1", 0, Some(pitch()))]),
    )
    .unwrap();
    let layers = one(200);
    let recipe = filled(&roof_recipe(&roof), &layers).unwrap();
    assert_eq!(recipe.args["form"], "solved");
    assert_eq!(recipe.args["slopes"], serde_json::json!(["o0", "o1"]));
    let solid = &recipe.args["layers"][0]["solid"];
    let Solid::Unite { profiles, by } = &shape(&roof, &layers).unwrap().solids[0].solid else {
        panic!("a solved layer is its pieces united");
    };
    assert_eq!(solid["by"], serde_json::json!(by));
    assert_eq!(solid["unite"], serde_json::json!(profiles));
    assert_eq!(profiles.len(), 2);
}
