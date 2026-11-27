//! Wall joins in plan, L and T (job 5.1, note CON-A4-F2-P2): each layer's outline at an L and at a
//! T, for a three-layer and a five-layer type, both sides of each joint and both orders of
//! drawing, with exact corner points; every joint's pieces tile the joined shape (no two overlap,
//! and their areas add up to the shape's, exactly); and each case the rule does not cover is left
//! unjoined with its reason.

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_walls::joins::join;
use construct_walls::{Function, Joined, PlanLayer, PlanWall, lines_meet};

use Function::{Finish, Membrane, Structure, Substrate, Thermal};

/// Lining, stud and lining: 20, 100 and 20 mm.
const THREE: [(Function, i64); 3] = [(Finish, 20), (Structure, 100), (Finish, 20)];

/// Cladding, a cavity, sheathing, the frame and a lining: 20, 50, 10, 140 and 15 mm (235 mm, so
/// its centreline is at 117.5 mm from either face).
const FIVE: [(Function, i64); 5] = [
    (Finish, 20),
    (Thermal, 50),
    (Substrate, 10),
    (Structure, 140),
    (Finish, 15),
];

/// Insulation between two finishes, with no structure.
const SOFT: [(Function, i64); 3] = [(Finish, 20), (Thermal, 100), (Finish, 20)];

fn steps(mm: i64) -> i64 {
    mm * 6400
}

/// A wall from `start` to `end` (millimetres), its layers centred on its line, 2700 mm high.
fn wall(id: &str, start: [i64; 2], end: [i64; 2], layers: &[(Function, i64)]) -> PlanWall {
    let total: i64 = layers.iter().map(|l| steps(l.1)).sum();
    let mut face = total / 2;
    let mut listed = Vec::new();
    for &(function, thick) in layers {
        listed.push(PlanLayer {
            function,
            exterior: Length::from_steps(face),
            interior: Length::from_steps(face - steps(thick)),
            wraps: false,
        });
        face -= steps(thick);
    }
    let at = |[x, y]: [i64; 2]| [Length::from_steps(steps(x)), Length::from_steps(steps(y))];
    // The core: the strongest layers, as each type here has them together.
    let best = layers.iter().filter_map(|l| l.0.priority()).min().unwrap();
    let first = layers
        .iter()
        .position(|l| l.0.priority() == Some(best))
        .unwrap();
    let last = layers
        .iter()
        .rposition(|l| l.0.priority() == Some(best))
        .unwrap();
    PlanWall::new(
        id.to_owned(),
        (at(start), at(end)),
        (Length::from_mm(0), Length::from_mm(2700)),
        listed,
        [first, last + 1],
    )
}

/// An outline in millimetres (each corner is a whole number of them in these cases).
fn mm(outline: &[[Length; 2]]) -> Vec<[i64; 2]> {
    outline
        .iter()
        .map(|[x, y]| {
            assert_eq!(x.steps() % 6400, 0);
            assert_eq!(y.steps() % 6400, 0);
            [x.steps() / 6400, y.steps() / 6400]
        })
        .collect()
}

/// Every layer's pieces of a joined wall, in millimetres.
fn pieces(joined: &Joined) -> Vec<Vec<Vec<[i64; 2]>>> {
    joined
        .layers
        .iter()
        .map(|layer| layer.iter().map(|p| mm(p)).collect())
        .collect()
}

/// Wall `me` of `walls`, joined.
fn joined(walls: &[PlanWall], me: &str) -> Joined {
    let at = walls.iter().position(|w| w.id == me).unwrap();
    join(walls, at)
}

/// Each edge of a polygon, from a corner to the next.
fn edges(corners: &[[i64; 2]]) -> impl Iterator<Item = ([i64; 2], [i64; 2])> + '_ {
    corners
        .iter()
        .copied()
        .zip(corners.iter().copied().cycle().skip(1))
}

/// Which way a path turns at `to`: positive to the left.
fn turn(from: [i64; 2], to: [i64; 2], after: [i64; 2]) -> i128 {
    i128::from(to[0] - from[0]) * i128::from(after[1] - to[1])
        - i128::from(to[1] - from[1]) * i128::from(after[0] - to[0])
}

