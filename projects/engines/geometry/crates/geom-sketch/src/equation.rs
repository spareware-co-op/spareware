//! Equations: each constraint kind's residual, written once over [`Scalar`], its singular set, and
//! the assembly of a sketch's residuals and Jacobian.
//!
//! What it does: a kind implements [`Residual`]: its residual function, a length that is zero exactly
//! when the constraint holds, and the part of its inputs where it has no derivative. Its derivatives
//! come from running that one function on dual numbers (`geom_num::dual`), never from a second text.
//! A kind's inputs are [`Slot`]s: a sketch variable, or a constant, such as a dimension's value or
//! the characteristic length of an angle, which carries no derivative. A dimension given as a named
//! scalar is the same kind with a variable in that slot. [`Assembly::new`] lays out the rows of the
//! driving constraints and of each arc's two ends, in authoring order, and the Jacobian's stored
//! entries. [`Assembly::evaluate`] then fills the residuals and the Jacobian, in the variables the
//! solver sees: each divided by its power of two, so each column is multiplied by it. It checks the
//! inputs against each singular set first.
//!
//! Guarantees:
//!
//! - **Derivatives are generated.** A Jacobian entry is the dual number's, and tests hold it to
//!   central differences of the same residual, on every kind (chapter 15.4).
//! - **Every residual is a length, with a gradient near one.** A distance's gradient has length one
//!   at its solution; an angle's or an incidence's is scaled by a characteristic length of its lines,
//!   a power of two fixed at the start, so one tolerance serves every constraint.
//! - **Smooth forms.** A distance d uses (|PQ|² − d²) / 2d, a polynomial in the points. Square roots
//!   appear only in a point's distance from a line, and a sweep's length.
//! - **Singular sets are declared.** [`Residual::singular`] is checked before differentiating, and an
//!   input on one is [`SketchError::Degenerate`], naming the constraint. The remedy of section 15.4,
//!   a perturbation derived from keys, is the solver's (G222).
//! - **The rows are structure alone.** The Jacobian's pattern depends only on which items a sketch
//!   has. [`Assembly::rescale`] reads values, once, at the start of a solve; those lengths then stay
//!   fixed through it, so the residuals are one function of the variables.
//! - **Deterministic.** Rows run in authoring order, and a NaN anywhere is
//!   [`SketchError::Numeric`] from the operation that made it.
//!
//! Method: section 15.4 (`projects/engines/geometry/architecture/15-sketches.md`), on the forward
//! mode of `geom_num::dual`; each residual is elementary coordinate geometry, H. S. M. Coxeter,
//! *Introduction to Geometry*, 2nd ed., Wiley (1969), <https://archive.org/details/introductiontoge0000coxe>,
//! chapter 1; the angle between two vectors as a rotated cross product, which has no square root and
//! no branch.
//!
//! Consulted: ezpz (Zoo), MIT,
//!   <https://github.com/KittyCAD/ezpz/blob/915882cc731da31042ce494c89b10334ee57837a/ezpz/src/constraints.rs>,
//!   for which kinds a sketch solver needs; its residuals are hand-differentiated and none is taken.
//!
//! Cost: [`Assembly::new`] sorts the Jacobian's entries once; [`Assembly::evaluate_at`] is linear in
//! the rows, with no allocation. No recursion.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the geometry's: p a point, a and b a line's ends, c a centre, r a radius, u and v directions"
)]

extern crate alloc;

use alloc::vec::Vec;

use geom_linalg::Sparse;
use geom_num::NumError;
use geom_num::dual::Dual;
use geom_num::real::Real;
use geom_num::scalar::Scalar;

use crate::model::{
    Amount, Constraint, ConstraintKind, Entity, EntityKind, ItemId, Sketch, SketchError, Standing,
    Variable, power_of_two,
};

/// What a residual cannot be differentiated at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Singular {
    /// A line of no length has no direction.
    ZeroLengthLine,
    /// A circle of no radius, or a distance of zero, has no smooth form.
    ZeroRadius,
}

/// One input of a kind: a sketch variable, by its index, or a constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    /// A variable.
    Variable(usize),
    /// A constant, which carries no derivative.
    Constant(f64),
}

/// A kind's equations: `E` residuals on `N` inputs.
pub trait Residual<const N: usize, const E: usize> {
    /// The residuals at `x`, written once over [`Scalar`], so that the one text gives values,
    /// enclosures and derivatives.
    ///
    /// # Errors
    ///
    /// What the arithmetic refuses.
    fn residuals<S: Scalar>(&self, x: [S; N]) -> Result<[S; E], NumError>;

    /// Whether `x` is on the singular set.
    fn singular(&self, _x: &[f64; N]) -> Option<Singular> {
        None
    }
}

fn cross<S: Scalar>(u: [S; 2], v: [S; 2]) -> Result<S, NumError> {
    u[0].mul(v[1])?.sub(u[1].mul(v[0])?)
}

fn dot<S: Scalar>(u: [S; 2], v: [S; 2]) -> Result<S, NumError> {
    u[0].mul(v[0])?.add(u[1].mul(v[1])?)
}

fn minus<S: Scalar>(p: [S; 2], q: [S; 2]) -> Result<[S; 2], NumError> {
    Ok([p[0].sub(q[0])?, p[1].sub(q[1])?])
}

/// (|p|² − d²) / 2d.
fn smooth<S: Scalar>(p: [S; 2], d: S) -> Result<S, NumError> {
    dot(p, p)?.sub(d.square()?)?.div(d.add(d)?)
}

/// The two points are one: also concentric circles, by their centres.
pub struct Coincident;

impl Residual<4, 2> for Coincident {
    fn residuals<S: Scalar>(&self, x: [S; 4]) -> Result<[S; 2], NumError> {
        let [ax, ay, bx, by] = x;
        Ok([ax.sub(bx)?, ay.sub(by)?])
    }
}

