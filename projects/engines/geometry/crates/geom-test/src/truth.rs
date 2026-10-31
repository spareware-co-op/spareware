//! The closed-form truth corpus: shapes whose answers are known in closed form, computed once in
//! arbitrary precision, to hold the Certified paths to their bounds (chapter 28.9; piece G179).
//!
//! What it does: an [`Enclosed`] is a real number held between two binary fractions of [`BITS`]
//! bits, in exact rational arithmetic that rounds outward. [`pi`], [`sinpi`] and [`cospi`] and
//! [`fresnel`] are computed from their series with their remainders bounded, and the closed forms
//! are built on them: [`box_volume`], [`steinmetz`], [`sphere`], [`sphere_cap`], [`torus`],
//! [`pappus`], [`capsule`] and [`villarceau`], the last a grid point on a circle a plane through a
//! torus's centre cuts from it. [`judge`] holds one certified result to one truth: each result must
//! contain the truth, and a bound more than [`LOOSE`] times the real error is reported. [`CORPUS`]
//! lists every family, what checks it today and the board piece that brings the operation still
//! missing; `tests/truth.rs` runs what exists, so the corpus grows with each curved family.
//!
//! Guarantees:
//!
//! - **Shares nothing with the kernel.** Every value comes from rational arithmetic on dashu's big
//!   integers here, from the definitions; no kernel number type and no double takes part.
//! - **The truth is enclosed, never approximated.** Every step rounds outward and every series
//!   carries its remainder, so an [`Enclosed`] holds its exact value, and [`Enclosed::width`] says
//!   how tightly: below 2^-280 for every value in the corpus's range.
//! - **A broken bound is named.** [`judge`] says [`Verdict::Broken`] whenever a result does not
//!   hold the whole truth, and never rounds in the result's favour.
//!
//! Method: chapter 28.9's closed-form truth
//! (`projects/engines/geometry/architecture/28-testing.md`); π by Machin's formula
//! (W. Jones, *Synopsis Palmariorum Matheseos*, 1706), sine and cosine by their Taylor series, and
//! the Fresnel integrals by their term-by-term integrated series (DLMF 7.6.4, 7.6.6,
//! <https://dlmf.nist.gov/7.6>), each alternating with terms that fall, so the first term left out
//! bounds the remainder.
//!
//! Cost: a few thousand big-number operations a value; the rationals are trimmed to [`BITS`] bits
//! at every step, so they do not grow. Recursion: none.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision numbers, which cannot, and every division is by a value shown nonzero beside
// it (a power of two, a count, a positive length).
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact rational arithmetic, which cannot overflow; divisors are shown nonzero"
)]

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use geom_num::interval::Interval;

/// The bits after the binary point every enclosure keeps.
pub const BITS: usize = 320;

/// How many times the real error a bound may be before it is reported (28.9).
pub const LOOSE: u32 = 1000;

/// `2^-BITS`, as a rational.
fn step() -> RBig {
    RBig::from_parts(IBig::ONE, UBig::ONE << BITS)
}

/// Whether a series term of size `x` is small enough to stop at. Rounding outward holds a falling
/// term at a step or two, never below, so the test is against the place it settles at; the term
/// then bounds what is left, as for any term.
fn spent(x: &RBig) -> bool {
    *x <= step() * whole(4)
}

/// A whole number as a rational.
fn whole(n: u64) -> RBig {
    RBig::from(UBig::from(n))
}

/// `x` rounded down to a multiple of `2^-BITS`.
fn down(x: &RBig) -> RBig {
    let scale = UBig::ONE << BITS;
    let n = x.numerator() * IBig::from(scale.clone());
    let d = IBig::from(x.denominator().clone());
    // The denominator is positive, and / truncates toward zero, so it rounds up below zero.
    let mut q = &n / &d;
    if &q * &d > n {
        q -= IBig::ONE;
    }
    RBig::from_parts(q, scale)
}

/// `x` rounded up to a multiple of `2^-BITS`.
fn up(x: &RBig) -> RBig {
    -down(&-x.clone())
}

/// A real number between two binary fractions, in arithmetic that rounds outward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enclosed {
    lo: RBig,
    hi: RBig,
}

