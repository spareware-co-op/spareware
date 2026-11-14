//! The generated filters of the line and axis predicates (`line_sphere`, `axis_gap`, `cross_norm`,
//! `line_cylinder`, `line_cone` and `line_cone_lead`) against exact arithmetic, through the public
//! interface of `geom_num::generated`.
//!
//! - **At the bound.** For each filter and each monomial of its scale that can be the largest, an
//!   input where that monomial is the largest by a factor of at least 2^8 and the exact value is
//!   within a factor of four below the bound, where the filter must return `None`, and its twin
//!   within a factor of four above the bound, where it must return the exact sign. Every step of
//!   these inputs is a whole number below 2^53 times a power of two, so the filter's `f64` steps
//!   are exact: its computed value is the exact value, its computed scale is the exact scale, and
//!   its bound is the constant times that scale, rounded once. The comparison of the exact value
//!   with that bound is made here in `num-bigint`. The scale's monomials are read from each
//!   committed module's header, its `ERROR` from its code.
//! - **Unit maxima.** `line_cone` and `line_cone_lead` carry monomials that are never the largest
//!   alone (x²·y² is at most the larger of x⁴ and y⁴). With every group maximum one, each monomial
//!   is one, and an input whose exact value lies between the bound and twice the bound must be
//!   decided: a scale that counted two monomials would leave it undecided.
//! - **Everywhere else.** On random and near-tangent inputs, with differences confined to axes and
//!   coordinate planes half the time: every sign a filter gives is the exact sign, a filter
//!   decides every input whose exact value is beyond three times its bound (the bound and the
//!   filter's whole error together are below twice the constant times the scale), it never decides
//!   an exact zero with a scale above zero, and it answers `Zero` at once on a zero scale. The exact
//!   paths equal the descriptions' formulas, evaluated here in `num-bigint`.
//!
//! Seeds: the randomised test prints `seed: <hex>` and runs from a fixed default;
//! `SPAREKIT_SEED=<hex>` replays another (chapter 28.17). Case counts are a fiftieth in debug
//! builds.

// The crate denies native arithmetic and lossy casts in kernel code (Cargo.toml); tests may use
// them, and std's reductions, to build their oracles.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "oracles and generators; the rules are for kernel code"
)]
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "points and steps are named as in the descriptions"
)]
// Helpers outside `#[test]` functions are not covered by clippy.toml's allow-*-in-tests settings;
// clippy.toml says tests may use these, and a broken oracle must stop the test loudly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use geom_num::generated::{
    axis_gap, cross_norm, line_cone, line_cone_lead, line_cylinder, line_sphere,
};
use geom_num::grid::{COORD_MAX, P3};
use geom_num::{Sign, Wide};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};

// ---------------------------------------------------------------------------------------------
// Seeds and the generator.
// ---------------------------------------------------------------------------------------------

/// The default seed of the randomised test here.
const DEFAULT_SEED: u64 = 0x5d1c_0000_9a7e_0862;

/// `SplitMix64` (Steele, Lea and Flood, OOPSLA 2014), as `geom-test` has it, written here as in
/// this crate's other generated-predicate tests.
struct Rng(u64);

impl Rng {
    fn for_test(name: &str) -> Rng {
        let seed = std::env::var("SPAREKIT_SEED")
            .ok()
            .and_then(|s| u64::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_SEED);
        println!("{name}: seed: {seed:016x}");
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in lo..=hi.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo) as u64 + 1;
        lo + (self.next() % span) as i64
    }
}

/// Release builds run every count in full; debug builds a fiftieth.
fn cases(release: usize) -> usize {
    if cfg!(debug_assertions) {
        release / 50
    } else {
        release
    }
}

// ---------------------------------------------------------------------------------------------
// The six filters behind one input.
// ---------------------------------------------------------------------------------------------

type V = [i64; 3];
type B3 = [BigInt; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Sphere,
    Gap,
    Cross,
    Cylinder,
    Cone,
    Lead,
}

const KINDS: [Kind; 6] = [
    Kind::Sphere,
    Kind::Gap,
    Kind::Cross,
    Kind::Cylinder,
    Kind::Cone,
    Kind::Lead,
];

