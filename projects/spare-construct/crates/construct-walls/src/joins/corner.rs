//! The joint through an L corner's square (note CON-A4-F2-P2, "An L"): which layer of which wall
//! takes each part of the square, as a path through the grid of the two walls' layer faces.
//!
//! Wall A comes into the square from one side and wall B from the other; each wall's layers are
//! numbered from the outside of the corner. The path runs from the square's outer corner (A's face
//! 0 crossing B's face 0) to its inner corner, visiting crossings `(i, j)` of A's face `i` and
//! B's face `j`. Each step ends one layer or a pair: [`Step::Down`] ends A's layer `i` on B's face
//! `j` (A's layer ran through B's layers inside `j`); [`Step::Left`] ends B's layer `j` on A's face
//! `i`; [`Step::Mitre`] ends both, A's layer `i` and B's layer `j` meeting on the line from
//! crossing `(i, j)` to `(i + 1, j + 1)`. So the two walls' parts of the square meet face to face
//! along the path, and tile it.
//!
//! The path is found strongest first ([`path`]): the strongest layer of each wall in play (the
//! outermost of equals) settle their meeting, and what lies outside and inside it is solved the
//! same way, so no layer ever crosses the joint of a stronger pair. Where the two are of the same
//! priority, the joint's kind settles it ([`Tie`]): a mitre meets on their crossing's diagonal, a
//! butt runs its first wall's layer through (job 6.1).

/// How two layers of the same priority meet at a corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tie {
    /// On the mitre of their crossing.
    Mitre,
    /// A's runs through, as if it were the stronger.
    A,
    /// B's runs through.
    B,
}

#[cfg(test)]
impl Tie {
    /// The same tie seen from the other wall.
    pub(super) fn swapped(self) -> Tie {
        match self {
            Tie::Mitre => Tie::Mitre,
            Tie::A => Tie::B,
            Tie::B => Tie::A,
        }
    }
}

/// One step of the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// A's layer `i` ends on B's face `j`.
    Down,
    /// B's layer `j` ends on A's face `i`.
    Left,
    /// A's layer `i` and B's layer `j` meet on the mitre of their crossing.
    Mitre,
}

/// The first of the strongest (the lowest rank) of `ranks` from `lo` to before `hi`.
fn strongest(ranks: &[u8], lo: usize, hi: usize) -> Option<(usize, u8)> {
    let mut best: Option<(usize, u8)> = None;
    for at in lo..hi {
        let rank = *ranks.get(at)?;
        if best.is_none_or(|(_, b)| rank < b) {
            best = Some((at, rank));
        }
    }
    best
}

/// The part of the square from crossing `(i0, j0)` to crossing `(i1, j1)`, as steps, each with
/// the crossing it starts from, pushed in order on `out`.
fn solve(
    ra: &[u8],
    rb: &[u8],
    tie: Tie,
    (i0, j0): (usize, usize),
    (i1, j1): (usize, usize),
    out: &mut Vec<(usize, usize, Step)>,
) -> Option<()> {
    let (Some((a, rank_a)), Some((b, rank_b))) = (strongest(ra, i0, i1), strongest(rb, j0, j1))
    else {
        // One wall has nothing left here: the other's layers run to its far side.
        for i in i0..i1 {
            out.push((i, j0, Step::Down));
        }
        for j in j0..j1 {
            out.push((i1, j, Step::Left));
        }
        return Some(());
    };
    let next = |v: usize| v.checked_add(1);
    let order = match (rank_a.cmp(&rank_b), tie) {
        (std::cmp::Ordering::Equal, Tie::A) => std::cmp::Ordering::Less,
        (std::cmp::Ordering::Equal, Tie::B) => std::cmp::Ordering::Greater,
        (order, _) => order,
    };
    match order {
        std::cmp::Ordering::Equal => {
            solve(ra, rb, tie, (i0, j0), (a, b), out)?;
            out.push((a, b, Step::Mitre));
            solve(ra, rb, tie, (next(a)?, next(b)?), (i1, j1), out)
        }
        std::cmp::Ordering::Less => {
            for i in i0..=a {
                out.push((i, j0, Step::Down));
            }
            solve(ra, rb, tie, (next(a)?, j0), (i1, j1), out)
        }
        std::cmp::Ordering::Greater => {
            for j in j0..=b {
                out.push((i0, j, Step::Left));
            }
            solve(ra, rb, tie, (i0, next(b)?), (i1, j1), out)
        }
    }
}

/// The path through the square of two walls whose layers, from the outside of the corner, have
/// the ranks `ra` (wall A) and `rb` (wall B), a lower rank stronger, ties met as `tie` says: each
/// step with the crossing `(i, j)` it starts from, from `(0, 0)` to `(ra.len(), rb.len())`.
pub(super) fn path(ra: &[u8], rb: &[u8], tie: Tie) -> Option<Vec<(usize, usize, Step)>> {
    let mut out = Vec::with_capacity(ra.len().checked_add(rb.len())?);
    solve(ra, rb, tie, (0, 0), (ra.len(), rb.len()), &mut out)?;
    Some(out)
}