impl Enclosed {
    /// The exact value `x`, rounded outward.
    #[must_use]
    pub fn of(x: &RBig) -> Enclosed {
        Enclosed {
            lo: down(x),
            hi: up(x),
        }
    }

    /// The whole number `n`.
    #[must_use]
    pub fn from_whole(n: u64) -> Enclosed {
        Enclosed::of(&whole(n))
    }

    /// The double `x`, exactly; `None` if it is not finite.
    #[must_use]
    pub fn from_double(x: f64) -> Option<Enclosed> {
        RBig::try_from(x).ok().map(|r| Enclosed::of(&r))
    }

    /// The lower end.
    #[must_use]
    pub const fn lo(&self) -> &RBig {
        &self.lo
    }

    /// The upper end.
    #[must_use]
    pub const fn hi(&self) -> &RBig {
        &self.hi
    }

    /// `hi − lo`.
    #[must_use]
    pub fn width(&self) -> RBig {
        &self.hi - &self.lo
    }

    /// `self + other`.
    #[must_use]
    pub fn add(&self, other: &Enclosed) -> Enclosed {
        Enclosed {
            lo: down(&(&self.lo + &other.lo)),
            hi: up(&(&self.hi + &other.hi)),
        }
    }

    /// `−self`.
    #[must_use]
    pub fn neg(&self) -> Enclosed {
        Enclosed {
            lo: -self.hi.clone(),
            hi: -self.lo.clone(),
        }
    }

    /// `self − other`.
    #[must_use]
    pub fn sub(&self, other: &Enclosed) -> Enclosed {
        self.add(&other.neg())
    }

    /// `self × other`.
    #[must_use]
    pub fn mul(&self, other: &Enclosed) -> Enclosed {
        let corners = [
            &self.lo * &other.lo,
            &self.lo * &other.hi,
            &self.hi * &other.lo,
            &self.hi * &other.hi,
        ];
        let least = corners.iter().min().cloned().unwrap_or(RBig::ZERO);
        let most = corners.iter().max().cloned().unwrap_or(RBig::ZERO);
        Enclosed {
            lo: down(&least),
            hi: up(&most),
        }
    }

    /// `self / n`, for a whole number above zero.
    #[must_use]
    pub fn div_whole(&self, n: u64) -> Enclosed {
        let d = whole(n.max(1));
        Enclosed {
            lo: down(&(&self.lo / &d)),
            hi: up(&(&self.hi / &d)),
        }
    }

    /// `self²`.
    #[must_use]
    pub fn square(&self) -> Enclosed {
        self.mul(self)
    }

    /// `self` widened by `r` each way.
    fn plus_or_minus(&self, r: &RBig) -> Enclosed {
        Enclosed {
            lo: down(&(&self.lo - r)),
            hi: up(&(&self.hi + r)),
        }
    }

    /// The largest size of a value in `self`.
    fn size(&self) -> RBig {
        let abs = |x: &RBig| {
            if *x < RBig::ZERO {
                -x.clone()
            } else {
                x.clone()
            }
        };
        abs(&self.lo).max(abs(&self.hi))
    }

    /// Whether `self` holds `x`.
    #[must_use]
    pub fn holds(&self, x: &RBig) -> bool {
        self.lo <= *x && *x <= self.hi
    }
}

/// atan(1/x), for a whole x above 1.
fn atan_inverse(x: u64) -> Enclosed {
    let square = whole(x) * whole(x);
    let mut power = RBig::ONE / whole(x);
    let mut sum = Enclosed::of(&RBig::ZERO);
    let mut k = 0_u64;
    loop {
        let term = Enclosed::of(&power).div_whole(2 * k + 1);
        sum = if k % 2 == 0 {
            sum.add(&term)
        } else {
            sum.sub(&term)
        };
        power = up(&(&power / &square));
        k += 1;
        if spent(&power) {
            // The series alternates with falling terms: the next one bounds what is left.
            return sum.plus_or_minus(&power);
        }
    }
}

/// π, by Machin's formula: 16 atan(1/5) − 4 atan(1/239).
#[must_use]
pub fn pi() -> Enclosed {
    let fifth = atan_inverse(5).mul(&Enclosed::from_whole(16));
    let far = atan_inverse(239).mul(&Enclosed::from_whole(4));
    fifth.sub(&far)
}