fn twice_area(corners: &[[i64; 2]]) -> i128 {
    edges(corners)
        .map(|(from, to)| {
            i128::from(from[0]) * i128::from(to[1]) - i128::from(from[1]) * i128::from(to[0])
        })
        .sum()
}

/// Whether some edge of `one` (counter-clockwise) has all of `other` on or outside it.
fn separates(one: &[[i64; 2]], other: &[[i64; 2]]) -> bool {
    edges(one).any(|(from, to)| other.iter().all(|&corner| turn(from, to, corner) <= 0))
}

/// The area of rectangles, each `[x0, x1, y0, y1]` in tenths of a millimetre, in square steps.
fn area(rects: &[[i64; 4]]) -> i128 {
    rects
        .iter()
        .map(|[x0, x1, y0, y1]| i128::from((x1 - x0) * 640) * i128::from((y1 - y0) * 640))
        .sum()
}

/// Every piece of every wall is strictly convex and counter-clockwise, no two pieces overlap, and
/// their areas add up to `area` square steps, exactly: the pieces tile a shape of that area with
/// no gap.
fn tiles(walls: &[&Joined], area: i128) {
    let all: Vec<Vec<[i64; 2]>> = walls
        .iter()
        .flat_map(|w| w.layers.iter().flatten())
        .map(|p| p.iter().map(|[x, y]| [x.steps(), y.steps()]).collect())
        .collect();
    for piece in &all {
        let next: Vec<_> = edges(piece).map(|(_, to)| to).collect();
        for ((from, to), after) in edges(piece).zip(next.iter().cycle().skip(1)) {
            assert!(
                turn(from, to, *after) > 0,
                "{piece:?} turns left at every corner"
            );
        }
    }
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert!(
                separates(a, b) || separates(b, a),
                "{a:?} and {b:?} overlap"
            );
        }
    }
    let sum: i128 = all.iter().map(|p| twice_area(p)).sum();
    assert_eq!(
        sum,
        2 * area,
        "the pieces' areas add up to the joined shape's"
    );
}

/// An L of two three-layer walls along x and y: every layer mitres, each corner exactly where two
/// faces cross; the outer corner at (70, -70). Both walls see the same joint, whichever order
/// they are given in, and their pieces tile the L.
#[test]
fn an_l_of_two_three_layer_walls_mitres_every_layer() {
    let a = wall("A", [-3000, 0], [0, 0], &THREE);
    let b = wall("B", [0, 0], [0, 3000], &THREE);
    let ab = [a.clone(), b.clone()];
    let ja = joined(&ab, "A");
    assert_eq!(ja.with, ["B"]);
    assert!(ja.unjoined.is_empty(), "{:?}", ja.unjoined);
    assert_eq!(
        pieces(&ja),
        vec![
            vec![vec![[-3000, -70], [70, -70], [50, -50], [-3000, -50]]],
            vec![vec![[-3000, -50], [50, -50], [-50, 50], [-3000, 50]]],
            vec![vec![[-3000, 50], [-50, 50], [-70, 70], [-3000, 70]]],
        ]
    );
    let jb = joined(&ab, "B");
    assert_eq!(
        pieces(&jb),
        vec![
            vec![vec![[70, -70], [70, 3000], [50, 3000], [50, -50]]],
            vec![vec![[50, -50], [50, 3000], [-50, 3000], [-50, 50]]],
            vec![vec![[-50, 50], [-50, 3000], [-70, 3000], [-70, 70]]],
        ]
    );
    assert_eq!(
        mm(&ja.outline),
        vec![[-3000, 70], [-3000, -70], [70, -70], [-70, 70]]
    );
    let ba = [b, a];
    assert_eq!(
        joined(&ba, "A"),
        ja,
        "the order they come in changes nothing"
    );
    assert_eq!(joined(&ba, "B"), jb);
    tiles(
        &[&ja, &jb],
        area(&[[-30_000, 700, -700, 700], [-700, 700, 700, 30_000]]),
    );
}

