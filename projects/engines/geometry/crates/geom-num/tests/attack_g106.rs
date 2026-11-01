//! Attacks on G106's claim for the numbers layer: reference mode leaves no generated filter running.
//! Every call of a generated filter outside this crate's own tests goes through `filtered`, so the
//! `reference` feature reaches them all, and `FILTERS` says which build this is.

use geom_num::predicate::FILTERS;

const SOURCES: [(&str, &str); 3] = [
    ("predicate.rs", include_str!("../src/predicate.rs")),
    ("plane.rs", include_str!("../src/plane.rs")),
    ("crossing.rs", include_str!("../src/crossing.rs")),
];

/// The lines of `text` before its unit tests that call a generated filter outside `filtered`.
fn unfiltered(text: &str) -> Vec<String> {
    let code = text.split("#[cfg(test)]").next().unwrap_or(text);
    let mut found = Vec::new();
    let mut last = "";
    for line in code.lines() {
        let code_line = !line.trim_start().starts_with("//");
        let calls = line.contains("generated::") && line.contains("::filter(");
        if code_line && calls && !line.contains("filtered(|| ") && !last.contains("filtered(|| {") {
            found.push(line.trim().to_owned());
        }
        last = line;
    }
    found
}

#[test]
fn every_filter_call_goes_through_the_switch() {
    let mut calls = 0;
    for (name, text) in SOURCES {
        assert_eq!(unfiltered(text), Vec::<String>::new(), "{name}");
        calls += text.matches("filtered(||").count();
    }
    // The four explicit predicates, the line-plane pair, the three-plane side and the crossing pair.
    assert_eq!(calls, 9);
}

#[test]
fn a_call_outside_the_switch_is_found() {
    let seeded = "fn a() {\n    match generated::orient2d::filter(a, b, c) {}\n}\n";
    assert_eq!(unfiltered(seeded).len(), 1);
    let held = "fn a() {\n    filtered(|| generated::orient2d::filter(a, b, c));\n}\n";
    assert_eq!(unfiltered(held).len(), 0);
    let multi = "fn a() {\n    filtered(|| {\n        generated::tpi_side::filter(\n";
    assert_eq!(unfiltered(multi).len(), 0);
    let tested = "#[cfg(test)]\nfn a() {\n    generated::orient2d::filter(a, b, c);\n}\n";
    assert_eq!(unfiltered(tested).len(), 0);
}

#[test]
fn the_build_says_whether_it_filters() {
    assert_eq!(FILTERS, !cfg!(feature = "reference"));
}
