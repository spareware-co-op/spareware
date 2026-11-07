//! Attack tests from SMLib's confirmed defects, batch 3, fourth line: surface self-intersection
//! (`geom-intersect`, piece G176).
//!
//! Each test feeds the kernel the input of one defect in solidcore's log of upstream SMLib defects
//! (tangency-labs/solidcore, Apache-2.0, commit b4de2149aadbd737268091c51d8390a7d3cdda70,
//! `plan/upstream-issues/`, one file per defect, cited by file name at each test). The case is
//! rebuilt from the defect's description in our own API; no fixture is copied. Each test asserts
//! what the kernel owes for that input: the crossings the geometry says, or a defined refusal, and
//! never a panic, a hang, a NaN or a silent wrong answer.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::type_complexity,
    clippy::similar_names,
    reason = "tests: native arithmetic and indexing, platform maths on an independent oracle (the surface's own equation), prose that names defect files, and a failure stops the test"
)]

use geom_curve::{Basis as Leaf, Frame, Trimmed};
use geom_intersect::IntersectError;
use geom_intersect::selfcross::{SelfCrossing, self_intersect};
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};
use geom_surface::{Offset, Revolution, SurfaceKind, Torus};

const FLOOR: f64 = 1.0 / 128.0;
const CEILING: f64 = 1.0 / 16.0;

/// One turn, in half-turns.
const TURN: f64 = 2.0;

fn meter(work: u64) -> WorkMeter<'static> {
    WorkMeter::new(Budget::of_work(work))
}

fn up() -> Frame<3> {
    Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap()
}

/// A quadratic in the plane y = 0: rho = 2 + 8t - 8t^2, z = 4t.
fn vase() -> SurfaceKind {
    let basis = Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let points = [[2.0, 0.0, 0.0], [6.0, 0.0, 2.0], [2.0, 0.0, 4.0]];
    let profile = Trimmed::whole(Leaf::Nurbs(Curve::new(basis, &points).unwrap())).unwrap();
    SurfaceKind::Revolution(Revolution::new(profile, [0.0; 3], up(), (0.0, TURN)).unwrap())
}

/// Nothing found: no branch, nothing unproven, nothing flagged.
fn clean(found: &SelfCrossing, what: &str) {
    assert!(
        found.branches.is_empty(),
        "{what}: {:?}",
        found.branches.len()
    );
    assert!(
        found.unproven.is_empty(),
        "{what}: {:?}",
        found.unproven.len()
    );
    assert!(
        found.flagged.is_empty(),
        "{what}: {:?}",
        found.flagged.len()
    );
}

/// solidcore plan/upstream-issues/WR-539--torus-check-spindle-self-intersection-has-no-report.md
/// (b4de2149, confirmed by reading the pinned source): every spindle torus with a whole turn of U
/// failed validation with no report, since the test took the angle the minor circle crosses the
/// axis at as 90 degrees plus acos(R / r) instead of plus asin(R / r), and tested the major
/// interval against it. The input is the defect's: the spindle torus R 1, r 2, whose apexes are at
/// 120 degrees either side, as a whole turn of V, from -120 to 150 degrees, the outer sheet from
/// -120 to 120, and the inner sheet from 120 to 240. Its sheets meet only at the two apexes on the
/// axis, which are poles, so the surface has no curve of self-intersection and is searched without
/// a branch, an unproven pair or a flag; a horn torus (R = r) and a ring torus are the same.
#[test]
fn wr_539_a_spindle_torus_has_no_curve_of_self_intersection() {
    let third = 1.0 / 3.0;
    let spans: [(f64, f64); 4] = [
        (0.0, TURN),
        (-2.0 * third, 5.0 * 0.5 * third),
        (-2.0 * third, 2.0 * third),
        (2.0 * third, 4.0 * third),
    ];
    for (major, minor) in [(1.0, 2.0), (1.0, 1.0), (3.0, 1.0)] {
        for v in spans {
            let torus = SurfaceKind::Torus(
                Torus::new([0.0; 3], major, minor, up(), ((0.0, TURN), v)).unwrap(),
            );
            let found = self_intersect(&torus, FLOOR, CEILING, &mut meter(1 << 24)).unwrap();
            clean(&found, &format!("R {major}, r {minor}, v {v:?}"));
        }
    }
}

/// solidcore plan/upstream-issues/HANG-26--the-offset-self-intersection-of-a-surface-of-revolution-s.md
/// (b4de2149, confirmed on the reference build): the self-intersection of the offset of a surface of
/// revolution took 19 s at distance 10 and did not return, or ran out of memory, at distances from
/// 1e3 to 1e300. The input is the defect's: the revolution of a quadratic profile, a vase, offset by
/// 0.5, 10, 100, 1e3, 1e8 and 1e300, and the vase itself. Each search returns, or runs out of its
/// budget, or is refused for a distance outside the kernel's range; none hangs, panics or fails an
/// internal check, and every point it does return is a number. The vase itself, a simple surface,
/// has no crossing.
#[test]
fn hang_26_the_self_intersection_of_a_revolution_s_offset_stops_at_its_budget() {
    clean(
        &self_intersect(&vase(), FLOOR, CEILING, &mut meter(1 << 24)).unwrap(),
        "the vase",
    );
    for distance in [0.5, 10.0, 100.0, 1e3, 1e8, 1e300] {
        let offset = SurfaceKind::Offset(Offset::new(vase(), distance).unwrap());
        match self_intersect(&offset, FLOOR, CEILING, &mut meter(1 << 22)) {
            Ok(found) => {
                for branch in &found.branches {
                    for v in &branch.vertices {
                        assert!(
                            v.iter().all(|i| i.lo().is_finite() && i.hi().is_finite()),
                            "{distance}: {v:?}"
                        );
                    }
                }
            }
            Err(
                IntersectError::BudgetExceeded
                | IntersectError::UnprovenMaths
                | IntersectError::FrameRange
                | IntersectError::InvalidInput,
            ) => {}
            Err(e) => panic!("{distance}: {e:?}"),
        }
    }
}
