//! The picture a face is split in: a plane's own, a cylinder's, cone's or sphere's seen from a
//! point inside it, or a torus's seen along its axis (11.4 step 5, G244).
//!
//! What it does: a face's loops are ordered, measured and tested in a picture, a map from the
//! face onto part of a plane that is one to one and keeps what lies inside a loop inside it. A
//! flat face's picture drops the coordinate its plane's normal is largest in
//! ([`Picture::Flat`], as [`crate::inside`] does). A face on a cylinder, or on one nappe of a cone,
//! is seen from a grid point C inside the carrier, on its axis beyond the face's end
//! ([`Central`]): the picture of a point P is (X₁, X₂) / w, with X = P − C, X₁ and X₂ its two
//! coordinates the axis is smallest in, and w its depth, the distance from C in the third. Every
//! ray from C meets the carrier once, because the carrier bounds a convex solid and C lies inside
//! it, and the face lies on one side of the plane through C across the depth, so no two points of
//! the face have one picture. **A line of the picture is a plane through C**, so the ray test of a
//! flat face ([`crate::inside::ray_parity`]) serves a curved one with nothing changed but the
//! plane. A cylinder's whole side is a ring between two closed curves, and a cone's apex is an
//! ordinary point: the picture has no seam and no pole, and no angle is computed. A sphere's face
//! is seen in the same way from a point between its boundary and a pole, the face's oriented loops
//! saying which cap is the face ([`Central::of_face`]). A torus bounds no convex solid; a face on
//! one whose axis lies along a coordinate, with its boundary on one side of the middle plane, is
//! seen along the axis instead ([`Graph`]): that half of the torus is a graph over the plane
//! square to the axis, and the picture is a flat face's, the axis's coordinate dropped.
//!
//! Guarantees:
//!
//! - **One to one, by proof.** [`Central::of_face`] gives a picture only when it has proven, with
//!   enclosures, that C lies inside the carrier's solid and that the whole face lies in front of
//!   C; [`Graph::of_face`] only when the face's boundary is proven in one half of its torus and
//!   the face's loops bound a positive area there. A face it cannot prove this for gets none, and
//!   waits.
//! - **Seen from outside.** The first coordinate is mirrored where needed, so a loop that runs
//!   counter-clockwise in the picture runs counter-clockwise on the carrier seen from outside it.
//! - **Whole numbers where they can be.** The ray's plane is a plane of whole numbers through C
//!   and the point, and a straight line's tangent in the picture is exact whenever its origin and
//!   direction are whole.
//! - **Deterministic.** C is fixed by the carrier and the face's boundary alone.
//!
//! Method: the central projection of a convex surface from a point inside it (a gnomonic chart),
//! which takes planes through the centre to lines; the architecture's "its own parameter space"
//! for a face (chapter 11.4, step 5,
//! `projects/engines/geometry/architecture/11-booleans.md`), chosen in place of (u, v) so that
//! chapter 11.17's rule holds (no angle function in a topology module) and a seam or a pole never
//! cuts a face. `SMLib` splits a face at its surface's seams and works on trim curves in (u, v)
//! (`SmTrimmingTools::SplitAtSeams`, read in solidcore's Rust port, where the split itself is not
//! ported); that is not taken.
//!
//! Consulted: solidcore, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcore/blob/ae70d1c03d8664e3392bd7c38dd2a8e51a7bf845/crates/solidcore/src/topology/trimming_tools.rs>,
//!   for how seams are treated; no code copied.
//! SPDX-License-Identifier: Apache-2.0
//!
//! Cost: [`Central::of_face`] bounds each boundary edge once; every other function is a few
//! interval or wide products. No recursion.

use geom_intersect::IntersectError;
use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::interval::Interval;
use geom_num::plane::Plane;
use geom_num::wide::I256;
use geom_num::{NumError, P3, Sign};
use geom_run::WorkMeter;
use geom_topo::{ParamCurve, ParamSurface, SurfaceKind};

use crate::carrier::CarrierIx;
use crate::crossing::Enclosed;
use crate::interfere::{Arrangement, BuildError, Entity, coordinates, nearest_point};
use crate::rule::{greater, lesser};

/// How many parameter pieces a curved piece's area is summed over, level by level, until its
/// loop's sense is proven.
const LEVELS: [u32; 5] = [16, 64, 256, 1024, 4096];

/// The picture a face is split in (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Picture {
    /// A plane's: the two coordinates kept, the third dropped.
    Flat {
        /// The two coordinates kept, in order.
        keep: [usize; 2],
    },
    /// A cylinder's or cone's, seen from a point inside it.
    Central(Central),
}

/// A carrier seen from a grid point inside it (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Central {
    /// The point the carrier is seen from.
    centre: P3,
    /// The picture's two coordinates, then the one the depth is measured in.
    axes: [usize; 3],
    /// Whether the face lies below the centre in the depth's coordinate.
    below: bool,
    /// Whether the first coordinate is mirrored, so that the picture is seen from outside.
    mirrored: bool,
}

/// A double as a whole number in a wide integer, if it is one and small enough.
pub(crate) fn whole(x: f64) -> Option<I256> {
    let rounded = geom_num::math::round_ties_even(x).ok()?.get();
    if rounded != x || x.abs() >= 9.0e15 {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number below 2^53 in size, which an integer holds exactly"
    )]
    Some(I256::from_i128(i128::from(x as i64)))
}

/// Three doubles, each enclosed as itself.
fn exact3(v: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let [x, y, z] = v;
    Ok([
        Interval::point(x)?,
        Interval::point(y)?,
        Interval::point(z)?,
    ])
}

/// a · b, enclosed.
fn dot(a: [Interval; 3], b: [Interval; 3]) -> Result<Interval, NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    a0.mul(b0)?.add(a1.mul(b1)?)?.add(a2.mul(b2)?)
}

/// a − b, enclosed.
fn minus(a: [Interval; 3], b: [Interval; 3]) -> Result<[Interval; 3], NumError> {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    Ok([a0.sub(b0)?, a1.sub(b1)?, a2.sub(b2)?])
}

/// The enclosure that holds both.
fn join(a: Interval, b: Interval) -> Result<Interval, NumError> {
    Interval::new(lesser(a.lo(), b.lo()), greater(a.hi(), b.hi()))
}

/// The box `lo..hi`, enclosed.
fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Result<[Interval; 3], NumError> {
    let ([lx, ly, lz], [hx, hy, hz]) = (lo, hi);
    Ok([
        Interval::new(lx, hx)?,
        Interval::new(ly, hy)?,
        Interval::new(lz, hz)?,
    ])
}

/// A cylinder, or one nappe of a cone: its axis, and its radius along it, measured away from the
/// axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Round {
    /// The point of the axis that heights are measured from.
    middle: [Interval; 3],
    /// The axis, a unit vector: in doubles, and enclosed.
    axis: ([f64; 3], [Interval; 3]),
    /// The radius at the height of the middle.
    radius: Interval,
    /// How much the radius grows for each step up the axis: nothing on a cylinder.
    slope: Interval,
    /// Whether the radius is one radius at every height: a cylinder.
    straight: bool,
    /// Whether the radius does not shrink up the axis.
    up: bool,
}

/// An enclosure's magnitudes.
fn magnitude(x: Interval) -> Result<Interval, NumError> {
    Interval::new(
        greater(greater(x.lo(), -x.hi()), 0.0),
        greater(-x.lo(), x.hi()),
    )
}

