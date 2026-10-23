//! The measured code: one function per operation, which runs the operation on every input of a
//! batch and stores each result.
//!
//! Every harness calls exactly these functions through [`crate::harness::Bench`], and the golden
//! digests are taken from the results they store, so the code whose speed is gated is the code whose
//! output is checked: no benchmark can get faster by becoming wrong without failing its golden digest
//! (chapter 23.13, "Output hash").
//!
//! How a kernel is written, and why:
//!
//! - **One call per input, in order.** Each input passes through [`black_box`], so the compiler can
//!   neither precompute a result nor merge or vectorise calls across inputs: the count is the cost
//!   of the operation called once per input.
//! - **Every result is stored**, in a buffer the caller owns and that already has room for the whole
//!   batch, so nothing can be optimised away and nothing is allocated or freed. Hashing the results
//!   happens later, outside the measured run.
//! - **Never inlined.** Each kernel is `#[inline(never)]`, so the warm-up pass and the measured
//!   pass execute the same machine code (see [`crate::harness`]), and a change in one kernel cannot
//!   change how another is compiled.
//!
//! The loop adds a few instructions per input to every count: load the input, pass it through
//! `black_box`, store the result, step the loop. They are the same before and after a change, so they
//! dilute a regression rather than hide it: a change that costs x% of the operation alone shows as
//! x · op / (op + loop) percent. In WebAssembly fuel the loop costs about 27 to 33 per input, so
//! the 5% gate trips at a rise of about 46% in interval negation alone and 5.3% in `orient2d`; the
//! crate documentation lists the cheapest operations and how the loop was measured.
//!
//! Guarantees: each kernel calls exactly the `geom-num` function it is named after, once per input
//! and in order, and stores every result (`tests/attack_g06.rs` recomputes each benchmark's results
//! by calling `geom-num` directly); it allocates nothing when the buffer has room for the batch,
//! as every harness gives it.
//!
//! Method: the batch-of-independent-calls microbenchmark with an optimisation barrier per input, as
//! divan documents it (<https://docs.rs/divan/0.1.21/divan/fn.black_box.html>), with the results
//! written to memory instead of folded into a checksum.
//!
//! Cost: one call of the operation per input, plus the loop above; no allocation, because the
//! buffer already holds one result per input.

use core::hint::black_box;

/// Runs `op` on every input in order, replacing the contents of `out` with the results. `out`
/// already has room for the batch (the caller made it so), so nothing is allocated.
#[inline(always)]
fn run<T, R>(batch: &[T], out: &mut Vec<R>, mut op: impl FnMut(&T) -> R) {
    out.clear();
    out.extend(batch.iter().map(|input| op(black_box(input))));
}

/// The exact predicates of `geom_num::predicate`. Each returns its sign, or the error it reports
/// (`BudgetExceeded`, which no grid input can cause), and the kernel stores whichever it returns.
pub mod predicate {
    use geom_num::grid::{P2, P3};
    use geom_num::predicate as p;
    use geom_num::{NumError, Sign};

    use super::run;

    /// `orient2d` on each triple.
    #[inline(never)]
    pub fn orient2d(batch: &[[P2; 3]], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |&[a, b, c]| p::orient2d(a, b, c));
    }

    /// `orient3d` on each quadruple.
    #[inline(never)]
    pub fn orient3d(batch: &[[P3; 4]], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |&[a, b, c, d]| p::orient3d(a, b, c, d));
    }

    /// `incircle` on each quadruple.
    #[inline(never)]
    pub fn incircle(batch: &[[P2; 4]], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |&[a, b, c, d]| p::incircle(a, b, c, d));
    }

    /// `insphere` on each quintuple.
    #[inline(never)]
    pub fn insphere(batch: &[[P3; 5]], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |&[a, b, c, d, e]| p::insphere(a, b, c, d, e));
    }
}

/// Signs of exact integers: `geom_num::sign`.
pub mod sign {
    use geom_num::Sign;

    use super::run;

    /// `Sign::of_i128` on each value.
    #[inline(never)]
    pub fn of_i128(batch: &[i128], out: &mut Vec<Sign>) {
        run(batch, out, |&v| Sign::of_i128(v));
    }
}

/// Checked arithmetic, comparison and conversion on `geom_num::wide::Wide<L>`, for any width. The
/// catalogue instantiates each at the widths the kernel uses (I256, I320, I384 and I512) and, for the
/// growth checks, at 2, 4 and 8 limbs.
pub mod wide {
    use core::cmp::Ordering;

