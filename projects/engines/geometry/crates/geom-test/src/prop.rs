//! Property tests run from a seed: proptest with the seed discipline, and grid strategies that draw
//! the same cases on every target.
//!
//! What it does: [`run`] runs a proptest property from a [`Seed`] resolved and printed by
//! [`crate::seed::resolve`], so a property test has the same seed rules as every other seeded test.
//! [`config`] builds the proptest configuration it uses. [`first_value`] draws one case from a
//! seed. [`coord`], [`p2`] and [`p3`] are proptest strategies for grid coordinates and points.
//!
//! Guarantees:
//!
//! - **The seed decides every case, or nothing runs.** [`config`] fixes the seed, the generator
//!   (`ChaCha20`) and every runner setting that changes which cases are drawn or how they shrink,
//!   so proptest's runner variables (`PROPTEST_RNG_SEED`, `PROPTEST_RNG_ALGORITHM`, `PROPTEST_CASES`,
//!   `PROPTEST_MAX_SHRINK_ITERS` and the other limits) cannot change a run: `SPAREKIT_SEED` is the
//!   one override. That is why [`run`] drives proptest's `TestRunner` directly; the `proptest!`
//!   macro re-reads those variables after its configuration is built. Two settings cannot be fixed
//!   that way, so [`run`] and [`first_value`] refuse to draw anything, with
//!   [`Failure::Environment`], while either is in force:
//!   - `PROPTEST_MAX_DEFAULT_SIZE_RANGE`. proptest 1.12 reads it once per process into
//!     `Config::default()`, and every collection strategy of default size (`any::<Vec<T>>()`,
//!     `any::<String>()`, `vec(s, SizeRange::default())`) takes its size from there when the
//!     strategy is built, before [`run`] sees it (`collection.rs`, `impl Default for SizeRange`).
//!     A runner's own `max_default_size_range` is never read. So a seed printed in a shell with the
//!     variable set would draw other cases in a shell without it. Strategies with explicit sizes
//!     (`vec(s, 0..=n)`) never read it.
//!   - Forking and timeouts (`PROPTEST_FORK`, `PROPTEST_TIMEOUT`, read when another crate in the
//!     build turns on proptest's `fork` or `timeout` feature). A forked case runs in another
//!     process, and a timeout lets a clock decide a result (28.19).
//! - **The same cases on every target.** These strategies draw exactly one 64-bit word from
//!   proptest's `ChaCha20` stream per coordinate and make the coordinate from it with
//!   [`crate::rng::SplitMix64`], in integer arithmetic only. proptest's own samplers are not used
//!   for them: proptest 1.12 samples a `usize` range as a `u64` on 64-bit targets and as a `u32` on
//!   32-bit ones (`num.rs`, `sample_uniform!`), so `proptest::sample::select`, for one, reads the
//!   stream at different offsets on wasm32 and on x86-64, and every later value of the case
//!   differs. A strategy a caller builds from proptest's samplers keeps that weakness; a test whose
//!   cases must replay across targets draws from these strategies or from [`crate::grid`].
//! - **No files.** Failure persistence is off: a run never reads or writes `proptest-regressions`
//!   files, so its result depends only on the seed and the code. A failure becomes a permanent
//!   fixture by the route of 28.16, not by a file proptest keeps.
//! - **A failure carries its replay.** [`Failure`] names the property, the seed, the minimal failing
//!   input after shrinking, and the `SPAREKIT_SEED` value that replays it.
//! - **Grid strategies stay on the grid and shrink towards zero.** Every value has magnitude at
//!   most [`COORD_MAX`]. A coordinate shrinks by binary search from its drawn value towards zero,
//!   whichever kind of value was drawn, so for a property that fails exactly when |v| ≥ k (k > 0)
//!   the minimal failing coordinate proptest reports is ±k.
//!
//! Method: proptest (MIT OR Apache-2.0, <https://github.com/proptest-rs/proptest>), the
//! Hypothesis-style property tester the kernel's tests use (28.2). Its runner draws each case from a
//! `ChaCha20` stream seeded by `RngSeed::Fixed`, and shrinks a failure to a minimal case. A
//! coordinate shrinks with proptest's own `num::i64::BinarySearch`, which halves the distance
//! towards zero while the property still fails and backs off when it passes: bisection, as in
//! Knuth, *The Art of Computer Programming*, volume 3, section 6.2.1
//! (<https://www-cs-faculty.stanford.edu/~knuth/taocp.html>).
//!
//! Cost: one run of the property per case, plus shrinking after a failure, at most about 41
//! property runs per coordinate (a binary search over at most 2^40 magnitudes). One read of
//! proptest's process-wide configuration per [`run`] or [`first_value`].

