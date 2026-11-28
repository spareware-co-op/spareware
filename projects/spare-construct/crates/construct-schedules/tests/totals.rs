//! Schedules, second half: grouping with headers and footers, totals worked by hand on a small
//! door schedule, equal rows folded into one with a count, calculated fields with units, the
//! cells that refuse an edit and say why, the sheet list and the view list, a schedule placed on a
//! sheet and drawn from its live table, and the new stored entries round trip.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{CategoryTable, DOORS, SHEETS, VIEWS};
use construct_params::{
    Area, Definition, Holder, Kind, Length, ProjectParams, Units, Value, Volume,
};
use construct_phases::{DesignOptions, ElementStaging, PhaseList};
use construct_schedules::{
    Basis, Calculated, EditTarget, Field, Item, Placements, Reading, Schedule, ScheduleError,
    SchedulePlacement, Sort, Subject, Table, TotalKind, Totals, draw, table,
};
use kit_core::Id;
use std::collections::BTreeMap;

fn i(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn staging() -> ElementStaging {
    let phases = PhaseList::new().add(i("phase-new"), "New").unwrap();
    ElementStaging::new(&phases, &DesignOptions::new(), i("phase-new"), None, None).unwrap()
}

/// Width and height (lengths), leaves (a whole number) and level (text), each on the door
/// instance, and a fire rating the door's type holds.
fn params(categories: &CategoryTable) -> ProjectParams {
    let mut table = ProjectParams::new();
    for (id, name, kind, holder) in [
        ("width", "Width", Kind::Length, Holder::Instance),
        ("height", "Height", Kind::Length, Holder::Instance),
        ("leaves", "Leaves", Kind::Whole, Holder::Instance),
        ("level", "Level", Kind::Text, Holder::Instance),
        ("fire", "Fire rating", Kind::Text, Holder::Type),
    ] {
        let definition = Definition::new(i(id), name, kind, holder).unwrap();
        table = table.add_project_parameter(definition).unwrap();
        table = table
            .bind(categories, &i(id), &[DOORS], holder, None)
            .unwrap();
    }
    table
}

fn put(readings: &mut BTreeMap<String, Reading>, key: &str, value: Value, shown: &str) {
    readings.insert(
        key.to_owned(),
        Reading {
            value,
            shown: shown.to_owned(),
        },
    );
}

fn door(id: &str, kind: &str, level: &str, width: i32, height: i32, leaves: i64) -> Subject {
    let mut readings = BTreeMap::new();
    put(
        &mut readings,
        "parameter:width",
        Value::Length(Length::from_mm(width)),
        &format!("{width} mm"),
    );
    put(
        &mut readings,
        "parameter:height",
        Value::Length(Length::from_mm(height)),
        &format!("{height} mm"),
    );
    put(
        &mut readings,
        "parameter:leaves",
        Value::Whole(leaves),
        &leaves.to_string(),
    );
    put(
        &mut readings,
        "parameter:level",
        Value::Text(level.to_owned()),
        level,
    );
    Subject {
        id: i(id),
        category: DOORS,
        name: id.to_owned(),
        type_name: Some(kind.to_owned()),
        readings,
    }
}

/// Five doors: three on level 1 (two single 900 by 2100, one double 1800 by 2100) and two on level
/// 2 (a single 900 and a single 1000, both 2100 high).
fn doors() -> Vec<Subject> {
    vec![
        door("d1", "Single", "L1", 900, 2100, 1),
        door("d2", "Single", "L1", 900, 2100, 1),
        door("d3", "Double", "L1", 1800, 2100, 2),
        door("d4", "Single", "L2", 900, 2100, 1),
        door("d5", "Single", "L2", 1000, 2100, 1),
    ]
}

fn mm(n: i32) -> Value {
    Value::Length(Length::from_mm(n))
}

fn field(id: &str) -> Field {
    Field::Parameter(i(id))
}

fn schedule() -> Schedule {
    Schedule::new(i("s1"), "Door schedule", DOORS, staging()).unwrap()
}

/// What each item of a table is, in a line of words.
fn lines(table: &Table) -> Vec<String> {
    table
        .items
        .iter()
        .map(|item| match item {
            Item::Header(h) => format!("header {}={}", h.name, h.shown.clone().unwrap_or_default()),
            Item::Row(r) => format!(
                "row {} x{}",
                r.elements
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+"),
                r.count
            ),
            Item::Footer(f) => format!(
                "footer {}={} n={}",
                f.name,
                f.shown.clone().unwrap_or_default(),
                f.elements
            ),
        })
        .collect()
}

#[test]
fn grouping_gives_a_header_and_a_footer_to_each_group_and_totals_by_hand() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule()
        .with_fields(vec![
            Field::Type,
            field("width"),
            field("height"),
            field("leaves"),
        ])
        .with_sorting(vec![
            Sort {
                field: field("level"),
                descending: false,
                header: true,
                footer: true,
            },
            Sort::by(Field::Type, false),
        ])
        .with_totals(vec![
            Totals {
                field: field("width"),
                kinds: vec![TotalKind::Sum, TotalKind::Min, TotalKind::Max],
            },
            Totals {
                field: field("leaves"),
                kinds: vec![TotalKind::Sum, TotalKind::Count],
            },
        ])
        .with_grand_totals(true);
    list.check(&basis).unwrap();
    let made = table(&list, &basis, &Units::default(), &doors());
    assert_eq!(
        lines(&made),
        [
            "header Level=L1",
            "row d3 x1",
            "row d1 x1",
            "row d2 x1",
            "footer Level=L1 n=3",
            "header Level=L2",
            "row d4 x1",
            "row d5 x1",
            "footer Level=L2 n=2",
        ]
    );
    let Item::Footer(one) = &made.items[4] else {
        panic!("a footer");
    };
    let by = |footer: &construct_schedules::Footer, column: usize, kind: TotalKind| {
        footer
            .totals
            .iter()
            .find(|t| t.column == column && t.kind == kind)
            .unwrap()
            .clone()
    };
    // Level 1, by hand: widths 900 + 900 + 1800 = 3600 mm, least 900, greatest 1800; leaves
    // 1 + 1 + 2 = 4 from 3 doors.
    assert_eq!(by(one, 1, TotalKind::Sum).value, Some(mm(3600)));
    assert_eq!(by(one, 1, TotalKind::Sum).shown.as_deref(), Some("3600 mm"));
    assert_eq!(by(one, 1, TotalKind::Min).value, Some(mm(900)));
    assert_eq!(by(one, 1, TotalKind::Max).value, Some(mm(1800)));
    assert_eq!(by(one, 3, TotalKind::Sum).value, Some(Value::Whole(4)));
    assert_eq!(by(one, 3, TotalKind::Count).value, Some(Value::Whole(3)));
    let Item::Footer(two) = &made.items[8] else {
        panic!("a footer");
    };
    // Level 2: 900 + 1000 = 1900 mm, leaves 2.
    assert_eq!(by(two, 1, TotalKind::Sum).value, Some(mm(1900)));
    assert_eq!(by(two, 3, TotalKind::Sum).value, Some(Value::Whole(2)));
    // The grand total: five doors, 3600 + 1900 = 5500 mm, six leaves, greatest 1800 mm.
    let grand = made.grand.as_ref().unwrap();
    assert_eq!(grand.elements, 5);
    assert_eq!(grand.level, None);
    assert_eq!(by(grand, 1, TotalKind::Sum).value, Some(mm(5500)));
    assert_eq!(by(grand, 1, TotalKind::Max).value, Some(mm(1800)));
    assert_eq!(by(grand, 3, TotalKind::Sum).value, Some(Value::Whole(6)));
    assert_eq!(by(grand, 3, TotalKind::Count).value, Some(Value::Whole(5)));
    assert_eq!(made.elements, 5);
}