/// One filter's input: its points in the description's order (a, b, c, then f, or d for
/// `cross_norm`; unused ones at the origin) and its whole numbers (r, g, or s and t).
#[derive(Clone, Copy, Debug)]
struct Input {
    kind: Kind,
    points: [V; 4],
    whole: [i64; 2],
}

fn p3(v: V) -> P3 {
    P3::new(v[0], v[1], v[2]).unwrap()
}

fn big<const L: usize>(w: &Wide<L>) -> BigInt {
    let bytes: Vec<u8> = w
        .to_le_limbs()
        .iter()
        .flat_map(|l| l.to_le_bytes())
        .collect();
    BigInt::from_signed_bytes_le(&bytes)
}

fn sign_of(v: &BigInt) -> Sign {
    if v.is_zero() {
        Sign::Zero
    } else if v.is_negative() {
        Sign::Negative
    } else {
        Sign::Positive
    }
}

/// p − q.
fn minus(p: V, q: V) -> B3 {
    std::array::from_fn(|i| BigInt::from(p[i]) - BigInt::from(q[i]))
}

fn cross(u: &B3, v: &B3) -> B3 {
    [
        &u[1] * &v[2] - &u[2] * &v[1],
        &u[2] * &v[0] - &u[0] * &v[2],
        &u[0] * &v[1] - &u[1] * &v[0],
    ]
}

fn dot(u: &B3, v: &B3) -> BigInt {
    &u[0] * &v[0] + &u[1] * &v[1] + &u[2] * &v[2]
}

fn squared(v: &BigInt) -> BigInt {
    v * v
}

