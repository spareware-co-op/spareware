//! Fuzz target: `geom-num`'s exact decisions on lines and quadrics agree with big integers, and its
//! Sturm chains count the roots a polynomial was built from, on any bytes.
//!
//! What it does: [`decode`] turns a fuzzer's bytes into a [`Case`]: a line, a sphere, a cylinder and a
//! cone at small sizes or at the grid's edge, a torus, and four factors of a quartic. [`check`] holds
//! each discriminant to the same polynomial's discriminant in `num-bigint`, each filter to its exact
//! path, the torus's filter to its chain, and a chain to the roots of the factors it was built from.
//!
//! Guarantees:
//!
//! - **Any bytes are a case.** Every byte string, the empty one included, decodes to a case.
//! - **A bounded input.** [`decode`] reads at most [`MAX_INPUT_BYTES`] bytes, derived below; a unit
//!   test builds the input that reaches it.
//! - **A panic is a failure; a broken law is a [`Finding`]** naming the law and the case.
//!
//! Method: chapter 28.5's oracle stack (`projects/engines/geometry/architecture/28-testing.md`), with
//! `num-bigint` as the exact oracle of 29.3, sharing no code with `geom-num`'s wide integers.
//!
//! Cost: a handful of predicates and two Sturm chains per case; at most [`MAX_INPUT_BYTES`] bytes
//! read.

use geom_num::Sign;
use geom_num::grid::{COORD_MAX, Coord, P3, Steps};
use geom_num::interval::Interval;
use geom_num::quadric::{
    Cone, Cylinder, Line, Slope, Sphere, Surface, Torus, line_cone, line_cylinder, line_sphere,
    polynomial, roots,
};
use geom_num::sturm::{Big, Chain, filtered};
use geom_num::wide::I320;
use geom_test::{ByteSource, Entropy};
use num_bigint::BigInt;

use crate::Finding;

/// The most bytes [`decode`] reads from any input: a rule byte and a word for each of 12 coordinates,
/// a byte for each of 5 sizes, and 4 bytes for the quartic's factors: 12 · 9 + 5 + 4 = 117.
pub const MAX_INPUT_BYTES: usize = 117;

/// One input of the target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The line.
    pub line: Line,
    /// The sphere.
    pub sphere: Sphere,
    /// The cylinder.
    pub cylinder: Cylinder,
    /// The cone.
    pub cone: Cone,
    /// The torus.
    pub torus: Torus,
    /// The quartic's four small whole roots, or pairs read as complex factors.
    pub factors: [i8; 4],
}

fn coordinate(src: &mut ByteSource<'_>) -> i64 {
    let rule = src.byte();
    let word = i64::from_le_bytes(src.word().to_le_bytes());
    match rule % 4 {
        0 => word % 16,
        1 => word % 4096,
        2 => word % (COORD_MAX + 1),
        _ => {
            if word < 0 {
                -COORD_MAX
            } else {
                COORD_MAX
            }
        }
    }
}

fn point(src: &mut ByteSource<'_>) -> P3 {
    let (x, y, z) = (coordinate(src), coordinate(src), coordinate(src));
    P3::new(x, y, z).unwrap_or(P3 {
        x: Coord::ZERO,
        y: Coord::ZERO,
        z: Coord::ZERO,
    })
}

fn size(src: &mut ByteSource<'_>) -> Steps {
    Steps(u64::from(src.byte()).saturating_add(1))
}

/// The case in `data`, and how many bytes it read.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the points and sizes are named as in the description of each surface"
)]
pub fn decode(data: &[u8]) -> (Case, usize) {
    let mut src = ByteSource::new(data);
    let (a, b, c, f, g, h) = (
        point(&mut src),
        point(&mut src),
        point(&mut src),
        point(&mut src),
        size(&mut src),
        size(&mut src),
    );
    let (i, j, k) = (size(&mut src), size(&mut src), size(&mut src));
    let factors = [
        i8::from_le_bytes([src.byte()]),
        i8::from_le_bytes([src.byte()]),
        i8::from_le_bytes([src.byte()]),
        i8::from_le_bytes([src.byte()]),
    ];
    let case = Case {
        line: Line { a, b },
        sphere: Sphere {
            centre: c,
            radius: g,
        },
        cylinder: Cylinder {
            base: c,
            along: f,
            radius: h,
        },
        cone: Cone {
            apex: c,
            along: f,
            slope: Slope {
                rise: i.0,
                run: j.0,
            },
        },
        torus: Torus {
            centre: c,
            along: f,
            major: k.checked_add(g).unwrap_or(k),
            minor: g,
        },
        factors,
    };
    (case, data.len().saturating_sub(src.remaining()))
}

#[allow(
    clippy::arithmetic_side_effects,
    reason = "big integers, which cannot overflow"
)]
fn bigint(v: &I320) -> BigInt {
    let mut out = BigInt::from(0);
    for (i, limb) in v.to_le_limbs().iter().enumerate() {
        out += BigInt::from(*limb) << i.saturating_mul(64);
    }
    if v.is_negative() {
        out -= BigInt::from(1) << 320_u32;
    }
    out
}