#[test]
fn a_header_alone_and_a_footer_alone_and_no_grouping_each_give_only_what_they_name() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let base = schedule().with_fields(vec![field("width")]);
    let with = |header: bool, footer: bool| {
        let list = base.clone().with_sorting(vec![Sort {
            field: field("level"),
            descending: true,
            header,
            footer,
        }]);
        lines(&table(&list, &basis, &Units::default(), &doors()))
    };
    assert_eq!(
        with(true, false),
        [
            "header Level=L2",
            "row d4 x1",
            "row d5 x1",
            "header Level=L1",
            "row d1 x1",
            "row d2 x1",
            "row d3 x1"
        ]
    );
    assert_eq!(
        with(false, true),
        [
            "row d4 x1",
            "row d5 x1",
            "footer Level=L2 n=2",
            "row d1 x1",
            "row d2 x1",
            "row d3 x1",
            "footer Level=L1 n=3"
        ]
    );
    let plain = base.with_sorting(vec![Sort::by(field("level"), false)]);
    assert_eq!(
        lines(&table(&plain, &basis, &Units::default(), &doors())),
        [
            "row d1 x1",
            "row d2 x1",
            "row d3 x1",
            "row d4 x1",
            "row d5 x1"
        ]
    );
}

#[test]
fn equal_rows_fold_into_one_with_a_count_and_totals_still_count_every_element() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule()
        .with_fields(vec![Field::Type, field("width"), field("height")])
        .with_sorting(vec![
            Sort::by(Field::Type, false),
            Sort::by(field("width"), false),
        ])
        .with_totals(vec![Totals {
            field: field("width"),
            kinds: vec![TotalKind::Sum],
        }])
        .with_grand_totals(true)
        .with_itemize(false);
    list.check(&basis).unwrap();
    let made = table(&list, &basis, &Units::default(), &doors());
    // Single 900 by 2100 stands for d1, d2 and d4: one row, a count of three.
    assert_eq!(lines(&made), ["row d3 x1", "row d1+d2+d4 x3", "row d5 x1"]);
    let Item::Row(folded) = &made.items[1] else {
        panic!("a row");
    };
    assert_eq!(folded.cells[1].value, Some(mm(900)));
    assert_eq!(folded.count, 3);
    // The total of widths is the total of five doors, not of three rows: 900 * 3 + 1800 + 1000.
    let grand = made.grand.unwrap();
    assert_eq!(grand.totals[0].value, Some(mm(5500)));
    // Itemized again, the same schedule has a row for every door.
    let itemized = list.with_itemize(true);
    assert_eq!(
        lines(&table(&itemized, &basis, &Units::default(), &doors())).len(),
        5
    );
}

