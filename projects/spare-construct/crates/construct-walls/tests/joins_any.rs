//! Wall joins of job 6.1 (notes CON-A4-F2-P2-wall-joins and CON-A4-F2-P4-wall-wrapping): crosses,
//! a person's choices (butt with its order, mitre, square-off, joins allowed or not at an end),
//! joins at any angle, and wrapped free ends. Each kind on the plan's axes with exact corners and
//! exact tiling, the same configuration turned on the whole ratio 3-4-5 (every corner the turned
//! corner, exactly), and at an angle off every whole ratio (each corner within half a step of the
//! true crossing, every piece strictly convex, the areas adding up to within the rounding).

#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "test code; the rules are for the crate's own code"
)]

use construct_params::Length;
use construct_walls::joins::join;
use construct_walls::{
    AtEnds, Choice, Function, JoinKind, Joined, Meeting, PlanLayer, PlanWall, meeting,
};

use Function::{Finish, Structure, Substrate, Thermal};

/// Lining, stud and lining: 20, 100 and 20 mm; the linings wrap.
const THREE: [(Function, i64, bool); 3] = [
    (Finish, 20, true),
    (Structure, 100, false),
    (Finish, 20, true),
];

/// Cladding (wraps), a cavity, sheathing, the frame and a lining (wraps): 20, 50, 10, 140, 15.
const FIVE: [(Function, i64, bool); 5] = [
    (Finish, 20, true),
    (Thermal, 50, false),
    (Substrate, 10, false),
    (Structure, 140, false),
    (Finish, 15, true),
];

/// Insulation between two finishes, with no structure.
const SOFT: [(Function, i64, bool); 3] = [
    (Finish, 20, false),
    (Thermal, 100, false),
    (Finish, 20, false),
];

fn steps(mm: i64) -> i64 {
    mm * 6400
}

/// A wall from `start` to `end` in steps, its layers centred on its line, its core its strongest
/// layers.
fn wall_steps(
    id: &str,
    start: [i64; 2],
    end: [i64; 2],
    layers: &[(Function, i64, bool)],
) -> PlanWall {
    let total: i64 = layers.iter().map(|l| steps(l.1)).sum();
    let mut face = total / 2;
    let mut listed = Vec::new();
    for &(function, thick, wraps) in layers {
        listed.push(PlanLayer {
            function,
            exterior: Length::from_steps(face),
            interior: Length::from_steps(face - steps(thick)),
            wraps,
        });
        face -= steps(thick);
    }
    let best = layers.iter().filter_map(|l| l.0.priority()).min().unwrap();
    let first = layers
        .iter()
        .position(|l| l.0.priority() == Some(best))
        .unwrap();
    let last = layers
        .iter()
        .rposition(|l| l.0.priority() == Some(best))
        .unwrap();
    let at = |[x, y]: [i64; 2]| [Length::from_steps(x), Length::from_steps(y)];
    PlanWall::new(
        id.to_owned(),
        (at(start), at(end)),
        (Length::from_mm(0), Length::from_mm(2700)),
        listed,
        [first, last + 1],
    )
}

/// A wall from `start` to `end` in millimetres.
fn wall(id: &str, start: [i64; 2], end: [i64; 2], layers: &[(Function, i64, bool)]) -> PlanWall {
    wall_steps(id, start.map(steps), end.map(steps), layers)
}

/// The same configuration turned by the whole ratio 3-4-5 about the origin: exact in steps, as
/// every coordinate here is a whole number of millimetres, 6,400 steps, a multiple of 5.
fn turn([x, y]: [i64; 2]) -> [i64; 2] {
    assert_eq!(x % 5, 0);
    assert_eq!(y % 5, 0);
    [(3 * x - 4 * y) / 5, (4 * x + 3 * y) / 5]
}