impl Input {
    /// The committed module's text.
    fn module(&self) -> &'static str {
        match self.kind {
            Kind::Sphere => include_str!("../src/generated/line_sphere.rs"),
            Kind::Gap => include_str!("../src/generated/axis_gap.rs"),
            Kind::Cross => include_str!("../src/generated/cross_norm.rs"),
            Kind::Cylinder => include_str!("../src/generated/line_cylinder.rs"),
            Kind::Cone => include_str!("../src/generated/line_cone.rs"),
            Kind::Lead => include_str!("../src/generated/line_cone_lead.rs"),
        }
    }

    fn filter(&self) -> Option<Sign> {
        let [a, b, c, f] = self.points.map(p3);
        let [s, t] = self.whole.map(i128::from);
        match self.kind {
            Kind::Sphere => line_sphere::filter(a, b, c, s),
            Kind::Gap => axis_gap::filter(a, b, c, s),
            Kind::Cross => cross_norm::filter(a, b, c, f),
            Kind::Cylinder => line_cylinder::filter(a, b, c, f, s),
            Kind::Cone => line_cone::filter(a, b, c, f, s, t),
            Kind::Lead => line_cone_lead::filter(a, b, c, f, s, t),
        }
    }

    /// The module's exact value.
    fn value(&self) -> BigInt {
        let [a, b, c, f] = self.points.map(p3);
        let [s, t] = self.whole.map(i128::from);
        match self.kind {
            Kind::Sphere => big(&line_sphere::value(a, b, c, s).unwrap()),
            Kind::Gap => big(&axis_gap::value(a, b, c, s).unwrap()),
            Kind::Cross => big(&cross_norm::value(a, b, c, f).unwrap()),
            Kind::Cylinder => big(&line_cylinder::value(a, b, c, f, s).unwrap()),
            Kind::Cone => big(&line_cone::value(a, b, c, f, s, t).unwrap()),
            Kind::Lead => big(&line_cone_lead::value(a, b, c, f, s, t).unwrap()),
        }
    }

    /// The module's exact sign.
    fn exact(&self) -> Option<Sign> {
        let [a, b, c, f] = self.points.map(p3);
        let [s, t] = self.whole.map(i128::from);
        match self.kind {
            Kind::Sphere => line_sphere::exact(a, b, c, s),
            Kind::Gap => axis_gap::exact(a, b, c, s),
            Kind::Cross => cross_norm::exact(a, b, c, f),
            Kind::Cylinder => line_cylinder::exact(a, b, c, f, s),
            Kind::Cone => line_cone::exact(a, b, c, f, s, t),
            Kind::Lead => line_cone_lead::exact(a, b, c, f, s, t),
        }
    }

    /// The description's formula, in `num-bigint`.
    fn oracle(&self) -> BigInt {
        let [a, b, c, f] = self.points;
        let [s, t] = self.whole.map(BigInt::from);
        match self.kind {
            // r² |d|² − |d × w|², with d = b − a and w = a − c.
            Kind::Sphere => {
                let (d, w) = (minus(b, a), minus(a, c));
                let u = cross(&d, &w);
                squared(&s) * dot(&d, &d) - dot(&u, &u)
            }
            // |w × e|² − g² |e|², with e = b − a and w = c − a.
            Kind::Gap => {
                let (e, w) = (minus(b, a), minus(c, a));
                let u = cross(&w, &e);
                dot(&u, &u) - squared(&s) * dot(&e, &e)
            }
            // |(b − a) × (d − c)|².
            Kind::Cross => {
                let n = cross(&minus(b, a), &minus(f, c));
                dot(&n, &n)
            }
            // r² |n|² − (n · w)², with n = d × e, d = b − a, e = f − c and w = a − c.
            Kind::Cylinder => {
                let (d, e, w) = (minus(b, a), minus(f, c), minus(a, c));
                let n = cross(&d, &e);
                squared(&s) * dot(&n, &n) - squared(&dot(&n, &w))
            }
            // s² |e|² |n|² − (s² + t²) (e · n)², with n = w × d.
            Kind::Cone => {
                let (d, e, w) = (minus(b, a), minus(f, c), minus(a, c));
                let n = cross(&w, &d);
                squared(&s) * dot(&e, &e) * dot(&n, &n)
                    - (squared(&s) + squared(&t)) * squared(&dot(&e, &n))
            }
            // t² |d × e|² − s² (d · e)², with d = b − a and e = f − c.
            Kind::Lead => {
                let (d, e) = (minus(b, a), minus(f, c));
                let u = cross(&d, &e);
                squared(&t) * dot(&u, &u) - squared(&s) * squared(&dot(&d, &e))
            }
        }
    }

    /// The group maxima, exactly: x, y and z over the description's translations, then the group
    /// of its whole numbers (one where it has none).
    fn maxima(&self) -> [BigInt; 4] {
        let [a, b, c, f] = self.points;
        let vectors: Vec<B3> = match self.kind {
            Kind::Sphere => vec![minus(b, a), minus(a, c)],
            Kind::Gap => vec![minus(b, a), minus(c, a)],
            Kind::Cross | Kind::Lead => vec![minus(b, a), minus(f, c)],
            Kind::Cylinder | Kind::Cone => vec![minus(b, a), minus(f, c), minus(a, c)],
        };
        let axis = |k: usize| vectors.iter().map(|v| v[k].abs()).max().unwrap();
        let [s, t] = self.whole.map(|v| BigInt::from(v).abs());
        let last = match self.kind {
            Kind::Sphere | Kind::Gap | Kind::Cylinder => s,
            Kind::Cross => BigInt::one(),
            Kind::Cone | Kind::Lead => s.max(t),
        };
        [axis(0), axis(1), axis(2), last]
    }

    /// Each monomial of the scale with its exact value, in the header's order.
    fn monomials(&self) -> Vec<(Monomial, BigInt)> {
        let maxima = self.maxima();
        scale_of(self.module())
            .into_iter()
            .map(|m| {
                let value = (0..4).fold(BigInt::one(), |p, g| p * maxima[g].pow(m[g]));
                (m, value)
            })
            .collect()
    }

    /// The exact scale: the largest monomial.
    fn scale(&self) -> BigInt {
        self.monomials().into_iter().map(|(_, v)| v).max().unwrap()
    }

    /// The same input with its axes permuted: axis i becomes axis `to[i]`.
    fn permuted(&self, to: [usize; 3]) -> Input {
        Input {
            points: self.points.map(|p| turn(p, to)),
            ..*self
        }
    }
}

fn turn<T: Copy + Default>(p: [T; 3], to: [usize; 3]) -> [T; 3] {
    let mut q = [T::default(); 3];
    for i in 0..3 {
        q[to[i]] = p[i];
    }
    q
}

