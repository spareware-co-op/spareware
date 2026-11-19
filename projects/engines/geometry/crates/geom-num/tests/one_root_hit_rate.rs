//! How often the one-root filter decides on a tangency-heavy corpus (piece G189; chapter 31's risk
//! R3), and that it is never wrong there: the figure chapter 14's as-built note reports, native and
//! on wasm32. Run with `--nocapture` to read it.
//!
//! The corpus is what fillets and offsets leave: circles, each with lines at exact tangency and a
//! step either side of it, whose crossings lie a hair apart, and the same circles' crossings with
//! lines across them, which are far apart. Comparing them is the arrangement's work (14.4), and the
//! near pairs are where a filter fails.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_num::arc::Circle2;
use geom_num::carrier::Carrier;
use geom_num::grid::P2;
use geom_num::one_root::{Crossing2, Root};

/// Pythagorean directions: each (a, b) with a² + b² a square, so a line can be exactly tangent.
const TRIPLES: [(i128, i128, i128); 6] = [
    (3, 4, 5),
    (5, 12, 13),
    (8, 15, 17),
    (7, 24, 25),
    (20, 21, 29),
    (12, 35, 37),
];

fn crossings(l: &Carrier, circle: &Circle2) -> Vec<Crossing2> {
    [Root::First, Root::Second]
        .into_iter()
        .filter_map(|root| Crossing2::of_circle(l, circle, root).ok())
        .collect()
}

#[test]
fn the_filter_decides_most_of_a_tangency_heavy_corpus_and_is_never_wrong() {
    let mut points = Vec::new();
    for (i, x) in (-3_i64..3).enumerate() {
        for (j, y) in (-3_i64..3).enumerate() {
            let centre = P2::new(x * 1_000_003, y * 700_001).unwrap();
            let radius = 5_000_i128 * 5 * 13 * (1 + i as i128 + j as i128);
            let circle = Circle2::new(centre, radius).unwrap();
            let (cx, cy) = (i128::from(centre.x.get()), i128::from(centre.y.get()));
            for (a, b, h) in TRIPLES {
                // a x + b y = a cx + b cy ± h r touches the circle; a step in c either side does not,
                // or crosses it a hair apart.
                let touch = a * cx + b * cy + h * radius;
                for by in [-1, 0, 1, 7, -40] {
                    let l = Carrier::new(a, b, touch - by).unwrap();
                    points.extend(crossings(&l, &circle));
                }
                let across = Carrier::new(b, -a, b * cx - a * cy + 1).unwrap();
                points.extend(crossings(&across, &circle));
            }
        }
    }
    let (mut decided, mut total, mut exact) = (0_u64, 0_u64, 0_u64);
    for (i, p) in points.iter().enumerate() {
        for q in points.iter().skip(i + 1).step_by(37) {
            total += 1;
            if let Some(sign) = p.cmp_x_filter(q) {
                decided += 1;
                assert_eq!(p.cmp_x(q).unwrap(), sign);
            } else {
                exact += 1;
                let answer = p.cmp_x(q).unwrap();
                assert_eq!(answer, q.cmp_x(p).unwrap().negate());
            }
        }
    }
    let share = decided * 1_000 / total;
    println!(
        "tangency-heavy corpus: {} points, {total} comparisons, the filter decided {decided} \
         ({}.{}%), the exact path {exact}",
        points.len(),
        share / 10,
        share % 10
    );
    assert!(decided > 0 && exact > 0);
}
