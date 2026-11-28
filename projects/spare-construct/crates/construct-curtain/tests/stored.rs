//! The stored forms: a type's grid and parts in its system block, a wall's edits in its recipe,
//! and the recipe a builder reads. Each writes one spelling and reads back only that.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{all, every, glazed, mm, mullion, six_by_three, solid, spec};
use construct_categories::{CURTAIN_WALLS, CategoryTable, WALLS};
use construct_curtain::{
    Crossing, CurtainError, CurtainSpec, Direction, Edits, Filled, Justify, Layout, Mullions,
    Panel, Segment, curtain_recipe, default_curtains, edits_of, embedded_recipe, filled,
    new_curtain_type, reads_type, spec_of, type_arg, with_edits, with_line, with_spec,
};
use construct_params::{Length, Value};
use construct_types::ElementType;
use kit_core::Id;
use serde_json::json;

/// The 6 m by 3 m type with mullions everywhere, written: compact JSON, its fields in order.
#[test]
fn a_type_is_written_in_one_spelling() {
    let spec = spec(
        every(1500),
        every(1000),
        all(&mullion(50, 150)),
        Crossing::Vertical,
    );
    let text = spec.to_text().unwrap();
    let profile = r#"{"shape":"rectangle","width":"length:q0:320000","depth":"length:q0:960000","offset":"length:q0:0","material":"timber-douglas-fir"}"#;
    let expected = format!(
        r#"{{"version":1,"vertical":{{"layout":"spacing","spacing":"length:q0:9600000","justify":"start"}},"horizontal":{{"layout":"spacing","spacing":"length:q0:6400000","justify":"start"}},"panel":{{"kind":"glazed","thickness":"length:q0:153600","offset":"length:q0:0","material":"glass-plate"}},"mullions":{{"interior-vertical":{profile},"interior-horizontal":{profile},"border-vertical":{profile},"border-horizontal":{profile}}},"crossing":"vertical"}}"#
    );
    assert_eq!(text, expected);
    assert_eq!(CurtainSpec::from_text(&text).unwrap(), spec);
}

