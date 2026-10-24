//! Exact transforms of the grid: whole-step translations, quarter turns about the axes, mirrors.
//!
//! These are the transforms that keep every grid point on the grid (chapter 7: they "stay exact").
//! Their linear parts are exactly the 48 signed permutation matrices, the symmetries of the cube:
//! in each row and each column one entry is +1 or −1 and the others are 0. A [`GridTransform`] is
//! one of them plus a translation by whole grid steps:
//!
//! ```text
//! out[i] = ±in[source[i]] + translation[i]      for i = x, y, z
//! ```
//!
//! Guarantees:
//!
//! - **Exact.** Applying, composing and inverting use integer arithmetic only. Composition and
//!   inversion of signed permutations are again signed permutations, so nothing ever rounds.
//! - **Range-checked.** Applying to a point checks that the result stays within the frame's
//!   ±(2^40 − 1) steps and returns [`NumError::OutOfRange`] otherwise. Translations are kept within
//!   [`TRANSLATION_MAX`] = 2(2^40 − 1) steps per axis, the largest that can map some grid point to
//!   another; a composition whose translation would exceed it returns `OutOfRange` too.
//! - **Orientation known exactly.** The determinant is ±1, computed from the permutation's parity
//!   and the signs; [`GridTransform::reverses_orientation`] says whether a mirror is involved,
//!   which reverses arcs, face normals and winding numbers.
//! - **The same result in every build profile.** Every integer step (a negation, a sum) is a
//!   checked method, never a native operator, so nothing depends on overflow checks being on. The
//!   bit budget below proves each fits; one that did not would be reported as
//!   [`NumError::BudgetExceeded`], never answered.
//!
//! Bit budget of [`GridTransform::apply`]: |in| ≤ 2^40 − 1 and |translation| ≤ 2^41 − 2, so each
//! output is below 2^42 in magnitude and is computed in `i64` without overflow before its range
//! check. Negations are of values at most 2^41 − 2 in magnitude, which `i64` holds with their
//! opposites; a composed translation is a sum of two of them, below 2^42, in `i128`.
//!
//! Method: the hyperoctahedral group B₃ (the symmetry group of the cube) represented as signed
//! permutations, composed by composing the permutations and multiplying the signs, with the
//! inverse given by the transpose, as in A. Björner and F. Brenti, *Combinatorics of Coxeter
//! Groups*, Graduate Texts in Mathematics 231, Springer (2005), section 8.1 (the group `B_n` of
//! signed permutations), <https://doi.org/10.1007/3-540-27596-7>; chapter 7 of the architecture
//! names which transforms stay exact
//! (`projects/engines/geometry/architecture/07-geometry-types.md`).
//!
//! Cost: constant time, no allocation. Applying is, per axis, one pick, an optional negation, one
//! addition and one range check; composing and inverting rebuild three rows (composing checks
//! three translations); the determinant counts at most three inversions and three negations.

use geom_num::NumError;
use geom_num::grid::{COORD_MAX, Coord, P3};

use crate::error::budget;
use crate::vector::V3;

/// The largest translation per axis, 2(2^40 − 1) steps: the largest distance between two grid
/// coordinates of one frame.
pub const TRANSLATION_MAX: i64 = 2 * COORD_MAX;

/// A coordinate axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    /// The x axis (easting in a map frame).
    X,
    /// The y axis (northing in a map frame).
    Y,
    /// The z axis (height).
    Z,
}

impl Axis {
    /// The three axes in order.
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    /// 0, 1 or 2.
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    /// The component of `v` along this axis.
    #[must_use]
    pub fn pick<T: Copy>(self, v: [T; 3]) -> T {
        let [x, y, z] = v;
        match self {
            Axis::X => x,
            Axis::Y => y,
            Axis::Z => z,
        }
    }

    /// The two other axes in cyclic order: (Y, Z) for X, (Z, X) for Y, (X, Y) for Z. A positive
    /// quarter turn about this axis carries the first onto the second (the right-hand rule).
    #[must_use]
    pub const fn others(self) -> (Axis, Axis) {
        match self {
            Axis::X => (Axis::Y, Axis::Z),
            Axis::Y => (Axis::Z, Axis::X),
            Axis::Z => (Axis::X, Axis::Y),
        }
    }
}

/// One row of a signed permutation plus translation: `out = ±in[source] + offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Row {
    source: Axis,
    negate: bool,
    offset: i64,
}

impl Row {
    /// The identity row for an axis.
    const fn identity(axis: Axis) -> Row {
        Row {
            source: axis,
            negate: false,
            offset: 0,
        }
    }

    /// `±v[source]`, without the offset; `None` only for a negation that does not fit, which the
    /// callers' bounds (|v| ≤ 2^41 − 2) rule out.
    fn linear(self, v: [i64; 3]) -> Option<i64> {
        let x = self.source.pick(v);
        if self.negate {
            x.checked_neg()
        } else {
            Some(x)
        }
    }
}

