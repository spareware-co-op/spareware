//! The list of every benchmark: each operation, the `geom-num` function it measures, its group and
//! the input classes it is measured on; the growth checks; and the functions deliberately left
//! unmeasured, with the reason for each.
//!
//! [`OPS`] and [`GROWTH`] are the single list. Every harness finds its benchmarks here: the
//! instruction-count gate looks each one up by name ([`prepare_named`]), the WebAssembly fuel gate
//! runs every name ([`names`]), the wall-clock harness runs every case, and the golden tests check
//! every digest. Tests check that the gate's source lists exactly these benchmarks
//! (`tests/coverage.rs`), and that every public function of `geom-num` is either measured here or
//! named in [`UNMEASURED`] with a reason (principle 12: every operation ships with a benchmark).
//!
//! A benchmark's name is `group/op/class` (`growth/op/l<limbs>` for the growth checks), matching
//! the gate's identifiers `instructions::group::op class` and the module paths of
//! [`crate::inputs`] and [`crate::kernels`].
//!
//! Guarantees: every name is unique and prepares exactly one benchmark, and no other string
//! prepares one ([`prepare_named`] returns `None`); the tests check both.
//!
//! Method: one table drives every harness, so a benchmark cannot be timed but not gated, or gated
//! but not checked; each harness's own registry (divan's `args`,
//! <https://docs.rs/divan/0.1.21/divan/attr.bench.html>, and Gungraun's benchmark groups) is filled
//! from it.
//!
//! Cost: the tables are static. Finding a name scans them once (a few hundred entries), and
//! preparing a benchmark builds its batch, O([`inputs::BATCH`]).

use core::fmt;

use crate::canonical::Canonical;
use crate::harness::{Bench, Bound, Kernel};
use crate::inputs::Class::{
    Degenerate, Extreme, NearlyDegenerate, OnGrid, Overflow, Typical, WrongDirection,
};
use crate::inputs::{self, Class};
use crate::kernels;

/// A benchmark ready to run, whatever its types.
pub type Prepared = Box<dyn Bench>;

/// Binds a batch to its kernel.
fn bind<T: Canonical + 'static, R: Canonical + 'static>(
    batch: Vec<T>,
    kernel: Kernel<T, R>,
) -> Prepared {
    Box::new(Bound::new(batch, kernel))
}

/// One operation and the classes it is measured on.
pub struct Op {
    /// The group: the `geom-num` module the operation belongs to.
    pub group: &'static str,
    /// The operation's name within its group, as in [`crate::kernels`].
    pub name: &'static str,
    /// The `geom-num` function measured, as `module::function` or `Type::method`.
    pub measures: &'static str,
    /// The input classes, in the order they are reported.
    pub classes: &'static [Class],
    /// Builds the inputs of a class and binds them to the kernel.
    prepare: fn(Class) -> Prepared,
}

/// One benchmark: an operation on one class of inputs.
#[derive(Clone, Copy)]
pub struct Case {
    /// The operation.
    pub op: &'static Op,
    /// The input class.
    pub class: Class,
}

impl Case {
    /// Builds this benchmark's inputs, ready to run.
    #[must_use]
    pub fn prepare(&self) -> Prepared {
        (self.op.prepare)(self.class)
    }

    /// The name, `group/op/class`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{}/{}/{}", self.op.group, self.op.name, self.class)
    }
}

impl fmt::Display for Case {
    /// `op/class`: the name within the group, as the wall-clock harness lists it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.op.name, self.class)
    }
}

/// Classes of the predicates, `Plane::side` and `Plane::side_of_tpi`.
const TDNE: &[Class] = &[Typical, Degenerate, NearlyDegenerate, Extreme];
/// Classes of `Plane::through` and `Tpi::new`.
const TDE: &[Class] = &[Typical, Degenerate, Extreme];
/// Classes of checked wide-integer arithmetic, of the random draws and of cutting stream ranges.
const TEO: &[Class] = &[Typical, Extreme, Overflow];
/// Classes of most floating-point operations, `Tpi::homogeneous`, `coarsen`, comparisons and
/// starting a stream at a block.
const TE: &[Class] = &[Typical, Extreme];
/// Classes of conversions that can fail, and of a stream range's stream by index: in range, and
/// out of range.
const TO: &[Class] = &[Typical, Overflow];
/// The class of operations with a single path, or whose paths one batch covers in rotation.
const T: &[Class] = &[Typical];

