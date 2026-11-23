//! geom-solidcore's engine through the engine suite (`tests/suite`), the cases every engine of the
//! seam is judged by, and what is this engine's own: its edition, the one conversion of
//! millimetres at a program's seam, and the interim register (`tests/interim.json`), which names
//! for each operation of the seam what answers it today and what will make it exact.

mod suite;

use std::collections::BTreeSet;

use geom_num::{Exactness, I256};
use geom_op::engine::{Engine, EngineError, Profile, Section, Volume};
use geom_solidcore::engine::{
    BY_BOOL, BY_CUT, BY_LOFT, BY_PRISM, BY_SWEEP, BY_UNION, EngineSolid, SOLIDCORE, SolidcoreEngine,
};
use geom_topo::Body;
use serde_json::Value;

use suite::{Checked, Drawn, STEP, at, mm};

impl Checked for SolidcoreEngine {
    fn body(solid: &EngineSolid) -> (&Body, Exactness) {
        (solid.body(), solid.exactness())
    }
}

fn engine() -> SolidcoreEngine {
    SolidcoreEngine::new(STEP)
}

#[test]
fn a_wall_with_a_door_is_the_box_less_the_opening() {
    suite::a_wall_with_a_door_is_the_box_less_the_opening(engine());
}

#[test]
fn a_box_cut_at_mid_height_is_one_rectangle_on_its_four_sides() {
    suite::a_box_cut_at_mid_height_is_one_rectangle_on_its_four_sides(engine());
}

#[test]
fn the_wall_with_its_door_cut_in_plan_has_the_gap() {
    suite::the_wall_with_its_door_cut_in_plan_has_the_gap(engine());
}

#[test]
fn a_plane_that_touches_or_misses_cuts_nothing() {
    suite::a_plane_that_touches_or_misses_cuts_nothing(engine());
}

#[test]
fn a_plane_the_engine_does_not_cut_yet_is_a_value() {
    suite::a_plane_the_engine_does_not_cut_yet_is_a_value(engine());
}

#[test]
fn a_profile_with_a_hole_extrudes_to_the_box_less_the_hole() {
    suite::a_profile_with_a_hole_extrudes_to_the_box_less_the_hole(engine());
}

#[test]
fn a_vector_in_the_profile_s_plane_is_an_invalid_input() {
    suite::a_vector_in_the_profile_s_plane_is_an_invalid_input(engine());
}

#[test]
fn the_mesh_is_closed_and_bounded_by_the_wall() {
    suite::the_mesh_is_closed_and_bounded_by_the_wall(engine());
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    suite::the_same_input_gives_the_same_bytes(engine);
}

#[test]
fn a_sloped_strip_along_a_straight_boundary_is_its_profile_times_its_length() {
    suite::a_sloped_strip_along_a_straight_boundary_is_its_profile_times_its_length(engine());
}

#[test]
fn two_strips_meeting_at_a_corner_unite_into_one_solid() {
    suite::two_strips_meeting_at_a_corner_unite_into_one_solid(engine());
}

#[test]
fn four_strips_round_a_rectangle_site_unite_into_one_closed_solid() {
    suite::four_strips_round_a_rectangle_site_unite_into_one_closed_solid(engine());
}

#[test]
fn a_strip_cut_by_a_box_loses_the_box_s_share() {
    suite::a_strip_cut_by_a_box_loses_the_box_s_share(engine());
}

#[test]
fn two_strips_sharing_an_exactly_coincident_face_unite_into_one() {
    suite::two_strips_sharing_an_exactly_coincident_face_unite_into_one(engine());
}

#[test]
fn a_loft_that_folds_back_is_a_fault_not_a_solid() {
    suite::a_loft_that_folds_back_is_a_fault_not_a_solid(engine());
}

#[test]
fn a_loft_whose_walls_cross_is_a_fault_not_a_solid() {
    suite::a_loft_whose_walls_cross_is_a_fault_not_a_solid(engine());
}

#[test]
fn sections_of_different_counts_are_an_invalid_input() {
    suite::sections_of_different_counts_are_an_invalid_input(engine());
}

