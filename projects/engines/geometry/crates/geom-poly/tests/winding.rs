//! Sparse winding vectors and the tests on them (G127, 11.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_methods,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_poly::winding::{Kept, Term, Winding, WindingError, WindingTest, kept};
use proptest::prelude::*;

fn vector(pairs: &[(u32, i32)]) -> Winding {
    let mut w = Winding::new();
    for (entry, by) in pairs {
        w.add(*entry, *by).unwrap();
    }
    w
}

#[test]
fn the_outside_is_the_empty_list() {
    let w = Winding::new();
    assert_eq!(w.pairs(), []);
    assert_eq!(w.count(7), 0);
    assert_eq!(w, Winding::default());
}

#[test]
fn pairs_stay_sorted_and_zeros_leave() {
    let mut w = vector(&[(9, 1), (2, 1), (5, -2)]);
    assert_eq!(w.pairs(), [(2, 1), (5, -2), (9, 1)]);
    assert_eq!(
        (w.count(2), w.count(5), w.count(9), w.count(3)),
        (1, -2, 1, 0)
    );
    w.add(5, 2).unwrap();
    assert_eq!(w.pairs(), [(2, 1), (9, 1)]);
    w.add(3, 0).unwrap();
    assert_eq!(w.pairs(), [(2, 1), (9, 1)]);
    w.add(2, 3).unwrap();
    assert_eq!(w.count(2), 4);
}

#[test]
fn a_count_past_its_range_changes_nothing() {
    let mut w = vector(&[(1, i32::MAX)]);
    assert_eq!(w.add(1, 1), Err(WindingError::Overflow));
    assert_eq!(w.count(1), i32::MAX);
}

#[test]
fn a_vector_refuses_room_no_allocator_can_give() {
    let mut w = vector(&[(1, 1)]);
    assert_eq!(w.reserve(usize::MAX), Err(WindingError::OutOfMemory));
    assert_eq!(w.reserve(4), Ok(()));
    assert_eq!(w.pairs(), [(1, 1)]);
}

#[test]
fn a_winding_error_says_what_was_wrong() {
    for (error, text) in [
        (WindingError::Overflow, "a winding number left its range"),
        (
            WindingError::OutOfMemory,
            "the allocator refused a winding vector's room",
        ),
        (
            WindingError::BadTest,
            "a winding test's terms do not leave one answer",
        ),
        (WindingError::TooDeep, "a winding test is deeper than 64"),
    ] {
        assert_eq!(error.to_string(), text);
    }
}

#[test]
fn a_test_must_leave_one_answer() {
    let bad = WindingError::BadTest;
    for terms in [
        vec![],
        vec![Term::And],
        vec![Term::Or],
        vec![Term::Not],
        vec![Term::Inside(0), Term::And],
        vec![Term::Inside(0), Term::Or],
        vec![Term::Inside(0), Term::Inside(1)],
    ] {
        assert_eq!(WindingTest::new(terms), Err(bad));
    }
    assert!(WindingTest::new(vec![Term::Inside(0)]).is_ok());
    assert!(WindingTest::new(vec![Term::Inside(0), Term::Not]).is_ok());
    assert!(WindingTest::new(vec![Term::AtLeast(1), Term::Inside(0), Term::Or]).is_ok());
}

/// `depth` answers piled up, the deepest true and the rest false, and folded with `op`.
fn pile(depth: usize, op: Term) -> Result<WindingTest, WindingError> {
    let mut terms = vec![Term::Inside(0)];
    terms.extend(std::iter::repeat_n(Term::Inside(1), depth - 1));
    terms.extend(std::iter::repeat_n(op, depth - 1));
    WindingTest::new(terms)
}

#[test]
fn a_test_is_as_deep_as_sixty_four() {
    assert_eq!(pile(65, Term::Or), Err(WindingError::TooDeep));
    let w = vector(&[(0, 1)]);
    assert!(pile(64, Term::Or).unwrap().holds(&w));
    assert!(!pile(64, Term::And).unwrap().holds(&w));
    assert!(pile(64, Term::Or).unwrap().holds(&vector(&[(1, 1)])));
    assert!(!pile(64, Term::Or).unwrap().holds(&Winding::new()));
}

#[test]
fn each_term_reads_the_vector() {
    let w = vector(&[(1, 1), (2, 2), (3, -1)]);
    let holds = |terms: Vec<Term>| WindingTest::new(terms).unwrap().holds(&w);
    assert!(holds(vec![Term::Inside(1)]));
    assert!(!holds(vec![Term::Inside(0)]));
    assert!(!holds(vec![Term::Inside(3)]));
    assert!(holds(vec![Term::AtLeast(2)]));
    assert!(!holds(vec![Term::AtLeast(3)]));
    assert!(holds(vec![Term::AtLeast(0)]));
    assert!(!holds(vec![Term::Inside(1), Term::Not]));
    assert!(holds(vec![Term::Inside(0), Term::Not]));
    for (a, b, and, or) in [
        (1, 2, true, true),
        (1, 0, false, true),
        (0, 2, false, true),
        (0, 4, false, false),
    ] {
        let both = vec![Term::Inside(a), Term::Inside(b)];
        let with = |op: Term| {
            let mut terms = both.clone();
            terms.push(op);
            holds(terms)
        };
        assert_eq!(with(Term::And), and, "{a} {b}");
        assert_eq!(with(Term::Or), or, "{a} {b}");
    }
}

