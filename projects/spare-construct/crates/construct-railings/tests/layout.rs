//! Railings laid out, against hand-worked figures: balusters counted and placed by their spacing
//! and justification, lengths along a pitch, the step at a run's foot, the line round a stair's
//! landing, and every point within half a step of where it truly is.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

mod common;

use common::{MM, at, laid, laid_stair, mm, on_stair, sketched, spaced, stair, steps};
use construct_railings::{Hand, Justify, RailingError, RailingType, Side, StairHost, Vertex};
use construct_stairs::Turn;

/// By hand: a 3 m railing at 100 mm takes ceil(3000 / 100) - 1 = 29 balusters, however they are
/// justified (from the start at 100, 200, ... 2900 mm), and two posts; its rail is 3 m long.
#[test]
fn a_three_metre_railing_at_100_mm_takes_29_balusters() {
    for justify in [Justify::Start, Justify::Centre, Justify::End] {
        let mut kind = spaced(100);
        kind.balusters.justify = justify;
        let layout = laid(&sketched(kind, vec![at(0, 0), at(3000, 0)]), None);
        assert_eq!(layout.balusters, 29, "{justify:?}");
        assert_eq!(layout.posts, 2);
        assert_eq!(layout.rail_length, mm(3000));
        let balusters = &layout.lines[0].segments[0].balusters;
        assert_eq!(balusters[0].centre, [100 * MM, 0]);
        assert_eq!(balusters[28].centre, [2900 * MM, 0]);
        // 100 mm apart, 20 mm deep: 80 mm clear between them; 65 mm from a post (100 - 25 - 10).
        assert_eq!(layout.lines[0].segments[0].widest.shown, mm(80));
    }
}

/// By hand: 3.05 m at 100 mm takes 30 centred, 75 mm from each post; from the start the 30th
/// would stand at 3000 mm, 50 mm from the end post's centre, inside the 50 mm post, so 29 stand.
#[test]
fn balusters_inside_a_post_are_left_out() {
    let mut kind = spaced(100);
    let centred = laid(&sketched(kind.clone(), vec![at(0, 0), at(3050, 0)]), None);
    assert_eq!(centred.balusters, 30);
    assert_eq!(
        centred.lines[0].segments[0].balusters[0].centre,
        [75 * MM, 0]
    );
    kind.balusters.justify = Justify::Start;
    let started = laid(&sketched(kind, vec![at(0, 0), at(3050, 0)]), None);
    assert_eq!(started.balusters, 29);
    // The gap left by the one left out: from 2900 mm to the end post, 150 - 10 - 25 mm.
    assert_eq!(started.lines[0].segments[0].widest.shown, mm(115));
}

/// A path at 45 degrees, 1414.21 mm long, at 110 mm: ceil(12.86) - 1 = 12 balusters, centred, the
/// first 707.107 - 5.5 x 110 = 102.107 mm along, at 72.206 mm on each axis (462,082.54 steps,
/// rounded once to 462,083). Its rail is 9,050,967 steps (1414.21 mm), rounded once.
#[test]
fn a_path_at_any_angle_is_laid_out_exactly_and_rounded_once() {
    let layout = laid(
        &sketched(RailingType::standard(), vec![at(0, 0), at(1000, 1000)]),
        None,
    );
    assert_eq!(layout.balusters, 12);
    let first = &layout.lines[0].segments[0].balusters[0];
    assert_eq!(first.centre, [462_083, 462_083]);
    assert_eq!(steps(layout.rail_length), 9_050_967);
    // Every baluster within half a step of its true place, and the line's two halves alike.
    let balusters = &layout.lines[0].segments[0].balusters;
    for (k, b) in balusters.iter().enumerate() {
        let t = 1000.0 * 2f64.sqrt() / 2.0 + (k as f64 + 1.0 - 6.5) * 110.0;
        let truth = t / 2f64.sqrt() * MM as f64;
        assert!((b.centre[0] as f64 - truth).abs() <= 0.5, "{k}");
        assert_eq!(b.centre[0], b.centre[1]);
    }
}

/// The stair of 17 risers: main private, 3060 mm from floor to floor, 17 risers of 180 mm on
/// 280 mm goings, one run of 16 treads, 4480 mm long. A railing on its right side, 50 mm in,
/// follows the pitch line from the first nosing (180 mm up) to the last (3060 mm): by hand
/// sqrt(4480^2 + 2880^2) = 320 sqrt(277) = 5325.8614 mm, 34,085,513.17 steps, rounded once to
/// 34,085,513. Its balusters, centred at 110 mm: ceil(4480 / 110) - 1 = 40, the first 95 mm
/// along, its foot 50 mm above the pitch line there (241.0714 mm up: 1,862,857 steps), its top
/// under the rail, 1000 mm less the rail's 50 mm depth stretched by sqrt(277) / 14 (7,562,438
/// steps). The first post stands on the floor and reaches the rail: 1180 mm.
#[test]
fn a_railing_on_the_stair_of_17_risers_follows_the_pitch() {
    let s = stair(&[None], &[]);
    let laid_out = laid_stair(&s, 3060);
    assert_eq!(laid_out.riser_count(), 17);
    let host = StairHost {
        stair: &s,
        layout: &laid_out,
        shift: mm(0),
    };
    let railing = on_stair(RailingType::standard(), Side::Right, 50);
    let layout = laid(&railing, Some(host));
    assert_eq!(layout.lines.len(), 1);
    let line = &layout.lines[0];
    assert_eq!(
        line.line.vertices,
        vec![
            Vertex {
                at: [0, 50 * MM],
                low: 0,
                high: 180 * MM
            },
            Vertex {
                at: [4480 * MM, 50 * MM],
                low: 3060 * MM,
                high: 3060 * MM
            },
        ]
    );
    assert_eq!(line.line.inner, Hand::Left);
    assert_eq!(steps(layout.rail_length), 34_085_513);
    assert_eq!(layout.balusters, 40);
    let first = &line.segments[0].balusters[0];
    assert_eq!(first.centre, [95 * MM, 50 * MM]);
    assert_eq!((first.foot, first.top), (1_862_857, 7_562_438));
    assert_eq!(layout.posts, 2);
    let flight = line.segments[0].flight.unwrap();
    assert_eq!((flight.riser, flight.going), (mm(180), mm(280)));
}

