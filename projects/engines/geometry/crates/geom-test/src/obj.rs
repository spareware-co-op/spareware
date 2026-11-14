//! A test-only reader of Wavefront OBJ meshes onto the grid: the polyhedral corpus's way in for
//! Manifold's regression models (chapter 11.17's item 15; Gate 4, G156), before the kernel's own
//! readers (chapter 24) exist.
//!
//! What it does: [`read`] takes the `v` and `f` lines of an OBJ file and nothing else (texture
//! coordinates, normals, groups and materials are skipped). Each coordinate is read as the exact
//! decimal it is written as: its digits are one integer, parsed by `dashu`'s integer parser, and
//! its point and exponent a power of ten, so `13009.6982000000007246854` is exactly that number.
//! [`Read::to_grid`] then puts the mesh on the grid at one scale for the whole model:
//!
//! - **The scale.** Grid steps of `10^-k` model units, `k` the largest whole number (it may be
//!   negative) for which every coordinate of the model, times `10^k`, lies within ±[`REACH`]
//!   (2^30), which leaves the grid's ±(2^40 − 1) room for a moved copy.
//! - **The rounding.** Each coordinate times `10^k` is rounded to the nearest whole step, a tie to
//!   the even step. Nothing else changes: corners that round to one point stay two corners of the
//!   list, and the mesh checker decides what becomes of them.
//!
//! A face's corners are written `i`, `i/t`, `i//n` or `i/t/n`, counted from 1, or from the end
//! when negative.
//!
//! Why not a crate: `tobj` (MIT), the usual OBJ reader, gives positions as `f32` or `f64`, which
//! rounds a decimal before it reaches the grid, and this reading must be exact. The two kinds of
//! line read here are split on white space; every number is parsed by `dashu-int` and the
//! standard library.
//!
//! Guarantees:
//!
//! - **Exact.** No coordinate passes through a double: the grid value is the written decimal
//!   rounded once, half to even, at the stated scale.
//! - **Refused, never guessed.** A malformed number, a face of fewer than three corners, an index
//!   outside the vertex list and a file without vertices are [`ObjError`]s, with the line.
//! - **The same on every target.** Integer arithmetic only.
//!
//! Method: the OBJ format's `v` and `f` statements as Wavefront's specification gives them
//! (Wavefront Technologies, "Object Files (.obj)", Advanced Visualizer manual, appendix B1,
//! <https://paulbourke.net/dataformats/obj/>); rounding half to even, IEEE 754-2019's
//! roundTiesToEven applied to an exact decimal (<https://doi.org/10.1109/IEEESTD.2019.8766229>).
//!
//! Cost: linear in the file; each coordinate costs a big-integer parse and, at the scale, one
//! division by a power of ten.

// The lint is for fixed-width arithmetic, which can overflow. Every operator here is on dashu's
// arbitrary-precision integers, which cannot, and every division is by a power of ten.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "arbitrary-precision integers, which cannot overflow"
)]

use core::fmt;
use core::str::FromStr;

use dashu_int::IBig;

/// How far from the origin a coordinate may reach once on the grid: 2^30 steps.
pub const REACH: i64 = 1 << 30;

/// A coordinate as written: `digits × 10^exponent`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    /// The digits, with the sign.
    pub digits: IBig,
    /// The power of ten they are scaled by.
    pub exponent: i64,
}

/// A mesh as the file writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// The vertices, in the file's order.
    pub vertices: Vec<[Decimal; 3]>,
    /// The faces, each its corners as indices into `vertices` from 0.
    pub faces: Vec<Vec<u32>>,
}

/// A mesh on the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    /// The vertices, in steps of `10^-scale` model units.
    pub points: Vec<[i64; 3]>,
    /// The faces, as read.
    pub faces: Vec<Vec<u32>>,
    /// `k`: one step is `10^-k` model units.
    pub scale: i64,
}

/// Why a file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjError {
    /// A `v` line without three numbers, or a number that does not parse. The line, from 1.
    Vertex(usize),
    /// An `f` line with fewer than three corners, or a corner that is not an index of a vertex.
    /// The line, from 1.
    Face(usize),
    /// No vertex at all.
    Empty,
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjError::Vertex(line) => write!(f, "line {line}: not a vertex of three numbers"),
            ObjError::Face(line) => write!(f, "line {line}: not a face of three or more vertices"),
            ObjError::Empty => write!(f, "no vertices"),
        }
    }
}

impl std::error::Error for ObjError {}

