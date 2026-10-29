//! A typed transform as the analytic kinds apply it: its entries, its classification, the images of
//! stored points, vectors and normals, and the exact image of a kind's vectors for the deviation.
//!
//! What it does: [`Motion::of`] reads a [`TypedTransform`] for a kind of dimension D: its entries
//! (`crate::affine`), its certified classification (`geom-math`), which decides whether circles
//! stay circles (a similarity) and whether it mirrors, and the scale factor a similarity applies
//! to lengths (exactly 1 when it keeps lengths). Its images of stored data are doubles; its image
//! of a kind's form encloses the exact image, so a kind compares the curve it builds with the
//! exact image of the curve it had.
//!
//! Guarantees:
//!
//! - **Planar kinds see the plane.** For a 2D kind the classification is of the transform's 2×2
//!   block, so a uniform scale of the plane keeps a circle a circle.
//! - **Typed.** A degenerate transform (singular, or too close to it to decide) is refused for the
//!   analytic kinds, whose definitions need independent vectors; a 2D kind takes only a transform
//!   acting in the plane.
//! - **Normals carried, not reflected.** A normal maps by n ↦ sign(det M)·M·n, so the frame built
//!   from the images, x̂′ along M·x and ŷ′ = n̂′ × x̂′, is exactly the image of x̂ and ŷ under a
//!   similarity M = s·R: (R·a) × (R·b) = det R · R·(a × b) for an orthogonal R.
//! - **Exact where the arithmetic is.** The enclosed images use `tight`, so an exact transform of
//!   exact data gives exact points.
//!
//! Method: affine images of points and vectors, and the cross-product identity above for
//! orthogonal matrices; the classification is `geom-math`'s (G03), and the rule that a transform
//! changes a kind explicitly is chapter 7's (`projects/engines/geometry/architecture/07-geometry-types.md`).
//!
//! Cost: the classification (a few dozen interval operations), then twelve products per image.

use geom_math::{Classification, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_num::math;

use crate::affine::Entries;
use crate::error::CurveError;
use crate::form::Form;
use crate::frame::{lift, lower};
use crate::tight::{self, point};

/// The classification of a transform acting in the plane, as the plane sees it: z is left alone,
/// so the 3D classification would call a uniform scale of the plane non-uniform. The transform is
/// classified with z scaled by √|det A| instead, A its 2×2 block, which makes it a similarity of
/// space exactly when A is one of the plane (within `geom-math`'s certified tolerance, far above
/// the rounding of the root); the mirror is A's, the same as the transform's.
fn planar_class(t: &TypedTransform) -> Result<Classification, CurveError> {
    let [[m00, m01, _, tx], [m10, m11, _, ty], _] = t.to_transform()?.rows();
    let z = math::sqrt((m00 * m11 - m01 * m10).abs())?.get();
    let scaled =
        Transform::from_rows([[m00, m01, 0.0, tx], [m10, m11, 0.0, ty], [0.0, 0.0, z, 0.0]])?;
    Ok(scaled.classify()?)
}

/// A transform read for the analytic kinds.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Motion {
    entries: Entries,
    class: Classification,
    /// The factor a similarity multiplies lengths by: 1 when it keeps lengths.
    pub(crate) scale: f64,
}

impl Motion {
    /// The transform `t` for a kind of dimension D.
    ///
    /// # Errors
    ///
    /// [`CurveError::Dimension`] unless D is 2 or 3; [`CurveError::NotInPlane`] for a 2D kind and a
    /// transform that moves z; [`CurveError::Degenerate`] for a degenerate transform.
    pub(crate) fn of<const D: usize>(t: &TypedTransform) -> Result<Motion, CurveError> {
        if D != 2 && D != 3 {
            return Err(CurveError::Dimension { dimension: D });
        }
        let entries = Entries::of(t)?;
        if D == 2 && !entries.acts_in_plane() {
            return Err(CurveError::NotInPlane);
        }
        let class = if D == 2 {
            planar_class(t)?
        } else {
            t.classify()?
        };
        if class.kind == geom_math::TransformKind::Degenerate {
            return Err(CurveError::Degenerate);
        }
        let scale = if class.keeps_lengths() {
            1.0
        } else {
            let [r0, r1, r2] = entries.rows();
            let [a, _, _, _] = *r0;
            let [b, _, _, _] = *r1;
            let [c, _, _, _] = *r2;
            math::sqrt(a * a + b * b + c * c)?.get()
        };
        if !(scale.is_finite() && scale > 0.0) {
            return Err(CurveError::NotFinite);
        }
        Ok(Motion {
            entries,
            class,
            scale,
        })
    }

