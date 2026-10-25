//! Attacks on piece G13 (the recursion check). Each test states what the board ("a tool lists
//! every recursive cycle in kernel crates; a cycle without a stated depth bound fails CI; a test at
//! each stated bound runs on a small wasm32 stack"), chapter 28.8 or the check's own guarantees
//! (`src/recursion.rs` and its modules) require, and fails while the check does not do it.
//!
//! Every attack builds a small crate in memory and runs the analysis on it, as the binary runs it
//! on the workspace: the crate stands for a kernel crate that ships, the standard library is the
//! check's model. A cycle is named by the functions it runs, as `recursion list` prints them.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fixtures; the rules are for kernel code"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};

use geom_test::recursion::model::{CrateSource, Role};
use geom_test::recursion::{OwnedReader, Report, Rule, analyse};

const LIB: &str = "projects/engines/geometry/crates/geom-seed/src/lib.rs";
const TESTS: &str = "projects/engines/geometry/crates/geom-seed/tests/recursion.rs";

/// Analyses the crate `geom-seed` whose `lib.rs` is `lib` (and, when given, whose
/// `tests/recursion.rs` is `tests`).
fn run_with(lib: &str, tests: Option<&str>) -> Report {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    files.insert(LIB.to_owned(), lib.to_owned());
    let mut sources = vec![CrateSource {
        package: "geom-seed".to_owned(),
        lib: "geom_seed".to_owned(),
        folder: "projects/engines/geometry/crates/geom-seed".to_owned(),
        root: LIB.to_owned(),
        role: Role::Ships,
    }];
    if let Some(t) = tests {
        files.insert(TESTS.to_owned(), t.to_owned());
        sources.push(CrateSource {
            package: "geom-seed".to_owned(),
            lib: "recursion".to_owned(),
            folder: "projects/engines/geometry/crates/geom-seed".to_owned(),
            root: TESTS.to_owned(),
            role: Role::BoundTests,
        });
    }
    let read: OwnedReader =
        Box::new(move |p: &str| files.get(p).cloned().ok_or_else(|| format!("no file {p}")));
    analyse(sources, read)
}

fn run(lib: &str) -> Report {
    run_with(lib, None)
}

/// The problems other than unbounded cycles and recursive types: code the check could not read,
/// statement errors.
fn unread(r: &Report) -> Vec<String> {
    r.problems
        .iter()
        .filter(|p| p.rule == Rule::Unread)
        .map(ToString::to_string)
        .collect()
}

