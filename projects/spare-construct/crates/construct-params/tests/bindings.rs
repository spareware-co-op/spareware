//! Project parameters bound to categories: the binding rules, unbinding with its report, "which
//! parameters does an element of category C have", and the stored form.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_categories::{
    ANALYTICAL_MEMBERS, CategoryId, CategoryKind, CategoryTable, DOORS, IMPORTED_DRAWINGS, LEVELS,
    ROOMS, WALLS,
};
use construct_params::{
    Definition, Holder, HolderValues, Kind, ProjectError, ProjectParams, SharedParameter, Value,
    Values, can_hold,
};
use kit_core::Id;

const FIRE: &str = "6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7";
/// The built-in "Core cut line" subcategory of walls.
const CORE_CUT_LINE: CategoryId = CategoryId(101);

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

fn def(text: &str, name: &str, kind: Kind) -> Definition {
    Definition::new(id(text), name, kind, Holder::Instance).unwrap()
}

fn table() -> CategoryTable {
    CategoryTable::new()
}

fn with_params() -> ProjectParams {
    ProjectParams::new()
        .add_project_parameter(def("mark", "Mark", Kind::Text))
        .unwrap()
        .add_project_parameter(def("cost", "cost code", Kind::Text))
        .unwrap()
        .add_shared_parameter(
            &SharedParameter::new(def(FIRE, "Fire rating", Kind::Text), "Fire").unwrap(),
        )
        .unwrap()
}

#[test]
fn binding_gives_every_element_of_the_categories_the_parameter() {
    let bound = with_params()
        .bind(&table(), &id(FIRE), &[WALLS, DOORS], Holder::Instance, None)
        .unwrap();
    for category in [WALLS, DOORS] {
        let names: Vec<&str> = bound
            .parameters_of(&table(), category)
            .iter()
            .map(|a| a.definition.name())
            .collect();
        assert_eq!(names, ["Fire rating"]);
    }
    assert_eq!(bound.parameters_of(&table(), ROOMS).len(), 0);
    assert_eq!(
        bound.binding(&id(FIRE)),
        Some((Holder::Instance, vec![WALLS, DOORS]))
    );
}

#[test]
fn bound_as_type_or_instance_is_reported() {
    let bound = with_params()
        .bind(&table(), &id("mark"), &[DOORS], Holder::Type, None)
        .unwrap()
        .bind(&table(), &id(FIRE), &[DOORS], Holder::Instance, None)
        .unwrap();
    let have = bound.parameters_of(&table(), DOORS);
    let holders: Vec<(&str, Holder, Option<&str>)> = have
        .iter()
        .map(|a| (a.definition.name(), a.holder, a.group))
        .collect();
    assert_eq!(
        holders,
        [
            ("Fire rating", Holder::Instance, Some("Fire")),
            ("Mark", Holder::Type, None)
        ]
    );
}

#[test]
fn parameters_of_comes_in_a_stable_order_whatever_order_they_were_bound() {
    let forward = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap()
        .bind(&table(), &id("cost"), &[WALLS], Holder::Instance, None)
        .unwrap()
        .bind(&table(), &id(FIRE), &[WALLS], Holder::Instance, None)
        .unwrap();
    let backward = with_params()
        .bind(&table(), &id(FIRE), &[WALLS], Holder::Instance, None)
        .unwrap()
        .bind(&table(), &id("cost"), &[WALLS], Holder::Instance, None)
        .unwrap()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    let names = |p: &ProjectParams| -> Vec<String> {
        p.parameters_of(&table(), WALLS)
            .iter()
            .map(|a| a.definition.name().to_owned())
            .collect()
    };
    // By name with case ignored: "cost code" sorts before "Fire rating" and "Mark".
    assert_eq!(names(&forward), ["cost code", "Fire rating", "Mark"]);
    assert_eq!(names(&forward), names(&backward));
    assert_eq!(forward.to_bytes().unwrap(), backward.to_bytes().unwrap());
}

