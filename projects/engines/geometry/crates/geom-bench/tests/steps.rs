//! The step mix (chapter 23.2): each corpus is measured, and each step is reached where it should
//! be (G124).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "tests: native arithmetic, and a failure stops the test"
)]

use geom_bench::steps::mix;
use geom_num::Step;

#[test]
fn the_same_corpora_give_the_same_counts() {
    assert_eq!(mix(), mix());
}

#[test]
fn a_building_reaches_every_early_step() {
    let mixes = mix();
    let building = &mixes[0];
    assert_eq!(building.corpus, "buildings");
    for step in [Step::Identity, Step::Narrow, Step::Axis] {
        assert!(building.settled_by(step) > 0, "{step:?}");
    }
}

#[test]
fn a_part_takes_the_filter_and_a_tie_the_exact_path() {
    let mixes = mix();
    let part = &mixes[1];
    assert_eq!(part.corpus, "mechanical parts");
    assert!(part.settled_by(Step::Identity) > 0);
    assert!(part.settled_by(Step::Filter) > 0);
    let near = &mixes[2];
    assert_eq!(near.corpus, "near-degenerate");
    assert!(near.settled_by(Step::Axis) > 0);
    assert!(near.settled_by(Step::Exact) + near.settled_by(Step::Filter) > 0);
}