const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [1, 2, 0],
    [2, 0, 1],
    [0, 2, 1],
    [2, 1, 0],
    [1, 0, 2],
];

// ---------------------------------------------------------------------------------------------
// The filters' constants and scales, as the committed modules state them.
// ---------------------------------------------------------------------------------------------

/// A monomial of a scale: the exponents of the x, y and z maxima and of the whole numbers' group.
type Monomial = [u32; 4];

/// The `ERROR` constant of a committed generated module.
fn error_of(module: &str) -> f64 {
    let line = module
        .lines()
        .find(|l| l.starts_with("const ERROR: f64 = f64::from_bits(0x"))
        .expect("an ERROR constant");
    let hex = line
        .trim_start_matches("const ERROR: f64 = f64::from_bits(0x")
        .trim_end_matches(");")
        .replace('_', "");
    f64::from_bits(u64::from_str_radix(&hex, 16).unwrap())
}

/// The monomials of a committed module's scale, from its header's `Scale S` row.
fn scale_of(module: &str) -> Vec<Monomial> {
    let row = module
        .lines()
        .find_map(|l| l.strip_prefix("//! | Scale S | "))
        .expect("a Scale S row")
        .trim_end_matches(" |");
    let list = row
        .strip_prefix("max{")
        .map_or(row, |r| r.trim_end_matches('}'));
    list.split(", ")
        .map(|term| {
            let mut m = [0u32; 4];
            for factor in term.split('·') {
                let (name, power) = factor.split_once('^').unwrap_or((factor, "1"));
                let group = match name {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    _ => 3,
                };
                m[group] += power.parse::<u32>().unwrap();
            }
            m
        })
        .collect()
}

/// Whether |v| > `bound`, exactly: an `f64` is a whole number times a power of two.
fn exceeds(v: &BigInt, bound: f64) -> bool {
    let bits = bound.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    assert!(bound > 0.0 && exponent != 0 && exponent != 0x7ff);
    let mantissa = BigInt::from((bits & ((1 << 52) - 1)) | (1 << 52));
    let shift = exponent - 1075;
    if shift >= 0 {
        v.abs() > (mantissa << shift as usize)
    } else {
        (v.abs() << (-shift) as usize) > mantissa
    }
}

/// The bound as the filter computes it on an input whose steps are exact in `f64`: the constant
/// times the exact scale, rounded once. The scale must be an `f64` exactly.
fn bound_of(input: &Input) -> f64 {
    let scale = input.scale();
    let as_f64 = scale.to_f64().unwrap();
    assert_eq!(
        BigInt::from_f64(as_f64).unwrap(),
        scale,
        "{input:?}: the scale is not an f64"
    );
    error_of(input.module()) * as_f64
}

// ---------------------------------------------------------------------------------------------
// At the bound.
// ---------------------------------------------------------------------------------------------

/// Which side of the bound a pinned input lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    /// At most the bound and above a quarter of it: the filter must return `None`.
    Under,
    /// Above the bound and at most `within` times it: the filter must return the exact sign.
    Over,
}

/// Checks one pinned input: `largest`, if given, is the scale's largest monomial by a factor of at
/// least 2^8; the exact value lies on `side` of the bound, within a factor of `within`; the exact
/// path equals the oracle; and the filter returns `None` under the bound and the exact sign over
/// it.
fn check_pinned(input: &Input, largest: Option<Monomial>, side: Side, within: f64) {
    let v = input.oracle();
    assert_eq!(input.value(), v, "{input:?}");
    assert_eq!(input.exact(), Some(sign_of(&v)), "{input:?}");
    if let Some(largest) = largest {
        let monomials = input.monomials();
        let scale = input.scale();
        for (m, value) in &monomials {
            if *m == largest {
                assert_eq!(*value, scale, "{input:?}: {m:?} is not the largest");
            } else {
                assert!(value << 8usize <= scale, "{input:?}: {m:?} is too near");
            }
        }
        assert!(monomials.iter().any(|(m, _)| *m == largest), "{input:?}");
    }
    let bound = bound_of(input);
    match side {
        Side::Under => {
            assert!(!exceeds(&v, bound), "{input:?}: {v} is over {bound}");
            assert!(exceeds(&v, bound / within), "{input:?}: {v} is far under");
            assert_eq!(input.filter(), None, "{input:?}");
        }
        Side::Over => {
            assert!(exceeds(&v, bound), "{input:?}: {v} is under {bound}");
            assert!(!exceeds(&v, bound * within), "{input:?}: {v} is far over");
            assert_eq!(input.filter(), Some(sign_of(&v)), "{input:?}");
        }
    }
}