/// Every benchmarked operation, grouped as the gate groups them. One line per operation, so the
/// list reads as a table.
#[rustfmt::skip]
pub static OPS: &[Op] = &[
    // Exact predicates.
    Op { group: "predicate", name: "orient2d", measures: "predicate::orient2d", classes: TDNE, prepare: |c| bind(inputs::predicate::orient2d(c), kernels::predicate::orient2d) },
    Op { group: "predicate", name: "orient3d", measures: "predicate::orient3d", classes: TDNE, prepare: |c| bind(inputs::predicate::orient3d(c), kernels::predicate::orient3d) },
    Op { group: "predicate", name: "incircle", measures: "predicate::incircle", classes: TDNE, prepare: |c| bind(inputs::predicate::incircle(c), kernels::predicate::incircle) },
    Op { group: "predicate", name: "insphere", measures: "predicate::insphere", classes: TDNE, prepare: |c| bind(inputs::predicate::insphere(c), kernels::predicate::insphere) },
    // Signs.
    Op { group: "sign", name: "of_i128", measures: "Sign::of_i128", classes: T, prepare: |c| bind(inputs::sign::of_i128(c), kernels::sign::of_i128) },
    // Fixed-width integers, at each width the kernel uses.
    Op { group: "wide", name: "i256_add", measures: "Wide::checked_add", classes: TEO, prepare: |c| bind(inputs::wide::add::<4>(c), kernels::wide::add::<4>) },
    Op { group: "wide", name: "i256_sub", measures: "Wide::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide::sub::<4>(c), kernels::wide::sub::<4>) },
    Op { group: "wide", name: "i256_mul", measures: "Wide::checked_mul", classes: TEO, prepare: |c| bind(inputs::wide::mul::<4>(c), kernels::wide::mul::<4>) },
    Op { group: "wide", name: "i256_neg", measures: "Wide::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide::neg::<4>(c), kernels::wide::neg::<4>) },
    Op { group: "wide", name: "i256_cmp", measures: "Wide::cmp", classes: TE, prepare: |c| bind(inputs::wide::cmp::<4>(c), kernels::wide::cmp::<4>) },
    Op { group: "wide", name: "i256_signum", measures: "Wide::signum", classes: T, prepare: |c| bind(inputs::wide::signum::<4>(c), kernels::wide::signum::<4>) },
    Op { group: "wide", name: "i256_to_i128", measures: "Wide::to_i128", classes: TO, prepare: |c| bind(inputs::wide::to_i128::<4>(c), kernels::wide::to_i128::<4>) },
    Op { group: "wide", name: "i256_from_i128", measures: "Wide::from_i128", classes: T, prepare: |c| bind(inputs::wide::from_i128::<4>(c), kernels::wide::from_i128::<4>) },
    Op { group: "wide", name: "i320_add", measures: "Wide::checked_add", classes: TEO, prepare: |c| bind(inputs::wide::add::<5>(c), kernels::wide::add::<5>) },
    Op { group: "wide", name: "i320_sub", measures: "Wide::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide::sub::<5>(c), kernels::wide::sub::<5>) },
    Op { group: "wide", name: "i320_mul", measures: "Wide::checked_mul", classes: TEO, prepare: |c| bind(inputs::wide::mul::<5>(c), kernels::wide::mul::<5>) },
    Op { group: "wide", name: "i320_neg", measures: "Wide::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide::neg::<5>(c), kernels::wide::neg::<5>) },
    Op { group: "wide", name: "i320_cmp", measures: "Wide::cmp", classes: TE, prepare: |c| bind(inputs::wide::cmp::<5>(c), kernels::wide::cmp::<5>) },
    Op { group: "wide", name: "i320_signum", measures: "Wide::signum", classes: T, prepare: |c| bind(inputs::wide::signum::<5>(c), kernels::wide::signum::<5>) },
    Op { group: "wide", name: "i320_to_i128", measures: "Wide::to_i128", classes: TO, prepare: |c| bind(inputs::wide::to_i128::<5>(c), kernels::wide::to_i128::<5>) },
    Op { group: "wide", name: "i320_from_i128", measures: "Wide::from_i128", classes: T, prepare: |c| bind(inputs::wide::from_i128::<5>(c), kernels::wide::from_i128::<5>) },
    Op { group: "wide", name: "i384_add", measures: "Wide::checked_add", classes: TEO, prepare: |c| bind(inputs::wide::add::<6>(c), kernels::wide::add::<6>) },
    Op { group: "wide", name: "i384_sub", measures: "Wide::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide::sub::<6>(c), kernels::wide::sub::<6>) },
    Op { group: "wide", name: "i384_mul", measures: "Wide::checked_mul", classes: TEO, prepare: |c| bind(inputs::wide::mul::<6>(c), kernels::wide::mul::<6>) },
    Op { group: "wide", name: "i384_neg", measures: "Wide::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide::neg::<6>(c), kernels::wide::neg::<6>) },
    Op { group: "wide", name: "i384_cmp", measures: "Wide::cmp", classes: TE, prepare: |c| bind(inputs::wide::cmp::<6>(c), kernels::wide::cmp::<6>) },
    Op { group: "wide", name: "i384_signum", measures: "Wide::signum", classes: T, prepare: |c| bind(inputs::wide::signum::<6>(c), kernels::wide::signum::<6>) },
    Op { group: "wide", name: "i384_to_i128", measures: "Wide::to_i128", classes: TO, prepare: |c| bind(inputs::wide::to_i128::<6>(c), kernels::wide::to_i128::<6>) },
    Op { group: "wide", name: "i384_from_i128", measures: "Wide::from_i128", classes: T, prepare: |c| bind(inputs::wide::from_i128::<6>(c), kernels::wide::from_i128::<6>) },
    Op { group: "wide", name: "i512_add", measures: "Wide::checked_add", classes: TEO, prepare: |c| bind(inputs::wide::add::<8>(c), kernels::wide::add::<8>) },
    Op { group: "wide", name: "i512_sub", measures: "Wide::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide::sub::<8>(c), kernels::wide::sub::<8>) },
    Op { group: "wide", name: "i512_mul", measures: "Wide::checked_mul", classes: TEO, prepare: |c| bind(inputs::wide::mul::<8>(c), kernels::wide::mul::<8>) },
    Op { group: "wide", name: "i512_neg", measures: "Wide::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide::neg::<8>(c), kernels::wide::neg::<8>) },
    Op { group: "wide", name: "i512_cmp", measures: "Wide::cmp", classes: TE, prepare: |c| bind(inputs::wide::cmp::<8>(c), kernels::wide::cmp::<8>) },
    Op { group: "wide", name: "i512_signum", measures: "Wide::signum", classes: T, prepare: |c| bind(inputs::wide::signum::<8>(c), kernels::wide::signum::<8>) },
    Op { group: "wide", name: "i512_to_i128", measures: "Wide::to_i128", classes: TO, prepare: |c| bind(inputs::wide::to_i128::<8>(c), kernels::wide::to_i128::<8>) },
    Op { group: "wide", name: "i512_from_i128", measures: "Wide::from_i128", classes: T, prepare: |c| bind(inputs::wide::from_i128::<8>(c), kernels::wide::from_i128::<8>) },
    // The width changes the plane code makes, and the narrowing that can fail.
    Op { group: "wide", name: "i256_to_i384", measures: "Wide::resize", classes: T, prepare: |c| bind(inputs::wide::resize::<4, 6>(c), kernels::wide::resize::<4, 6>) },
    Op { group: "wide", name: "i320_to_i384", measures: "Wide::resize", classes: T, prepare: |c| bind(inputs::wide::resize::<5, 6>(c), kernels::wide::resize::<5, 6>) },
    Op { group: "wide", name: "i384_to_i256", measures: "Wide::resize", classes: TO, prepare: |c| bind(inputs::wide::resize::<6, 4>(c), kernels::wide::resize::<6, 4>) },
    // Planes and the points where three meet.
    Op { group: "plane", name: "through", measures: "Plane::through", classes: TDE, prepare: |c| bind(inputs::plane::through(c), kernels::plane::through) },
    Op { group: "plane", name: "side", measures: "Plane::side", classes: TDNE, prepare: |c| bind(inputs::plane::side(c), kernels::plane::side) },
    Op { group: "plane", name: "tpi_new", measures: "Tpi::new", classes: TDE, prepare: |c| bind(inputs::plane::tpi_new(c), kernels::plane::tpi_new) },
    Op { group: "plane", name: "tpi_homogeneous", measures: "Tpi::homogeneous", classes: TE, prepare: |c| bind(inputs::plane::tpi_homogeneous(c), kernels::plane::tpi_homogeneous) },
    Op { group: "plane", name: "side_of_tpi", measures: "Plane::side_of_tpi", classes: TDNE, prepare: |c| bind(inputs::plane::side_of_tpi(c), kernels::plane::side_of_tpi) },
    Op { group: "plane", name: "to_grid", measures: "Tpi::to_grid", classes: &[Typical, OnGrid, Extreme, Overflow], prepare: |c| bind(inputs::plane::to_grid(c), kernels::plane::to_grid) },
    // Error-free transformations.
    Op { group: "eft", name: "two_sum", measures: "eft::two_sum", classes: TE, prepare: |c| bind(inputs::eft::two_sum(c), kernels::eft::two_sum) },
    Op { group: "eft", name: "fast_two_sum", measures: "eft::fast_two_sum", classes: TE, prepare: |c| bind(inputs::eft::fast_two_sum(c), kernels::eft::fast_two_sum) },
    Op { group: "eft", name: "split", measures: "eft::split", classes: TE, prepare: |c| bind(inputs::eft::split(c), kernels::eft::split) },
    Op { group: "eft", name: "two_prod", measures: "eft::two_prod", classes: TE, prepare: |c| bind(inputs::eft::two_prod(c), kernels::eft::two_prod) },
    // Intervals.
    Op { group: "interval", name: "new", measures: "Interval::new", classes: TE, prepare: |c| bind(inputs::interval::new(c), kernels::interval::new) },
    Op { group: "interval", name: "contains", measures: "Interval::contains", classes: T, prepare: |c| bind(inputs::interval::contains(c), kernels::interval::contains) },
    Op { group: "interval", name: "add", measures: "Interval::add", classes: TE, prepare: |c| bind(inputs::interval::add(c), kernels::interval::add) },
    Op { group: "interval", name: "sub", measures: "Interval::sub", classes: TE, prepare: |c| bind(inputs::interval::sub(c), kernels::interval::sub) },
    Op { group: "interval", name: "mul", measures: "Interval::mul", classes: TE, prepare: |c| bind(inputs::interval::mul(c), kernels::interval::mul) },
    Op { group: "interval", name: "div", measures: "Interval::div", classes: TE, prepare: |c| bind(inputs::interval::div(c), kernels::interval::div) },
    Op { group: "interval", name: "mul_add", measures: "Interval::mul_add", classes: TE, prepare: |c| bind(inputs::interval::mul_add(c), kernels::interval::mul_add) },
    Op { group: "interval", name: "square", measures: "Interval::square", classes: TE, prepare: |c| bind(inputs::interval::square(c), kernels::interval::square) },
    Op { group: "interval", name: "sqrt", measures: "Interval::sqrt", classes: TE, prepare: |c| bind(inputs::interval::sqrt(c), kernels::interval::sqrt) },
    Op { group: "interval", name: "norm", measures: "Interval::norm", classes: TE, prepare: |c| bind(inputs::interval::norm(c), kernels::interval::norm) },
    Op { group: "interval", name: "neg", measures: "Interval::neg", classes: T, prepare: |c| bind(inputs::interval::neg(c), kernels::interval::neg) },
    Op { group: "interval", name: "certain_sign", measures: "Interval::certain_sign", classes: T, prepare: |c| bind(inputs::interval::certain_sign(c), kernels::interval::certain_sign) },
    // Grid coordinates and grid steps.
    Op { group: "grid", name: "coord_new", measures: "Coord::new", classes: TO, prepare: |c| bind(inputs::grid::coord_new(c), kernels::grid::coord_new) },
    Op { group: "grid", name: "coord_from_i128", measures: "Coord::from_i128", classes: TO, prepare: |c| bind(inputs::grid::coord_from_i128(c), kernels::grid::coord_from_i128) },
    Op { group: "grid", name: "p2_new", measures: "P2::new", classes: TO, prepare: |c| bind(inputs::grid::p2_new(c), kernels::grid::p2_new) },
    Op { group: "grid", name: "p3_new", measures: "P3::new", classes: TO, prepare: |c| bind(inputs::grid::p3_new(c), kernels::grid::p3_new) },
    Op { group: "grid", name: "refine", measures: "grid::refine", classes: &[Typical, Overflow, WrongDirection], prepare: |c| bind(inputs::grid::refine(c), kernels::grid::refine) },
    Op { group: "grid", name: "coarsen", measures: "grid::coarsen", classes: &[Typical, Extreme, WrongDirection], prepare: |c| bind(inputs::grid::coarsen(c), kernels::grid::coarsen) },
    Op { group: "grid", name: "steps_refine", measures: "Steps::refine", classes: &[Typical, Overflow, WrongDirection], prepare: |c| bind(inputs::grid::steps_refine(c), kernels::grid::steps_refine) },
    Op { group: "grid", name: "steps_per", measures: "GridStep::steps_per", classes: T, prepare: |c| bind(inputs::grid::steps_per(c), kernels::grid::steps_per) },
    // Finite doubles in total order.
    Op { group: "real", name: "total_max", measures: "Real::total_max", classes: T, prepare: |c| bind(inputs::real::total_max(c), kernels::real::total_max) },
    Op { group: "real", name: "total_min", measures: "Real::total_min", classes: T, prepare: |c| bind(inputs::real::total_min(c), kernels::real::total_min) },
    Op { group: "real", name: "cmp", measures: "Real::cmp", classes: T, prepare: |c| bind(inputs::real::cmp(c), kernels::real::cmp) },
    // Exactness classes.
    Op { group: "exactness", name: "combine", measures: "Exactness::combine", classes: T, prepare: |c| bind(inputs::exactness::combine(c), kernels::exactness::combine) },
    Op { group: "exactness", name: "within", measures: "Exactness::within", classes: T, prepare: |c| bind(inputs::exactness::within(c), kernels::exactness::within) },
    Op { group: "exactness", name: "combine_within", measures: "Exactness::combine_within", classes: T, prepare: |c| bind(inputs::exactness::combine_within(c), kernels::exactness::combine_within) },
    // Deterministic maths.
    Op { group: "math", name: "sqrt", measures: "math::sqrt", classes: TE, prepare: |c| bind(inputs::math::sqrt(c), kernels::math::sqrt) },
    Op { group: "math", name: "sin", measures: "math::sin", classes: TE, prepare: |c| bind(inputs::math::sin(c), kernels::math::sin) },
    Op { group: "math", name: "cos", measures: "math::cos", classes: TE, prepare: |c| bind(inputs::math::cos(c), kernels::math::cos) },
    Op { group: "math", name: "tan", measures: "math::tan", classes: TE, prepare: |c| bind(inputs::math::tan(c), kernels::math::tan) },
    Op { group: "math", name: "asin", measures: "math::asin", classes: TE, prepare: |c| bind(inputs::math::asin(c), kernels::math::asin) },
    Op { group: "math", name: "acos", measures: "math::acos", classes: TE, prepare: |c| bind(inputs::math::acos(c), kernels::math::acos) },
    Op { group: "math", name: "atan", measures: "math::atan", classes: TE, prepare: |c| bind(inputs::math::atan(c), kernels::math::atan) },
    Op { group: "math", name: "atan2", measures: "math::atan2", classes: TE, prepare: |c| bind(inputs::math::atan2(c), kernels::math::atan2) },
    Op { group: "math", name: "exp", measures: "math::exp", classes: TE, prepare: |c| bind(inputs::math::exp(c), kernels::math::exp) },
    Op { group: "math", name: "ln", measures: "math::ln", classes: TE, prepare: |c| bind(inputs::math::ln(c), kernels::math::ln) },
    Op { group: "math", name: "log2", measures: "math::log2", classes: TE, prepare: |c| bind(inputs::math::log2(c), kernels::math::log2) },
    Op { group: "math", name: "log10", measures: "math::log10", classes: TE, prepare: |c| bind(inputs::math::log10(c), kernels::math::log10) },
    Op { group: "math", name: "pow", measures: "math::pow", classes: TE, prepare: |c| bind(inputs::math::pow(c), kernels::math::pow) },
    Op { group: "math", name: "powi", measures: "math::powi", classes: TE, prepare: |c| bind(inputs::math::powi(c), kernels::math::powi) },
    Op { group: "math", name: "fma", measures: "math::fma", classes: TE, prepare: |c| bind(inputs::math::fma(c), kernels::math::fma) },
    Op { group: "math", name: "hypot", measures: "math::hypot", classes: TE, prepare: |c| bind(inputs::math::hypot(c), kernels::math::hypot) },
    Op { group: "math", name: "cbrt", measures: "math::cbrt", classes: TE, prepare: |c| bind(inputs::math::cbrt(c), kernels::math::cbrt) },
    Op { group: "math", name: "abs", measures: "math::abs", classes: TE, prepare: |c| bind(inputs::math::abs(c), kernels::math::abs) },
    Op { group: "math", name: "floor", measures: "math::floor", classes: TE, prepare: |c| bind(inputs::math::floor(c), kernels::math::floor) },
    Op { group: "math", name: "ceil", measures: "math::ceil", classes: TE, prepare: |c| bind(inputs::math::ceil(c), kernels::math::ceil) },
    Op { group: "math", name: "round_ties_even", measures: "math::round_ties_even", classes: TE, prepare: |c| bind(inputs::math::round_ties_even(c), kernels::math::round_ties_even) },
    Op { group: "math", name: "sinpi", measures: "math::sinpi", classes: TE, prepare: |c| bind(inputs::math::sinpi(c), kernels::math::sinpi) },
    Op { group: "math", name: "cospi", measures: "math::cospi", classes: TE, prepare: |c| bind(inputs::math::cospi(c), kernels::math::cospi) },
    // Counter-based random numbers.
    Op { group: "rng", name: "philox4x32_10", measures: "rng::philox4x32_10", classes: T, prepare: |c| bind(inputs::rng::philox4x32_10(c), kernels::rng::philox4x32_10) },
    Op { group: "rng", name: "stream_from", measures: "Philox::stream_from", classes: TE, prepare: |c| bind(inputs::rng::stream_from(c), kernels::rng::stream_from) },
    Op { group: "rng", name: "next_u32", measures: "Stream::next_u32", classes: TEO, prepare: |c| bind(inputs::rng::next_u32(c), kernels::rng::next_u32) },
    Op { group: "rng", name: "next_u64", measures: "Stream::next_u64", classes: TEO, prepare: |c| bind(inputs::rng::next_u64(c), kernels::rng::next_u64) },
    Op { group: "rng", name: "next_f64", measures: "Stream::next_f64", classes: TEO, prepare: |c| bind(inputs::rng::next_f64(c), kernels::rng::next_f64) },
    Op { group: "rng", name: "below", measures: "Stream::below", classes: TEO, prepare: |c| bind(inputs::rng::below(c), kernels::rng::below) },
    Op { group: "rng", name: "range_inclusive", measures: "Stream::range_inclusive", classes: TEO, prepare: |c| bind(inputs::rng::range_inclusive(c), kernels::rng::range_inclusive) },
    Op { group: "rng", name: "range_philox", measures: "StreamRange::philox", classes: TO, prepare: |c| bind(inputs::rng::range_philox(c), kernels::rng::range_philox) },
    Op { group: "rng", name: "chunk", measures: "StreamRange::chunk", classes: TEO, prepare: |c| bind(inputs::rng::chunk(c), kernels::rng::chunk) },
];

/// The groups of [`OPS`], in order.
pub const GROUPS: [&str; 11] = [
    "predicate",
    "sign",
    "wide",
    "plane",
    "eft",
    "interval",
    "grid",
    "real",
    "exactness",
    "math",
    "rng",
];

/// The group of the growth checks.
pub const GROWTH_GROUP: &str = "growth";

/// Every benchmark of [`OPS`], in order.
pub fn cases() -> impl Iterator<Item = Case> {
    OPS.iter()
        .flat_map(|op| op.classes.iter().map(move |&class| Case { op, class }))
}

/// The benchmarks of one group of [`OPS`], in order.
pub fn cases_in(group: &'static str) -> impl Iterator<Item = Case> {
    cases().filter(move |case| case.op.group == group)
}

/// A scale benchmark (chapter 28.14, "Growth"): an operation whose cost depends on a size, run at
/// n, 2n and 4n, whose work must grow no faster than its stated cost.
pub struct Growth {
    /// The name within the growth group.
    pub name: &'static str,
    /// The `geom-num` function measured.
    pub measures: &'static str,
    /// The stated cost: the work grows as size^exponent.
    pub exponent: u32,
    /// The method the stated cost comes from.
    pub cost: &'static str,
    /// The sizes, n, 2n and 4n, in limbs of 64 bits.
    pub sizes: [usize; 3],
    /// Builds the batch at a size (one of `sizes`) and binds it to the kernel at that width.
    prepare: fn(usize) -> Option<Prepared>,
}

/// One growth benchmark: a scale benchmark at one size.
#[derive(Clone, Copy)]
pub struct GrowthCase {
    /// The scale benchmark.
    pub growth: &'static Growth,
    /// The size, in limbs.
    pub size: usize,
}

impl GrowthCase {
    /// Builds this benchmark's inputs, ready to run (`None` only for a size not in the list).
    #[must_use]
    pub fn prepare(&self) -> Option<Prepared> {
        (self.growth.prepare)(self.size)
    }

    /// The name, `growth/op/l<size>`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{GROWTH_GROUP}/{}/l{}", self.growth.name, self.size)
    }
}

impl fmt::Display for GrowthCase {
    /// `op/l<size>`, as the wall-clock harness lists it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/l{}", self.growth.name, self.size)
    }
}

/// Defines the preparation of a growth benchmark at 2, 4 and 8 limbs.
macro_rules! at_widths {
    ($input:ident, $kernel:ident) => {
        |limbs| match limbs {
            2 => Some(bind(
                inputs::growth::$input::<2>(),
                kernels::wide::$kernel::<2>,
            )),
            4 => Some(bind(
                inputs::growth::$input::<4>(),
                kernels::wide::$kernel::<4>,
            )),
            8 => Some(bind(
                inputs::growth::$input::<8>(),
                kernels::wide::$kernel::<8>,
            )),
            _ => None,
        }
    };
}

/// The scale benchmarks: schoolbook arithmetic on fixed-width integers, whose cost Knuth states
/// (volume 2, section 4.3.1): addition, subtraction, negation and comparison are linear in the
/// number of limbs, multiplication quadratic.
#[rustfmt::skip]
pub static GROWTH: &[Growth] = &[
    Growth { name: "wide_add", measures: "Wide::checked_add", exponent: 1, cost: "Knuth 4.3.1, algorithm A", sizes: [2, 4, 8], prepare: at_widths!(add, add) },
    Growth { name: "wide_sub", measures: "Wide::checked_sub", exponent: 1, cost: "Knuth 4.3.1, algorithm S", sizes: [2, 4, 8], prepare: at_widths!(sub, sub) },
    Growth { name: "wide_neg", measures: "Wide::checked_neg", exponent: 1, cost: "Knuth 4.3.1, algorithm S on 0 − x", sizes: [2, 4, 8], prepare: at_widths!(neg, neg) },
    Growth { name: "wide_cmp", measures: "Wide::cmp", exponent: 1, cost: "Knuth 4.3.1, comparison from the top limb", sizes: [2, 4, 8], prepare: at_widths!(cmp, cmp) },
    Growth { name: "wide_mul", measures: "Wide::checked_mul", exponent: 2, cost: "Knuth 4.3.1, algorithm M", sizes: [2, 4, 8], prepare: at_widths!(mul, mul) },
];

/// Every growth benchmark, in [`GROWTH`] order, each at its three sizes.
pub fn growth_cases() -> impl Iterator<Item = GrowthCase> {
    GROWTH.iter().flat_map(|growth| {
        growth
            .sizes
            .iter()
            .map(move |&size| GrowthCase { growth, size })
    })
}

/// The name of every benchmark: [`cases`], then [`growth_cases`].
#[must_use]
pub fn names() -> Vec<String> {
    cases()
        .map(|case| case.name())
        .chain(growth_cases().map(|case| case.name()))
        .collect()
}

/// The benchmark with this name, built and ready to run, or `None` if there is no such benchmark.
#[must_use]
pub fn prepare_named(name: &str) -> Option<Prepared> {
    let mut parts = name.split('/');
    let (group, op, variant) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    if group == GROWTH_GROUP {
        // The size must be written exactly as the name writes it: `l2`, never `l02` or `l+2`.
        let growth = GROWTH.iter().find(|g| g.name == op)?;
        let &size = growth
            .sizes
            .iter()
            .find(|&&size| variant.strip_prefix('l') == Some(size.to_string().as_str()))?;
        return (growth.prepare)(size);
    }
    let class = Class::named(variant)?;
    let op = OPS.iter().find(|o| o.group == group && o.name == op)?;
    op.classes.contains(&class).then(|| (op.prepare)(class))
}

/// The public functions of `geom-num` that have no benchmark of their own, each with the reason.
/// `tests/coverage.rs` checks that every public function of `geom-num` is measured by an entry of
/// [`OPS`] or [`GROWTH`], or listed here, and that nothing here is stale.
///
/// Three kinds of function qualify, and no other: field accessors and raw constructors, which
/// compute nothing; constants, including the interval functions that refuse every input until a
/// proven version exists (a benchmark of a refusal would measure only the loop, and would fail the
/// gate the day the function is built); and functions whose whole body is one `match` or one call
/// to a function that is measured, whose cost is inside that function's benchmark.
/// `tests/coverage.rs` checks that each constant still refuses, so its entry cannot outlive it.
#[rustfmt::skip]
pub static UNMEASURED: &[(&str, &str)] = &[
    ("Coord::get", "field accessor"),
    ("Steps::get", "field accessor"),
    ("Real::get", "field accessor"),
    ("Rounded::value", "field accessor"),
    ("Rounded::is_exact", "field accessor"),
    ("Interval::lo", "field accessor"),
    ("Interval::hi", "field accessor"),
    ("Plane::n", "field accessor"),
    ("Plane::d", "field accessor"),
    ("Tpi::planes", "field accessor"),
    ("Wide::to_le_limbs", "field accessor"),
    ("Wide::from_le_limbs", "raw constructor"),
    ("Wide::max_value", "a constant: every limb is fixed"),
    ("Wide::min_value", "a constant: every limb is fixed"),
    ("Wide::one", "one call to Wide::from_i128, which is measured"),
    ("Wide::from_i64", "one call to Wide::from_i128, which is measured"),
    ("Wide::is_negative", "one test of the top bit, inside every Wide benchmark"),
    ("Wide::is_zero", "measured inside Wide::signum, whose batch has zeros and values with only the top limb set"),
    ("Interval::point", "one call to Interval::new, which is measured"),
    ("Interval::sin", "a constant: always Err(UnprovenMaths) until build piece G47 proves the sine's error and adds its benchmark"),
    ("Interval::cos", "a constant: always Err(UnprovenMaths) until build piece G47 proves the cosine's error and adds its benchmark"),
    ("Interval::atan2", "a constant: always Err(UnprovenMaths) until build piece G47 proves the arctangent's error and adds its benchmark"),
    ("Interval::exp", "a constant: always Err(UnprovenMaths) until build piece G47 proves the exponential's error and adds its benchmark"),
    ("Interval::ln", "a constant: always Err(UnprovenMaths) until build piece G47 proves the logarithm's error and adds its benchmark"),
    ("Real::new", "one finiteness test, the same test Interval::new's benchmark measures"),
    ("Sign::of_i64", "one call to Sign::of_i128, which is measured"),
    ("Sign::negate", "one match on a three-valued enum"),
    ("Sign::mul", "one match on two three-valued enums"),
    ("Sign::is_zero", "one match on a three-valued enum"),
    ("Sign::as_i8", "one match on a three-valued enum"),
    ("GridStep::quarter_nm", "one match on a three-valued enum"),
    ("GridStep::rung", "one match on a three-valued enum"),
    ("GridStep::finer", "one match on a three-valued enum"),
    ("GridStep::coarser", "one match on a three-valued enum"),
    ("Exactness::rank", "one match on a three-valued enum"),
    ("Exactness::bound", "one match on a three-valued enum"),
    ("Steps::checked_add", "one match on one checked addition, inside Exactness::combine's benchmark"),
    ("ladder::quarter_nm", "one checked multiplication by GridStep::quarter_nm, for reports; never on a kernel path"),
    ("Philox::new", "raw constructor"),
    ("Philox::seed", "field accessor"),
    ("Philox::stream_id", "field accessor"),
    ("Philox::key", "field accessor: the seed's two 32-bit halves"),
    ("Philox::counter", "field accessor: the block index's and the stream id's 32-bit halves, laid out as the counter"),
    ("Philox::block", "one call to rng::philox4x32_10, which is measured, on the counter and key Philox::counter and Philox::key lay out"),
    ("Philox::stream", "one call to Philox::stream_from, which is measured"),
    ("StreamRange::all", "raw constructor"),
    ("StreamRange::seed", "field accessor"),
    ("StreamRange::first", "field accessor"),
    ("StreamRange::last", "field accessor"),
    ("StreamRange::count", "one match on one checked addition, of one to last − first"),
    ("Stream::philox", "field accessor"),
    ("Stream::position", "field accessor"),
    ("Stream::remaining", "field accessor: a constant, 2^66, less the position field"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_prepares_and_no_other() {
        for name in names() {
            assert!(prepare_named(&name).is_some(), "{name}");
        }
        for bad in [
            "",
            "predicate",
            "predicate/orient2d",
            "predicate/orient2d/overflow",
            "predicate/orient2d/typical/x",
            "growth/wide_add/l3",
            "growth/wide_add/4",
            "growth/wide_add/l02",
            "growth/wide_add/l+2",
            "growth/wide_add/l2 ",
            "growth/wide_add/L2",
            "growth/nope/l2",
            "predicate/orient2d/Typical",
            "predicate/orient2d/typical ",
            "predicate//typical",
            "/orient2d/typical",
            "nope/a/b",
        ] {
            assert!(prepare_named(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn growth_sizes_double() {
        for growth in GROWTH {
            let [n, n2, n4] = growth.sizes;
            assert_eq!((n2, n4), (2 * n, 4 * n), "{}", growth.name);
            assert!(growth.exponent >= 1);
        }
    }
}