fn turned(w: &PlanWall) -> PlanWall {
    let mut out = w.clone();
    let t = |p: [Length; 2]| turn([p[0].steps(), p[1].steps()]).map(Length::from_steps);
    out.start = t(w.start);
    out.end = t(w.end);
    out
}

fn joined(walls: &[PlanWall], me: &str) -> Joined {
    let at = walls.iter().position(|w| w.id == me).unwrap();
    join(walls, at)
}

fn steps_of(outline: &[[Length; 2]]) -> Vec<[i64; 2]> {
    outline
        .iter()
        .map(|[x, y]| [x.steps(), y.steps()])
        .collect()
}

fn mm(outline: &[[Length; 2]]) -> Vec<[i64; 2]> {
    steps_of(outline)
        .into_iter()
        .map(|[x, y]| {
            assert_eq!(x % 6400, 0, "{outline:?}");
            assert_eq!(y % 6400, 0, "{outline:?}");
            [x / 6400, y / 6400]
        })
        .collect()
}

fn pieces_mm(j: &Joined) -> Vec<Vec<Vec<[i64; 2]>>> {
    j.layers
        .iter()
        .map(|layer| layer.iter().map(|p| mm(p)).collect())
        .collect()
}

fn edges(corners: &[[i64; 2]]) -> impl Iterator<Item = ([i64; 2], [i64; 2])> + '_ {
    corners
        .iter()
        .copied()
        .zip(corners.iter().copied().cycle().skip(1))
}

