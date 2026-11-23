//! Exact rational points in plan: where two wall faces cross need not be a grid point, so the
//! search for a room's loop works on exact rationals (dashu-ratio) and only its answer is asked to
//! be on the grid. This is the one module that does arithmetic on them; nothing here rounds.
//!
//! Every operation on [`RBig`] is exact and cannot overflow, so the crate's rule that every
//! fixed-width step is a checked method (clippy's `arithmetic_side_effects`) does not apply to
//! them; the allow below is for this module's rational operators alone.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rationals (dashu-ratio) cannot overflow; the lint guards fixed-width numbers"
)]

use dashu_int::IBig;
use dashu_ratio::RBig;
use std::cmp::Ordering;

/// An exact point in plan, in grid steps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Pt {
    pub(crate) x: RBig,
    pub(crate) y: RBig,
}

/// A whole number of steps as a rational.
pub(crate) fn whole(n: i64) -> RBig {
    RBig::from(IBig::from(n))
}

impl Pt {
    /// The grid point `[x, y]`.
    pub(crate) fn grid(at: [i64; 2]) -> Self {
        let [x, y] = at;
        Self {
            x: whole(x),
            y: whole(y),
        }
    }

    /// `self - other`, as a vector.
    pub(crate) fn less(&self, other: &Self) -> Self {
        Self {
            x: &self.x - &other.x,
            y: &self.y - &other.y,
        }
    }

    /// The point `t` of the way from `self` to `to`.
    pub(crate) fn toward(&self, to: &Self, t: &RBig) -> Self {
        Self {
            x: &self.x + (&to.x - &self.x) * t,
            y: &self.y + (&to.y - &self.y) * t,
        }
    }

    /// Halfway from `self` to `to`.
    pub(crate) fn mid(&self, to: &Self) -> Self {
        let two = whole(2);
        Self {
            x: (&self.x + &to.x) / &two,
            y: (&self.y + &to.y) / &two,
        }
    }

    /// The grid point it is, if it is one.
    pub(crate) fn on_grid(&self) -> Option<[i64; 2]> {
        let whole = |v: &RBig| {
            v.denominator()
                .is_one()
                .then(|| i64::try_from(v.numerator()).ok())
                .flatten()
        };
        Some([whole(&self.x)?, whole(&self.y)?])
    }
}

/// `a x b`, the cross product of two plan vectors.
pub(crate) fn cross(a: &Pt, b: &Pt) -> RBig {
    &a.x * &b.y - &a.y * &b.x
}

/// `a . b`, the dot product of two plan vectors.
pub(crate) fn dot(a: &Pt, b: &Pt) -> RBig {
    &a.x * &b.x + &a.y * &b.y
}

/// The sign of a rational, as an ordering against zero.
pub(crate) fn sign(v: &RBig) -> Ordering {
    v.cmp(&RBig::ZERO)
}

/// Where the segments `from`-`to` and `start`-`end` meet, as fractions of the way along
/// `from`-`to`: one value where they cross or touch at a point, the two ends of their overlap
/// where they are collinear and overlap, nothing where they do not meet. Ends included.
pub(crate) fn meeting(from: &Pt, to: &Pt, start: &Pt, end: &Pt) -> Vec<RBig> {
    let along = to.less(from);
    let other = end.less(start);
    let gap = start.less(from);
    let turn = cross(&along, &other);
    let squared = dot(&along, &along);
    if sign(&turn) == Ordering::Equal {
        if sign(&cross(&gap, &along)) != Ordering::Equal || sign(&squared) == Ordering::Equal {
            return Vec::new();
        }
        // Collinear: where the other's two ends lie along this one, kept within it.
        let first = dot(&gap, &along) / &squared;
        let second = dot(&end.less(from), &along) / &squared;
        let (low, high) = if first <= second {
            (first, second)
        } else {
            (second, first)
        };
        let low = if low < RBig::ZERO { RBig::ZERO } else { low };
        let high = if high > RBig::ONE { RBig::ONE } else { high };
        return match low.cmp(&high) {
            Ordering::Less => vec![low, high],
            Ordering::Equal => vec![low],
            Ordering::Greater => Vec::new(),
        };
    }
    let here = cross(&gap, &other) / &turn;
    let there = cross(&gap, &along) / &turn;
    let within = |v: &RBig| *v >= RBig::ZERO && *v <= RBig::ONE;
    if within(&here) && within(&there) {
        vec![here]
    } else {
        Vec::new()
    }
}