/// `10^n`.
fn ten_to(n: u64) -> IBig {
    let mut out = IBig::ONE;
    for _ in 0..n {
        out *= IBig::from(10_u8);
    }
    out
}

/// A decimal number, `[-+]digits[.digits][(e|E)[-+]digits]`, exactly.
fn decimal(text: &str) -> Option<Decimal> {
    let (mantissa, power) = match text.find(['e', 'E']) {
        Some(at) => (
            text.get(..at)?,
            text.get(at.checked_add(1)?..)?.parse::<i64>().ok()?,
        ),
        None => (text, 0),
    };
    let (sign, unsigned) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa.strip_prefix('+').unwrap_or(mantissa)),
    };
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let plain = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() && fraction.is_empty() || !plain(whole) || !plain(fraction) {
        return None;
    }
    let digits = IBig::from_str(&format!("{sign}{whole}{fraction}0")).ok()?;
    let places = i64::try_from(fraction.len()).ok()?.checked_add(1)?;
    Some(Decimal {
        digits,
        exponent: power.checked_sub(places)?,
    })
}

/// A face's corner: its vertex index, from 0.
fn corner(text: &str, vertices: usize) -> Option<u32> {
    let index: i64 = text.split('/').next()?.parse().ok()?;
    let count = i64::try_from(vertices).ok()?;
    let from_zero = if index < 0 {
        count.checked_add(index)?
    } else {
        index.checked_sub(1)?
    };
    (0..count)
        .contains(&from_zero)
        .then(|| u32::try_from(from_zero).ok())
        .flatten()
}

/// Reads the vertices and faces of an OBJ file.
///
/// # Errors
///
/// As [`ObjError`] says.
pub fn read(text: &str) -> Result<Read, ObjError> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let number = row.saturating_add(1);
        let mut words = line.split_whitespace();
        match words.next() {
            Some("v") => {
                let mut coordinate = || words.next().and_then(decimal);
                let (Some(x), Some(y), Some(z)) = (coordinate(), coordinate(), coordinate()) else {
                    return Err(ObjError::Vertex(number));
                };
                vertices.push([x, y, z]);
            }
            Some("f") => {
                let face: Option<Vec<u32>> = words.map(|w| corner(w, vertices.len())).collect();
                match face {
                    Some(face) if face.len() >= 3 => faces.push(face),
                    _ => return Err(ObjError::Face(number)),
                }
            }
            _ => {}
        }
    }
    if vertices.is_empty() {
        return Err(ObjError::Empty);
    }
    Ok(Read { vertices, faces })
}

/// Whether `|d| × 10^k` is at most [`REACH`].
fn within(d: &Decimal, k: i64) -> bool {
    let reach = IBig::from(REACH);
    let magnitude = if d.digits < IBig::ZERO {
        -d.digits.clone()
    } else {
        d.digits.clone()
    };
    let Some(power) = d.exponent.checked_add(k) else {
        return false;
    };
    let size = power.unsigned_abs();
    if power >= 0 {
        magnitude * ten_to(size) <= reach
    } else {
        magnitude <= reach * ten_to(size)
    }
}

/// `d × 10^k` rounded to a whole number, a tie to the even one.
fn rounded(d: &Decimal, k: i64) -> Option<i64> {
    let power = d.exponent.checked_add(k)?;
    let size = power.unsigned_abs();
    let whole = if power >= 0 {
        &d.digits * ten_to(size)
    } else {
        let divisor = ten_to(size);
        let (quotient, remainder) = floor_div(&d.digits, &divisor);
        let twice = remainder * IBig::from(2_u8);
        let up = twice > divisor
            || (twice == divisor && quotient.clone() % IBig::from(2_u8) != IBig::ZERO);
        if up { quotient + IBig::ONE } else { quotient }
    };
    i64::try_from(whole).ok()
}

/// `a` divided by the positive `b`, rounded down, and the remainder, from 0 up to `b`.
fn floor_div(a: &IBig, b: &IBig) -> (IBig, IBig) {
    let (mut quotient, mut remainder) = (a / b, a % b);
    if remainder < IBig::ZERO {
        quotient -= IBig::ONE;
        remainder += b;
    }
    (quotient, remainder)
}

