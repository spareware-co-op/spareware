//! Project information: the built-in fields and their pinned IDs, the issue date as a calendar
//! date, extras, what a title block reads, and the stored form.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::{
    Definition, Holder, InfoError, InfoField, IssueDate, Kind, ProjectInfo, Units, Value,
    is_builtin_id,
};
use kit_core::Id;

fn id(text: &str) -> Id {
    Id::parse(text).unwrap()
}

#[test]
fn the_built_in_ids_are_pinned() {
    let pinned = [
        (InfoField::Name, "project-info.name", "Project name"),
        (InfoField::Number, "project-info.number", "Project number"),
        (InfoField::Client, "project-info.client", "Client"),
        (InfoField::Address, "project-info.address", "Address"),
        (InfoField::Stage, "project-info.stage", "Stage"),
        (
            InfoField::IssueDate,
            "project-info.issue-date",
            "Issue date",
        ),
    ];
    assert_eq!(InfoField::ALL.len(), pinned.len());
    for (field, text, label) in pinned {
        assert_eq!(field.id_text(), text);
        assert_eq!(field.label(), label);
        let definition = field.definition().unwrap();
        assert_eq!(definition.id().as_str(), text);
        assert_eq!(definition.kind(), Kind::Text);
        assert!(is_builtin_id(&id(text)));
    }
    assert!(!is_builtin_id(&id("project-info.extra")));
}

#[test]
fn text_fields_are_set_and_read_and_the_original_is_kept() {
    let empty = ProjectInfo::new();
    let named = empty.set_text(InfoField::Name, "Harbour House").unwrap();
    assert_eq!(empty.text(InfoField::Name), None);
    assert_eq!(named.text(InfoField::Name), Some("Harbour House"));
    let all = named
        .set_text(InfoField::Number, "24-017")
        .unwrap()
        .set_text(InfoField::Client, "A. Client Ltd")
        .unwrap()
        .set_text(InfoField::Address, "1 Example Street")
        .unwrap()
        .set_text(InfoField::Stage, "Developed design")
        .unwrap();
    assert_eq!(all.text(InfoField::Stage), Some("Developed design"));
    let cleared = all.clear(InfoField::Client);
    assert_eq!(cleared.text(InfoField::Client), None);
    assert_eq!(cleared.text(InfoField::Number), Some("24-017"));
}

