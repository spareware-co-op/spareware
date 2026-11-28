//! Exact positions along an offset line, and the one place a value is rounded.
//!
//! A reference meets the offset line `origin + s * d` at a parameter `s` that is a fraction of
//! whole numbers: for a point, `s = ((q - origin) . d) / (d . d)`; for a line through `a` running
//! along `e`, `s = ((a - origin) x e) / (d x e)`. The distance between two of them is
//! `|s2 - s1| * sqrt(d . d)`, so its square is a fraction too. Everything is done on whole numbers
//! of any size (`num-bigint`, MIT OR Apache-2.0; fractions are construct-params' `Ratio`), so
//! nothing is rounded until [`round_root`] gives the nearest whole step to the square root, a half
//! rounding up, and says whether that root was whole. Why this is written here and not taken from a
//! library: the library's `Ratio` holds the fractions and `BigInt::sqrt` the floor of the root;
//! the choice of the nearest step and the exactness flag are the project's own rule.

#![allow(
    clippy::arithmetic_side_effects,
    reason = "big integers and fractions cannot overflow; every conversion back is checked"
)]

use construct_params::Ratio;
use num_bigint::BigInt;

/// A fraction of whole numbers of any size.
pub type Frac = Ratio<BigInt>;

fn big(value: i64) -> BigInt {
    BigInt::from(value)
}

/// The offset line: a point on it and the vector it runs along.
#[derive(Clone, Copy, Debug)]
pub struct Axis {
    /// A point of the line, in steps.
    pub origin: [i64; 2],
    /// The vector it runs along, in steps; not zero.
    pub d: [i64; 2],
}

impl Axis {
    /// `d . d`.
    #[must_use]
    pub fn dd(&self) -> BigInt {
        let [dx, dy] = self.d;
        big(dx) * big(dx) + big(dy) * big(dy)
    }

    /// The parameter where the foot of the perpendicular from `q` falls.
    #[must_use]
    pub fn of_point(&self, q: [i64; 2]) -> Frac {
        let [qx, qy] = q;
        let [ox, oy] = self.origin;
        let [dx, dy] = self.d;
        let num = (big(qx) - big(ox)) * big(dx) + (big(qy) - big(oy)) * big(dy);
        Frac::new(num, self.dd())
    }

    /// The parameter where the line through `a` running along `e` meets the line; `None` when
    /// they are parallel.
    #[must_use]
    pub fn of_line(&self, a: [i64; 2], e: [i64; 2]) -> Option<Frac> {
        let [ax, ay] = a;
        let [ox, oy] = self.origin;
        let [dx, dy] = self.d;
        let [ex, ey] = e;
        let denom = big(dx) * big(ey) - big(dy) * big(ex);
        if denom == BigInt::from(0) {
            return None;
        }
        let num = (big(ax) - big(ox)) * big(ey) - (big(ay) - big(oy)) * big(ex);
        Some(Frac::new(num, denom))
    }

    /// The point at parameter `s`, rounded to the nearest step in each coordinate (a half away
    /// from zero); `None` beyond 64 bits.
    #[must_use]
    pub fn point_at(&self, s: &Frac) -> Option<[i64; 2]> {
        let at = |origin: i64, d: i64| -> Option<i64> {
            let moved = Frac::from_integer(big(origin)) + s * Frac::from_integer(big(d));
            i64::try_from(&moved.round().to_integer()).ok()
        };
        Some([
            at(self.origin[0], self.d[0])?,
            at(self.origin[1], self.d[1])?,
        ])
    }

    /// The point halfway between parameters `a` and `b`, to the nearest step.
    #[must_use]
    pub fn point_between(&self, a: &Frac, b: &Frac) -> Option<[i64; 2]> {
        self.point_at(&((a + b) / Frac::from_integer(big(2))))
    }

    /// The square of the distance between parameters `a` and `b` along the line.
    #[must_use]
    pub fn squared_between(&self, a: &Frac, b: &Frac) -> Frac {
        let gap = b - a;
        &gap * &gap * Frac::from_integer(self.dd())
    }
}

/// The nearest whole number to the square root of `square` (a half rounds up), and whether the
/// root is that whole number exactly. `None` when it does not fit 64 bits.
#[must_use]
pub fn round_root(square: &Frac) -> Option<(i64, bool)> {
    let floor = square.floor().to_integer();
    let n = floor.sqrt();
    let whole = Frac::from_integer(&n * &n) == *square;
    let twice_up = &n * 2 + 1;
    let halfway = Frac::new(&twice_up * &twice_up, big(4));
    let k = if *square >= halfway { n + 1 } else { n };
    i64::try_from(&k).ok().map(|steps| (steps, whole))
}