    /// Whether circles stay circles.
    pub(crate) fn similar(&self) -> bool {
        self.class.keeps_circles()
    }

    /// Whether the transform mirrors.
    pub(crate) fn mirror(&self) -> bool {
        self.class.mirror
    }

    /// M·x (+ t for a point), in doubles, in a fixed order.
    fn apply(&self, x: [f64; 3], translate: bool) -> Result<[f64; 3], CurveError> {
        let mut out = [0.0; 3];
        for (o, row) in out.iter_mut().zip(self.entries.rows()) {
            let [m0, m1, m2, t] = *row;
            let [x0, x1, x2] = x;
            let mut s = m0 * x0 + m1 * x1 + m2 * x2;
            if translate {
                s += t;
            }
            *o = s;
        }
        if out.iter().all(|v| v.is_finite()) {
            Ok(out)
        } else {
            Err(CurveError::NotFinite)
        }
    }

    /// The image of a point.
    pub(crate) fn point<const D: usize>(&self, p: [f64; D]) -> Result<[f64; D], CurveError> {
        Ok(lower(&self.apply(lift(&p), true)?, 0.0))
    }

    /// The image of a vector.
    pub(crate) fn vector<const D: usize>(&self, v: [f64; D]) -> Result<[f64; D], CurveError> {
        Ok(lower(&self.apply(lift(&v), false)?, 0.0))
    }

    /// The image of a normal: sign(det M)·M·n.
    pub(crate) fn normal(&self, n: [f64; 3]) -> Result<[f64; 3], CurveError> {
        let m = self.apply(n, false)?;
        Ok(if self.mirror() { m.map(|x| -x) } else { m })
    }

    /// M·x (+ t for a point) enclosed, for an enclosed x.
    fn apply_enclosed<const D: usize>(
        &self,
        x: &[Interval; D],
        translate: bool,
    ) -> Result<[Interval; D], CurveError> {
        let zero = point(0.0)?;
        let mut v = [zero; 3];
        for (o, &xi) in v.iter_mut().zip(x) {
            *o = xi;
        }
        let mut out = [zero; 3];
        for (o, row) in out.iter_mut().zip(self.entries.exact_rows()) {
            let [m0, m1, m2, t] = *row;
            let [x0, x1, x2] = v;
            let mut s = tight::mul(m0, x0)?;
            s = tight::add(s, tight::mul(m1, x1)?)?;
            s = tight::add(s, tight::mul(m2, x2)?)?;
            if translate {
                s = tight::add(s, t)?;
            }
            *o = s;
        }
        Ok(lower(&out, zero))
    }

    /// The exact image of a form's vectors, enclosed: c as a point, u, v and w as vectors.
    pub(crate) fn form_image<const D: usize>(
        &self,
        form: &Form<D>,
    ) -> Result<[[Interval; D]; 4], CurveError> {
        let [c, u, v, w] = &form.exact;
        Ok([
            self.apply_enclosed(c, true)?,
            self.apply_enclosed(u, false)?,
            self.apply_enclosed(v, false)?,
            self.apply_enclosed(w, false)?,
        ])
    }

    /// The exact image of an enclosed point (with the translation) or vector (without).
    pub(crate) fn enclosed<const D: usize>(
        &self,
        x: &[Interval; D],
        is_point: bool,
    ) -> Result<[Interval; D], CurveError> {
        self.apply_enclosed(x, is_point)
    }
}
