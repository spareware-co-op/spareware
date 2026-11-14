//! Piece G346's branch and bound, round by round (`continuity::join`): how it starts, when it
//! halves and when it stops.
//!
//! - **An ask.** It reads back the measure, tightness and cap it was made with.
//! - **The first round.** The range is cut at every break of the pcurves strictly inside it, and
//!   nowhere else: a pcurve with knots at 0.3 and 0.6 (and at its ends) starts with three pieces.
//! - **The cap.** A round runs only if its halves fit under the cap: a join that starts with one
//!   piece measures 1, then 2, then 4 more, so caps of 2, 3, 6 and 7 measure 1, 3, 3 and 7.
//! - **Tight means tight.** A piece whose bound is exactly the asked tightness above the witness
//!   is not halved, and the run is as asked.
//! - **One double and no further.** A maximum at an end of the range, asked with no slack, is
//!   halved down to the piece one double wide beside it, at the range's end and at its start, and
//!   the run stops long before the cap.
//! - **Pinned rounds.** Joins' counts of pieces and their witnesses, among them one whose first
//!   round leaves a piece wide and a piece already tight, so a change to the halving, the dropping
//!   or the stopping shows.
//!
//! Every run here has a finite budget below a run of the default cap, so a change that would never
//! stop, or would halve to the cap, fails at once.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "tests: oracles and generators, and a failure stops the test"
)]

mod common;

use common::{along, graph, strip};
use geom_analysis::continuity::{self, Ask, Maximum, Upper};
use geom_curve::{CurveKind, Line};
use geom_nurbs::{Basis, Curve};
use geom_run::{Budget, WorkMeter};
use geom_surface::SurfaceKind;
use geom_surface::class_a::{Measure, Side};

/// A meter with a cap of work far above any run here but below a run of 2¹⁴ pieces, so a change
/// that would never stop, or would halve to the default cap, fails at once.
fn bounded() -> WorkMeter<'static> {
    WorkMeter::new(Budget::new(1_000_000, 1 << 28))
}

fn pair<'a>(a: &'a SurfaceKind, b: &'a SurfaceKind, p: &'a CurveKind<2>) -> [Side<'a>; 2] {
    [
        Side {
            surface: a,
            pcurve: p,
            turned: false,
        },
        Side {
            surface: b,
            pcurve: p,
            turned: false,
        },
    ]
}

fn upper(m: &Maximum) -> f64 {
    match m.upper {
        Upper::Bounded(u) => u,
        Upper::Unresolved { from, to } => panic!("unresolved over [{from}, {to}]"),
    }
}

const H: [f64; 3] = [1.0, 1.5, 1.0];

/// The join of the strip and z = x²·h(y), whose curvature jump 2·h(t) peaks at 2.5 at t = 1/2.
fn parabola() -> (SurfaceKind, SurfaceKind, CurveKind<2>) {
    (strip(0.0), graph(&[0.0, 0.0, 1.0], &H), along(0.0))
}

/// The pcurve u = 0, v = t on t in [0, 1] as a degree-1 B-spline with knots at `inner`, so its
/// breaks are 0, `inner` and 1 though it is straight.
fn knotted_line(inner: &[f64]) -> CurveKind<2> {
    let mut knots = vec![0.0, 0.0];
    knots.extend_from_slice(inner);
    knots.extend([1.0, 1.0]);
    let mut pts = vec![[0.0, 0.0]];
    pts.extend(inner.iter().map(|&t| [0.0, t]));
    pts.push([0.0, 1.0]);
    CurveKind::Nurbs(Curve::new(Basis::new(1, knots).unwrap(), &pts).unwrap())
}

#[test]
fn an_ask_reads_back_what_it_was_given() {
    let ask = Ask::new(Measure::NormalAngle, 0.25).unwrap();
    assert_eq!(ask.measure(), Measure::NormalAngle);
    assert_eq!(ask.tight(), 0.25);
    assert_eq!(ask.pieces(), Ask::PIECES);
    assert_eq!(ask.with_pieces(7).pieces(), 7);
    assert_eq!(ask.with_pieces(7).tight(), 0.25);
}

#[test]
fn the_first_round_cuts_at_the_breaks_strictly_inside() {
    let (a, b, _) = parabola();
    for (inner, want) in [(&[0.3, 0.6][..], 3), (&[0.5][..], 2), (&[][..], 1)] {
        let p = knotted_line(inner);
        // A tightness no piece misses: one round only.
        let m = continuity::join(
            pair(&a, &b, &p),
            Ask::new(Measure::Curvature, 1e9).unwrap(),
            &mut bounded(),
        )
        .unwrap();
        assert_eq!(m.pieces, want, "{inner:?}: {m:?}");
        assert!(m.as_asked && m.lower() <= 2.5 && 2.5 <= upper(&m), "{m:?}");
    }
}

#[test]
fn a_round_runs_only_if_its_halves_fit_under_the_cap() {
    let (a, b, p) = parabola();
    for cap in 0..=8 {
        let ask = Ask::new(Measure::Curvature, 0.0).unwrap().with_pieces(cap);
        let m = continuity::join(pair(&a, &b, &p), ask, &mut bounded()).unwrap();
        assert!(m.pieces <= cap.max(1), "cap {cap}: {m:?}");
        assert!(!m.as_asked && m.lower() <= 2.5 && 2.5 <= upper(&m), "{m:?}");
    }
    let ask = |cap| Ask::new(Measure::Curvature, 0.0).unwrap().with_pieces(cap);
    let run = |cap| {
        continuity::join(pair(&a, &b, &p), ask(cap), &mut bounded())
            .unwrap()
            .pieces
    };
    // One piece, then its two halves, then their four: 3 fits a cap of 3, not of 2, and 7 a cap
    // of 7, not of 6.
    assert_eq!([run(2), run(3), run(6), run(7)], [1, 3, 3, 7]);
}