#[test]
fn a_subcategory_is_refused_and_names_its_parent() {
    let err = with_params()
        .bind(
            &table(),
            &id("mark"),
            &[CORE_CUT_LINE],
            Holder::Instance,
            None,
        )
        .unwrap_err();
    assert_eq!(
        err,
        ProjectError::SubcategoryNotBindable {
            category: CORE_CUT_LINE,
            parent: WALLS
        }
    );
    // A project's own subcategory is the same.
    let (own, own_id) = table().add_subcategory(WALLS, "Fire rated").unwrap();
    assert!(matches!(
        with_params().bind(&own, &id("mark"), &[own_id], Holder::Instance, None),
        Err(ProjectError::SubcategoryNotBindable { parent, .. }) if parent == WALLS
    ));
    // One refusal binds nothing, even if other categories in the list are fine.
    let p = with_params();
    assert!(
        p.bind(
            &table(),
            &id("mark"),
            &[WALLS, CORE_CUT_LINE],
            Holder::Instance,
            None
        )
        .is_err()
    );
    assert_eq!(p.binding(&id("mark")), None);
}

#[test]
fn an_element_of_a_subcategory_has_its_parents_parameters() {
    let bound = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    assert_eq!(
        bound.parameters_of(&table(), CORE_CUT_LINE),
        bound.parameters_of(&table(), WALLS)
    );
    assert_eq!(bound.parameters_of(&table(), CategoryId(424_242)).len(), 0);
}

#[test]
fn a_category_that_cannot_hold_the_parameter_is_refused() {
    for category in [IMPORTED_DRAWINGS, ANALYTICAL_MEMBERS] {
        assert!(matches!(
            with_params().bind(&table(), &id("mark"), &[category], Holder::Type, None),
            Err(ProjectError::CannotHold {
                holder: Holder::Type,
                ..
            })
        ));
        // As an instance parameter it is fine.
        assert!(
            with_params()
                .bind(&table(), &id("mark"), &[category], Holder::Instance, None)
                .is_ok()
        );
    }
    assert!(can_hold(CategoryKind::Datum, Holder::Type));
    assert!(can_hold(CategoryKind::Model, Holder::Type));
    assert!(can_hold(CategoryKind::Annotation, Holder::Type));
    assert!(can_hold(CategoryKind::View, Holder::Type));
    assert!(!can_hold(CategoryKind::Import, Holder::Type));
    assert!(!can_hold(CategoryKind::Analytical, Holder::Type));
    assert!(
        with_params()
            .bind(&table(), &id("mark"), &[LEVELS], Holder::Type, None)
            .is_ok()
    );
}

#[test]
fn other_binding_refusals() {
    let p = with_params();
    assert_eq!(
        p.bind(&table(), &id("nope"), &[WALLS], Holder::Instance, None),
        Err(ProjectError::UnknownParameter(id("nope")))
    );
    assert_eq!(
        p.bind(&table(), &id("mark"), &[], Holder::Instance, None),
        Err(ProjectError::NoCategories)
    );
    assert_eq!(
        p.bind(
            &table(),
            &id("mark"),
            &[CategoryId(424_242)],
            Holder::Instance,
            None
        ),
        Err(ProjectError::UnknownCategory(CategoryId(424_242)))
    );
    let bound = p
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    assert!(matches!(
        bound.bind(&table(), &id("mark"), &[DOORS], Holder::Type, None),
        Err(ProjectError::HolderConflict {
            existing: Holder::Instance,
            requested: Holder::Type,
            ..
        })
    ));
}

#[test]
fn binding_again_changes_nothing_and_adds_categories() {
    let once = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    let twice = once
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    assert_eq!(once, twice);
    let more = once
        .bind(&table(), &id("mark"), &[DOORS], Holder::Instance, None)
        .unwrap();
    assert_eq!(
        more.binding(&id("mark")),
        Some((Holder::Instance, vec![WALLS, DOORS]))
    );
}