/// Two values are one, `sign`·d apart: b − a − sign·d. Horizontal and vertical lines and distances,
/// radii, offsets and fixes.
pub struct Apart {
    /// +1, or −1 for an offset that shrinks.
    pub sign: f64,
}

impl Residual<3, 1> for Apart {
    fn residuals<S: Scalar>(&self, x: [S; 3]) -> Result<[S; 1], NumError> {
        let [a, b, d] = x;
        let shift = S::constant(Real::new(self.sign)?).mul(d)?;
        Ok([b.sub(a)?.sub(shift)?])
    }
}

/// The radius r is half the diameter d.
pub struct Halved;

impl Residual<2, 1> for Halved {
    fn residuals<S: Scalar>(&self, x: [S; 2]) -> Result<[S; 1], NumError> {
        let [r, d] = x;
        Ok([r.sub(d.mul(S::constant(Real::new(0.5)?))?)?])
    }
}

/// `p` is on the line ab: the cross product over a length.
pub struct OnLine;

impl Residual<7, 1> for OnLine {
    fn residuals<S: Scalar>(&self, x: [S; 7]) -> Result<[S; 1], NumError> {
        let [px, py, ax, ay, bx, by, c] = x;
        let (u, v) = (minus([bx, by], [ax, ay])?, minus([px, py], [ax, ay])?);
        Ok([cross(u, v)?.div(c)?])
    }

    fn singular(&self, x: &[f64; 7]) -> Option<Singular> {
        let [_, _, ax, ay, bx, by, _] = *x;
        (ax == bx && ay == by).then_some(Singular::ZeroLengthLine)
    }
}

/// Both of cd's ends are on the line ab.
pub struct Collinear;

impl Residual<9, 2> for Collinear {
    fn residuals<S: Scalar>(&self, x: [S; 9]) -> Result<[S; 2], NumError> {
        let [ax, ay, bx, by, cx, cy, dx, dy, l] = x;
        let u = minus([bx, by], [ax, ay])?;
        Ok([
            cross(u, minus([cx, cy], [ax, ay])?)?.div(l)?,
            cross(u, minus([dx, dy], [ax, ay])?)?.div(l)?,
        ])
    }

    fn singular(&self, x: &[f64; 9]) -> Option<Singular> {
        (x[0] == x[2] && x[1] == x[3]).then_some(Singular::ZeroLengthLine)
    }
}

/// `p` is `d` from `c`: (|PC|² − d²) / 2d. Distances, lengths, points on circles, arcs' ends.
pub struct AtDistance;

impl Residual<5, 1> for AtDistance {
    fn residuals<S: Scalar>(&self, x: [S; 5]) -> Result<[S; 1], NumError> {
        let [px, py, cx, cy, d] = x;
        Ok([smooth(minus([px, py], [cx, cy])?, d)?])
    }

    fn singular(&self, x: &[f64; 5]) -> Option<Singular> {
        (x[4] == 0.0).then_some(Singular::ZeroRadius)
    }
}

/// `p` is `r + side·d` from `c`: a point's distance from a circle, and circles tangent at a point
/// other than a shared one.
pub struct AtSum {
    /// +1 for outside, or external tangency; −1 for inside, or internal.
    pub sign: f64,
}

impl Residual<6, 1> for AtSum {
    fn residuals<S: Scalar>(&self, x: [S; 6]) -> Result<[S; 1], NumError> {
        let [px, py, cx, cy, r, d] = x;
        let sum = r.add(S::constant(Real::new(self.sign)?).mul(d)?)?;
        Ok([smooth(minus([px, py], [cx, cy])?, sum)?])
    }

    fn singular(&self, x: &[f64; 6]) -> Option<Singular> {
        (x[4] + self.sign * x[5] == 0.0).then_some(Singular::ZeroRadius)
    }
}

/// `m` is the middle of ab: midpoints, symmetry about a point, equal spacing.
pub struct Middle;

impl Residual<6, 2> for Middle {
    fn residuals<S: Scalar>(&self, x: [S; 6]) -> Result<[S; 2], NumError> {
        let [mx, my, ax, ay, bx, by] = x;
        let half = S::constant(Real::new(0.5)?);
        Ok([
            ax.add(bx)?.mul(half)?.sub(mx)?,
            ay.add(by)?.mul(half)?.sub(my)?,
        ])
    }
}

/// The directions of ab and cd make the angle `t`, in half-turns, turning from ab to cd: the cross
/// product of cd with ab turned by `t`, over a length. Parallel lines are `t` = 0, right angles
/// `t` = ½, and an arc's sweep `t` itself.
pub struct Turned;

impl Residual<10, 1> for Turned {
    fn residuals<S: Scalar>(&self, x: [S; 10]) -> Result<[S; 1], NumError> {
        let [ax, ay, bx, by, cx, cy, dx, dy, t, l] = x;
        let (u, v) = (minus([bx, by], [ax, ay])?, minus([dx, dy], [cx, cy])?);
        let (sin, cos) = (t.sinpi()?, t.cospi()?);
        Ok([cross(u, v)?.mul(cos)?.sub(dot(u, v)?.mul(sin)?)?.div(l)?])
    }

    fn singular(&self, x: &[f64; 10]) -> Option<Singular> {
        let zero = (x[0] == x[2] && x[1] == x[3]) || (x[4] == x[6] && x[5] == x[7]);
        zero.then_some(Singular::ZeroLengthLine)
    }
}

/// The line ab points at the angle `t`, in half-turns, from the frame's x axis.
pub struct Heading;

impl Residual<5, 1> for Heading {
    fn residuals<S: Scalar>(&self, x: [S; 5]) -> Result<[S; 1], NumError> {
        let [ax, ay, bx, by, t] = x;
        let (dx, dy) = (bx.sub(ax)?, by.sub(ay)?);
        Ok([dy.mul(t.cospi()?)?.sub(dx.mul(t.sinpi()?)?)?])
    }