/// An exact transform of the grid: a signed permutation of the axes followed by a translation by
/// whole grid steps.
///
/// Made only by the constructors and operations below, so the rows always form a permutation and
/// every translation component is at most [`TRANSLATION_MAX`] in magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridTransform {
    rows: [Row; 3],
}

/// A translation component, or `OutOfRange` beyond [`TRANSLATION_MAX`].
fn translation_component(v: i128) -> Result<i64, NumError> {
    if v.unsigned_abs() > u128::from(TRANSLATION_MAX.unsigned_abs()) {
        return Err(NumError::OutOfRange { value: v });
    }
    i64::try_from(v).map_err(|_| NumError::OutOfRange { value: v })
}

impl GridTransform {
    /// The identity.
    pub const IDENTITY: GridTransform = GridTransform {
        rows: [
            Row::identity(Axis::X),
            Row::identity(Axis::Y),
            Row::identity(Axis::Z),
        ],
    };

    /// The translation by `t` grid steps.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a component exceeds [`TRANSLATION_MAX`] in magnitude.
    pub fn translation(t: [i64; 3]) -> Result<GridTransform, NumError> {
        GridTransform::from_parts([Axis::X, Axis::Y, Axis::Z], [false; 3], t)
    }

    /// The rotation by `turns` quarter turns about `axis` through the origin, counter-clockwise
    /// when looking from the positive end of the axis towards the origin (the right-hand rule).
    /// Any integer is accepted; it is taken modulo 4.
    #[must_use]
    pub fn quarter_turns(axis: Axis, turns: i32) -> GridTransform {
        let (a, b) = axis.others();
        // Rows for the two other axes, as (source of a's row, negate, source of b's row, negate).
        // One quarter turn maps (a, b) to (−b, a): new a = −old b, new b = old a.
        let (sa, na, sb, nb) = match turns.rem_euclid(4) {
            0 => (a, false, b, false),
            1 => (b, true, a, false),
            2 => (a, true, b, true),
            _ => (b, false, a, true),
        };
        let row = |out: Axis| -> Row {
            if out == a {
                Row {
                    source: sa,
                    negate: na,
                    offset: 0,
                }
            } else if out == b {
                Row {
                    source: sb,
                    negate: nb,
                    offset: 0,
                }
            } else {
                Row::identity(out)
            }
        };
        GridTransform {
            rows: Axis::ALL.map(row),
        }
    }

    /// The mirror that negates the coordinate along `axis`: the reflection in the plane through the
    /// origin perpendicular to that axis. It reverses orientation.
    #[must_use]
    pub fn mirror(axis: Axis) -> GridTransform {
        GridTransform {
            rows: Axis::ALL.map(|out| Row {
                source: out,
                negate: out == axis,
                offset: 0,
            }),
        }
    }

    /// The transform with `out[i] = ±in[source[i]] + translation[i]`, the sign negative where
    /// `negate[i]` is true.
    ///
    /// # Errors
    ///
    /// [`NumError::Degenerate`] if `source` is not a permutation of the three axes;
    /// [`NumError::OutOfRange`] if a translation component exceeds [`TRANSLATION_MAX`] in
    /// magnitude.
    pub fn from_parts(
        source: [Axis; 3],
        negate: [bool; 3],
        translation: [i64; 3],
    ) -> Result<GridTransform, NumError> {
        let [s0, s1, s2] = source;
        if s0 == s1 || s0 == s2 || s1 == s2 {
            return Err(NumError::Degenerate);
        }
        let [n0, n1, n2] = negate;
        let [t0, t1, t2] = translation;
        Ok(GridTransform {
            rows: [
                Row {
                    source: s0,
                    negate: n0,
                    offset: translation_component(i128::from(t0))?,
                },
                Row {
                    source: s1,
                    negate: n1,
                    offset: translation_component(i128::from(t1))?,
                },
                Row {
                    source: s2,
                    negate: n2,
                    offset: translation_component(i128::from(t2))?,
                },
            ],
        })
    }

    /// For each output axis, the input axis it reads.
    #[must_use]
    pub fn source(&self) -> [Axis; 3] {
        self.rows.map(|r| r.source)
    }

    /// For each output axis, whether the input is negated.
    #[must_use]
    pub fn negate(&self) -> [bool; 3] {
        self.rows.map(|r| r.negate)
    }

    /// The translation, in grid steps.
    #[must_use]
    pub fn translation_part(&self) -> [i64; 3] {
        self.rows.map(|r| r.offset)
    }

    /// The linear part as a matrix of 0, +1 and −1: row i has its nonzero entry in column
    /// `source[i]`.
    #[must_use]
    pub fn matrix(&self) -> [[i8; 3]; 3] {
        self.rows.map(|r| {
            let s = if r.negate { -1 } else { 1 };
            Axis::ALL.map(|col| if col == r.source { s } else { 0 })
        })
    }