/// What uniting two strips at slivers of 0.5, 1 and 3 mm gives today, recorded as it is (no
/// retry, heal or nudge: that is a later job's): this engine unites flat-faced strips by the
/// kernel's exact geom-bool, so the raised and overlapping pairs are one closed solid of one
/// shell and the closed form's volume at every size, and the pair apart is one closed body of two
/// shells, the gap kept exactly as wide as it was, never closed over.
#[test]
fn the_sliver_unions_are_as_recorded() {
    use suite::{Outcome, SLIVERS, Sliver};
    let mut e = engine();
    let mut found = Vec::new();
    for sliver in [Sliver::Raised, Sliver::Overlapping, Sliver::Apart] {
        for tenths in SLIVERS {
            found.push((
                sliver,
                tenths,
                suite::united_at_a_sliver(&mut e, sliver, tenths),
            ));
        }
    }
    let expected: Vec<_> = [Sliver::Raised, Sliver::Overlapping, Sliver::Apart]
        .into_iter()
        .flat_map(|sliver| {
            let shells = if sliver == Sliver::Apart { 2 } else { 1 };
            SLIVERS.map(|tenths| (sliver, tenths, Outcome::Closed { shells }))
        })
        .collect();
    assert_eq!(found, expected);
}

/// Each loft and union goes to the part that answers it: a strip is the engine's own rows, Exact;
/// the union of two flat-faced strips is the kernel's geom-bool, as exact as the strips.
#[test]
fn each_loft_and_union_goes_to_the_part_that_answers_it() {
    let mut e = engine();
    let a = suite::strip(&mut e, (0, 0), (10_000, 0), 0, (0, 1), 2).expect("a strip");
    assert_eq!(a.made_by(), BY_LOFT);
    assert_eq!(a.exactness(), Exactness::Exact);
    let b = suite::strip(&mut e, (10_000, 0), (20_000, 0), 0, (0, 1), 2).expect("a strip");
    let u = e.unite(&a, &b).expect("the strips united");
    assert_eq!(u.made_by(), BY_BOOL);
    assert_eq!(Some(u.exactness()), a.exactness().combine(b.exactness()));
}

/// A loft whose walls cross (the second section half a turn round from the first, each wall a
/// flat crossed quadrilateral through the strip's axis) is refused as it is written: each crossed
/// wall's two halves turn opposite ways and cancel, so the wall has no area and no plane faces
/// out of it. An error, never a solid.
#[test]
fn a_crossed_loft_is_refused_for_walls_with_no_area() {
    let mut e = engine();
    let square = |x: i64| {
        vec![
            at(x, 0, 0),
            at(x, 2000, 0),
            at(x, 2000, 2000),
            at(x, 0, 2000),
        ]
    };
    let mut turned = square(10_000);
    turned.rotate_left(2);
    let found = e.loft(&[Section { corners: square(0) }, Section { corners: turned }]);
    assert!(
        matches!(
            &found,
            Err(EngineError::Refused { operation: "loft", reason })
                if reason == "a face with no area"
        ),
        "{:?}",
        found.err()
    );
}

/// A wall whose four corners are not on one plane is not one this engine makes yet: the kernel
/// has no kind for a ruled surface, so the loft is refused as unsupported before solidcore is
/// reached, never handed on to come back as a body the conversion cannot write.
#[test]
fn a_loft_wall_that_is_not_flat_is_unsupported() {
    let mut e = engine();
    let first = Section {
        corners: vec![
            at(0, 0, 0),
            at(0, 2000, 0),
            at(0, 2000, 2000),
            at(0, 0, 2000),
        ],
    };
    let second = Section {
        corners: vec![
            at(10_000, 0, 0),
            at(10_000, 2000, 0),
            at(10_000, 2000, 3000),
            at(10_000, 0, 2000),
        ],
    };
    let found = e.loft(&[first, second]);
    assert!(
        matches!(
            &found,
            Err(EngineError::Unsupported { operation: "loft", what })
                if what.starts_with("a wall whose four corners are not on one plane")
        ),
        "{:?}",
        found.err()
    );
}