/// The L turned the other way, so the outer side of the corner is the walls' interior, and drawn
/// the other way round (B first, ending at the corner): the same mitres on the other side.
#[test]
fn an_l_on_the_interior_side_drawn_either_way_mitres_the_same() {
    for (a, b) in [
        (
            wall("A", [-3000, 0], [0, 0], &THREE),
            wall("B", [0, 0], [0, -3000], &THREE),
        ),
        (
            wall("A", [0, 0], [-3000, 0], &THREE),
            wall("B", [0, -3000], [0, 0], &THREE),
        ),
    ] {
        let ab = [a, b];
        let (ja, jb) = (joined(&ab, "A"), joined(&ab, "B"));
        assert_eq!(ja.with, ["B"]);
        assert_eq!(jb.with, ["A"]);
        // The outer corner is (70, 70): A's face at y = 70 runs to x = 70.
        let outer: Vec<[i64; 2]> = pieces(&ja).concat().concat();
        assert!(outer.contains(&[70, 70]), "{outer:?}");
        assert!(outer.contains(&[-50, -50]), "{outer:?}");
        tiles(
            &[&ja, &jb],
            area(&[[-30_000, 700, -700, 700], [-700, 700, -30_000, -700]]),
        );
    }
}

/// A five-layer type at an L, with itself and with the three-layer type, both ways round: the
/// pieces always tile the corner, and the joint is the same from either wall.
#[test]
fn an_l_of_five_layers_tiles_the_corner_with_itself_and_with_three() {
    let five_a = wall("A", [-3000, 0], [0, 0], &FIVE);
    let five_b = wall("B", [0, 0], [0, 3000], &FIVE);
    let three_b = wall("B", [0, 0], [0, 3000], &THREE);
    let same = [five_a.clone(), five_b];
    let (ja, jb) = (joined(&same, "A"), joined(&same, "B"));
    // Every layer mitres: A's exterior cladding runs to the outer corner (117.5, -117.5).
    assert_eq!(ja.layers.len(), 5);
    let cladding = &ja.layers[0][0];
    assert!(cladding.contains(&[Length::from_steps(752_000), Length::from_steps(-752_000)]));
    // The band is 235 mm, the L's arms 3117.5 and 2882.5 mm.
    tiles(
        &[&ja, &jb],
        area(&[[-30_000, 1175, -1175, 1175], [-1175, 1175, 1175, 30_000]]),
    );
    for walls in [
        [five_a.clone(), three_b.clone()],
        [three_b.clone(), five_a.clone()],
    ] {
        let (ja, jb) = (joined(&walls, "A"), joined(&walls, "B"));
        assert_eq!(ja.with, ["B"]);
        assert_eq!(jb.with, ["A"]);
        // A's band, y in [-117.5, 117.5], to B's outer face; B's, x in [-70, 70], above it.
        tiles(
            &[&ja, &jb],
            area(&[[-30_000, 700, -1175, 1175], [-700, 700, 1175, 30_000]]),
        );
    }
}