/// Every layout, panel kind and an empty mullion slot read back as written; anything not in the
/// one spelling is refused, never repaired.
#[test]
fn every_layout_and_panel_reads_back() {
    let door = Panel::Door(
        construct_curtain::DoorPanel::new(
            "sha256:00000000000000000000000000000000000000000000000000000000000000ff",
            "d900x2100",
        )
        .unwrap(),
    );
    for (vertical, horizontal, panel) in [
        (Layout::None, Layout::Count { count: 3 }, Panel::Empty),
        (
            Layout::Most { spacing: mm(1200) },
            Layout::Spacing {
                spacing: mm(900),
                justify: Justify::Centre,
            },
            solid(50),
        ),
        (
            Layout::Spacing {
                spacing: mm(1000),
                justify: Justify::End,
            },
            Layout::None,
            door,
        ),
    ] {
        let spec = CurtainSpec::new(
            vertical,
            horizontal,
            panel,
            Mullions {
                interior_vertical: Some(mullion(60, 120)),
                ..Mullions::default()
            },
            Crossing::Horizontal,
        )
        .unwrap();
        let text = spec.to_text().unwrap();
        assert_eq!(CurtainSpec::from_text(&text).unwrap(), spec, "{text}");
        assert!(text.contains(r#""interior-horizontal":null"#));
        let spaced = text.replacen(':', ": ", 1);
        assert!(matches!(
            CurtainSpec::from_text(&spaced),
            Err(CurtainError::Malformed(_))
        ));
        let later = text.replacen(r#""version":1"#, r#""version":2"#, 1);
        assert!(matches!(
            CurtainSpec::from_text(&later),
            Err(CurtainError::Malformed(_))
        ));
    }
    let odd = spec(
        every(1500),
        Layout::None,
        Mullions::default(),
        Crossing::Vertical,
    )
    .to_text()
    .unwrap()
    .replace("length:q0:153600", "length:q0:153601");
    assert!(matches!(
        CurtainSpec::from_text(&odd),
        Err(CurtainError::OddSteps { .. })
    ));
}

/// What a type refuses: a spacing at the tolerance or below, a count of none, a mullion or a
/// panel of no size, too large, or whose half falls between two steps.
#[test]
fn a_type_refuses_what_cannot_be_built() {
    assert!(matches!(
        CurtainSpec::new(
            Layout::Spacing {
                spacing: Length::from_steps(64),
                justify: Justify::Start
            },
            Layout::None,
            glazed(24),
            Mullions::default(),
            Crossing::Vertical
        ),
        Err(CurtainError::NotPositive { .. })
    ));
    assert_eq!(
        CurtainSpec::new(
            Layout::None,
            Layout::Count { count: 0 },
            glazed(24),
            Mullions::default(),
            Crossing::Vertical
        )
        .unwrap_err(),
        CurtainError::BadCount {
            direction: Direction::Horizontal,
            count: 0
        }
    );
    let m = common::material("timber-douglas-fir");
    assert!(matches!(
        construct_curtain::Profile::rectangle(mm(0), mm(100), mm(0), m.clone()),
        Err(CurtainError::NotPositive { .. })
    ));
    assert!(matches!(
        construct_curtain::Profile::rectangle(mm(2001), mm(100), mm(0), m.clone()),
        Err(CurtainError::TooLarge { .. })
    ));
    assert!(matches!(
        construct_curtain::Profile::rectangle(Length::from_steps(321), mm(100), mm(0), m.clone()),
        Err(CurtainError::OddSteps { .. })
    ));
    assert!(matches!(
        construct_curtain::Profile::rectangle(mm(50), mm(100), mm(-2001), m),
        Err(CurtainError::TooLarge { .. })
    ));
}

/// A wall's edits are written in order, every length in its stored spelling; none is no argument.
#[test]
fn a_walls_edits_are_written_in_one_spelling() {
    assert_eq!(Edits::new().to_json().unwrap(), None);
    assert_eq!(Edits::from_json(None).unwrap(), Edits::new());
    let wall = six_by_three(Mullions::default())
        .with_line(Direction::Vertical, mm(750))
        .unwrap()
        .without_line(Direction::Horizontal, mm(2000))
        .unwrap()
        .with_segment_omitted(Segment {
            direction: Direction::Vertical,
            at: mm(1500),
            from: mm(0),
        })
        .unwrap()
        .with_panel([mm(3000), mm(0)], Some(Panel::Empty))
        .unwrap();
    let json = wall.edits().to_json().unwrap().unwrap();
    let q0 = |mm: i64| Value::Length(common::mm(mm)).stored();
    assert_eq!(
        json,
        json!({
            "added": { "vertical": [q0(750)], "horizontal": [] },
            "removed": { "vertical": [], "horizontal": [q0(2000)] },
            "omitted": [{ "line": "vertical", "at": q0(1500), "from": q0(0) }],
            "panels": [{ "at": [q0(3000), q0(0)], "panel": { "kind": "empty" } }],
        })
    );
    assert_eq!(Edits::from_json(Some(&json)).unwrap(), *wall.edits());
    let empty = json!({ "added": { "vertical": [], "horizontal": [] },
        "removed": { "vertical": [], "horizontal": [] }, "omitted": [], "panels": [] });
    assert!(matches!(
        Edits::from_json(Some(&empty)),
        Err(CurtainError::Malformed(_))
    ));
    let mut twice = json.clone();
    twice["added"]["vertical"] = json!([q0(750), q0(750)]);
    assert!(matches!(
        Edits::from_json(Some(&twice)),
        Err(CurtainError::Malformed(_))
    ));
    let mut extra = json;
    extra["more"] = json!(1);
    assert!(matches!(
        Edits::from_json(Some(&extra)),
        Err(CurtainError::Malformed(_))
    ));
}

/// A free wall's recipe reads its type when it is built: `{"type":"curtain"}` until it is filled
/// with the type's text, its grid argument written only when it has edits. A filled recipe reads
/// back as the wall it says; an embedded one is laid out once it is seated on its host's line.
#[test]
fn the_recipe_reads_its_type_and_its_edits() {
    let wall = six_by_three(Mullions::default())
        .with_line(Direction::Vertical, mm(750))
        .unwrap();
    let height = json!(Value::Length(mm(3000)).stored());
    let recipe = curtain_recipe([mm(0), mm(0)], [mm(6000), mm(0)], height, wall.edits()).unwrap();
    assert_eq!(recipe.op, "wall.curtain");
    assert!(reads_type(&recipe));
    assert_eq!(recipe.args["curtain"], type_arg());
    assert_eq!(edits_of(&recipe).unwrap(), *wall.edits());
    assert!(matches!(
        Filled::of(&recipe),
        Err(CurtainError::Malformed(_))
    ));
    let built = filled(&recipe, wall.spec()).unwrap();
    assert!(!reads_type(&built));
    let read = Filled::of(&built).unwrap();
    assert_eq!(read.curtain, wall);
    assert_eq!((read.start, read.end), ([mm(0), mm(0)], [mm(6000), mm(0)]));
    let plain = with_edits(&built, &Edits::new()).unwrap();
    assert!(!plain.args.contains_key("grid"));

    // At an angle, the length is the line's to the nearest step: 3 by 4 m is exactly 5 m.
    let angled = curtain_recipe(
        [mm(0), mm(0)],
        [mm(3000), mm(4000)],
        json!(Value::Length(mm(3000)).stored()),
        &Edits::new(),
    )
    .unwrap();
    let angled = Filled::of(&filled(&angled, wall.spec()).unwrap()).unwrap();
    assert_eq!(angled.curtain.length(), mm(5000));

    let embedded = embedded_recipe(mm(4000), mm(2400), &Edits::new()).unwrap();
    assert!(construct_curtain::is_embedded(&embedded));
    assert_eq!(
        construct_curtain::embedded_length(&embedded).unwrap(),
        mm(4000)
    );
    let embedded = filled(&embedded, wall.spec()).unwrap();
    assert!(matches!(
        Filled::of(&embedded),
        Err(CurtainError::Malformed(_))
    ));
    let seated = with_line(&embedded, [mm(1000), mm(0)], [mm(5000), mm(0)]).unwrap();
    assert_eq!(Filled::of(&seated).unwrap().curtain.length(), mm(4000));
}

/// A curtain wall type is an element type of the curtain walls category holding its grid and
/// parts in its block, read back as given; a type of another category is refused.
#[test]
fn a_type_is_an_element_type_of_curtain_walls() {
    let categories = CategoryTable::new();
    let spec = spec(
        every(1500),
        every(1000),
        all(&mullion(50, 150)),
        Crossing::Vertical,
    );
    let id = Id::parse("cw-1").unwrap();
    let ty = new_curtain_type(id.clone(), "Shopfront", &spec, &categories).unwrap();
    assert_eq!(ty.category(), CURTAIN_WALLS);
    assert_eq!(spec_of(&ty).unwrap(), spec);
    let wall = ElementType::new(Id::parse("w-1").unwrap(), "Wall", WALLS, &categories).unwrap();
    assert!(matches!(
        with_spec(wall.clone(), &spec),
        Err(CurtainError::NotACurtainType { .. })
    ));
    assert!(matches!(
        spec_of(&wall),
        Err(CurtainError::NotACurtainType { .. })
    ));
    let bare = ElementType::new(id, "Bare", CURTAIN_WALLS, &categories).unwrap();
    assert!(matches!(
        spec_of(&bare),
        Err(CurtainError::NoCurtainBlock { .. })
    ));
}

/// The default set: three types, each with its permanent ID, each laying out a 6 m by 3 m wall.
#[test]
fn the_default_set_lays_out() {
    let defaults = default_curtains().unwrap();
    let ids: Vec<&str> = defaults.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ids,
        ["curtain-1500x1000", "curtain-storefront", "curtain-glazing"]
    );
    let panels: Vec<usize> = defaults
        .iter()
        .map(|d| {
            common::wall(6000, 3000, d.spec.clone())
                .grid()
                .unwrap()
                .panels
                .len()
        })
        .collect();
    // 1.5 m from the middle: lines at 1.5, 3 and 4.5 m (4 columns) by 1 m rows (3): 12. At most
    // 1.2 m: 5 equal columns, one row: 5. One pane: 1.
    assert_eq!(panels, [12, 5, 1]);
}