/// The sign of `f`'s discriminant, in big integers.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "big integers, which cannot overflow; a, b, c and d are the discriminant's own names"
)]
fn discriminant(line: &Line, surface: &Surface) -> Result<Sign, Finding> {
    let (f, _) = polynomial::<I320>(line, surface)
        .ok_or_else(|| Finding::new("a quadric's polynomial fits", || format!("{surface:?}")))?;
    let [c, b, a, ..] = f;
    let d = bigint(&b) * bigint(&b) - BigInt::from(4) * bigint(&a) * bigint(&c);
    Ok(match d.sign() {
        num_bigint::Sign::Plus => Sign::Positive,
        num_bigint::Sign::NoSign => Sign::Zero,
        num_bigint::Sign::Minus => Sign::Negative,
    })
}

/// The polynomial of four factors: (t − a)(t − b), or t² + a² + 1 when a is even, in turn.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "products of at most four numbers of 8 bits fit an i128; the names are the factors'"
)]
fn built(factors: [i8; 4]) -> ([Big; 5], u8, bool) {
    let [a, b, c, d] = factors.map(i128::from);
    let quadratic = |x: i128, y: i128| -> ([i128; 3], Vec<i128>) {
        if x % 2 == 0 {
            ([x * x + 1, 0, 1], Vec::new())
        } else {
            ([x * y, -(x + y), 1], vec![x, y])
        }
    };
    let (p, first) = quadratic(a, b);
    let (q, second) = quadratic(c, d);
    let mut out = [Big::ZERO; 5];
    for (i, u) in p.iter().enumerate() {
        for (j, v) in q.iter().enumerate() {
            if let Some(slot) = out.get_mut(i.saturating_add(j)) {
                *slot = slot
                    .checked_add(&Big::from_i128(u * v))
                    .unwrap_or(Big::ZERO);
            }
        }
    }
    let mut real: Vec<i128> = first.iter().chain(&second).copied().collect();
    let total = real.len();
    real.sort_unstable();
    real.dedup();
    let distinct = u8::try_from(real.len()).unwrap_or(u8::MAX);
    // A complex pair can only repeat when its two factors are the same quadratic.
    let same = first.is_empty() && second.is_empty() && p == q;
    (out, distinct, real.len() < total || same)
}

/// `case` held to every law.
///
/// # Errors
///
/// The first law it breaks.
pub fn check(case: &Case) -> Result<(), Finding> {
    let detail = || format!("{case:?}");
    let line = case.line;
    if line.a == line.b {
        return Ok(());
    }
    let wrong = |law| Err(Finding::new(law, detail));
    let sphere = Surface::Sphere(case.sphere);
    let cylinder = Surface::Cylinder(case.cylinder);
    let cone = Surface::Cone(case.cone);
    if line_sphere(&line, &case.sphere).ok() != Some(discriminant(&line, &sphere)?) {
        return wrong("line_sphere is its polynomial's discriminant");
    }
    if case.cylinder.base != case.cylinder.along {
        if line_cylinder(&line, &case.cylinder).ok() != Some(discriminant(&line, &cylinder)?) {
            return wrong("line_cylinder is its polynomial's discriminant");
        }
        if line_cone(&line, &case.cone).ok() != Some(discriminant(&line, &cone)?) {
            return wrong("line_cone is its polynomial's discriminant");
        }
        let torus = Surface::Torus(case.torus);
        let exact = polynomial::<I320>(&line, &torus).and_then(|(f, _)| {
            let mut wide = [Big::ZERO; 5];
            for (slot, c) in wide.iter_mut().zip(&f) {
                *slot = c.resize()?;
            }
            Chain::of(&wide).ok().map(|c| c.roots())
        });
        let Some(exact) = exact else {
            return wrong("a line's quartic with a torus has a chain");
        };
        let quick = polynomial::<Interval>(&line, &torus).and_then(|(f, d)| filtered(&f, d));
        if quick.is_some_and(|q| q != exact) {
            return wrong("the torus's filter agrees with its chain");
        }
        if roots(&line, &torus).ok() != Some(exact) {
            return wrong("roots is the chain's count");
        }
    }
    let (f, distinct, repeated) = built(case.factors);
    let Ok(chain) = Chain::of(&f) else {
        return wrong("a quartic has a chain");
    };
    let found = chain.roots();
    if found.distinct != distinct || (repeated && !found.repeated) {
        return wrong("a chain counts the roots a polynomial was built from");
    }
    Ok(())
}

/// `decode` then `check`: the plain function every engine drives.
///
/// # Errors
///
/// The first law the case breaks.
pub fn harness(data: &[u8]) -> Result<(), Finding> {
    check(&decode(data).0)
}
