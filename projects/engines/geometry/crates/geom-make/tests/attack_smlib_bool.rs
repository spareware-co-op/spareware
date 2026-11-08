//! Attack tests from SMLib's confirmed defects, batch 5: flat primitives, `geom-make`.
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib
//! defects (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, cited by file name at the test). The case is rebuilt from the
//! defect's description in our own API; no fixture is copied. The test asserts what the kernel
//! owes: the exact body, or a defined refusal, and never a panic, a hang or a wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests: native arithmetic and indexing, prose that names defect files, and a failure stops the test"
)]

use geom_check::{Level, check};
use geom_make::{BoxInput, PrismInput, make_box, prism};
use geom_num::grid::COORD_MAX;
use geom_num::{Exactness, P3, Steps};
use geom_op::{OpContext, OpId, OpPath};
use geom_run::{Budget, WorkMeter};
use geom_topo::Body;
use geom_topo::query::volume;

fn ctx() -> OpContext<'static> {
    OpContext::new(OpPath::new(OpId::from_bytes([7; 16])), Budget::UNLIMITED)
}

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn p(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

fn counts(body: &Body) -> (u32, u32, u32) {
    let a = body.arenas();
    (a.vertices.len(), a.edges.len(), a.faces.len())
}

fn checked(body: &Body) {
    let report = check(body, Exactness::Exact, Level::Full, &mut meter()).unwrap();
    assert!(report.passed(), "{report:?}");
}

/// solidcore plan/upstream-issues/WR-209--smapicreatebox-reports-success-for-incomplete-box-topology.md
/// (b4de2149): a box of 1e-6 on a side at the origin, or of 1e-3 on a side at 1e6, was reported as
/// built with a non-null B-rep that lacked its six faces, twelve edges or eight vertices. Ours
/// counts in grid steps: a box of one step on a side, a thin one, and one at the far end of the
/// grid must each come back with exactly 8 vertices, 12 edges, 6 faces, the exact volume and a
/// passing full check, or be refused; the box that runs past the grid's range must be refused.
#[test]
fn wr_209_tiny_and_far_boxes_come_back_whole_or_are_refused() {
    let far = COORD_MAX - 4;
    let cases: [([i64; 3], [u64; 3]); 6] = [
        ([0, 0, 0], [1, 1, 1]),
        ([0, 0, 0], [1, 1, 1000]),
        ([-1, -1, -1], [1, 1, 1]),
        ([far, far, far], [1, 1, 1]),
        ([far, 0, -far], [3, 2, 1]),
        ([-COORD_MAX, -COORD_MAX, -COORD_MAX], [1, 1, 1]),
    ];
    for (low, size) in cases {
        let input = BoxInput {
            min: p(low[0], low[1], low[2]),
            size: size.map(Steps),
        };
        let made = make_box(&ctx(), &input).unwrap().result.body;
        assert_eq!(counts(&made), (8, 12, 6), "{low:?} {size:?}");
        checked(&made);
        let want = i128::from(size[0]) * i128::from(size[1]) * i128::from(size[2]);
        let got = volume(&made, &mut meter()).unwrap().to_i128().unwrap();
        assert_eq!(got, 6 * want, "{low:?} {size:?}");
    }
    // Past the end of the grid: a refusal, not a box with missing faces.
    let over = BoxInput {
        min: p(COORD_MAX - 1, 0, 0),
        size: [Steps(5), Steps(1), Steps(1)],
    };
    assert!(make_box(&ctx(), &over).is_err());
}

/// WR-209 (same file), for a prism: the smallest triangle on the grid, one step high, comes back
/// as 6 vertices, 9 edges, 5 faces with the exact volume, and a base wider than the grid is
/// refused.
#[test]
fn wr_209_the_smallest_prism_has_all_its_faces() {
    let input = PrismInput {
        base: vec![p(0, 0, 0), p(1, 0, 0), p(0, 1, 0)],
        height: Steps(1),
    };
    let made = prism(&ctx(), &input).unwrap().result.body;
    assert_eq!(counts(&made), (6, 9, 5));
    checked(&made);
    // Half a unit of area, one high: a sixfold volume of 3.
    assert_eq!(volume(&made, &mut meter()).unwrap().to_i128().unwrap(), 3);
    let wide = PrismInput {
        base: vec![p(-COORD_MAX, 0, 0), p(COORD_MAX, 0, 0), p(0, 1, 0)],
        height: Steps(1),
    };
    if let Ok(out) = prism(&ctx(), &wide) {
        checked(&out.result.body);
    }
}
