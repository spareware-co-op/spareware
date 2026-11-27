//! Columns as data: sections and their rings, types, the record and its recipe, positions on
//! grids that follow the grids, heights between levels, quantities and the read-only parameters.
//! Every number below is worked by hand in the comment above it.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{COLUMNS, CategoryTable, FLOORS};
use construct_columns::{
    AREA, Column, ColumnError, ColumnSpec, HEIGHT, MASS, MOST_HEIGHT, MOST_SIDE, Position,
    ROUND_SIDES, Section, Top, VOLUME, check_height, column_of, column_recipe, column_record,
    default_column_types, default_columns, filled_by_type, finish, grids_read, height_between,
    new_column_type, quantities, read_only, reads_section, spec_of, with_column, with_spec,
};
use construct_grids::{Datums, Grid, Point, Shape};
use construct_levels::{Level, Tie};
use construct_materials::{MaterialId, Materials};
use construct_params::{Angle, Area, Decimal, Length, Value, Volume};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn mm(mm: i64) -> Length {
    Length::checked_from_mm(mm).unwrap()
}

fn degrees(d: i64) -> Angle {
    Angle::from_units(d * Angle::UNITS_PER_DEGREE)
}

fn concrete() -> MaterialId {
    MaterialId::parse("concrete-regular").unwrap()
}

fn square_400() -> ColumnSpec {
    ColumnSpec {
        section: Section::rectangular(mm(400), mm(400)).unwrap(),
        material: concrete(),
    }
}

fn level(name: &str, elevation_mm: i64) -> Level {
    Level::new(id(name), name, mm(elevation_mm), true).unwrap()
}

fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> Shape {
    Shape::line(Point::new(mm(x0), mm(y0)), Point::new(mm(x1), mm(y1))).unwrap()
}

/// A 3 by 2 grid: vertical lines `g1`, `g2`, `g3` at x = 0, 6000 and 12000 mm, horizontal lines
/// `ga` and `gb` at y = 0 and 5000 mm, each running 1 m past the last crossing.
fn grid_3_by_2() -> Datums {
    let mut datums = Datums::new();
    for (name, x) in [("g1", 0), ("g2", 6000), ("g3", 12000)] {
        datums
            .add_grid(Grid::new(id(name), name, line(x, -1000, x, 6000)).unwrap())
            .unwrap();
    }
    for (name, y) in [("ga", 0), ("gb", 5000)] {
        datums
            .add_grid(Grid::new(id(name), name, line(-1000, y, 13000, y)).unwrap())
            .unwrap();
    }
    datums
}

fn on_grids(first: &str, second: &str) -> Position {
    Position::Grids {
        first: id(first),
        second: id(second),
        cross: 0,
    }
}

fn filled(
    column: &Column,
    spec: &ColumnSpec,
    height: Length,
    datums: &Datums,
) -> serde_json::Value {
    let recipe = column_recipe(column);
    assert!(reads_section(&recipe));
    let mut recipe = filled_by_type(&recipe, spec).unwrap();
    recipe.args.insert(
        "height".to_owned(),
        serde_json::Value::from(Value::Length(height).stored()),
    );
    let done = finish(&recipe, datums).unwrap();
    serde_json::to_value(&done.args).unwrap()
}

/// A 400 by 400 mm column from level 1 (0 mm) to level 2 (3000 mm) is 3000 mm tall: its section is
/// 0.4 m by 0.4 m = 160,000 mm2 and its volume 160,000 mm2 x 3000 mm = 480,000,000 mm3
/// (0.48 m3), each exact in grid steps.
#[test]
fn a_400_by_400_column_between_two_levels_holds_480_000_000_mm3() {
    let (one, two) = (level("l1", 0), level("l2", 3000));
    let height = height_between((&one, mm(0)), (&two, mm(0))).unwrap();
    assert_eq!(height, mm(3000));
    let q = quantities(&square_400(), height, &Materials::default_metric()).unwrap();
    assert_eq!(q.height, mm(3000));
    assert_eq!(q.area, Area::from_mm2(160_000));
    assert_eq!(q.volume, Volume::from_mm3(480_000_000));
    // The mass is the library's one function on that volume of concrete.
    let library = Materials::default_metric();
    assert_eq!(
        q.mass_grams,
        Some(library.mass_grams(&concrete(), q.volume).unwrap())
    );
}

