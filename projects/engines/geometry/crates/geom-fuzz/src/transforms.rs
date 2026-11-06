//! Fuzz target: the typed transforms move grid points exactly as their definitions say, and
//! decimals round-trip, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a grid point, two grid
//! transforms (signed permutations with whole-step translations), a rational transform
//! (p ↦ (M·p + t)/d with small integers), a rational rotation (an integer quaternion), and a short
//! text. [`check`] holds four laws, each decided exactly in rationals:
//!
//! - **A grid transform composes and inverts exactly** (piece G45): (g ∘ h)(p) = g(h(p)) and
//!   g⁻¹(g(p)) = p.
//! - **A rational transform's image is its exact image rounded once to the grid**, ties to even,
//!   and says it is exact exactly when no rounding happened (G45, G46).
//! - **A rational rotation's image is likewise its exact image M(q)·p/|q|² rounded once** (G46).
//! - **A decimal round-trips**: text `geom_math::Decimal` accepts, written back, parses to the same
//!   value (G48).
//!
//! [`harness`] is `decode` then `check`, the function every fuzzing engine drives.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Coordinates are drawn through rule bytes that make the frame's
//!   limits ±(2^40 − 1), 0 and ±1 common; a transform the constructors refuse, or an image out of
//!   the frame's range, is a typed error and legal.
//! - **A panic is a failure; a broken law is a [`Finding`].**
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below.
//!
//! Method: each transform's definition in `geom-math` (chapter 7's typed transforms,
//! `projects/engines/geometry/architecture/07-geometry-types.md`), evaluated exactly in dashu's
//! rationals, and round-half-to-even read from the exact quotient; coverage-guided fuzzing
//! (chapter 28.5).
//!
//! Cost: a handful of transform applications and one decimal parse per input; at most
//! [`MAX_INPUT_BYTES`] bytes read.

// The lint is for fixed-width arithmetic, which can overflow. The oracle's arithmetic is on
// dashu's arbitrary-precision numbers, which cannot.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "exact arithmetic in the oracle, which cannot overflow"
)]
#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': x, y, z coordinates, p a point, g and h transforms, m, t, d and q a transform's parts"
)]

use dashu_ratio::RBig;
use geom_math::{Axis, Decimal, GridTransform, RationalRotation, RationalTransform};
use geom_num::grid::{COORD_MAX, P3};
use geom_test::{ByteSource, Entropy};

use crate::Finding;

/// The longest text a case reads.
pub const TEXT: usize = 24;

/// The most bytes [`decode`] reads from any input: 97.
///
/// A point of three coordinates (a rule byte and at most eight bytes each), two grid transforms of
/// two bytes, a rational transform of nine one-byte entries, three translation coordinates (a
/// rule byte and at most eight bytes each) and a denominator byte, a quaternion of four bytes, a
/// length byte and [`TEXT`] text bytes: 27 + 4 + 9 + 27 + 1 + 4 + 1 + 24 = 97. The unit test
/// `the_input_bound_is_reached_and_never_passed` builds an input that reads exactly this many.
pub const MAX_INPUT_BYTES: usize = 27 + 4 + 9 + 27 + 1 + 4 + 1 + TEXT;

/// One input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The grid point's coordinates.
    pub point: [i64; 3],
    /// Two grid transforms: quarter turns about an axis, a mirror, and a whole-step translation,
    /// each from two bytes.
    pub grid: [[u8; 2]; 2],
    /// The rational transform's matrix entries, −4 to 4.
    pub m: [[i64; 3]; 3],
    /// Its translation.
    pub t: [i64; 3],
    /// Its denominator, 1 to 8.
    pub d: i64,
    /// The rotation's quaternion, each −8 to 7.
    pub q: [i64; 4],
    /// The text to parse as a decimal.
    pub text: Vec<u8>,
}