/// A T of three-layer walls: the abutting wall's finishes stop at the through wall's near finish
/// face, its stud at the through wall's stud, and the through wall's near lining is cut back across
/// the stud's width, in two pieces. From either side of the through wall, and with the abutting
/// wall drawn toward it or away from it.
#[test]
fn a_t_of_three_layer_walls_cuts_the_near_lining_back_for_the_stud() {
    let c = wall("C", [-3000, 0], [3000, 0], &THREE);
    let d = wall("D", [0, -3000], [0, 0], &THREE);
    let cd = [c.clone(), d.clone()];
    let (jc, jd) = (joined(&cd, "C"), joined(&cd, "D"));
    assert_eq!(jc.with, ["D"]);
    assert_eq!(jd.with, ["C"]);
    // C's exterior lining is at y in [-70, -50], on D's side.
    assert_eq!(
        pieces(&jc),
        vec![
            vec![
                vec![[-3000, -70], [-50, -70], [-50, -50], [-3000, -50]],
                vec![[50, -70], [3000, -70], [3000, -50], [50, -50]]
            ],
            vec![vec![[-3000, -50], [3000, -50], [3000, 50], [-3000, 50]]],
            vec![vec![[-3000, 50], [3000, 50], [3000, 70], [-3000, 70]]],
        ]
    );
    assert_eq!(
        pieces(&jd),
        vec![
            vec![vec![[70, -3000], [70, -70], [50, -70], [50, -3000]]],
            vec![vec![[50, -3000], [50, -50], [-50, -50], [-50, -3000]]],
            vec![vec![[-50, -3000], [-50, -70], [-70, -70], [-70, -3000]]],
        ]
    );
    assert_eq!(
        mm(&jc.outline),
        vec![
            [-3000, 70],
            [-3000, -70],
            [-50, -70],
            [-50, -50],
            [50, -50],
            [50, -70],
            [3000, -70],
            [3000, 70]
        ]
    );
    tiles(
        &[&jc, &jd],
        area(&[[-30_000, 30_000, -700, 700], [-700, 700, -30_000, -700]]),
    );
    let dc = [d, c.clone()];
    assert_eq!(
        joined(&dc, "C"),
        jc,
        "the order they come in changes nothing"
    );
    // From the other side, drawn away from C: C's interior lining is cut back.
    let up = wall("D", [0, 0], [0, 3000], &THREE);
    let cu = [c, up];
    let (jc, ju) = (joined(&cu, "C"), joined(&cu, "D"));
    assert_eq!(jc.layers[0].len(), 1);
    assert_eq!(jc.layers[2].len(), 2);
    assert_eq!(
        mm(&jc.layers[2][0]),
        [[-3000, 50], [-50, 50], [-50, 70], [-3000, 70]]
    );
    tiles(
        &[&jc, &ju],
        area(&[[-30_000, 30_000, -700, 700], [-700, 700, 700, 30_000]]),
    );
}

/// A five-layer wall abutting a five-layer wall, and a three-layer one abutting it: each layer
/// stops at the first through layer as strong or stronger, and the pieces tile the T.
#[test]
fn a_t_of_five_layers_stops_each_layer_at_the_first_as_strong() {
    let c = wall("C", [-3000, 0], [3000, 0], &FIVE);
    let d = wall("D", [0, -3000], [0, 0], &FIVE);
    let cd = [c.clone(), d];
    let (jc, jd) = (joined(&cd, "C"), joined(&cd, "D"));
    // C's near side is its exterior (y = -117.5): cladding, cavity, sheathing, frame from there.
    // D's frame passes the cladding, cavity and sheathing; its sheathing passes cladding and
    // cavity; its cavity passes the cladding; its cladding and lining stop on C's face.
    assert_eq!(
        jc.layers[0].len(),
        2,
        "cladding cut for D's cavity, sheathing and frame"
    );
    assert_eq!(
        jc.layers[1].len(),
        2,
        "cavity cut for D's sheathing and frame"
    );
    assert_eq!(jc.layers[2].len(), 2, "sheathing cut for D's frame");
    assert_eq!(jc.layers[3].len(), 1);
    assert_eq!(jc.layers[4].len(), 1);
    // C's band, 6000 by 235 mm, and D's below C's face.
    tiles(
        &[&jc, &jd],
        area(&[
            [-30_000, 30_000, -1175, 1175],
            [-1175, 1175, -30_000, -1175],
        ]),
    );
    let three = wall("D", [0, -3000], [0, 0], &THREE);
    let c3 = [c, three];
    let (jc, jd) = (joined(&c3, "C"), joined(&c3, "D"));
    assert_eq!(jc.with, ["D"]);
    assert_eq!(jc.layers[0].len(), 2);
    assert_eq!(jc.layers[2].len(), 2, "the stud reaches C's frame");
    tiles(
        &[&jc, &jd],
        area(&[[-30_000, 30_000, -1175, 1175], [-700, 700, -30_000, -1175]]),
    );
}

