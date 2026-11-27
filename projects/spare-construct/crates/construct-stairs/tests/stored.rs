//! A stair's record and its recipe in their one spelling: written, read back the same, anything
//! else refused; the recipe the builders take once its rise is read; and what it reads back as
//! parameters.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, mm, settings};
use construct_levels::Tie;
use construct_model::geometry::Recipe;
use construct_params::{Length, Slope, Value};
use construct_stairs::{
    Direction, LandingSpec, MATERIAL, RunSpec, Stair, StairError, Turn, filled, is_stair,
    is_stair_recipe, layout, read_only, rise_of, stair_of, stair_recipe, stair_record, with_stair,
};
use kit_core::Id;
use serde_json::json;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

/// An L stair at (1000, 2000) going +y, 1100 mm wide, its first run of 8 risers and its second
/// taking the rest, a 1200 mm landing turning left.
fn sample() -> Stair {
    let mut s = settings("common");
    s.start = [mm(1000), mm(2000)];
    s.direction = Direction::PlusY;
    s.width = mm(1100);
    s.runs = vec![RunSpec { risers: Some(8) }, RunSpec::default()];
    s.landings = vec![LandingSpec {
        length: mm(1200),
        turn: Turn::Left,
    }];
    Stair::new(s).unwrap()
}

/// The recipe holds the stair's settings in construct-params' stored spellings and reads its
/// rise from its levels; it reads back as the same stair.
#[test]
fn a_stairs_recipe_reads_back_as_the_same_stair() {
    let recipe = stair_recipe(&sample());
    assert_eq!((recipe.op.as_str(), recipe.op_version), ("stair", 1));
    assert!(is_stair_recipe(&recipe));
    assert_eq!(
        serde_json::to_string(&recipe.args).unwrap(),
        r#"{"category":"common","direction":"+y","landings":[{"length":"length:q0:7680000","turn":"left"}],"rise":{"tie":"top"},"rules":"nz-d1-as1","runs":[{"risers":8},{}],"start":["length:q0:6400000","length:q0:12800000"],"waist":"length:q0:960000","width":"length:q0:7040000"}"#
    );
    assert_eq!(stair_of(&recipe).unwrap(), sample());
    assert_eq!(rise_of(&recipe).unwrap(), None);
    // A going set is stored; none leaves the category's least.
    let mut s = sample().settings();
    s.going = Some(mm(300));
    let recipe = stair_recipe(&Stair::new(s).unwrap());
    assert_eq!(recipe.args["going"], json!("length:q0:1920000"));
    assert_eq!(stair_of(&recipe).unwrap().going(), Some(mm(300)));
}

/// The record is of the stairs category, its base and its top tied to levels, at the origin.
#[test]
fn a_stairs_record_ties_its_base_and_top() {
    let base = Tie {
        level: id("L-1"),
        offset: mm(0),
    };
    let top = Tie {
        level: id("L-2"),
        offset: mm(-50),
    };
    let record = stair_record(&id("S-1"), &id("DOC"), "Stair 1", &base, &top, &sample());
    assert!(is_stair(&record));
    assert_eq!(record.kind, "stair");
    assert_eq!(
        serde_json::to_string(&record.props).unwrap(),
        r#"{"@category":"800","@level":"L-1","@level.offset":"length:q0:0","@top.level":"L-2","@top.offset":"length:q0:-320000"}"#
    );
    assert_eq!(stair_of(&record.recipe).unwrap(), sample());
    let mut s = sample().settings();
    s.width = mm(1200);
    let wider = with_stair(record.clone(), &Stair::new(s).unwrap()).unwrap();
    assert_eq!(wider.props, record.props);
    assert_eq!(stair_of(&wider.recipe).unwrap().width(), mm(1200));
    let mut level = record;
    level.kind = "level".to_owned();
    assert!(matches!(
        with_stair(level, &sample()),
        Err(StairError::NotAStair(_))
    ));
}

/// A recipe not exactly as written is refused, never repaired.
#[test]
fn a_recipe_not_as_written_is_refused() {
    let base = stair_recipe(&sample());
    let refused = |change: &dyn Fn(&mut Recipe)| {
        let mut r = base.clone();
        change(&mut r);
        stair_of(&r).unwrap_err()
    };
    let malformed = |e: StairError| matches!(e, StairError::Malformed(_));
    assert!(malformed(refused(&|r| {
        r.args.insert("width".to_owned(), json!("length:1100"));
    })));
    assert!(malformed(refused(&|r| {
        r.args.insert("extra".to_owned(), json!(1));
    })));
    assert!(malformed(refused(&|r| {
        r.args.insert("direction".to_owned(), json!("north"));
    })));
    assert!(malformed(refused(&|r| {
        r.args.insert("rise".to_owned(), json!(2700));
    })));
    assert!(malformed(refused(&|r| {
        r.args
            .insert("runs".to_owned(), json!([{ "risers": 8, "x": 1 }, {}]));
    })));
    assert!(malformed(refused(&|r| {
        r.args.remove("waist");
    })));
    assert_eq!(
        refused(&|r| {
            r.args
                .insert("runs".to_owned(), json!([{ "risers": 1 }, {}]));
        }),
        StairError::FewRisers { run: 1, risers: 1 }
    );
    assert_eq!(
        refused(&|r| {
            r.args.insert("landings".to_owned(), json!([]));
        }),
        StairError::LandingCount {
            runs: 2,
            landings: 0
        }
    );
    assert!(matches!(
        refused(&|r| r.op_version = 2),
        StairError::NotAStair(_)
    ));
}