/// Each cycle's members, as sets of the names after the crate's own prefix.
fn cycles(r: &Report) -> Vec<BTreeSet<String>> {
    r.cycles
        .iter()
        .map(|c| {
            c.members
                .iter()
                .map(|m| m.replace("geom_seed::", ""))
                .collect()
        })
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

/// Asserts the crate reads without problems and has exactly the cycles `want`.
#[track_caller]
fn assert_cycles(r: &Report, want: &[&[&str]]) {
    assert!(
        unread(r).is_empty(),
        "the check could not read the crate: {:#?}",
        unread(r)
    );
    let got = cycles(r);
    let want: Vec<BTreeSet<String>> = want.iter().map(|w| set(w)).collect();
    assert_eq!(got, want, "cycles");
}

// ---------------------------------------------------------------------------------------------
// "A tool lists every recursive cycle": the routes a call can take.
// ---------------------------------------------------------------------------------------------

#[test]
fn direct_and_mutual_recursion_are_cycles() {
    let r = run(
        "pub fn f(n: u32) -> u32 { if n == 0 { 0 } else { f(n - 1) } }\n\
                 pub fn g(n: u32) -> u32 { if n == 0 { 0 } else { h(n - 1) } }\n\
                 pub fn h(n: u32) -> u32 { g(n) }\n\
                 pub fn leaf(n: u32) -> u32 { n }",
    );
    assert_cycles(&r, &[&["f"], &["g", "h"]]);
}

#[test]
fn a_method_on_the_receivers_type_is_followed_and_a_fields_method_of_the_same_name_is_not_a_cycle()
{
    let r = run("pub struct A { v: Vec<u8> } pub struct B { a: A }\n\
                 impl A { pub fn len(&self) -> usize { self.v.len() } }\n\
                 impl B { pub fn len(&self) -> usize { self.a.len() } pub fn walk(&self, n: u32) { if n > 0 { self.walk(n - 1) } } }");
    assert_cycles(&r, &[&["B::walk"]]);
}

#[test]
fn recursion_through_a_trait_implementation_and_a_generic_function_is_a_cycle() {
    let r = run("pub trait Visit { fn visit(&self, depth: u32); }\n\
                 pub struct Leaf;\n\
                 impl Visit for Leaf { fn visit(&self, depth: u32) { if depth > 0 { go(self, depth - 1) } } }\n\
                 pub fn go<T: Visit>(t: &T, depth: u32) { t.visit(depth) }");
    assert_cycles(&r, &[&["<Leaf as Visit>::visit", "go"]]);
}

#[test]
fn a_closure_handed_to_a_generic_function_is_called_by_it() {
    let r = run(
        "pub fn apply<F: Fn(u32) -> u32>(f: F, x: u32) -> u32 { f(x) }\n\
                 pub fn r(x: u32) -> u32 { if x == 0 { 0 } else { apply(|y| r(y - 1), x) } }",
    );
    assert_cycles(&r, &[&["apply", "r"]]);
}

#[test]
fn the_standard_library_calls_back_closures_and_trait_implementations() {
    // Option::map runs its closure; an iterator's map runs it lazily, in the consumer; sort runs
    // Ord::cmp; sort_by runs the function it is handed.
    let r = run("use core::cmp::Ordering;\n\
                 pub fn via_option(x: Option<u32>) -> Option<u32> { x.map(|v| via_option(Some(v)).unwrap_or(0)) }\n\
                 pub fn via_iter(v: &[u32]) -> usize { v.iter().map(|_| via_iter(&[])).count() }\n\
                 #[derive(PartialEq, Eq, PartialOrd)] pub struct K(u8);\n\
                 impl Ord for K { fn cmp(&self, o: &Self) -> Ordering { let mut v = vec![K(1)]; v.sort(); Ordering::Equal } }\n\
                 pub fn by(a: &u8, b: &u8) -> Ordering { let mut v = vec![1u8]; v.sort_by(by); Ordering::Equal }");
    let got = cycles(&r);
    assert!(unread(&r).is_empty(), "{:#?}", unread(&r));
    for want in [
        set(&["via_option"]),
        set(&["via_iter"]),
        set(&["<K as Ord>::cmp"]),
        set(&["by"]),
    ] {
        assert!(got.contains(&want), "{want:?} missing from {got:?}");
    }
}

#[test]
fn recursion_through_a_trait_object_is_a_cycle() {
    let r = run("pub trait Node { fn visit(&self); }\n\
                 pub struct A { next: Option<Box<dyn Node>> }\n\
                 impl Node for A { fn visit(&self) { if let Some(n) = &self.next { n.visit() } } }");
    assert_cycles(&r, &[&["<A as Node>::visit"]]);
}

#[test]
fn recursion_through_a_function_pointer_is_a_cycle() {
    let r = run("pub fn call(g: fn(u32), n: u32) { g(n) }\n\
                 pub fn h(n: u32) { if n > 0 { call(h, n - 1) } }");
    assert_cycles(&r, &[&["call", "h"]]);
}

#[test]
fn a_closure_returned_as_impl_trait_is_followed_where_it_is_called() {
    let r = run("pub fn make() -> impl Fn(u32) { |n| again(n) }\n\
                 pub fn again(n: u32) { if n > 0 { make()(n - 1) } }");
    assert_cycles(&r, &[&["again", "make"]]);
}

#[test]
fn operators_question_marks_and_formatting_call_their_implementations() {
    let r = run("use core::ops::Add;\n\
                 #[derive(Clone, Copy)] pub struct V(u8);\n\
                 impl Add for V { type Output = V; fn add(self, o: V) -> V { if o.0 == 0 { self } else { V(o.0 - 1) + self } } }\n\
                 #[derive(Debug)] pub struct E1; #[derive(Debug)] pub struct E2;\n\
                 impl From<E1> for E2 { fn from(_: E1) -> E2 { f(0).unwrap_err() } }\n\
                 pub fn g() -> Result<(), E1> { Err(E1) }\n\
                 pub fn f(n: u8) -> Result<(), E2> { if n > 0 { g()?; } Ok(()) }\n\
                 pub struct T(u8);\n\
                 impl core::fmt::Display for T { fn fmt(&self, w: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { if self.0 > 0 { write!(w, \"{}\", T(self.0 - 1)) } else { Ok(()) } } }");
    let got = cycles(&r);
    assert!(unread(&r).is_empty(), "{:#?}", unread(&r));
    for want in [
        set(&["<V as Add>::add"]),
        set(&["<E2 as From>::from", "f"]),
        set(&["<T as Display>::fmt"]),
    ] {
        assert!(got.contains(&want), "{want:?} missing from {got:?}");
    }
}

#[test]
fn a_trait_method_named_as_a_value_is_called_on_what_it_is_handed() {
    let r = run("pub trait Cancel { fn requested(&self) -> bool; }\n\
                 pub struct Flag(u8);\n\
                 impl Cancel for Flag { fn requested(&self) -> bool { poll(Some(&Flag(self.0))) } }\n\
                 pub fn poll(c: Option<&Flag>) -> bool { c.is_some_and(Cancel::requested) }");
    assert_cycles(&r, &[&["<Flag as Cancel>::requested", "poll"]]);
}

#[test]
fn default_bodies_and_generic_self_recursion_are_cycles() {
    let r = run(
        "pub trait T { fn a(&self, n: u32) { if n > 0 { self.b(n - 1) } } fn b(&self, n: u32) { self.a(n) } }\n\
                 pub struct S; impl T for S {}\n\
                 pub fn each<X: Clone>(x: X, n: u32) { if n > 0 { each(x.clone(), n - 1) } }",
    );
    assert_cycles(&r, &[&["T::a", "T::b"], &["each"]]);
}

#[test]
fn deref_index_and_drop_are_calls_too() {
    let r = run("use core::ops::{Deref, Index};\n\
                 pub struct W(u8);\n\
                 impl Deref for W { type Target = u8; fn deref(&self) -> &u8 { let w = W(self.0); let _ = w.count_ones(); &self.0 } }\n\
                 pub struct I(u8);\n\
                 impl Index<usize> for I { type Output = u8; fn index(&self, i: usize) -> &u8 { let _ = I(0)[i]; &self.0 } }\n\
                 pub struct D(u8);\n\
                 impl Drop for D { fn drop(&mut self) { if self.0 > 0 { let _inner = D(self.0 - 1); } } }");
    let got = cycles(&r);
    assert!(unread(&r).is_empty(), "{:#?}", unread(&r));
    for want in [
        set(&["<W as Deref>::deref"]),
        set(&["<I as Index>::index"]),
        set(&["<D as Drop>::drop"]),
    ] {
        assert!(got.contains(&want), "{want:?} missing from {got:?}");
    }
}

#[test]
fn recursive_types_are_listed_with_their_derived_implementations() {
    let r = run(
        "#[derive(Clone, Debug)] pub struct Tree { kids: Vec<Tree> }\n\
                 #[derive(Clone)] pub enum List { Nil, Cons(u8, Box<List>) }\n\
                 pub struct Flat { x: u8 }",
    );
    assert!(unread(&r).is_empty(), "{:#?}", unread(&r));
    let types: Vec<BTreeSet<String>> = r
        .types
        .iter()
        .map(|t| {
            t.members
                .iter()
                .map(|m| m.replace("geom_seed::", ""))
                .collect()
        })
        .collect();
    assert_eq!(types, [set(&["List"]), set(&["Tree"])]);
    let got = cycles(&r);
    assert!(got.contains(&set(&["<Tree as Clone>::clone"])), "{got:?}");
    assert!(got.contains(&set(&["<List as Clone>::clone"])), "{got:?}");
}

// ---------------------------------------------------------------------------------------------
// No false cycles: a cycle the check lists must be one the program can run.
// ---------------------------------------------------------------------------------------------

#[test]
fn generic_implementations_at_nested_types_are_not_cycles() {
    // `W<W<u8>>::clone` calls `W<u8>::clone`, another copy: no recursion.
    let r = run("#[derive(Clone, PartialEq, Debug)] pub struct W<T>(T);\n\
                 pub trait Size { fn size(&self) -> usize; }\n\
                 impl Size for u8 { fn size(&self) -> usize { 1 } }\n\
                 impl<T: Size> Size for Vec<T> { fn size(&self) -> usize { self.iter().map(Size::size).count() } }\n\
                 pub fn use_them() -> usize { let w = W(W(1u8)); let _ = w.clone() == w; vec![vec![1u8]].size() }");
    assert_cycles(&r, &[]);
}

#[test]
fn test_code_and_the_standard_librarys_model_are_not_listed() {
    let r = run("pub fn ok() {}\n\
                 #[cfg(test)] mod tests { fn r() { r() } }");
    assert_cycles(&r, &[]);
}

// ---------------------------------------------------------------------------------------------
// Code the check cannot read is a problem, never a pass.
// ---------------------------------------------------------------------------------------------

#[test]
fn code_the_check_cannot_read_is_reported() {
    for (lib, what) in [
        ("pub fn f() { nowhere::g() }", "resolves to nothing"),
        (
            "pub fn f() { core::ptr::nonexistent_function() }",
            "resolves to nothing",
        ),
        ("pub fn f() -> u8 { mystery!(1) }", "macro"),
    ] {
        let r = run(lib);
        let u = unread(&r);
        assert!(u.iter().any(|p| p.contains(what)), "{lib}: {u:#?}");
    }
}

#[test]
fn polymorphic_recursion_is_reported_rather_than_followed_forever() {
    let r = run("pub fn grow<T: Clone>(x: T, n: u32) { if n > 0 { grow(vec![x], n - 1) } }");
    assert!(
        !r.problems.is_empty() || !r.cycles.is_empty(),
        "infinitely many copies must not pass silently"
    );
}

// ---------------------------------------------------------------------------------------------
// "A cycle without a stated depth bound fails": the statements.
// ---------------------------------------------------------------------------------------------

const STATED: &str = "//! The seed crate.\n\
//!\n\
//! Recursion:\n\
//!\n\
//! - Cycle: `walk`.\n\
//!   Bound: 64 (`MAX_DEPTH`).\n\
//!   Why: each call goes one level down, and `walk` stops at `MAX_DEPTH`.\n\
//!   Test: `walk_at_its_bound`.\n\
pub const MAX_DEPTH: u32 = 64;\n\
pub fn walk(n: u32) -> u32 { if n == 0 || n > MAX_DEPTH { 0 } else { walk(n - 1) + 1 } }\n";

const BOUND_TEST: &str =
    "#[test]\nfn walk_at_its_bound() { assert_eq!(geom_seed::walk(64), 64); }\n";

#[test]
fn a_cycle_without_a_statement_fails() {
    let r = run("pub fn walk(n: u32) -> u32 { if n == 0 { 0 } else { walk(n - 1) } }");
    assert!(
        r.problems
            .iter()
            .any(|p| p.rule == Rule::Unbounded && p.what.contains("walk")),
        "{:#?}",
        r.problems
    );
}

#[test]
fn a_stated_cycle_with_its_test_passes() {
    let r = run_with(STATED, Some(BOUND_TEST));
    assert!(r.problems.is_empty(), "{:#?}", r.problems);
    assert_eq!(r.bound_tests.len(), 1);
    assert_eq!(r.bound_tests[0].2, ["walk_at_its_bound"]);
}

#[test]
fn a_statement_untrue_to_the_code_fails() {
    // The wrong function, a bound its constant does not hold, a test that is missing, and a test
    // that does not reach the cycle.
    let wrong_name = STATED.replace("Cycle: `walk`", "Cycle: `walked`");
    let r = run_with(&wrong_name, Some(BOUND_TEST));
    assert!(
        r.problems.iter().any(|p| p.rule == Rule::Stale),
        "{:#?}",
        r.problems
    );
    let wrong_bound = STATED.replace("Bound: 64", "Bound: 65");
    let r = run_with(&wrong_bound, Some(BOUND_TEST));
    assert!(
        r.problems.iter().any(|p| p.rule == Rule::Bound),
        "{:#?}",
        r.problems
    );
    let r = run_with(STATED, None);
    assert!(
        r.problems.iter().any(|p| p.rule == Rule::Test),
        "{:#?}",
        r.problems
    );
    let r = run_with(
        STATED,
        Some("#[test]\nfn walk_at_its_bound() { assert_eq!(1, 1); }\n"),
    );
    assert!(
        r.problems
            .iter()
            .any(|p| p.rule == Rule::Test && p.what.contains("does not reach")),
        "{:#?}",
        r.problems
    );
}

// ---------------------------------------------------------------------------------------------
// Deterministic.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_same_tree_gives_the_same_report() {
    let lib = "pub fn g(n: u32) -> u32 { if n == 0 { 0 } else { h(n - 1) } } pub fn h(n: u32) -> u32 { g(n) }";
    let a = run(lib);
    let b = run(lib);
    assert_eq!(cycles(&a), cycles(&b));
    assert_eq!(a.problems, b.problems);
    assert_eq!(a.instances, b.instances);
    assert_eq!(a.edges, b.edges);
}