use core::fmt;
use core::num::NonZeroU64;

use geom_num::grid::{COORD_MAX, Coord, P2, P3};
use proptest::num::i64::BinarySearch;
use proptest::prelude::{BoxedStrategy, Rng as _, Strategy};
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestCaseError, TestError, TestRunner};

use crate::grid::EXTREMES;
use crate::rng::SplitMix64;
use crate::seed::{Seed, SeedError, fixed, resolve};

/// proptest's default number of tries for one case before the run gives up on its filters.
const MAX_LOCAL_REJECTS: u32 = 65_536;

/// proptest's default number of filtered-out cases before the run gives up.
const MAX_GLOBAL_REJECTS: u32 = 1_024;

/// proptest's default limit on regenerating a `flat_map` value during shrinking.
const MAX_FLAT_MAP_REGENS: u32 = 1_000_000;

/// proptest's default size range for collections of default size: `0..100`.
pub const MAX_DEFAULT_SIZE_RANGE: usize = 100;

/// The label [`first_value`] prints its seed under.
const FIRST_VALUE: &str = "first value of a strategy";

/// The proptest configuration for `cases` cases drawn from `seed`: proptest's documented defaults
/// for every runner setting that changes which cases are drawn or how they shrink, set here
/// explicitly so no environment variable can change them, `ChaCha20` seeded with `seed`, and no
/// failure files.
#[must_use]
pub fn config(seed: Seed, cases: u32) -> Config {
    // Fields not named here (`result_cache`, and `fork` and `timeout` if another crate in the build
    // turns those features on) keep the process-wide defaults. `result_cache` changes nothing that
    // is drawn; `fork` and `timeout` are refused by `environment`.
    Config {
        cases,
        max_local_rejects: MAX_LOCAL_REJECTS,
        max_global_rejects: MAX_GLOBAL_REJECTS,
        max_flat_map_regens: MAX_FLAT_MAP_REGENS,
        // proptest reads this field only from the process-wide default, never from a runner's
        // configuration (see the module header). It is set to the default all the same, and
        // `environment` refuses a run whose process-wide default differs.
        max_default_size_range: MAX_DEFAULT_SIZE_RANGE,
        max_shrink_iters: u32::MAX,
        max_shrink_time: 0,
        verbose: 0,
        failure_persistence: None,
        source_file: None,
        test_name: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed.get()),
        ..Config::default()
    }
}

/// Why a seeded property run failed.
#[derive(Clone, PartialEq, Eq)]
pub enum Failure {
    /// `SPAREKIT_SEED` was set but is not a seed.
    Seed(SeedError),
    /// proptest's process-wide configuration would change the cases a seed draws, or how they
    /// run, so nothing was drawn. The text names the variable and what to do.
    Environment(String),
    /// The property failed, or the run gave up on its filters.
    Property {
        /// The property's label.
        label: String,
        /// The seed of the run.
        seed: Seed,
        /// proptest's report: the reason and the minimal failing input.
        report: String,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Seed(e) => write!(f, "{e}"),
            Failure::Environment(why) => write!(f, "nothing was drawn: {why}"),
            Failure::Property {
                label,
                seed,
                report,
            } => write!(
                f,
                "property {label:?} failed with seed: {seed}\n{report}\n\
                 replay with SPAREKIT_SEED={seed}"
            ),
        }
    }
}

/// The same text as `Display`, so a test that returns `Result<(), Failure>` shows the whole report.
impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for Failure {}