/// cos x if `even`, sin x otherwise, by the Taylor series: terms xᵏ/k! with k of one parity,
/// alternating and falling once k passes |x|.
fn circular(x: &Enclosed, even: bool) -> Enclosed {
    let square = x.square();
    let mut term = if even {
        Enclosed::from_whole(1)
    } else {
        x.clone()
    };
    let mut k = u64::from(!even);
    let mut sum = term.clone();
    let mut negative = false;
    loop {
        term = term.mul(&square).div_whole((k + 1) * (k + 2));
        k += 2;
        negative = !negative;
        sum = if negative {
            sum.sub(&term)
        } else {
            sum.add(&term)
        };
        if spent(&term.size()) && whole(k) > x.size() {
            return sum.plus_or_minus(&term.size());
        }
    }
}

/// sin(π·q), for a rational `q`.
#[must_use]
pub fn sinpi(q: &RBig) -> Enclosed {
    circular(&pi().mul(&Enclosed::of(q)), false)
}

/// cos(π·q), for a rational `q`.
#[must_use]
pub fn cospi(q: &RBig) -> Enclosed {
    circular(&pi().mul(&Enclosed::of(q)), true)
}

/// ∫₀ˢ cos(k·u²) du and ∫₀ˢ sin(k·u²) du: the clothoid of curvature 2k·u from a straight start,
/// and the Fresnel integrals C(s) and S(s) at k = π/2. Termwise, Σ (−1)ⁿ k²ⁿ s⁴ⁿ⁺¹ / ((2n)!(4n+1))
/// and Σ (−1)ⁿ k²ⁿ⁺¹ s⁴ⁿ⁺³ / ((2n+1)!(4n+3)).
#[must_use]
pub fn fresnel(k: &Enclosed, s: &RBig) -> [Enclosed; 2] {
    let s = Enclosed::of(s);
    let rate = k.mul(&s.square());
    // k^order s^(2 order + 1) / order!, stepped as one number: its power of s and its factorial,
    // apart, would pass and fall below what an enclosure holds.
    let mut part = s;
    let mut sums = [Enclosed::of(&RBig::ZERO), Enclosed::of(&RBig::ZERO)];
    let mut order = 0_u64;
    loop {
        // The term part / (2 order + 1), to C for an even order and to S for an odd one, its sign
        // changing every second order.
        let term = part.div_whole(2 * order + 1);
        let at = usize::from(order % 2 == 1);
        let negative = order % 4 >= 2;
        let sum = sums.get(at).cloned().unwrap_or_else(|| term.clone());
        let next = if negative {
            sum.sub(&term)
        } else {
            sum.add(&term)
        };
        if let Some(slot) = sums.get_mut(at) {
            *slot = next;
        }
        order += 1;
        part = part.mul(&rate).div_whole(order);
        if spent(&term.size()) && whole(order) > rate.size() {
            let left = term.size();
            return sums.map(|sum| sum.plus_or_minus(&left));
        }
    }
}

/// The volume of a box of sides `a`, `b` and `c`.
#[must_use]
pub fn box_volume(a: &RBig, b: &RBig, c: &RBig) -> RBig {
    a * b * c
}

/// The volume of the intersection of two cylinders of radius `r` crossing at right angles, the
/// Steinmetz solid: 16r³/3.
#[must_use]
pub fn steinmetz(r: &RBig) -> RBig {
    r * r * r * whole(16) / whole(3)
}

/// The volume of a ball of radius `r`: 4πr³/3.
#[must_use]
pub fn sphere(r: &RBig) -> Enclosed {
    pi().mul(&Enclosed::of(&(r * r * r * whole(4))))
        .div_whole(3)
}

/// The volume of the cap of height `h` of a ball of radius `r`: πh²(3r − h)/3.
#[must_use]
pub fn sphere_cap(r: &RBig, h: &RBig) -> Enclosed {
    let rest = h * h * (whole(3) * r - h);
    pi().mul(&Enclosed::of(&rest)).div_whole(3)
}

/// The volume of a ring torus of radii `major` and `minor`: 2π²Rr².
#[must_use]
pub fn torus(major: &RBig, minor: &RBig) -> Enclosed {
    let rest = whole(2) * major * minor * minor;
    pi().square().mul(&Enclosed::of(&rest))
}