    fn singular(&self, x: &[f64; 5]) -> Option<Singular> {
        (x[0] == x[2] && x[1] == x[3]).then_some(Singular::ZeroLengthLine)
    }
}

/// `p`'s signed distance from the line ab less the stored side times `d`. A point's distance from a
/// line, parallel lines a distance apart, and a line tangent to a circle.
pub struct FromLine {
    /// +1 for left of ab, −1 for right.
    pub sign: f64,
}

impl Residual<7, 1> for FromLine {
    fn residuals<S: Scalar>(&self, x: [S; 7]) -> Result<[S; 1], NumError> {
        let [px, py, ax, ay, bx, by, d] = x;
        let (u, v) = (minus([bx, by], [ax, ay])?, minus([px, py], [ax, ay])?);
        let size = dot(u, u)?.sqrt()?;
        let want = S::constant(Real::new(self.sign)?).mul(d)?;
        Ok([cross(u, v)?.div(size)?.sub(want)?])
    }

    fn singular(&self, x: &[f64; 7]) -> Option<Singular> {
        let [_, _, ax, ay, bx, by, _] = *x;
        (ax == bx && ay == by).then_some(Singular::ZeroLengthLine)
    }
}

/// Two lines have one length: (|ab|² − |cd|²) over a length.
pub struct SameLength;

impl Residual<9, 1> for SameLength {
    fn residuals<S: Scalar>(&self, x: [S; 9]) -> Result<[S; 1], NumError> {
        let [ax, ay, bx, by, cx, cy, dx, dy, l] = x;
        let (u, v) = (minus([bx, by], [ax, ay])?, minus([dx, dy], [cx, cy])?);
        Ok([dot(u, u)?.sub(dot(v, v)?)?.div(l.add(l)?)?])
    }
}

/// An arc's length is `length`: r·θ, θ the sweep from s to e about c. The angle is measured from the
/// sweep `t` the arc had at the start, in half-turns, so the branch of the arctangent sits half a turn
/// from where the solve starts, and a half-circle's length is as smooth as any other's.
pub struct SweptLength;

impl Residual<9, 1> for SweptLength {
    fn residuals<S: Scalar>(&self, x: [S; 9]) -> Result<[S; 1], NumError> {
        let [cx, cy, sx, sy, ex, ey, r, length, t] = x;
        let (u, v) = (minus([sx, sy], [cx, cy])?, minus([ex, ey], [cx, cy])?);
        let (across, along) = (cross(u, v)?, dot(u, v)?);
        let (sin, cos) = (t.sinpi()?, t.cospi()?);
        let turned = across.mul(cos)?.sub(along.mul(sin)?)?;
        let on = along.mul(cos)?.add(across.mul(sin)?)?;
        Ok([turned
            .atan2(on)?
            .add(S::pi().mul(t)?)?
            .mul(r)?
            .sub(length)?])
    }

    fn singular(&self, x: &[f64; 9]) -> Option<Singular> {
        let [cx, cy, sx, sy, ex, ey, ..] = *x;
        ((cx == sx && cy == sy) || (cx == ex && cy == ey)).then_some(Singular::ZeroRadius)
    }
}

/// The line ab is at right angles to the radius from c to p: the dot product of the two, over a
/// length. A line tangent to an arc at a shared point.
pub struct AcrossRadius;

impl Residual<9, 1> for AcrossRadius {
    fn residuals<S: Scalar>(&self, x: [S; 9]) -> Result<[S; 1], NumError> {
        let [px, py, cx, cy, ax, ay, bx, by, l] = x;
        let radius = minus([px, py], [cx, cy])?;
        Ok([dot(radius, minus([bx, by], [ax, ay])?)?.div(l)?])
    }
}

/// The centres c and d are in line with `p`: two arcs tangent at a shared point.
pub struct InLine;

impl Residual<7, 1> for InLine {
    fn residuals<S: Scalar>(&self, x: [S; 7]) -> Result<[S; 1], NumError> {
        let [px, py, cx, cy, dx, dy, l] = x;
        let (u, v) = (minus([px, py], [cx, cy])?, minus([px, py], [dx, dy])?);
        Ok([cross(u, v)?.div(l)?])
    }
}

/// a and b are mirror images in the line cd: their middle is on it, and ab crosses it square.
pub struct Mirror;

impl Residual<9, 2> for Mirror {
    fn residuals<S: Scalar>(&self, x: [S; 9]) -> Result<[S; 2], NumError> {
        let [ax, ay, bx, by, cx, cy, dx, dy, l] = x;
        let half = S::constant(Real::new(0.5)?);
        let middle = [ax.add(bx)?.mul(half)?, ay.add(by)?.mul(half)?];
        let u = minus([dx, dy], [cx, cy])?;
        Ok([
            cross(u, minus(middle, [cx, cy])?)?.div(l)?,
            dot(u, minus([bx, by], [ax, ay])?)?.div(l)?,
        ])
    }

    fn singular(&self, x: &[f64; 9]) -> Option<Singular> {
        (x[4] == x[6] && x[5] == x[7]).then_some(Singular::ZeroLengthLine)
    }
}

/// b − a = d − c: a linear pattern's step.
pub struct SameStep;

impl Residual<8, 2> for SameStep {
    fn residuals<S: Scalar>(&self, x: [S; 8]) -> Result<[S; 2], NumError> {
        let [ax, ay, bx, by, cx, cy, dx, dy] = x;
        Ok([bx.sub(ax)?.sub(dx.sub(cx)?)?, by.sub(ay)?.sub(dy.sub(cy)?)?])
    }
}

/// b is a turned by `t` half-turns about c: a circular pattern's step.
pub struct TurnedAbout;

impl Residual<7, 2> for TurnedAbout {
    fn residuals<S: Scalar>(&self, x: [S; 7]) -> Result<[S; 2], NumError> {
        let [cx, cy, ax, ay, bx, by, t] = x;
        let (u, v) = (minus([ax, ay], [cx, cy])?, minus([bx, by], [cx, cy])?);
        let (sin, cos) = (t.sinpi()?, t.cospi()?);
        Ok([
            u[0].mul(cos)?.sub(u[1].mul(sin)?)?.sub(v[0])?,
            u[0].mul(sin)?.add(u[1].mul(cos)?)?.sub(v[1])?,
        ])
    }
}