#[test]
fn a_calculated_field_is_a_formula_with_units_and_exact() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let area = Calculated::new("Door area", "Width * Height", Kind::Area).unwrap();
    let leaf = Calculated::new("Leaf width", "Width / Leaves", Kind::Length).unwrap();
    let list = schedule()
        .with_calculated(vec![area, leaf])
        .with_fields(vec![
            field("width"),
            Field::Calculated("Door area".to_owned()),
            Field::Calculated("Leaf width".to_owned()),
        ])
        .with_totals(vec![Totals {
            field: Field::Calculated("Door area".to_owned()),
            kinds: vec![TotalKind::Sum],
        }])
        .with_grand_totals(true);
    list.check(&basis).unwrap();
    let made = table(&list, &basis, &Units::default(), &doors());
    let cells = |at: usize| {
        let Item::Row(row) = &made.items[at] else {
            panic!("a row");
        };
        row.cells.clone()
    };
    // d1: 0.9 m by 2.1 m is 1.89 m2, exactly; d3: a 1.8 m double leaf is two 900 mm leaves.
    assert_eq!(
        cells(0)[1].value,
        Some(Value::Area(Area::from_mm2(1_890_000)))
    );
    assert_eq!(
        cells(2)[1].value,
        Some(Value::Area(Area::from_mm2(3_780_000)))
    );
    assert_eq!(cells(2)[2].value, Some(mm(900)));
    // The total is the sum of the exact areas: 1.89 + 1.89 + 3.78 + 1.89 + 2.10 = 11.55 m2.
    assert_eq!(
        made.grand.unwrap().totals[0].value,
        Some(Value::Area(Area::from_mm2(11_550_000)))
    );
    // A calculated field can be sorted and filtered like any other.
    let sorted = list.clone().with_sorting(vec![Sort::by(
        Field::Calculated("Door area".to_owned()),
        true,
    )]);
    let made = table(&sorted, &basis, &Units::default(), &doors());
    assert_eq!(lines(&made)[0], "row d3 x1");
    // A volume from three lengths and a number from a ratio of two lengths.
    let volume = Calculated::new("Slab", "Width * Height * 100 mm", Kind::Volume).unwrap();
    let ratio = Calculated::new("Ratio", "Height / Width", Kind::Number).unwrap();
    let more = schedule()
        .with_calculated(vec![volume, ratio])
        .with_fields(vec![
            Field::Calculated("Slab".to_owned()),
            Field::Calculated("Ratio".to_owned()),
        ]);
    more.check(&basis).unwrap();
    let made = table(&more, &basis, &Units::default(), &doors());
    let Item::Row(first) = &made.items[0] else {
        panic!("a row");
    };
    assert_eq!(
        first.cells[0].value,
        Some(Value::Volume(Volume::from_mm3(900 * 2100 * 100)))
    );
}

