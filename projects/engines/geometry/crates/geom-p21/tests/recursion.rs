//! geom-p21's recursive types at their stated bound (chapter 28.8, `value`'s Recursion field): a
//! parameter nested `MAX_DEPTH` lists deep is built, cloned, compared, printed and dropped, and one
//! list more is refused. The recursion gate runs this on a small stack.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "tests: a failure stops the test"
)]

use geom_p21::{List, MAX_DEPTH, Param, TooDeepError};

#[test]
fn deepest_param_drops_clones_compares_and_prints() {
    let mut deepest = Param::Typed("LEAF".into(), List::new(vec![Param::Real(1.5)]).unwrap());
    while deepest.depth() < MAX_DEPTH {
        deepest = Param::List(List::new(vec![Param::Null, deepest]).unwrap());
    }
    assert_eq!(deepest.depth(), MAX_DEPTH);
    let copy = deepest.clone();
    assert_eq!(copy, deepest);
    let printed = format!("{copy:?}");
    assert!(printed.contains("LEAF"));
    assert_eq!(List::new(vec![copy]), Err(TooDeepError));
    drop(deepest);
}