/// Twice the signed area of a closed loop of points: above zero anticlockwise.
pub(crate) fn twice_area(ring: &[Pt]) -> RBig {
    let mut total = RBig::ZERO;
    for (at, a) in ring.iter().enumerate() {
        let b = ring.get(at.saturating_add(1)).or_else(|| ring.first());
        if let Some(b) = b {
            total += cross(a, b);
        }
    }
    total
}

/// Where a point is against a closed loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    /// Strictly inside.
    Inside,
    /// Strictly outside.
    Outside,
    /// On the loop's edge from corner `k` to corner `k + 1`.
    On(usize),
}

/// Where `p` is against the closed loop `ring` (either way round), exactly: on an edge, else by
/// the count of edges a ray to `+x` from `p` crosses.
pub(crate) fn side(p: &Pt, ring: &[Pt]) -> Side {
    let mut inside = false;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get(k.saturating_add(1)).or_else(|| ring.first()) else {
            continue;
        };
        let ab = b.less(a);
        let ap = p.less(a);
        let turn = cross(&ab, &ap);
        if sign(&turn) == Ordering::Equal {
            let along = dot(&ap, &ab);
            if along >= RBig::ZERO && along <= dot(&ab, &ab) {
                return Side::On(k);
            }
        }
        if (a.y > p.y) != (b.y > p.y) && (b.y > a.y) == (sign(&turn) == Ordering::Greater) {
            inside = !inside;
        }
    }
    if inside { Side::Inside } else { Side::Outside }
}

/// Which part of a turn anticlockwise from the direction `from` the direction `to` is: 0 for a
/// turn in `(0, pi]`, 1 for `(pi, 2 pi)`, 2 for none (straight along `from`, which comes last).
fn half(from: &Pt, to: &Pt) -> u8 {
    match (sign(&cross(from, to)), sign(&dot(from, to))) {
        (Ordering::Greater, _) | (Ordering::Equal, Ordering::Less) => 0,
        (Ordering::Less, _) => 1,
        (Ordering::Equal, _) => 2,
    }
}

/// Whether, turning anticlockwise from the direction `from`, the direction `a` is reached
/// strictly before `b`: the angles compared in `(0, 2 pi]`, exactly.
pub(crate) fn sooner(from: &Pt, a: &Pt, b: &Pt) -> bool {
    match half(from, a).cmp(&half(from, b)) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => sign(&cross(a, b)) == Ordering::Greater,
    }
}

/// The rational `n / d` for whole `n` and `d` (with `d` not zero).
#[cfg(test)]
pub(crate) fn ratio(n: i64, d: i64) -> RBig {
    whole(n) / whole(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_segments_meet_once_and_collinear_ones_at_their_overlap() {
        let p = |x, y| Pt::grid([x, y]);
        assert_eq!(
            meeting(&p(0, 0), &p(4, 0), &p(1, -1), &p(1, 1)),
            vec![ratio(1, 4)]
        );
        assert_eq!(
            meeting(&p(0, 0), &p(4, 0), &p(1, 0), &p(6, 0)),
            vec![ratio(1, 4), RBig::ONE]
        );
        assert_eq!(
            meeting(&p(0, 0), &p(4, 0), &p(0, 1), &p(4, 1)),
            Vec::<RBig>::new()
        );
        assert_eq!(
            meeting(&p(0, 0), &p(4, 0), &p(4, 0), &p(4, 3)),
            vec![RBig::ONE]
        );
    }

    #[test]
    fn a_point_is_inside_outside_or_on_a_loop() {
        let square: Vec<Pt> = [[0, 0], [4, 0], [4, 4], [0, 4]]
            .into_iter()
            .map(Pt::grid)
            .collect();
        assert_eq!(side(&Pt::grid([1, 1]), &square), Side::Inside);
        assert_eq!(side(&Pt::grid([5, 1]), &square), Side::Outside);
        assert_eq!(side(&Pt::grid([4, 2]), &square), Side::On(1));
        assert_eq!(twice_area(&square), whole(32));
        let half = Pt {
            x: ratio(1, 2),
            y: ratio(7, 2),
        };
        assert_eq!(side(&half, &square), Side::Inside);
        assert_eq!(half.on_grid(), None);
        assert_eq!(Pt::grid([3, -2]).on_grid(), Some([3, -2]));
    }
}