fn refused(formula: &str, kind: Kind) -> String {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule().with_calculated(vec![Calculated::new("Made", formula, kind).unwrap()]);
    list.check(&basis).unwrap_err().to_string()
}

#[test]
fn a_wrong_unit_or_an_unknown_field_is_refused_in_words() {
    // A length plus a bare number: no unit.
    let said = refused("Width + 2", Kind::Length);
    assert!(said.contains("Made") && said.contains("unit"), "{said}");
    // A length times a length is not a length.
    let said = refused("Width * Height", Kind::Length);
    assert!(said.contains("Made"), "{said}");
    // A field that is not there, and what is.
    let said = refused("Width * Depth", Kind::Area);
    assert!(
        said.contains("no field called Depth") && said.contains("Width"),
        "{said}"
    );
    // Text cannot be used in a formula.
    let said = refused("Level + 1", Kind::Number);
    assert!(said.contains("Level holds text"), "{said}");
    // A formula that does not read.
    assert!(matches!(
        Calculated::new("Made", "Width *", Kind::Area).unwrap_err(),
        ScheduleError::Calculated { .. }
    ));
    // A name another field has, case aside, and a kind a formula does not give.
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let taken = schedule().with_calculated(vec![
        Calculated::new("width", "Height", Kind::Length).unwrap(),
    ]);
    assert!(
        taken
            .check(&basis)
            .unwrap_err()
            .to_string()
            .contains("already has that name")
    );
    assert!(Calculated::new("Name", "1", Kind::Text).is_err());
    assert!(Calculated::new("  ", "1", Kind::Number).is_err());
}

