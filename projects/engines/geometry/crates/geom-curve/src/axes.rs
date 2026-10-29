//! How an analytic kind stores its vectors: lengths on an exact orthonormal frame, or the vectors
//! themselves when the kind is the affine image of one.
//!
//! What it does: [`Axes`] holds the two vectors u and v of an ellipse, hyperbola, parabola or
//! involute, and [`HelixAxes`] the three of a helix. The `Frame` form gives lengths along an exact
//! [`Frame`]: u = a·x̂, v = b·ŷ (and w = (pitch/2)·n̂ for a helix), so the semi-axes, the focal length
//! or the radius are exact numbers of the data. The `Vectors` form stores u, v (and w) directly:
//! it is what a transform that is not a similarity makes of the `Frame` form, with the same
//! parameter, because principal axes found again would shift the parameter by an angle no double
//! holds (chapter 7: "a non-uniform scale turns a circle into an ellipse", explicitly).
//!
//! Guarantees:
//!
//! - **Checked.** Lengths are finite and positive; `Vectors` are finite and u and v independent,
//!   decided exactly.
//! - **Exact reversal.** Reversing negates v (and w): on a frame, by negating its normal, which
//!   negates ŷ and n̂ exactly.
//! - **Transforms keep the form when they can.** A similarity maps a `Frame` form to a `Frame`
//!   form (lengths times the scale, a helix's pitch negated by a mirror); any other transform gives
//!   `Vectors`.
//!
//! Method: the conjugate-diameter form of an ellipse, c + cos θ·u + sin θ·v, the image of the unit
//! circle under the affine map taking the axes to u and v, of which the principal-axis form is the
//! case u ⟂ v; and its analogues for the other kinds. Chapter 7 asks for the kind change to be
//! explicit (`projects/engines/geometry/architecture/07-geometry-types.md`).
//!
//! Cost: constant.

use crate::error::CurveError;
use crate::form::{Form, Shape};
use crate::frame::{Frame, lift, lower, not_parallel};
use crate::moved::Motion;
use crate::tight::{self, point};

/// Two vectors of an analytic kind (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axes<const D: usize> {
    /// u = a·x̂ and v = b·ŷ on an exact orthonormal frame.
    Frame {
        /// The frame.
        frame: Frame<D>,
        /// The length along x̂.
        a: f64,
        /// The length along ŷ.
        b: f64,
    },
    /// The vectors themselves.
    Vectors {
        /// u.
        u: [f64; D],
        /// v.
        v: [f64; D],
    },
}

/// The three vectors of a helix (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HelixAxes<const D: usize> {
    /// u = r·x̂, v = r·ŷ and w = (pitch/2)·n̂: a circular helix rising `pitch` per turn along n̂,
    /// right-handed when the pitch is positive.
    Frame {
        /// The frame: the axis is its normal.
        frame: Frame<D>,
        /// The radius.
        radius: f64,
        /// The rise per full turn (two half-turns), signed.
        pitch: f64,
    },
    /// The vectors themselves: u, v and the rise w per half-turn.
    Vectors {
        /// u.
        u: [f64; D],
        /// v.
        v: [f64; D],
        /// w.
        w: [f64; D],
    },
}

/// A positive finite length.
fn check_length(x: f64) -> Result<(), CurveError> {
    if !x.is_finite() {
        Err(CurveError::NotFinite)
    } else if x > 0.0 {
        Ok(())
    } else {
        Err(CurveError::Degenerate)
    }
}

/// Finite, independent vectors.
fn check_vectors<const D: usize>(u: &[f64; D], v: &[f64; D]) -> Result<(), CurveError> {
    if !u.iter().chain(v).all(|x| x.is_finite()) {
        return Err(CurveError::NotFinite);
    }
    if not_parallel(lift(u), lift(v)) {
        Ok(())
    } else {
        Err(CurveError::Degenerate)
    }
}

/// The form c + f₁·(a·x̂) + f₂·(b·ŷ) + f₃·(lift·n̂) on `frame`.
fn frame_form<const D: usize>(
    shape: Shape,
    center: [f64; D],
    frame: &Frame<D>,
    lengths: [f64; 3],
) -> Result<Form<D>, CurveError> {
    let unit = frame.unit()?;
    let zero = point(0.0)?;
    let mut doubles = [[0.0; D]; 4];
    let mut exact = [[zero; D]; 4];
    if let (Some(d), Some(e)) = (doubles.first_mut(), exact.first_mut()) {
        *d = center;
        for (x, &c) in e.iter_mut().zip(&center) {
            *x = point(c)?;
        }
    }
    for (((d, e), (ud, ue)), &len) in doubles
        .iter_mut()
        .zip(exact.iter_mut())
        .skip(1)
        .zip(unit.doubles.iter().zip(&unit.exact))
        .zip(&lengths)
    {
        *d = lower(&ud.map(|x| len * x), 0.0);
        let l = point(len)?;
        let mut v = [zero; 3];
        for (o, &x) in v.iter_mut().zip(ue) {
            *o = tight::mul(l, x)?;
        }
        *e = lower(&v, zero);
    }
    if !doubles.iter().flatten().all(|x| x.is_finite()) {
        return Err(CurveError::NotFinite);
    }
    Ok(Form {
        shape,
        doubles,
        exact,
    })
}

