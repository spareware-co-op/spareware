//! Where a roof is: the 6 m by 4 m gable at 1 in 4 and at 30 degrees, a mono-pitch roof and a
//! flat roof on an L, each corner on the kernel's grid, each figure exact. The numbers below were
//! worked by hand from the rules in shape.rs's header (steps of the length grid, 6,400 to the
//! millimetre; heights the nearest step, a half up).

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, degrees, edges, gable, id, l_shape, layer, mm, mono, rafters, slope};
use construct_params::{Area, Length, Volume};
use construct_roofs::{Form, Roof, Solid, default_roofs, shape};
use construct_slabs::{Function, Layers};

/// The 1 in 4 gable with 150 mm rafters: the ridge midway over the footprint at y = 2 m, 500 mm
/// above the base at the bottom face; the rafters stand 150 x sqrt(17) / 4 = 154.6 mm high
/// measured upright (989,545 steps), so the ridge is 654.6 mm up on the top face. The eaves are
/// 125 mm below the base, 500 mm out. It covers 6.6 m by 5 m in plan, 33 m2, and 34.0 m2 as laid
/// (33 x sqrt(17) / 4). Its one layer is a chevron lofted from verge to verge, 5 m across by
/// 154.6 mm upright, 6.6 m long.
#[test]
fn a_one_in_four_gable() {
    let roof = gable(slope(1, 4));
    assert_eq!(
        *roof.form(),
        Form::Gable {
            eaves: [id("o0"), id("o2")]
        }
    );
    let built = shape(&roof, &rafters()).unwrap();
    let ridge = built.ridge.clone().unwrap();
    assert_eq!(ridge.from, [-300 * MM, 2000 * MM]);
    assert_eq!(ridge.to, [6300 * MM, 2000 * MM]);
    assert_eq!(ridge.height, Length::from_steps(3_200_000 + 989_545));
    assert_eq!(
        built.faces,
        [Length::from_steps(989_545), Length::from_steps(0)]
    );
    assert_eq!(built.plan_area, Area::from_mm2(33_000_000));
    assert_eq!(
        built.slope_area,
        Area::from_square_steps(1_393_279_853_008_720)
    );
    let outline: Vec<(&str, [i64; 2])> = built
        .outline
        .iter()
        .map(|(edge, at)| (edge.as_str(), *at))
        .collect();
    assert_eq!(
        outline,
        [
            ("o0", [-300 * MM, -500 * MM]),
            ("o1", [6300 * MM, -500 * MM]),
            ("o2", [6300 * MM, 4500 * MM]),
            ("o3", [-300 * MM, 4500 * MM]),
        ]
    );
    assert_eq!(built.solids.len(), 1);
    let Solid::Loft { sections } = &built.solids[0].solid else {
        panic!("a sloped layer is lofted");
    };
    let (x0, x1) = (-300 * MM, 6300 * MM);
    let across = |x: i64| {
        vec![
            [x, -500 * MM, -125 * MM],
            [x, 2000 * MM, 500 * MM],
            [x, 4500 * MM, -125 * MM],
            [x, 4500 * MM, -125 * MM + 989_545],
            [x, 2000 * MM, 500 * MM + 989_545],
            [x, -500 * MM, -125 * MM + 989_545],
        ]
    };
    assert_eq!(sections[0], across(x0));
    assert_eq!(sections[1], across(x1));
    // 5 m across by 989,545 steps upright, 6.6 m long.
    let volume = i128::from(5000 * MM) * 989_545 * i128::from(6600 * MM);
    assert_eq!(built.solids[0].volume, Volume::from_cubic_steps(volume));
    assert_eq!(volume, 1_337_548_185_600_000_000_000);
}

/// The same gable at 30 degrees, worked as 577,350,269 in 1,000,000,000: the ridge 1,154.70 mm
/// above the base on the bottom face (7,390,083 steps, the nearest to 2 m x tan 30), the rafters
/// 150 / cos 30 = 173.2 mm upright, so the ridge 1,327.9 mm up on the top face; the eaves
/// 288.7 mm below the base; 33 m2 in plan and 38.1 m2 as laid (33 / cos 30).
#[test]
fn a_thirty_degree_gable() {
    let roof = gable(degrees(30));
    let built = shape(&roof, &rafters()).unwrap();
    assert_eq!(
        built.faces,
        [Length::from_steps(1_108_513), Length::from_steps(0)]
    );
    assert_eq!(
        built.ridge.unwrap().height,
        Length::from_steps(7_390_083 + 1_108_513)
    );
    assert_eq!(built.plan_area, Area::from_mm2(33_000_000));
    assert_eq!(
        built.slope_area,
        Area::from_square_steps(1_560_785_623_588_310)
    );
    let Solid::Loft { sections } = &built.solids[0].solid else {
        panic!("a sloped layer is lofted");
    };
    assert_eq!(sections[0][0], [-300 * MM, -500 * MM, -1_847_521]);
    assert_eq!(sections[0][1], [-300 * MM, 2000 * MM, 7_390_083]);
    assert_eq!(
        built.solids[0].volume,
        Volume::from_cubic_steps(1_498_354_851_840_000_000_000)
    );
    // The true heights are within half a step of these: 2 m x tan 30 is 7,390,083.45 steps.
    let true_ridge = 2000.0 * (std::f64::consts::PI / 6.0).tan() * 6400.0;
    assert!((true_ridge - 7_390_083.0).abs() < 0.5);
}