    /// The determinant of the linear part, +1 or −1: the parity of the permutation times the
    /// product of the signs. The parity of the count of inversions and negations is their
    /// exclusive or, so no count is formed.
    #[must_use]
    pub fn determinant(&self) -> i8 {
        let [r0, r1, r2] = self.rows;
        let (a, b, c) = (r0.source.index(), r1.source.index(), r2.source.index());
        let inversions_odd = (a > b) ^ (a > c) ^ (b > c);
        let negations_odd = r0.negate ^ r1.negate ^ r2.negate;
        if inversions_odd == negations_odd {
            1
        } else {
            -1
        }
    }

    /// Whether the transform reverses orientation (its determinant is −1): it involves a mirror, so
    /// arcs run the other way and outward normals must be flipped.
    #[must_use]
    pub fn reverses_orientation(&self) -> bool {
        self.determinant() < 0
    }

    /// Whether this is the identity.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == GridTransform::IDENTITY
    }

    /// The image of a grid point.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a coordinate of the image leaves the frame's ±(2^40 − 1) range
    /// (see the module's bit budget). [`NumError::BudgetExceeded`] only if a checked step failed,
    /// which that budget rules out.
    pub fn apply(&self, p: P3) -> Result<P3, NumError> {
        let v = [p.x.get(), p.y.get(), p.z.get()];
        let [r0, r1, r2] = self.rows;
        let out = |r: Row| -> Result<Coord, NumError> {
            Coord::new(budget(r.linear(v).and_then(|x| x.checked_add(r.offset)))?)
        };
        Ok(P3 {
            x: out(r0)?,
            y: out(r1)?,
            z: out(r2)?,
        })
    }

    /// The image of a grid vector: the linear part only, since vectors are not translated. Exact
    /// and always in range, because permuting and negating components keeps their magnitudes.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked negation failed, which the vector range rules
    /// out (|component| ≤ 2^41 − 2); then the vector constructor's range check cannot fail either,
    /// since every output is ± an input component. For every vector the result is `Ok`.
    pub fn apply_vector(&self, v: V3) -> Result<V3, NumError> {
        let a = v.to_array();
        let [r0, r1, r2] = self.rows;
        let out = |r: Row| budget(r.linear(a));
        V3::new(out(r0)?, out(r1)?, out(r2)?)
    }

    /// The composition `self ∘ first`: the transform that applies `first`, then `self`.
    ///
    /// Row i of the result reads `first`'s row `source[i]`: its source axis, the product of the two
    /// signs, and the translation `±first.offset[source[i]] + offset[i]`.
    ///
    /// # Errors
    ///
    /// [`NumError::OutOfRange`] if a translation component of the result exceeds
    /// [`TRANSLATION_MAX`] in magnitude. [`NumError::BudgetExceeded`] only if a checked step
    /// failed, which the module's bit budget rules out.
    pub fn compose(&self, first: &GridTransform) -> Result<GridTransform, NumError> {
        let row = |r: Row| -> Result<Row, NumError> {
            let inner = r.source.pick(first.rows);
            let moved = if r.negate {
                budget(inner.offset.checked_neg())?
            } else {
                inner.offset
            };
            let offset = budget(i128::from(moved).checked_add(i128::from(r.offset)))?;
            Ok(Row {
                source: inner.source,
                negate: r.negate != inner.negate,
                offset: translation_component(offset)?,
            })
        };
        let [r0, r1, r2] = self.rows;
        Ok(GridTransform {
            rows: [row(r0)?, row(r1)?, row(r2)?],
        })
    }

    /// The inverse transform, exactly.
    ///
    /// The linear part of a signed permutation is orthogonal, so its inverse is its transpose: if
    /// row i reads axis j with sign σ, the inverse's row j reads axis i with sign σ. The
    /// translation becomes −(linear part)⁻¹ · translation, so the inverse's row j has offset
    /// −σ · `offset[i]`, which keeps every magnitude within [`TRANSLATION_MAX`].
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] only if a checked negation failed, which the translation bound
    /// rules out (|offset| ≤ 2^41 − 2): for every grid transform the result is `Ok`.
    pub fn inverse(&self) -> Result<GridTransform, NumError> {
        let mut inv = GridTransform::IDENTITY.rows;
        for (out, r) in Axis::ALL.into_iter().zip(self.rows) {
            let row = Row {
                source: out,
                negate: r.negate,
                offset: if r.negate {
                    r.offset
                } else {
                    budget(r.offset.checked_neg())?
                },
            };
            // The rows of `self` form a permutation, so each target slot is written exactly once.
            if let Some(slot) = inv.get_mut(usize::from(r.source.index())) {
                *slot = row;
            }
        }
        Ok(GridTransform { rows: inv })
    }
}

impl Default for GridTransform {
    /// The identity.
    fn default() -> Self {
        GridTransform::IDENTITY
    }
}
