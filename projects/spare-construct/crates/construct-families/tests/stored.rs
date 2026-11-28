//! A family as a document holds it (CON-A7-F2-P3, piece A73): written and read back to the same
//! family, read strictly (another spelling, another content id or an unknown source is refused),
//! and what a document takes: every library template passes; a recipe too long, one the checker
//! refuses, one with no type and one whose type goes over the evaluator's limits are refused,
//! naming the type.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::Fixture;
use construct_families::{
    Limits, LoadRefusal, MOST_RECIPE_BYTES, Recipe, Source, StoredFamily, loadable, read_text,
};
use kit_core::Id;

const TEMPLATES: [&str; 3] = [
    include_str!("../../../library/doors-windows/single-swing-door.json"),
    include_str!("../../../library/doors-windows/fixed-window.json"),
    include_str!("../../../library/doors-windows/sliding-window.json"),
];

fn window() -> Recipe {
    Recipe::from_json(TEMPLATES[1]).unwrap()
}

/// A stored family is its recipe's one written form, that form's content id and its source's
/// written form; reading those back gives the same family, and anything else is refused.
#[test]
fn a_stored_family_reads_back_as_written_and_strictly() {
    let sources = [
        Source::Library(Id::parse("library.doors-windows.fixed-window").unwrap()),
        Source::Text,
        Source::Generated("fixed-window".to_owned()),
    ];
    for source in sources {
        let family = StoredFamily::new(window(), source.clone());
        assert_eq!(family.content, family.recipe.content_id());
        let read =
            StoredFamily::read(&family.text(), family.content.as_str(), &source.write()).unwrap();
        assert_eq!(read, family);
        assert_eq!(Source::read(&source.write()), Some(source));
    }
    assert_eq!(
        Source::Library(Id::parse("a.b").unwrap()).write(),
        "library:a.b"
    );
    let family = StoredFamily::new(window(), Source::Text);
    // the same recipe in another spelling
    let spaced = format!(" {}", family.text());
    let e = StoredFamily::read(&spaced, family.content.as_str(), "text").unwrap_err();
    assert_eq!(e, "its recipe is not in its one written form");
    let e = StoredFamily::read(&family.text(), "sha256:00", "text").unwrap_err();
    assert!(
        e.starts_with("its content id sha256:00 is not that of its recipe"),
        "{e}"
    );
    let e = StoredFamily::read(&family.text(), family.content.as_str(), "shop").unwrap_err();
    assert_eq!(e, "shop is not a family's source");
    assert!(StoredFamily::read("{}", family.content.as_str(), "text").is_err());
    for bad in [
        "",
        "library:",
        "generated:",
        "generated:Fixed",
        "library:-x",
    ] {
        assert_eq!(Source::read(bad), None, "{bad}");
    }
    let keys: Vec<String> = family.type_keys().iter().map(ToString::to_string).collect();
    assert_eq!(keys.len(), 8);
    assert_eq!(keys[0], "w600x600");
}

/// Every template of the library is taken; the checks run in order, each refusal saying why.
#[test]
fn a_document_takes_a_recipe_that_checks_and_builds() {
    let fixture = Fixture::new();
    for text in TEMPLATES {
        let recipe = read_text(text).unwrap();
        loadable(&recipe, &fixture.library(), Limits::DEFAULT).unwrap();
    }
    // too long, before it is read
    let long = format!("{}{}", TEMPLATES[1], " ".repeat(MOST_RECIPE_BYTES));
    let e = read_text(&long).unwrap_err();
    assert!(matches!(e, LoadRefusal::TooLong { .. }));
    assert!(
        e.to_string().ends_with("over the 262144 a document takes"),
        "{e}"
    );
    assert!(matches!(read_text("[]"), Err(LoadRefusal::Unreadable(_))));
    // the checker's problems
    let mut odd = window();
    odd.category = construct_categories::CategoryId(999_999);
    let e = loadable(&odd, &fixture.library(), Limits::DEFAULT).unwrap_err();
    assert!(matches!(e, LoadRefusal::Problems(_)));
    assert!(
        e.to_string().starts_with("the recipe does not check: "),
        "{e}"
    );
    // nothing to place
    let mut none = window();
    none.types.clear();
    none.type_table = None;
    let e = loadable(&none, &fixture.library(), Limits::DEFAULT).unwrap_err();
    assert_eq!(e.to_string(), "the recipe names no type");
    // over the evaluator's limits: the first type is named
    let mut tight = Limits::DEFAULT;
    tight.solid_steps = 1;
    let e = loadable(&window(), &fixture.library(), tight).unwrap_err();
    let LoadRefusal::Type { key, .. } = &e else {
        panic!("{e}");
    };
    assert_eq!(key.as_str(), "w600x600");
    assert!(
        e.to_string()
            .starts_with("its type w600x600 does not build: "),
        "{e}"
    );
}