/// Every check of a stair's own shape, in words.
#[test]
fn a_stairs_own_shape_is_checked() {
    let new = |f: &dyn Fn(&mut construct_stairs::Settings)| {
        let mut s = settings("common");
        f(&mut s);
        Stair::new(s)
    };
    assert_eq!(
        new(&|s| s.runs = Vec::new()).unwrap_err(),
        StairError::NoRuns
    );
    assert_eq!(
        new(&|s| s.width = mm(0)).unwrap_err().to_string(),
        "The width must be more than nothing."
    );
    assert_eq!(
        new(&|s| s.waist = mm(-1)).unwrap_err().to_string(),
        "The waist must be more than nothing."
    );
    assert!(matches!(
        new(&|s| s.width = mm(100_001)).unwrap_err(),
        StairError::OutOfRange { .. }
    ));
    assert_eq!(
        new(&|s| {
            s.runs = vec![RunSpec::default(); 2];
            s.landings = vec![LandingSpec {
                length: mm(900),
                turn: Turn::Left,
            }];
        })
        .unwrap_err()
        .to_string(),
        "Landing 1 turns, and is 900 mm long: a landing that turns is at least as long as the stair is wide (1000 mm), so the next run fits beside it."
    );
    assert!(matches!(
        new(&|s| {
            s.runs = vec![RunSpec { risers: Some(600) }; 2];
            s.landings = vec![LandingSpec {
                length: mm(1000),
                turn: Turn::Straight,
            }];
        })
        .unwrap_err(),
        StairError::OutOfRange { .. }
    ));
}

/// Filled with its rise, the recipe the builders take holds the material and each piece, in grid
/// steps: a run's treads, plan outline and the two sections of its loft, a landing's outline
/// extruded by its waist.
#[test]
fn the_filled_recipe_holds_each_piece() {
    let mut recipe = stair_recipe(&sample());
    recipe
        .args
        .insert("rise".to_owned(), json!(Value::Length(mm(2700)).stored()));
    assert_eq!(rise_of(&recipe).unwrap(), Some(mm(2700)));
    let built = filled(&recipe).unwrap();
    assert_eq!(
        built.args.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["material", "pieces"]
    );
    assert_eq!(built.args["material"], json!(MATERIAL));
    let pieces = built.args["pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 3);
    assert_eq!(pieces[0]["id"], json!("run.1"));
    assert_eq!(pieces[0]["treads"], json!(7));
    let loft = pieces[0]["loft"].as_array().unwrap();
    assert_eq!(loft.len(), 2);
    assert_eq!(
        loft[0].as_array().unwrap().len(),
        loft[1].as_array().unwrap().len()
    );
    assert_eq!(pieces[1]["id"], json!("landing.1"));
    assert_eq!(pieces[1]["by"], json!([0, 0, 150 * MM]));
    assert_eq!(pieces[1]["extrude"].as_array().unwrap().len(), 4);
    assert_eq!(pieces[2]["treads"], json!(6));
    // A recipe whose rise is not read yet is not built.
    assert!(matches!(
        filled(&stair_recipe(&sample())),
        Err(StairError::Malformed(_))
    ));
    // The material is one the default library holds.
    let library = construct_materials::Materials::default_metric();
    assert!(
        library
            .get(&construct_materials::MaterialId::parse(MATERIAL).unwrap())
            .is_some()
    );
}

/// What a stair reads back: a common stairway rising 2500 mm has 14 risers, the tallest
/// 1,142,858 steps, 13 treads of 280 mm, a total run of 3640 mm, a pitch of that riser over the
/// going, and its width.
#[test]
fn a_stair_reads_back_its_quantities() {
    let mut s = settings("common");
    s.runs = vec![RunSpec::default()];
    let l = layout(&Stair::new(s).unwrap(), mm(2500)).unwrap();
    let rows: Vec<(String, Option<Value>)> = read_only(&l)
        .unwrap()
        .into_iter()
        .map(|(def, value)| (def.id().to_string(), value))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("stair.rise".to_owned(), Some(Value::Length(mm(2500)))),
            ("stair.risers".to_owned(), Some(Value::Whole(14))),
            (
                "stair.riser-height".to_owned(),
                Some(Value::Length(Length::from_steps(1_142_858)))
            ),
            ("stair.treads".to_owned(), Some(Value::Whole(13))),
            ("stair.going".to_owned(), Some(Value::Length(mm(280)))),
            ("stair.run-length".to_owned(), Some(Value::Length(mm(3640)))),
            (
                "stair.pitch".to_owned(),
                Some(Value::Slope(Slope::new(1_142_858, 280 * MM).unwrap()))
            ),
            ("stair.width".to_owned(), Some(Value::Length(mm(1000)))),
        ]
    );
}