#[test]
fn the_issue_date_is_a_real_calendar_date() {
    for (y, m, d) in [
        (2026, 11, 22),
        (2024, 2, 29),
        (2000, 2, 29),
        (1, 1, 1),
        (9999, 12, 31),
    ] {
        assert!(IssueDate::new(y, m, d).is_ok(), "{y}-{m}-{d}");
    }
    for (y, m, d) in [
        (2026, 2, 29),
        (1900, 2, 29),
        (2026, 4, 31),
        (2026, 0, 1),
        (2026, 13, 1),
        (2026, 1, 0),
        (0, 1, 1),
        (10_000, 1, 1),
    ] {
        assert!(
            matches!(IssueDate::new(y, m, d), Err(InfoError::BadDate(_))),
            "{y}-{m}-{d}"
        );
    }
    let date = IssueDate::new(2026, 3, 7).unwrap();
    assert_eq!(date.to_string(), "2026-03-07");
    assert_eq!(IssueDate::parse("2026-03-07").unwrap(), date);
    for bad in [
        "2026-3-7",
        "26-03-07",
        "2026/03/07",
        "2026-02-30",
        "",
        "2026-03-07-1",
        "+026-03-07",
    ] {
        assert!(IssueDate::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_issue_date_goes_through_its_own_function() {
    let date = IssueDate::new(2026, 11, 22).unwrap();
    let info = ProjectInfo::new().set_issue_date(date).unwrap();
    assert_eq!(info.issue_date(), Some(date));
    assert_eq!(info.text(InfoField::IssueDate), None);
    assert_eq!(
        info.to_props()
            .get("project-info.issue-date")
            .map(String::as_str),
        Some("text:2026-11-22")
    );
    assert_eq!(
        ProjectInfo::new().set_text(InfoField::IssueDate, "2026-11-22"),
        Err(InfoError::WrongField(InfoField::IssueDate))
    );
}

#[test]
fn extras_are_project_defined_parameters() {
    let permit =
        Definition::new(id("permit"), "Permit number", Kind::Text, Holder::Instance).unwrap();
    let floors = Definition::new(id("floors"), "Storeys", Kind::Whole, Holder::Instance).unwrap();
    let info = ProjectInfo::new()
        .add_extra(permit.clone())
        .unwrap()
        .add_extra(floors)
        .unwrap()
        .set_extra(&id("permit"), Value::Text("BC-1".to_owned()))
        .unwrap()
        .set_extra(&id("floors"), Value::Whole(3))
        .unwrap();
    assert_eq!(
        info.extra(&id("permit")),
        Some(&Value::Text("BC-1".to_owned()))
    );
    assert_eq!(info.extras().count(), 2);
    // Adding the same extra again changes nothing.
    assert_eq!(info.add_extra(permit).unwrap(), info);
}

#[test]
fn extras_refusals() {
    let info = ProjectInfo::new();
    let mk = |i: &str, n: &str, k| Definition::new(id(i), n, k, Holder::Instance).unwrap();
    assert!(matches!(
        info.add_extra(mk("project-info.name", "Mine", Kind::Text)),
        Err(InfoError::ReservedId(_))
    ));
    assert_eq!(
        info.add_extra(mk("x", "client", Kind::Text)),
        Err(InfoError::DuplicateName("client".to_owned()))
    );
    let with = info.add_extra(mk("permit", "Permit", Kind::Text)).unwrap();
    assert_eq!(
        with.add_extra(mk("permit", "Permit", Kind::Number)),
        Err(InfoError::Conflict(id("permit")))
    );
    assert_eq!(
        with.add_extra(mk("other", "PERMIT", Kind::Text)),
        Err(InfoError::DuplicateName("PERMIT".to_owned()))
    );
    assert_eq!(
        with.set_extra(&id("ghost"), Value::Whole(1)),
        Err(InfoError::UnknownExtra(id("ghost")))
    );
    assert!(matches!(
        with.set_extra(&id("permit"), Value::Whole(1)),
        Err(InfoError::Param(_))
    ));
}

#[test]
fn a_title_block_reads_every_line_in_a_fixed_order() {
    let info = ProjectInfo::new()
        .add_extra(Definition::new(id("floors"), "Storeys", Kind::Whole, Holder::Instance).unwrap())
        .unwrap()
        .set_extra(&id("floors"), Value::Whole(3))
        .unwrap()
        .set_text(InfoField::Name, "Harbour House")
        .unwrap()
        .set_issue_date(IssueDate::new(2026, 11, 22).unwrap())
        .unwrap();
    let rows = info.rows(&Units::default());
    let seen: Vec<(&str, Option<&str>)> = rows
        .iter()
        .map(|r| (r.label.as_str(), r.text.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [
            ("Project name", Some("Harbour House")),
            ("Project number", None),
            ("Client", None),
            ("Address", None),
            ("Stage", None),
            ("Issue date", Some("2026-11-22")),
            ("Storeys", Some("3")),
        ]
    );
}

#[test]
fn stored_form_is_the_same_bytes_and_reads_back() {
    let a = ProjectInfo::new()
        .set_text(InfoField::Name, "Harbour House")
        .unwrap()
        .set_issue_date(IssueDate::new(2026, 11, 22).unwrap())
        .unwrap();
    let b = ProjectInfo::new()
        .set_issue_date(IssueDate::new(2026, 11, 22).unwrap())
        .unwrap()
        .set_text(InfoField::Name, "Harbour House")
        .unwrap();
    let bytes = a.to_bytes().unwrap();
    assert_eq!(bytes, b.to_bytes().unwrap());
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        concat!(
            r#"{"format":1,"extras":[],"values":{"#,
            r#""project-info.issue-date":"text:2026-11-22","#,
            r#""project-info.name":"text:Harbour House"}}"#
        )
    );
    assert_eq!(ProjectInfo::from_bytes(&bytes).unwrap(), a);
}

#[test]
fn reading_refuses_what_breaks_a_rule() {
    let good = ProjectInfo::new()
        .set_text(InfoField::Name, "H")
        .unwrap()
        .set_issue_date(IssueDate::new(2026, 11, 22).unwrap())
        .unwrap();
    let text = String::from_utf8(good.to_bytes().unwrap()).unwrap();
    let read = |t: &str| ProjectInfo::from_bytes(t.as_bytes());
    assert_eq!(
        read(&text.replace(r#""format":1"#, r#""format":3"#)),
        Err(InfoError::UnsupportedFormat(3))
    );
    assert!(matches!(
        read(&text.replace("2026-11-22", "2026-02-30")),
        Err(InfoError::BadDate(_))
    ));
    assert!(matches!(
        read(&text.replace("project-info.name", "project-info.nome")),
        Err(InfoError::Malformed(_))
    ));
    assert!(matches!(
        read(&text.replace("text:H", "whole:1")),
        Err(InfoError::Param(_))
    ));
    assert!(matches!(
        read(&text.replace(r#""format":1,"#, r#""format": 1,"#)),
        Err(InfoError::Malformed(_))
    ));
    assert!(matches!(read(""), Err(InfoError::Malformed(_))));
}