impl Round {
    /// The carrier `surface` as a cylinder or one nappe of a cone; `None` for any other kind, and
    /// for a cone whose domain reaches both nappes.
    pub(crate) fn of(surface: &SurfaceKind) -> Result<Option<Round>, BuildError> {
        // The carrier's radius is `radius` + `slope` · v at the height `sign` · v along its axis.
        let (middle, frame, (radius, slope), sign) = match surface {
            SurfaceKind::Cylinder(s) => (s.center(), s.frame(), (s.radius(), 0.0), s.axial_sign()),
            SurfaceKind::Cone(s) => (s.center(), s.frame(), s.radius_and_slope(), s.axial_sign()),
            _ => return Ok(None),
        };
        let (_, (v0, v1)) = surface.domain();
        // One nappe: the radius keeps one sign over the surface's own domain, an apex at an end
        // of it allowed. The sign is that of `radius` + `slope` · v summed with no rounding, as
        // an enclosure of a radius of exactly nothing would hold both signs.
        let radius_sign = |v: f64| {
            let (product, error) = two_prod(slope, v);
            let mut sum = ExactSum::new();
            sum.add_slice(&[radius, product, error]);
            sum.round_ieee()
        };
        let (r0, r1) = (radius_sign(v0), radius_sign(v1));
        let positive = r0 >= 0.0 && r1 >= 0.0;
        let negative = r0 <= 0.0 && r1 <= 0.0;
        if !(positive || negative) {
            return Ok(None);
        }
        let outward = |r: Interval| if positive { r } else { r.neg() };
        let straight = slope == 0.0;
        let (doubles, enclosed) = frame.unit_vectors().map_err(IntersectError::from)?;
        let ([_, _, axis], [_, _, n]) = (doubles, enclosed);
        Ok(Some(Round {
            middle: exact3(middle)?,
            axis: (axis, n),
            radius: outward(Interval::point(radius)?),
            slope: if straight {
                Interval::point(0.0)?
            } else {
                outward(Interval::point(slope)?.div(Interval::point(sign)?)?)
            },
            straight,
            // A cone's radius grows up its axis when its slope and its axial factor have one sign
            // on the nappe of positive radius, and two on the other.
            up: straight || (slope.signum() == sign.signum()) == positive,
        }))
    }

    /// The height along the axis of the points in the box `at`.
    fn height(&self, at: [Interval; 3]) -> Result<Interval, NumError> {
        dot(minus(at, self.middle)?, self.axis.1)
    }

    /// The radius at the height `h`.
    fn radius_at(&self, h: Interval) -> Result<Interval, NumError> {
        self.radius.add(self.slope.mul(h)?)
    }

    /// How far the grid point `p` lies from the carrier, enclosed, in closed form. In the
    /// half-plane of heights and distances from the axis the carrier is a straight ray, and the
    /// point's distance from the ray's line is its distance from the carrier wherever the foot of
    /// its perpendicular lies on the ray. Where that is not proven, near a cone's apex, the
    /// distance is at least that and at most the distance from the apex.
    pub(crate) fn gap(&self, p: P3) -> Result<Interval, NumError> {
        let from = minus(exact3(coordinates(p))?, self.middle)?;
        let h = dot(from, self.axis.1)?;
        let ([f0, f1, f2], [n0, n1, n2]) = (from, self.axis.1);
        let radial = Interval::norm(&[
            f0.sub(h.mul(n0)?)?,
            f1.sub(h.mul(n1)?)?,
            f2.sub(h.mul(n2)?)?,
        ])?;
        let steep = Interval::point(1.0)?.add(self.slope.square()?)?;
        let from_line = magnitude(radial.sub(self.radius_at(h)?)?)?.div(steep.sqrt()?)?;
        let foot = h
            .add(self.slope.mul(radial.sub(self.radius)?)?)?
            .div(steep)?;
        if self.radius_at(foot)?.certain_sign() == Some(Sign::Positive) {
            return Ok(from_line);
        }
        let apex = self.radius.neg().div(self.slope)?;
        let from_apex = Interval::norm(&[radial, h.sub(apex)?])?;
        Interval::new(from_line.lo(), greater(from_apex.hi(), from_line.hi()))
    }
}

/// A sphere: its centre and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ball {
    /// Its centre.
    centre: [f64; 3],
    /// Its radius.
    radius: f64,
}

impl Ball {
    /// How far the grid point `p` lies from the sphere, enclosed in closed form: the size of
    /// |p − c| − r.
    pub(crate) fn gap(&self, p: P3) -> Result<Interval, NumError> {
        let from = minus(exact3(coordinates(p))?, exact3(self.centre)?)?;
        magnitude(Interval::norm(&from)?.sub(Interval::point(self.radius)?)?)
    }
}

/// A carrier that bounds a convex solid, which a face on it is seen from inside: a cylinder, one
/// nappe of a cone, or a sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Solid {
    /// A cylinder, or one nappe of a cone.
    Round(Round),
    /// A sphere.
    Ball(Ball),
}

impl Solid {
    /// The carrier `surface` as one of these; `None` for any other kind, and for a cone whose
    /// domain reaches both nappes.
    pub(crate) fn of(surface: &SurfaceKind) -> Result<Option<Solid>, BuildError> {
        if let SurfaceKind::Sphere(s) = surface {
            return Ok(Some(Solid::Ball(Ball {
                centre: s.center(),
                radius: s.radius(),
            })));
        }
        Ok(Round::of(surface)?.map(Solid::Round))
    }

    /// The carrier of face `f` of `arr`, if it is one of these.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face or carrier that is not there.
    pub(crate) fn of_face(arr: &Arrangement, f: usize) -> Result<Option<Solid>, BuildError> {
        let face = arr
            .faces
            .get(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        let CarrierIx::Surface(ix) = face.surface else {
            return Ok(None);
        };
        let surface = arr
            .carriers
            .surface(ix)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        Solid::of(surface)
    }

    /// How far the grid point `p` lies from the carrier, enclosed in closed form.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn gap(&self, p: P3) -> Result<Interval, NumError> {
        match self {
            Solid::Round(round) => round.gap(p),
            Solid::Ball(ball) => ball.gap(p),
        }
    }
}

/// A ring torus whose axis lies along a coordinate: its centre, that coordinate, and its major
/// and minor radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tube {
    /// Its centre.
    centre: [f64; 3],
    /// The coordinate its axis lies along.
    axis: usize,
    /// The radius of its core circle.
    major: f64,
    /// The radius of its tube.
    minor: f64,
}

impl Tube {
    /// The carrier `surface` as one of these; `None` for any other kind, a torus whose tube
    /// reaches its axis, and one whose axis does not lie exactly along a coordinate.
    pub(crate) fn of(surface: &SurfaceKind) -> Option<Tube> {
        let SurfaceKind::Torus(torus) = surface else {
            return None;
        };
        let (major, minor) = torus.radii();
        if minor >= major {
            return None;
        }
        let normal = torus.frame().normal();
        let axis = normal.iter().position(|part| part.abs() == 1.0)?;
        if normal
            .iter()
            .enumerate()
            .any(|(k, part)| k != axis && *part != 0.0)
        {
            return None;
        }
        Some(Tube {
            centre: torus.center(),
            axis,
            major,
            minor,
        })
    }

    /// The carrier of face `f` of `arr`, if it is one of these.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face or carrier that is not there.
    pub(crate) fn of_face(arr: &Arrangement, f: usize) -> Result<Option<Tube>, BuildError> {
        let face = arr
            .faces
            .get(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        let CarrierIx::Surface(ix) = face.surface else {
            return Ok(None);
        };
        let surface = arr
            .carriers
            .surface(ix)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        Ok(Tube::of(surface))
    }

    /// The two coordinates of the picture along the axis, in order: the axis's is dropped.
    pub(crate) fn keep(&self) -> [usize; 2] {
        let [first, second, _] = axes_for(self.axis);
        [first, second]
    }

    /// A vector's part along the axis's coordinate, and its distance from the axis's line.
    fn split(&self, v: [Interval; 3]) -> Result<(Interval, Interval), NumError> {
        let [first, second, third] = axes_for(self.axis);
        let part = |k: usize| v.get(k).copied().ok_or(NumError::Degenerate);
        Ok((
            part(third)?,
            Interval::norm(&[part(first)?, part(second)?])?,
        ))
    }

    /// The height above the torus's middle plane and the distance from its axis of the grid
    /// point `p`, enclosed.
    fn place_of(&self, p: P3) -> Result<(Interval, Interval), NumError> {
        self.split(minus(exact3(coordinates(p))?, exact3(self.centre)?)?)
    }

    /// How far the grid point `p` lies from the torus, enclosed in closed form: with h its height
    /// above the torus's middle plane and ρ its distance from the axis, the size of
    /// √((ρ − R)² + h²) − r, its distance from the core circle less the tube's radius.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn gap(&self, p: P3) -> Result<Interval, NumError> {
        let (h, rho) = self.place_of(p)?;
        let core = Interval::norm(&[rho.sub(Interval::point(self.major)?)?, h])?;
        magnitude(core.sub(Interval::point(self.minor)?)?)
    }
}

/// A face on a torus whose axis lies along a coordinate, seen along that axis: the torus's half
/// on one side of its middle plane, square to the axis, is the graph of a height over the ring
/// between the circles of radii R − r and R + r about the axis, so dropping the axis's coordinate
/// is one to one on it. A face whose boundary lies in one half lies in that half or holds the
/// whole of the other ([`Graph::of_face`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Graph {
    /// The torus.
    tube: Tube,
    /// Whether the face lies where the axis's coordinate is larger than at the centre.
    up: bool,
    /// The least height of the face's boundary's box from the middle plane: no point of the face
    /// is nearer that plane, as the height has no lowest point inside the half.
    floor: f64,
}