/// Offsets move the base and the top: a base 100 mm above level 1 and a top 50 mm below level 2
/// stand 3000 - 100 - 50 = 2850 mm. A level moved up 200 mm makes it 3050 mm.
#[test]
fn offsets_and_a_moved_level_change_the_height() {
    let one = level("l1", 0);
    let mut two = level("l2", 3000);
    assert_eq!(
        height_between((&one, mm(100)), (&two, mm(-50))).unwrap(),
        mm(2850)
    );
    two = level("l2", 3200);
    assert_eq!(
        height_between((&one, mm(100)), (&two, mm(-50))).unwrap(),
        mm(3050)
    );
}

/// A top at its base has no height; a top below its base is refused by name and says how far below;
/// a height past 100 m is refused.
#[test]
fn a_top_below_its_base_is_refused_by_name() {
    let (one, low) = (level("l1", 3000), level("l0", 1000));
    assert_eq!(
        height_between((&one, mm(0)), (&one, mm(0))),
        Err(ColumnError::NoHeight)
    );
    let below = height_between((&one, mm(0)), (&low, mm(0))).unwrap_err();
    assert_eq!(below, ColumnError::TopBelowBase { by: mm(2000) });
    assert_eq!(below.code(), "TopBelowBase");
    assert_eq!(
        below.to_string(),
        "the column's top is 2000 mm below its base: a column stands up from its base"
    );
    assert_eq!(check_height(mm(100_000)), Ok(mm(100_000)));
    assert!(matches!(
        check_height(mm(100_001)),
        Err(ColumnError::BadSize(_))
    ));
    assert_eq!(MOST_HEIGHT, mm(100_000));
}

/// Sizes: above zero, whole even steps, at most 5 m.
#[test]
fn sizes_are_checked() {
    assert!(Section::rectangular(mm(0), mm(400)).is_err());
    assert!(Section::rectangular(mm(400), mm(-1)).is_err());
    assert!(
        Section::round(Length::from_steps(2_559_999)).is_err(),
        "odd steps"
    );
    assert!(Section::rectangular(mm(5001), mm(400)).is_err());
    assert!(Section::round(MOST_SIDE).is_ok());
    assert!(Section::round(Length::from_steps(2_560_000)).is_ok());
    let odd = Section::round(Length::from_steps(2_559_999)).unwrap_err();
    assert!(odd.to_string().contains("even number of grid steps"));
}

/// An unturned 400 by 200 mm section about (1000, 2000) mm has its corners at x = 800 and 1200,
/// y = 1900 and 2100, anticlockwise from the lower left; turned a quarter it is 200 by 400 about
/// the same centre, exactly, and turned a half it has the same corners from the opposite one.
#[test]
fn a_rectangle_turns_by_quarters_exactly() {
    let section = Section::rectangular(mm(400), mm(200)).unwrap();
    let centre = [mm(1000).steps(), mm(2000).steps()];
    let at = |x: i64, y: i64| [mm(x).steps(), mm(y).steps()];
    assert_eq!(
        section.ring(centre, degrees(0)).unwrap(),
        vec![at(800, 1900), at(1200, 1900), at(1200, 2100), at(800, 2100)]
    );
    assert_eq!(
        section.ring(centre, degrees(90)).unwrap(),
        vec![at(1100, 1800), at(1100, 2200), at(900, 2200), at(900, 1800)]
    );
    assert_eq!(
        section.ring(centre, degrees(180)).unwrap(),
        vec![at(1200, 2100), at(800, 2100), at(800, 1900), at(1200, 1900)]
    );
    assert_eq!(
        section.ring(centre, degrees(-90)).unwrap(),
        section.ring(centre, degrees(270)).unwrap()
    );
}