#[test]
fn totals_are_refused_where_they_make_no_sense() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let base = schedule().with_fields(vec![Field::Type, field("width")]);
    let said = |totals: Vec<Totals>| base.clone().with_totals(totals).check(&basis).unwrap_err();
    // The sum of text.
    let error = said(vec![Totals {
        field: Field::Type,
        kinds: vec![TotalKind::Sum],
    }]);
    assert!(matches!(error, ScheduleError::TotalNotFor { .. }));
    assert!(error.to_string().contains("holds text"));
    // A count of text is fine; a column the schedule does not list has no totals.
    base.clone()
        .with_totals(vec![Totals {
            field: Field::Type,
            kinds: vec![TotalKind::Count],
        }])
        .check(&basis)
        .unwrap();
    assert!(matches!(
        said(vec![Totals {
            field: field("height"),
            kinds: vec![TotalKind::Sum]
        }]),
        ScheduleError::TotalNotListed(_)
    ));
    assert!(matches!(
        said(vec![Totals {
            field: field("width"),
            kinds: vec![]
        }]),
        ScheduleError::NoTotals(_)
    ));
    assert!(matches!(
        said(vec![Totals {
            field: field("width"),
            kinds: vec![TotalKind::Sum, TotalKind::Sum]
        }]),
        ScheduleError::DuplicateTotal(_)
    ));
}

#[test]
fn a_cell_edit_goes_back_to_the_element_or_is_refused_in_words() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule()
        .with_calculated(vec![
            Calculated::new("Door area", "Width * Height", Kind::Area).unwrap(),
        ])
        .with_fields(vec![
            Field::Name,
            Field::Type,
            field("width"),
            field("fire"),
            Field::Calculated("Door area".to_owned()),
        ]);
    list.check(&basis).unwrap();
    assert_eq!(
        list.edit_target(&basis, &Field::Name).unwrap(),
        EditTarget::Name
    );
    let EditTarget::Parameter(width) = list.edit_target(&basis, &field("width")).unwrap() else {
        panic!("a parameter");
    };
    assert_eq!(width.id(), &i("width"));
    let refusal = |f: Field| list.edit_target(&basis, &f).unwrap_err().to_string();
    // The type's value shown in an instance schedule, the type's name, a calculated field.
    let said = refusal(field("fire"));
    assert!(
        said.starts_with("Fire rating cannot be edited here:")
            && said.contains("held by the element's type"),
        "{said}"
    );
    assert!(refusal(Field::Type).contains("type"));
    assert!(refusal(Field::Calculated("Door area".to_owned())).contains("formula"));
    // A field the schedule lacks is not read-only: it is not there.
    assert!(matches!(
        list.edit_target(&basis, &field("nothing")).unwrap_err(),
        ScheduleError::NoSuchField { .. }
    ));
}

fn sheet_subject(id: &str, number: &str, name: &str, views: i64) -> Subject {
    let mut readings = BTreeMap::new();
    put(
        &mut readings,
        "quantity:Number",
        Value::Text(number.to_owned()),
        number,
    );
    put(
        &mut readings,
        "quantity:Paper",
        Value::Text("A1 landscape".to_owned()),
        "A1 landscape",
    );
    put(
        &mut readings,
        "quantity:Views placed",
        Value::Whole(views),
        &views.to_string(),
    );
    Subject {
        id: i(id),
        category: SHEETS,
        name: name.to_owned(),
        type_name: None,
        readings,
    }
}

#[test]
fn the_sheet_list_and_the_view_list_are_schedules_of_their_categories() {
    let categories = CategoryTable::new();
    let params = ProjectParams::new();
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = Schedule::new(i("sl"), "Sheet list", SHEETS, staging())
        .unwrap()
        .with_fields(vec![
            Field::Quantity("Number".to_owned()),
            Field::Name,
            Field::Quantity("Views placed".to_owned()),
        ])
        .with_sorting(vec![Sort::by(Field::Quantity("Number".to_owned()), false)])
        .with_totals(vec![Totals {
            field: Field::Quantity("Views placed".to_owned()),
            kinds: vec![TotalKind::Sum],
        }])
        .with_grand_totals(true);
    list.check(&basis).unwrap();
    let sheets = vec![
        sheet_subject("sh2", "A-102", "Second floor", 3),
        sheet_subject("sh1", "A-101", "First floor", 2),
    ];
    let made = table(&list, &basis, &Units::default(), &sheets);
    assert_eq!(lines(&made), ["row sh1 x1", "row sh2 x1"]);
    assert_eq!(made.grand.unwrap().totals[0].value, Some(Value::Whole(5)));
    // The view list stands the same way; a model has no revisions yet, so no revision list.
    let views = Schedule::new(i("vl"), "View list", VIEWS, staging())
        .unwrap()
        .with_fields(vec![
            Field::Name,
            Field::Quantity("Kind".to_owned()),
            Field::Quantity("Scale".to_owned()),
            Field::Quantity("On sheet".to_owned()),
        ]);
    views.check(&basis).unwrap();
}