fn turn_at(from: [i64; 2], to: [i64; 2], after: [i64; 2]) -> i128 {
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

fn separates(one: &[[i64; 2]], other: &[[i64; 2]]) -> bool {
    edges(one).any(|(from, to)| other.iter().all(|&corner| turn_at(from, to, corner) <= 0))
}

fn all_pieces(walls: &[&Joined]) -> Vec<Vec<[i64; 2]>> {
    walls
        .iter()
        .flat_map(|w| w.layers.iter().flatten())
        .map(|p| steps_of(p))
        .collect()
}

/// Every piece is strictly convex and counter-clockwise.
fn convex(walls: &[&Joined]) {
    for piece in all_pieces(walls) {
        let next: Vec<_> = edges(&piece).map(|(_, to)| to).collect();
        for ((from, to), after) in edges(&piece).zip(next.iter().cycle().skip(1)) {
            assert!(
                turn_at(from, to, *after) > 0,
                "{piece:?} turns left at every corner"
            );
        }
    }
}

/// Every piece is strictly convex, no two overlap, and their areas add up to `area` square steps
/// exactly.
fn tiles(walls: &[&Joined], area: i128) {
    convex(walls);
    let all = all_pieces(walls);
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

/// The area of rectangles `[x0, x1, y0, y1]` in tenths of a millimetre, in square steps.
fn area(rects: &[[i64; 4]]) -> i128 {
    rects
        .iter()
        .map(|[x0, x1, y0, y1]| i128::from((x1 - x0) * 640) * i128::from((y1 - y0) * 640))
        .sum()
}

fn choose(walls: &mut [PlanWall], keeper: &str, other: &str, kind: JoinKind, first: Option<&str>) {
    let w = walls.iter_mut().find(|w| w.id == keeper).unwrap();
    w.joining.with.insert(
        other.to_owned(),
        Choice {
            kind,
            first: first.map(str::to_owned),
        },
    );
}

/// The pieces of every wall of `walls`, and of the same walls turned 3-4-5, are the same corners
/// turned, exactly; both tile.
fn same_turned(walls: &[PlanWall], area: i128) {
    let turned_walls: Vec<PlanWall> = walls.iter().map(turned).collect();
    let flat: Vec<Joined> = walls.iter().map(|w| joined(walls, &w.id)).collect();
    let round: Vec<Joined> = turned_walls
        .iter()
        .map(|w| joined(&turned_walls, &w.id))
        .collect();
    for (a, b) in flat.iter().zip(&round) {
        assert_eq!(a.with, b.with);
        assert_eq!(a.layers.len(), b.layers.len());
        for (la, lb) in a.layers.iter().zip(&b.layers) {
            let la: Vec<Vec<[i64; 2]>> = la
                .iter()
                .map(|p| steps_of(p).into_iter().map(turn).collect())
                .collect();
            let lb: Vec<Vec<[i64; 2]>> = lb.iter().map(|p| steps_of(p)).collect();
            assert_eq!(la, lb, "a layer's pieces, turned");
        }
        let oa: Vec<[i64; 2]> = steps_of(&a.outline).into_iter().map(turn).collect();
        assert_eq!(oa, steps_of(&b.outline), "the outline, turned");
    }
    tiles(&flat.iter().collect::<Vec<_>>(), area);
    tiles(&round.iter().collect::<Vec<_>>(), area);
}

/// A cross of two walls of one type: the wall of the lower ID runs through, the other is cut in
/// two, its stud stopping on the through wall's stud from both sides and its linings on the
/// through wall's faces; the through wall's linings are cut back on both sides for the stud.
/// Choosing the other first switches them; turned 3-4-5 it is the same, exactly.
#[test]
fn a_cross_runs_one_wall_through_and_cuts_the_other_in_two() {
    let walls = vec![
        wall("A", [-3000, 0], [3000, 0], &THREE),
        wall("B", [0, -3000], [0, 3000], &THREE),
    ];
    let (ja, jb) = (joined(&walls, "A"), joined(&walls, "B"));
    assert_eq!(ja.with, ["B"]);
    assert_eq!(jb.with, ["A"]);
    assert!(ja.unjoined.is_empty() && jb.unjoined.is_empty());
    // A's linings each in two pieces, its stud whole.
    assert_eq!(
        ja.layers.iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 1, 2]
    );
    assert_eq!(
        pieces_mm(&ja)[0],
        vec![
            vec![[-3000, -70], [-50, -70], [-50, -50], [-3000, -50]],
            vec![[50, -70], [3000, -70], [3000, -50], [50, -50]]
        ]
    );
    // B cut in two at every layer: its stud stops on A's stud faces, its linings on A's faces.
    assert_eq!(
        pieces_mm(&jb)[1],
        vec![
            vec![[50, -3000], [50, -50], [-50, -50], [-50, -3000]],
            vec![[50, 50], [50, 3000], [-50, 3000], [-50, 50]]
        ]
    );
    assert_eq!(
        pieces_mm(&jb)[0],
        vec![
            vec![[70, -3000], [70, -70], [50, -70], [50, -3000]],
            vec![[70, 70], [70, 3000], [50, 3000], [50, 70]]
        ]
    );
    // The cut wall keeps its whole outline; the through wall's dips round the stud both sides.
    assert_eq!(mm(&jb.outline).len(), 4);
    assert_eq!(mm(&ja.outline).len(), 12);
    let both = |walls: &[PlanWall]| {
        let (ja, jb) = (joined(walls, "A"), joined(walls, "B"));
        // Each piece once: A's band and B's arms outside it.
        tiles(
            &[&ja, &jb],
            area(&[
                [-30_000, 30_000, -700, 700],
                [-700, 700, -30_000, -700],
                [-700, 700, 700, 30_000],
            ]),
        );
    };
    both(&walls);
    let mut switched = walls.clone();
    choose(&mut switched, "A", "B", JoinKind::Butt, Some("B"));
    let (sa, sb) = (joined(&switched, "A"), joined(&switched, "B"));
    assert_eq!(
        sb.layers.iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 1, 2]
    );
    assert_eq!(
        sa.layers.iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 2, 2]
    );
    both(&switched);
    same_turned(
        &walls,
        area(&[
            [-30_000, 30_000, -700, 700],
            [-700, 700, -30_000, -700],
            [-700, 700, 700, 30_000],
        ]),
    );
}