/// A 400 by 400 mm section turned 30 degrees: the lower left corner (-200, -200) mm goes to
/// x = -200 cos 30 + 200 sin 30 = -200 x 0.8660254038 + 100 = -73.2050808 mm = -468,512.517 steps
/// and y = -200 sin 30 - 200 cos 30 = -100 - 173.2050808 = -273.2050808 mm = -1,748,512.517
/// steps, each rounded once: -468,513 and -1,748,513 (a half or more rounds away from zero).
#[test]
fn a_turn_by_any_angle_rounds_each_corner_once() {
    let section = Section::rectangular(mm(400), mm(400)).unwrap();
    let ring = section.ring([0, 0], degrees(30)).unwrap();
    assert_eq!(ring[0], [-468_513, -1_748_513]);
    // The four corners are a quarter turn apart about the centre: the second is the first turned
    // 90 degrees, (x, y) -> (-y, x), up to the one rounding.
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        assert!(
            (b[0] + a[1]).abs() <= 1 && (b[1] - a[0]).abs() <= 1,
            "{a:?} {b:?}"
        );
    }
    // Its area is still the square's, to within the rounding of its corners.
    let twice = construct_columns::twice_area(&ring).unwrap();
    let exact = 2 * 160_000_i128 * Area::SQUARE_STEPS_PER_MM2;
    assert!(
        (twice - exact).abs() < 4 * 1_300_000 * 6400,
        "twice area {twice}"
    );
}

/// A round section of 450 mm is a polygon of 48 corners on its circle (the interim): each corner
/// within a step of 225 mm from the centre, opposite corners opposite, and its area 48/2 x r^2 x
/// sin(7.5 degrees) = 24 x 50,625 x 0.1305262 = 158,589 mm2 (0.28 per cent under the circle's
/// pi r^2 = 159,043 mm2).
#[test]
fn a_round_section_is_a_polygon_of_48_corners() {
    assert_eq!(ROUND_SIDES, 48);
    let section = Section::round(mm(450)).unwrap();
    let ring = section.ring([0, 0], degrees(0)).unwrap();
    assert_eq!(ring.len(), 48);
    assert_eq!(ring[0], [mm(225).steps(), 0]);
    assert_eq!(ring[12], [0, mm(225).steps()]);
    let r = mm(225).steps();
    for corner in &ring {
        let d2 = i128::from(corner[0]).pow(2) + i128::from(corner[1]).pow(2);
        let r2 = i128::from(r).pow(2);
        assert!((d2 - r2).abs() <= 2 * i128::from(r) + 2, "{corner:?}");
    }
    for k in 0..24 {
        assert_eq!(ring[k][0], -ring[k + 24][0]);
        assert_eq!(ring[k][1], -ring[k + 24][1]);
    }
    let area = section.area().unwrap();
    let mm2 = area.square_steps() / Area::SQUARE_STEPS_PER_MM2;
    assert_eq!(mm2, 158_589);
    assert!(area < Area::from_mm2(159_043));
}

/// The columns of a 3 by 2 grid: six crossings, g1 x ga at (0, 0) ... g3 x gb at (12000, 5000).
/// Move grid g2 500 mm along x and its two columns follow; the other four stay where they were.
#[test]
fn columns_on_a_3_by_2_grid_follow_a_moved_grid_line() {
    let mut datums = grid_3_by_2();
    let spec = square_400();
    let place = |datums: &Datums, v: &str, h: &str| {
        let column = Column {
            position: on_grids(v, h),
            turn: degrees(0),
            top: Top::Height(mm(3000)),
        };
        let args = filled(&column, &spec, mm(3000), datums);
        // The ring's first corner is the lower left: its x and y are the centre less 200 mm.
        let first = args["ring"][0].as_array().unwrap().clone();
        (
            first[0].as_i64().unwrap() + mm(200).steps(),
            first[1].as_i64().unwrap() + mm(200).steps(),
        )
    };
    let mut before = Vec::new();
    for v in ["g1", "g2", "g3"] {
        for h in ["ga", "gb"] {
            before.push(place(&datums, v, h));
        }
    }
    assert_eq!(
        before,
        vec![
            (0, 0),
            (0, mm(5000).steps()),
            (mm(6000).steps(), 0),
            (mm(6000).steps(), mm(5000).steps()),
            (mm(12000).steps(), 0),
            (mm(12000).steps(), mm(5000).steps()),
        ]
    );
    let moved = datums
        .grid(&id("g2"))
        .unwrap()
        .shape()
        .moved(mm(500), mm(0))
        .unwrap();
    datums.reshape_grid(&id("g2"), moved).unwrap();
    let mut after = Vec::new();
    for v in ["g1", "g2", "g3"] {
        for h in ["ga", "gb"] {
            after.push(place(&datums, v, h));
        }
    }
    assert_eq!(after[2], (mm(6500).steps(), 0));
    assert_eq!(after[3], (mm(6500).steps(), mm(5000).steps()));
    for k in [0, 1, 4, 5] {
        assert_eq!(after[k], before[k]);
    }
}