/// a = k·b + m: a linear relation between scalars.
pub struct Linear;

impl Residual<4, 1> for Linear {
    fn residuals<S: Scalar>(&self, x: [S; 4]) -> Result<[S; 1], NumError> {
        let [a, b, k, m] = x;
        Ok([a.sub(k.mul(b)?)?.sub(m)?])
    }
}

/// a = b·c: the first polynomial relation between scalars.
pub struct Product;

impl Residual<3, 1> for Product {
    fn residuals<S: Scalar>(&self, x: [S; 3]) -> Result<[S; 1], NumError> {
        let [a, b, c] = x;
        Ok([a.sub(b.mul(c)?)?])
    }
}

fn zeros(n: usize) -> Result<Vec<f64>, SketchError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| SketchError::OutOfMemory)?;
    v.resize(n, 0.0);
    Ok(v)
}

/// One rule in an assembly: where its inputs and its Jacobian entries are.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    id: ItemId,
    kind: ConstraintKind,
    /// Its first residual.
    first: usize,
    /// Its inputs, at `Assembly::slots[at..]`.
    at: usize,
    /// The first of its stored places.
    from: usize,
}

/// The index of `(owner, slot)` among `variables`.
fn index(variables: &[Variable], owner: ItemId, slot: u8) -> Result<usize, SketchError> {
    variables
        .binary_search_by_key(&(owner, slot), |v| (v.owner, v.slot))
        .map_err(|_| SketchError::WrongKind(owner))
}

/// A kind's inputs, as it lays them out.
struct Inputs<'a> {
    sketch: &'a Sketch,
    variables: &'a [Variable],
    out: &'a mut Vec<Slot>,
}

impl Inputs<'_> {
    fn add(&mut self, slot: Slot) -> Result<(), SketchError> {
        self.out
            .try_reserve(1)
            .map_err(|_| SketchError::OutOfMemory)?;
        self.out.push(slot);
        Ok(())
    }

    fn variable(&mut self, owner: ItemId, slot: u8) -> Result<(), SketchError> {
        let i = index(self.variables, owner, slot)?;
        self.add(Slot::Variable(i))
    }

    /// A point's x and y: constants, for a reference.
    fn point(&mut self, id: ItemId) -> Result<(), SketchError> {
        self.coordinate(id, false)?;
        self.coordinate(id, true)
    }

    fn line(&mut self, id: ItemId) -> Result<(), SketchError> {
        let [a, b] = self.sketch.line(id)?;
        self.point(a)?;
        self.point(b)
    }

    fn amount(&mut self, amount: Amount) -> Result<(), SketchError> {
        match amount {
            Amount::Value(x) => self.add(Slot::Constant(x)),
            Amount::Named(id) => self.variable(id, 0),
        }
    }

    fn constant(&mut self, x: f64) -> Result<(), SketchError> {
        self.add(Slot::Constant(x))
    }

    /// The point `id`'s y, or its x.
    fn coordinate(&mut self, id: ItemId, y: bool) -> Result<(), SketchError> {
        if let EntityKind::Reference { x, y: up } = self.sketch.entity(id)?.kind {
            return self.constant(if y { up } else { x });
        }
        self.variable(id, u8::from(y))
    }

    /// A curve's centre and radius.
    fn round(&mut self, id: ItemId) -> Result<(), SketchError> {
        let centre = self.sketch.centre(id)?;
        self.point(centre)?;
        self.variable(id, 0)
    }
}