/// At a cross the stronger wall runs through whatever the choice says: a wall of insulation is
/// cut by a stud wall, never the other way.
#[test]
fn at_a_cross_the_stronger_runs_through() {
    let mut walls = vec![
        wall("A", [-3000, 0], [3000, 0], &SOFT),
        wall("B", [0, -3000], [0, 3000], &THREE),
    ];
    choose(&mut walls, "A", "B", JoinKind::Butt, Some("A"));
    let (ja, jb) = (joined(&walls, "A"), joined(&walls, "B"));
    assert_eq!(jb.layers[1].len(), 1, "B's stud runs through");
    assert!(ja.layers.iter().all(|l| l.len() == 2), "A is cut in two");
    tiles(
        &[&ja, &jb],
        area(&[
            [-30_000, -700, -700, 700],
            [700, 30_000, -700, 700],
            [-700, 700, -30_000, 30_000],
        ]),
    );
}

/// A butt at a corner: the first wall's finish and stud run to the other's outer face, the other's
/// finish and stud stop on the first's stud and the linings fit round; choosing the other first
/// mirrors it. A square-off at a right angle is the same as a butt. Both tile, on the axes and
/// turned 3-4-5.
#[test]
fn a_butt_at_a_corner_runs_the_first_wall_through() {
    let walls = vec![
        wall("A", [-3000, 0], [0, 0], &THREE),
        wall("B", [0, 0], [0, 3000], &THREE),
    ];
    let l_area = area(&[[-30_000, 700, -700, 700], [-700, 700, 700, 30_000]]);
    let mut butt = walls.clone();
    choose(&mut butt, "A", "B", JoinKind::Butt, Some("A"));
    let (ja, jb) = (joined(&butt, "A"), joined(&butt, "B"));
    assert_eq!(
        pieces_mm(&ja),
        vec![
            vec![vec![[-3000, -70], [70, -70], [70, -50], [-3000, -50]]],
            vec![vec![[-3000, -50], [70, -50], [70, 50], [-3000, 50]]],
            vec![vec![[-3000, 50], [-50, 50], [-50, 70], [-3000, 70]]],
        ]
    );
    assert_eq!(
        pieces_mm(&jb)[1],
        vec![vec![[50, 50], [50, 3000], [-50, 3000], [-50, 50]]]
    );
    tiles(&[&ja, &jb], l_area);
    same_turned(&butt, l_area);
    let mut square = walls.clone();
    choose(&mut square, "A", "B", JoinKind::SquareOff, Some("A"));
    assert_eq!(
        joined(&square, "A"),
        ja,
        "at a right angle a square-off is a butt"
    );
    assert_eq!(joined(&square, "B"), jb);
    let mut other = walls.clone();
    choose(&mut other, "A", "B", JoinKind::Butt, Some("B"));
    let (oa, ob) = (joined(&other, "A"), joined(&other, "B"));
    assert_eq!(
        pieces_mm(&ob)[1],
        vec![vec![[50, -70], [50, 3000], [-50, 3000], [-50, -70]]],
        "B's stud runs to A's outer face"
    );
    tiles(&[&oa, &ob], l_area);
    // A mitre, stored or not, is the rule's own.
    let mut mitre = walls.clone();
    choose(&mut mitre, "A", "B", JoinKind::Mitre, None);
    assert_eq!(joined(&mitre, "A"), joined(&walls, "A"));
}

/// The L, the T and the five-layer L, turned 3-4-5: every corner is the turned corner, exactly.
#[test]
fn joins_turned_on_a_whole_ratio_are_the_same_joins_exactly() {
    same_turned(
        &[
            wall("A", [-3000, 0], [0, 0], &THREE),
            wall("B", [0, 0], [0, 3000], &THREE),
        ],
        area(&[[-30_000, 700, -700, 700], [-700, 700, 700, 30_000]]),
    );
    same_turned(
        &[
            wall("C", [-3000, 0], [3000, 0], &THREE),
            wall("D", [0, -3000], [0, 0], &THREE),
        ],
        area(&[[-30_000, 30_000, -700, 700], [-700, 700, -30_000, -700]]),
    );
    same_turned(
        &[
            wall("A", [-3000, 0], [0, 0], &FIVE),
            wall("B", [0, 0], [0, 3000], &THREE),
        ],
        area(&[[-30_000, 700, -1175, 1175], [-700, 700, 1175, 30_000]]),
    );
}

