// SPDX-License-Identifier: Apache-2.0
//! The exact-rank spike (`geom_test::exact_rank`, piece G231): Bareiss elimination held to plain
//! rational elimination, and the rank of generated sketches' Jacobians in exact rationals held to
//! the kernel's pivoted-QR diagnosis and to the families' known diagnoses.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "tests; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use dashu_int::IBig;
use geom_test::exact_rank::{bareiss_rank, chain_cost, chain_matrix, gauss_rank, jacobian};
use geom_test::oracle::families::{cases, kernel};
use geom_test::seed::resolve;
use geom_test::{Seed, SplitMix64};

fn seed() -> Seed {
    resolve("exact_rank", Seed::DEFAULT).unwrap()
}

fn matrix(rng: &mut SplitMix64, rows: usize, columns: usize, rank_target: usize) -> Vec<Vec<IBig>> {
    // Rows made of `rank_target` independent ones and combinations of them.
    let base: Vec<Vec<i64>> = (0..rank_target)
        .map(|_| {
            (0..columns)
                .map(|_| i64::from(u8::try_from(rng.next_u64() % 19).unwrap_or(0)) - 9)
                .collect()
        })
        .collect();
    (0..rows)
        .map(|i| {
            let mut row = vec![0_i64; columns];
            if i < rank_target {
                row.clone_from(&base[i]);
            } else {
                for b in &base {
                    let k = i64::from(u8::try_from(rng.next_u64() % 5).unwrap_or(0)) - 2;
                    for (slot, x) in row.iter_mut().zip(b) {
                        *slot += k * x;
                    }
                }
            }
            row.into_iter().map(IBig::from).collect()
        })
        .collect()
}

#[test]
fn bareiss_agrees_with_plain_elimination_on_matrices_of_every_shape() {
    let mut rng = SplitMix64::new(seed());
    for _ in 0..200 {
        let rows = 1 + (rng.next_u64() % 9) as usize;
        let columns = 1 + (rng.next_u64() % 9) as usize;
        let target = (rng.next_u64() % (rows.min(columns) as u64 + 1)) as usize;
        let m = matrix(&mut rng, rows, columns, target);
        let b = bareiss_rank(&m).unwrap();
        assert_eq!(b.rank, gauss_rank(&m).unwrap(), "{m:?}");
        assert!(b.rank <= target, "{m:?}");
        assert_eq!(b.pivots.len(), b.rank);
    }
}

#[test]
fn the_rank_of_known_matrices() {
    let ints = |rows: &[&[i64]]| -> Vec<Vec<IBig>> {
        rows.iter()
            .map(|r| r.iter().map(|x| IBig::from(*x)).collect())
            .collect()
    };
    assert_eq!(bareiss_rank(&ints(&[])).unwrap().rank, 0);
    assert_eq!(bareiss_rank(&ints(&[&[0, 0], &[0, 0]])).unwrap().rank, 0);
    assert_eq!(bareiss_rank(&ints(&[&[1, 2], &[2, 4]])).unwrap().rank, 1);
    assert_eq!(
        bareiss_rank(&ints(&[&[1, 2, 3], &[4, 5, 6], &[7, 8, 9]]))
            .unwrap()
            .rank,
        2
    );
    assert_eq!(
        bareiss_rank(&ints(&[&[2, 0, 0], &[0, 3, 0], &[0, 0, 5]]))
            .unwrap()
            .rank,
        3
    );
    // A column of zeros in the middle is skipped.
    let skipped = bareiss_rank(&ints(&[&[1, 0, 2], &[3, 0, 4]])).unwrap();
    assert_eq!((skipped.rank, skipped.pivots), (2, vec![0, 2]));
    assert!(bareiss_rank(&[vec![IBig::ONE], vec![]]).is_err());
    assert!(gauss_rank(&[vec![IBig::ONE], vec![]]).is_err());
}

#[test]
fn the_exact_rank_of_a_family_is_the_one_it_is_built_to_have() {
    let mut polynomial = 0;
    let mut refused = 0;
    for case in cases(seed(), 400) {
        let Ok(rows) = jacobian(&case.job) else {
            refused += 1;
            continue;
        };
        polynomial += 1;
        let columns = case.job.points.len() * 2;
        let exact = bareiss_rank(&rows).unwrap();
        assert_eq!(
            exact.rank,
            gauss_rank(&rows).unwrap(),
            "case {}",
            case.job.id
        );
        // Degrees of freedom are the unknowns less the rank, and the dependent equations are the
        // redundant and conflicting groups (each of these families has groups of one).
        assert_eq!(
            columns - exact.rank,
            case.expect.freedom,
            "case {} ({})",
            case.job.id,
            case.family.name()
        );
        assert_eq!(
            rows.len() - exact.rank,
            case.expect.redundant + case.expect.conflicting,
            "case {} ({})",
            case.job.id,
            case.family.name()
        );
        // And the kernel's pivoted-QR diagnosis says the same.
        let kernel = kernel(&case).unwrap();
        assert_eq!(columns - exact.rank, kernel.freedom_least);
        assert_eq!(kernel.freedom_least, kernel.freedom_most);
        assert_eq!(
            rows.len() - exact.rank,
            kernel.redundant + kernel.conflicting
        );
    }
    // The growth family draws angles, which have no polynomial form; the rest are read.
    assert!(polynomial >= 300, "{polynomial}");
    assert!(refused > 0 && refused <= 40, "{refused}");
}

#[test]
fn an_angle_has_no_polynomial_form() {
    let case = cases(seed(), 400)
        .into_iter()
        .find(|c| jacobian(&c.job).is_err())
        .expect("a growth case with an angle");
    assert!(jacobian(&case.job).unwrap_err().contains("angle"));
}

#[test]
fn a_job_naming_a_point_it_lacks_is_refused() {
    let mut case = cases(seed(), 6).remove(0);
    case.job
        .rules
        .push(geom_test::oracle::sketches::Rule::Level { a: 0, b: 99 });
    assert!(jacobian(&case.job).unwrap_err().contains("no point 99"));
}

#[test]
fn the_cost_of_a_chain_grows_as_the_note_says() {
    let costs: Vec<_> = [5, 10, 20]
        .iter()
        .map(|n| chain_cost(*n).unwrap())
        .collect();
    assert!(costs[0].products < costs[1].products && costs[1].products < costs[2].products);
    assert!(costs[0].bits < costs[2].bits);
    // A chain has full rank: as many independent equations as unknowns.
    for (points, cost) in [5, 10, 20].iter().zip(&costs) {
        assert_eq!(cost.rank, points * 2);
    }
    let m = chain_matrix(12, 7).unwrap();
    assert_eq!(bareiss_rank(&m).unwrap().rank, 24);
}

#[test]
#[ignore = "the cost table of the note: cargo test -p geom-test --test exact_rank --release -- --ignored --nocapture"]
#[allow(
    clippy::disallowed_methods,
    reason = "a timing, printed and not asserted"
)]
fn print_the_cost_table() {
    println!("points unknowns rank bits products milliseconds");
    for points in [10, 20, 40, 80, 160, 250, 330, 500] {
        let start = std::time::Instant::now();
        let cost = chain_cost(points).unwrap();
        println!(
            "{points} {} {} {} {} {}",
            points * 2,
            cost.rank,
            cost.bits,
            cost.products,
            start.elapsed().as_millis()
        );
    }
}
