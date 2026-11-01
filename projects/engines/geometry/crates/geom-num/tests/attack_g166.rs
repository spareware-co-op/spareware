//! Attacks on G166: every pencil type, each sign of its canonical pair, and what the two quadrics
//! share read off the pair by hand, then the pair scrambled by whole-number changes of coordinates
//! and of the pencil's basis, which leave the answer alone. Natural quadrics whose meetings a hand can
//! check come after, and the budget at the frame's edge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::large_types_passed_by_value,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::type_complexity,
    reason = "test code: a failure should stop loudly"
)]

use geom_num::NumError;
use geom_num::grid::{P3, Steps};
use geom_num::interval::Interval;
use geom_num::pencil::{
    Entry, Form, Inertia, Pencil, Quartic, Segre, Topology, classify, cone_form, cylinder_form,
    filtered, inertia_certain, inertia_of, revolution, sphere_form,
};
use geom_num::quadric::{self, Ring, Slope};
use geom_num::sign::Sign;
use geom_num::sturm::{gcd, square_root};
use geom_num::wide::Wide;

const NONE: Topology = Topology {
    quartic: None,
    cubics: 0,
    conics: 0,
    lines: 0,
    double_conics: 0,
    double_lines: 0,
    points: 0,
    meets: 0,
    same: false,
};

const SIGNS: [i128; 2] = [1, -1];

type Small = [[i128; 4]; 4];

/// A pencil tA + B by its two matrices.
#[derive(Clone, Copy)]
struct Pair {
    a: Small,
    b: Small,
}

fn zero() -> Pair {
    Pair {
        a: [[0; 4]; 4],
        b: [[0; 4]; 4],
    }
}

impl Pair {
    fn sym(m: &mut Small, i: usize, j: usize, v: i128) {
        m[i][j] += v;
        if i != j {
            m[j][i] += v;
        }
    }

    /// A Jordan block of `size` at the real root `a`, of sign `e`, on the variables from `at`:
    /// e((t − a)·(the antidiagonal) + (the diagonal below it)).
    fn block(mut self, at: usize, size: usize, a: i128, e: i128) -> Pair {
        for i in 0..size {
            let j = size - 1 - i;
            if i <= j {
                Pair::sym(&mut self.a, at + i, at + j, e);
                Pair::sym(&mut self.b, at + i, at + j, -a * e);
            }
        }
        for i in 1..size {
            let j = size - i;
            if i <= j {
                Pair::sym(&mut self.b, at + i, at + j, e);
            }
        }
        self
    }

    /// Roots a ± bi on two variables: Re[(t − a − bi)(x + iy)²].
    fn conjugate(mut self, at: usize, a: i128, b: i128) -> Pair {
        Pair::sym(&mut self.a, at, at, 1);
        Pair::sym(&mut self.a, at + 1, at + 1, -1);
        Pair::sym(&mut self.b, at, at, -a);
        Pair::sym(&mut self.b, at + 1, at + 1, a);
        Pair::sym(&mut self.b, at, at + 1, b);
        self
    }

    /// Roots a ± bi, a block of two for each, on four variables: Re[(t − z)2w₁w₂ + w₂²].
    fn conjugate_block(mut self, a: i128, b: i128) -> Pair {
        // w₁ = x₀ + ix₁, w₂ = x₂ + ix₃: Re(2w₁w₂) = 2x₀x₂ − 2x₁x₃, Im(2w₁w₂) = 2x₀x₃ + 2x₁x₂.
        for (i, j, v) in [(0, 2, 1), (1, 3, -1)] {
            Pair::sym(&mut self.a, i, j, v);
            Pair::sym(&mut self.b, i, j, -a * v);
        }
        for (i, j) in [(0, 3), (1, 2)] {
            Pair::sym(&mut self.b, i, j, b);
        }
        Pair::sym(&mut self.b, 2, 2, 1);
        Pair::sym(&mut self.b, 3, 3, -1);
        self
    }

    /// `u`ᵀ(·)`u` on both.
    fn scrambled(self, u: &Small) -> Pair {
        let turn = |m: &Small| {
            let mut out = [[0; 4]; 4];
            for (i, row) in out.iter_mut().enumerate() {
                for (j, slot) in row.iter_mut().enumerate() {
                    for k in 0..4 {
                        for l in 0..4 {
                            *slot += u[k][i] * m[k][l] * u[l][j];
                        }
                    }
                }
            }
            out
        };
        Pair {
            a: turn(&self.a),
            b: turn(&self.b),
        }
    }

    /// A new basis of the pencil, of determinant one.
    fn mixed(self, [p, q, r, s]: [i128; 4]) -> Pair {
        let both = |x: i128, y: i128| {
            let mut out = [[0; 4]; 4];
            for (i, row) in out.iter_mut().enumerate() {
                for (j, slot) in row.iter_mut().enumerate() {
                    *slot = x * self.a[i][j] + y * self.b[i][j];
                }
            }
            out
        };
        Pair {
            a: both(p, q),
            b: both(r, s),
        }
    }