/// Each extrusion goes to the part that answers it. The wall drawn in plan, no holes, level and
/// swept straight up, is the kernel's own exact `make.prism`, Exact. The wall drawn in elevation
/// (swept along y), the profile with a hole, and an L-shaped plan swept up, which `make.prism`
/// refuses as not strictly convex, are solidcore's linear sweep, Certified; the L's volume is
/// still exactly its area times its height.
#[test]
fn each_extrusion_goes_to_the_part_that_answers_it() {
    let mut e = engine();
    let plan = suite::wall(&mut e, Drawn::InPlan);
    assert_eq!(plan.made_by(), BY_PRISM);
    assert_eq!(plan.exactness(), Exactness::Exact);
    let elevation = suite::wall(&mut e, Drawn::InElevation);
    assert_eq!(elevation.made_by(), BY_SWEEP);
    assert!(matches!(elevation.exactness(), Exactness::Certified { .. }));
    let ell = Profile {
        outer: vec![
            at(0, 0, 0),
            at(1000, 0, 0),
            at(1000, 200, 0),
            at(200, 200, 0),
            at(200, 1000, 0),
            at(0, 1000, 0),
        ],
        holes: vec![],
    };
    let l = e.extrude(&ell, [0, 0, mm(2700)]).expect("the L");
    assert_eq!(l.made_by(), BY_SWEEP);
    let six = |mm3: i128| I256::from_i128(6 * mm3 * i128::from(mm(1)).pow(3));
    assert_eq!(
        e.volume(&l).expect("its volume"),
        Volume::SixTimes {
            cubic_steps: six((1000 * 200 + 200 * 800) * 2700),
            exactness: l.exactness(),
        }
    );
}

/// The door is cut by the kernel's geom-bool from either wall, the kernel's prism or solidcore's
/// sweep, and the result is Exact from the prism and as exact as the sweep from the sweep.
#[test]
fn the_door_is_cut_by_geom_bool_from_either_wall() {
    let mut e = engine();
    for drawn in suite::BOTH {
        let wall = suite::wall(&mut e, drawn);
        let cut = suite::wall_with_door(&mut e, drawn);
        assert_eq!(cut.made_by(), BY_BOOL, "{drawn:?}");
        assert_eq!(cut.exactness(), wall.exactness(), "{drawn:?}");
    }
}

/// The engine names itself: solidcore at the commit the crate's Cargo.toml pins for every
/// solidcore crate, the grid step it works on, solidcore's tolerances in metres (a zone of 1e-5 m,
/// the 0.01 mm of the model's tolerance), and which part answers each operation: the subtraction
/// is the kernel's geom-bool, the loft solidcore's skin, and the union the kernel's geom-bool for
/// flat-faced solids and solidcore's union for curved ones.
#[test]
fn the_engine_names_solidcore_and_its_pin() {
    let ed = engine().edition();
    assert_eq!(ed.engine, "geom-solidcore");
    assert_eq!(ed.version, SOLIDCORE);
    assert_eq!(ed.step, STEP);
    let pin = SOLIDCORE.rsplit(' ').next().expect("a commit");
    assert_eq!(pin.len(), 40);
    let rev = format!("rev = \"{pin}\"");
    let lines: Vec<&str> = include_str!("../Cargo.toml")
        .lines()
        .filter(|l| l.starts_with("solidcore"))
        .collect();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines.iter().all(|l| l.contains(&rev)), "{lines:?}");
    let zone = ed
        .tolerances
        .iter()
        .find(|t| t.name == "zone_tol_3d")
        .expect("the zone tolerance");
    assert_eq!((zone.value, zone.unit.as_str()), (1e-5, "m"));
    let by = |op: &str| -> Vec<&str> {
        ed.answers
            .iter()
            .filter(|a| a.operation == op)
            .map(|a| a.by.as_str())
            .collect()
    };
    assert_eq!(by("extrude"), [BY_PRISM, BY_SWEEP]);
    assert_eq!(by("subtract"), [BY_BOOL]);
    assert_eq!(by("loft"), [BY_LOFT]);
    assert_eq!(by("unite"), [BY_BOOL, BY_UNION]);
    assert!(by("volume").iter().all(|b| b.contains("six-volume")));
    assert!(by("mesh").iter().all(|b| b.contains("solidcore-tess")));
    assert_eq!(by("section"), [BY_CUT]);
}

/// Whole millimetres become steps exactly, once, at the program's seam: 1 mm is 6,400 steps of
/// `Q0`, and the wall's 4000, 2700 and 200 mm are whole numbers of steps; solidcore is handed
/// them in metres, the f64 nearest each.
#[test]
fn whole_millimetres_become_steps_exactly() {
    assert_eq!(mm(1), 6_400);
    assert_eq!(
        [mm(4000), mm(2700), mm(200)],
        [25_600_000, 17_280_000, 1_280_000]
    );
    let metres = |steps: i64| geom_solidcore::convert::to_metres(steps, STEP);
    assert_eq!(
        [metres(mm(4000)), metres(mm(2700)), metres(mm(200))],
        [4.0, 2.7, 0.2]
    );
}