impl<const D: usize> Axes<D> {
    /// Checks the lengths or vectors.
    pub(crate) fn check(&self) -> Result<(), CurveError> {
        match self {
            Axes::Frame { a, b, .. } => {
                check_length(*a)?;
                check_length(*b)
            }
            Axes::Vectors { u, v } => check_vectors(u, v),
        }
    }

    /// The form of a kind with these axes and `shape` about `center`.
    pub(crate) fn form(&self, shape: Shape, center: [f64; D]) -> Result<Form<D>, CurveError> {
        match self {
            Axes::Frame { frame, a, b } => frame_form(shape, center, frame, [*a, *b, 0.0]),
            Axes::Vectors { u, v } => Form::exact_vectors(shape, [center, *u, *v, [0.0; D]]),
        }
    }

    /// The axes traversed the other way: v negated.
    #[must_use]
    pub(crate) fn reversed(&self) -> Axes<D> {
        match self {
            Axes::Frame { frame, a, b } => Axes::Frame {
                frame: frame.reversed(),
                a: *a,
                b: *b,
            },
            Axes::Vectors { u, v } => Axes::Vectors {
                u: *u,
                v: v.map(|x| -x),
            },
        }
    }

    /// The axes under `m`, given the kind's form (whose u and v the `Vectors` form maps).
    pub(crate) fn moved(&self, form: &Form<D>, m: &Motion) -> Result<Axes<D>, CurveError> {
        match self {
            Axes::Frame { frame, a, b } if m.similar() => Ok(Axes::Frame {
                frame: Frame::new(m.vector(frame.x())?, m.normal(frame.normal())?)?,
                a: m.scale * a,
                b: m.scale * b,
            }),
            _ => {
                let [_, u, v, _] = form.doubles;
                Ok(Axes::Vectors {
                    u: m.vector(u)?,
                    v: m.vector(v)?,
                })
            }
        }
    }
}

impl<const D: usize> HelixAxes<D> {
    /// Checks the lengths or vectors.
    pub(crate) fn check(&self) -> Result<(), CurveError> {
        match self {
            HelixAxes::Frame { radius, pitch, .. } => {
                check_length(*radius)?;
                if !pitch.is_finite() {
                    return Err(CurveError::NotFinite);
                }
                if *pitch == 0.0 {
                    return Err(CurveError::Degenerate);
                }
                Ok(())
            }
            HelixAxes::Vectors { u, v, w } => {
                check_vectors(u, v)?;
                if w.iter().all(|x| x.is_finite()) {
                    Ok(())
                } else {
                    Err(CurveError::NotFinite)
                }
            }
        }
    }

    /// The form of a helix with these axes about `center`.
    pub(crate) fn form(&self, center: [f64; D]) -> Result<Form<D>, CurveError> {
        match self {
            HelixAxes::Frame {
                frame,
                radius,
                pitch,
            } => frame_form(
                Shape::Helical,
                center,
                frame,
                [*radius, *radius, pitch * 0.5],
            ),
            HelixAxes::Vectors { u, v, w } => {
                Form::exact_vectors(Shape::Helical, [center, *u, *v, *w])
            }
        }
    }

    /// The axes traversed the other way: v and w negated.
    #[must_use]
    pub(crate) fn reversed(&self) -> HelixAxes<D> {
        match self {
            HelixAxes::Frame {
                frame,
                radius,
                pitch,
            } => HelixAxes::Frame {
                frame: frame.reversed(),
                radius: *radius,
                pitch: *pitch,
            },
            HelixAxes::Vectors { u, v, w } => HelixAxes::Vectors {
                u: *u,
                v: v.map(|x| -x),
                w: w.map(|x| -x),
            },
        }
    }

    /// The axes under `m`, given the helix's form.
    pub(crate) fn moved(&self, form: &Form<D>, m: &Motion) -> Result<HelixAxes<D>, CurveError> {
        match self {
            HelixAxes::Frame {
                frame,
                radius,
                pitch,
            } if m.similar() => Ok(HelixAxes::Frame {
                frame: Frame::new(m.vector(frame.x())?, m.normal(frame.normal())?)?,
                radius: m.scale * radius,
                pitch: if m.mirror() {
                    -(m.scale * pitch)
                } else {
                    m.scale * pitch
                },
            }),
            _ => {
                let [_, u, v, w] = form.doubles;
                Ok(HelixAxes::Vectors {
                    u: m.vector(u)?,
                    v: m.vector(v)?,
                    w: m.vector(w)?,
                })
            }
        }
    }
}