    use geom_num::Sign;
    use geom_num::wide::Wide;

    use super::run;

    /// `checked_add` on each pair.
    #[inline(never)]
    pub fn add<const L: usize>(batch: &[(Wide<L>, Wide<L>)], out: &mut Vec<Option<Wide<L>>>) {
        run(batch, out, |(x, y)| x.checked_add(y));
    }

    /// `checked_sub` on each pair.
    #[inline(never)]
    pub fn sub<const L: usize>(batch: &[(Wide<L>, Wide<L>)], out: &mut Vec<Option<Wide<L>>>) {
        run(batch, out, |(x, y)| x.checked_sub(y));
    }

    /// `checked_mul` on each pair.
    #[inline(never)]
    pub fn mul<const L: usize>(batch: &[(Wide<L>, Wide<L>)], out: &mut Vec<Option<Wide<L>>>) {
        run(batch, out, |(x, y)| x.checked_mul(y));
    }

    /// `checked_neg` on each value.
    #[inline(never)]
    pub fn neg<const L: usize>(batch: &[Wide<L>], out: &mut Vec<Option<Wide<L>>>) {
        run(batch, out, Wide::checked_neg);
    }

    /// `Ord::cmp` on each pair.
    #[inline(never)]
    pub fn cmp<const L: usize>(batch: &[(Wide<L>, Wide<L>)], out: &mut Vec<Ordering>) {
        run(batch, out, |(x, y)| x.cmp(y));
    }

    /// `signum` on each value.
    #[inline(never)]
    pub fn signum<const L: usize>(batch: &[Wide<L>], out: &mut Vec<Sign>) {
        run(batch, out, Wide::signum);
    }

    /// `to_i128` on each value.
    #[inline(never)]
    pub fn to_i128<const L: usize>(batch: &[Wide<L>], out: &mut Vec<Option<i128>>) {
        run(batch, out, Wide::to_i128);
    }

    /// `from_i128` on each value.
    #[inline(never)]
    pub fn from_i128<const L: usize>(batch: &[i128], out: &mut Vec<Wide<L>>) {
        run(batch, out, |&v| Wide::from_i128(v));
    }

    /// `resize` from `Wide<L>` to `Wide<M>` on each value.
    #[inline(never)]
    pub fn resize<const L: usize, const M: usize>(
        batch: &[Wide<L>],
        out: &mut Vec<Option<Wide<M>>>,
    ) {
        run(batch, out, Wide::resize::<M>);
    }
}

/// Plane-based geometry: `geom_num::plane`.
pub mod plane {
    use geom_num::grid::{P3, Rounded};
    use geom_num::plane::{Plane, Tpi};
    use geom_num::wide::{I256, I320};
    use geom_num::{NumError, Sign};

    use super::run;

    /// `Plane::through` on each triple.
    #[inline(never)]
    pub fn through(batch: &[[P3; 3]], out: &mut Vec<Result<Plane, NumError>>) {
        run(batch, out, |&[a, b, c]| Plane::through(a, b, c));
    }

    /// `Plane::side` on each (plane, point) pair.
    #[inline(never)]
    pub fn side(batch: &[(Plane, P3)], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |(p, q)| p.side(*q));
    }

    /// `Tpi::new` on each plane triple.
    #[inline(never)]
    pub fn tpi_new(batch: &[[Plane; 3]], out: &mut Vec<Result<Tpi, NumError>>) {
        run(batch, out, |&[p, q, r]| Tpi::new(p, q, r));
    }

    /// `Tpi::homogeneous` on each meeting point.
    #[inline(never)]
    pub fn tpi_homogeneous(batch: &[Tpi], out: &mut Vec<Result<([I320; 3], I256), NumError>>) {
        run(batch, out, Tpi::homogeneous);
    }

    /// `Plane::side_of_tpi` on each (plane, meeting point) pair.
    #[inline(never)]
    pub fn side_of_tpi(batch: &[(Plane, Tpi)], out: &mut Vec<Result<Sign, NumError>>) {
        run(batch, out, |(p, t)| p.side_of_tpi(t));
    }

    /// `Tpi::to_grid` on each meeting point.
    #[inline(never)]
    pub fn to_grid(batch: &[Tpi], out: &mut Vec<Result<Rounded<P3>, NumError>>) {
        run(batch, out, Tpi::to_grid);
    }
}

/// Error-free transformations: `geom_num::eft`.
pub mod eft {
    use geom_num::eft as e;