/// One coordinate through its rule byte: the limits and their neighbours, 0, ±1, a small number,
/// or a word read from the input, folded into the frame's range.
fn coord(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    match rule & 7 {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => COORD_MAX,
        4 => 0i64.saturating_sub(COORD_MAX),
        5 => i64::from(rule >> 3),
        6 => 0i64.saturating_sub(i64::from(rule >> 3)),
        _ => {
            let w = src.word();
            let span = u64::try_from(COORD_MAX)
                .unwrap_or(1)
                .saturating_mul(2)
                .saturating_add(1);
            let folded = w.checked_rem(span).unwrap_or(0);
            i64::try_from(folded).unwrap_or(0).saturating_sub(COORD_MAX)
        }
    }
}

/// A small signed integer from a byte: `byte mod (2·half + 1) − half`.
fn small(src: &mut ByteSource<'_>, half: u8) -> i64 {
    let span = half.saturating_mul(2).saturating_add(1);
    i64::from(src.byte().checked_rem(span).unwrap_or(0)) - i64::from(half)
}

/// Any bytes as a case, and how many bytes were read.
#[must_use]
pub fn decode(bytes: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(bytes);
    let point = [coord(&mut src), coord(&mut src), coord(&mut src)];
    let mut grid = [[0u8; 2]; 2];
    for g in &mut grid {
        *g = [src.byte(), src.byte()];
    }
    let mut m = [[0i64; 3]; 3];
    for row in &mut m {
        for e in row.iter_mut() {
            *e = small(&mut src, 4);
        }
    }
    let t = [coord(&mut src), coord(&mut src), coord(&mut src)];
    let d = i64::from(src.byte() & 7).saturating_add(1);
    let q = [
        small(&mut src, 8),
        small(&mut src, 8),
        small(&mut src, 8),
        small(&mut src, 8),
    ];
    let len = usize::from(src.byte())
        .checked_rem(TEXT.saturating_add(1))
        .unwrap_or(0);
    let text: Vec<u8> = (0..len).map(|_| src.byte()).collect();
    let case = Case {
        point,
        grid,
        m,
        t,
        d,
        q,
        text,
    };
    let read = bytes.len().saturating_sub(src.remaining());
    (case, read)
}

/// The grid transform of two bytes: quarter turns about an axis, then a mirror, then a whole-step
/// translation.
fn grid(bytes: [u8; 2]) -> Option<GridTransform> {
    let [a, b] = bytes;
    let axis = match a & 3 {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    };
    let turn = GridTransform::quarter_turns(axis, i32::from(a >> 2 & 3));
    let mirrored = if a & 0x10 == 0 {
        turn
    } else {
        GridTransform::mirror(axis).compose(&turn).ok()?
    };
    let k = i64::from(b) - 128;
    let shift =
        GridTransform::translation([k, k.saturating_mul(3), 0i64.saturating_sub(k)]).ok()?;
    shift.compose(&mirrored).ok()
}

/// x exactly.
fn rat(x: i128) -> RBig {
    RBig::from(x)
}

/// x rounded half to even, as an integer; `None` beyond `i128`.
fn round_even(x: &RBig) -> Option<i128> {
    let f = i128::try_from(x.floor()).ok()?;
    let frac = x - rat(f);
    let half = rat(1) / rat(2);
    let odd = f.checked_rem(2).is_some_and(|r| r != 0);
    let up = frac > half || (frac == half && odd);
    if up { f.checked_add(1) } else { Some(f) }
}

/// Whether `got` is `exact` rounded half to even, coordinate by coordinate, and `is_exact` says
/// whether it equals `exact`.
fn rounds(got: P3, is_exact: bool, exact: &[RBig; 3]) -> bool {
    let g = [got.x.get(), got.y.get(), got.z.get()];
    let integral = exact.iter().all(RBig::is_int);
    g.iter()
        .zip(exact)
        .all(|(&v, x)| round_even(x) == Some(i128::from(v)))
        && is_exact == integral
}

/// The rounding law on given values: whether `got`, with its exactness `is_exact`, is
/// `numer`/`d` rounded half to even, coordinate by coordinate. It is the check the two image laws
/// make, given here so tests can show it fails on a wrong image (28.4, fault injection).
///
/// # Errors
///
/// A [`Finding`] when it is not, or `got` is not a grid point.
pub fn check_rounding(
    got: [i64; 3],
    is_exact: bool,
    numer: [i64; 3],
    d: i64,
) -> Result<(), Finding> {
    let [x, y, z] = got;
    let ok = P3::new(x, y, z).is_ok_and(|p| {
        let exact = numer.map(|n| rat(i128::from(n)) / rat(i128::from(d)));
        rounds(p, is_exact, &exact)
    });
    if ok {
        Ok(())
    } else {
        Err(Finding::new(
            "an image is its exact image rounded once, ties to even",
            || format!("{got:?} ({is_exact}) against {numer:?}/{d}"),
        ))
    }
}