/// The true crossing of two lines, in floating point: only to check a bound against.
fn true_meet(p: [f64; 2], d: [f64; 2], q: [f64; 2], e: [f64; 2]) -> [f64; 2] {
    let den = d[0] * e[1] - d[1] * e[0];
    let s = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / den;
    [p[0] + s * d[0], p[1] + s * d[1]]
}

/// The faces of a wall as true lines, in floating point: each through its true offset from the
/// rounded start corner the builders use.
fn faces_of(w: &PlanWall) -> Vec<([f64; 2], [f64; 2])> {
    let (s, e) = (
        [w.start[0].steps() as f64, w.start[1].steps() as f64],
        [w.end[0].steps() as f64, w.end[1].steps() as f64],
    );
    let d = [e[0] - s[0], e[1] - s[1]];
    let len = d[0].hypot(d[1]);
    let mut out = vec![];
    let mut offsets = vec![w.layers[0].exterior.steps()];
    offsets.extend(w.layers.iter().map(|l| l.interior.steps()));
    for o in offsets {
        let o = o as f64;
        out.push(([s[0] + o * d[1] / len, s[1] - o * d[0] / len], d));
    }
    out
}

/// Joins at an angle off every whole ratio, on (7, 4) and (-2, 9): an L at about 73 degrees
/// mitred, butted and squared off, and a T at an angle. Every piece is strictly convex, every
/// corner is within a step of where two true faces cross (half a step for the rounding of each
/// face and half for the corner's own), the joint is the same seen from either wall, and the
/// areas add up to the true shape's to within the rounding.
#[test]
fn joins_at_any_angle_round_each_corner_once() {
    let a = wall_steps("A", [-7 * 3_000_000, -4 * 3_000_000], [0, 0], &THREE);
    let b = wall_steps("B", [0, 0], [-2 * 2_000_000, 9 * 2_000_000], &THREE);
    for (kind, first) in [
        (None, None),
        (Some(JoinKind::Butt), Some("A")),
        (Some(JoinKind::Butt), Some("B")),
        (Some(JoinKind::SquareOff), Some("A")),
    ] {
        let mut walls = vec![a.clone(), b.clone()];
        if let Some(kind) = kind {
            choose(&mut walls, "A", "B", kind, first);
        }
        let (ja, jb) = (joined(&walls, "A"), joined(&walls, "B"));
        assert_eq!(ja.with, ["B"], "{kind:?} {first:?}: {:?}", ja.unjoined);
        assert_eq!(jb.with, ["A"]);
        convex(&[&ja, &jb]);
        // Every corner of A's pieces is near some crossing of A's faces with B's, or near A's own
        // start corners, or (square-off) on A's squared end.
        let (fa, fb) = (faces_of(&walls[0]), faces_of(&walls[1]));
        let crossings: Vec<[f64; 2]> = fa
            .iter()
            .flat_map(|&(p, d)| fb.iter().map(move |&(q, e)| true_meet(p, d, q, e)))
            .collect();
        for corner in ja.layers.iter().flatten().flatten() {
            let c = [corner[0].steps() as f64, corner[1].steps() as f64];
            let near_crossing = crossings
                .iter()
                .any(|x| (x[0] - c[0]).abs() <= 2.0 && (x[1] - c[1]).abs() <= 2.0);
            let far_end = c[0] < -1_000_000.0;
            assert!(
                near_crossing || far_end || kind == Some(JoinKind::SquareOff),
                "{kind:?}: {c:?} is no crossing"
            );
        }
        // The same joint from either wall: every corner of A's on the joint is one of B's too,
        // where the two walls' pieces meet.
        let corners_b: Vec<[i64; 2]> = all_pieces(&[&jb]).concat();
        let shared = all_pieces(&[&ja])
            .concat()
            .into_iter()
            .filter(|c| corners_b.contains(c))
            .count();
        assert!(
            shared >= 2,
            "{kind:?}: the two walls share their joint's corners"
        );
    }
    // A square-off's through wall ends its layers that run through (its lining and stud) on one
    // line square to its own.
    let mut walls = vec![a.clone(), b.clone()];
    choose(&mut walls, "A", "B", JoinKind::SquareOff, Some("A"));
    let ja = joined(&walls, "A");
    let d = [7_i128, 4];
    let at = |p: &[i64; 2]| i128::from(p[0]) * d[0] + i128::from(p[1]) * d[1];
    let mut along: Vec<i128> = Vec::new();
    for layer in &ja.layers[..2] {
        let mut corners: Vec<i128> = steps_of(&layer[0]).iter().map(at).collect();
        corners.sort_unstable();
        along.extend(&corners[2..]);
    }
    let spread = along.iter().max().unwrap() - along.iter().min().unwrap();
    // On a square line every corner is the same distance along A, to within the rounding.
    assert!(spread <= 2 * (7 + 4), "{along:?}");
    // A T at an angle: D abuts C from below at about 77 degrees.
    let c = wall("C", [-3000, 0], [3000, 0], &THREE);
    let d = wall_steps("D", [-2 * 2_000_000, -9 * 2_000_000], [0, 0], &THREE);
    let walls = vec![c, d];
    let (jc, jd) = (joined(&walls, "C"), joined(&walls, "D"));
    assert_eq!(jc.with, ["D"], "{:?}", jc.unjoined);
    assert_eq!(
        jc.layers[0].len(),
        2,
        "C's near lining cut back for D's stud"
    );
    convex(&[&jc, &jd]);
}

