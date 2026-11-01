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
    /// The group: the `geom-num` module the operation belongs to, or, for another kernel crate's
    /// hot paths (decision 32.44), a group named for that crate.
    pub group: &'static str,
    /// The operation's name within its group, as in [`crate::kernels`].
    pub name: &'static str,
    /// The `geom-num` function measured, as `module::function` or `Type::method`; a function of
    /// another kernel crate is written with its crate's path, `geom_math::Type::method`.
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
/// Classes of the perturbed predicates: the exact path, and an exact tie the perturbation decides.
const TD: &[Class] = &[Typical, Degenerate];
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
    // Fixed-width integers in 32-bit limbs (piece G42), at the same bit widths as above.
    Op { group: "wide32", name: "w8_add", measures: "Wide32::checked_add", classes: TEO, prepare: |c| bind(inputs::wide32::add::<4, 8>(c), kernels::wide32::add::<8>) },
    Op { group: "wide32", name: "w8_sub", measures: "Wide32::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide32::sub::<4, 8>(c), kernels::wide32::sub::<8>) },
    Op { group: "wide32", name: "w8_product", measures: "Wide32::product", classes: TEO, prepare: |c| bind(inputs::wide32::product::<4, 8>(c), kernels::wide32::product::<8>) },
    Op { group: "wide32", name: "w8_neg", measures: "Wide32::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide32::neg::<4, 8>(c), kernels::wide32::neg::<8>) },
    Op { group: "wide32", name: "w8_cmp", measures: "Wide32::cmp", classes: TE, prepare: |c| bind(inputs::wide32::cmp::<4, 8>(c), kernels::wide32::cmp::<8>) },
    Op { group: "wide32", name: "w8_signum", measures: "Wide32::signum", classes: T, prepare: |c| bind(inputs::wide32::signum::<4, 8>(c), kernels::wide32::signum::<8>) },
    Op { group: "wide32", name: "w8_to_i128", measures: "Wide32::to_i128", classes: TO, prepare: |c| bind(inputs::wide32::to_i128::<4, 8>(c), kernels::wide32::to_i128::<8>) },
    Op { group: "wide32", name: "w8_from_i128", measures: "Wide32::from_i128", classes: T, prepare: |c| bind(inputs::wide32::from_i128::<8>(c), kernels::wide32::from_i128::<8>) },
    Op { group: "wide32", name: "w10_add", measures: "Wide32::checked_add", classes: TEO, prepare: |c| bind(inputs::wide32::add::<5, 10>(c), kernels::wide32::add::<10>) },
    Op { group: "wide32", name: "w10_sub", measures: "Wide32::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide32::sub::<5, 10>(c), kernels::wide32::sub::<10>) },
    Op { group: "wide32", name: "w10_product", measures: "Wide32::product", classes: TEO, prepare: |c| bind(inputs::wide32::product::<5, 10>(c), kernels::wide32::product::<10>) },
    Op { group: "wide32", name: "w10_neg", measures: "Wide32::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide32::neg::<5, 10>(c), kernels::wide32::neg::<10>) },
    Op { group: "wide32", name: "w10_cmp", measures: "Wide32::cmp", classes: TE, prepare: |c| bind(inputs::wide32::cmp::<5, 10>(c), kernels::wide32::cmp::<10>) },
    Op { group: "wide32", name: "w10_signum", measures: "Wide32::signum", classes: T, prepare: |c| bind(inputs::wide32::signum::<5, 10>(c), kernels::wide32::signum::<10>) },
    Op { group: "wide32", name: "w10_to_i128", measures: "Wide32::to_i128", classes: TO, prepare: |c| bind(inputs::wide32::to_i128::<5, 10>(c), kernels::wide32::to_i128::<10>) },
    Op { group: "wide32", name: "w10_from_i128", measures: "Wide32::from_i128", classes: T, prepare: |c| bind(inputs::wide32::from_i128::<10>(c), kernels::wide32::from_i128::<10>) },
    Op { group: "wide32", name: "w12_add", measures: "Wide32::checked_add", classes: TEO, prepare: |c| bind(inputs::wide32::add::<6, 12>(c), kernels::wide32::add::<12>) },
    Op { group: "wide32", name: "w12_sub", measures: "Wide32::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide32::sub::<6, 12>(c), kernels::wide32::sub::<12>) },
    Op { group: "wide32", name: "w12_product", measures: "Wide32::product", classes: TEO, prepare: |c| bind(inputs::wide32::product::<6, 12>(c), kernels::wide32::product::<12>) },
    Op { group: "wide32", name: "w12_neg", measures: "Wide32::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide32::neg::<6, 12>(c), kernels::wide32::neg::<12>) },
    Op { group: "wide32", name: "w12_cmp", measures: "Wide32::cmp", classes: TE, prepare: |c| bind(inputs::wide32::cmp::<6, 12>(c), kernels::wide32::cmp::<12>) },
    Op { group: "wide32", name: "w12_signum", measures: "Wide32::signum", classes: T, prepare: |c| bind(inputs::wide32::signum::<6, 12>(c), kernels::wide32::signum::<12>) },
    Op { group: "wide32", name: "w12_to_i128", measures: "Wide32::to_i128", classes: TO, prepare: |c| bind(inputs::wide32::to_i128::<6, 12>(c), kernels::wide32::to_i128::<12>) },
    Op { group: "wide32", name: "w12_from_i128", measures: "Wide32::from_i128", classes: T, prepare: |c| bind(inputs::wide32::from_i128::<12>(c), kernels::wide32::from_i128::<12>) },
    Op { group: "wide32", name: "w16_add", measures: "Wide32::checked_add", classes: TEO, prepare: |c| bind(inputs::wide32::add::<8, 16>(c), kernels::wide32::add::<16>) },
    Op { group: "wide32", name: "w16_sub", measures: "Wide32::checked_sub", classes: TEO, prepare: |c| bind(inputs::wide32::sub::<8, 16>(c), kernels::wide32::sub::<16>) },
    Op { group: "wide32", name: "w16_product", measures: "Wide32::product", classes: TEO, prepare: |c| bind(inputs::wide32::product::<8, 16>(c), kernels::wide32::product::<16>) },
    Op { group: "wide32", name: "w16_neg", measures: "Wide32::checked_neg", classes: TEO, prepare: |c| bind(inputs::wide32::neg::<8, 16>(c), kernels::wide32::neg::<16>) },
    Op { group: "wide32", name: "w16_cmp", measures: "Wide32::cmp", classes: TE, prepare: |c| bind(inputs::wide32::cmp::<8, 16>(c), kernels::wide32::cmp::<16>) },
    Op { group: "wide32", name: "w16_signum", measures: "Wide32::signum", classes: T, prepare: |c| bind(inputs::wide32::signum::<8, 16>(c), kernels::wide32::signum::<16>) },
    Op { group: "wide32", name: "w16_to_i128", measures: "Wide32::to_i128", classes: TO, prepare: |c| bind(inputs::wide32::to_i128::<8, 16>(c), kernels::wide32::to_i128::<16>) },
    Op { group: "wide32", name: "w16_from_i128", measures: "Wide32::from_i128", classes: T, prepare: |c| bind(inputs::wide32::from_i128::<16>(c), kernels::wide32::from_i128::<16>) },
    Op { group: "wide32", name: "w8_to_w12", measures: "Wide32::resize", classes: T, prepare: |c| bind(inputs::wide32::resize::<4, 6, 8>(c), kernels::wide32::resize::<8, 12>) },
    Op { group: "wide32", name: "w10_to_w12", measures: "Wide32::resize", classes: T, prepare: |c| bind(inputs::wide32::resize::<5, 6, 10>(c), kernels::wide32::resize::<10, 12>) },
    Op { group: "wide32", name: "w12_to_w8", measures: "Wide32::resize", classes: TO, prepare: |c| bind(inputs::wide32::resize::<6, 4, 12>(c), kernels::wide32::resize::<12, 8>) },
    Op { group: "wide32", name: "w12_to_wide", measures: "Wide32::to_wide", classes: T, prepare: |c| bind(inputs::wide32::to_wide::<6, 12>(c), kernels::wide32::to_wide::<12, 6>) },
    Op { group: "wide32", name: "w3_product_i64", measures: "Wide32::product_i64", classes: TEO, prepare: |c| bind(inputs::wide32::product_i64(c), kernels::wide32::product_i64::<3>) },
    // Planes and the points where three meet.
    Op { group: "plane", name: "through", measures: "Plane::through", classes: TDE, prepare: |c| bind(inputs::plane::through(c), kernels::plane::through) },
    Op { group: "plane", name: "side", measures: "Plane::side", classes: TDNE, prepare: |c| bind(inputs::plane::side(c), kernels::plane::side) },
    Op { group: "plane", name: "tpi_new", measures: "Tpi::new", classes: TDE, prepare: |c| bind(inputs::plane::tpi_new(c), kernels::plane::tpi_new) },
    Op { group: "plane", name: "tpi_homogeneous", measures: "Tpi::homogeneous", classes: TE, prepare: |c| bind(inputs::plane::tpi_homogeneous(c), kernels::plane::tpi_homogeneous) },
    Op { group: "plane", name: "side_of_tpi", measures: "Plane::side_of_tpi", classes: TDNE, prepare: |c| bind(inputs::plane::side_of_tpi(c), kernels::plane::side_of_tpi) },
    Op { group: "plane", name: "to_grid", measures: "Tpi::to_grid", classes: &[Typical, OnGrid, Extreme, Overflow], prepare: |c| bind(inputs::plane::to_grid(c), kernels::plane::to_grid) },
    Op { group: "plane", name: "lpi_new", measures: "Lpi::new", classes: TDE, prepare: |c| bind(inputs::plane::lpi_new(c), kernels::plane::lpi_new) },
    Op { group: "plane", name: "lpi_homogeneous", measures: "Lpi::homogeneous", classes: TE, prepare: |c| bind(inputs::plane::lpi_homogeneous(c), kernels::plane::lpi_homogeneous) },
    Op { group: "plane", name: "side_of_lpi", measures: "Plane::side_of_lpi", classes: TDNE, prepare: |c| bind(inputs::plane::side_of_lpi(c), kernels::plane::side_of_lpi) },
    Op { group: "plane", name: "lpi_to_grid", measures: "Lpi::to_grid", classes: &[Typical, OnGrid, Extreme, Overflow], prepare: |c| bind(inputs::plane::lpi_to_grid(c), kernels::plane::lpi_to_grid) },
    Op { group: "crossing", name: "ssi_new", measures: "Ssi::new", classes: TDE, prepare: |c| bind(inputs::crossing::ssi_new(c), kernels::crossing::ssi_new) },
    Op { group: "crossing", name: "ssi_homogeneous", measures: "Ssi::homogeneous", classes: TE, prepare: |c| bind(inputs::crossing::ssi_homogeneous(c), kernels::crossing::ssi_homogeneous) },
    Op { group: "crossing", name: "ssi_orient2d", measures: "Ssi::orient2d", classes: TDNE, prepare: |c| bind(inputs::crossing::ssi_orient2d(c), kernels::crossing::ssi_orient2d) },
    Op { group: "crossing", name: "ssi_to_grid", measures: "Ssi::to_grid", classes: &[Typical, OnGrid, Extreme, Overflow], prepare: |c| bind(inputs::crossing::ssi_to_grid(c), kernels::crossing::ssi_to_grid) },
    Op { group: "crossing", name: "carrier_side", measures: "Carrier::side", classes: TDNE, prepare: |c| bind(inputs::carrier::side(c), kernels::carrier::side) },
    Op { group: "crossing", name: "carrier_order", measures: "Carrier::order", classes: TDNE, prepare: |c| bind(inputs::carrier::order(c), kernels::carrier::order) },
    Op { group: "crossing", name: "carrier_meet", measures: "Carrier::meet", classes: TDE, prepare: |c| bind(inputs::carrier::meet(c), kernels::carrier::meet) },
    Op { group: "crossing", name: "rational_cmp_x", measures: "Rational2::cmp_x", classes: TDNE, prepare: |c| bind(inputs::carrier::cmp_x(c), kernels::carrier::cmp_x) },
    Op { group: "crossing", name: "rational_orient", measures: "Rational2::orient", classes: TDNE, prepare: |c| bind(inputs::carrier::orient(c), kernels::carrier::orient) },
    Op { group: "crossing", name: "ellipse_side", measures: "Ellipse2::side", classes: TDNE, prepare: |c| bind(inputs::conic::side(c), kernels::conic::side) },
    Op { group: "crossing", name: "ellipse_meets", measures: "Ellipse2::meets", classes: TDNE, prepare: |c| bind(inputs::conic::meets(c), kernels::conic::meets) },
    Op { group: "crossing", name: "crossing_of_ellipse", measures: "Crossing2::of_ellipse", classes: TDE, prepare: |c| bind(inputs::conic::of_ellipse(c), kernels::conic::of_ellipse) },
    Op { group: "crossing", name: "crossing_against_ellipse", measures: "Crossing2::against_ellipse", classes: TDE, prepare: |c| bind(inputs::conic::against_ellipse(c), kernels::conic::against_ellipse) },
    Op { group: "crossing", name: "ellipse_meeting", measures: "Ellipse2::meeting", classes: TDNE, prepare: |c| bind(inputs::conic::meeting(c), kernels::conic::meeting) },
    Op { group: "crossing", name: "quartic_compare", measures: "quartic::compare", classes: TDE, prepare: |c| bind(inputs::conic::compare(c), kernels::conic::compare) },
    Op { group: "crossing", name: "round_side_of_grid", measures: "Round::side_of_grid", classes: TDNE, prepare: |c| bind(inputs::arc::round_grid(c), kernels::arc::round_grid) },
    Op { group: "crossing", name: "round_side", measures: "Round::side", classes: TDNE, prepare: |c| bind(inputs::arc::round_side(c), kernels::arc::round_side) },
    Op { group: "crossing", name: "round_meets", measures: "Round::meets", classes: TDNE, prepare: |c| bind(inputs::arc::round_meets(c), kernels::arc::round_meets) },
    Op { group: "crossing", name: "circle_side", measures: "Circle2::side", classes: TDNE, prepare: |c| bind(inputs::arc::circle_side(c), kernels::arc::circle_side) },
    Op { group: "crossing", name: "circle_meets", measures: "Circle2::meets", classes: TDNE, prepare: |c| bind(inputs::arc::circle_meets(c), kernels::arc::circle_meets) },
    Op { group: "crossing", name: "crossing_of_circle", measures: "Crossing2::of_circle", classes: TDE, prepare: |c| bind(inputs::one_root::of_circle(c), kernels::one_root::of_circle) },
    Op { group: "crossing", name: "crossing_of_round", measures: "Crossing2::of_round", classes: TD, prepare: |c| bind(inputs::one_root::of_round(c), kernels::one_root::of_round) },
    Op { group: "crossing", name: "crossing_cmp_x", measures: "Crossing2::cmp_x", classes: TDNE, prepare: |c| bind(inputs::one_root::cmp_x(c), kernels::one_root::cmp_x) },
    Op { group: "crossing", name: "crossing_side", measures: "Crossing2::side", classes: TDNE, prepare: |c| bind(inputs::one_root::side(c), kernels::one_root::side) },
    Op { group: "crossing", name: "crossing_against_circle", measures: "Crossing2::against_circle", classes: TDE, prepare: |c| bind(inputs::one_root::against_circle(c), kernels::one_root::against_circle) },
    Op { group: "crossing", name: "arc_order_crossings", measures: "Arc2::order_crossings", classes: &[Typical], prepare: |c| bind(inputs::one_root::order_crossings(c), kernels::one_root::order_crossings) },
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
    Op { group: "interval", name: "sinpi", measures: "Interval::sinpi", classes: TE, prepare: |c| bind(inputs::interval::sinpi(c), kernels::interval::sinpi) },
    Op { group: "interval", name: "cospi", measures: "Interval::cospi", classes: TE, prepare: |c| bind(inputs::interval::cospi(c), kernels::interval::cospi) },
    Op { group: "interval", name: "exp", measures: "Interval::exp", classes: TE, prepare: |c| bind(inputs::interval::exp(c), kernels::interval::exp) },
    Op { group: "interval", name: "ln", measures: "Interval::ln", classes: TE, prepare: |c| bind(inputs::interval::ln(c), kernels::interval::ln) },
    Op { group: "interval", name: "atan2", measures: "Interval::atan2", classes: TE, prepare: |c| bind(inputs::interval::atan2(c), kernels::interval::atan2) },
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
    // Deterministic maths, with proven error bounds.
    Op { group: "math", name: "sqrt", measures: "math::sqrt", classes: TE, prepare: |c| bind(inputs::math::sqrt(c), kernels::math::sqrt) },
    Op { group: "math", name: "sinpi", measures: "math::sinpi", classes: TE, prepare: |c| bind(inputs::math::sinpi(c), kernels::math::sinpi) },
    Op { group: "math", name: "cospi", measures: "math::cospi", classes: TE, prepare: |c| bind(inputs::math::cospi(c), kernels::math::cospi) },
    Op { group: "math", name: "exp", measures: "math::exp", classes: TE, prepare: |c| bind(inputs::math::exp(c), kernels::math::exp) },
    Op { group: "math", name: "ln", measures: "math::ln", classes: TE, prepare: |c| bind(inputs::math::ln(c), kernels::math::ln) },
    Op { group: "math", name: "atan2", measures: "math::atan2", classes: TE, prepare: |c| bind(inputs::math::atan2(c), kernels::math::atan2) },
    Op { group: "math", name: "fma", measures: "math::fma", classes: TE, prepare: |c| bind(inputs::math::fma(c), kernels::math::fma) },
    Op { group: "math", name: "abs", measures: "math::abs", classes: TE, prepare: |c| bind(inputs::math::abs(c), kernels::math::abs) },
    Op { group: "math", name: "floor", measures: "math::floor", classes: TE, prepare: |c| bind(inputs::math::floor(c), kernels::math::floor) },
    Op { group: "math", name: "ceil", measures: "math::ceil", classes: TE, prepare: |c| bind(inputs::math::ceil(c), kernels::math::ceil) },
    Op { group: "math", name: "round_ties_even", measures: "math::round_ties_even", classes: TE, prepare: |c| bind(inputs::math::round_ties_even(c), kernels::math::round_ties_even) },
    // The maths functions without a proven error bound.
    Op { group: "unproven", name: "sin", measures: "unproven::sin", classes: TE, prepare: |c| bind(inputs::math::unproven::sin(c), kernels::math::unproven::sin) },
    Op { group: "unproven", name: "cos", measures: "unproven::cos", classes: TE, prepare: |c| bind(inputs::math::unproven::cos(c), kernels::math::unproven::cos) },
    Op { group: "unproven", name: "tan", measures: "unproven::tan", classes: TE, prepare: |c| bind(inputs::math::unproven::tan(c), kernels::math::unproven::tan) },
    Op { group: "unproven", name: "asin", measures: "unproven::asin", classes: TE, prepare: |c| bind(inputs::math::unproven::asin(c), kernels::math::unproven::asin) },
    Op { group: "unproven", name: "acos", measures: "unproven::acos", classes: TE, prepare: |c| bind(inputs::math::unproven::acos(c), kernels::math::unproven::acos) },
    Op { group: "unproven", name: "atan", measures: "unproven::atan", classes: TE, prepare: |c| bind(inputs::math::unproven::atan(c), kernels::math::unproven::atan) },
    Op { group: "unproven", name: "atan2", measures: "unproven::atan2", classes: TE, prepare: |c| bind(inputs::math::unproven::atan2(c), kernels::math::unproven::atan2) },
    Op { group: "unproven", name: "exp", measures: "unproven::exp", classes: TE, prepare: |c| bind(inputs::math::unproven::exp(c), kernels::math::unproven::exp) },
    Op { group: "unproven", name: "ln", measures: "unproven::ln", classes: TE, prepare: |c| bind(inputs::math::unproven::ln(c), kernels::math::unproven::ln) },
    Op { group: "unproven", name: "log2", measures: "unproven::log2", classes: TE, prepare: |c| bind(inputs::math::unproven::log2(c), kernels::math::unproven::log2) },
    Op { group: "unproven", name: "log10", measures: "unproven::log10", classes: TE, prepare: |c| bind(inputs::math::unproven::log10(c), kernels::math::unproven::log10) },
    Op { group: "unproven", name: "pow", measures: "unproven::pow", classes: TE, prepare: |c| bind(inputs::math::unproven::pow(c), kernels::math::unproven::pow) },
    Op { group: "unproven", name: "powi", measures: "unproven::powi", classes: TE, prepare: |c| bind(inputs::math::unproven::powi(c), kernels::math::unproven::powi) },
    Op { group: "unproven", name: "hypot", measures: "unproven::hypot", classes: TE, prepare: |c| bind(inputs::math::unproven::hypot(c), kernels::math::unproven::hypot) },
    Op { group: "unproven", name: "cbrt", measures: "unproven::cbrt", classes: TE, prepare: |c| bind(inputs::math::unproven::cbrt(c), kernels::math::unproven::cbrt) },
    Op { group: "unproven", name: "sinpi", measures: "unproven::sinpi", classes: TE, prepare: |c| bind(inputs::math::unproven::sinpi(c), kernels::math::unproven::sinpi) },
    Op { group: "unproven", name: "cospi", measures: "unproven::cospi", classes: TE, prepare: |c| bind(inputs::math::unproven::cospi(c), kernels::math::unproven::cospi) },
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
    // Double-double arithmetic.
    Op { group: "dd", name: "two_prod_fma", measures: "dd::two_prod_fma", classes: TE, prepare: |c| bind(inputs::dd::two_prod_fma(c), kernels::dd::two_prod_fma) },
    Op { group: "dd", name: "from_sum", measures: "DD::from_sum", classes: TE, prepare: |c| bind(inputs::dd::from_sum(c), kernels::dd::from_sum) },
    Op { group: "dd", name: "exact_product", measures: "DD::exact_product", classes: TE, prepare: |c| bind(inputs::dd::exact_product(c), kernels::dd::exact_product) },
    Op { group: "dd", name: "from_i64", measures: "DD::from_i64", classes: T, prepare: |c| bind(inputs::dd::from_i64(c), kernels::dd::from_i64) },
    Op { group: "dd", name: "from_i128", measures: "DD::from_i128", classes: TO, prepare: |c| bind(inputs::dd::from_i128(c), kernels::dd::from_i128) },
    Op { group: "dd", name: "sign", measures: "DD::sign", classes: T, prepare: |c| bind(inputs::dd::sign(c), kernels::dd::sign) },
    Op { group: "dd", name: "negate", measures: "DD::negate", classes: T, prepare: |c| bind(inputs::dd::negate(c), kernels::dd::negate) },
    Op { group: "dd", name: "abs", measures: "DD::abs", classes: T, prepare: |c| bind(inputs::dd::abs(c), kernels::dd::abs) },
    Op { group: "dd", name: "add", measures: "DD::add", classes: TE, prepare: |c| bind(inputs::dd::add(c), kernels::dd::add) },
    Op { group: "dd", name: "add_f64", measures: "DD::add_f64", classes: TE, prepare: |c| bind(inputs::dd::add_f64(c), kernels::dd::add_f64) },
    Op { group: "dd", name: "mul_f64", measures: "DD::mul_f64", classes: TE, prepare: |c| bind(inputs::dd::mul_f64(c), kernels::dd::mul_f64) },
    Op { group: "dd", name: "mul", measures: "DD::mul", classes: TE, prepare: |c| bind(inputs::dd::mul(c), kernels::dd::mul) },
    Op { group: "dd", name: "div_f64", measures: "DD::div_f64", classes: TE, prepare: |c| bind(inputs::dd::div_f64(c), kernels::dd::div_f64) },
    Op { group: "dd", name: "div", measures: "DD::div", classes: TE, prepare: |c| bind(inputs::dd::div(c), kernels::dd::div) },
    Op { group: "dd", name: "sqrt", measures: "DD::sqrt", classes: TE, prepare: |c| bind(inputs::dd::sqrt(c), kernels::dd::sqrt) },
    Op { group: "dd", name: "round_shifted", measures: "DD::round_shifted", classes: TEO, prepare: |c| bind(inputs::dd::round_shifted(c), kernels::dd::round_shifted) },
    Op { group: "dd", name: "cmp", measures: "DD::cmp", classes: T, prepare: |c| bind(inputs::dd::cmp(c), kernels::dd::cmp) },
    // The exact sum of doubles.
    Op { group: "exact_sum", name: "add", measures: "ExactSum::add", classes: TE, prepare: |c| bind(inputs::exact_sum::add(c), kernels::exact_sum::add) },
    Op { group: "exact_sum", name: "add_slice", measures: "ExactSum::add_slice", classes: TE, prepare: |c| bind(inputs::exact_sum::add_slice(c), kernels::exact_sum::add_slice) },
    Op { group: "exact_sum", name: "add_iter", measures: "ExactSum::add_iter", classes: TE, prepare: |c| bind(inputs::exact_sum::add_iter(c), kernels::exact_sum::add_iter) },
    Op { group: "exact_sum", name: "merge", measures: "ExactSum::merge", classes: TE, prepare: |c| bind(inputs::exact_sum::merge(c), kernels::exact_sum::merge) },
    Op { group: "exact_sum", name: "round", measures: "ExactSum::round", classes: TE, prepare: |c| bind(inputs::exact_sum::round(c), kernels::exact_sum::round) },
    Op { group: "exact_sum", name: "round_ieee", measures: "ExactSum::round_ieee", classes: TE, prepare: |c| bind(inputs::exact_sum::round_ieee(c), kernels::exact_sum::round_ieee) },
    // The generated filters and exact values.
    Op { group: "generated", name: "orient2d_filter", measures: "generated::orient2d::filter", classes: TDNE, prepare: |c| bind(inputs::generated::orient2d_filter(c), kernels::generated::orient2d_filter) },
    Op { group: "generated", name: "orient2d_value", measures: "generated::orient2d::value", classes: TDNE, prepare: |c| bind(inputs::generated::orient2d_value(c), kernels::generated::orient2d_value) },
    Op { group: "generated", name: "orient3d_filter", measures: "generated::orient3d::filter", classes: TDNE, prepare: |c| bind(inputs::generated::orient3d_filter(c), kernels::generated::orient3d_filter) },
    Op { group: "generated", name: "orient3d_value", measures: "generated::orient3d::value", classes: TDNE, prepare: |c| bind(inputs::generated::orient3d_value(c), kernels::generated::orient3d_value) },
    Op { group: "generated", name: "incircle_filter", measures: "generated::incircle::filter", classes: TDNE, prepare: |c| bind(inputs::generated::incircle_filter(c), kernels::generated::incircle_filter) },
    Op { group: "generated", name: "incircle_value", measures: "generated::incircle::value", classes: TDNE, prepare: |c| bind(inputs::generated::incircle_value(c), kernels::generated::incircle_value) },
    Op { group: "generated", name: "insphere_filter", measures: "generated::insphere::filter", classes: TDNE, prepare: |c| bind(inputs::generated::insphere_filter(c), kernels::generated::insphere_filter) },
    Op { group: "generated", name: "insphere_value", measures: "generated::insphere::value", classes: TDNE, prepare: |c| bind(inputs::generated::insphere_value(c), kernels::generated::insphere_value) },
    Op { group: "generated", name: "plane_side_filter", measures: "generated::plane_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::plane_side_filter(c), kernels::generated::plane_side_filter) },
    Op { group: "generated", name: "plane_side_value", measures: "generated::plane_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::plane_side_value(c), kernels::generated::plane_side_value) },
    Op { group: "generated", name: "tpi_weight_filter", measures: "generated::tpi_weight::filter", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_weight_filter(c), kernels::generated::tpi_weight_filter) },
    Op { group: "generated", name: "tpi_weight_value", measures: "generated::tpi_weight::value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_weight_value(c), kernels::generated::tpi_weight_value) },
    Op { group: "generated", name: "tpi_side_filter", measures: "generated::tpi_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_side_filter(c), kernels::generated::tpi_side_filter) },
    Op { group: "generated", name: "tpi_side_value", measures: "generated::tpi_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_side_value(c), kernels::generated::tpi_side_value) },
    Op { group: "generated", name: "lpi_weight_filter", measures: "generated::lpi_weight::filter", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_weight_filter(c), kernels::generated::lpi_weight_filter) },
    Op { group: "generated", name: "lpi_weight_value", measures: "generated::lpi_weight::value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_weight_value(c), kernels::generated::lpi_weight_value) },
    Op { group: "generated", name: "lpi_side_filter", measures: "generated::lpi_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_side_filter(c), kernels::generated::lpi_side_filter) },
    Op { group: "generated", name: "lpi_side_value", measures: "generated::lpi_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_side_value(c), kernels::generated::lpi_side_value) },
    Op { group: "generated", name: "ssi_weight_filter", measures: "generated::ssi_weight::filter", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_weight_filter(c), kernels::generated::ssi_weight_filter) },
    Op { group: "generated", name: "ssi_weight_value", measures: "generated::ssi_weight::value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_weight_value(c), kernels::generated::ssi_weight_value) },
    Op { group: "generated", name: "ssi_side_filter", measures: "generated::ssi_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_side_filter(c), kernels::generated::ssi_side_filter) },
    Op { group: "generated", name: "ssi_side_value", measures: "generated::ssi_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_side_value(c), kernels::generated::ssi_side_value) },
    Op { group: "generated", name: "carrier_side_filter", measures: "generated::carrier_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_side_filter(c), kernels::generated::carrier_side_filter) },
    Op { group: "generated", name: "carrier_side_value", measures: "generated::carrier_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_side_value(c), kernels::generated::carrier_side_value) },
    Op { group: "generated", name: "carrier_order_filter", measures: "generated::carrier_order::filter", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_order_filter(c), kernels::generated::carrier_order_filter) },
    Op { group: "generated", name: "carrier_order_value", measures: "generated::carrier_order::value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_order_value(c), kernels::generated::carrier_order_value) },
    Op { group: "generated", name: "rational_cmp_filter", measures: "generated::rational_cmp::filter", classes: TDNE, prepare: |c| bind(inputs::generated::rational_cmp_filter(c), kernels::generated::rational_cmp_filter) },
    Op { group: "generated", name: "rational_cmp_value", measures: "generated::rational_cmp::value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_cmp_value(c), kernels::generated::rational_cmp_value) },
    Op { group: "generated", name: "rational_orient_filter", measures: "generated::rational_orient::filter", classes: TDNE, prepare: |c| bind(inputs::generated::rational_orient_filter(c), kernels::generated::rational_orient_filter) },
    Op { group: "generated", name: "rational_orient_value", measures: "generated::rational_orient::value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_orient_value(c), kernels::generated::rational_orient_value) },
    Op { group: "generated", name: "ellipse_side_filter", measures: "generated::ellipse_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_side_filter(c), kernels::generated::ellipse_side_filter) },
    Op { group: "generated", name: "ellipse_side_value", measures: "generated::ellipse_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_side_value(c), kernels::generated::ellipse_side_value) },
    Op { group: "generated", name: "ellipse_line_filter", measures: "generated::ellipse_line::filter", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_line_filter(c), kernels::generated::ellipse_line_filter) },
    Op { group: "generated", name: "ellipse_line_value", measures: "generated::ellipse_line::value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_line_value(c), kernels::generated::ellipse_line_value) },
    Op { group: "generated", name: "round_grid_filter", measures: "generated::round_grid::filter", classes: TDNE, prepare: |c| bind(inputs::generated::round_grid_filter(c), kernels::generated::round_grid_filter) },
    Op { group: "generated", name: "round_grid_value", measures: "generated::round_grid::value", classes: TDNE, prepare: |c| bind(inputs::generated::round_grid_value(c), kernels::generated::round_grid_value) },
    Op { group: "generated", name: "round_rational_filter", measures: "generated::round_rational::filter", classes: TDNE, prepare: |c| bind(inputs::generated::round_rational_filter(c), kernels::generated::round_rational_filter) },
    Op { group: "generated", name: "round_rational_value", measures: "generated::round_rational::value", classes: TDNE, prepare: |c| bind(inputs::generated::round_rational_value(c), kernels::generated::round_rational_value) },
    Op { group: "generated", name: "round_line_filter", measures: "generated::round_line::filter", classes: TDNE, prepare: |c| bind(inputs::generated::round_line_filter(c), kernels::generated::round_line_filter) },
    Op { group: "generated", name: "round_line_value", measures: "generated::round_line::value", classes: TDNE, prepare: |c| bind(inputs::generated::round_line_value(c), kernels::generated::round_line_value) },
    Op { group: "generated", name: "circle_side_filter", measures: "generated::circle_side::filter", classes: TDNE, prepare: |c| bind(inputs::generated::circle_side_filter(c), kernels::generated::circle_side_filter) },
    Op { group: "generated", name: "circle_side_value", measures: "generated::circle_side::value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_side_value(c), kernels::generated::circle_side_value) },
    Op { group: "generated", name: "circle_line_filter", measures: "generated::circle_line::filter", classes: TDNE, prepare: |c| bind(inputs::generated::circle_line_filter(c), kernels::generated::circle_line_filter) },
    Op { group: "generated", name: "circle_line_value", measures: "generated::circle_line::value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_line_value(c), kernels::generated::circle_line_value) },
    Op { group: "generated", name: "circle_gap_filter", measures: "generated::circle_gap::filter", classes: TDNE, prepare: |c| bind(inputs::generated::circle_gap_filter(c), kernels::generated::circle_gap_filter) },
    Op { group: "generated", name: "circle_gap_value", measures: "generated::circle_gap::value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_gap_value(c), kernels::generated::circle_gap_value) },
    // The plain-i128 variant of each exact path (chapter 23.3; piece G42), on inputs of the same
    // classes as the shipped value's.
    Op { group: "generated", name: "orient2d_value_plain", measures: "generated::plain::orient2d_value", classes: TDNE, prepare: |c| bind(inputs::generated::orient2d_value_plain(c), kernels::plain::orient2d_value) },
    Op { group: "generated", name: "orient3d_value_plain", measures: "generated::plain::orient3d_value", classes: TDNE, prepare: |c| bind(inputs::generated::orient3d_value_plain(c), kernels::plain::orient3d_value) },
    Op { group: "generated", name: "incircle_value_plain", measures: "generated::plain::incircle_value", classes: TDNE, prepare: |c| bind(inputs::generated::incircle_value_plain(c), kernels::plain::incircle_value) },
    Op { group: "generated", name: "insphere_value_plain", measures: "generated::plain::insphere_value", classes: TDNE, prepare: |c| bind(inputs::generated::insphere_value_plain(c), kernels::plain::insphere_value) },
    Op { group: "generated", name: "plane_side_value_plain", measures: "generated::plain::plane_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::plane_side_value_plain(c), kernels::plain::plane_side_value) },
    Op { group: "generated", name: "tpi_weight_value_plain", measures: "generated::plain::tpi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_weight_value_plain(c), kernels::plain::tpi_weight_value) },
    Op { group: "generated", name: "tpi_side_value_plain", measures: "generated::plain::tpi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_side_value_plain(c), kernels::plain::tpi_side_value) },
    Op { group: "generated", name: "lpi_weight_value_plain", measures: "generated::plain::lpi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_weight_value_plain(c), kernels::plain::lpi_weight_value) },
    Op { group: "generated", name: "lpi_side_value_plain", measures: "generated::plain::lpi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_side_value_plain(c), kernels::plain::lpi_side_value) },
    Op { group: "generated", name: "ssi_weight_value_plain", measures: "generated::plain::ssi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_weight_value_plain(c), kernels::plain::ssi_weight_value) },
    Op { group: "generated", name: "ssi_side_value_plain", measures: "generated::plain::ssi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_side_value_plain(c), kernels::plain::ssi_side_value) },
    Op { group: "generated", name: "carrier_side_value_plain", measures: "generated::plain::carrier_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_side_value_plain(c), kernels::plain::carrier_side_value) },
    Op { group: "generated", name: "carrier_order_value_plain", measures: "generated::plain::carrier_order_value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_order_value_plain(c), kernels::plain::carrier_order_value) },
    Op { group: "generated", name: "rational_cmp_value_plain", measures: "generated::plain::rational_cmp_value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_cmp_value_plain(c), kernels::plain::rational_cmp_value) },
    Op { group: "generated", name: "rational_orient_value_plain", measures: "generated::plain::rational_orient_value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_orient_value_plain(c), kernels::plain::rational_orient_value) },
    Op { group: "generated", name: "ellipse_side_value_plain", measures: "generated::plain::ellipse_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_side_value_plain(c), kernels::plain::ellipse_side_value) },
    Op { group: "generated", name: "ellipse_line_value_plain", measures: "generated::plain::ellipse_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_line_value_plain(c), kernels::plain::ellipse_line_value) },
    Op { group: "generated", name: "round_grid_value_plain", measures: "generated::plain::round_grid_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_grid_value_plain(c), kernels::plain::round_grid_value) },
    Op { group: "generated", name: "round_rational_value_plain", measures: "generated::plain::round_rational_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_rational_value_plain(c), kernels::plain::round_rational_value) },
    Op { group: "generated", name: "round_line_value_plain", measures: "generated::plain::round_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_line_value_plain(c), kernels::plain::round_line_value) },
    Op { group: "generated", name: "circle_side_value_plain", measures: "generated::plain::circle_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_side_value_plain(c), kernels::plain::circle_side_value) },
    Op { group: "generated", name: "circle_line_value_plain", measures: "generated::plain::circle_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_line_value_plain(c), kernels::plain::circle_line_value) },
    Op { group: "generated", name: "circle_gap_value_plain", measures: "generated::plain::circle_gap_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_gap_value_plain(c), kernels::plain::circle_gap_value) },
    // Avnaim et al.'s sign of orient2d, chapter 23.3's reference (piece G42).
    Op { group: "generated", name: "orient2d_avnaim", measures: "avnaim::orient2d", classes: TDNE, prepare: |c| bind(inputs::generated::orient2d_avnaim(c), kernels::avnaim::orient2d) },
    Op { group: "generated", name: "orient3d_avnaim", measures: "avnaim::orient3d", classes: TDNE, prepare: |c| bind(inputs::generated::orient3d_avnaim(c), kernels::avnaim::orient3d) },
    // The width-aware limb variant of each exact path (chapter 23.3; piece G42).
    Op { group: "generated", name: "orient2d_value_limbs", measures: "generated::limbs::orient2d_value", classes: TDNE, prepare: |c| bind(inputs::generated::orient2d_value_limbs(c), kernels::limbs::orient2d_value) },
    Op { group: "generated", name: "orient3d_value_limbs", measures: "generated::limbs::orient3d_value", classes: TDNE, prepare: |c| bind(inputs::generated::orient3d_value_limbs(c), kernels::limbs::orient3d_value) },
    Op { group: "generated", name: "incircle_value_limbs", measures: "generated::limbs::incircle_value", classes: TDNE, prepare: |c| bind(inputs::generated::incircle_value_limbs(c), kernels::limbs::incircle_value) },
    Op { group: "generated", name: "insphere_value_limbs", measures: "generated::limbs::insphere_value", classes: TDNE, prepare: |c| bind(inputs::generated::insphere_value_limbs(c), kernels::limbs::insphere_value) },
    Op { group: "generated", name: "plane_side_value_limbs", measures: "generated::limbs::plane_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::plane_side_value_limbs(c), kernels::limbs::plane_side_value) },
    Op { group: "generated", name: "tpi_weight_value_limbs", measures: "generated::limbs::tpi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_weight_value_limbs(c), kernels::limbs::tpi_weight_value) },
    Op { group: "generated", name: "tpi_side_value_limbs", measures: "generated::limbs::tpi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::tpi_side_value_limbs(c), kernels::limbs::tpi_side_value) },
    Op { group: "generated", name: "lpi_weight_value_limbs", measures: "generated::limbs::lpi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_weight_value_limbs(c), kernels::limbs::lpi_weight_value) },
    Op { group: "generated", name: "lpi_side_value_limbs", measures: "generated::limbs::lpi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::lpi_side_value_limbs(c), kernels::limbs::lpi_side_value) },
    Op { group: "generated", name: "ssi_weight_value_limbs", measures: "generated::limbs::ssi_weight_value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_weight_value_limbs(c), kernels::limbs::ssi_weight_value) },
    Op { group: "generated", name: "ssi_side_value_limbs", measures: "generated::limbs::ssi_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::ssi_side_value_limbs(c), kernels::limbs::ssi_side_value) },
    Op { group: "generated", name: "carrier_side_value_limbs", measures: "generated::limbs::carrier_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_side_value_limbs(c), kernels::limbs::carrier_side_value) },
    Op { group: "generated", name: "carrier_order_value_limbs", measures: "generated::limbs::carrier_order_value", classes: TDNE, prepare: |c| bind(inputs::generated::carrier_order_value_limbs(c), kernels::limbs::carrier_order_value) },
    Op { group: "generated", name: "rational_cmp_value_limbs", measures: "generated::limbs::rational_cmp_value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_cmp_value_limbs(c), kernels::limbs::rational_cmp_value) },
    Op { group: "generated", name: "rational_orient_value_limbs", measures: "generated::limbs::rational_orient_value", classes: TDNE, prepare: |c| bind(inputs::generated::rational_orient_value_limbs(c), kernels::limbs::rational_orient_value) },
    Op { group: "generated", name: "ellipse_side_value_limbs", measures: "generated::limbs::ellipse_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_side_value_limbs(c), kernels::limbs::ellipse_side_value) },
    Op { group: "generated", name: "ellipse_line_value_limbs", measures: "generated::limbs::ellipse_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::ellipse_line_value_limbs(c), kernels::limbs::ellipse_line_value) },
    Op { group: "generated", name: "round_grid_value_limbs", measures: "generated::limbs::round_grid_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_grid_value_limbs(c), kernels::limbs::round_grid_value) },
    Op { group: "generated", name: "round_rational_value_limbs", measures: "generated::limbs::round_rational_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_rational_value_limbs(c), kernels::limbs::round_rational_value) },
    Op { group: "generated", name: "round_line_value_limbs", measures: "generated::limbs::round_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::round_line_value_limbs(c), kernels::limbs::round_line_value) },
    Op { group: "generated", name: "circle_side_value_limbs", measures: "generated::limbs::circle_side_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_side_value_limbs(c), kernels::limbs::circle_side_value) },
    Op { group: "generated", name: "circle_line_value_limbs", measures: "generated::limbs::circle_line_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_line_value_limbs(c), kernels::limbs::circle_line_value) },
    Op { group: "generated", name: "circle_gap_value_limbs", measures: "generated::limbs::circle_gap_value", classes: TDNE, prepare: |c| bind(inputs::generated::circle_gap_value_limbs(c), kernels::limbs::circle_gap_value) },
    Op { group: "perturb", name: "orient2d", measures: "perturb::orient2d", classes: TD, prepare: |c| bind(inputs::perturb::orient2d(c), kernels::perturb::orient2d) },
    Op { group: "perturb", name: "orient3d", measures: "perturb::orient3d", classes: TD, prepare: |c| bind(inputs::perturb::orient3d(c), kernels::perturb::orient3d) },
    Op { group: "perturb", name: "incircle", measures: "perturb::incircle", classes: TD, prepare: |c| bind(inputs::perturb::incircle(c), kernels::perturb::incircle) },
    Op { group: "perturb", name: "insphere", measures: "perturb::insphere", classes: TD, prepare: |c| bind(inputs::perturb::insphere(c), kernels::perturb::insphere) },
    // The interval Newton operators, one step each, and the Jacobians they rest on (G52).
    Op { group: "newton", name: "krawczyk_2", measures: "newton::krawczyk", classes: TE, prepare: |c| bind(inputs::newton::krawczyk_2(c), kernels::newton::krawczyk_2) },
    Op { group: "newton", name: "hansen_sengupta_2", measures: "newton::hansen_sengupta", classes: TE, prepare: |c| bind(inputs::newton::hansen_sengupta_2(c), kernels::newton::hansen_sengupta_2) },
    Op { group: "newton", name: "krawczyk_4", measures: "newton::krawczyk", classes: TE, prepare: |c| bind(inputs::newton::krawczyk_4(c), kernels::newton::krawczyk_4) },
    Op { group: "newton", name: "hansen_sengupta_4", measures: "newton::hansen_sengupta", classes: TE, prepare: |c| bind(inputs::newton::hansen_sengupta_4(c), kernels::newton::hansen_sengupta_4) },
    Op { group: "dual", name: "jacobian_real", measures: "dual::jacobian", classes: TE, prepare: |c| bind(inputs::newton::jacobian_real(c), kernels::dual::jacobian_real) },
    Op { group: "dual", name: "jacobian_interval", measures: "dual::jacobian", classes: TE, prepare: |c| bind(inputs::newton::jacobian_interval(c), kernels::dual::jacobian_interval) },
    // The root-isolation engine, each domain isolated whole, and the double-double balls it falls
    // back to (G53).
    Op { group: "isolate", name: "cubic_1", measures: "isolate::isolate", classes: TE, prepare: |c| bind(inputs::isolate::cubic_1(c), kernels::isolate::cubic_1) },
    Op { group: "isolate", name: "circle_line_2", measures: "isolate::isolate", classes: TE, prepare: |c| bind(inputs::isolate::circle_line_2(c), kernels::isolate::circle_line_2) },
    Op { group: "ball", name: "cubic", measures: "Ball::from_interval", classes: TE, prepare: |c| bind(inputs::isolate::ball_cubic(c), kernels::ball::cubic) },
    // geom-math's hot paths (pieces G45, G46 and G48; decision 32.44): the typed transforms, the
    // rational rotation and its search, a frame's map conversion and the decimal reader.
    Op { group: "math_types", name: "apply_exact", measures: "geom_math::TypedTransform::apply", classes: T, prepare: |c| bind(inputs::math_types::apply_exact(c), kernels::math_types::apply_exact) },
    Op { group: "math_types", name: "apply_rational", measures: "geom_math::TypedTransform::apply", classes: T, prepare: |c| bind(inputs::math_types::apply_rational(c), kernels::math_types::apply_rational) },
    Op { group: "math_types", name: "apply_certified", measures: "geom_math::TypedTransform::apply", classes: T, prepare: |c| bind(inputs::math_types::apply_certified(c), kernels::math_types::apply_certified) },
    Op { group: "math_types", name: "rotation_apply", measures: "geom_math::RationalRotation::apply", classes: T, prepare: |c| bind(inputs::math_types::rotation_apply(c), kernels::math_types::rotation_apply) },
    Op { group: "math_types", name: "plane_within", measures: "geom_math::RationalRotation::plane_within", classes: T, prepare: |c| bind(inputs::math_types::plane_within(c), kernels::math_types::plane_within) },
    Op { group: "math_types", name: "to_map", measures: "geom_math::FrameRecord::to_map", classes: T, prepare: |c| bind(inputs::math_types::to_map(c), kernels::math_types::to_map) },
    // geom-linalg's hot paths (pieces G49 and G50; decision 32.44): the pivoted QR and the SVD.
    Op { group: "linalg", name: "pivoted_qr", measures: "geom_linalg::PivotedQr::factor", classes: T, prepare: |c| bind(inputs::linalg::pivoted_qr(c), kernels::linalg::pivoted_qr) },
    Op { group: "linalg", name: "svd_jacobi", measures: "geom_linalg::Svd::jacobi", classes: T, prepare: |c| bind(inputs::linalg::svd_jacobi(c), kernels::linalg::svd_jacobi) },
    // geom-bvh's hot paths (pieces G54 and G55; decision 32.44): the box tree and the cell index,
    // each built for a scene and queried.
    Op { group: "bvh", name: "tree_build_query", measures: "geom_bvh::BoxTree::build", classes: T, prepare: |c| bind(inputs::bvh::tree_build_query(c), kernels::bvh::tree_build_query) },
    Op { group: "bvh", name: "cells_within", measures: "geom_bvh::CellIndex::build", classes: T, prepare: |c| bind(inputs::bvh::cells_within(c), kernels::bvh::cells_within) },
    // geom-nurbs's hot paths (pieces G56, G58 and G59; decision 32.44): evaluation, interpolation
    // and the Bernstein bound.
    Op { group: "nurbs", name: "curve_point", measures: "geom_nurbs::Curve::point", classes: T, prepare: |c| bind(inputs::nurbs::curve_point(c), kernels::nurbs::curve_point) },
    Op { group: "nurbs", name: "interpolate", measures: "geom_nurbs::Curve::interpolate", classes: T, prepare: |c| bind(inputs::nurbs::interpolate(c), kernels::nurbs::interpolate) },
    Op { group: "nurbs", name: "bernstein_bound", measures: "geom_nurbs::BernsteinCurve::bound", classes: T, prepare: |c| bind(inputs::nurbs::bernstein_bound(c), kernels::nurbs::bernstein_bound) },
    // geom-curve's hot paths (pieces G60 to G64 and G67; decision 32.44), through the kind
    // contract: a helix's point and enclosure, a bulge polyline's and a transition's conversion to
    // NURBS, and a transition's point.
    Op { group: "curve", name: "helix_point", measures: "geom_curve::CurveKind::point", classes: T, prepare: |c| bind(inputs::curve::helix_point(c), kernels::curve::helix_point) },
    Op { group: "curve", name: "helix_enclose", measures: "geom_curve::CurveKind::enclose", classes: T, prepare: |c| bind(inputs::curve::helix_enclose(c), kernels::curve::helix_enclose) },
    Op { group: "curve", name: "bulge_to_nurbs", measures: "geom_curve::CurveKind::to_nurbs", classes: T, prepare: |c| bind(inputs::curve::bulge_to_nurbs(c), kernels::curve::bulge_to_nurbs) },
    Op { group: "curve", name: "transition_point", measures: "geom_curve::Transition::point", classes: T, prepare: |c| bind(inputs::curve::transition_point(c), kernels::curve::transition_point) },
    Op { group: "curve", name: "transition_to_nurbs", measures: "geom_curve::Transition::to_nurbs", classes: T, prepare: |c| bind(inputs::curve::transition_to_nurbs(c), kernels::curve::transition_to_nurbs) },
    // geom-surface's hot paths (pieces G65 to G68; decision 32.44), through the kind contract: a
    // torus's point and enclosure, and an offset cylinder's point.
    Op { group: "surface", name: "torus_point", measures: "geom_surface::SurfaceKind::point", classes: T, prepare: |c| bind(inputs::surface::torus_point(c), kernels::surface::torus_point) },
    Op { group: "surface", name: "torus_enclose", measures: "geom_surface::SurfaceKind::enclose", classes: T, prepare: |c| bind(inputs::surface::torus_enclose(c), kernels::surface::torus_enclose) },
    Op { group: "surface", name: "offset_point", measures: "geom_surface::SurfaceKind::point", classes: T, prepare: |c| bind(inputs::surface::offset_point(c), kernels::surface::offset_point) },
    Op { group: "surface", name: "torus_second_bound", measures: "geom_surface::ParamSurface::second_derivative_bound", classes: T, prepare: |c| bind(inputs::surface::torus_second_bound(c), kernels::surface::torus_second_bound) },
    Op { group: "surface", name: "torus_normal_bound", measures: "geom_surface::ParamSurface::normal_derivative_bound", classes: T, prepare: |c| bind(inputs::surface::torus_normal_bound(c), kernels::surface::torus_normal_bound) },
    // Root finding and quadrature (piece G51; decision 32.44).
    Op { group: "brent", name: "zero", measures: "brent::zero", classes: T, prepare: |c| bind(inputs::roots::zero(c), kernels::roots::zero) },
    Op { group: "quadrature", name: "gauss_legendre", measures: "quadrature::gauss_legendre", classes: T, prepare: |c| bind(inputs::roots::gauss_legendre(c), kernels::roots::gauss_legendre) },
    Op { group: "math_types", name: "decimal_parse", measures: "geom_math::Decimal::parse", classes: T, prepare: |c| bind(inputs::math_types::decimal_parse(c), kernels::math_types::decimal_parse) },
];

/// The groups of [`OPS`], in order.
pub const GROUPS: [&str; 30] = [
    "predicate",
    "sign",
    "wide",
    "wide32",
    "plane",
    "crossing",
    "eft",
    "interval",
    "grid",
    "real",
    "exactness",
    "math",
    "unproven",
    "rng",
    "dd",
    "exact_sum",
    "generated",
    "perturb",
    "newton",
    "dual",
    "isolate",
    "ball",
    "math_types",
    "bvh",
    "nurbs",
    "curve",
    "surface",
    "brent",
    "quadrature",
    "linalg",
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
    ("Lpi::points", "field accessor"),
    ("Lpi::plane", "field accessor"),
    ("Ssi::points", "field accessor"),
    ("Round::new", "field constructor"),
    ("Round::on", "field accessor"),
    ("Round::centre", "field accessor"),
    ("Arc2::new", "one call of Round::side_of_grid, which is measured"),
    ("Arc2::bisector", "two differences, four products and Carrier::new"),
    ("Arc2::round", "field accessor"),
    ("Arc2::start", "field accessor"),
    ("Arc2::end", "field accessor"),
    ("Arc2::holds", "Carrier::through and one Carrier::side, which is measured"),
    ("Arc2::order", "one Rational2::orient, which is measured"),
    ("Circle2::new", "two range checks"),
    ("Circle2::centre", "field accessor"),
    ("Circle2::radius", "field accessor"),
    ("Circle2::against", "two calls of generated::circle_gap, which is measured"),
    ("Crossing2::of_circles", "the same steps as Crossing2::of_round, on the radical line"),
    ("Crossing2::of_rounds", "the same steps as Crossing2::of_round, on the radical line"),
    ("Crossing2::of_round_and_circle", "the same steps as Crossing2::of_round, on the radical line"),
    ("Crossing2::of_point", "the enclosures of three integers, as Crossing2::of_circle's"),
    ("Crossing2::parts", "field accessor"),
    ("Crossing2::enclosure", "field accessor"),
    ("Crossing2::cmp_x_filter", "two interval subtractions, the first step of Crossing2::cmp_x"),
    ("Crossing2::cmp_y", "the same path as Crossing2::cmp_x, on the other coordinate"),
    ("Crossing2::same", "two comparisons, as Crossing2::cmp_x is measured"),
    ("Crossing2::order", "two comparisons, as Crossing2::cmp_x is measured"),
    ("Crossing2::against_round", "the same steps as Crossing2::against_circle, with two more products"),
    ("Arc2::holds_crossing", "Carrier::through and one Crossing2::side, which is measured"),
    ("one_root::sign_of_root", "the last step of Crossing2::cmp_x, which is measured"),
    ("one_root::sign_of_two_roots", "the last step of Arc2::order_crossings, which is measured"),
    ("Ellipse2::new", "range checks and a greatest common divisor"),
    ("Ellipse2::of_circle", "one call of Ellipse2::new"),
    ("Ellipse2::centre", "field accessor"),
    ("Ellipse2::major", "field accessor"),
    ("Ellipse2::ratio", "field accessor"),
    ("Ellipse2::quartic", "a few dozen products, the first step of Ellipse2::meeting, which is measured"),
    ("Conic::of_ellipse", "the form's products, part of what Ellipse2::meeting is measured doing"),
    ("Conic::of_circle", "one product"),
    ("Conic::of_round", "a few products, as Conic::of_ellipse's"),
    ("Conic::of_carrier", "field constructor"),
    ("Conic::of_fine", "field constructor, as Conic::of_carrier's, on a FineLine's coefficients"),
    ("Conic::of_disc", "a few products about the grid point below the centre, as Conic::of_round's"),
    ("Conic::about", "a few dozen whole-number products moving the lower terms, once per meeting about a point"),
    ("Conic::coefficients", "field accessor"),
    ("Conic::origin", "field accessor"),
    ("Disc::new", "range checks"),
    ("Disc::of_round", "a few products: a Round's centre and squared radius"),
    ("Disc::of_circle", "one product, a grid circle's squared radius"),
    ("Disc::centre", "field accessor"),
    ("Disc::squared_radius", "field accessor"),
    ("Disc::side", "the same steps as Round::side, which is measured, on a squared radius given"),
    ("Disc::meets", "the same steps as Round::meets, which is measured, on a squared radius given"),
    ("Disc::meets_fine", "the same steps as Disc::meets, on a FineLine's wider coefficients"),
    ("Disc::gap_to_carrier", "Disc::meets's discriminant and one bounded arcsine, once per join of a tangent construction"),
    ("Disc::gap_to_fine", "the same steps as Disc::gap_to_carrier, on a FineLine's coefficients"),
    ("Disc::gap_to_disc", "the radical line's discriminant and one bounded arcsine, as Disc::gap_to_carrier's"),
    ("FineLine::new", "range checks"),
    ("FineLine::coefficients", "field accessor"),
    ("FineLine::side", "three products and two sums, as Carrier::side's, which is measured, in wider integers"),
    ("Crossing2::of_disc", "the same steps as Crossing2::of_round, which is measured, on a Disc's parts"),
    ("Crossing2::of_fine", "the same steps as Crossing2::of_disc, on a FineLine's coefficients"),
    ("Crossing2::of_discs", "the same steps as Crossing2::of_round, on the radical line"),
    ("Crossing2::of_lines", "Cramer's rule, six products, and the enclosures of three integers, as Crossing2::of_point's"),
    ("Crossing2::of_fine_and_ellipse", "the same steps as Crossing2::of_ellipse, which is measured, on a FineLine's coefficients"),
    ("Crossing2::side_of_fine", "the same path as Crossing2::side, which is measured, on a FineLine's coefficients"),
    ("Crossing2::against_disc", "the same steps as Crossing2::against_circle, which is measured, with a Disc's two more products"),
    ("RealRoot::angle", "two enclosures of a root's stretch and the half-angle formulas' few dozen interval operations"),
    ("one_root::enclose", "a few interval operations per limb, the first step of every Crossing2 construction, which are measured"),
    ("Quartic::new", "field constructor"),
    ("Quartic::is_zero", "a scan of five coefficients"),
    ("Quartic::coefficients", "field accessor"),
    ("Quartic::roots", "what Ellipse2::meeting spends its time in, which is measured"),
    ("Roots::iter", "field accessor"),
    ("Roots::count", "a scan of four slots"),
    ("RealRoot::piece", "field accessor"),
    ("RealRoot::multiplicity", "field accessor"),
    ("Carrier::new", "three range checks, on the way to every carrier the benchmarks draw"),
    ("Carrier::through", "two differences and two products, then Carrier::new"),
    ("Carrier::coefficients", "field accessor"),
    ("Carrier::direction", "field accessor with one sign turned"),
    ("Carrier::parallel", "one difference of products, the weight Carrier::meet measures"),
    ("Carrier::same", "three differences of products, the terms Carrier::meet measures"),
    ("Rational2::new", "three range checks and a sign turn, on the way to Carrier::meet's result"),
    ("Rational2::coordinates", "field accessor"),
    ("Rational2::cmp_y", "the same predicate as Rational2::cmp_x, which is measured"),
    ("Rational2::same", "two calls of the predicate Rational2::cmp_x measures"),
    ("Wide::to_le_limbs", "field accessor"),
    ("Wide::from_le_limbs", "raw constructor"),
    ("Wide::max_value", "a constant: every limb is fixed"),
    ("Wide::min_value", "a constant: every limb is fixed"),
    ("Wide::one", "one call to Wide::from_i128, which is measured"),
    ("Wide::from_i64", "one call to Wide::from_i128, which is measured"),
    ("Wide::is_negative", "one test of the top bit, inside every Wide benchmark"),
    ("Wide::is_zero", "measured inside Wide::signum, whose batch has zeros and values with only the top limb set"),
    ("Wide32::to_le_limbs", "field accessor"),
    ("Wide32::from_le_limbs", "raw constructor"),
    ("Wide32::max_value", "a constant: every limb is fixed"),
    ("Wide32::min_value", "a constant: every limb is fixed"),
    ("Wide32::from_i64", "measured inside Wide32::product_i64, which converts both operands with it"),
    ("Wide32::product_by_i64", "one call to Wide32::product with the i64 in two limbs, which is measured"),
    ("Wide32::is_negative", "one test of the top bit, inside every Wide32 benchmark"),
    ("Wide32::is_zero", "measured inside Wide32::signum, whose batch has zeros and values with only the top limb set"),
    ("Interval::point", "one call to Interval::new, which is measured"),
    ("Interval::sin", "a constant: always Err(UnprovenMaths): no radian sine is proven; Interval::sinpi, which is certified, is measured"),
    ("Interval::cos", "a constant: always Err(UnprovenMaths): no radian cosine is proven; Interval::cospi, which is certified, is measured"),
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
    ("brent::minimum", "a search: its cost is its count of the caller's function's evaluations, at most the cap, which the result reports and tests/brent.rs checks, plus a few dozen f64 operations per evaluation"),
    ("quadrature::integrate", "2n + 1 evaluations of the caller's function per panel, counted in the result, and exact sums: the cost is the function's"),
    ("Dual::new", "a constructor: stores its two arguments"),
    ("Dual::value", "field accessor"),
    ("Dual::partials", "field accessor"),
    ("Dual::variables", "N stores per unknown, measured inside dual::jacobian, which calls it once per evaluation"),
    ("Ball::to_interval", "two outward roundings, measured inside ball/cubic, which calls it once per box"),
    ("Ball::mid", "field accessor"),
    ("Ball::rad", "field accessor"),
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
    ("DD::from_f64", "one finiteness test, and a zero made +0: the test Real::new makes"),
    ("DD::from_real", "raw constructor: the Real's double, a zero made +0"),
    ("DD::hi", "field accessor"),
    ("DD::lo", "field accessor"),
    ("DD::to_f64", "field accessor: the leading part, which is the value correctly rounded"),
    ("DD::sub", "one call to DD::add, which is measured, on the exact negation DD::negate gives"),
    ("DD::sub_f64", "one call to DD::add_f64, which is measured, on the negated double"),
    ("ExactSum::new", "a constant: every chunk zero"),
    ("generated::orient2d::exact", "one call to generated::orient2d::value, which is measured, and the sign of the value"),
    ("generated::orient3d::exact", "one call to generated::orient3d::value, which is measured, and the sign of the value"),
    ("generated::incircle::exact", "one call to generated::incircle::value, which is measured, and the sign of the value"),
    ("generated::insphere::exact", "one call to generated::insphere::value, which is measured, and the sign of the value"),
    ("generated::plane_side::exact", "one call to generated::plane_side::value, which is measured, and the sign of the value"),
    ("generated::tpi_weight::exact", "one call to generated::tpi_weight::value, which is measured, and the sign of the value"),
    ("generated::tpi_side::exact", "one call to generated::tpi_side::value, which is measured, and the sign of the value"),
    ("generated::lpi_weight::exact", "one call to generated::lpi_weight::value, which is measured, and the sign of the value"),
    ("generated::lpi_side::exact", "one call to generated::lpi_side::value, which is measured, and the sign of the value"),
    ("generated::ssi_weight::exact", "one call to generated::ssi_weight::value, which is measured, and the sign of the value"),
    ("generated::ssi_side::exact", "one call to generated::ssi_side::value, which is measured, and the sign of the value"),
    ("generated::carrier_side::exact", "one call to generated::carrier_side::value, which is measured, and the sign of the value"),
    ("generated::carrier_order::exact", "one call to generated::carrier_order::value, which is measured, and the sign of the value"),
    ("generated::rational_cmp::exact", "one call to generated::rational_cmp::value, which is measured, and the sign of the value"),
    ("generated::rational_orient::exact", "one call to generated::rational_orient::value, which is measured, and the sign of the value"),
    ("generated::ellipse_side::exact", "one call to generated::ellipse_side::value, which is measured, and the sign of the value"),
    ("generated::ellipse_line::exact", "one call to generated::ellipse_line::value, which is measured, and the sign of the value"),
    ("generated::round_grid::exact", "one call to generated::round_grid::value, which is measured, and the sign of the value"),
    ("generated::round_rational::exact", "one call to generated::round_rational::value, which is measured, and the sign of the value"),
    ("generated::round_line::exact", "one call to generated::round_line::value, which is measured, and the sign of the value"),
    ("generated::circle_side::exact", "one call to generated::circle_side::value, which is measured, and the sign of the value"),
    ("generated::circle_line::exact", "one call to generated::circle_line::value, which is measured, and the sign of the value"),
    ("generated::circle_gap::exact", "one call to generated::circle_gap::value, which is measured, and the sign of the value"),
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