/// Checks a pinned pair under every permutation of the axes, and returns the monomials that were
/// the largest.
fn check_pair(under: &Input, over: &Input, largest: Monomial) -> Vec<Monomial> {
    PERMUTATIONS
        .iter()
        .map(|&to| {
            let [x, y, z] = turn([largest[0], largest[1], largest[2]], to);
            let turned = [x, y, z, largest[3]];
            check_pinned(&under.permuted(to), Some(turned), Side::Under, 4.0);
            check_pinned(&over.permuted(to), Some(turned), Side::Over, 4.0);
            turned
        })
        .collect()
}

/// Whether the pinned pairs made every monomial of the module's scale the largest, but `never`.
fn assert_covered(kind: Kind, seen: &[Monomial], never: &[Monomial]) {
    let probe = Input {
        kind,
        points: [[0; 3]; 4],
        whole: [0; 2],
    };
    for m in scale_of(probe.module()) {
        assert_eq!(
            seen.contains(&m),
            !never.contains(&m),
            "{kind:?}: monomial {m:?}"
        );
    }
}

const O: V = [0, 0, 0];

/// `line_sphere`, with d = (0, 0, 1): the value is r² − `w_x`² − `w_y`², and `w_z` enters the scale
/// alone. z²·r² is the largest with a large r and `w_z`; x²·z² with a large `w_x` and `w_z`.
#[test]
fn line_sphere_decides_exactly_at_its_bound_for_each_monomial() {
    let with_r = |z: i64| Input {
        kind: Kind::Sphere,
        points: [[3, 4, z], [3, 4, z + 1], O, O],
        whole: [1 << 20, 0],
    };
    let without_r = |z: i64| Input {
        kind: Kind::Sphere,
        points: [[1 << 20, 0, z], [1 << 20, 0, z + 1], O, O],
        whole: [1, 0],
    };
    let mut seen = check_pair(&with_r(1 << 24), &with_r(1 << 23), [0, 0, 2, 2]);
    seen.extend(check_pair(
        &without_r(1 << 24),
        &without_r(1 << 23),
        [2, 0, 2, 0],
    ));
    assert_covered(Kind::Sphere, &seen, &[]);
}

/// `axis_gap`, with e = (0, 0, 1): the value is `w_x`² + `w_y`² − g², and `w_z` enters the scale
/// alone.
#[test]
fn axis_gap_decides_exactly_at_its_bound_for_each_monomial() {
    let with_g = |z: i64| Input {
        kind: Kind::Gap,
        points: [O, [0, 0, 1], [3, 4, z], O],
        whole: [1 << 20, 0],
    };
    let without_g = |z: i64| Input {
        kind: Kind::Gap,
        points: [O, [0, 0, 1], [1 << 20, 0, z], O],
        whole: [1, 0],
    };
    let mut seen = check_pair(&with_g(1 << 24), &with_g(1 << 23), [0, 0, 2, 2]);
    seen.extend(check_pair(
        &without_g(1 << 24),
        &without_g(1 << 23),
        [2, 0, 2, 0],
    ));
    assert_covered(Kind::Gap, &seen, &[]);
}

/// `cross_norm`, with p = (1, 0, 0) and q = (x, 2^20, 0): the value is 2^40, and `q_x` enters the
/// scale alone.
#[test]
fn cross_norm_decides_exactly_at_its_bound_for_each_monomial() {
    let input = |x: i64| Input {
        kind: Kind::Cross,
        points: [O, [1, 0, 0], O, [x, 1 << 20, 0]],
        whole: [0, 0],
    };
    let seen = check_pair(&input(1 << 24), &input(1 << 23), [2, 2, 0, 0]);
    assert_covered(Kind::Cross, &seen, &[]);
}