/// `kind`'s inputs, in the order its residual reads them, and how many equations it has.
///
/// # Errors
///
/// What the sketch's lookups refuse; [`SketchError::OutOfMemory`].
#[allow(
    clippy::too_many_lines,
    reason = "one arm for each kind, laid out as its residual reads"
)]
pub(crate) fn inputs_of(
    sketch: &Sketch,
    kind: &ConstraintKind,
    variables: &[Variable],
    out: &mut Vec<Slot>,
) -> Result<usize, SketchError> {
    let mut i = Inputs {
        sketch,
        variables,
        out,
    };
    // A scale's slot starts at 1, and `rescale` sets it.
    let unit = 1.0;
    Ok(match *kind {
        ConstraintKind::Coincident { a, b } => {
            i.point(a)?;
            i.point(b)?;
            2
        }
        ConstraintKind::Concentric { a, b } => {
            let (a, b) = (sketch.centre(a)?, sketch.centre(b)?);
            i.point(a)?;
            i.point(b)?;
            2
        }
        ConstraintKind::Horizontal { line } | ConstraintKind::Vertical { line } => {
            let [a, b] = sketch.line(line)?;
            let y = matches!(kind, ConstraintKind::Horizontal { .. });
            i.coordinate(a, y)?;
            i.coordinate(b, y)?;
            i.constant(0.0)?;
            1
        }
        ConstraintKind::Across { a, b, distance } | ConstraintKind::Upward { a, b, distance } => {
            let y = matches!(kind, ConstraintKind::Upward { .. });
            i.coordinate(a, y)?;
            i.coordinate(b, y)?;
            i.amount(distance)?;
            1
        }
        ConstraintKind::Radius { circle, radius } => {
            i.constant(0.0)?;
            i.variable(circle, 0)?;
            i.amount(radius)?;
            1
        }
        ConstraintKind::Diameter { circle, diameter } => {
            i.variable(circle, 0)?;
            i.amount(diameter)?;
            1
        }
        ConstraintKind::EqualRadius { a, b } => {
            i.variable(a, 0)?;
            i.variable(b, 0)?;
            i.constant(0.0)?;
            1
        }
        ConstraintKind::Offset { from, to, by, .. } => {
            i.variable(from, 0)?;
            i.variable(to, 0)?;
            i.amount(by)?;
            1
        }
        ConstraintKind::Fix {
            entity,
            slot,
            value,
        } => {
            i.constant(value)?;
            i.variable(entity, slot)?;
            i.constant(0.0)?;
            1
        }
        ConstraintKind::OnLine { point, line } => {
            i.point(point)?;
            i.line(line)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::Collinear { a, b } => {
            i.line(a)?;
            i.line(b)?;
            i.constant(unit)?;
            2
        }
        ConstraintKind::OnCircle { point, circle } => {
            i.point(point)?;
            i.round(circle)?;
            1
        }
        ConstraintKind::Distance { a, b, distance } => {
            i.point(a)?;
            i.point(b)?;
            i.amount(distance)?;
            1
        }
        ConstraintKind::Length { line, length } => {
            i.line(line)?;
            i.amount(length)?;
            1
        }
        ConstraintKind::FromCircle {
            point,
            circle,
            distance,
            ..
        } => {
            i.point(point)?;
            i.round(circle)?;
            i.amount(distance)?;
            1
        }
        ConstraintKind::TangentCircles { a, b, .. } => {
            // Inputs p, c, r, d: a's centre is the point, b's the centre, and a's radius the distance.
            i.point(sketch.centre(a)?)?;
            i.round(b)?;
            i.variable(a, 0)?;
            1
        }
        ConstraintKind::Midpoint { point, a, b }
        | ConstraintKind::SymmetricAbout { point, a, b } => {
            i.point(point)?;
            i.point(a)?;
            i.point(b)?;
            2
        }
        ConstraintKind::Spaced { a, b, c } => {
            i.point(b)?;
            i.point(a)?;
            i.point(c)?;
            2
        }
        ConstraintKind::Turned { a, b, angle } => {
            i.line(a)?;
            i.line(b)?;
            i.amount(angle)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::Parallel { a, b } => {
            i.line(a)?;
            i.line(b)?;
            i.constant(0.0)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::Perpendicular { a, b } => {
            i.line(a)?;
            i.line(b)?;
            i.constant(0.5)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::Heading { line, angle } => {
            i.line(line)?;
            i.amount(angle)?;
            1
        }
        ConstraintKind::Sweep { arc, angle } => {
            let [c, s, e] = sketch.arc(arc)?;
            i.point(c)?;
            i.point(s)?;
            i.point(c)?;
            i.point(e)?;
            i.amount(angle)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::ArcLength { arc, length } => {
            let [c, s, e] = sketch.arc(arc)?;
            i.point(c)?;
            i.point(s)?;
            i.point(e)?;
            i.variable(arc, 0)?;
            i.amount(length)?;
            i.constant(0.0)?;
            1
        }
        ConstraintKind::FromLine {
            point,
            line,
            distance,
            ..
        } => {
            i.point(point)?;
            i.line(line)?;
            i.amount(distance)?;
            1
        }
        ConstraintKind::LinesApart { a, b, distance, .. } => {
            let [start, _] = sketch.line(b)?;
            i.point(start)?;
            i.line(a)?;
            i.amount(distance)?;
            1
        }
        ConstraintKind::TangentLine { line, circle, .. } => {
            i.point(sketch.centre(circle)?)?;
            i.line(line)?;
            i.variable(circle, 0)?;
            1
        }
        ConstraintKind::EqualLength { a, b } => {
            i.line(a)?;
            i.line(b)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::TangentAtEnd { line, arc, point } => {
            i.point(point)?;
            i.point(sketch.centre(arc)?)?;
            i.line(line)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::ArcsTangentAtEnd { a, b, point } => {
            i.point(point)?;
            i.point(sketch.centre(a)?)?;
            i.point(sketch.centre(b)?)?;
            i.constant(unit)?;
            1
        }
        ConstraintKind::Symmetric { a, b, line } => {
            i.point(a)?;
            i.point(b)?;
            i.line(line)?;
            i.constant(unit)?;
            2
        }
        ConstraintKind::LinearPattern { a, b, c, d } => {
            i.point(a)?;
            i.point(b)?;
            i.point(c)?;
            i.point(d)?;
            2
        }
        ConstraintKind::CircularPattern {
            centre,
            a,
            b,
            angle,
        } => {
            i.point(centre)?;
            i.point(a)?;
            i.point(b)?;
            i.amount(angle)?;
            2
        }
        ConstraintKind::Linear { a, b, ratio, plus } => {
            i.amount(Amount::Named(a))?;
            i.amount(Amount::Named(b))?;
            i.constant(ratio)?;
            i.constant(plus)?;
            1
        }
        ConstraintKind::Product { a, b, c } => {
            i.amount(Amount::Named(a))?;
            i.amount(Amount::Named(b))?;
            i.amount(Amount::Named(c))?;
            1
        }
    })
}

/// A sketch's residuals and Jacobian.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembly {
    variables: Vec<Variable>,
    rows: Vec<Row>,
    slots: Vec<Slot>,
    /// The stored place of each row's entries, in its equations' and inputs' order.
    places: Vec<usize>,
    residuals: Vec<f64>,
    jacobian: Sparse,
}

/// What each arc contributes: its two ends on its circle, as rules with the arc's own id.
fn arc_rules(entity: &Entity, out: &mut Vec<(ItemId, ConstraintKind)>) -> Result<(), SketchError> {
    if let EntityKind::Arc { start, end, .. } = entity.kind {
        out.try_reserve(2).map_err(|_| SketchError::OutOfMemory)?;
        for point in [start, end] {
            out.push((
                entity.id,
                ConstraintKind::OnCircle {
                    point,
                    circle: entity.id,
                },
            ));
        }
    }
    Ok(())
}

/// Every rule that binds the solver, in authoring order: each driving constraint, and each arc's two
/// ends, at the arc's id.
pub(crate) fn rules_of(sketch: &Sketch) -> Result<Vec<(ItemId, ConstraintKind)>, SketchError> {
    let mut rules = Vec::new();
    for entity in sketch.entities() {
        arc_rules(entity, &mut rules)?;
    }
    for constraint in sketch.constraints() {
        if constraint.standing == Standing::Driving {
            rules.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
            rules.push((constraint.id, constraint.kind));
        }
    }
    rules.sort_by_key(|(id, _)| *id);
    Ok(rules)
}

/// The angle from `u` to `v` anticlockwise, in half-turns from 0 up to 2; 0 if either has no length,
/// which `singular` then refuses.
fn sweep_of(u: [f64; 2], v: [f64; 2]) -> Result<f64, SketchError> {
    let across = Real::new(u[0] * v[1] - u[1] * v[0])?;
    let along = Real::new(u[0] * v[0] + u[1] * v[1])?;
    if across.get() == 0.0 && along.get() == 0.0 {
        return Ok(0.0);
    }
    let half_turns = across.atan2(along)?.div(Real::pi())?.get();
    Ok(if half_turns < 0.0 {
        half_turns + 2.0
    } else {
        half_turns
    })
}

/// The cheapest constant that keeps a gradient near one: the larger of two lengths, rounded down to a
/// power of two, and 1 for two lengths of nothing.
fn characteristic(a: f64, b: f64) -> f64 {
    let (a, b) = (a.abs(), b.abs());
    let most = if a > b { a } else { b };
    if most > 0.0 { power_of_two(most) } else { 1.0 }
}

/// The size of the vector from (ax, ay) to (bx, by), in the largest of its parts: within √2 of its
/// length, which is all a scale needs.
fn span(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let (dx, dy) = ((bx - ax).abs(), (by - ay).abs());
    if dx > dy { dx } else { dy }
}

impl Assembly {
    /// The rows of `sketch`'s rules, with their lengths read from its values.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`]; [`SketchError::Linear`]; what the sketch's lookups refuse.
    pub fn new(sketch: &Sketch) -> Result<Assembly, SketchError> {
        let variables = sketch.variables()?;
        let (mut rows, mut slots, mut triplets) = (Vec::new(), Vec::new(), Vec::new());
        let mut equations = 0_usize;
        for (id, kind) in rules_of(sketch)? {
            let at = slots.len();
            let from = triplets.len();
            let here = inputs_of(sketch, &kind, &variables, &mut slots)?;
            let list = slots.get(at..).ok_or(SketchError::Internal)?;
            for e in 0..here {
                let row = equations.checked_add(e).ok_or(SketchError::Internal)?;
                for slot in list {
                    if let Slot::Variable(column) = slot {
                        triplets
                            .try_reserve(1)
                            .map_err(|_| SketchError::OutOfMemory)?;
                        triplets.push((row, *column, 0.0));
                    }
                }
            }
            rows.try_reserve(1).map_err(|_| SketchError::OutOfMemory)?;
            rows.push(Row {
                id,
                kind,
                first: equations,
                at,
                from,
            });
            equations = equations.checked_add(here).ok_or(SketchError::Internal)?;
        }
        let jacobian = Sparse::from_triplets(equations, variables.len(), &triplets)?;
        let mut places = Vec::new();
        places
            .try_reserve_exact(triplets.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        for &(row, column, _) in &triplets {
            places.push(jacobian.place_of(row, column)?);
        }
        let mut assembly = Assembly {
            residuals: zeros(equations)?,
            variables,
            rows,
            slots,
            places,
            jacobian,
        };
        let values = assembly.start()?;
        assembly.rescale(&values)?;
        Ok(assembly)
    }

    /// The variables' values as the sketch gave them.
    ///
    /// # Errors
    ///
    /// [`SketchError::OutOfMemory`].
    pub fn start(&self) -> Result<Vec<f64>, SketchError> {
        let mut x = Vec::new();
        x.try_reserve_exact(self.variables.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        x.extend(self.variables.iter().map(|v| v.value));
        Ok(x)
    }

    /// The variables, in the Jacobian's column order.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// The residuals, in steps, after [`Assembly::evaluate`].
    #[must_use]
    pub fn residuals(&self) -> &[f64] {
        &self.residuals
    }

    /// The Jacobian, by the scaled variables, after [`Assembly::evaluate`].
    #[must_use]
    pub const fn jacobian(&self) -> &Sparse {
        &self.jacobian
    }

    /// The value of `slot` at `x`.
    fn read(slot: Slot, x: &[f64]) -> Result<f64, SketchError> {
        match slot {
            Slot::Constant(c) => Ok(c),
            Slot::Variable(i) => x.get(i).copied().ok_or(SketchError::Internal),
        }
    }

    /// Fixes each row's constants at `x`: the lengths that scale its residual, and the turn an arc's
    /// sweep has crossed. They stay fixed until the next call, so a solve has one function.
    ///
    /// # Errors
    ///
    /// [`SketchError::Internal`] for an `x` of another length.
    pub fn rescale(&mut self, x: &[f64]) -> Result<(), SketchError> {
        for place in 0..self.rows.len() {
            let row = *self.rows.get(place).ok_or(SketchError::Internal)?;
            let value = |slots: &[Slot], i: usize| -> Result<f64, SketchError> {
                let at = row.at.checked_add(i).ok_or(SketchError::Internal)?;
                Assembly::read(*slots.get(at).ok_or(SketchError::Internal)?, x)
            };
            let line = |slots: &[Slot], from: usize| -> Result<f64, SketchError> {
                let at = |k: usize| {
                    from.checked_add(k)
                        .ok_or(SketchError::Internal)
                        .and_then(|i| value(slots, i))
                };
                Ok(span(at(0)?, at(1)?, at(2)?, at(3)?))
            };
            let s = &self.slots;
            let (spot, length) = match row.kind {
                ConstraintKind::OnLine { .. } => (6, characteristic(line(s, 2)?, 0.0)),
                ConstraintKind::Collinear { .. } => (8, characteristic(line(s, 0)?, 0.0)),
                ConstraintKind::Turned { .. }
                | ConstraintKind::Parallel { .. }
                | ConstraintKind::Perpendicular { .. }
                | ConstraintKind::Sweep { .. } => (9, characteristic(line(s, 0)?, line(s, 4)?)),
                ConstraintKind::EqualLength { .. } => (8, characteristic(line(s, 0)?, line(s, 4)?)),
                ConstraintKind::TangentAtEnd { .. } => {
                    (8, characteristic(line(s, 0)?, line(s, 4)?))
                }
                ConstraintKind::ArcsTangentAtEnd { .. } => {
                    let (px, py) = (value(s, 0)?, value(s, 1)?);
                    let far = span(px, py, value(s, 4)?, value(s, 5)?);
                    (6, characteristic(line(s, 0)?, far))
                }
                ConstraintKind::Symmetric { .. } => (8, characteristic(line(s, 4)?, 0.0)),
                ConstraintKind::ArcLength { .. } => {
                    let (cx, cy) = (value(s, 0)?, value(s, 1)?);
                    let (sx, sy) = (value(s, 2)? - cx, value(s, 3)? - cy);
                    let (ex, ey) = (value(s, 4)? - cx, value(s, 5)? - cy);
                    (8, sweep_of([sx, sy], [ex, ey])?)
                }
                _ => continue,
            };
            let at = row.at.checked_add(spot).ok_or(SketchError::Internal)?;
            *self.slots.get_mut(at).ok_or(SketchError::Internal)? = Slot::Constant(length);
        }
        Ok(())
    }

    /// Runs one kind: reads its inputs, checks its singular set, and stores what it gives.
    fn run<R: Residual<N, E>, const N: usize, const E: usize>(
        &mut self,
        kind: &R,
        row: &Row,
        x: &[f64],
    ) -> Result<(), SketchError> {
        let list: &[Slot; N] = self
            .slots
            .get(row.at..)
            .and_then(|tail| tail.get(..N))
            .and_then(|part| part.try_into().ok())
            .ok_or(SketchError::Internal)?;
        let mut values = [0.0; N];
        for (value, slot) in values.iter_mut().zip(list) {
            *value = Assembly::read(*slot, x)?;
        }
        if kind.singular(&values).is_some() {
            return Err(SketchError::Degenerate(row.id));
        }
        let mut seeds = [Dual::new(Real::ZERO, [Real::ZERO; N]); N];
        for (j, ((seed, value), slot)) in seeds.iter_mut().zip(&values).zip(list).enumerate() {
            let mut partials = [Real::ZERO; N];
            if matches!(slot, Slot::Variable(_))
                && let Some(unit) = partials.get_mut(j)
            {
                *unit = Real::new(1.0)?;
            }
            *seed = Dual::new(Real::new(*value)?, partials);
        }
        let result = kind.residuals(seeds)?;
        let mut stored = self
            .places
            .get(row.from..)
            .ok_or(SketchError::Internal)?
            .iter();
        for (e, out) in result.iter().enumerate() {
            let at = row.first.checked_add(e).ok_or(SketchError::Internal)?;
            *self.residuals.get_mut(at).ok_or(SketchError::Internal)? = out.value().get();
            for (partial, slot) in out.partials().iter().zip(list) {
                if let Slot::Variable(i) = slot {
                    let scale = self.variables.get(*i).ok_or(SketchError::Internal)?.scale;
                    let spot = *stored.next().ok_or(SketchError::Internal)?;
                    // A variable can come twice in one row, as a shared corner does.
                    self.jacobian.add_at(spot, partial.get() * scale)?;
                }
            }
        }
        Ok(())
    }

    /// Fills the residuals and the Jacobian at the values of `sketch`, which must have this pattern.
    ///
    /// # Errors
    ///
    /// As [`Assembly::evaluate_at`].
    pub fn evaluate(&mut self, sketch: &Sketch) -> Result<(), SketchError> {
        let values = sketch.variables()?;
        let mut x = Vec::new();
        x.try_reserve_exact(values.len())
            .map_err(|_| SketchError::OutOfMemory)?;
        x.extend(values.iter().map(|v| v.value));
        self.evaluate_at(&x)
    }

    /// Fills the residuals and the Jacobian at `x`, one value for each variable.
    ///
    /// # Errors
    ///
    /// [`SketchError::Degenerate`] for an input on a singular set, naming the constraint;
    /// [`SketchError::Numeric`] for what the arithmetic refuses.
    pub fn evaluate_at(&mut self, x: &[f64]) -> Result<(), SketchError> {
        self.jacobian.clear_values();
        for place in 0..self.rows.len() {
            let row = *self.rows.get(place).ok_or(SketchError::Internal)?;
            self.one(&row, x)?;
        }
        Ok(())
    }

    fn one(&mut self, row: &Row, x: &[f64]) -> Result<(), SketchError> {
        match row.kind {
            ConstraintKind::Coincident { .. } | ConstraintKind::Concentric { .. } => {
                self.run(&Coincident, row, x)
            }
            ConstraintKind::Horizontal { .. }
            | ConstraintKind::Vertical { .. }
            | ConstraintKind::Across { .. }
            | ConstraintKind::Upward { .. }
            | ConstraintKind::Radius { .. }
            | ConstraintKind::EqualRadius { .. }
            | ConstraintKind::Fix { .. } => self.run(&Apart { sign: 1.0 }, row, x),
            ConstraintKind::Offset { grows, .. } => {
                let sign = if grows { 1.0 } else { -1.0 };
                self.run(&Apart { sign }, row, x)
            }
            ConstraintKind::Diameter { .. } => self.run(&Halved, row, x),
            ConstraintKind::OnLine { .. } => self.run(&OnLine, row, x),
            ConstraintKind::Collinear { .. } => self.run(&Collinear, row, x),
            ConstraintKind::OnCircle { .. }
            | ConstraintKind::Distance { .. }
            | ConstraintKind::Length { .. } => self.run(&AtDistance, row, x),
            ConstraintKind::FromCircle { outside, .. }
            | ConstraintKind::TangentCircles { outside, .. } => {
                let sign = if outside { 1.0 } else { -1.0 };
                self.run(&AtSum { sign }, row, x)
            }
            ConstraintKind::Midpoint { .. }
            | ConstraintKind::SymmetricAbout { .. }
            | ConstraintKind::Spaced { .. } => self.run(&Middle, row, x),
            ConstraintKind::Turned { .. }
            | ConstraintKind::Parallel { .. }
            | ConstraintKind::Perpendicular { .. }
            | ConstraintKind::Sweep { .. } => self.run(&Turned, row, x),
            ConstraintKind::Heading { .. } => self.run(&Heading, row, x),
            ConstraintKind::ArcLength { .. } => self.run(&SweptLength, row, x),
            ConstraintKind::FromLine { side, .. }
            | ConstraintKind::LinesApart { side, .. }
            | ConstraintKind::TangentLine { side, .. } => {
                self.run(&FromLine { sign: side.sign() }, row, x)
            }
            ConstraintKind::EqualLength { .. } => self.run(&SameLength, row, x),
            ConstraintKind::TangentAtEnd { .. } => self.run(&AcrossRadius, row, x),
            ConstraintKind::ArcsTangentAtEnd { .. } => self.run(&InLine, row, x),
            ConstraintKind::Symmetric { .. } => self.run(&Mirror, row, x),
            ConstraintKind::LinearPattern { .. } => self.run(&SameStep, row, x),
            ConstraintKind::CircularPattern { .. } => self.run(&TurnedAbout, row, x),
            ConstraintKind::Linear { .. } => self.run(&Linear, row, x),
            ConstraintKind::Product { .. } => self.run(&Product, row, x),
        }
    }
}

/// The angle from u to v, in half-turns, from −1 up to 1.
fn angle_between(u: [f64; 2], v: [f64; 2]) -> Result<f64, SketchError> {
    let across = Real::new(u[0] * v[1] - u[1] * v[0])?;
    let along = Real::new(u[0] * v[0] + u[1] * v[1])?;
    Ok(across.atan2(along)?.div(Real::pi())?.get())
}

/// What a dimension measures now, signed as its stored side: reported, never solved.
///
/// # Errors
///
/// [`SketchError::NotADimension`] for any other constraint; [`SketchError::Degenerate`] on a singular
/// set; [`SketchError::Numeric`].
pub fn measure(sketch: &Sketch, constraint: &Constraint) -> Result<f64, SketchError> {
    let id = constraint.id;
    let at = |p: ItemId| sketch.point(p);
    let along = |line: ItemId| -> Result<[f64; 2], SketchError> {
        let [a, b] = sketch.line(line)?;
        let (a, b) = (at(a)?, at(b)?);
        let u = [b[0] - a[0], b[1] - a[1]];
        if u == [0.0, 0.0] {
            return Err(SketchError::Degenerate(id));
        }
        Ok(u)
    };
    let length = |p: [f64; 2], q: [f64; 2]| -> Result<f64, SketchError> {
        let (dx, dy) = (Real::new(p[0] - q[0])?, Real::new(p[1] - q[1])?);
        Ok(dx.square()?.add(dy.square()?)?.sqrt()?.get())
    };
    let from_line = |point: ItemId, line: ItemId, sign: f64| -> Result<f64, SketchError> {
        let u = along(line)?;
        let [a, _] = sketch.line(line)?;
        let (p, a) = (at(point)?, at(a)?);
        Ok((u[0] * (p[1] - a[1]) - u[1] * (p[0] - a[0])) / length(u, [0.0, 0.0])? * sign)
    };
    let sweep = |arc: ItemId| -> Result<f64, SketchError> {
        let [c, s, e] = sketch.arc(arc)?;
        let (c, s, e) = (at(c)?, at(s)?, at(e)?);
        if s == c || e == c {
            return Err(SketchError::Degenerate(id));
        }
        sweep_of([s[0] - c[0], s[1] - c[1]], [e[0] - c[0], e[1] - c[1]])
    };
    match constraint.kind {
        ConstraintKind::Distance { a, b, .. } => length(at(a)?, at(b)?),
        ConstraintKind::Length { line, .. } => length(along(line)?, [0.0, 0.0]),
        ConstraintKind::FromLine {
            point, line, side, ..
        } => from_line(point, line, side.sign()),
        ConstraintKind::LinesApart { a, b, side, .. } => {
            let [start, _] = sketch.line(b)?;
            from_line(start, a, side.sign())
        }
        ConstraintKind::FromCircle {
            point,
            circle,
            outside,
            ..
        } => {
            let gap = length(at(point)?, at(sketch.centre(circle)?)?)? - sketch.radius(circle)?;
            Ok(if outside { gap } else { -gap })
        }
        ConstraintKind::Across { a, b, .. } => Ok(at(b)?[0] - at(a)?[0]),
        ConstraintKind::Upward { a, b, .. } => Ok(at(b)?[1] - at(a)?[1]),
        ConstraintKind::Radius { circle, .. } => sketch.radius(circle),
        ConstraintKind::Diameter { circle, .. } => Ok(sketch.radius(circle)? * 2.0),
        ConstraintKind::Turned { a, b, .. } => angle_between(along(a)?, along(b)?),
        ConstraintKind::Heading { line, .. } => angle_between([1.0, 0.0], along(line)?),
        ConstraintKind::Sweep { arc, .. } => sweep(arc),
        ConstraintKind::ArcLength { arc, .. } => {
            let half_turns = Real::new(sweep(arc)?)?;
            let radius = Real::new(sketch.radius(arc)?)?;
            Ok(half_turns.mul(Real::pi())?.mul(radius)?.get())
        }
        _ => Err(SketchError::NotADimension(id)),
    }
}