impl Graph {
    /// The picture of face `f` of `arr` along its torus's axis, if its carrier is a [`Tube`] and
    /// the picture is proven one to one on the face; `None` for any other face.
    ///
    /// The face's boundary's box is proven on one side of the middle plane, and then the face is
    /// part of that half or holds the whole of the other: its loops say which, the area they bound
    /// with the face on their left being positive in the picture for the first and negative for
    /// the second ([`crate::split::face_area`]). In that half the outward normal points away from
    /// the middle plane, so the half is seen from outside from that side.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face, edge or carrier that is not there; a boundary curve's
    /// bound's or area's errors but an area left undecided, which leaves the face with none.
    pub(crate) fn of_face(
        arr: &Arrangement,
        f: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Graph>, BuildError> {
        let Some((tube, up, floor)) = Graph::side(arr, f, meter)? else {
            return Ok(None);
        };
        let picture = Picture::Flat { keep: tube.keep() };
        match crate::split::face_area(arr, f, &picture, meter) {
            Ok(Some(area)) if area.certain_sign() == Some(Sign::Positive) => {
                Ok(Some(Graph { tube, up, floor }))
            }
            Ok(_) | Err(BuildError::UndecidedArea { .. }) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// Face `f`'s torus, which side of its middle plane the face's boundary's box is proven on
    /// (true where the axis's coordinate is larger), and the box's least height from that plane;
    /// `None` for a face on any other carrier or with a box not proven on one side.
    ///
    /// # Errors
    ///
    /// As [`Graph::of_face`].
    pub(crate) fn side(
        arr: &Arrangement,
        f: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<(Tube, bool, f64)>, BuildError> {
        let Some(tube) = Tube::of_face(arr, f)? else {
            return Ok(None);
        };
        let Some(bounds) = boundary_box(arr, f, meter)? else {
            return Ok(None);
        };
        let (Some(part), Some(c)) = (bounds.get(tube.axis), tube.centre.get(tube.axis)) else {
            return Err(IntersectError::Internal.into());
        };
        let h = part.sub(Interval::point(*c)?)?;
        Ok(match h.certain_sign() {
            Some(Sign::Positive) => Some((tube, true, h.lo())),
            Some(Sign::Negative) => Some((tube, false, h.neg().lo())),
            Some(Sign::Zero) | None => None,
        })
    }

    /// The torus.
    pub(crate) fn tube(&self) -> &Tube {
        &self.tube
    }

    /// The picture's two coordinates.
    pub(crate) fn keep(&self) -> [usize; 2] {
        self.tube.keep()
    }

    /// The least height of the face from the middle plane.
    pub(crate) fn floor(&self) -> f64 {
        self.floor
    }

    /// The height of the grid point `p` from the middle plane, positive on the face's side.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn height(&self, p: P3) -> Result<Interval, NumError> {
        let (h, _) = self.tube.place_of(p)?;
        Ok(if self.up { h } else { h.neg() })
    }

    /// How far from the grid point `p` a point of the face's half may lie whose picture is within
    /// `slack` of `p`'s: `None` when no point of the torus has such a picture.
    ///
    /// Such a point's distance ρ from the axis is within `slack` of `p`'s, and its height h is
    /// √(r² − (ρ − R)²); it lies within √(slack² + (h − hₚ)²) of `p`, with hₚ `p`'s own height.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn reach(&self, p: P3, slack: f64) -> Result<Option<f64>, NumError> {
        let (h, rho) = self.tube.place_of(p)?;
        let own = if self.up { h } else { h.neg() };
        let off = rho
            .add(Interval::new(-slack, slack)?)?
            .sub(Interval::point(self.tube.major)?)?;
        let room = Interval::point(self.tube.minor)?
            .square()?
            .sub(off.square()?)?;
        if room.hi() < 0.0 {
            return Ok(None);
        }
        let heights = Interval::new(greater(room.lo(), 0.0), room.hi())?.sqrt()?;
        let rise = magnitude(heights.sub(own)?)?;
        Ok(Some(Interval::norm(&[Interval::point(slack)?, rise])?.hi()))
    }
}

/// The box of face `f`'s boundary, enclosed: the bounds of its edges' curves.
fn boundary_box(
    arr: &Arrangement,
    f: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[Interval; 3]>, BuildError> {
    let face = arr
        .faces
        .get(f)
        .ok_or(BuildError::Missing(Entity::Face(f)))?;
    let mut so_far: Option<[Interval; 3]> = None;
    for e in face.loops.iter().flatten().map(|u| u.edge) {
        let edge = arr
            .edges
            .get(e)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let curve = arr
            .carriers
            .curve(edge.curve)
            .ok_or(BuildError::Missing(Entity::Edge(e)))?;
        let held = curve
            .bound(curve.domain(), meter)
            .map_err(IntersectError::from)?;
        let here = boxed(held.lo, held.hi)?;
        so_far = Some(match so_far {
            Some([first, second, third]) => {
                let [one, two, three] = here;
                [join(first, one)?, join(second, two)?, join(third, three)?]
            }
            None => here,
        });
    }
    Ok(so_far)
}

/// The picture's coordinates for a depth measured in coordinate `drop`: the other two in order,
/// then `drop`.
fn axes_for(drop: usize) -> [usize; 3] {
    match drop {
        0 => [1, 2, 0],
        1 => [0, 2, 1],
        _ => [0, 1, 2],
    }
}

impl Central {
    /// The picture of face `f` of `arr`, if its carrier is a cylinder, one nappe of a cone or a
    /// sphere and the picture is proven one to one on the face; `None` for any other face.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face, edge or carrier that is not there; a boundary curve's
    /// bound's errors.
    pub(crate) fn of_face(
        arr: &Arrangement,
        f: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Central>, BuildError> {
        match Solid::of_face(arr, f)? {
            Some(Solid::Round(round)) => Central::of_round(arr, (f, &round), meter),
            Some(Solid::Ball(ball)) => Central::of_ball(arr, (f, &ball), meter),
            None => Ok(None),
        }
    }

    /// The picture of face `f` on the sphere `ball`, if one is proven one to one on it.
    ///
    /// A plane through a point C inside the sphere cuts it in two caps, and the picture from C
    /// is one to one on the cap in front of the plane. The face's boundary lies in front when C
    /// stands past its box towards a pole, halfway from the box to the pole; then the face is
    /// that cap's part inside its loops, or holds the whole cap behind. Its loops say which: the
    /// area they bound with the face on their left is positive in the picture for the first and
    /// negative for the second ([`crate::split::face_area`]). C is tried towards each pole of the
    /// three coordinates in turn, each coordinate's upper pole first, and the first picture
    /// proven to hold the face is taken: one for every face that keeps clear of some cap round a
    /// pole beyond its boundary's box. A face with no boundary, the whole sphere, has none.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face, edge or carrier that is not there; a boundary curve's
    /// bound's or area's errors but an area left undecided, which leaves that pole.
    fn of_ball(
        arr: &Arrangement,
        (f, ball): (usize, &Ball),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Central>, BuildError> {
        let Some(bounds) = boundary_box(arr, f, meter)? else {
            return Ok(None);
        };
        let proven = |x: Interval| x.certain_sign() == Some(Sign::Positive);
        let centre = exact3(ball.centre)?;
        let radius = Interval::point(ball.radius)?;
        for drop in 0..3 {
            let (Some(part), Some(c)) = (bounds.get(drop), ball.centre.get(drop)) else {
                return Err(IntersectError::Internal.into());
            };
            for below in [true, false] {
                // Seen from below the upper pole, the face lies below C; from above the lower,
                // above it.
                let (edge, pole) = if below {
                    (part.hi(), c + ball.radius)
                } else {
                    (part.lo(), c - ball.radius)
                };
                let mut at = ball.centre;
                if let Some(slot) = at.get_mut(drop) {
                    *slot = edge / 2.0 + pole / 2.0;
                }
                let Ok(stand) = nearest_point(at) else {
                    continue;
                };
                let made = Central {
                    centre: stand,
                    axes: axes_for(drop),
                    below,
                    mirrored: below == (drop != 1),
                };
                let from = minus(exact3(coordinates(stand))?, centre)?;
                let depth = made.depth(bounds)?;
                if !(proven(radius.square()?.sub(dot(from, from)?)?) && proven(depth)) {
                    continue;
                }
                match crate::split::face_area(arr, f, &Picture::Central(made), meter) {
                    Ok(Some(area)) if proven(area) => return Ok(Some(made)),
                    Ok(None) => return Ok(None),
                    Ok(Some(_)) | Err(BuildError::UndecidedArea { .. }) => {}
                    Err(other) => return Err(other),
                }
            }
        }
        Ok(None)
    }

    /// The picture of face `f` on the cylinder or cone `round`, if it is proven one to one on the
    /// face.
    ///
    /// The centre stands on the axis, three of the carrier's radii at the face's far end and
    /// eight steps beyond that end: the end along the axis for a cylinder, the wide end for a
    /// cone. A face's heights along the axis lie within its boundary's, and on a cone between
    /// the apex and the boundary's wide end, because the height has no highest or lowest point
    /// inside a face except at an apex.
    ///
    /// # Errors
    ///
    /// [`BuildError::Missing`] for a face, edge or carrier that is not there; a boundary curve's
    /// bound's errors.
    fn of_round(
        arr: &Arrangement,
        (f, round): (usize, &Round),
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<Central>, BuildError> {
        let face = arr
            .faces
            .get(f)
            .ok_or(BuildError::Missing(Entity::Face(f)))?;
        // The heights of the face's boundary along the axis.
        let mut heights: Option<Interval> = None;
        for e in face.loops.iter().flatten().map(|u| u.edge) {
            let edge = arr
                .edges
                .get(e)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let curve = arr
                .carriers
                .curve(edge.curve)
                .ok_or(BuildError::Missing(Entity::Edge(e)))?;
            let b = curve
                .bound(curve.domain(), meter)
                .map_err(IntersectError::from)?;
            let h = round.height(boxed(b.lo, b.hi)?)?;
            heights = Some(match heights {
                Some(so_far) => join(so_far, h)?,
                None => h,
            });
        }
        let Some(heights) = heights else {
            return Ok(None);
        };
        // The far end: where a cone is widest, and the end a cylinder's axis points to.
        let (far, near) = if round.up {
            (heights.hi(), heights.lo())
        } else {
            (heights.lo(), heights.hi())
        };
        let reach = round.radius_at(Interval::point(far)?)?.hi();
        let beyond = 3.0 * reach + 8.0;
        let stand = if round.up { far + beyond } else { far - beyond };
        let (pointing, n) = round.axis;
        let [a0, a1, a2] = pointing;
        let [m0, m1, m2] = round.middle.map(Interval::lo);
        let Ok(centre) = nearest_point([m0 + stand * a0, m1 + stand * a1, m2 + stand * a2]) else {
            return Ok(None);
        };
        // The depth is measured in the coordinate the axis is largest in, the first of equals.
        let mut drop = 0_usize;
        for (k, part) in pointing.iter().enumerate() {
            if part.abs() > pointing.get(drop).copied().unwrap_or(0.0).abs() {
                drop = k;
            }
        }
        let axes = axes_for(drop);
        let rising = !pointing
            .get(drop)
            .copied()
            .unwrap_or(0.0)
            .is_sign_negative();
        let below = rising == round.up;
        let made = Central {
            centre,
            axes,
            below,
            // Seen from inside, a picture whose coordinates run as the kept two do is the mirror
            // of the view from outside when the depth runs down a right-handed third coordinate.
            mirrored: below == (drop != 1),
        };
        // C inside the solid: nearer the axis than the carrier is at its height.
        let from_middle = minus(exact3(coordinates(centre))?, round.middle)?;
        let height = dot(from_middle, n)?;
        let off_axis = dot(from_middle, from_middle)?.sub(height.square()?)?;
        let here = round.radius_at(height)?;
        let room = here.square()?.sub(off_axis)?;
        let proven = |x: Interval| x.certain_sign() == Some(Sign::Positive);
        if !(proven(here) && proven(room)) {
            return Ok(None);
        }
        // The face in front of C: at each end of its heights the carrier's axis is deeper than
        // its radius, and so in between, as both run straight. A cone's face may reach its apex.
        let base = if round.straight {
            Interval::point(near)?
        } else {
            round.radius.neg().div(round.slope)?
        };
        let [_, _, depth_of_centre] = made.turned(from_middle)?;
        let [_, _, depth_of_axis] = made.turned(n)?;
        for h in [Interval::point(far)?, base] {
            let depth = h.mul(depth_of_axis)?.sub(depth_of_centre)?;
            if !proven(depth.sub(round.radius_at(h)?)?) {
                return Ok(None);
            }
        }
        Ok(Some(made))
    }

    /// The point the carrier is seen from.
    pub(crate) fn centre(&self) -> P3 {
        self.centre
    }

    /// How deep the points of the box `at` lie, measured from the centre into the picture.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn depth(&self, at: [Interval; 3]) -> Result<Interval, NumError> {
        let [_, _, w] = self.lifted(at)?;
        Ok(w)
    }

    /// How deep the grid point `p` lies ([`Central::depth`]).
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn depth_of(&self, p: P3) -> Result<Interval, NumError> {
        self.depth(exact3(coordinates(p))?)
    }

    /// The least depth of face `f`'s boundary in the picture, when the face lies on a sphere:
    /// no point of the face lies shallower. A depth along a line has no lowest point on a sphere
    /// but the pole behind the centre, which the face, in front, does not hold, so the face is
    /// shallowest on its boundary. `None` for a face on any other carrier.
    ///
    /// # Errors
    ///
    /// As [`Central::of_face`].
    pub(crate) fn floor(
        &self,
        arr: &Arrangement,
        f: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Option<f64>, BuildError> {
        let Some(Solid::Ball(_)) = Solid::of_face(arr, f)? else {
            return Ok(None);
        };
        let Some(bounds) = boundary_box(arr, f, meter)? else {
            return Ok(None);
        };
        Ok(Some(self.depth(bounds)?.lo()))
    }

    /// How far a point of the carrier near the grid point `p` may be from the point of the carrier
    /// the picture puts `p` at, its picture's point on the carrier, and from `p` itself: `None`
    /// when this is not proven.
    ///
    /// The picture's point is where the ray from the centre C through `p` meets the carrier. With
    /// g the distance from `p` to the carrier and a the distance from C, by the convexity of the
    /// carrier's solid it lies within g |p − C| / (a − g) of `p` (the solid holds the cone from the
    /// ball of radius a round C to the point of the carrier nearest `p`, and lies on one side of
    /// the plane that touches the carrier there). The reach is the larger of that and g, so that
    /// `p`'s nearest point of the carrier lies within it too; the ball of that radius round `p` is
    /// proven in front of C, where the picture is one to one on the carrier.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn reach(
        &self,
        solid: &Solid,
        centre_gap: Interval,
        p: P3,
    ) -> Result<Option<f64>, NumError> {
        let g = solid.gap(p)?.hi();
        let a = centre_gap.lo();
        if a <= g {
            return Ok(None);
        }
        let at = exact3(coordinates(p))?;
        let far = Interval::norm(&minus(at, exact3(coordinates(self.centre))?)?)?;
        let along = Interval::point(g)?
            .mul(far)?
            .div(Interval::point(a)?.sub(Interval::point(g)?)?)?;
        let reach = greater(g, along.hi());
        let [_, _, depth] = self.lifted(at)?;
        if depth.sub(Interval::point(reach)?)?.certain_sign() != Some(Sign::Positive) {
            return Ok(None);
        }
        Ok(Some(reach))
    }

    /// The parts of a vector along the picture's two coordinates and its depth, a vector from
    /// the centre to a point in front of it having a positive depth.
    fn turned(&self, v: [Interval; 3]) -> Result<[Interval; 3], NumError> {
        let [first, second, third] = self.axes;
        let part = |k: usize| v.get(k).copied().ok_or(NumError::Degenerate);
        let (x, y, w) = (part(first)?, part(second)?, part(third)?);
        Ok([
            if self.mirrored { x.neg() } else { x },
            y,
            if self.below { w.neg() } else { w },
        ])
    }

    /// [`Central::turned`], in whole numbers.
    fn turned_whole(&self, v: [I256; 3]) -> Option<[I256; 3]> {
        let [first, second, third] = self.axes;
        let (x, y, w) = (*v.get(first)?, *v.get(second)?, *v.get(third)?);
        Some([
            if self.mirrored { x.checked_neg()? } else { x },
            y,
            if self.below { w.checked_neg()? } else { w },
        ])
    }

    /// A point's two coordinates times its depth, and its depth.
    fn lifted(&self, at: [Interval; 3]) -> Result<[Interval; 3], NumError> {
        self.turned(minus(at, exact3(coordinates(self.centre))?)?)
    }

    /// The picture of the points in the box `at`: refused when the box is not proven in front of
    /// the centre.
    pub(crate) fn point(&self, at: [Interval; 3]) -> Result<[Interval; 2], NumError> {
        let [x, y, w] = self.lifted(at)?;
        if w.certain_sign() != Some(Sign::Positive) {
            return Err(NumError::Degenerate);
        }
        Ok([x.div(w)?, y.div(w)?])
    }

    /// The picture's tangent, up to a positive factor, of a curve at a point in the box `at`
    /// with its tangent in the box `along`.
    pub(crate) fn tangent(
        &self,
        at: [Interval; 3],
        along: [Interval; 3],
    ) -> Result<[Interval; 2], NumError> {
        let [x, y, w] = self.lifted(at)?;
        let [dx, dy, dw] = self.turned(along)?;
        Ok([dx.mul(w)?.sub(x.mul(dw)?)?, dy.mul(w)?.sub(y.mul(dw)?)?])
    }

    /// How fast the picture of a point in the box `at` moves, the point moving at a velocity in
    /// the box `along`: refused when the box is not proven in front of the centre.
    pub(crate) fn rate(
        &self,
        at: [Interval; 3],
        along: [Interval; 3],
    ) -> Result<[Interval; 2], NumError> {
        let [_, _, w] = self.lifted(at)?;
        if w.certain_sign() != Some(Sign::Positive) {
            return Err(NumError::Degenerate);
        }
        let [x, y] = self.tangent(at, along)?;
        let scale = w.square()?;
        Ok([x.div(scale)?, y.div(scale)?])
    }

    /// The picture's tangent of a straight line, exactly, when its origin and direction are whole:
    /// a line's picture is straight, so its tangent is one direction all along it.
    pub(crate) fn line_tangent(&self, origin: [f64; 3], direction: [f64; 3]) -> Option<[I256; 2]> {
        let [o0, o1, o2] = origin;
        let [d0, d1, d2] = direction;
        let c = [
            self.centre.x.get(),
            self.centre.y.get(),
            self.centre.z.get(),
        ]
        .map(I256::from_i64);
        let [c0, c1, c2] = c;
        let from_centre = [
            whole(o0)?.checked_sub(&c0)?,
            whole(o1)?.checked_sub(&c1)?,
            whole(o2)?.checked_sub(&c2)?,
        ];
        let [x, y, w] = self.turned_whole(from_centre)?;
        let [dx, dy, dw] = self.turned_whole([whole(d0)?, whole(d1)?, whole(d2)?])?;
        Some([
            dx.checked_mul(&w)?.checked_sub(&x.checked_mul(&dw)?)?,
            dy.checked_mul(&w)?.checked_sub(&y.checked_mul(&dw)?)?,
        ])
    }

    /// The plane through the centre that holds the picture's line through `point` along
    /// `direction`.
    fn ray_plane(&self, point: P3, (run, rise): (i64, i64)) -> Result<Plane, NumError> {
        let centre = [
            self.centre.x.get(),
            self.centre.y.get(),
            self.centre.z.get(),
        ]
        .map(i128::from);
        let here = [point.x.get(), point.y.get(), point.z.get()].map(i128::from);
        let [first, second, third] = self.axes;
        let part = |k: usize| -> Result<i128, NumError> {
            let (at, from) = (here.get(k), centre.get(k));
            at.zip(from)
                .and_then(|(at, from)| at.checked_sub(*from))
                .ok_or(NumError::BudgetExceeded)
        };
        let (x, y, w) = (part(first)?, part(second)?, part(third)?);
        // The direction in space: along the picture's coordinates, with no depth.
        let run = if self.mirrored {
            i128::from(run).wrapping_neg()
        } else {
            i128::from(run)
        };
        let rise = i128::from(rise);
        let times = |a: i128, b: i128| a.checked_mul(b).ok_or(NumError::BudgetExceeded);
        // (x, y, w) × (run, rise, 0), whose sign does not matter to the plane.
        let across = [
            times(w, rise)?.wrapping_neg(),
            times(w, run)?,
            times(x, rise)?
                .checked_sub(times(y, run)?)
                .ok_or(NumError::BudgetExceeded)?,
        ];
        let mut normal = [0_i128; 3];
        let mut offset = 0_i128;
        for (k, value) in [first, second, third].into_iter().zip(across) {
            if let Some(slot) = normal.get_mut(k) {
                *slot = value;
            }
            let at_centre = times(value, centre.get(k).copied().unwrap_or(0))?;
            offset = offset
                .checked_sub(at_centre)
                .ok_or(NumError::BudgetExceeded)?;
        }
        Plane::new(normal, offset)
    }

    /// A number whose sign says where the box `lo..hi` lies along the ray from `point` along
    /// `direction`: positive ahead of the point. A box not proven in front of the centre gets
    /// every number.
    fn along(
        &self,
        point: P3,
        (run, rise): (i64, i64),
        (lo, hi): ([f64; 3], [f64; 3]),
    ) -> Result<Interval, NumError> {
        let [x0, y0, w0] = self.lifted(exact3(coordinates(point))?)?;
        if w0.certain_sign() != Some(Sign::Positive) {
            return Err(NumError::Degenerate);
        }
        let [x, y, w] = self.lifted(boxed(lo, hi)?)?;
        if w.certain_sign() != Some(Sign::Positive) {
            return Interval::new(f64::MIN, f64::MAX);
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "the direction's parts are at most 3"
        )]
        let (run, rise) = (Interval::point(run as f64)?, Interval::point(rise as f64)?);
        // (x / w − x₀ / w₀, y / w − y₀ / w₀) · (run, rise), times the two depths.
        run.mul(x.mul(w0)?.sub(x0.mul(w)?)?)?
            .add(rise.mul(y.mul(w0)?.sub(y0.mul(w)?)?)?)
    }
}

impl Picture {
    /// The parameter pieces a curved piece's area is summed over, level by level. A flat face's
    /// areas are closed forms: one level.
    pub(crate) fn levels(&self) -> &'static [u32] {
        match self {
            Picture::Flat { .. } => &[1],
            Picture::Central(_) => &LEVELS,
        }
    }

    /// The plane of whole numbers that holds the picture's line through `point` along
    /// `direction`: for a flat face the plane through the point that holds the direction and the
    /// dropped axis, for a curved one the plane through the centre.
    ///
    /// # Errors
    ///
    /// [`NumError::BudgetExceeded`] for a plane past its budget.
    pub(crate) fn ray_plane(&self, point: P3, direction: (i64, i64)) -> Result<Plane, NumError> {
        match self {
            Picture::Flat {
                keep: [first, second],
            } => {
                let at = [point.x.get(), point.y.get(), point.z.get()];
                let coord = |k: usize| i128::from(at.get(k).copied().unwrap_or(0));
                let (from_first, from_second) = (coord(*first), coord(*second));
                let (run, rise) = (i128::from(direction.0), i128::from(direction.1));
                let fall = rise.checked_neg().ok_or(NumError::BudgetExceeded)?;
                // The ray's plane holds the dropped axis and the direction: its normal is the
                // direction turned a quarter, (−rise, run), in the picture's coordinates.
                let mut normal = [0_i128; 3];
                if let Some(slot) = normal.get_mut(*first) {
                    *slot = fall;
                }
                if let Some(slot) = normal.get_mut(*second) {
                    *slot = run;
                }
                let offset = fall
                    .checked_mul(from_first)
                    .and_then(|x| x.checked_add(run.checked_mul(from_second)?))
                    .and_then(i128::checked_neg)
                    .ok_or(NumError::BudgetExceeded)?;
                Plane::new(normal, offset)
            }
            Picture::Central(central) => central.ray_plane(point, direction),
        }
    }

    /// A number whose sign says where the box `lo..hi` lies along the ray from `point` along
    /// `direction`: positive ahead of the point, negative behind it. For a flat face it is the
    /// distance along the ray times the direction's length.
    ///
    /// # Errors
    ///
    /// An enclosure's; [`NumError::Degenerate`] for a point that is not in front of a curved
    /// face's centre.
    pub(crate) fn along(
        &self,
        point: P3,
        direction: (i64, i64),
        (lo, hi): ([f64; 3], [f64; 3]),
    ) -> Result<Interval, NumError> {
        match self {
            Picture::Flat {
                keep: [first, second],
            } => {
                let at = coordinates(point);
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "the direction's parts are at most 3"
                )]
                let (run, rise) = (direction.0 as f64, direction.1 as f64);
                let part = |k: usize, scale: f64| -> Result<Interval, NumError> {
                    let span = Interval::new(
                        lo.get(k).copied().unwrap_or(0.0),
                        hi.get(k).copied().unwrap_or(0.0),
                    )?;
                    let origin = at.get(k).copied().unwrap_or(0.0);
                    Interval::point(scale)?.mul(span.sub(Interval::point(origin)?)?)
                };
                part(*first, run)?.add(part(*second, rise)?)
            }
            Picture::Central(central) => central.along(point, direction, (lo, hi)),
        }
    }

    /// How near the ray a join may come before the direction fails: how far from the ray's plane
    /// (in the units of the plane's value), and how far behind the point (in the units of
    /// [`Picture::along`]). A step each way on a flat face; a step from the plane on a curved one,
    /// and nothing behind, where the units of the distance along change from point to point.
    ///
    /// # Errors
    ///
    /// An enclosure's.
    pub(crate) fn clearance(
        &self,
        direction: (i64, i64),
        ray: &Enclosed,
    ) -> Result<(f64, f64), NumError> {
        match self {
            Picture::Flat { .. } => {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "the direction's parts are at most 3"
                )]
                let (run, rise) = (direction.0 as f64, direction.1 as f64);
                // A step, in the units of the ray plane's value and of the distance along the
                // ray: both are lengths times the direction's length.
                let step = Interval::point(run)?
                    .square()?
                    .add(Interval::point(rise)?.square()?)?
                    .sqrt()?
                    .hi();
                Ok((step, step))
            }
            Picture::Central(_) => Ok((ray.norm.hi(), 0.0)),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code: a failure should stop loudly"
)]
mod tests {
    use super::*;
    use crate::interfere::{EdgeIn, EdgeUse, FaceIn};
    use geom_curve::{Circle, CurveKind, Frame};
    use geom_run::Budget;
    use geom_surface::{Cone, Cylinder, Sphere, Torus};