    use super::run;

    /// `two_sum` on each pair.
    #[inline(never)]
    pub fn two_sum(batch: &[(f64, f64)], out: &mut Vec<(f64, f64)>) {
        run(batch, out, |&(a, b)| e::two_sum(a, b));
    }

    /// `fast_two_sum` on each pair.
    #[inline(never)]
    pub fn fast_two_sum(batch: &[(f64, f64)], out: &mut Vec<(f64, f64)>) {
        run(batch, out, |&(a, b)| e::fast_two_sum(a, b));
    }

    /// `split` on each value.
    #[inline(never)]
    pub fn split(batch: &[f64], out: &mut Vec<(f64, f64)>) {
        run(batch, out, |&a| e::split(a));
    }

    /// `two_prod` on each pair.
    #[inline(never)]
    pub fn two_prod(batch: &[(f64, f64)], out: &mut Vec<(f64, f64)>) {
        run(batch, out, |&(a, b)| e::two_prod(a, b));
    }
}

/// Interval arithmetic: `geom_num::interval::Interval`.
pub mod interval {
    use geom_num::interval::Interval;
    use geom_num::{NumError, Sign};

    use super::run;

    /// `Interval::new` on each (lo, hi) pair.
    #[inline(never)]
    pub fn new(batch: &[(f64, f64)], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&(lo, hi)| Interval::new(lo, hi));
    }

    /// `Interval::contains` on each (interval, value) pair.
    #[inline(never)]
    pub fn contains(batch: &[(Interval, f64)], out: &mut Vec<bool>) {
        run(batch, out, |&(x, v)| x.contains(v));
    }

    /// `Interval::add` on each pair.
    #[inline(never)]
    pub fn add(batch: &[(Interval, Interval)], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&(x, y)| x.add(y));
    }

    /// `Interval::sub` on each pair.
    #[inline(never)]
    pub fn sub(batch: &[(Interval, Interval)], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&(x, y)| x.sub(y));
    }

    /// `Interval::mul` on each pair.
    #[inline(never)]
    pub fn mul(batch: &[(Interval, Interval)], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&(x, y)| x.mul(y));
    }

    /// `Interval::div` on each pair.
    #[inline(never)]
    pub fn div(batch: &[(Interval, Interval)], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&(x, y)| x.div(y));
    }

    /// `Interval::mul_add` on each (a, b, c) triple: a · b + c.
    #[inline(never)]
    pub fn mul_add(
        batch: &[(Interval, Interval, Interval)],
        out: &mut Vec<Result<Interval, NumError>>,
    ) {
        run(batch, out, |&(a, b, c)| a.mul_add(b, c));
    }

    /// `Interval::square` on each interval.
    #[inline(never)]
    pub fn square(batch: &[Interval], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&x| x.square());
    }

    /// `Interval::sqrt` on each interval.
    #[inline(never)]
    pub fn sqrt(batch: &[Interval], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |&x| x.sqrt());
    }

    /// `Interval::norm` on each vector of three components: the length of a 3D difference.
    #[inline(never)]
    pub fn norm(batch: &[[Interval; 3]], out: &mut Vec<Result<Interval, NumError>>) {
        run(batch, out, |components| Interval::norm(components));
    }

    /// `Interval::neg` on each interval.
    #[inline(never)]
    pub fn neg(batch: &[Interval], out: &mut Vec<Interval>) {
        run(batch, out, |&x| x.neg());
    }

    /// `Interval::certain_sign` on each interval.
    #[inline(never)]
    pub fn certain_sign(batch: &[Interval], out: &mut Vec<Option<Sign>>) {
        run(batch, out, |&x| x.certain_sign());
    }
}

/// Grid coordinates and moving them between grid steps: `geom_num::grid`.
pub mod grid {
    use geom_num::NumError;
    use geom_num::grid::{self as g, Coord, GridStep, P2, P3, Rounded, Steps};

    use super::run;

    /// `Coord::new` on each step count.
    #[inline(never)]
    pub fn coord_new(batch: &[i64], out: &mut Vec<Result<Coord, NumError>>) {
        run(batch, out, |&v| Coord::new(v));
    }

    /// `Coord::from_i128` on each step count.
    #[inline(never)]
    pub fn coord_from_i128(batch: &[i128], out: &mut Vec<Result<Coord, NumError>>) {
        run(batch, out, |&v| Coord::from_i128(v));
    }