#[test]
fn a_schedule_placed_on_a_sheet_is_its_own_record_and_is_drawn_from_the_live_table() {
    let at = SchedulePlacement::new(
        i("pl1"),
        i("s1"),
        i("sheet1"),
        Length::from_mm(20),
        Length::from_mm(400),
    );
    let record = at.to_record(&i("doc"));
    assert_eq!(record.kind, "schedule-placement");
    assert_eq!(record.props["@placement.x"], "length:q0:128000");
    assert_eq!(SchedulePlacement::from_record(&record).unwrap(), at);
    let mut wrong = record.clone();
    wrong
        .props
        .insert("@placement.x".to_owned(), "length:q0:0128000".to_owned());
    assert!(SchedulePlacement::from_record(&wrong).is_err());
    let mut all = Placements::from_records([&record]).unwrap();
    assert_eq!(all.of_sheet(&i("sheet1")).len(), 1);
    // One schedule stands on a sheet once, but may stand on another.
    let again = SchedulePlacement::new(
        i("pl2"),
        i("s1"),
        i("sheet1"),
        Length::from_mm(1),
        Length::from_mm(1),
    );
    assert!(matches!(
        all.add(again).unwrap_err(),
        ScheduleError::DuplicatePlacement(..)
    ));
    all.add(SchedulePlacement::new(
        i("pl3"),
        i("s1"),
        i("sheet2"),
        Length::from_mm(1),
        Length::from_mm(1),
    ))
    .unwrap();
    assert_eq!(all.of_schedule(&i("s1")).len(), 2);
    let moved = all
        .get(&i("pl1"))
        .unwrap()
        .clone()
        .moved(Length::from_mm(30), Length::from_mm(300));
    all.replace(moved).unwrap();
    assert_eq!(all.get(&i("pl1")).unwrap().x(), Length::from_mm(30));
    all.remove(&i("pl3")).unwrap();
    assert_eq!(all.len(), 2 - 1);

    // The drawing is of the table as it is now: a new door is a new row.
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule().with_fields(vec![field("level")]);
    let mut model = vec![
        door("d1", "Single", "L1", 900, 2100, 1),
        door("d2", "Single", "L10", 900, 2100, 1),
    ];
    let drawn = draw(&table(&list, &basis, &Units::default(), &model), "Doors");
    // Four rows (title, heading, two doors) of 5 mm. The column holds "Level" (5 characters), "L1"
    // and "L10" at 1.5 mm and 1 mm either side: 5 * 1.5 + 2 = 9.5 mm, which the 5 character title
    // does not outgrow.
    assert_eq!(drawn.height, Length::from_mm(20));
    assert_eq!(drawn.width, Length::from_steps(60_800));
    assert_eq!(drawn.texts.len(), 4);
    assert_eq!(drawn.texts[0].text, "Doors");
    assert!(drawn.texts[0].bold);
    assert_eq!(drawn.texts[3].text, "L10");
    assert_eq!(drawn.texts[3].y, Length::from_mm(15));
    model.push(door("d3", "Single", "L2", 900, 2100, 1));
    let later = draw(&table(&list, &basis, &Units::default(), &model), "Doors");
    assert_eq!(later.height, Length::from_mm(25));
    assert_eq!(
        draw(&table(&list, &basis, &Units::default(), &model), "Doors"),
        later
    );
}