/// Refuses a run whose cases or execution the process environment would change: a process-wide
/// default collection size other than proptest's default, or forking or a timeout in `config`.
fn environment(config: &Config) -> Result<(), Failure> {
    let size = Config::default().max_default_size_range;
    if size != MAX_DEFAULT_SIZE_RANGE {
        return Err(Failure::Environment(format!(
            "PROPTEST_MAX_DEFAULT_SIZE_RANGE makes proptest's default collection size 0..{size} \
             (its default is 0..{MAX_DEFAULT_SIZE_RANGE}) for every collection strategy built \
             in this process, so a printed seed would draw other cases where it is unset. \
             Unset PROPTEST_MAX_DEFAULT_SIZE_RANGE; to test other sizes, give the collection \
             strategies explicit sizes"
        )));
    }
    if config.fork() {
        return Err(Failure::Environment(format!(
            "PROPTEST_FORK or PROPTEST_TIMEOUT (timeout {} ms) asks proptest to run each case in \
             a forked process, with a clock deciding a timeout; a seeded run is in-process and \
             untimed. Unset both",
            config.timeout()
        )));
    }
    Ok(())
}

/// Runs `test` on `cases` cases drawn from `strategy`, from the seed [`resolve`] gives for `label`
/// and `default` (printed before the first case).
///
/// # Errors
///
/// [`Failure::Seed`] if `SPAREKIT_SEED` is malformed; [`Failure::Environment`] if proptest's
/// process-wide configuration would change the cases (see the module header);
/// [`Failure::Property`] with the shrunk failing input if `test` fails on any case.
pub fn run<S, F>(
    label: &str,
    default: Seed,
    cases: u32,
    strategy: &S,
    test: F,
) -> Result<(), Failure>
where
    S: Strategy,
    S::Value: fmt::Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    let seed = resolve(label, default).map_err(Failure::Seed)?;
    let config = config(seed, cases);
    environment(&config)?;
    let mut runner = TestRunner::new(config);
    runner.run(strategy, test).map_err(|e| Failure::Property {
        label: label.to_owned(),
        seed,
        report: report(&e),
    })
}

/// proptest's report of a failed run.
fn report<V: fmt::Debug>(e: &TestError<V>) -> String {
    match e {
        TestError::Abort(why) => format!("aborted: {why}"),
        TestError::Fail(why, input) => {
            format!("failed: {why}\nminimal failing input: {input:#?}")
        }
    }
}

/// The first value `strategy` draws from `seed`, before any shrinking: for checking that a seed
/// reproduces the same cases. The seed is printed with [`fixed`]; `SPAREKIT_SEED` does not apply.
///
/// # Errors
///
/// [`Failure::Environment`] as for [`run`]; [`Failure::Property`] if the strategy rejects
/// every try.
pub fn first_value<S: Strategy>(strategy: &S, seed: Seed) -> Result<S::Value, Failure> {
    let seed = fixed(FIRST_VALUE, seed);
    let config = config(seed, 1);
    environment(&config)?;
    let mut runner = TestRunner::new(config);
    strategy
        .new_tree(&mut runner)
        .map(|tree| tree.current())
        .map_err(|why| Failure::Property {
            label: FIRST_VALUE.to_owned(),
            seed,
            report: format!("the strategy rejected every try: {why}"),
        })
}

/// How many kinds of coordinate [`draw_coord`] chooses between, by weight: anywhere in the frame
/// (4 of 6), small (1 of 6), an extreme (1 of 6).
const KINDS: NonZeroU64 = match NonZeroU64::new(6) {
    Some(n) => n,
    None => NonZeroU64::MIN,
};

/// The largest magnitude of a small coordinate.
const SMALL: i64 = 16;