/// Two grids that do not cross (parallel lines) have no column on them: refused by name; a
/// crossing number beyond the crossings there are is refused too; a grid the document lacks is
/// refused.
#[test]
fn grids_that_do_not_cross_hold_no_column() {
    let datums = grid_3_by_2();
    let none = on_grids("g1", "g2").resolve(&datums).unwrap_err();
    assert!(matches!(none, ColumnError::NoCrossing { .. }), "{none:?}");
    assert_eq!(none.code(), "BadGrid");
    let second = Position::Grids {
        first: id("g1"),
        second: id("ga"),
        cross: 1,
    }
    .resolve(&datums)
    .unwrap_err();
    assert!(matches!(
        second,
        ColumnError::NoSuchCrossing {
            wanted: 1,
            found: 1,
            ..
        }
    ));
    assert!(matches!(
        on_grids("g1", "nope").resolve(&datums),
        Err(ColumnError::Grid(_))
    ));
    // A grid moved clear of the other no longer meets it.
    let mut datums = datums;
    let away = datums
        .grid(&id("ga"))
        .unwrap()
        .shape()
        .moved(mm(0), mm(-9000))
        .unwrap();
    datums.reshape_grid(&id("ga"), away).unwrap();
    assert!(matches!(
        on_grids("g1", "ga").resolve(&datums),
        Err(ColumnError::NoCrossing { .. })
    ));
}

/// A column's record holds its type, its base's tie and (for a top at a level) its top's tie; its
/// recipe holds its position, turn and height argument; it reads back exactly, and a changed top
/// changes only those.
#[test]
fn a_column_record_reads_back_as_written() {
    let base = Tie {
        level: id("l1"),
        offset: mm(100),
    };
    let top = Tie {
        level: id("l2"),
        offset: mm(-50),
    };
    let column = Column {
        position: on_grids("g1", "ga"),
        turn: degrees(15),
        top: Top::Level(top.clone()),
    };
    let record = column_record(&id("c1"), &id("doc"), "C1", &id("ty"), &base, &column);
    assert_eq!(record.kind, "column");
    assert_eq!(record.props.get("@category"), Some(&"1100".to_owned()));
    assert_eq!(record.props.get("@type"), Some(&"ty".to_owned()));
    assert_eq!(record.props.get("@level"), Some(&"l1".to_owned()));
    assert_eq!(record.props.get("@top.level"), Some(&"l2".to_owned()));
    assert_eq!(column_of(&record).unwrap(), column);
    assert_eq!(grids_read(&record.recipe), vec![id("g1"), id("ga")]);

    let unconnected = Column {
        position: Position::Point {
            x: mm(1500),
            y: mm(-250),
        },
        turn: degrees(0),
        top: Top::Height(mm(2700)),
    };
    let changed = with_column(record, &unconnected).unwrap();
    assert_eq!(column_of(&changed).unwrap(), unconnected);
    assert!(!changed.props.contains_key("@top.level"));
    assert!(!changed.props.contains_key("@top.offset"));
    assert_eq!(grids_read(&changed.recipe), Vec::<Id>::new());
    assert_eq!(
        changed.props.get("@level.offset").unwrap(),
        &Value::Length(mm(100)).stored()
    );
}

/// The recipe's bytes are pinned: a column at (1000, 2000) mm, turned 90 degrees, 3000 mm tall.
#[test]
fn the_recipe_is_pinned() {
    let column = Column {
        position: Position::Point {
            x: mm(1000),
            y: mm(2000),
        },
        turn: degrees(90),
        top: Top::Height(mm(3000)),
    };
    let recipe = column_recipe(&column);
    assert_eq!(
        serde_json::to_string(&recipe).unwrap(),
        r#"{"owner":"construct","schema":1,"op":"column","op_version":1,"args":{"at":{"point":["length:q0:6400000","length:q0:12800000"]},"height":"length:q0:19200000","section":{"type":"section"},"turn":"angle:810000000"}}"#
    );
}