    fn forms(&self) -> (Form<Entry>, Form<Entry>) {
        let form = |m: &Small| m.map(|row| row.map(Entry::from_i128));
        (form(&self.a), form(&self.b))
    }
}

fn step(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A whole-number matrix of determinant one: the identity under a few shears.
fn shears(state: &mut u64) -> Small {
    let mut u = [[0; 4]; 4];
    for (i, row) in u.iter_mut().enumerate() {
        row[i] = 1;
    }
    for _ in 0..8 {
        let i = (step(state) % 4) as usize;
        let j = (step(state) % 4) as usize;
        if i == j {
            continue;
        }
        let k = i128::from(step(state) % 5) - 2;
        for row in &mut u {
            row[j] += k * row[i];
        }
    }
    u
}

/// The answer on the pair, and on it scrambled a dozen ways.
fn check(name: &str, pair: Pair, segre: Segre, want: Topology) {
    let (a, b) = pair.forms();
    let got = classify(&a, &b).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    assert_eq!((got.segre, got.topology), (segre, want), "{name}");
    let mut state = 0x166;
    for mix in [
        [1, 0, 0, 1],
        [1, 1, 0, 1],
        [2, 1, 1, 1],
        [1, -2, 1, -1],
        [3, 2, 4, 3],
    ] {
        let u = shears(&mut state);
        let moved = pair.scrambled(&u).mixed(mix);
        let (a, b) = moved.forms();
        let got = classify(&a, &b).unwrap_or_else(|e| panic!("{name} {mix:?}: {e:?}"));
        assert_eq!((got.segre, got.topology), (segre, want), "{name} {mix:?}");
    }
}

fn smooth(ovals: u8) -> Topology {
    Topology {
        quartic: Some(Quartic::Smooth { ovals }),
        ..NONE
    }
}

fn quartic(q: Quartic) -> Topology {
    Topology {
        quartic: Some(q),
        ..NONE
    }
}

/// Whether Σ σᵢ(t − aᵢ)xᵢ² has a definite member: a sample of each gap, read directly.
fn definite(diagonal: &[(i128, i128)]) -> bool {
    let doubled = |t: i128| diagonal.iter().map(move |(a, s)| s * (t - 2 * a));
    let mut samples: Vec<i128> = diagonal.iter().map(|(a, _)| 2 * a + 1).collect();
    samples.push(diagonal.iter().map(|(a, _)| 2 * a).min().unwrap() - 1);
    samples
        .iter()
        .any(|t| doubled(*t).all(|v| v > 0) || doubled(*t).all(|v| v < 0))
        || diagonal.iter().map(|(_, s)| *s).all(|s| s == diagonal[0].1)
}

#[test]
fn four_simple_roots() {
    for a in SIGNS {
        for b in SIGNS {
            for c in SIGNS {
                for d in SIGNS {
                    let pair = zero()
                        .block(0, 1, 0, a)
                        .block(1, 1, 1, b)
                        .block(2, 1, 2, c)
                        .block(3, 1, 3, d);
                    let empty = definite(&[(0, a), (1, b), (2, c), (3, d)]);
                    let want = if empty { NONE } else { smooth(2) };
                    check("[1111] real", pair, Segre::Simple, want);
                }
            }
        }
    }
    for a in SIGNS {
        for b in SIGNS {
            let pair = zero()
                .block(0, 1, 0, a)
                .block(1, 1, 1, b)
                .conjugate(2, 2, 1);
            check("[1111] two real", pair, Segre::Simple, smooth(1));
        }
    }
    let pair = zero().conjugate(0, 0, 1).conjugate(2, 3, 2);
    check("[1111] none real", pair, Segre::Simple, smooth(2));
}

#[test]
fn one_double_root_of_rank_three() {
    for e in SIGNS {
        for s3 in SIGNS {
            for s4 in SIGNS {
                for (b, c) in [(1, 2), (-1, 2), (-2, -1)] {
                    let pair = zero()
                        .block(0, 2, 0, e)
                        .block(2, 1, b, s3)
                        .block(3, 1, c, s4);
                    let (r3, r4) = (-s3 * b, -s4 * c);
                    let want = if r3 * r4 < 0 {
                        quartic(Quartic::Crossing)
                    } else if e * r3 > 0 {
                        Topology { points: 1, ..NONE }
                    } else {
                        quartic(Quartic::Isolated)
                    };
                    check("[211]", pair, Segre::Double, want);
                }
            }
        }
        let pair = zero().block(0, 2, 0, e).conjugate(2, 1, 1);
        check(
            "[211] conjugate",
            pair,
            Segre::Double,
            quartic(Quartic::Crossing),
        );
    }
}

#[test]
fn one_triple_root_of_rank_three() {
    for e in SIGNS {
        for s in SIGNS {
            for c in [1, -1] {
                let pair = zero().block(0, 3, 0, e).block(3, 1, c, s);
                check("[31]", pair, Segre::Triple, quartic(Quartic::Cusp));
            }
        }
    }
}

#[test]
fn two_double_roots_of_rank_three() {
    let real = Topology {
        cubics: 1,
        lines: 1,
        meets: 2,
        ..NONE
    };
    for e in SIGNS {
        for f in SIGNS {
            let pair = zero().block(0, 2, 0, e).block(2, 2, 1, f);
            check("[22]", pair, Segre::TwoDoubles, real);
        }
    }
    let pair = zero().conjugate_block(1, 2);
    let far = Topology { meets: 0, ..real };
    check("[22] conjugate", pair, Segre::TwoDoubles, far);
}

#[test]
fn a_fourfold_root_of_rank_three() {
    for e in SIGNS {
        let pair = zero().block(0, 4, 0, e);
        let want = Topology {
            cubics: 1,
            lines: 1,
            meets: 1,
            ..NONE
        };
        check("[4]", pair, Segre::Fourfold, want);
    }
}

#[test]
fn one_double_root_of_rank_two() {
    for s1 in SIGNS {
        for s2 in SIGNS {
            for s3 in SIGNS {
                for s4 in SIGNS {
                    for (b, c) in [(1, 2), (-1, 2), (-2, -1)] {
                        let pair = zero()
                            .block(0, 1, 0, s1)
                            .block(1, 1, 0, s2)
                            .block(2, 1, b, s3)
                            .block(3, 1, c, s4);
                        let (r3, r4) = (-s3 * b, -s4 * c);
                        let planes = r3 * r4 < 0;
                        let cross = s1 * s2 < 0;
                        let want = match (planes, cross) {
                            (true, true) => Topology {
                                conics: 2,
                                meets: 2,
                                ..NONE
                            },
                            (true, false) => {
                                // Each plane's conic is real exactly when the other coordinates
                                // weigh against the pair's sign: s₃(c − b)/c.
                                if s1 * s3 * (c - b) * c < 0 {
                                    Topology { conics: 2, ..NONE }
                                } else {
                                    NONE
                                }
                            }
                            (false, true) => Topology { points: 2, ..NONE },
                            (false, false) => NONE,
                        };
                        check("[(11)11]", pair, Segre::Pair, want);
                    }
                }
            }
        }
    }
    for s in SIGNS {
        let pair = zero()
            .block(0, 1, 0, 1)
            .block(1, 1, 0, -1)
            .conjugate(2, 1, 1);
        let want = Topology {
            conics: 2,
            meets: 2,
            ..NONE
        };
        check("[(11)11] crossing, conjugate", pair, Segre::Pair, want);
        let pair = zero()
            .block(0, 1, 0, s)
            .block(1, 1, 0, s)
            .conjugate(2, 1, 1);
        check(
            "[(11)11] one conic, conjugate",
            pair,
            Segre::Pair,
            Topology { conics: 1, ..NONE },
        );
    }
}

#[test]
fn double_roots_of_ranks_two_and_three() {
    for s1 in SIGNS {
        for s2 in SIGNS {
            for e in SIGNS {
                for b in [1, -2] {
                    let pair = zero()
                        .block(0, 1, 0, s1)
                        .block(1, 1, 0, s2)
                        .block(2, 2, b, e);
                    let want = if s1 * s2 < 0 {
                        Topology {
                            conics: 1,
                            lines: 2,
                            meets: 3,
                            ..NONE
                        }
                    } else if s1 * e * b < 0 {
                        Topology {
                            conics: 1,
                            points: 1,
                            ..NONE
                        }
                    } else {
                        Topology { points: 1, ..NONE }
                    };
                    check("[(11)2]", pair, Segre::PairAndDouble, want);
                }
            }
        }
    }
}

#[test]
fn two_double_roots_of_rank_two() {
    for s1 in SIGNS {
        for s2 in SIGNS {
            for s3 in SIGNS {
                for s4 in SIGNS {
                    let pair = zero()
                        .block(0, 1, 0, s1)
                        .block(1, 1, 0, s2)
                        .block(2, 1, 1, s3)
                        .block(3, 1, 1, s4);
                    let want = match (s1 * s2 < 0, s3 * s4 < 0) {
                        (true, true) => Topology {
                            lines: 4,
                            meets: 4,
                            ..NONE
                        },
                        (false, false) => NONE,
                        _ => Topology { points: 2, ..NONE },
                    };
                    check("[(11)(11)]", pair, Segre::TwoPairs, want);
                }
            }
        }
    }
    let pair = zero().conjugate(0, 1, 2).conjugate(2, 1, 2);
    check(
        "[(11)(11)] conjugate",
        pair,
        Segre::TwoPairs,
        Topology { lines: 2, ..NONE },
    );
}

#[test]
fn two_double_roots_of_rank_two_on_irrational_roots() {
    // (x₀² − 2x₁²) t-roots: the members at ±√2 by a pencil with entries in whole numbers.
    let mut a: Small = [[0; 4]; 4];
    let mut b: Small = [[0; 4]; 4];
    // tA + B on two 2 × 2 blocks [[t, 2], [1·, t]]-like: symmetric as [[t, 2], [2, 2t]].
    for at in [0, 2] {
        a[at][at] = 1;
        a[at + 1][at + 1] = 2;
        b[at][at + 1] = 2;
        b[at + 1][at] = 2;
    }
    // At +√2 each block keeps one positive eigenvalue, at −√2 one negative: both members definite,
    // and t = 2 between... the whole pencil holds a definite member.
    let pair = Pair { a, b };
    let got = classify(&pair.forms().0, &pair.forms().1).unwrap();
    assert_eq!((got.segre, got.real_roots), (Segre::TwoPairs, 2));
    assert_eq!(got.topology, NONE);
    // The second block negated: mixed at both.
    let mut flipped = pair;
    for row in 2..4 {
        for column in 2..4 {
            flipped.a[row][column] = -flipped.a[row][column];
            flipped.b[row][column] = -flipped.b[row][column];
        }
    }
    let got = classify(&flipped.forms().0, &flipped.forms().1).unwrap();
    assert_eq!(got.segre, Segre::TwoPairs);
    let four = Topology {
        lines: 4,
        meets: 4,
        ..NONE
    };
    assert_eq!(got.topology, four);
    // The second block [[2, t], [t, 1]], positive at both roots: mixed at −√2 alone.
    let mut one = pair;
    for row in 2..4 {
        for column in 2..4 {
            one.a[row][column] = 0;
            one.b[row][column] = 0;
        }
    }
    one.a[2][3] = 1;
    one.a[3][2] = 1;
    one.b[2][2] = 2;
    one.b[3][3] = 1;
    let got = classify(&one.forms().0, &one.forms().1).unwrap();
    assert_eq!(got.segre, Segre::TwoPairs);
    assert_eq!(got.topology, Topology { points: 2, ..NONE });
}

#[test]
fn one_triple_root_of_rank_two() {
    for e in SIGNS {
        for s in SIGNS {
            for t in SIGNS {
                for c in [1, -1] {
                    let pair = zero().block(0, 2, 0, e).block(2, 1, 0, s).block(3, 1, c, t);
                    let want = if e * t * c > 0 {
                        Topology {
                            conics: 2,
                            meets: 1,
                            ..NONE
                        }
                    } else {
                        Topology { points: 1, ..NONE }
                    };
                    check("[(21)1]", pair, Segre::TriplePair, want);
                }
            }
        }
    }
}

#[test]
fn fourfold_roots_of_rank_two() {
    for e in SIGNS {
        for s in SIGNS {
            let pair = zero().block(0, 3, 0, e).block(3, 1, 0, s);
            let want = if e * s < 0 {
                Topology {
                    conics: 1,
                    lines: 2,
                    meets: 1,
                    ..NONE
                }
            } else {
                Topology { conics: 1, ..NONE }
            };
            check("[(31)]", pair, Segre::FourfoldPair, want);
            let pair = zero().block(0, 2, 0, e).block(2, 2, 0, s);
            let want = if e * s < 0 {
                Topology {
                    double_lines: 1,
                    lines: 2,
                    meets: 2,
                    ..NONE
                }
            } else {
                Topology {
                    double_lines: 1,
                    ..NONE
                }
            };
            check("[(22)]", pair, Segre::FourfoldTwoBlocks, want);
        }
    }
}

#[test]
fn roots_of_rank_one() {
    for s1 in SIGNS {
        for s2 in SIGNS {
            for s3 in SIGNS {
                for t in SIGNS {
                    for c in [1, -1] {
                        let pair = zero()
                            .block(0, 1, 0, s1)
                            .block(1, 1, 0, s2)
                            .block(2, 1, 0, s3)
                            .block(3, 1, c, t);
                        let want = if s1 == s2 && s2 == s3 {
                            NONE
                        } else {
                            Topology {
                                double_conics: 1,
                                ..NONE
                            }
                        };
                        check("[(111)1]", pair, Segre::TripleRankOne, want);
                    }
                }
                let pair = zero()
                    .block(0, 2, 0, s1)
                    .block(2, 1, 0, s2)
                    .block(3, 1, 0, s3);
                let want = if s2 * s3 < 0 {
                    Topology {
                        double_lines: 2,
                        meets: 1,
                        ..NONE
                    }
                } else {
                    Topology { points: 1, ..NONE }
                };
                check("[(211)]", pair, Segre::FourfoldRankOne, want);
            }
        }
    }
}

#[test]
fn one_quadric_twice() {
    for (signs, empty) in [
        ([1, 1, 1, 1], true),
        ([-1, -1, -1, -1], true),
        ([1, 1, 1, -1], false),
        ([1, 1, -1, -1], false),
    ] {
        let mut a: Small = [[0; 4]; 4];
        for i in 0..4 {
            a[i][i] = signs[i];
        }
        let mut b = a;
        for row in &mut b {
            for v in row {
                *v *= 3;
            }
        }
        let want = if empty {
            NONE
        } else {
            Topology { same: true, ..NONE }
        };
        check("[(1111)]", Pair { a, b }, Segre::Same, want);
    }
}

/// A pencil of conics on three variables, the fourth the shared apex.
#[test]
fn cones_over_one_apex() {
    let line = |lines, double_lines| Topology {
        lines,
        double_lines,
        meets: 1,
        ..NONE
    };
    let apex = Topology { points: 1, ..NONE };
    for a in SIGNS {
        for b in SIGNS {
            for c in SIGNS {
                let pair = zero().block(0, 1, 0, a).block(1, 1, 1, b).block(2, 1, 2, c);
                let empty = definite(&[(0, a), (1, b), (2, c)]);
                check(
                    "[111]",
                    pair,
                    Segre::OneApex,
                    if empty { apex } else { line(4, 0) },
                );
                let pair = zero().block(0, 1, 0, a).conjugate(1, 1, 1);
                check("[111] conjugate", pair, Segre::OneApex, line(2, 0));
                // A double root of rank two.
                let pair = zero().block(0, 2, 0, a).block(2, 1, 2, b);
                let want = if a * b * 2 > 0 {
                    Topology {
                        double_lines: 1,
                        lines: 2,
                        meets: 1,
                        ..NONE
                    }
                } else {
                    Topology {
                        double_lines: 1,
                        ..NONE
                    }
                };
                check("[21]", pair, Segre::OneApex, want);
                // A double root of rank one.
                let pair = zero().block(0, 1, 0, a).block(1, 1, 0, b).block(2, 1, 2, c);
                let want = if a * b < 0 { line(0, 2) } else { apex };
                check("[(11)1]", pair, Segre::OneApex, want);
            }
            let pair = zero().block(0, 3, 0, a);
            check("[3]", pair, Segre::OneApex, line(2, 0));
            let pair = zero().block(0, 2, 0, a).block(2, 1, 0, b);
            let want = Topology {
                double_lines: 1,
                ..NONE
            };
            check("[(21)]", pair, Segre::OneApex, want);
        }
    }
    let same = |signs: [i128; 3], want: Topology| {
        let mut a: Small = [[0; 4]; 4];
        for i in 0..3 {
            a[i][i] = signs[i];
        }
        let mut b = a;
        for row in &mut b {
            for v in row {
                *v *= 2;
            }
        }
        check("[(111)]", Pair { a, b }, Segre::OneApex, want);
    };
    same([1, 1, 1], apex);
    same([1, 1, -1], Topology { same: true, ..NONE });
}

#[test]
fn cones_that_touch_along_a_line() {
    // 2x₂(t x₀ + x₁) + σ(t − a)x₃²: the kernel moves along the line x₂ = x₃ = 0.
    for s in SIGNS {
        for root in [0, 2] {
            let mut pair = zero();
            Pair::sym(&mut pair.a, 0, 2, 1);
            Pair::sym(&mut pair.b, 1, 2, 1);
            Pair::sym(&mut pair.a, 3, 3, s);
            Pair::sym(&mut pair.b, 3, 3, -root * s);
            let want = Topology {
                conics: 1,
                double_lines: 1,
                meets: 1,
                ..NONE
            };
            check("cones touching", pair, Segre::TouchingCones, want);
        }
    }
}

#[test]
fn every_member_of_too_low_a_rank_is_refused() {
    let mut pair = zero();
    pair.a[0][0] = 1;
    pair.b[1][1] = 1;
    let (a, b) = pair.forms();
    assert_eq!(classify(&a, &b), Err(NumError::Degenerate));
}

fn e(v: i64) -> Entry {
    Entry::from_i128(i128::from(v))
}

fn v3(p: [i64; 3]) -> [Entry; 3] {
    p.map(e)
}

fn sphere(c: [i64; 3], r: i64) -> Form<Entry> {
    revolution(v3(c), v3([0, 0, 0]), e(1), e(0), e(r * r)).unwrap()
}

fn dot(a: [i64; 3], b: [i64; 3]) -> i64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cylinder(c: [i64; 3], d: [i64; 3], r: i64) -> Form<Entry> {
    let n = dot(d, d);
    revolution(v3(c), v3(d), e(n), e(1), e(r * r * n)).unwrap()
}

/// The cone of slope s/t at `c` about `d`.
fn cone(c: [i64; 3], d: [i64; 3], s: i64, t: i64) -> Form<Entry> {
    let n = dot(d, d);
    revolution(v3(c), v3(d), e(t * t * n), e(s * s + t * t), e(0)).unwrap()
}

fn on(a: &Form<Entry>, b: &Form<Entry>) -> Pencil {
    classify(a, b).unwrap()
}

const Z: [i64; 3] = [0, 0, 1];
const X: [i64; 3] = [1, 0, 0];

#[test]
fn a_sphere_and_a_cylinder() {
    let s = sphere([0, 0, 0], 5);
    assert_eq!(on(&s, &cylinder([1, 0, 0], Z, 1)).topology, smooth(2));
    assert_eq!(on(&s, &cylinder([5, 0, 0], Z, 1)).topology, smooth(1));
    assert_eq!(on(&s, &cylinder([10, 0, 0], Z, 1)).topology, NONE);
    // Viviani's window: a cylinder through the centre and touching the sphere inside.
    let viviani = on(&sphere([0, 0, 0], 2), &cylinder([1, 0, 0], Z, 1));
    assert_eq!(viviani.segre, Segre::Double);
    assert_eq!(viviani.topology, quartic(Quartic::Crossing));
    // A sphere inside the cylinder, apart from it and touching it from inside, and from outside.
    assert_eq!(
        on(&sphere([0, 0, 0], 1), &cylinder([1, 0, 0], Z, 5)).topology,
        NONE
    );
    let touch = Topology { points: 1, ..NONE };
    assert_eq!(
        on(&sphere([0, 0, 0], 2), &cylinder([1, 0, 0], Z, 3)).topology,
        touch
    );
    assert_eq!(
        on(&sphere([0, 0, 0], 1), &cylinder([3, 0, 0], Z, 2)).topology,
        touch
    );
    // Coaxial: two circles, one at the equator, none.
    let two = on(&sphere([0, 0, 0], 5), &cylinder([0, 0, 0], Z, 3));
    assert_eq!(two.topology.conics, 2);
    let equator = on(&sphere([0, 0, 0], 5), &cylinder([0, 0, 0], Z, 5));
    assert_eq!(equator.topology.double_conics, 1);
}

#[test]
fn two_cylinders() {
    let a = cylinder([0, 0, 0], Z, 1);
    // Axes square and meeting: a thin pipe through a wide one, and Steinmetz's two ellipses.
    assert_eq!(on(&a, &cylinder([0, 0, 0], X, 2)).topology, smooth(2));
    let tee = on(&a, &cylinder([0, 0, 0], X, 1));
    assert_eq!(tee.segre, Segre::Pair);
    assert_eq!(
        tee.topology,
        Topology {
            conics: 2,
            meets: 2,
            ..NONE
        }
    );
    // Axes one apart, two apart and five apart, equal radii, and on a slant three apart.
    assert_eq!(on(&a, &cylinder([0, 1, 0], X, 1)).topology, smooth(1));
    assert_eq!(
        on(&a, &cylinder([0, 2, 0], X, 1)).topology,
        Topology { points: 1, ..NONE }
    );
    assert_eq!(on(&a, &cylinder([0, 5, 0], X, 1)).topology, NONE);
    assert_eq!(on(&a, &cylinder([3, 3, 3], [0, 1, 1], 1)).topology, NONE);
    // Parallel axes: two lines, one, none; all members singular at one point at infinity.
    let parallel = on(&a, &cylinder([1, 0, 0], Z, 1));
    assert_eq!(parallel.segre, Segre::OneApex);
    assert_eq!(parallel.topology.lines, 2);
    let touching = on(&a, &cylinder([2, 0, 0], Z, 1));
    assert_eq!(touching.topology.double_lines, 1);
    assert_eq!(on(&a, &cylinder([3, 0, 0], Z, 1)).topology.lines, 0);
}

#[test]
fn cones_with_other_quadrics() {
    let c = cone([0, 0, 0], Z, 1, 1);
    // A cylinder through the apex: the node of the quartic sits there.
    let through = on(&c, &cylinder([1, 0, 0], Z, 1));
    assert_eq!(through.segre, Segre::Double);
    assert_eq!(through.topology, quartic(Quartic::Crossing));
    // A cylinder about the cone's axis: two circles.
    let coaxial = on(&c, &cylinder([0, 0, 0], Z, 3));
    assert_eq!(coaxial.topology.conics, 2);
    // A cylinder holding one ruling: that line and a twisted cubic.
    let ruling = on(&c, &cylinder([0, 1, 0], [1, 0, 1], 1));
    assert_eq!(ruling.topology.cubics, 1);
    assert_eq!(ruling.topology.lines, 1);
    // Two cones of one apex: four rulings, two along which they touch, and only the apex.
    let o = [0, 0, 0];
    assert_eq!(on(&cone(o, Z, 2, 1), &cone(o, X, 2, 1)).topology.lines, 4);
    assert_eq!(
        on(&cone(o, Z, 1, 1), &cone(o, X, 1, 1))
            .topology
            .double_lines,
        2
    );
    assert_eq!(
        on(&cone(o, Z, 1, 2), &cone(o, X, 1, 2)).topology,
        Topology { points: 1, ..NONE }
    );
    // One slope on parallel axes, the second apex on the first cone: they touch along that ruling,
    // and share the conic at infinity.
    let along = on(&cone(o, Z, 1, 1), &cone([1, 0, 1], Z, 1, 1));
    assert_eq!(along.segre, Segre::TouchingCones);
    assert_eq!(along.topology.double_lines, 1);
    // One axis and one slope: a circle, and that conic again.
    let coaxial = on(&cone(o, Z, 1, 1), &cone([0, 0, 2], Z, 1, 1));
    assert_eq!(coaxial.segre, Segre::Pair);
    assert_eq!(coaxial.topology, Topology { conics: 2, ..NONE });
}

#[test]
fn two_spheres() {
    let a = sphere([0, 0, 0], 5);
    // They share the circle at infinity, which has no real point, and their real circle.
    let met = on(&a, &sphere([6, 0, 0], 5));
    assert_eq!(met.segre, Segre::Pair);
    assert_eq!(met.topology, Topology { conics: 1, ..NONE });
    assert_eq!(on(&a, &sphere([20, 0, 0], 5)).topology, NONE);
    assert_eq!(on(&a, &sphere([0, 0, 0], 4)).topology, NONE);
    assert_eq!(on(&a, &a).topology, NONE.same_as());
}

trait Whole {
    fn same_as(self) -> Topology;
}

impl Whole for Topology {
    fn same_as(self) -> Topology {
        Topology { same: true, ..self }
    }
}

#[test]
fn the_filter_agrees_where_it_answers() {
    let i = |v: i64| Interval::of(v).unwrap();
    let form = |c: [i64; 3], d: [i64; 3], alpha: i64, beta: i64, gamma: i64| {
        revolution(c.map(i), d.map(i), i(alpha), i(beta), i(gamma)).unwrap()
    };
    let sphere_i = |r: i64| form([0, 0, 0], [0, 0, 0], 1, 0, r * r);
    let tube = |x: i64, r: i64| form([x, 0, 0], Z, 1, 1, r * r);
    assert_eq!(filtered(&sphere_i(5), &tube(1, 1)), Some(smooth(2)));
    assert_eq!(filtered(&sphere_i(5), &tube(5, 1)), Some(smooth(1)));
    assert_eq!(filtered(&sphere_i(5), &tube(10, 1)), Some(NONE));
    // Tangent: no interval excludes the double root.
    assert_eq!(filtered(&sphere_i(2), &tube(1, 1)), None);
}

#[test]
fn the_budget_holds_at_the_frame_s_edge() {
    let far = (1 << 40) - 1;
    let tip = (1 << 41) - 2;
    let radius = (1_i64 << 53) - 1;
    // Sizes past an i64's squares, so built in entries: each cone and cylinder at its largest.
    let big = |v: i64| Entry::from_i128(i128::from(v));
    let square = |v: i64| big(v).checked_mul(&big(v)).unwrap();
    let d = [tip, tip, tip];
    let n = square(tip).checked_mul(&big(3)).unwrap();
    let c = [far, -far, far];
    let tube = revolution(
        c.map(big),
        d.map(big),
        n,
        big(1),
        square(radius).checked_mul(&n).unwrap(),
    )
    .unwrap();
    let cone_form = revolution(
        [-far, far, far].map(big),
        [tip, -tip, tip].map(big),
        square(radius).checked_mul(&n).unwrap(),
        square(radius).checked_add(&square(radius - 1)).unwrap(),
        big(0),
    )
    .unwrap();
    let got = classify(&cone_form, &tube).unwrap();
    assert!(got.real_roots <= 4);
}

#[test]
fn inertia_reads_each_sign_pattern() {
    let plus = Sign::Positive;
    let minus = Sign::Negative;
    let zero_sign = Sign::Zero;
    // diag(1, 2, 3, 4), diag(1, −2, −3, 4), diag(0, 1, −1, 0), and diag(−1, −1, −1, −1).
    assert_eq!(
        inertia_of([plus, plus, plus, plus]),
        Inertia {
            positive: 4,
            negative: 0
        }
    );
    assert_eq!(
        inertia_of([zero_sign, minus, plus, plus]),
        Inertia {
            positive: 2,
            negative: 2
        }
    );
    assert_eq!(
        inertia_of([zero_sign, minus, zero_sign, zero_sign]),
        Inertia {
            positive: 1,
            negative: 1
        }
    );
    assert_eq!(
        inertia_of([minus, plus, minus, plus]),
        Inertia {
            positive: 0,
            negative: 4
        }
    );
}

fn grid(x: i64, y: i64, z: i64) -> P3 {
    P3::new(x, y, z).unwrap()
}

#[test]
fn natural_quadrics_are_written_about_any_grid_point() {
    let origin = grid(1, 2, 3);
    let s = quadric::Sphere {
        centre: grid(4, 2, 3),
        radius: Steps(5),
    };
    assert_eq!(sphere_form(&s, origin), Some(sphere([3, 0, 0], 5)));
    let c = quadric::Cylinder {
        base: grid(1, 2, 3),
        along: grid(1, 2, 7),
        radius: Steps(2),
    };
    assert_eq!(
        cylinder_form(&c, origin),
        Some(cylinder([0, 0, 0], [0, 0, 4], 2))
    );
    let k = quadric::Cone {
        apex: grid(1, 2, 3),
        along: grid(1, 2, 4),
        slope: Slope { rise: 3, run: 2 },
    };
    assert_eq!(
        cone_form(&k, origin),
        Some(cone([0, 0, 0], [0, 0, 1], 3, 2))
    );
}

#[test]
fn a_definite_member_in_a_far_gap_is_found_by_the_turned_chain() {
    // Roots of b + k·a at k = −7.1, −7.2, −7.3 and −7.4, which t reads past ±1.
    let diagonal = |signs: [i128; 4], a: i128, offset: i128| {
        let mut pair = zero();
        for (i, s) in signs.into_iter().enumerate() {
            pair.a[i][i] = s * a;
            pair.b[i][i] = s * (offset + i128::try_from(i).unwrap() + 1);
        }
        pair
    };
    for (a, offset, signs, empty) in [
        (10, 70, [1, 1, 1, 1], true),
        (10, 70, [-1, -1, 1, 1], true),
        (10, 70, [1, -1, 1, -1], false),
        (-10, -71, [1, 1, 1, 1], true),
        (-10, -71, [1, 1, -1, -1], true),
        (-10, -71, [1, -1, 1, -1], false),
    ] {
        let (x, y) = diagonal(signs, a, offset).forms();
        let got = classify(&x, &y).unwrap();
        assert_eq!(got.segre, Segre::Simple);
        let want = if empty { NONE } else { smooth(2) };
        assert_eq!(got.topology, want, "{signs:?}");
    }
}

#[test]
fn certain_inertia_is_read_or_refused() {
    let form = |d: [i64; 4]| {
        let mut f = [[Interval::of(0).unwrap(); 4]; 4];
        for (i, v) in d.into_iter().enumerate() {
            f[i][i] = Interval::of(v).unwrap();
        }
        f
    };
    let inertia = |positive, negative| Inertia { positive, negative };
    assert_eq!(inertia_certain(&form([1, 2, 3, 4])), Some(inertia(4, 0)));
    assert_eq!(inertia_certain(&form([1, 2, -5, -7])), Some(inertia(2, 2)));
    // The trace is zero, and no interval proves that.
    assert_eq!(inertia_certain(&form([1, 1, -1, -1])), None);
}

#[test]
fn the_filter_gives_way_when_its_walk_cannot_part_the_roots() {
    let i = |v: i64| Interval::of(v).unwrap();
    // Roots at ±1 and ±(1 + 2^-40) of t, parted by no step of the walk.
    let mut a = [[i(0); 4]; 4];
    let mut b = [[i(0); 4]; 4];
    for (k, (x, y)) in [
        (1, 1),
        (-1, 1),
        ((1 << 40) + 1, 1 << 40),
        (-(1 << 40) - 1, 1 << 40),
    ]
    .into_iter()
    .enumerate()
    {
        a[k][k] = i(y);
        b[k][k] = i(-x);
    }
    assert_eq!(filtered(&a, &b), None);
}

#[test]
fn whole_numbers_divide_down() {
    type W = Wide<4>;
    let w = |v: i64| W::from_i64(v);
    assert_eq!(gcd(&w(0), &w(-7)), Some(w(7)));
    assert_eq!(gcd(&w(12), &w(0)), Some(w(12)));
    assert_eq!(gcd(&w(-12), &w(18)), Some(w(6)));
    assert_eq!(gcd(&w(1 << 40), &w(3 << 38)), Some(w(1 << 38)));
    assert_eq!(gcd(&w(35), &w(64)), Some(w(1)));
    assert_eq!(square_root(&w(-4)), None);
    assert_eq!(square_root(&w(0)), Some(w(0)));
    assert_eq!(square_root(&w(1)), Some(w(1)));
    assert_eq!(square_root(&w(144)), Some(w(12)));
    assert_eq!(square_root(&w(145)), None);
    let huge = w(1 << 62).checked_mul(&w(1 << 62)).unwrap();
    assert_eq!(square_root(&huge), Some(w(1 << 62)));
}