#[test]
fn the_new_entries_round_trip_and_an_old_schedule_is_the_same_bytes() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let plain = schedule().with_fields(vec![field("width")]);
    let record = plain.to_record(&i("doc"));
    for key in [
        "@schedule.itemize",
        "@schedule.grand",
        "@schedule.totals",
        "@schedule.calculated",
    ] {
        assert!(!record.props.contains_key(key), "{key}");
    }
    assert_eq!(
        record.props["@schedule.sorting"], "[]",
        "an old schedule's text is unchanged"
    );
    let full = schedule()
        .with_calculated(vec![
            Calculated::new("Door area", "Width * Height", Kind::Area).unwrap(),
        ])
        .with_fields(vec![
            field("width"),
            Field::Calculated("Door area".to_owned()),
        ])
        .with_sorting(vec![Sort {
            field: field("level"),
            descending: false,
            header: true,
            footer: false,
        }])
        .with_totals(vec![Totals {
            field: field("width"),
            kinds: vec![TotalKind::Sum, TotalKind::Max],
        }])
        .with_grand_totals(true)
        .with_itemize(false);
    full.check(&basis).unwrap();
    let record = full.to_record(&i("doc"));
    assert_eq!(record.props["@schedule.itemize"], "no");
    assert_eq!(record.props["@schedule.grand"], "yes");
    assert_eq!(
        record.props["@schedule.totals"],
        r#"[{"field":"parameter:width","do":["sum","max"]}]"#
    );
    assert_eq!(
        record.props["@schedule.sorting"],
        r#"[{"field":"parameter:level","order":"ascending","header":true}]"#
    );
    assert_eq!(
        record.props["@schedule.calculated"],
        r#"[{"name":"Door area","formula":"Width * Height","kind":"area"}]"#
    );
    assert_eq!(Schedule::from_record(&record).unwrap(), full);
    // A spelling that is not the one spelling is refused, never repaired.
    let mut wrong = record.clone();
    wrong
        .props
        .insert("@schedule.itemize".to_owned(), "yes".to_owned());
    assert!(Schedule::from_record(&wrong).is_err());
    let mut wrong = record;
    wrong
        .props
        .insert("@schedule.grand".to_owned(), "no".to_owned());
    assert!(Schedule::from_record(&wrong).is_err());
}

#[test]
fn a_grouped_schedule_is_drawn_with_its_headers_footers_and_totals() {
    let categories = CategoryTable::new();
    let params = params(&categories);
    let basis = Basis {
        categories: &categories,
        params: &params,
    };
    let list = schedule()
        .with_fields(vec![Field::Type, field("width"), field("leaves")])
        .with_sorting(vec![Sort {
            field: field("level"),
            descending: false,
            header: true,
            footer: true,
        }])
        .with_totals(vec![Totals {
            field: field("width"),
            kinds: vec![TotalKind::Sum, TotalKind::Min, TotalKind::Max],
        }])
        .with_grand_totals(true);
    let drawn = draw(&table(&list, &basis, &Units::default(), &doors()), "Doors");
    let texts: Vec<&str> = drawn.texts.iter().map(|t| t.text.as_str()).collect();
    // The header, the doors' cells, the footer with its label and the totals worked by hand
    // (900 + 900 + 1800 = 3600 mm, least 900, greatest 1800), and the grand total.
    assert!(texts.contains(&"Level: L1"));
    assert!(texts.contains(&"Level: L1 (3)"));
    assert!(texts.contains(&"sum: 3600 mm  min: 900 mm  max: 1800 mm"));
    assert!(texts.contains(&"sum: 1900 mm  min: 900 mm  max: 1000 mm"));
    assert!(texts.contains(&"Grand total (5)"));
    assert!(texts.contains(&"sum: 5500 mm  min: 900 mm  max: 1800 mm"));
    // Title, headings, two groups of a header, their doors and a footer, and the grand total.
    assert_eq!(
        drawn.height,
        Length::from_mm(5 * (2 + (1 + 3 + 1) + (1 + 2 + 1) + 1))
    );
}