/// A coordinate from one word: anywhere in −(2^40 − 1)..=2^40 − 1 (4 in 6), small in −16..=16 (1 in
/// 6), or one of [`EXTREMES`]: ±(2^40 − 1), ±(2^40 − 2), 0 and ±1 (1 in 6), each value of a kind
/// equally likely. The word seeds a `SplitMix64` stream, whose draws are exact integer arithmetic.
fn draw_coord(word: u64) -> i64 {
    let mut rng = SplitMix64::new(Seed(word));
    let value = match rng.below(KINDS) {
        0..=3 => rng.range_i64(-COORD_MAX, COORD_MAX),
        4 => rng.range_i64(-SMALL, SMALL),
        _ => u64::try_from(EXTREMES.len())
            .ok()
            .and_then(NonZeroU64::new)
            .and_then(|n| usize::try_from(rng.below(n)).ok())
            .and_then(|i| EXTREMES.get(i).copied()),
    };
    // Every branch above yields a value; zero stands for the impossible `None`, because library
    // code may not panic.
    value.unwrap_or(0)
}

/// The coordinate strategy: one `ChaCha20` word per coordinate, made into a value by
/// [`draw_coord`].
#[derive(Clone, Copy, Debug)]
struct CoordStrategy;

impl Strategy for CoordStrategy {
    type Tree = CoordTree;
    type Value = Coord;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let word = runner.rng().next_u64();
        Ok(CoordTree(BinarySearch::new(draw_coord(word))))
    }
}

/// A drawn coordinate and its shrinking: proptest's binary search from the drawn value towards
/// zero. Every value it visits lies between zero and the drawn value, so on the grid.
#[derive(Clone, Copy, Debug)]
struct CoordTree(BinarySearch);

impl ValueTree for CoordTree {
    type Value = Coord;

    fn current(&self) -> Coord {
        // Between zero and a drawn value of magnitude at most COORD_MAX, so `new` cannot fail;
        // library code may not panic, so the impossible error maps to zero.
        Coord::new(self.0.current()).unwrap_or(Coord::ZERO)
    }

    fn simplify(&mut self) -> bool {
        self.0.simplify()
    }

    fn complicate(&mut self) -> bool {
        self.0.complicate()
    }
}

/// A grid coordinate, as [`draw_coord`] makes it from one word of the case's stream. Shrinks
/// towards zero.
pub fn coord() -> BoxedStrategy<Coord> {
    CoordStrategy.boxed()
}

/// A 2D grid point, each coordinate by [`coord`], x first.
pub fn p2() -> BoxedStrategy<P2> {
    (CoordStrategy, CoordStrategy)
        .prop_map(|(x, y)| P2 { x, y })
        .boxed()
}