/// A U-shaped stair (main private, 3060 mm: runs of 9 and 8 risers, 2240 and 1960 mm, a 1000 mm
/// landing turning back on the left) with a railing on both sides, 50 mm in. The right side goes
/// round the landing's three outer edges; the left side, on the inside, runs on to the end of the
/// first run, round the end of the well 50 mm past it, and back to the foot of the second run,
/// where it steps up one riser (1620 to 1800 mm).
#[test]
fn a_railing_follows_a_stair_round_its_landing() {
    let s = stair(&[None, None], &[(1000, Turn::BackLeft)]);
    let laid_out = laid_stair(&s, 3060);
    let host = StairHost {
        stair: &s,
        layout: &laid_out,
        shift: mm(0),
    };
    let layout = laid(
        &on_stair(RailingType::standard(), Side::Both, 50),
        Some(host),
    );
    let plan = |l: usize| -> Vec<[i64; 2]> {
        layout.lines[l]
            .line
            .vertices
            .iter()
            .map(|v| [v.at[0] / MM, v.at[1] / MM])
            .collect()
    };
    assert_eq!(
        plan(0),
        vec![
            [0, 50],
            [2240, 50],
            [3190, 50],
            [3190, 1950],
            [2240, 1950],
            [280, 1950]
        ]
    );
    assert_eq!(
        plan(1),
        vec![
            [0, 950],
            [2240, 950],
            [2290, 950],
            [2290, 1050],
            [2240, 1050],
            [280, 1050]
        ]
    );
    let left = &layout.lines[1].line.vertices;
    assert_eq!((left[4].low, left[4].high), (1620 * MM, 1800 * MM));
    assert_eq!(layout.lines[1].line.inner, Hand::Right);
    // Only the two runs are flights.
    let flights: Vec<bool> = layout.lines[1]
        .segments
        .iter()
        .map(|s| s.flight.is_some())
        .collect();
    assert_eq!(flights, vec![true, false, false, false, true]);
    assert_eq!(layout.posts, 12);
}

/// A quarter turn with a landing as long as the stair is wide: on the inside the line runs on to
/// the first run's end, across the landing's inner corner 50 mm in, and from the foot of the next
/// run, so neither flight's slope is moved onto the landing.
#[test]
fn a_railing_turns_with_a_quarter_landing() {
    let s = stair(&[None, None], &[(1000, Turn::Left)]);
    let laid_out = laid_stair(&s, 3060);
    let host = StairHost {
        stair: &s,
        layout: &laid_out,
        shift: mm(100),
    };
    let layout = laid(
        &on_stair(RailingType::standard(), Side::Left, 50),
        Some(host),
    );
    let vertices = &layout.lines[0].line.vertices;
    let plan: Vec<[i64; 2]> = vertices
        .iter()
        .map(|v| [v.at[0] / MM, v.at[1] / MM])
        .collect();
    assert_eq!(
        plan,
        vec![
            [0, 950],
            [2240, 950],
            [2290, 950],
            [2290, 1000],
            [2290, 2960]
        ]
    );
    // Every height is raised by the stair's base above the railing's, 100 mm.
    assert_eq!((vertices[0].low, vertices[0].high), (100 * MM, 280 * MM));
    assert_eq!((vertices[3].low, vertices[3].high), (1720 * MM, 1900 * MM));
}

/// A handrail stands `offset` in from the line on its inner side, at the mitre of its pieces:
/// along an L of two 2 m pieces, 60 mm in on the left, its corner is 60 mm in from each.
#[test]
fn a_handrail_stands_in_from_the_line() {
    let mut kind = RailingType::standard();
    kind.handrail = Some(construct_railings::Handrail {
        profile: construct_railings::Profile::Round { diameter: mm(40) },
        height: mm(900),
        offset: mm(60),
    });
    let railing = sketched(kind, vec![at(0, 0), at(2000, 0), at(2000, 2000)]);
    let layout = laid(&railing, None);
    let points = layout.lines[0].handrail.as_ref().unwrap();
    assert_eq!(
        points.clone(),
        vec![[0, 60 * MM], [1940 * MM, 60 * MM], [1940 * MM, 2000 * MM]]
    );
    assert_eq!(layout.handrail_length, Some(mm(1940 + 1940)));
    // Flipped, it stands on the right.
    let flipped = railing.flipped().unwrap();
    let points = laid(&flipped, None).lines[0].handrail.clone().unwrap();
    assert_eq!(points[1], [2060 * MM, -60 * MM]);
}

/// Balusters with no height left are refused: a foot higher than the rail's underside.
#[test]
fn balusters_with_no_height_are_refused() {
    let mut kind = RailingType::standard();
    kind.balusters.bottom = mm(960);
    let railing = sketched(kind, vec![at(0, 0), at(1000, 0)]);
    let refused =
        construct_railings::lay_out(railing.kind(), railing.lines(None).unwrap()).unwrap_err();
    assert_eq!(
        refused,
        RailingError::NoBalusterHeight {
            line: 1,
            segment: 1
        }
    );
}