/// Where each of A's layers ends, from the path: for layer `i`, the B faces its outer and its
/// inner face end on (`(j, j)` for a layer that runs through to B's face `j`, `(j, j + 1)` for one
/// on a mitre).
pub(super) fn ends_of_a(
    steps: &[(usize, usize, Step)],
    layers: usize,
) -> Option<Vec<(usize, usize)>> {
    let mut ends = vec![None; layers];
    for &(i, j, step) in steps {
        let end = match step {
            Step::Down => (j, j),
            Step::Mitre => (j, j.checked_add(1)?),
            Step::Left => continue,
        };
        *ends.get_mut(i)? = Some(end);
    }
    ends.into_iter().collect()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, reason = "tests")]
mod tests {
    use super::*;

    fn run(ra: &[u8], rb: &[u8]) -> Vec<(usize, usize, Step)> {
        path(ra, rb, Tie::Mitre).unwrap()
    }

    /// A butt runs the first wall's layers of each tie through: the same type both ways, A first,
    /// runs A's finish and core to B's outer face and stops A's lining on B's core, which B's
    /// core passes; B first, the mirror.
    #[test]
    fn a_butt_runs_the_first_wall_s_ties_through() {
        let same: &[u8] = &[4, 1, 4];
        let a_first = path(same, same, Tie::A).unwrap();
        assert_eq!(
            ends_of_a(&a_first, 3).unwrap(),
            vec![(0, 0), (0, 0), (2, 2)]
        );
        let b_first = path(same, same, Tie::B).unwrap();
        assert_eq!(
            ends_of_a(&b_first, 3).unwrap(),
            vec![(2, 2), (2, 2), (3, 3)]
        );
        // A stronger layer still runs through, whichever wall is first.
        let b_first = path(&[4, 1, 4], &[4, 3, 4], Tie::B).unwrap();
        assert_eq!(ends_of_a(&b_first, 3).unwrap()[1], (0, 0));
        // Swapping the walls and the tie mirrors the path.
        for tie in [Tie::Mitre, Tie::A, Tie::B] {
            let one = path(&[4, 1, 3], &[4, 1, 4], tie).unwrap();
            let other: Vec<_> = path(&[4, 1, 4], &[4, 1, 3], tie.swapped())
                .unwrap()
                .into_iter()
                .map(|(i, j, s)| {
                    let s = match s {
                        Step::Down => Step::Left,
                        Step::Left => Step::Down,
                        Step::Mitre => Step::Mitre,
                    };
                    (j, i, s)
                })
                .collect();
            assert_eq!(one, other, "{tie:?}");
        }
    }

    /// The same layers both ways mitre each pair.
    #[test]
    fn the_same_type_mitres_every_layer() {
        let steps = run(&[4, 3, 1, 4], &[4, 3, 1, 4]);
        assert_eq!(
            steps,
            vec![
                (0, 0, Step::Mitre),
                (1, 1, Step::Mitre),
                (2, 2, Step::Mitre),
                (3, 3, Step::Mitre)
            ]
        );
    }

    /// A core stronger than everything of the other wall runs through it, with what is outside
    /// it; the other's layers stop against it, and the inner finishes mitre.
    #[test]
    fn a_stronger_core_runs_through() {
        let steps = run(&[4, 1, 4], &[4, 3, 4]);
        assert_eq!(
            steps,
            vec![
                (0, 0, Step::Down),
                (1, 0, Step::Down),
                (2, 0, Step::Left),
                (2, 1, Step::Left),
                (2, 2, Step::Mitre)
            ]
        );
        assert_eq!(ends_of_a(&steps, 3).unwrap(), vec![(0, 0), (0, 0), (2, 3)]);
    }

    /// Swapping the walls mirrors the path: the rule does not depend on which is called A.
    #[test]
    fn swapping_the_walls_mirrors_the_path() {
        let cases: [(&[u8], &[u8]); 4] = [
            (&[4, 1, 4], &[4, 3, 4]),
            (&[4, 3, 1, 4], &[1, 4]),
            (&[255, 4, 2, 1, 1, 4], &[4, 1, 3, 255]),
            (&[1], &[4, 1, 4]),
        ];
        for (ra, rb) in cases {
            let mirror: Vec<_> = run(rb, ra)
                .into_iter()
                .map(|(i, j, s)| {
                    let s = match s {
                        Step::Down => Step::Left,
                        Step::Left => Step::Down,
                        Step::Mitre => Step::Mitre,
                    };
                    (j, i, s)
                })
                .collect();
            assert_eq!(run(ra, rb), mirror, "{ra:?} {rb:?}");
        }
    }

    /// Every path runs from the outer corner to the inner, one crossing at a time.
    #[test]
    fn every_path_is_a_walk_through_the_crossings() {
        let ranks: [&[u8]; 5] = [&[1], &[4, 1, 4], &[4, 3, 1, 4], &[255, 4, 255], &[1, 1, 2]];
        for ra in ranks {
            for rb in ranks {
                let mut at = (0, 0);
                for (i, j, step) in run(ra, rb) {
                    assert_eq!((i, j), at, "{ra:?} {rb:?}");
                    at = match step {
                        Step::Down => (i + 1, j),
                        Step::Left => (i, j + 1),
                        Step::Mitre => (i + 1, j + 1),
                    };
                }
                assert_eq!(at, (ra.len(), rb.len()), "{ra:?} {rb:?}");
            }
        }
    }
}