/// `line_cylinder`, with d = (1, 0, 0) and e = (0, 1, 0): the value is r² − `w_z`², and `w_x` and
/// `w_y` enter the scale alone.
#[test]
fn line_cylinder_decides_exactly_at_its_bound_for_each_monomial() {
    let input = |y: i64, z: i64, r: i64| Input {
        kind: Kind::Cylinder,
        points: [[1 << 11, y, z], [(1 << 11) + 1, y, z], O, [0, 1, 0]],
        whole: [r, 0],
    };
    let mut seen = check_pair(
        &input(1 << 11, 3, 1 << 20),
        &input(1 << 10, 3, 1 << 20),
        [2, 2, 0, 2],
    );
    seen.extend(check_pair(
        &input(1 << 11, 1 << 20, 1),
        &input(1 << 10, 1 << 20, 1),
        [2, 2, 2, 0],
    ));
    assert_covered(Kind::Cylinder, &seen, &[]);
}

/// `line_cone`, with e = (2^24, 0, 0), w = (`w_x`, 0, 0) and d = (0, 2^10, 0): n is along z, e · n
/// is zero, and the value is s² |e|² |n|². The monomial x²·y²·z² is never the largest alone: it is
/// the geometric mean of the other six.
#[test]
fn line_cone_decides_exactly_at_its_bound_for_each_monomial() {
    let input = |w: i64| Input {
        kind: Kind::Cone,
        points: [[w, 0, 0], [w, 1 << 10, 0], O, [1 << 24, 0, 0]],
        whole: [1 << 10, 3],
    };
    let seen = check_pair(&input(4), &input(8), [4, 2, 0, 2]);
    assert_covered(Kind::Cone, &seen, &[[2, 2, 2, 2]]);
}

/// `line_cone_lead`, with d = (2^26, 0, 0) and e = (0, `e_y`, 0): d · e is zero and the value is
/// t² |d|² |e|². The monomials a²·b² are never the largest alone: each is at most the larger of a⁴
/// and b⁴.
#[test]
fn line_cone_lead_decides_exactly_at_its_bound_for_each_monomial() {
    let input = |y: i64| Input {
        kind: Kind::Lead,
        points: [O, [1 << 26, 0, 0], O, [0, y, 0]],
        whole: [3, 1 << 10],
    };
    let seen = check_pair(&input(8), &input(16), [4, 0, 0, 2]);
    assert_covered(
        Kind::Lead,
        &seen,
        &[[0, 2, 2, 2], [2, 0, 2, 2], [2, 2, 0, 2]],
    );
}