    /// A face on `surface` bounded by whole circles, each (centre, radius), about `frame`'s axis,
    /// each run forward: on a cylinder or a cone only the boundary's heights are read.
    fn seen(surface: SurfaceKind, frame: Frame<3>, rims: &[([f64; 3], f64)]) -> Option<Central> {
        seen_run(surface, frame, rims, true)
    }

    /// As [`seen`], each circle run `forward` or back.
    fn seen_run(
        surface: SurfaceKind,
        frame: Frame<3>,
        rims: &[([f64; 3], f64)],
        forward: bool,
    ) -> Option<Central> {
        let mut arr = Arrangement::default();
        let mut loops = Vec::new();
        for &(centre, radius) in rims {
            let circle = Circle::new(centre, radius, frame, (0.0, 2.0)).unwrap();
            let curve = arr.carriers.add_curve(CurveKind::Circle(circle)).unwrap();
            arr.edges.push(EdgeIn {
                curve,
                ends: None,
                operand: 0,
            });
            loops.push(vec![EdgeUse {
                edge: arr.edges.len() - 1,
                forward,
            }]);
        }
        let ix = arr.carriers.add_surface(surface).unwrap();
        arr.faces.push(FaceIn {
            surface: CarrierIx::Surface(ix),
            loops,
            operand: 0,
        });
        Central::of_face(&arr, 0, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap()
    }

    #[test]
    fn the_centre_stands_three_radii_and_eight_steps_beyond_the_far_end() {
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let domain = ((0.0, 2.0), (0.0, 100.0));
        // A cylinder up the third coordinate, from 0 to 100: seen from above its top, mirrored.
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let cylinder = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 50.0, frame, domain).unwrap());
        let rims = [([0.0, 0.0, 0.0], 50.0), ([0.0, 0.0, 100.0], 50.0)];
        assert_eq!(
            seen(cylinder.clone(), frame, &rims),
            Some(Central {
                centre: grid(0, 0, 258),
                axes: [0, 1, 2],
                below: true,
                mirrored: true,
            })
        );
        // A face with no boundary has no heights to stand beyond.
        assert_eq!(seen(cylinder, frame, &[]), None);
        // Down the second coordinate: the far end is where the axis points, at −100.
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]).unwrap();
        let cylinder = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 50.0, frame, domain).unwrap());
        let rims = [([0.0, 0.0, 0.0], 50.0), ([0.0, -100.0, 0.0], 50.0)];
        assert_eq!(
            seen(cylinder, frame, &rims),
            Some(Central {
                centre: grid(0, -258, 0),
                axes: [0, 2, 1],
                below: false,
                mirrored: true,
            })
        );
        // An axis equally along the first two coordinates: the depth is the first's. The tilted
        // circles' boxes reach 50 further along the axis than the circles do.
        let frame = Frame::new([0.0, 0.0, 1.0], [1.0, 1.0, 0.0]).unwrap();
        let length = 100.0 * 2.0_f64.sqrt();
        let cylinder = SurfaceKind::Cylinder(
            Cylinder::new([0.0; 3], 50.0, frame, ((0.0, 2.0), (0.0, length))).unwrap(),
        );
        let rims = [([0.0, 0.0, 0.0], 50.0), ([100.0, 100.0, 0.0], 50.0)];
        assert_eq!(
            seen(cylinder, frame, &rims),
            Some(Central {
                centre: grid(247, 247, 0),
                axes: [1, 2, 0],
                below: true,
                mirrored: true,
            })
        );
        // A cone that narrows up its axis to its apex at 50: seen from below its wide end.
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let narrowing = ((0.0, 2.0), (0.0, 50.0));
        let cone = SurfaceKind::Cone(Cone::new([0.0; 3], 50.0, -1.0, frame, narrowing).unwrap());
        assert_eq!(
            seen(cone, frame, &[([0.0, 0.0, 0.0], 50.0)]),
            Some(Central {
                centre: grid(0, 0, -158),
                axes: [0, 1, 2],
                below: false,
                mirrored: false,
            })
        );
        // A cylinder thinner than the grid about an axis off it: no grid point is proven inside.
        let thin =
            SurfaceKind::Cylinder(Cylinder::new([0.5, 0.5, 0.0], 0.25, frame, domain).unwrap());
        let rims = [([0.5, 0.5, 0.0], 0.25), ([0.5, 0.5, 100.0], 0.25)];
        assert_eq!(seen(thin, frame, &rims), None);
        // Any other kind has no picture of this sort.
        let torus = SurfaceKind::Torus(
            Torus::new([0.0; 3], 50.0, 20.0, frame, ((0.0, 2.0), (0.0, 1.0))).unwrap(),
        );
        let rims = [([0.0, 0.0, 0.0], 70.0), ([0.0, 0.0, 0.0], 30.0)];
        assert_eq!(seen(torus, frame, &rims), None);
    }

    #[test]
    fn a_sphere_s_face_is_seen_from_the_first_pole_whose_cap_it_keeps_clear_of() {
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let sphere = SurfaceKind::Sphere(
            Sphere::new([0.0; 3], 50.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap(),
        );
        let seen_as = |rims: &[([f64; 3], f64)], forward: bool| {
            seen_run(sphere.clone(), frame, rims, forward)
        };
        let at = |centre: P3, axes: [usize; 3], below: bool, mirrored: bool| {
            Some(Central {
                centre,
                axes,
                below,
                mirrored,
            })
        };
        // The upper half, on the left of its equator run counter-clockwise about the third
        // coordinate: the equator reaches every pole of the first two coordinates, and the half
        // holds the upper pole's cap, so it is seen from halfway to the lower pole.
        let equator = [([0.0; 3], 50.0)];
        assert_eq!(
            seen_as(&equator, true),
            at(grid(0, 0, -25), [0, 1, 2], false, false)
        );
        // The lower half, the equator run the other way: from halfway to the upper pole.
        assert_eq!(
            seen_as(&equator, false),
            at(grid(0, 0, 25), [0, 1, 2], true, true)
        );
        // A cap round the upper pole, above the circle at height 40: from beside it, halfway from
        // its box to the pole of the first coordinate.
        let ring = [([0.0, 0.0, 40.0], 30.0)];
        assert_eq!(
            seen_as(&ring, true),
            at(grid(40, 0, 0), [1, 2, 0], true, true)
        );
        // The rest of the sphere holds a cap round every pole but the upper one: from halfway
        // between the circle and that pole.
        assert_eq!(
            seen_as(&ring, false),
            at(grid(0, 0, 45), [0, 1, 2], true, true)
        );
        // The whole sphere has no boundary, and no picture.
        assert_eq!(seen_as(&[], true), None);
    }

    #[test]
    fn an_enclosure_s_magnitudes_and_the_join_of_two() {
        let iv = |lo, hi| Interval::new(lo, hi).unwrap();
        let ends = |x: Interval| (x.lo(), x.hi());
        assert_eq!(ends(magnitude(iv(2.0, 3.0)).unwrap()), (2.0, 3.0));
        assert_eq!(ends(magnitude(iv(-3.0, -2.0)).unwrap()), (2.0, 3.0));
        assert_eq!(ends(magnitude(iv(-3.0, 2.0)).unwrap()), (0.0, 3.0));
        assert_eq!(ends(magnitude(iv(-2.0, 3.0)).unwrap()), (0.0, 3.0));
        assert_eq!(ends(join(iv(0.0, 1.0), iv(2.0, 3.0)).unwrap()), (0.0, 3.0));
        assert_eq!(
            ends(join(iv(2.0, 3.0), iv(-1.0, 2.5)).unwrap()),
            (-1.0, 3.0)
        );
    }

    fn at(x: f64, y: f64, z: f64) -> [Interval; 3] {
        exact3([x, y, z]).unwrap()
    }

    /// Whether an enclosure holds `value` and little else.
    fn near(x: Interval, value: f64) -> bool {
        x.lo() <= value && value <= x.hi() && x.hi() - x.lo() < 1e-9
    }

    #[test]
    fn a_point_s_gap_from_a_cylinder_and_a_cone() {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let domain = ((0.0, 2.0), (0.0, 100.0));
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let cylinder = Cylinder::new([5.0, 0.0, 0.0], 50.0, frame, domain).unwrap();
        let round = Round::of(&SurfaceKind::Cylinder(cylinder))
            .unwrap()
            .unwrap();
        assert!(near(round.gap(grid(5, 50, 7)).unwrap(), 0.0));
        assert!(near(round.gap(grid(5, -80, 7)).unwrap(), 30.0));
        assert!(near(round.gap(grid(5, 0, 900)).unwrap(), 50.0));
        // A cone of slope 1 with its apex at the origin: the distance is across the slope.
        let cone = Cone::new([0.0; 3], 0.0, 1.0, frame, domain).unwrap();
        let round = Round::of(&SurfaceKind::Cone(cone)).unwrap().unwrap();
        assert!(round.up && !round.straight);
        assert!(near(round.gap(grid(30, 40, 50)).unwrap(), 0.0));
        assert!(near(round.gap(grid(0, 0, 0)).unwrap(), 0.0));
        assert!(near(
            round.gap(grid(100, 0, 0)).unwrap(),
            50.0 * 2.0_f64.sqrt()
        ));
        assert!(near(
            round.gap(grid(0, 0, 100)).unwrap(),
            50.0 * 2.0_f64.sqrt()
        ));
        // Below the apex the foot of the perpendicular is off the nappe: the apex is the nearest
        // point, and the enclosure runs from the line's distance to the apex's.
        let behind = round.gap(grid(0, 30, -40)).unwrap();
        assert!(
            behind.lo() <= 49.5 && behind.lo() > 49.4 && behind.hi() >= 50.0,
            "{behind:?}"
        );
        assert!(behind.hi() < 50.001, "{behind:?}");
        // The other nappe, given by a domain below the apex, opens down the axis.
        let under = Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), (-100.0, 0.0))).unwrap();
        let round = Round::of(&SurfaceKind::Cone(under)).unwrap().unwrap();
        assert!(!round.up);
        assert!(near(round.gap(grid(30, 40, -50)).unwrap(), 0.0));
        assert!(round.gap(grid(30, 40, 50)).unwrap().lo() > 49.0);
        // A domain across the apex is two nappes: no carrier of this kind.
        let both = Cone::new([0.0; 3], 0.0, 1.0, frame, ((0.0, 2.0), (-100.0, 100.0))).unwrap();
        assert_eq!(Round::of(&SurfaceKind::Cone(both)).unwrap(), None);
        // A cone that narrows along its axis, here down the third coordinate, to its apex.
        let narrowing = ((0.0, 2.0), (0.0, 50.0));
        let turned = Cone::new([0.0; 3], 50.0, -1.0, frame.reversed(), narrowing).unwrap();
        let round = Round::of(&SurfaceKind::Cone(turned)).unwrap().unwrap();
        assert!(!round.up);
        assert!(near(round.gap(grid(30, 0, -20)).unwrap(), 0.0));
        assert!(near(round.gap(grid(0, 0, -50)).unwrap(), 0.0));
    }

    #[test]
    fn a_point_s_gap_from_a_sphere() {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let sphere = Sphere::new([10.0, 0.0, 0.0], 50.0, frame, ((0.0, 2.0), (-0.5, 0.5))).unwrap();
        let solid = Solid::of(&SurfaceKind::Sphere(sphere)).unwrap().unwrap();
        assert!(near(solid.gap(grid(40, 40, 0)).unwrap(), 0.0));
        assert!(near(solid.gap(grid(10, 0, 0)).unwrap(), 50.0));
        assert!(near(solid.gap(grid(10, 0, 80)).unwrap(), 30.0));
        assert!(near(solid.gap(grid(10, -20, 0)).unwrap(), 30.0));
    }

    #[test]
    fn a_torus_about_a_coordinate_and_a_point_s_gap_from_it() {
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let domain = ((0.0, 2.0), (0.0, 2.0));
        let up = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let torus = |frame, major, minor| {
            SurfaceKind::Torus(Torus::new([0.0, 0.0, 10.0], major, minor, frame, domain).unwrap())
        };
        let tube = Tube::of(&torus(up, 50.0, 25.0)).unwrap();
        assert_eq!(tube.keep(), [0, 1]);
        assert!(near(tube.gap(grid(75, 0, 10)).unwrap(), 0.0));
        assert!(near(tube.gap(grid(0, 50, 35)).unwrap(), 0.0));
        assert!(near(tube.gap(grid(50, 0, 10)).unwrap(), 25.0));
        assert!(near(tube.gap(grid(0, 0, 10)).unwrap(), 25.0));
        assert!(near(tube.gap(grid(0, -90, 10)).unwrap(), 15.0));
        // Down the second coordinate: the picture keeps the first and the third.
        let along = Frame::new([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]).unwrap();
        let tube = Tube::of(&torus(along, 50.0, 25.0)).unwrap();
        assert_eq!(tube.keep(), [0, 2]);
        assert!(near(tube.gap(grid(0, 25, 60)).unwrap(), 0.0));
        // A tube that reaches the axis, a tilted axis, and any other kind: none.
        assert_eq!(Tube::of(&torus(up, 25.0, 25.0)), None);
        let tilted = Frame::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0]).unwrap();
        assert_eq!(Tube::of(&torus(tilted, 50.0, 25.0)), None);
        let cylinder = Cylinder::new([0.0; 3], 5.0, up, domain).unwrap();
        assert_eq!(Tube::of(&SurfaceKind::Cylinder(cylinder)), None);
    }

    /// A face on the torus of radii 50 and 25 about the third coordinate through the origin,
    /// bounded by circles about the axis, each (centre's height, radius, forward).
    fn on_torus(rims: &[(f64, f64, bool)]) -> (Arrangement, Option<Graph>) {
        let frame = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let mut arr = Arrangement::default();
        let mut loops = Vec::new();
        for &(h, r, forward) in rims {
            let circle = Circle::new([0.0, 0.0, h], r, frame, (0.0, 2.0)).unwrap();
            let curve = arr.carriers.add_curve(CurveKind::Circle(circle)).unwrap();
            arr.edges.push(EdgeIn {
                curve,
                ends: None,
                operand: 0,
            });
            loops.push(vec![EdgeUse {
                edge: arr.edges.len() - 1,
                forward,
            }]);
        }
        let torus = Torus::new([0.0; 3], 50.0, 25.0, frame, ((0.0, 2.0), (0.0, 2.0))).unwrap();
        let ix = arr.carriers.add_surface(SurfaceKind::Torus(torus)).unwrap();
        arr.faces.push(FaceIn {
            surface: CarrierIx::Surface(ix),
            loops,
            operand: 0,
        });
        let graph = Graph::of_face(&arr, 0, &mut WorkMeter::new(Budget::UNLIMITED)).unwrap();
        (arr, graph)
    }

    #[test]
    fn a_torus_s_face_in_one_half_is_seen_along_its_axis() {
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        // The band above height 15, between the circles of radii 70 and 30 there.
        let (_, graph) = on_torus(&[(15.0, 70.0, true), (15.0, 30.0, false)]);
        let graph = graph.unwrap();
        assert!(graph.up);
        assert_eq!(graph.keep(), [0, 1]);
        assert!(
            graph.floor() <= 15.0 && graph.floor() > 14.999,
            "{}",
            graph.floor()
        );
        assert!(near(graph.height(grid(50, 0, 25)).unwrap(), 25.0));
        // Below the middle plane, seen from below: the outer circle run back.
        let (_, graph) = on_torus(&[(-15.0, 70.0, false), (-15.0, 30.0, true)]);
        let graph = graph.unwrap();
        assert!(!graph.up);
        assert!(near(graph.height(grid(50, 0, -25)).unwrap(), 25.0));
        // The rest of the torus beyond either band holds the other half, and a face reaching the
        // middle plane is in neither half: neither has a picture.
        assert_eq!(on_torus(&[(15.0, 70.0, false), (15.0, 30.0, true)]).1, None);
        assert_eq!(
            on_torus(&[(-15.0, 70.0, true), (-15.0, 30.0, false)]).1,
            None
        );
        assert_eq!(on_torus(&[(0.0, 75.0, true), (0.0, 25.0, false)]).1, None);
        assert_eq!(on_torus(&[]).1, None);
    }

    #[test]
    fn a_torus_s_reach_holds_every_point_whose_picture_is_near() {
        let grid = |x, y, z| P3::new(x, y, z).unwrap();
        let (_, graph) = on_torus(&[(15.0, 70.0, true), (15.0, 30.0, false)]);
        let graph = graph.unwrap();
        // On the top circle, a picture within 3 holds points of the half at most about 3 away in
        // the picture and next to nothing in height.
        let reach = graph.reach(grid(50, 0, 25), 3.0).unwrap().unwrap();
        assert!((3.0..3.3).contains(&reach), "{reach}");
        // At the band's outer circle the half is steep: a picture within 3 reaches far further
        // in height. The height at radius 73 is √(625 − 529) = 9.8 or so.
        let reach = graph.reach(grid(70, 0, 15), 3.0).unwrap().unwrap();
        assert!(reach > 5.9 && reach < 6.1, "{reach}");
        // Nothing of the torus has its picture within 3 of the axis.
        assert_eq!(graph.reach(grid(0, 0, 15), 3.0).unwrap(), None);
        // With no slack, a point on the torus reaches only itself.
        let reach = graph.reach(grid(50, 0, 25), 0.0).unwrap().unwrap();
        assert!(reach < 1e-6, "{reach}");
    }

    /// A centre at (0, 0, 100) looking down the third coordinate.
    fn down() -> Central {
        Central {
            centre: P3::new(0, 0, 100).unwrap(),
            axes: [0, 1, 2],
            below: true,
            mirrored: true,
        }
    }

    #[test]
    fn a_point_s_picture_is_its_offset_over_its_depth() {
        let c = down();
        let [x, y] = c.point(at(30.0, -20.0, 90.0)).unwrap();
        // Depth 10; the first coordinate mirrored.
        assert!(near(x, -3.0) && near(y, -2.0), "{x:?} {y:?}");
        // Above the centre is behind it.
        assert_eq!(c.point(at(30.0, -20.0, 110.0)), Err(NumError::Degenerate));
        assert_eq!(c.point(at(30.0, -20.0, 100.0)), Err(NumError::Degenerate));
        // Seen from below and not mirrored, the same point.
        let up = Central {
            below: false,
            mirrored: false,
            ..c
        };
        let [x, y] = up.point(at(30.0, -20.0, 110.0)).unwrap();
        assert!(near(x, 3.0) && near(y, -2.0), "{x:?} {y:?}");
    }

    #[test]
    fn a_line_s_tangent_is_one_direction_all_along_it() {
        let c = down();
        // The line from (10, 0, 0) along (0, 5, -5): depth 100 at its origin.
        let exact = c.line_tangent([10.0, 0.0, 0.0], [0.0, 5.0, -5.0]).unwrap();
        // (dx·w − x·dw, dy·w − y·dw) with (x, y, w) = (−10, 0, 100), (dx, dy, dw) = (0, 5, 5).
        assert_eq!(exact, [I256::from_i64(50), I256::from_i64(500)]);
        for t in [0.0, 1.0, 7.0] {
            let [x, y] = c
                .tangent(at(10.0, 5.0 * t, -5.0 * t), at(0.0, 5.0, -5.0))
                .unwrap();
            assert!(near(x, 50.0) && near(y, 500.0), "at {t}: {x:?} {y:?}");
        }
        assert_eq!(c.line_tangent([10.5, 0.0, 0.0], [0.0, 5.0, -5.0]), None);
        assert_eq!(c.line_tangent([10.0, 0.0, 0.0], [0.0, 0.5, -5.0]), None);
    }

    #[test]
    fn the_ray_s_plane_holds_the_centre_the_point_and_the_direction() {
        let c = down();
        let point = P3::new(30, -20, 90).unwrap();
        let plane = c.ray_plane(point, (2, 1)).unwrap();
        assert_eq!(plane.side(c.centre).unwrap(), Sign::Zero);
        assert_eq!(plane.side(point).unwrap(), Sign::Zero);
        // The picture's point (−3, −2) moved by (2, 1) is (−1, −1): at depth 10, (10, −10, 90).
        assert_eq!(
            plane.side(P3::new(10, -10, 90).unwrap()).unwrap(),
            Sign::Zero
        );
        assert_ne!(
            plane.side(P3::new(10, -11, 90).unwrap()).unwrap(),
            Sign::Zero
        );
        // Ahead along the ray is positive, behind negative, whatever the depth.
        let picture = Picture::Central(c);
        let ahead = |x: f64, y: f64, z: f64| {
            picture
                .along(point, (2, 1), ([x, y, z], [x, y, z]))
                .unwrap()
        };
        assert!(ahead(10.0, -10.0, 90.0).lo() > 0.0);
        assert!(ahead(20.0, -20.0, 80.0).lo() > 0.0);
        assert!(ahead(50.0, -30.0, 90.0).hi() < 0.0);
        assert!(near(ahead(30.0, -20.0, 90.0), 0.0));
        // The rate of a point's picture is its tangent over its depth squared.
        let [dx, dy] = c.rate(at(10.0, 0.0, 0.0), at(0.0, 5.0, -5.0)).unwrap();
        assert!(near(dx, 0.005) && near(dy, 0.05), "{dx:?} {dy:?}");
        assert_eq!(
            c.rate(at(10.0, 0.0, 100.0), at(0.0, 5.0, -5.0)),
            Err(NumError::Degenerate)
        );
        // A box that reaches the centre's height is not placed.
        let unknown = picture
            .along(point, (2, 1), ([10.0, -10.0, 90.0], [10.0, -10.0, 100.0]))
            .unwrap();
        assert!(unknown.lo() < 0.0 && unknown.hi() > 0.0);
        // A point behind the centre has no ray.
        assert_eq!(
            picture.along(P3::new(0, 0, 100).unwrap(), (1, 0), ([0.0; 3], [0.0; 3])),
            Err(NumError::Degenerate)
        );
    }

    #[test]
    fn a_flat_picture_s_ray_and_clearance() {
        let picture = Picture::Flat { keep: [0, 2] };
        let point = P3::new(5, 9, 7).unwrap();
        let plane = picture.ray_plane(point, (3, 1)).unwrap();
        assert_eq!(plane.side(point).unwrap(), Sign::Zero);
        assert_eq!(plane.side(P3::new(8, -4, 8).unwrap()).unwrap(), Sign::Zero);
        assert_ne!(plane.side(P3::new(8, -4, 9).unwrap()).unwrap(), Sign::Zero);
        let ahead = picture
            .along(point, (3, 1), ([8.0, 0.0, 8.0], [8.0, 0.0, 8.0]))
            .unwrap();
        assert!(near(ahead, 10.0), "{ahead:?}");
        let enclosed = Enclosed::of(&plane).unwrap();
        let (reach, back) = picture.clearance((3, 1), &enclosed).unwrap();
        assert!(reach >= 10.0_f64.sqrt() && reach < 3.17, "{reach}");
        assert_eq!(reach, back);
        assert_eq!(picture.levels(), &[1]);
        let central = Picture::Central(down());
        assert_eq!(central.levels().len(), 5);
        let (reach, back) = central.clearance((3, 1), &enclosed).unwrap();
        assert_eq!((reach, back), (enclosed.norm.hi(), 0.0));
    }
}