/// A 3D grid point, each coordinate by [`coord`], x first.
pub fn p3() -> BoxedStrategy<P3> {
    (CoordStrategy, CoordStrategy, CoordStrategy)
        .prop_map(|(x, y, z)| P3 { x, y, z })
        .boxed()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;
    use proptest::prelude::prop_assert;

    #[test]
    fn the_same_seed_gives_the_same_cases() {
        let a: Vec<P3> = (0..50)
            .map(|i| first_value(&p3(), Seed(i)).unwrap())
            .collect();
        let b: Vec<P3> = (0..50)
            .map(|i| first_value(&p3(), Seed(i)).unwrap())
            .collect();
        assert_eq!(a, b);
        assert_ne!(
            first_value(&p3(), Seed(1)).unwrap(),
            first_value(&p3(), Seed(2)).unwrap()
        );
    }

    /// The first cases of seeds 0..64, hashed: FNV-1a 64 over every coordinate's little-endian
    /// bytes. CI runs this natively on 64-bit targets and under wasm32-wasip1, so a seed draws the
    /// same proptest cases on every target. A change here changes every property test's cases: it
    /// must be deliberate and said so in its pull request.
    #[test]
    fn the_same_seed_gives_the_same_cases_on_every_target() {
        const GOLDEN: u64 = 0xc721_5d77_28da_881f;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a 64 offset basis
        for i in 0..64 {
            let p = first_value(&p3(), Seed(i)).unwrap();
            for c in [p.x, p.y, p.z] {
                for b in c.get().to_le_bytes() {
                    hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        assert_eq!(hash, GOLDEN, "{hash:#018x}");
    }

    #[test]
    fn config_fixes_the_seed_and_keeps_no_files() {
        let c = config(Seed(77), 12);
        assert_eq!(c.cases, 12);
        assert_eq!(c.rng_seed, RngSeed::Fixed(77));
        assert_eq!(c.rng_algorithm, RngAlgorithm::ChaCha);
        assert!(c.failure_persistence.is_none());
        assert!(!c.fork());
    }

    #[test]
    fn a_failing_property_reports_its_seed_and_shrunk_input() {
        // Fails for every value above 1000; shrinking must find exactly 1001.
        let r = run("shrinks", Seed(5), 256, &(0i64..=1 << 40), |v| {
            prop_assert!(v <= 1000);
            Ok(())
        });
        let Err(Failure::Property {
            label,
            seed,
            report,
        }) = r
        else {
            panic!("expected a property failure, got {r:?}");
        };
        assert_eq!(label, "shrinks");
        // The run's seed is the default unless SPAREKIT_SEED is set for this process.
        if std::env::var_os(crate::seed::ENV_VAR).is_none() {
            assert_eq!(seed, Seed(5));
        }
        assert!(report.contains("minimal failing input: 1001"), "{report}");
        let text = Failure::Property {
            label,
            seed,
            report,
        }
        .to_string();
        assert!(text.contains(&format!("replay with SPAREKIT_SEED={seed}")));
    }

    /// The minimal failing coordinate proptest finds for a property that fails exactly when
    /// |v| ≥ k, from `seed`.
    fn minimal_failing(seed: Seed, k: i64) -> Option<i64> {
        let mut runner = TestRunner::new(config(fixed("coordinate shrinking", seed), 1_024));
        match runner.run(&coord(), |c| {
            prop_assert!(c.get().abs() < k);
            Ok(())
        }) {
            Err(TestError::Fail(_, c)) => Some(c.get()),
            _ => None,
        }
    }

    /// Shrinking finds the exact boundary, from every kind of drawn value: the full range (most
    /// failures of |v| < 2^30), the extremes (the only failures of |v| < 2^40 − 2) and small values
    /// (|v| < 2).
    #[test]
    fn coordinates_shrink_to_the_exact_boundary() {
        for seed in 0..16 {
            for k in [2, 1 << 30, COORD_MAX - 1, COORD_MAX] {
                assert_eq!(
                    minimal_failing(Seed(seed), k).map(i64::abs),
                    Some(k),
                    "seed {seed}, k {k}"
                );
            }
        }
    }

    /// Every kind of coordinate is drawn, at about its weight, and every value is on the grid.
    #[test]
    fn every_kind_of_coordinate_is_drawn() {
        let mut rng = SplitMix64::new(fixed("coordinate kinds", Seed::DEFAULT));
        let (mut big, mut small, mut edge) = (0, 0, 0);
        let mut seen = [false; 7];
        for _ in 0..60_000 {
            let v = draw_coord(rng.next_u64());
            assert!(v.abs() <= COORD_MAX);
            big += usize::from(v.abs() > SMALL);
            small += usize::from(v.abs() <= SMALL);
            edge += usize::from(v.abs() >= COORD_MAX - 1);
            for (s, e) in seen.iter_mut().zip(EXTREMES) {
                *s |= v == e;
            }
        }
        // Expected: 40,000 anywhere in the frame (all but a share of about 2^-35 of them big) and
        // 10,000 extremes, 4/7 of them at the edges (about 5,714) and 3/7 small (about 4,286); plus
        // 10,000 small. So about 45,714 big, 14,286 small and 5,714 at the edges, each with a
        // standard deviation near 104 (72 for the edges); the bounds are at least 7 of them wide.
        assert!((44_000..47_500).contains(&big), "{big}");
        assert!((13_300..15_300).contains(&small), "{small}");
        assert!((5_200..6_250).contains(&edge), "{edge}");
        assert!(seen.iter().all(|&s| s), "{seen:?}");
    }

    #[test]
    fn grid_strategies_stay_on_the_grid() {
        let r = run("on the grid", Seed(6), 2_000, &p3(), |p| {
            for c in [p.x, p.y, p.z] {
                prop_assert!(c.get().abs() <= COORD_MAX);
            }
            Ok(())
        });
        assert!(r.is_ok(), "{r:?}");
    }
}