/// A wall with joins not allowed at an end is not joined there, and says why; the walls it meets
/// there are not shaped for it.
#[test]
fn a_disallowed_end_is_left_square() {
    let mut walls = vec![
        wall("A", [-3000, 0], [0, 0], &THREE),
        wall("B", [0, 0], [0, 3000], &THREE),
    ];
    walls[0].joining.allowed = [true, false];
    let (ja, jb) = (joined(&walls, "A"), joined(&walls, "B"));
    assert!(ja.with.is_empty() && jb.with.is_empty());
    assert!(
        ja.unjoined.iter().any(|w| w.contains("not allowed")),
        "{:?}",
        ja.unjoined
    );
    assert!(
        jb.unjoined.iter().any(|w| w.contains("does not allow")),
        "{:?}",
        jb.unjoined
    );
    let mut tee = vec![
        wall("C", [-3000, 0], [3000, 0], &THREE),
        wall("D", [0, -3000], [0, 0], &THREE),
    ];
    tee[1].joining.allowed = [true, false];
    let jc = joined(&tee, "C");
    assert!(jc.layers.is_empty(), "C is not notched");
    assert!(
        jc.unjoined.iter().any(|w| w.contains("does not allow")),
        "{:?}",
        jc.unjoined
    );
}

/// A free end wrapped on the exterior: the cladding turns across the cavity, sheathing and frame
/// to the frame's inner face; those three stop short by the cladding; the lining runs flush. On
/// the interior the lining turns to the frame's outer face. A joined end does not wrap; turned
/// 3-4-5 it is the same, exactly.
#[test]
fn a_free_end_wraps_its_side_s_wrapping_layers() {
    let mut w = wall("W", [0, 0], [3000, 0], &FIVE);
    w.wrap_ends = AtEnds::Exterior;
    let jw = joined(&[w.clone()], "W");
    assert!(jw.with.is_empty(), "{:?}", jw.with);
    let p = &jw.layers;
    // Faces at y = 117.5 (cladding's outside, the exterior on the right walking +x is y < 0).
    // The cladding: its strip, and at each end its turn across the cavity to the frame's inner
    // face, 20 mm deep along the wall.
    assert_eq!(p[0].len(), 3);
    assert_eq!(p[1].len(), 1);
    assert_eq!(p[4].len(), 1);
    let strip = |k: usize| steps_of(&jw.layers[k][0]);
    let xs = |c: &[[i64; 2]]| {
        let mut x: Vec<i64> = c.iter().map(|q| q[0]).collect();
        x.sort_unstable();
        x.dedup();
        x
    };
    assert_eq!(xs(&strip(0)), [0, steps(3000)]);
    assert_eq!(xs(&strip(1)), [steps(20), steps(2980)]);
    assert_eq!(xs(&strip(3)), [steps(20), steps(2980)]);
    assert_eq!(xs(&strip(4)), [0, steps(3000)]);
    // The outline is the plain strip's.
    assert_eq!(steps_of(&jw.outline).len(), 4);
    // The pieces tile the wall: 3000 by 235 mm.
    tiles(&[&jw], area(&[[0, 30_000, -1175, 1175]]));
    same_turned(&[w.clone()], area(&[[0, 30_000, -1175, 1175]]));
    // On the interior the lining turns, and the frame stops short by it.
    let mut i = w.clone();
    i.wrap_ends = AtEnds::Interior;
    let ji = joined(&[i], "W");
    assert_eq!(ji.layers[4].len(), 3);
    assert_eq!(xs(&steps_of(&ji.layers[3][0])), [steps(15), steps(2985)]);
    assert_eq!(xs(&steps_of(&ji.layers[0][0])), [0, steps(3000)]);
    tiles(&[&ji], area(&[[0, 30_000, -1175, 1175]]));
    // Joined at its start, it wraps only its end.
    let mut a = wall("A", [-3000, 0], [0, 0], &THREE);
    a.wrap_ends = AtEnds::Exterior;
    let b = wall("B", [0, 0], [0, 3000], &THREE);
    let walls = [a, b];
    let ja = joined(&walls, "A");
    assert_eq!(ja.with, ["B"]);
    assert_eq!(ja.layers[0].len(), 2, "the start's turn only");
    let jb = joined(&walls, "B");
    tiles(
        &[&ja, &jb],
        area(&[[-30_000, 700, -700, 700], [-700, 700, 700, 30_000]]),
    );
    // Nothing wraps where no layer on the side wraps.
    let mut soft = wall("S", [0, 0], [3000, 0], &SOFT);
    soft.wrap_ends = AtEnds::Exterior;
    assert_eq!(joined(&[soft], "S"), Joined::default());
}

/// How two walls meet, from their lines: a corner, a T naming the through wall, a cross.
#[test]
fn meeting_says_how_two_walls_meet() {
    let walls = vec![
        wall("A", [-3000, 0], [0, 0], &THREE),
        wall("B", [0, 0], [0, 3000], &THREE),
        wall("C", [-1500, -3000], [-1500, 0], &THREE),
        wall("D", [-2500, -1000], [-2500, 1000], &THREE),
        wall("E", [5000, 5000], [6000, 5000], &THREE),
    ];
    assert_eq!(meeting(&walls, 0, 1), Some(Meeting::Corner));
    assert_eq!(
        meeting(&walls, 0, 2),
        Some(Meeting::Tee {
            through: "A".to_owned()
        })
    );
    assert_eq!(
        meeting(&walls, 2, 0),
        Some(Meeting::Tee {
            through: "A".to_owned()
        })
    );
    assert_eq!(meeting(&walls, 0, 3), Some(Meeting::Cross));
    assert_eq!(meeting(&walls, 0, 4), None);
}