    /// `P2::new` on each pair of step counts.
    #[inline(never)]
    pub fn p2_new(batch: &[(i64, i64)], out: &mut Vec<Result<P2, NumError>>) {
        run(batch, out, |&(x, y)| P2::new(x, y));
    }

    /// `P3::new` on each triple of step counts.
    #[inline(never)]
    pub fn p3_new(batch: &[(i64, i64, i64)], out: &mut Vec<Result<P3, NumError>>) {
        run(batch, out, |&(x, y, z)| P3::new(x, y, z));
    }

    /// `refine` on each (coordinate, from, to) triple.
    #[inline(never)]
    pub fn refine(batch: &[(Coord, GridStep, GridStep)], out: &mut Vec<Result<Coord, NumError>>) {
        run(batch, out, |&(c, from, to)| g::refine(c, from, to));
    }

    /// `coarsen` on each (coordinate, from, to) triple.
    #[inline(never)]
    pub fn coarsen(
        batch: &[(Coord, GridStep, GridStep)],
        out: &mut Vec<Result<Rounded<Coord>, NumError>>,
    ) {
        run(batch, out, |&(c, from, to)| g::coarsen(c, from, to));
    }

    /// `GridStep::steps_per` on each (step, unit in quarter-nanometres) pair.
    #[inline(never)]
    pub fn steps_per(batch: &[(GridStep, u64)], out: &mut Vec<Option<u64>>) {
        run(batch, out, |&(step, unit)| step.steps_per(unit));
    }

    /// `Steps::refine` on each (count, from, to) triple.
    #[inline(never)]
    pub fn steps_refine(batch: &[(Steps, GridStep, GridStep)], out: &mut Vec<Option<Steps>>) {
        run(batch, out, |&(s, from, to)| s.refine(from, to));
    }
}

/// Finite doubles in total order: `geom_num::real::Real`.
pub mod real {
    use core::cmp::Ordering;

    use geom_num::real::Real;

    use super::run;

    /// `Real::total_max` on each pair.
    #[inline(never)]
    pub fn total_max(batch: &[(Real, Real)], out: &mut Vec<Real>) {
        run(batch, out, |&(x, y)| x.total_max(y));
    }

    /// `Real::total_min` on each pair.
    #[inline(never)]
    pub fn total_min(batch: &[(Real, Real)], out: &mut Vec<Real>) {
        run(batch, out, |&(x, y)| x.total_min(y));
    }

    /// `Ord::cmp` on each pair.
    #[inline(never)]
    pub fn cmp(batch: &[(Real, Real)], out: &mut Vec<Ordering>) {
        run(batch, out, |(x, y)| x.cmp(y));
    }
}

/// Exactness classes: `geom_num::exactness`.
pub mod exactness {
    use geom_num::grid::{GridStep, Steps};
    use geom_num::{Exactness, NumError};

    use super::run;

    /// `Exactness::combine` on each pair.
    #[inline(never)]
    pub fn combine(batch: &[(Exactness, Exactness)], out: &mut Vec<Option<Exactness>>) {
        run(batch, out, |&(x, y)| x.combine(y));
    }

    /// `Exactness::within` on each (class, ceiling, ceiling's step) triple.
    #[inline(never)]
    pub fn within(
        batch: &[(Exactness, Steps, GridStep)],
        out: &mut Vec<Result<Exactness, NumError>>,
    ) {
        run(batch, out, |&(x, ceiling, step)| x.within(ceiling, step));
    }

    /// `Exactness::combine_within` on each (class, class, ceiling, ceiling's step) quadruple.
    #[inline(never)]
    pub fn combine_within(
        batch: &[(Exactness, Exactness, Steps, GridStep)],
        out: &mut Vec<Result<Exactness, NumError>>,
    ) {
        run(batch, out, |&(x, y, ceiling, step)| {
            x.combine_within(y, ceiling, step)
        });
    }
}

/// The deterministic maths functions: `geom_num::math`. Each returns a `Real`, or the error it
/// reports (an argument outside its domain, or a result that would not be finite), and the kernel
/// stores whichever it returns.
pub mod math {
    use geom_num::math as m;
    use geom_num::{NumError, Real};

    use super::run;

    /// Defines a kernel for a one-argument function.
    macro_rules! unary {
        ($($name:ident),+ $(,)?) => {$(
            #[doc = concat!("`math::", stringify!($name), "` on each value.")]
            #[inline(never)]
            pub fn $name(batch: &[f64], out: &mut Vec<Result<Real, NumError>>) {
                run(batch, out, |&x| m::$name(x));
            }
        )+};
    }