/// solidcore's own plane section is the oracle of the engine's exact cut, on the walls solidcore's
/// sweep makes (drawn in elevation, whole and with the door as a hole of the profile), cut 1000 mm
/// up: it gives as many curves as the cut has edges, and each curve's two ends are an edge's two
/// ends within one step (solidcore's curves are doubles in metres, read back in steps). The
/// engine's answer is exact; solidcore's agrees within its floating point.
#[test]
fn the_cut_agrees_with_solidcore_s_plane_section() {
    use geom_solidcore::{Corner, Hole, Modeller};
    let xz = |x0: i64, z0: i64, x1: i64, z1: i64| {
        vec![at(x0, 0, z0), at(x1, 0, z0), at(x1, 0, z1), at(x0, 0, z1)]
    };
    let corners = |ps: &[geom_num::P3]| ps.iter().map(|&p| Corner::from(p)).collect::<Vec<_>>();
    for holes in [vec![], vec![xz(1550, 300, 2450, 2400)]] {
        let mut e = engine();
        let profile = Profile {
            outer: xz(0, 0, 4000, 2700),
            holes: holes.clone(),
        };
        let wall = e.extrude(&profile, [0, mm(200), 0]).expect("the wall");
        assert_eq!(wall.made_by(), BY_SWEEP);
        let cut = e
            .section(&wall, &suite::axis_plane(2, 1000, true))
            .expect("the cut");
        let ours: Vec<[[f64; 3]; 2]> = suite::loops_of(&cut)
            .iter()
            .flatten()
            .map(|(a, b, _)| {
                let f = |p: &geom_op::engine::CutPoint| p.num.map(|c| c as f64 / p.den as f64);
                [f(a), f(b)]
            })
            .collect();
        let mut m = Modeller::new(STEP);
        let theirs = geom_solidcore::Profile {
            outer: corners(&profile.outer),
            holes: holes.iter().map(|h| Hole::Polygon(corners(h))).collect(),
        };
        let s = m
            .extrude(&theirs, [0, mm(200), 0])
            .expect("solidcore's wall");
        let pieces = m
            .plane_section(&s, at(0, 0, 1000), [0.0, 0.0, 1.0])
            .expect("solidcore's section");
        assert_eq!(pieces.len(), ours.len(), "holes {holes:?}");
        let near = |p: [f64; 3], q: [f64; 3]| p.iter().zip(q).all(|(a, b)| (a - b).abs() <= 1.0);
        for piece in &pieces {
            let (t0, t1) = piece.span();
            let a = piece.point_at(&m, t0).expect("a point");
            let b = piece.point_at(&m, t1).expect("a point");
            assert!(
                ours.iter()
                    .any(|[p, q]| (near(a, *p) && near(b, *q)) || (near(a, *q) && near(b, *p))),
                "solidcore's curve {a:?} to {b:?} is no edge of the cut {ours:?}"
            );
        }
    }
}

/// The methods of the trait `Engine`, read from its declaration in geom-op.
fn engine_operations() -> BTreeSet<String> {
    let source = include_str!("../../crates/geom-op/src/engine.rs");
    let body = source
        .split("pub trait Engine {")
        .nth(1)
        .expect("the trait's declaration");
    body.lines()
        .filter_map(|l| l.trim().strip_prefix("fn "))
        .filter_map(|l| l.split(['(', '<']).next())
        .map(str::to_owned)
        .collect()
}