/// A mono-pitch roof rising at 1 in 4 from o0: 125 mm below the base at its eaves (500 mm out),
/// 1,075 mm above it at its high side (4.3 m in), in plan 6.6 m by 4.8 m, its one layer a
/// parallelogram lofted from verge to verge.
#[test]
fn a_mono_pitch_roof() {
    let roof = mono();
    assert_eq!(*roof.form(), Form::Mono { eaves: id("o0") });
    let built = shape(&roof, &rafters()).unwrap();
    assert_eq!(built.ridge, None);
    assert_eq!(built.plan_area, Area::from_mm2(6600 * 4800));
    assert_eq!(
        built.slope_area,
        Area::from_square_steps(1_337_548_658_888_371)
    );
    let Solid::Loft { sections } = &built.solids[0].solid else {
        panic!("a sloped layer is lofted");
    };
    assert_eq!(
        sections[0],
        [
            [-300 * MM, -500 * MM, -125 * MM],
            [-300 * MM, 4300 * MM, 1075 * MM],
            [-300 * MM, 4300 * MM, 1075 * MM + 989_545],
            [-300 * MM, -500 * MM, -125 * MM + 989_545],
        ]
    );
    let volume = i128::from(4800 * MM) * 989_545 * i128::from(6600 * MM);
    assert_eq!(built.solids[0].volume, Volume::from_cubic_steps(volume));
    // A roof rising from o1 runs across x instead: its eaves along x = 6 m, rising toward x = 0.
    let turned = Roof::new(common::rectangle(), &edges(&[("o1", 0, Some(slope(1, 2)))])).unwrap();
    let Solid::Loft { sections } = &shape(&turned, &rafters()).unwrap().solids[0].solid else {
        panic!("a sloped layer is lofted");
    };
    assert_eq!(sections[0][0], [6000 * MM, 0, 0]);
    assert_eq!(sections[0][1], [0, 0, 3000 * MM]);
    assert_eq!(sections[1][0], [6000 * MM, 4000 * MM, 0]);
}

/// A flat roof on the L with 300 mm over o0 and 600 mm over o3: each edge moved out square to
/// itself, its layers extruded straight up from the base, 52.8 m2 in plan and as laid.
#[test]
fn a_flat_roof_on_an_l() {
    let roof = Roof::new(l_shape(), &edges(&[("o0", 300, None), ("o3", 600, None)])).unwrap();
    let two = Layers::new(vec![
        layer(Function::Thermal, 100, "insulation-foam-glass"),
        layer(Function::Structure, 200, "concrete-regular"),
    ])
    .unwrap();
    let built = shape(&roof, &two).unwrap();
    let corners: Vec<[i64; 2]> = built.outline.iter().map(|(_, at)| *at).collect();
    let at = |x: i64, y: i64| [x * MM, y * MM];
    assert_eq!(
        corners,
        [
            at(0, -300),
            at(8000, -300),
            at(8000, 4000),
            at(4600, 4000),
            at(4600, 8000),
            at(0, 8000)
        ]
    );
    assert_eq!(built.plan_area, Area::from_mm2(52_800_000));
    assert_eq!(built.slope_area, built.plan_area);
    assert_eq!(built.faces, [mm(300), mm(200), mm(0)]);
    assert_eq!(built.ridge, None);
    let Solid::Extrude { profile, by } = &built.solids[1].solid else {
        panic!("a flat layer is extruded");
    };
    assert_eq!(*by, [0, 0, 200 * MM]);
    assert_eq!(profile[0], [0, -300 * MM, 0]);
    let Solid::Extrude { profile, by } = &built.solids[0].solid else {
        panic!("a flat layer is extruded");
    };
    assert_eq!(*by, [0, 0, 100 * MM]);
    assert_eq!(profile[3], [4600 * MM, 4000 * MM, 200 * MM]);
    assert_eq!(built.solids[0].volume, Volume::from_mm3(5_280_000_000));
    assert_eq!(built.solids[1].volume, Volume::from_mm3(10_560_000_000));
}

/// Layers share their faces bit for bit: the default steel roof's sheet (0.55 mm) on its
/// rafters, a membrane between them with no solid of its own; the sheet's lower face is the
/// rafters' upper face, corner for corner, and every face of a slope is parallel to the bottom
/// face, moved up by one whole number of steps.
#[test]
fn layers_share_their_faces_bit_for_bit() {
    let steel = default_roofs().unwrap().remove(0);
    assert_eq!(steel.id, "roof-steel-rafter-150");
    let built = shape(&gable(slope(1, 4)), &steel.layers).unwrap();
    assert_eq!(
        built.faces,
        [
            Length::from_steps(993_174),
            Length::from_steps(989_545),
            Length::from_steps(989_545),
            Length::from_steps(0)
        ]
    );
    let layers: Vec<usize> = built.solids.iter().map(|s| s.layer).collect();
    assert_eq!(layers, [0, 2], "the membrane has no solid");
    let section = |at: usize| match &built.solids[at].solid {
        Solid::Loft { sections } => sections[0].clone(),
        _ => panic!("a sloped layer is lofted"),
    };
    let (sheet, rafters) = (section(0), section(1));
    let mut rafters_top: Vec<[i64; 3]> = rafters[3..].to_vec();
    rafters_top.reverse();
    assert_eq!(sheet[..3], rafters_top[..]);
    for (low, high) in sheet[..3].iter().zip(sheet[3..].iter().rev()) {
        assert_eq!(high[2] - low[2], 993_174 - 989_545);
    }
}