    /// Defines a kernel for a two-argument function.
    macro_rules! binary {
        ($($name:ident),+ $(,)?) => {$(
            #[doc = concat!("`math::", stringify!($name), "` on each pair.")]
            #[inline(never)]
            pub fn $name(batch: &[(f64, f64)], out: &mut Vec<Result<Real, NumError>>) {
                run(batch, out, |&(x, y)| m::$name(x, y));
            }
        )+};
    }

    /// `math::powi` on each (base, whole exponent) pair.
    #[inline(never)]
    pub fn powi(batch: &[(f64, i32)], out: &mut Vec<Result<Real, NumError>>) {
        run(batch, out, |&(x, n)| m::powi(x, n));
    }

    /// `math::fma` on each (a, b, c) triple: a · b + c, rounded once.
    #[inline(never)]
    pub fn fma(batch: &[(f64, f64, f64)], out: &mut Vec<Result<Real, NumError>>) {
        run(batch, out, |&(a, b, c)| m::fma(a, b, c));
    }

    unary!(
        sqrt,
        sin,
        cos,
        tan,
        asin,
        acos,
        atan,
        exp,
        ln,
        log2,
        log10,
        cbrt,
        abs,
        floor,
        ceil,
        round_ties_even,
        sinpi,
        cospi,
    );
    binary!(atan2, pow, hypot);
}

/// Counter-based random numbers: `geom_num::rng`. A draw moves its stream on, so each draw kernel
/// clones the input stream and draws from the clone, leaving the batch as it was for the next pass:
/// the clone, a copy of a few dozen bytes, is in every count of a draw.
pub mod rng {
    use geom_num::real::Real;
    use geom_num::rng::{self as r, Philox, RngError, Stream, StreamRange};

    use super::run;

    /// `philox4x32_10` on each (counter, key) pair.
    #[inline(never)]
    pub fn philox4x32_10(batch: &[([u32; 4], [u32; 2])], out: &mut Vec<[u32; 4]>) {
        run(batch, out, |&(counter, key)| r::philox4x32_10(counter, key));
    }

    /// `Philox::stream_from` on each (generator, block) pair.
    #[inline(never)]
    pub fn stream_from(batch: &[(Philox, u64)], out: &mut Vec<Stream>) {
        run(batch, out, |&(philox, block)| philox.stream_from(block));
    }

    /// `Stream::next_u32` on a clone of each stream.
    #[inline(never)]
    pub fn next_u32(batch: &[Stream], out: &mut Vec<Result<u32, RngError>>) {
        run(batch, out, |stream| stream.clone().next_u32());
    }

    /// `Stream::next_u64` on a clone of each stream.
    #[inline(never)]
    pub fn next_u64(batch: &[Stream], out: &mut Vec<Result<u64, RngError>>) {
        run(batch, out, |stream| stream.clone().next_u64());
    }

    /// `Stream::next_f64` on a clone of each stream.
    #[inline(never)]
    pub fn next_f64(batch: &[Stream], out: &mut Vec<Result<Real, RngError>>) {
        run(batch, out, |stream| stream.clone().next_f64());
    }

    /// `Stream::below` on a clone of each stream, with its bound.
    #[inline(never)]
    pub fn below(batch: &[(Stream, u64)], out: &mut Vec<Result<u64, RngError>>) {
        run(batch, out, |(stream, bound)| stream.clone().below(*bound));
    }

    /// `Stream::range_inclusive` on a clone of each stream, with its two ends.
    #[inline(never)]
    pub fn range_inclusive(batch: &[(Stream, u64, u64)], out: &mut Vec<Result<u64, RngError>>) {
        run(batch, out, |(stream, low, high)| {
            stream.clone().range_inclusive(*low, *high)
        });
    }

    /// `StreamRange::philox` on each (range, index) pair.
    #[inline(never)]
    pub fn range_philox(batch: &[(StreamRange, u64)], out: &mut Vec<Result<Philox, RngError>>) {
        run(batch, out, |&(range, index)| range.philox(index));
    }

    /// `StreamRange::chunk` on each (range, index, size) triple.
    #[inline(never)]
    pub fn chunk(batch: &[(StreamRange, u64, u64)], out: &mut Vec<Result<StreamRange, RngError>>) {
        run(batch, out, |&(range, index, size)| range.chunk(index, size));
    }
}