impl Read {
    /// The scale [`Read::to_grid`] uses: the largest `k` that keeps every coordinate within
    /// [`REACH`], or `None` for a model with every coordinate zero.
    #[must_use]
    pub fn scale(&self) -> Option<i64> {
        // A coordinate of d digits and exponent e is below 10^(d + e); 2^30 is above 10^9.
        let mut k: Option<i64> = None;
        for d in self.vertices.iter().flatten() {
            if d.digits == IBig::ZERO {
                continue;
            }
            let digits = i64::try_from(d.digits.to_string().trim_start_matches('-').len()).ok()?;
            let mut guess = 9_i64.checked_sub(digits)?.checked_sub(d.exponent)?;
            while !within(d, guess) {
                guess = guess.checked_sub(1)?;
            }
            while within(d, guess.checked_add(1)?) {
                guess = guess.checked_add(1)?;
            }
            k = Some(k.map_or(guess, |held| held.min(guess)));
        }
        k
    }

    /// The mesh on the grid at [`Read::scale`]; `None` for a model at the origin alone.
    #[must_use]
    pub fn to_grid(&self) -> Option<Grid> {
        let scale = self.scale()?;
        let points: Option<Vec<[i64; 3]>> = self
            .vertices
            .iter()
            .map(|[x, y, z]| Some([rounded(x, scale)?, rounded(y, scale)?, rounded(z, scale)?]))
            .collect();
        Some(Grid {
            points: points?,
            faces: self.faces.clone(),
            scale,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a failure should stop loudly"
    )]

    use super::*;

    fn d(text: &str) -> Decimal {
        decimal(text).unwrap()
    }

    #[test]
    fn decimals_are_read_exactly() {
        assert_eq!(
            d("1.5"),
            Decimal {
                digits: IBig::from(150),
                exponent: -2
            }
        );
        assert_eq!(
            d("-0.25e3"),
            Decimal {
                digits: IBig::from(-250),
                exponent: 0
            }
        );
        assert_eq!(
            d("+7"),
            Decimal {
                digits: IBig::from(70),
                exponent: -1
            }
        );
        assert_eq!(
            d(".5"),
            Decimal {
                digits: IBig::from(50),
                exponent: -2
            }
        );
        assert_eq!(
            d("3."),
            Decimal {
                digits: IBig::from(30),
                exponent: -1
            }
        );
        for bad in [
            "", ".", "-", "1e", "1.2.3", "0x10", "1,5", "nan", "inf", "- 1",
        ] {
            assert_eq!(decimal(bad), None, "{bad}");
        }
    }

    #[test]
    fn rounding_is_half_to_even() {
        for (text, k, want) in [
            ("0.5", 0, 0),
            ("1.5", 0, 2),
            ("2.5", 0, 2),
            ("-0.5", 0, 0),
            ("-1.5", 0, -2),
            ("-2.5", 0, -2),
            ("2.5000001", 0, 3),
            ("-2.4999999", 0, -2),
            ("1234.5678", 2, 123_457),
            ("1234.5678", -2, 12),
            ("1250", -2, 12),
            ("1350", -2, 14),
        ] {
            assert_eq!(rounded(&d(text), k), Some(want), "{text} at {k}");
        }
    }

    #[test]
    fn the_scale_is_the_finest_that_keeps_the_model_within_reach() {
        let mesh = read("v 1.5 -2 0\nv 1073741824 0 0\nf 1 2 2\n").unwrap();
        assert_eq!(mesh.scale(), Some(0));
        let small = read("v 0.001 0 0\nv 0 -0.002 0\n").unwrap();
        // 0.002 × 10^11 = 2 × 10^8 <= 2^30 < 2 × 10^9.
        assert_eq!(small.scale(), Some(11));
        let huge = read("v 3.4e38 0 0\n").unwrap();
        assert_eq!(huge.scale(), Some(-30));
        assert_eq!(huge.to_grid().unwrap().points, [[340_000_000, 0, 0]]);
        assert_eq!(read("v 0 0 0\n").unwrap().to_grid(), None);
    }

    #[test]
    fn faces_take_every_corner_form() {
        let mesh =
            read("# c\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nf 1/1 2//3 -1/2/3\ng x\n").unwrap();
        assert_eq!(mesh.faces, [vec![0, 1, 2]]);
        assert_eq!(read("v 0 0 0\nf 1 2 3\n"), Err(ObjError::Face(2)));
        assert_eq!(read("v 0 0 0\nv 1 0 0\nf 1 2\n"), Err(ObjError::Face(3)));
        assert_eq!(read("v 0 0\n"), Err(ObjError::Vertex(1)));
        assert_eq!(read("f 1 2 3\n"), Err(ObjError::Face(1)));
        assert_eq!(read("# nothing\n"), Err(ObjError::Empty));
    }
}