#[test]
fn adding_parameters_refuses_clashes() {
    let p = with_params();
    // The same project parameter again changes nothing.
    assert_eq!(
        p.add_project_parameter(def("mark", "Mark", Kind::Text))
            .unwrap(),
        p
    );
    // The same ID as another kind, or another name.
    assert!(matches!(
        p.add_project_parameter(def("mark", "Mark", Kind::Number)),
        Err(ProjectError::KindConflict { .. })
    ));
    assert!(matches!(
        p.add_project_parameter(def("mark", "Marking", Kind::Text)),
        Err(ProjectError::Conflict { field: "name", .. })
    ));
    // The same name under another ID, case ignored.
    assert_eq!(
        p.add_project_parameter(def("other", "MARK", Kind::Text)),
        Err(ProjectError::DuplicateName("MARK".to_owned()))
    );
    // A project information ID is reserved.
    assert!(matches!(
        p.add_project_parameter(def("project-info.name", "Mine", Kind::Text)),
        Err(ProjectError::ReservedId(_))
    ));
    // A project-only parameter cannot silently become a shared one.
    let as_shared = SharedParameter::new(def(FIRE, "Fire rating", Kind::Text), "Fire").unwrap();
    let project_only = ProjectParams::new()
        .add_project_parameter(def(FIRE, "Fire rating", Kind::Text))
        .unwrap();
    assert!(matches!(
        project_only.add_shared_parameter(&as_shared),
        Err(ProjectError::Conflict { field: "group", .. })
    ));
}

fn values_with(parameter: &str) -> Values {
    let mut values = Values::new();
    values
        .set(
            &def(parameter, "x", Kind::Text),
            Value::Text("v".to_owned()),
        )
        .unwrap();
    values
}

#[test]
fn unbinding_reports_how_many_values_go() {
    let bound = with_params()
        .bind(
            &table(),
            &id("mark"),
            &[WALLS, DOORS],
            Holder::Instance,
            None,
        )
        .unwrap();
    let has = values_with("mark");
    let other = values_with("cost");
    let none = Values::new();
    let holders = [
        HolderValues {
            category: WALLS,
            values: &has,
        },
        HolderValues {
            category: WALLS,
            values: &has,
        },
        HolderValues {
            category: WALLS,
            values: &none,
        },
        HolderValues {
            category: WALLS,
            values: &other,
        },
        HolderValues {
            category: DOORS,
            values: &has,
        },
        HolderValues {
            category: ROOMS,
            values: &has,
        },
    ];
    let unbound = bound.unbind(&id("mark"), &[WALLS], &holders).unwrap();
    // Two walls had a value; the door is still bound and the room never was.
    assert_eq!(unbound.dropped, 2);
    assert_eq!(
        unbound.params.binding(&id("mark")),
        Some((Holder::Instance, vec![DOORS]))
    );
    assert_eq!(unbound.params.parameters_of(&table(), WALLS).len(), 0);
    // The definition stays, and the original table is as it was.
    assert!(unbound.params.definition(&id("mark")).is_some());
    assert_eq!(bound.binding(&id("mark")).unwrap().1, [WALLS, DOORS]);
    // Unbinding the last category leaves the parameter defined and unbound.
    let last = unbound
        .params
        .unbind(&id("mark"), &[DOORS], &holders)
        .unwrap();
    assert_eq!(last.dropped, 1);
    assert_eq!(last.params.binding(&id("mark")), None);
    // With no elements given, nothing is reported dropped.
    assert_eq!(bound.unbind(&id("mark"), &[WALLS], &[]).unwrap().dropped, 0);
}

#[test]
fn unbinding_refusals() {
    let bound = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    assert!(matches!(
        bound.unbind(&id("nope"), &[WALLS], &[]),
        Err(ProjectError::UnknownParameter(_))
    ));
    assert_eq!(
        bound.unbind(&id("mark"), &[], &[]),
        Err(ProjectError::NoCategories)
    );
    assert!(matches!(
        bound.unbind(&id("mark"), &[WALLS, DOORS], &[]),
        Err(ProjectError::NotBound { category, .. }) if category == DOORS
    ));
    assert!(matches!(
        bound.unbind(&id("cost"), &[WALLS], &[]),
        Err(ProjectError::NotBound { .. })
    ));
}