/// A membrane has no priority: it never cuts, and stops at the first layer it meets.
#[test]
fn a_membrane_stops_at_the_first_layer_it_meets() {
    let wrapped: [(Function, i64); 4] =
        [(Finish, 20), (Membrane, 0), (Structure, 100), (Finish, 20)];
    let c = wall("C", [-3000, 0], [3000, 0], &wrapped);
    let d = wall("D", [0, -3000], [0, 0], &wrapped);
    let cd = [c, d];
    let (jc, jd) = (joined(&cd, "C"), joined(&cd, "D"));
    assert!(
        jc.layers[1].is_empty() && jd.layers[1].is_empty(),
        "no solid for a membrane"
    );
    tiles(
        &[&jc, &jd],
        area(&[[-30_000, 30_000, -700, 700], [-700, 700, -30_000, -700]]),
    );
}

/// What the rule does not cover is left unjoined, built as before, with the reason in words.
#[test]
fn what_the_rule_does_not_cover_is_left_unjoined_with_its_reason() {
    let a = wall("A", [-3000, 0], [0, 0], &THREE);
    let cases: Vec<(&str, Vec<PlanWall>, &str)> = vec![
        (
            "three walls at a corner",
            vec![
                a.clone(),
                wall("B", [0, 0], [0, 3000], &THREE),
                wall("E", [0, 0], [0, -3000], &THREE),
            ],
            "more than two walls",
        ),
        (
            "two walls in line",
            vec![a.clone(), wall("B", [0, 0], [3000, 0], &THREE)],
            "in line",
        ),
        (
            "a cross too near an end",
            vec![
                wall("A", [-3000, 0], [3000, 0], &THREE),
                wall("B", [2950, -3000], [2950, 3000], &THREE),
            ],
            "too near an end",
        ),
        (
            "a T too near the end",
            vec![
                wall("A", [-3000, 0], [3000, 0], &THREE),
                wall("B", [2950, -3000], [2950, 0], &THREE),
            ],
            "too near an end",
        ),
        (
            "a T whose structure would cut the through wall in two",
            vec![
                wall("A", [-3000, 0], [3000, 0], &SOFT),
                wall("B", [0, -3000], [0, 0], &THREE),
            ],
            "would cut it in two",
        ),
        (
            "a wall too short for its two joints",
            vec![
                wall("A", [-3000, 0], [0, 0], &THREE),
                wall("B", [0, 0], [0, 100], &THREE),
                wall("E", [0, 100], [3000, 100], &THREE),
            ],
            "too short",
        ),
    ];
    for (case, walls, words) in cases {
        let ja = joined(&walls, "A");
        assert!(ja.layers.is_empty() && ja.with.is_empty(), "{case}: {ja:?}");
        assert!(
            ja.unjoined.iter().any(|why| why.contains(words)),
            "{case}: {:?}",
            ja.unjoined
        );
        let jb = joined(&walls, "B");
        assert!(
            jb.with.is_empty() || case.contains("short"),
            "{case}: {jb:?}"
        );
    }
}

/// Walls that do not meet, or meet in plan on different heights, are not joined and say nothing.
#[test]
fn walls_that_do_not_meet_say_nothing() {
    let a = wall("A", [-3000, 0], [0, 0], &THREE);
    let apart = wall("B", [1, 0], [1, 3000], &THREE);
    let mut above = wall("B", [0, 0], [0, 3000], &THREE);
    above.base = Length::from_mm(2700);
    above.top = Length::from_mm(5400);
    for b in [apart, above] {
        let ja = joined(&[a.clone(), b], "A");
        assert_eq!(ja, Joined::default());
    }
    // Within the tolerance (0.01 mm, 64 steps) of each other the ends meet; one step more, not.
    let ln = |x0: i64, x1: i64, y: i64| {
        [
            [Length::from_steps(x0), Length::from_steps(y)],
            [Length::from_steps(x1), Length::from_steps(y)],
        ]
    };
    let vertical = [
        [Length::from_steps(64), Length::from_steps(0)],
        [Length::from_steps(64), Length::from_steps(1000)],
    ];
    assert!(lines_meet(ln(-1000, 0, 0), vertical));
    let further = [
        [Length::from_steps(65), Length::from_steps(0)],
        [Length::from_steps(65), Length::from_steps(1000)],
    ];
    assert!(!lines_meet(ln(-1000, 0, 0), further));
}
