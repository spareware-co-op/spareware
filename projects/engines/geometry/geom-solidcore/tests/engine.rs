//! geom-solidcore's engine through the engine suite (`tests/suite`), the cases every engine of the
//! seam is judged by, and what is this engine's own: its edition, the one conversion of
//! millimetres at a program's seam, and the interim register (`tests/interim.json`), which names
//! for each operation of the seam what answers it today and what will make it exact.

mod suite;

use std::collections::BTreeSet;

use geom_num::{Exactness, I256};
use geom_op::engine::{Engine, Profile, Volume};
use geom_solidcore::engine::{
    BY_BOOL, BY_PRISM, BY_SWEEP, EngineSolid, SOLIDCORE, SolidcoreEngine,
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
/// is the kernel's geom-bool.
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
    assert!(by("volume").iter().all(|b| b.contains("six-volume")));
    assert!(by("mesh").iter().all(|b| b.contains("solidcore-tess")));
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