#[test]
fn answers_keep_their_place_below_the_top() {
    let w = vector(&[(1, 1)]);
    let holds = |terms: Vec<Term>| WindingTest::new(terms).unwrap().holds(&w);
    // true, then false and true folded to false, then the two folded.
    let (t, f) = (Term::Inside(1), Term::Inside(0));
    assert!(holds(vec![t, f, t, Term::And, Term::Or]));
    assert!(!holds(vec![t, f, t, Term::And, Term::And]));
    assert!(!holds(vec![f, f, t, Term::Or, Term::And]));
    assert!(holds(vec![f, t, f, Term::Or, Term::Or]));
}

#[test]
fn the_selections_read_as_they_should() {
    let (none, a, b, both) = (
        Winding::new(),
        vector(&[(0, 1)]),
        vector(&[(1, 1)]),
        vector(&[(0, 1), (1, 1)]),
    );
    let answers = |test: &WindingTest| [&none, &a, &b, &both].map(|w| test.holds(w));
    assert_eq!(answers(&WindingTest::unite()), [false, true, true, true]);
    assert_eq!(
        answers(&WindingTest::intersect(2)),
        [false, false, false, true]
    );
    assert_eq!(
        answers(&WindingTest::subtract(0)),
        [false, true, false, false]
    );
}

#[test]
fn an_open_entry_leaves_only_what_turns_on_it() {
    let w = vector(&[(1, 1), (2, 1)]);
    let read = |terms: Vec<Term>, open: &[u32]| WindingTest::new(terms).unwrap().decided(&w, open);
    assert_eq!(read(vec![Term::Inside(1)], &[]), Some(true));
    assert_eq!(read(vec![Term::Inside(1)], &[1]), None);
    assert_eq!(read(vec![Term::Inside(0)], &[1]), Some(false));
    // Two known, up to two open.
    assert_eq!(read(vec![Term::AtLeast(2)], &[0, 3]), Some(true));
    assert_eq!(read(vec![Term::AtLeast(4)], &[0, 3]), None);
    assert_eq!(read(vec![Term::AtLeast(5)], &[0, 3]), Some(false));
    // Counts of open entries do not count as known.
    assert_eq!(read(vec![Term::AtLeast(2)], &[1, 2]), None);
    let (open, yes, no) = (Term::Inside(7), Term::Inside(1), Term::Inside(0));
    for (a, b, and, or) in [
        (yes, yes, Some(true), Some(true)),
        (yes, no, Some(false), Some(true)),
        (no, no, Some(false), Some(false)),
        (open, yes, None, Some(true)),
        (yes, open, None, Some(true)),
        (open, no, Some(false), None),
        (no, open, Some(false), None),
        (open, open, None, None),
    ] {
        assert_eq!(read(vec![a, b, Term::And], &[7]), and, "{a:?} {b:?}");
        assert_eq!(read(vec![a, b, Term::Or], &[7]), or, "{a:?} {b:?}");
    }
    assert_eq!(read(vec![open, Term::Not], &[7]), None);
    assert_eq!(read(vec![no, Term::Not], &[7]), Some(true));
}

#[test]
fn a_piece_is_kept_where_the_test_differs_across_it() {
    let (out, inside) = (Winding::new(), vector(&[(0, 1)]));
    let test = WindingTest::unite();
    assert_eq!(kept(&test, &out, &inside), Kept::AsIs);
    assert_eq!(kept(&test, &inside, &out), Kept::Flipped);
    assert_eq!(kept(&test, &inside, &inside), Kept::No);
    assert_eq!(kept(&test, &out, &out), Kept::No);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn any_order_of_crossings_gives_one_list(
        crossings in prop::collection::vec((0u32..6, -2i32..=2), 0..40)
    ) {
        let forward = vector(&crossings);
        let mut reversed = crossings.clone();
        reversed.reverse();
        prop_assert_eq!(&forward, &vector(&reversed));
        prop_assert!(forward.pairs().windows(2).all(|p| p[0].0 < p[1].0));
        prop_assert!(forward.pairs().iter().all(|(_, c)| *c != 0));
        for entry in 0..6 {
            let total: i32 = crossings
                .iter()
                .filter(|(e, _)| *e == entry)
                .map(|(_, by)| *by)
                .sum();
            prop_assert_eq!(forward.count(entry), total);
        }
    }
}
