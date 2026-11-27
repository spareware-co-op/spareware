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
//! same way, so no layer ever crosses the joint of a stronger pair.

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
    match rank_a.cmp(&rank_b) {
        std::cmp::Ordering::Equal => {
            solve(ra, rb, (i0, j0), (a, b), out)?;
            out.push((a, b, Step::Mitre));
            solve(ra, rb, (next(a)?, next(b)?), (i1, j1), out)
        }
        std::cmp::Ordering::Less => {
            for i in i0..=a {
                out.push((i, j0, Step::Down));
            }
            solve(ra, rb, (next(a)?, j0), (i1, j1), out)
        }
        std::cmp::Ordering::Greater => {
            for j in j0..=b {
                out.push((i0, j, Step::Left));
            }
            solve(ra, rb, (i0, next(b)?), (i1, j1), out)
        }
    }
}

/// The path through the square of two walls whose layers, from the outside of the corner, have
/// the ranks `ra` (wall A) and `rb` (wall B), a lower rank stronger: each step with the crossing
/// `(i, j)` it starts from, from `(0, 0)` to `(ra.len(), rb.len())`.
pub(super) fn path(ra: &[u8], rb: &[u8]) -> Option<Vec<(usize, usize, Step)>> {
    let mut out = Vec::with_capacity(ra.len().checked_add(rb.len())?);
    solve(ra, rb, (0, 0), (ra.len(), rb.len()), &mut out)?;
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
        path(ra, rb).unwrap()
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