/// The volume Pappus's theorem gives a plane region of `area` revolved a full turn about an axis
/// `distance` from its centroid: 2π·distance·area.
#[must_use]
pub fn pappus(area: &Enclosed, distance: &RBig) -> Enclosed {
    pi().mul(area).mul(&Enclosed::of(&(whole(2) * distance)))
}

/// The volume of a ball of radius `r` swept along a line of length `length`: πr²·length + 4πr³/3.
#[must_use]
pub fn capsule(r: &RBig, length: &RBig) -> Enclosed {
    pi().mul(&Enclosed::of(&(r * r * length))).add(&sphere(r))
}

/// A grid point of a Villarceau circle of the torus of major radius 25·`unit` and minor radius
/// 15·`unit` about the z axis: the plane through its centre at slope 3/4 touches it twice and cuts
/// it in circles of radius 25·unit about (0, ±15·unit, 0); this one is at the angle with cosine 3/5.
#[must_use]
pub fn villarceau(unit: i64) -> [i64; 3] {
    [
        unit.saturating_mul(12),
        unit.saturating_mul(35),
        unit.saturating_mul(9),
    ]
}

/// What a certified result came to against its truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It holds the truth, within [`LOOSE`] times the real error.
    Within,
    /// It holds the truth, with a bound more than [`LOOSE`] times the real error.
    Loose,
    /// It does not hold the truth: a correctness bug.
    Broken,
}

/// `claim` against `truth`. A result near a double's own rounding is not loose: the real error is
/// taken to be at least one unit in the last place of the truth.
#[must_use]
pub fn judge(claim: Interval, truth: &Enclosed) -> Verdict {
    let (Ok(lo), Ok(hi)) = (RBig::try_from(claim.lo()), RBig::try_from(claim.hi())) else {
        return Verdict::Broken;
    };
    if lo > *truth.lo() || hi < *truth.hi() {
        return Verdict::Broken;
    }
    let middle = (&lo + &hi) / whole(2);
    let bound = (&hi - &lo) / whole(2);
    let error = Enclosed::of(&middle).sub(truth).size();
    let unit = truth.size() / whole(1 << 52);
    if bound > error.max(unit) * whole(u64::from(LOOSE)) {
        Verdict::Loose
    } else {
        Verdict::Within
    }
}

/// What checks one family of the corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// `tests/truth.rs` holds the kernel to it today, by the test named.
    Checked(&'static str),
    /// The truth is here, and its operation comes with this board piece.
    Awaits(&'static str),
}

/// One family of the corpus and what checks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Family {
    /// Its name.
    pub name: &'static str,
    /// Its closed form, in words.
    pub form: &'static str,
    /// What checks it.
    pub status: Status,
}

/// Chapter 28.9's families.
pub const CORPUS: [Family; 10] = [
    Family {
        name: "box",
        form: "abc, exactly",
        status: Status::Checked("boxes_are_exact"),
    },
    Family {
        name: "implicit corners",
        form: "a rational volume held by a certified interval",
        status: Status::Checked("a_cut_corner_is_held_by_its_interval"),
    },
    Family {
        name: "analytic surface points",
        form: "cosines and sines of rational half-turns",
        status: Status::Checked("surface_points_hold_their_closed_forms"),
    },
    Family {
        name: "clothoid",
        form: "Fresnel integrals",
        status: Status::Checked("clothoid_points_hold_their_fresnel_integrals"),
    },
    Family {
        name: "Steinmetz solid",
        form: "16r³/3",
        status: Status::Awaits("G247"),
    },
    Family {
        name: "sphere cap",
        form: "πh²(3r − h)/3",
        status: Status::Awaits("G247"),
    },
    Family {
        name: "torus",
        form: "2π²Rr²",
        status: Status::Awaits("G247"),
    },
    Family {
        name: "revolve",
        form: "Pappus: 2π·distance·area",
        status: Status::Awaits("G255"),
    },
    Family {
        name: "Villarceau circles",
        form: "a plane through a torus's centre at sin θ = r/R cuts two circles of radius R",
        status: Status::Awaits("G167"),
    },
    Family {
        name: "capsule",
        form: "πr²L + 4πr³/3",
        status: Status::Awaits("G256"),
    },
];