#[test]
fn stored_form_is_the_same_bytes_and_reads_back() {
    let bound = with_params()
        .bind(&table(), &id(FIRE), &[WALLS, DOORS], Holder::Instance, None)
        .unwrap()
        .bind(&table(), &id("mark"), &[DOORS], Holder::Type, None)
        .unwrap();
    let bytes = bound.to_bytes().unwrap();
    assert_eq!(ProjectParams::from_bytes(&bytes, &table()).unwrap(), bound);
    assert_eq!(
        String::from_utf8(ProjectParams::new().to_bytes().unwrap()).unwrap(),
        r#"{"format":1,"parameters":[],"bindings":[]}"#
    );
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.starts_with(
            r#"{"format":1,"parameters":[{"group":"Fire","definition":{"id":"6f1d2c3a-"#
        )
    );
    assert!(text.contains(r#""bindings":[{"parameter":"6f1d2c3a-4b5e-4f60-8a71-92b3c4d5e6f7","holder":"instance","default":null,"categories":[100,300]}"#));
}

#[test]
fn reading_refuses_stored_bytes_that_break_a_rule() {
    let bound = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    let text = String::from_utf8(bound.to_bytes().unwrap()).unwrap();
    let read = |t: &str| ProjectParams::from_bytes(t.as_bytes(), &table());
    assert_eq!(
        read(&text.replace(r#""format":1"#, r#""format":9"#)),
        Err(ProjectError::UnsupportedFormat(9))
    );
    // A binding to a subcategory, an unknown category, or an undefined parameter.
    assert!(matches!(
        read(&text.replace("[100]", "[101]")),
        Err(ProjectError::SubcategoryNotBindable { .. })
    ));
    assert!(matches!(
        read(&text.replace("[100]", "[424242]")),
        Err(ProjectError::UnknownCategory(_))
    ));
    assert!(matches!(
        read(&text.replace(r#""parameter":"mark""#, r#""parameter":"ghost""#)),
        Err(ProjectError::UnknownParameter(_))
    ));
    // Not the one spelling: categories out of order.
    let two = with_params()
        .bind(
            &table(),
            &id("mark"),
            &[WALLS, DOORS],
            Holder::Instance,
            None,
        )
        .unwrap();
    let two_text = String::from_utf8(two.to_bytes().unwrap()).unwrap();
    assert!(matches!(
        read(&two_text.replace("[100,300]", "[300,100]")),
        Err(ProjectError::Malformed(_))
    ));
    assert!(matches!(read("garbage"), Err(ProjectError::Malformed(_))));
}

#[test]
fn a_binding_can_carry_a_default_checked_against_the_kind() {
    let fire = Value::Text("none".to_owned());
    let bound = with_params()
        .bind(
            &table(),
            &id(FIRE),
            &[WALLS],
            Holder::Instance,
            Some(fire.clone()),
        )
        .unwrap();
    assert_eq!(bound.default_of(&id(FIRE)), Some(&fire));
    let have = bound.parameters_of(&table(), WALLS);
    assert_eq!(have.first().and_then(|a| a.default), Some(&fire));
    // Binding again with the same default changes nothing; None clears it.
    let again = bound
        .bind(&table(), &id(FIRE), &[WALLS], Holder::Instance, Some(fire))
        .unwrap();
    assert_eq!(again, bound);
    let cleared = bound
        .bind(&table(), &id(FIRE), &[WALLS], Holder::Instance, None)
        .unwrap();
    assert_eq!(cleared.default_of(&id(FIRE)), None);
    // A default of the wrong kind is refused, and nothing is bound.
    let p = with_params();
    let wrong = Value::Whole(3);
    assert!(matches!(
        p.bind(&table(), &id(FIRE), &[WALLS], Holder::Instance, Some(wrong)),
        Err(ProjectError::BadDefault(_))
    ));
    assert_eq!(p.binding(&id(FIRE)), None);
    // A choice's default must be one of its keys.
    let colour = Definition::choice(
        id("colour"),
        "Colour",
        Holder::Instance,
        vec![construct_params::Choice::new("red", "Red")],
    )
    .unwrap();
    let c = ProjectParams::new().add_project_parameter(colour).unwrap();
    assert!(matches!(
        c.bind(
            &table(),
            &id("colour"),
            &[WALLS],
            Holder::Instance,
            Some(Value::Choice("blue".to_owned()))
        ),
        Err(ProjectError::BadDefault(_))
    ));
    assert!(
        c.bind(
            &table(),
            &id("colour"),
            &[WALLS],
            Holder::Instance,
            Some(Value::Choice("red".to_owned()))
        )
        .is_ok()
    );
}

#[test]
fn the_default_is_in_the_stored_form_and_read_back_checked() {
    let bound = with_params()
        .bind(
            &table(),
            &id("mark"),
            &[WALLS],
            Holder::Instance,
            Some(Value::Text("A".to_owned())),
        )
        .unwrap();
    let text = String::from_utf8(bound.to_bytes().unwrap()).unwrap();
    assert!(text.contains(r#""holder":"instance","default":"text:A","categories":[100]"#));
    assert_eq!(
        ProjectParams::from_bytes(text.as_bytes(), &table()).unwrap(),
        bound
    );
    let wrong = text.replace("text:A", "whole:3");
    assert!(matches!(
        ProjectParams::from_bytes(wrong.as_bytes(), &table()),
        Err(ProjectError::BadDefault(_))
    ));
}

#[test]
fn every_parameter_is_listed_bound_or_not_by_name() {
    let table_with = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    let names: Vec<&str> = table_with.definitions().iter().map(|d| d.name()).collect();
    assert_eq!(names, ["cost code", "Fire rating", "Mark"]);
    assert_eq!(ProjectParams::new().definitions().len(), 0);
}

#[test]
fn a_parameter_is_renamed_or_given_a_formula_and_keeps_everything_else() {
    let table = with_params()
        .bind(&table(), &id("mark"), &[WALLS], Holder::Instance, None)
        .unwrap();
    let renamed = Definition::new(id("mark"), "Tag", Kind::Text, Holder::Instance).unwrap();
    let next = table.redefine(renamed.clone()).unwrap();
    assert_eq!(next.definition(&id("mark")), Some(&renamed));
    assert_eq!(next.binding(&id("mark")), table.binding(&id("mark")));
    assert_eq!(next.len(), table.len());

    // A formula is the only other change a definition may take.
    let length = Definition::new(id("len"), "Len", Kind::Length, Holder::Instance).unwrap();
    let table = table.add_project_parameter(length.clone()).unwrap();
    let worked = length.clone().with_formula("2 * Len2").unwrap();
    let next = table.redefine(worked.clone()).unwrap();
    assert_eq!(
        next.definition(&id("len")).unwrap().formula(),
        Some("2 * Len2")
    );
    assert_eq!(
        next.redefine(length).unwrap(),
        table,
        "and taken away again"
    );

    // What values are kept under and checked against may not change, and a name stays unique.
    for (changed, want) in [
        (
            Definition::new(id("mark"), "Mark", Kind::Whole, Holder::Instance).unwrap(),
            "kind",
        ),
        (
            Definition::new(id("mark"), "Mark", Kind::Text, Holder::Type).unwrap(),
            "holder",
        ),
    ] {
        let refused = table.redefine(changed).unwrap_err();
        match (refused, want) {
            (ProjectError::KindConflict { .. }, "kind")
            | (
                ProjectError::Conflict {
                    field: "holder", ..
                },
                "holder",
            ) => {}
            (other, _) => panic!("{want}: {other:?}"),
        }
    }
    assert_eq!(
        table.redefine(
            Definition::new(id("mark"), "COST CODE", Kind::Text, Holder::Instance).unwrap()
        ),
        Err(ProjectError::DuplicateName("COST CODE".to_owned()))
    );
    assert_eq!(
        table.redefine(Definition::new(id("none"), "None", Kind::Text, Holder::Instance).unwrap()),
        Err(ProjectError::UnknownParameter(id("none")))
    );
}