/// With every axis maximum one, each monomial of the scale is the slope's square alone, and a value
/// between the bound and twice the bound is decided.
///
/// `line_cone`: e = (1, 1, 0), w = (0, 1, 0) and d = (1, 0, −1) give |e|² = |n|² = 2 and e · n =
/// −1, so the value is 3s² − t², which is 2 at s = 1542841 and t = 2672279 (a solution of t² − 3s² =
/// −2, the twelfth from (1, 1) under (t, s) → (2t + 3s, t + 2s)).
///
/// `line_cone_lead`: d = (1, 0, 0) and e = (1, 1, 1) give |d × e|² = 2 and d · e = 1, so the value
/// is 2t² − s², which is 7 at s = 14857739 and t = 10506008 (a solution of s² − 2t² = −7, the ninth
/// from (1, 2) under (s, t) → (3s + 4t, 2s + 3t)).
#[test]
fn cone_filters_count_one_monomial_at_unit_maxima() {
    let cone = Input {
        kind: Kind::Cone,
        points: [[0, 1, 0], [1, 1, -1], O, [1, 1, 0]],
        whole: [1_542_841, 2_672_279],
    };
    let lead = Input {
        kind: Kind::Lead,
        points: [O, [1, 0, 0], O, [1, 1, 1]],
        whole: [14_857_739, 10_506_008],
    };
    for (input, value) in [(cone, 2), (lead, 7)] {
        assert_eq!(input.oracle(), BigInt::from(value));
        let maxima = input.maxima();
        assert!(maxima[..3].iter().all(One::is_one), "{input:?}");
        let slope = squared(&maxima[3]);
        assert!(input.monomials().iter().all(|(_, v)| *v == slope));
        check_pinned(&input, None, Side::Over, 2.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Everywhere else.
// ---------------------------------------------------------------------------------------------

/// A vector with components up to 2^k for a random k up to `bits`, confined half the time to a
/// random axis, coordinate plane or the origin.
fn vector(rng: &mut Rng, bits: u64) -> V {
    let reach = 1i64 << (rng.next() % (bits + 1));
    let zeroed = if rng.next() & 1 == 0 {
        rng.next() & 7
    } else {
        0
    };
    std::array::from_fn(|i| {
        if (zeroed >> i) & 1 == 1 {
            0
        } else {
            rng.range(-reach, reach)
        }
    })
}

fn plus(p: V, q: V) -> V {
    std::array::from_fn(|i| p[i] + q[i])
}

/// The whole number nearest `real`, moved by up to two, within 0..=`limit`.
fn near(rng: &mut Rng, real: f64, limit: i64) -> i64 {
    let whole = if real.is_finite() {
        // The cast saturates, so a value past i64 is the limit too.
        (real.round() as i64).min(limit)
    } else {
        0
    };
    (whole + rng.range(-2, 2)).clamp(0, limit)
}

/// A whole number up to 2^k for a random k up to `bits`, within 0..=`limit`.
fn any(rng: &mut Rng, bits: u64, limit: i64) -> i64 {
    let reach = 1i64 << (rng.next() % (bits + 1));
    rng.range(0, reach).min(limit)
}

fn real(v: &BigInt) -> f64 {
    v.to_f64().unwrap()
}

/// A random input of `kind`: translations of up to 2^38 from a base of up to 2^38, and its whole
/// numbers either random or within two of the value that makes the line a tangent, the gap zero or
/// the leading coefficient zero. `cross_norm`'s second vector is, half the time, a whole multiple
/// of its first, moved by at most one step.
fn random(kind: Kind, rng: &mut Rng) -> Input {
    let c = vector(rng, 38);
    let tangent = rng.next() & 1 == 0;
    match kind {
        Kind::Sphere | Kind::Gap => {
            let (d, w) = (vector(rng, 38), vector(rng, 38));
            let limit = if kind == Kind::Sphere {
                (1 << 41) - 1
            } else {
                (1 << 42) - 1
            };
            let r = if tangent {
                let (db, wb) = (minus(d, O), minus(w, O));
                let u = cross(&db, &wb);
                near(
                    rng,
                    (real(&dot(&u, &u)) / real(&dot(&db, &db))).sqrt(),
                    limit,
                )
            } else {
                any(rng, 41, limit)
            };
            // line_sphere: w = a − c. axis_gap: w = c − a, with the base as a.
            let points = if kind == Kind::Sphere {
                [plus(c, w), plus(plus(c, w), d), c, O]
            } else {
                [c, plus(c, d), plus(c, w), O]
            };
            Input {
                kind,
                points,
                whole: [r, 0],
            }
        }
        Kind::Cross => {
            let a = vector(rng, 38);
            let (p, q) = if tangent {
                let p = vector(rng, 18);
                let k = rng.range(-(1 << 19), 1 << 19);
                let moved = if rng.next() & 1 == 0 {
                    O
                } else {
                    std::array::from_fn(|_| rng.range(-1, 1))
                };
                (p, plus(p.map(|v| v * k), moved))
            } else {
                (vector(rng, 38), vector(rng, 38))
            };
            Input {
                kind,
                points: [a, plus(a, p), c, plus(c, q)],
                whole: [0, 0],
            }
        }
        Kind::Cylinder => {
            let (d, e, w) = (vector(rng, 38), vector(rng, 38), vector(rng, 38));
            let limit = (1 << 41) - 1;
            let r = if tangent {
                let n = cross(&minus(d, O), &minus(e, O));
                let det = dot(&n, &minus(w, O));
                near(rng, real(&det).abs() / real(&dot(&n, &n)).sqrt(), limit)
            } else {
                any(rng, 41, limit)
            };
            Input {
                kind,
                points: [plus(c, w), plus(plus(c, w), d), c, plus(c, e)],
                whole: [r, 0],
            }
        }
        Kind::Cone | Kind::Lead => {
            let (d, e, w) = (vector(rng, 38), vector(rng, 38), vector(rng, 38));
            let limit = (1 << 40) - 1;
            let (db, eb, wb) = (minus(d, O), minus(e, O), minus(w, O));
            // s : t at the value where the result is zero. line_cone: s² (|e|² |n|² − (e · n)²) =
            // t² (e · n)². line_cone_lead: t² |d × e|² = s² (d · e)².
            let (rise, run) = if kind == Kind::Cone {
                let n = cross(&wb, &db);
                let en = squared(&dot(&eb, &n));
                (
                    real(&en).sqrt(),
                    real(&(dot(&eb, &eb) * dot(&n, &n) - en)).sqrt(),
                )
            } else {
                let u = cross(&db, &eb);
                (real(&dot(&u, &u)).sqrt(), real(&dot(&db, &eb)).abs())
            };
            let (s, t) = if tangent {
                let larger = any(rng, 39, limit).max(1);
                if rise <= run {
                    (near(rng, larger as f64 * rise / run, limit), larger)
                } else {
                    (larger, near(rng, larger as f64 * run / rise, limit))
                }
            } else {
                (any(rng, 40, limit), any(rng, 40, limit))
            };
            Input {
                kind,
                points: [plus(c, w), plus(plus(c, w), d), c, plus(c, e)],
                whole: [s, t],
            }
        }
    }
}

/// What a filter did over the random inputs.
#[derive(Default, Debug)]
struct Tally {
    positive: u64,
    negative: u64,
    zero_scale: u64,
    left: u64,
    /// Exact zeros with a scale above zero.
    zeros: u64,
    /// Inputs beyond three times the bound.
    clear: u64,
}

/// Every sign a filter gives is the exact sign; it decides every input beyond three times its
/// bound, never an exact zero with a scale above zero, and a zero scale at once. Its whole error is
/// at most its constant times the scale, and so is its computed bound to within a few parts in
/// 2^53, so a value beyond three times that is computed beyond the bound.
#[test]
fn filters_decide_what_is_clear_and_never_give_a_wrong_sign() {
    let mut rng = Rng::for_test("the quadric filters on random and near-tangent inputs");
    for kind in KINDS {
        let mut tally = Tally::default();
        for _ in 0..cases(100_000) {
            let input = random(kind, &mut rng);
            assert!(
                input
                    .points
                    .iter()
                    .flatten()
                    .all(|v| (-COORD_MAX..=COORD_MAX).contains(v))
            );
            let v = input.oracle();
            let sign = sign_of(&v);
            assert_eq!(input.value(), v, "{input:?}");
            assert_eq!(input.exact(), Some(sign), "{input:?}");
            let scale = input.scale();
            let filtered = input.filter();
            if let Some(s) = filtered {
                assert_eq!(s, sign, "{input:?}: the filter's sign is wrong");
            }
            if scale.is_zero() {
                assert_eq!(filtered, Some(Sign::Zero), "{input:?}");
                tally.zero_scale += 1;
                continue;
            }
            if v.is_zero() {
                assert_eq!(filtered, None, "{input:?}: an exact zero was decided");
                tally.zeros += 1;
            } else if real(&v).abs() > 3.0 * error_of(input.module()) * real(&scale) {
                assert_eq!(filtered, Some(sign), "{input:?}: a clear input was left");
                tally.clear += 1;
            }
            match filtered {
                Some(Sign::Positive) => tally.positive += 1,
                Some(Sign::Negative) => tally.negative += 1,
                Some(Sign::Zero) => panic!("{input:?}: Zero with a scale above zero"),
                None => tally.left += 1,
            }
        }
        println!("{kind:?}: {tally:?}");
        // cross_norm is a sum of squares: never negative.
        assert!(
            tally.positive > 0
                && (tally.negative > 0 || kind == Kind::Cross)
                && tally.left > 0
                && tally.zeros > 0
                && tally.zero_scale > 0
                && tally.clear > 0,
            "{kind:?}: an outcome never occurred: {tally:?}"
        );
    }
}