/// The filled recipe holds the ring, the height, the material and the grids: for the 400 by 400
/// column at the crossing of g2 and gb (6000, 5000) mm.
#[test]
fn the_filled_recipe_is_what_the_builders_take() {
    let datums = grid_3_by_2();
    let column = Column {
        position: on_grids("g2", "gb"),
        turn: degrees(0),
        top: Top::Height(mm(3000)),
    };
    let args = filled(&column, &square_400(), mm(3000), &datums);
    let (cx, cy, h) = (mm(6000).steps(), mm(5000).steps(), mm(200).steps());
    assert_eq!(
        args["ring"],
        serde_json::json!([
            [cx - h, cy - h],
            [cx + h, cy - h],
            [cx + h, cy + h],
            [cx - h, cy + h]
        ])
    );
    assert_eq!(args["height"], Value::Length(mm(3000)).stored());
    assert_eq!(args["material"], "concrete-regular");
    assert_eq!(args["grids"], serde_json::json!(["g2", "gb"]));
    assert_eq!(args.as_object().unwrap().len(), 4);
}

/// Two builds of one input are one byte string; a recipe that reads no type is returned as it is.
#[test]
fn building_is_deterministic() {
    let datums = grid_3_by_2();
    let column = Column {
        position: on_grids("g3", "ga"),
        turn: Angle::from_units(123_456_789),
        top: Top::Height(mm(2400)),
    };
    let a = filled(&column, &square_400(), mm(2400), &datums);
    let b = filled(&column, &square_400(), mm(2400), &datums);
    assert_eq!(a, b);
    let finished = column_recipe(&column);
    let done = filled_by_type(
        &filled_by_type(&finished, &square_400()).unwrap(),
        &square_400(),
    )
    .unwrap();
    assert!(!reads_section(&done));
}

/// A recipe filled by a type but with no height read from its levels is refused, as is a height at
/// or below zero.
#[test]
fn an_unfilled_height_is_refused() {
    let datums = grid_3_by_2();
    let column = Column {
        position: on_grids("g1", "ga"),
        turn: degrees(0),
        top: Top::Level(Tie {
            level: id("l2"),
            offset: mm(0),
        }),
    };
    let recipe = filled_by_type(&column_recipe(&column), &square_400()).unwrap();
    assert!(matches!(
        finish(&recipe, &datums),
        Err(ColumnError::NotAColumn(_))
    ));
    let mut down = recipe.clone();
    down.args.insert(
        "height".to_owned(),
        serde_json::Value::from(Value::Length(mm(-300)).stored()),
    );
    assert_eq!(
        finish(&down, &datums).unwrap_err(),
        ColumnError::TopBelowBase { by: mm(300) }
    );
    let mut none = recipe;
    none.args.insert(
        "height".to_owned(),
        serde_json::Value::from(Value::Length(mm(0)).stored()),
    );
    assert_eq!(finish(&none, &datums).unwrap_err(), ColumnError::NoHeight);
}

/// A column type's text is one spelling and pinned; another spelling or a bad size is refused.
#[test]
fn a_column_type_stores_its_section_once() {
    let spec = square_400();
    let text = spec.to_text().unwrap();
    assert_eq!(
        text,
        r#"{"version":1,"section":{"kind":"rect","width":"length:q0:2560000","depth":"length:q0:2560000"},"material":"concrete-regular"}"#
    );
    assert_eq!(ColumnSpec::from_text(&text).unwrap(), spec);
    let round = ColumnSpec {
        section: Section::round(mm(450)).unwrap(),
        material: concrete(),
    };
    assert_eq!(
        round.to_text().unwrap(),
        r#"{"version":1,"section":{"kind":"round","diameter":"length:q0:2880000"},"material":"concrete-regular"}"#
    );
    assert_eq!(
        ColumnSpec::from_text(&round.to_text().unwrap()).unwrap(),
        round
    );
    for bad in [
        text.replace("\"version\":1", "\"version\":2"),
        text.replace("length:q0:2560000", "length:400"),
        text.replace("length:q0:2560000", "length:q0:2559999"),
        text.replace("length:q0:2560000", "length:q0:0"),
        format!("{text} "),
        text.replace(",\"material\":\"concrete-regular\"", ""),
        text.replace("\"kind\":\"rect\"", "\"kind\":\"oval\""),
    ] {
        assert!(ColumnSpec::from_text(&bad).is_err(), "{bad}");
    }
}