#[test]
fn a_piece_exactly_as_tight_as_asked_is_not_halved() {
    let (a, b, p) = parabola();
    let s = pair(&a, &b, &p);
    let loose = continuity::join(
        s,
        Ask::new(Measure::Curvature, 1e9).unwrap(),
        &mut bounded(),
    )
    .unwrap();
    assert_eq!(loose.pieces, 1, "{loose:?}");
    let gap = upper(&loose) - loose.lower();
    assert!(gap > 0.0, "{loose:?}");
    let exact = continuity::join(
        s,
        Ask::new(Measure::Curvature, gap).unwrap(),
        &mut bounded(),
    )
    .unwrap();
    assert_eq!(exact.pieces, 1, "{exact:?}");
    assert!(exact.as_asked, "{exact:?}");
    assert_eq!(upper(&exact), upper(&loose));
}

/// The strip and z = x·y along u = 0, on `range` with v = `v0` + `dv`·t: the normals part by
/// atan(v), so the angle is largest, π/4, where v = 1.
fn opening(range: (f64, f64), v0: f64, dv: f64) -> (SurfaceKind, SurfaceKind, CurveKind<2>) {
    (
        strip(0.0),
        graph(&[0.0, 1.0], &[0.0, 1.0]),
        CurveKind::Line(Line::new([0.0, v0], [0.0, dv], range).unwrap()),
    )
}

#[test]
fn a_maximum_at_an_end_is_halved_down_to_one_double_and_no_further() {
    // At the range's end, t = 1, and at its start, t = 1 on [1, 2]: next to 1 the doubles are
    // 2⁻⁵³ below and 2⁻⁵² above, so either way about fifty rounds of a few pieces each.
    for (range, v0, dv) in [((0.0, 1.0), 0.0, 1.0), ((1.0, 2.0), 2.0, -1.0)] {
        let (a, b, p) = opening(range, v0, dv);
        let m = continuity::join(
            pair(&a, &b, &p),
            Ask::new(Measure::NormalAngle, 0.0).unwrap(),
            &mut bounded(),
        )
        .unwrap();
        let w = m.witness.unwrap();
        assert!((w.at - 1.0).abs() < 1e-12, "{m:?}");
        let pi4 = core::f64::consts::FRAC_PI_4;
        assert!(w.value.lo() <= pi4 && pi4 <= upper(&m), "{m:?}");
        // The cap is 2¹⁴; halving past single doubles would reach it.
        assert!(!m.as_asked && m.pieces < 4_000, "{m:?}");
    }
}

#[test]
fn pinned_rounds() {
    // The parabola's join, one piece and its halves round by round: (tightness, pieces).
    let (a, b, p) = parabola();
    for (tight, pieces) in [(1e-3, 1), (1e-6, 39), (1e-9, 59)] {
        let m = continuity::join(
            pair(&a, &b, &p),
            Ask::new(Measure::Curvature, tight).unwrap(),
            &mut bounded(),
        )
        .unwrap();
        assert_eq!((m.pieces, m.as_asked), (pieces, true), "{tight}: {m:?}");
        assert_eq!(m.witness.unwrap().at, 0.5, "{tight}: {m:?}");
        assert!(m.lower() <= 2.5 && 2.5 <= upper(&m) && upper(&m) - m.lower() <= tight);
    }
    // The same join starting from the pcurve's breaks: (breaks, tightness, cap, pieces, witness).
    // With a break at 0.6 and a tightness of 0.2, the first round leaves one piece wide and one
    // kept but already tight: only the wide one is halved, and its halves are all that count
    // against the cap.
    let runs: [(&[f64], f64, usize, usize, f64); 11] = [
        (&[0.6], 0.2, 4, 4, 0.45),
        (&[0.6], 0.2, Ask::PIECES, 4, 0.45),
        (&[0.65], 0.1, 4, 4, 0.4875),
        (&[0.45], 0.25, 4, 2, 0.725),
        (&[0.45], 0.25, 6, 2, 0.725),
        (&[0.45], 0.1, 16, 6, 0.5875),
        (&[0.45], 1e-4, Ask::PIECES, 26, 0.501_562_5),
        (&[0.3, 0.6], 0.25, 4, 3, 0.45),
        (&[0.3, 0.6], 0.25, 6, 3, 0.45),
        (&[0.3, 0.6], 0.1, 16, 3, 0.45),
        (&[0.3, 0.6], 1e-4, Ask::PIECES, 25, 0.501_562_5),
    ];
    for (inner, tight, cap, pieces, at) in runs {
        let p = knotted_line(inner);
        let m = continuity::join(
            pair(&a, &b, &p),
            Ask::new(Measure::Curvature, tight)
                .unwrap()
                .with_pieces(cap),
            &mut bounded(),
        )
        .unwrap();
        let ok = m.pieces == pieces && m.as_asked && (m.witness.unwrap().at - at).abs() < 1e-12;
        assert!(ok, "{inner:?} {tight} {cap}: {m:?}");
    }
}