/// The four laws on the case (module documentation).
///
/// # Errors
///
/// A [`Finding`] for the first law broken.
pub fn check(case: &Case) -> Result<(), Finding> {
    let [x, y, z] = case.point;
    let Ok(p) = P3::new(x, y, z) else {
        return Ok(());
    };

    // Grid transforms compose and invert exactly.
    let [first, second] = case.grid;
    if let (Some(g), Some(h)) = (grid(first), grid(second)) {
        if let (Ok(gh), Ok(hp)) = (g.compose(&h), h.apply(p))
            && let (Ok(a), Ok(b)) = (gh.apply(p), g.apply(hp))
            && a != b
        {
            return Err(Finding::new(
                "(g ∘ h)(p) = g(h(p)) for grid transforms",
                || format!("g {g:?}, h {h:?}, p {p:?}: {a:?} against {b:?}"),
            ));
        }
        if let (Ok(inv), Ok(gp)) = (g.inverse(), g.apply(p))
            && inv.apply(gp) != Ok(p)
        {
            return Err(Finding::new("g⁻¹(g(p)) = p for grid transforms", || {
                format!("g {g:?}, p {p:?}")
            }));
        }
    }

    // A rational transform's image is its exact image rounded once.
    let t = case.t.map(i128::from);
    if let Ok(r) = RationalTransform::new(case.m, t, case.d)
        && let Ok(got) = r.apply(p)
    {
        let (m, tr, d) = (
            r.matrix(),
            r.translation_part(),
            rat(i128::from(r.denominator())),
        );
        let pc = [x, y, z].map(i128::from);
        let exact: [RBig; 3] = core::array::from_fn(|i| {
            let row = m.get(i).copied().unwrap_or([0; 3]);
            let s = row.iter().zip(&pc).fold(RBig::ZERO, |acc, (&a, &b)| {
                acc + rat(i128::from(a)) * rat(b)
            });
            (s + rat(tr.get(i).copied().unwrap_or(0))) / &d
        });
        if !rounds(got.value(), got.is_exact(), &exact) {
            return Err(Finding::new(
                "a rational transform's image is its exact image rounded once, ties to even",
                || format!("{r:?} at {p:?}: {got:?}"),
            ));
        }
    }

    // A rational rotation's image is M(q)·p/|q|² rounded once.
    if let Ok(rot) = RationalRotation::new(case.q)
        && let (Ok(got), Ok(m)) = (rot.apply(p), rot.matrix())
    {
        let n = RBig::from(rot.norm2());
        let pc = [x, y, z].map(i128::from);
        let exact: [RBig; 3] = core::array::from_fn(|i| {
            let row = m.get(i).copied().unwrap_or([0; 3]);
            let s = row
                .iter()
                .zip(&pc)
                .fold(RBig::ZERO, |acc, (&a, &b)| acc + rat(a) * rat(b));
            s / &n
        });
        if !rounds(got.value(), got.is_exact(), &exact) {
            return Err(Finding::new(
                "a rational rotation's image is M(q)·p/|q|² rounded once, ties to even",
                || format!("{rot:?} at {p:?}: {got:?}"),
            ));
        }
    }

    // A decimal round-trips.
    if let Ok(text) = core::str::from_utf8(&case.text)
        && let Ok(d) = Decimal::parse(text)
        && Decimal::parse(&d.to_string()) != Ok(d)
    {
        return Err(Finding::new(
            "a decimal written back parses to itself",
            || format!("{text:?} → {d:?} → {:?}", d.to_string()),
        ));
    }
    Ok(())
}

/// [`decode`], then [`check`].
///
/// # Errors
///
/// As [`check`].
pub fn harness(bytes: &[u8]) -> Result<(), Finding> {
    check(&decode(bytes).0)
}