/// A type is of the columns category and keeps its spec in its system block; a type of another
/// category refuses it; a column type without one is named.
#[test]
fn a_column_type_is_an_element_type() {
    let categories = CategoryTable::new();
    let ty = new_column_type(id("ty"), "Concrete 400", &square_400(), &categories).unwrap();
    assert_eq!(ty.category(), COLUMNS);
    assert_eq!(spec_of(&ty).unwrap(), square_400());
    let wider = ColumnSpec {
        section: Section::rectangular(mm(600), mm(300)).unwrap(),
        material: concrete(),
    };
    let ty = with_spec(ty, &wider).unwrap();
    assert_eq!(spec_of(&ty).unwrap(), wider);

    let floor = construct_types::ElementType::new(id("fl"), "Floor", FLOORS, &categories).unwrap();
    assert!(matches!(
        spec_of(&floor),
        Err(ColumnError::NotAColumnType { .. })
    ));
    assert!(matches!(
        with_spec(floor, &wider),
        Err(ColumnError::NotAColumnType { .. })
    ));
    let bare = construct_types::ElementType::new(id("bare"), "Bare", COLUMNS, &categories).unwrap();
    assert!(matches!(
        spec_of(&bare),
        Err(ColumnError::NoColumnSection { .. })
    ));
}

/// Our own three types: a 400 by 400 concrete column, a 450 mm round concrete column and a 140 by
/// 140 timber post; each is a valid type that reads back, with a name of its own.
#[test]
fn the_default_types_are_three_of_our_own() {
    let defaults = default_columns().unwrap();
    assert_eq!(defaults.len(), 3);
    assert_eq!(defaults[0].spec, square_400());
    assert_eq!(
        defaults[1].spec.section,
        Section::Round { diameter: mm(450) }
    );
    assert_eq!(
        defaults[2].spec.section,
        Section::Rectangular {
            width: mm(140),
            depth: mm(140)
        }
    );
    let types = default_column_types(&CategoryTable::new()).unwrap();
    assert_eq!(types.len(), 3);
    for (ty, default) in types.iter().zip(&defaults) {
        assert_eq!(ty.name(), default.name);
        assert_eq!(spec_of(ty).unwrap(), default.spec);
    }
    let mut names: Vec<&str> = types
        .iter()
        .map(construct_types::ElementType::name)
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 3);
}

/// The read-only parameters: height 3000 mm, area 160,000 mm2, volume 480,000,000 mm3 and a mass in
/// kilograms to three places; the timber post's mass is known too; a material the library lacks
/// has none.
#[test]
fn quantities_are_read_only_parameters() {
    let library = Materials::default_metric();
    let q = quantities(&square_400(), mm(3000), &library).unwrap();
    let rows = read_only(&q).unwrap();
    let ids: Vec<String> = rows.iter().map(|(d, _)| d.id().to_string()).collect();
    assert_eq!(ids, vec![HEIGHT, AREA, VOLUME, MASS]);
    assert_eq!(rows[0].1, Some(Value::Length(mm(3000))));
    assert_eq!(rows[1].1, Some(Value::Area(Area::from_mm2(160_000))));
    assert_eq!(
        rows[2].1,
        Some(Value::Volume(Volume::from_mm3(480_000_000)))
    );
    let grams = q.mass_grams.unwrap();
    assert_eq!(
        rows[3].1,
        Some(Value::number(Decimal::new(
            i64::try_from(grams).unwrap(),
            3
        )))
    );
    let timber = ColumnSpec {
        section: Section::rectangular(mm(140), mm(140)).unwrap(),
        material: MaterialId::parse("timber-douglas-fir").unwrap(),
    };
    let post = quantities(&timber, mm(2700), &library).unwrap();
    // 140 x 140 x 2700 = 52,920,000 mm3.
    assert_eq!(post.volume, Volume::from_mm3(52_920_000));
    assert!(post.mass_grams.is_some());
    let odd = ColumnSpec {
        section: Section::rectangular(mm(140), mm(140)).unwrap(),
        material: MaterialId::parse("no-such-material").unwrap(),
    };
    let q = quantities(&odd, mm(2700), &library).unwrap();
    assert_eq!(q.mass_grams, None);
    assert_eq!(read_only(&q).unwrap()[3].1, None);
    assert!(quantities(&timber, mm(0), &library).is_err());
}