/// The interim register has a row for each operation of the trait `Engine` and each case of its
/// inputs that a different part answers, and nothing else: what answers it today, the test that
/// pins that answer (a test of this file), its known faults (each an id of
/// `tests/named-faults.json`), the tolerance it works to and the kernel piece or chapter that will
/// make it exact. The edition's answers are the register's rows but `edition`, word for word.
#[test]
fn every_engine_operation_has_a_row_in_the_interim_register() {
    let register: Value = serde_json::from_str(include_str!("interim.json")).expect("interim.json");
    let rows = register["operations"].as_array().expect("its operations");
    let text = |row: &Value, field: &str| -> String {
        let t = row[field].as_str().unwrap_or_default().to_owned();
        assert!(!t.is_empty(), "a row without its {field}: {row}");
        t
    };
    let named: Value =
        serde_json::from_str(include_str!("named-faults.json")).expect("named-faults.json");
    let fault_ids: BTreeSet<String> = named["faults"]
        .as_array()
        .expect("the named faults")
        .iter()
        .filter_map(|f| f["fault"].as_str())
        .filter_map(|f| f.split([',', ' ']).next())
        .map(str::to_owned)
        .collect();
    let tests = include_str!("engine.rs");
    let ed = engine().edition();
    let mut seen = BTreeSet::new();
    let mut registered = BTreeSet::new();
    for row in rows {
        let op = text(row, "operation");
        let case = text(row, "case");
        assert!(
            registered.insert((op.clone(), case.clone())),
            "two rows for {op}, {case}"
        );
        seen.insert(op.clone());
        let by = text(row, "answered_by");
        let test = text(row, "pinned_by");
        assert!(tests.contains(&format!("fn {test}()")), "no test {test}");
        text(row, "tolerance");
        text(row, "exact_by");
        for id in row["known_faults"].as_array().expect("its known faults") {
            let id = id.as_str().expect("a fault's id");
            assert!(fault_ids.contains(id), "{op}: no named fault {id}");
        }
        if op == "edition" {
            registered.remove(&(op, case));
        } else {
            let said = ed
                .answers
                .iter()
                .find(|a| a.operation == op && a.case == case)
                .map(|a| a.by.as_str());
            assert_eq!(
                said,
                Some(by.as_str()),
                "{op}, {case}: the edition answers otherwise"
            );
        }
    }
    assert_eq!(seen, engine_operations());
    let answered: BTreeSet<(String, String)> = ed
        .answers
        .iter()
        .map(|a| (a.operation.clone(), a.case.clone()))
        .collect();
    assert_eq!(answered, registered);
}

/// A loft over ground that bends between stations by less than half a degree, which solidcore's
/// skin joined into one bent face the kernel could not take in (the replay of a real site found it
/// for every loft of more than two stations), is one closed solid of one face per wall.
#[test]
fn a_loft_whose_walls_bend_less_than_half_a_degree_is_one_face_per_wall() {
    suite::a_loft_whose_walls_bend_less_than_half_a_degree_is_one_face_per_wall(engine());
}

/// A section two of whose sides turn by less than half a degree, which solidcore's skin joined
/// into one bent face (the replay found it for every strip of a void whose last side is carried on
/// along its slope and its corners then put on the grid), lofts to one face per side.
#[test]
fn a_section_whose_sides_turn_less_than_half_a_degree_lofts_to_one_face_per_side() {
    suite::a_section_whose_sides_turn_less_than_half_a_degree_lofts_to_one_face_per_side(engine());
}

#[test]
fn a_loft_over_uneven_ground_is_its_profile_times_its_length() {
    suite::a_loft_over_uneven_ground_is_its_profile_times_its_length(engine());
}

#[test]
fn the_same_loft_gives_the_same_bytes() {
    suite::the_same_loft_gives_the_same_bytes(engine);
}

/// Two solids that touch only along an edge, as two envelopes cut at a corner of the site meet
/// along its vertical line (the replay of a real site found it for the four of ten unions of its
/// cut envelopes that were refused), or only at a point, have no solid union: the kernel's
/// geom-bool makes the exact non-manifold body, and the engine refuses it saying so.
#[test]
fn a_union_of_solids_touching_only_along_an_edge_or_at_a_point_is_refused_by_name() {
    use suite::Touch;
    let mut e = engine();
    let edge = suite::united_touching(&mut e, Touch::AlongAnEdge);
    assert_eq!(
        edge,
        EngineError::Refused {
            operation: "unite",
            reason: "the result is not a solid: solids in it touch along an edge without sharing a face there (1 edges where more than two faces meet), so it is non-manifold, which no solid is".to_owned(),
        }
    );
    let point = suite::united_touching(&mut e, Touch::AtAPoint);
    assert_eq!(
        point,
        EngineError::Refused {
            operation: "unite",
            reason: "the result is not a solid: solids in it touch at a point (a vertex where the faces meet in more than one fan), so it is non-manifold, which no solid is".to_owned(),
        }
    );
}

#[test]
fn a_strip_cut_in_two_leaves_two_pieces_in_order() {
    suite::a_strip_cut_in_two_leaves_two_pieces_in_order(engine());
}
