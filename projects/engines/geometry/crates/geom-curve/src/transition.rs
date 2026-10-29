//! The transition kind: a plane curve whose curvature follows a law along its length, the
//! transition curves (spirals) of road and rail alignments.
//!
//! What it does: [`Transition`] stores a start point, a start direction, a length L and a
//! [`CurvatureLaw`], and is parameterised by arc length t from 0 to L; positive curvature turns
//! left. The laws are decision 32.33's closed set. From a start curvature κ₁ to an end curvature
//! κ₂, with ξ = t/L: the clothoid, Helmert, Bloss, cosine and sine transitions and the cant-coupled
//! transition, IFC 4.3's CLOTHOID, HELMERTCURVE, BLOSSCURVE, COSINECURVE, SINECURVE and the curve
//! IFC calls VIENNESEBEND. And IFC's parent spirals, read from a station along them:
//! `IfcClothoid` and the second-, third- and seventh-order polynomial spirals, `IfcCosineSpiral`
//! and `IfcSineSpiral`. It gives the point and derivatives in doubles, and certified enclosures of
//! them, of the turn of the heading and of the curvature, at any station.
//!
//! Guarantees:
//!
//! - **Exact heading.** The heading is the closed-form integral of the law's curvature: in
//!   half-turns, a polynomial of degree at most 8 in ξ plus at most one sine or cosine of an affine
//!   argument, its coefficients enclosed from the stored doubles. No quadrature touches it.
//! - **Certified positions.** The position is the start plus the integrals of the cosine and sine
//!   of the heading, each by the [`NODES`]-point Gauss–Legendre rule on panels of equal width in ξ.
//!   Each panel's remainder is bounded by Trefethen's theorem for integrands analytic in a
//!   Bernstein ellipse, the integrand's size on the ellipse bounded in interval arithmetic; the
//!   panel count is the smallest power of two (from 1, or 2 for Helmert's two halves) whose
//!   remainders sum to at most L·2⁻⁵⁶. An enclosure holds the rounding of every step and the
//!   remainders of every panel it passes.
//! - **Deterministic.** The panel count, and so every result, depends only on the stored doubles.
//! - **Located exactly.** A station is placed in its panel by an exact comparison, so an enclosure
//!   never mixes two panels, or Helmert's two halves.
//! - **Never a guess.** A law that needs more than [`MAX_PANELS`] panels is refused
//!   ([`CurveError::TooManyPanels`]), never evaluated with a bound it does not meet.
//!
//! Method: the heading as the exact integral of curvature and the position by certified
//! Gauss–Legendre quadrature (chapter 17.8). The laws are IFC 4.3's
//! (`IfcAlignmentHorizontalSegmentTypeEnum` and the `IfcSpiral` subtypes, in the record below). An
//! `IfcSineSpiral` is read by its heading, as `IfcSpiral` defines the curve (x = ∫cos Θ dl), where
//! its printed curvature disagrees with the heading's derivative. The remainder bound is
//! L. N. Trefethen, "Is Gauss quadrature better than Clenshaw–Curtis?", SIAM Review 50 (2008),
//! <https://doi.org/10.1137/060659831>, Theorem 4.5: on [−1, 1], |I − Iₙ| ≤ (64/15)·M·ρ⁻²ⁿ/(ρ² − 1)
//! for f analytic in the open ellipse Eρ with |f| ≤ M there; for f = cos πh or sin πh,
//! M ≤ cosh(π·sup |Im h|). The ellipse of a part of a panel lies inside the panel's, so the
//! panel's bound holds for every station in it. No code from the Clothoids library's `Fresnel.cc`:
//! the clothoid uses this same evaluator.
//!
//! Cost: construction O(P·n) evaluations of the heading for P panels of n nodes, with up to twelve
//! ellipses tried per panel; each evaluation n evaluations of the heading and of its sine and
//! cosine, and O(k²) more operations for the k-th derivative.
//!
//! Consulted: IFC 4.3 documentation (buildingSMART), CC-BY-ND-4.0,
//!   <https://github.com/buildingSMART/IFC4.3.x-development/tree/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas>,
//!   for the curvature and heading formulas of `IfcAlignmentHorizontalSegmentTypeEnum`'s
//!   transitions, `IfcClothoid`, the second-, third- and seventh-order polynomial spirals,
//!   `IfcCosineSpiral` and `IfcSineSpiral`, which the laws implement. No text adapted.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the formulas': h the heading, l the length, x and y the integrals"
)]

use alloc::vec;
use alloc::vec::Vec;

use geom_num::eft::two_prod;
use geom_num::exact_sum::ExactSum;
use geom_num::interval::Interval;
use geom_num::math;
use geom_num::quadrature::GaussRule;
use geom_num::quadrature_rules::GAUSS_LEGENDRE;
use geom_run::WorkMeter;

use crate::error::CurveError;
use crate::form;
use crate::tight::{self, point};

/// The Gauss–Legendre points per panel.
pub const NODES: usize = 8;

/// The most panels a transition is given: 2¹⁶.
pub const MAX_PANELS: usize = 65_536;

/// The rule of [`NODES`] points (entry n − 1 has n points).
const RULE: GaussRule = GAUSS_LEGENDRE[7];

/// The heading's polynomial coefficients: degree at most 8, the cant-coupled law's.
const TERMS: usize = 9;

/// The ellipses tried per panel: ρ = 2, 4, …, 2¹².
const ELLIPSES: usize = 12;

/// The sum of the panels' remainders is at most the length times this: 2⁻⁵⁶.
const TOLERANCE: f64 = 1.0 / 72_057_594_037_927_936.0;

/// How the curvature of a [`Transition`] changes along it (decision 32.33): one closed set, every
/// law evaluated by the one method of the module documentation.
///
/// The first six run from a start curvature κ₁ (`start`) to an end curvature κ₂ (`end`), in
/// reciprocal length units, over the transition's length L, with ξ = t/L and Δκ = κ₂ − κ₁. The
/// last three are IFC's parent spirals, read from arc length `from` along them, so the transition's
/// station t is the spiral's s = `from` + t; their terms are IFC's attributes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurvatureLaw {
    /// κ = κ₁ + ξ·Δκ: curvature changing at a constant rate (IFC's CLOTHOID).
    Clothoid {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
    },
    /// Helmert's transition (also called Schramm's): κ = κ₁ + 2ξ²·Δκ on the first half and
    /// κ₁ + (1 − 2(1 − ξ)²)·Δκ on the second (HELMERTCURVE).
    Helmert {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
    },
    /// Bloss's transition: κ = κ₁ + (3 − 2ξ)·ξ²·Δκ (BLOSSCURVE).
    Bloss {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
    },
    /// The cosine transition: κ = κ₁ + ½·(1 − cos πξ)·Δκ (COSINECURVE).
    Cosine {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
    },
    /// The sine transition (also called Klein's): κ = κ₁ + (ξ − sin(2πξ)/(2π))·Δκ (SINECURVE).
    Sine {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
    },
    /// The cant-coupled transition, the curve IFC calls VIENNESEBEND: it shapes the path of a
    /// centre of gravity at a height h above the rails, so its curvature has a term in the change
    /// Δψ of the cant angle: κ = κ₁ + Δκ·ξ⁴·(35 − 84ξ + 70ξ² − 20ξ³)
    /// − 420·(h·Δψ/L²)·ξ²·(1 − 4ξ + 5ξ² − 2ξ³). A seventh-order polynomial, which needs no cant
    /// to evaluate (chapter 17.8).
    CantCoupled {
        /// The curvature at the start.
        start: f64,
        /// The curvature at the end.
        end: f64,
        /// h, the height of the centre of gravity above the rails, in length units.
        height: f64,
        /// Δψ, the change of the cant angle over the transition, in radians.
        cant: f64,
    },
    /// IFC's `IfcClothoid` and polynomial spirals: κ(s) = Σₖ sign(Aₖ)·sᵏ/|Aₖ|ᵏ⁺¹ over the terms
    /// present, so the heading is Σₖ sign(Aₖ)·sᵏ⁺¹/((k + 1)·|Aₖ|ᵏ⁺¹). `terms[k]` is Aₖ: A₀
    /// IFC's `ConstantTerm`, A₁ its `LinearTerm`, up to A₇ its `SepticTerm`. An `IfcClothoid` is A₁
    /// alone (its `ClothoidConstant`); the second-, third- and seventh-order spirals use A₀ to A₂,
    /// A₃ and A₇.
    Polynomial {
        /// A₀ to A₇, each present or absent.
        terms: [Option<f64>; 8],
        /// The spiral's arc length at the transition's start.
        from: f64,
    },
    /// IFC's `IfcCosineSpiral`: heading s/A₀ + (Lc/(π·A₁))·sin(πs/Lc), curvature
    /// 1/A₀ + cos(πs/Lc)/A₁.
    CosineSpiral {
        /// A₀, IFC's `ConstantTerm`, if present.
        constant: Option<f64>,
        /// A₁, IFC's `CosineTerm`.
        cosine: f64,
        /// Lc, the length in IFC's formula: half the cosine's period.
        length: f64,
        /// The spiral's arc length at the transition's start.
        from: f64,
    },
    /// IFC's `IfcSineSpiral`: heading s/A₀ + ½·sign(A₁)·(s/A₁)² − (Ls/(2π·A₂))·(cos(2πs/Ls) − 1),
    /// curvature 1/A₀ + sign(A₁)·s/A₁² + sin(2πs/Ls)/A₂ (the heading's derivative; module
    /// documentation).
    SineSpiral {
        /// A₀, IFC's `ConstantTerm`, if present.
        constant: Option<f64>,
        /// A₁, IFC's `LinearTerm`, if present.
        linear: Option<f64>,
        /// A₂, IFC's `SineTerm`.
        sine: f64,
        /// Ls, the length in IFC's formula: the sine's period.
        length: f64,
        /// The spiral's arc length at the transition's start.
        from: f64,
    },
}

/// A sine or cosine term of the heading: `amplitude`·sinpi(`phase` + `rate`·ξ), or cospi.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wave {
    cosine: bool,
    amplitude: Interval,
    phase: Interval,
    rate: Interval,
    /// The three as doubles, for the fast path.
    doubles: [f64; 3],
}

/// The heading's turn from the start, in half-turns, as a function of ξ: Σ cₖ·ξᵏ plus the wave.
#[derive(Clone, Debug, PartialEq)]
struct Heading {
    poly: [Interval; TERMS],
    /// The coefficients as doubles, for the fast path.
    doubles: [f64; TERMS],
    /// One more than the degree.
    len: usize,
    wave: Option<Wave>,
}

/// One panel of the quadrature.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Panel {
    /// The integrals of cos πh and sin πh from 0 to the panel's start, in length units, with every
    /// earlier panel's remainder.
    x: Interval,
    y: Interval,
    /// Their middles, the fast path's start.
    doubles: [f64; 2],
    /// The bound on this panel's remainder, in length units.
    remainder: f64,
    /// Whether the panel uses Helmert's second half.
    second: bool,
}

/// A transition curve (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    start: [f64; 2],
    direction: [f64; 2],
    length: f64,
    law: CurvatureLaw,
    /// The direction made a unit vector, enclosed, and as doubles.
    unit: [Interval; 2],
    unit_doubles: [f64; 2],
    first: Heading,
    /// Helmert's second half.
    second: Option<Heading>,
    panels: Vec<Panel>,
    remainder: f64,
}

impl Transition {
    /// The transition from `start`, leaving in `direction` (any nonzero length), of arc length
    /// `length`, whose curvature follows `law`.
    ///
    /// # Errors
    ///
    /// [`CurveError::NotFinite`] for a value that is not finite or a computation that overflows;
    /// [`CurveError::Degenerate`] for a zero direction, a length that is not positive, a zero IFC
    /// term or an IFC formula length that is not positive; [`CurveError::TooManyPanels`] when the
    /// remainders need more than [`MAX_PANELS`] panels; [`CurveError::Stopped`] by the meter.
    pub fn new(
        start: [f64; 2],
        direction: [f64; 2],
        length: f64,
        law: CurvatureLaw,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Transition, CurveError> {
        if !start.iter().chain(&direction).all(|v| v.is_finite()) || !length.is_finite() {
            return Err(CurveError::NotFinite);
        }
        if direction == [0.0, 0.0] || length <= 0.0 {
            return Err(CurveError::Degenerate);
        }
        let d = [point(direction[0])?, point(direction[1])?];
        let norm = tight::norm(&d)?;
        let unit = [tight::div(d[0], norm)?, tight::div(d[1], norm)?];
        let (first, second) = headings(&law, length)?;
        let (panels, remainder) = build(&first, second.as_ref(), length, MAX_PANELS, meter)?;
        Ok(Transition {
            start,
            direction,
            length,
            law,
            unit,
            unit_doubles: unit.map(tight::middle),
            first,
            second,
            panels,
            remainder,
        })
    }

    /// The start point.
    #[must_use]
    pub fn start(&self) -> [f64; 2] {
        self.start
    }

    /// The start direction, as given.
    #[must_use]
    pub fn direction(&self) -> [f64; 2] {
        self.direction
    }

    /// The arc length.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length
    }

    /// The law.
    #[must_use]
    pub fn law(&self) -> CurvatureLaw {
        self.law
    }

    /// The parameter domain, [0, L].
    #[must_use]
    pub fn domain(&self) -> (f64, f64) {
        (0.0, self.length)
    }

    /// The number of quadrature panels.
    #[must_use]
    pub fn panels(&self) -> usize {
        self.panels.len()
    }

    /// The sum of the panels' remainder bounds, in length units: at most L·2⁻⁵⁶.
    #[must_use]
    pub fn remainder(&self) -> f64 {
        self.remainder
    }

    /// The point at station `t`, in doubles: the fast path, within the enclosure of
    /// [`Transition::enclose`] up to the rounding of its steps.
    ///
    /// # Errors
    ///
    /// [`CurveError::ParameterOutside`] unless 0 ≤ t ≤ L; [`CurveError::NotFinite`].
    pub fn point(&self, t: f64) -> Result<[f64; 2], CurveError> {
        form::check_parameter(self.domain(), t)?;
        let (panel, a, xi) = self.panel_of(t)?;
        let [x, y] = integral(self.heading_of(panel), a, (xi - a) * 0.5)?;
        let local = [
            panel.doubles[0] + self.length * x,
            panel.doubles[1] + self.length * y,
        ];
        finite(self.placed(local))
    }

    /// The point and its derivatives with respect to t at station `t`, in doubles: `out[k]` is
    /// the k-th derivative, `out[0]` the point.
    ///
    /// # Errors
    ///
    /// As [`Transition::point`].
    pub fn derivatives(&self, t: f64, out: &mut [[f64; 2]]) -> Result<(), CurveError> {
        let Some((first, rest)) = out.split_first_mut() else {
            return Ok(());
        };
        *first = self.point(t)?;
        let (panel, _, xi) = self.panel_of(t)?;
        let units = self.units(self.heading_of(panel), xi, rest.len())?;
        for (slot, g) in rest.iter_mut().zip(&units) {
            *slot = finite(self.turned(*g)?)?;
        }
        Ok(())
    }

    /// Certified enclosures of the point and of its derivatives with respect to t to `order`, at
    /// station `t`.
    ///
    /// # Errors
    ///
    /// As [`Transition::point`]; [`CurveError::Stopped`] by the meter.
    pub fn enclose(
        &self,
        t: f64,
        order: usize,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Vec<[Interval; 2]>, CurveError> {
        form::check_parameter(self.domain(), t)?;
        meter.charge(
            u64::try_from(order)
                .map_err(|_| CurveError::Defect)?
                .saturating_add(1),
        )?;
        let (index, xi) = self.locate(t)?;
        let panel = self.panels.get(index).ok_or(CurveError::Defect)?;
        let h = self.heading_of(panel);
        let a = ratio(index, self.panels.len())?;
        let half = xi.sub(point(a)?)?.mul(point(0.5)?)?;
        let [x, y] = integral(h, point(a)?, half)?;
        let (l, r) = (
            point(self.length)?,
            Interval::new(-panel.remainder, panel.remainder)?,
        );
        let local = [
            panel.x.add(l.mul(x)?)?.add(r)?,
            panel.y.add(l.mul(y)?)?.add(r)?,
        ];
        let [u, v] = self.unit;
        let mut out = Vec::with_capacity(order.saturating_add(1));
        out.push([
            point(self.start[0])?
                .add(u.mul(local[0])?)?
                .sub(v.mul(local[1])?)?,
            point(self.start[1])?
                .add(v.mul(local[0])?)?
                .add(u.mul(local[1])?)?,
        ]);
        for g in self.units(h, xi, order)? {
            out.push(self.turned(g)?);
        }
        Ok(out)
    }

    /// The heading's turn from the start at station `t`, in half-turns, enclosed: the closed form,
    /// with no quadrature.
    ///
    /// # Errors
    ///
    /// As [`Transition::point`].
    pub fn turn(&self, t: f64) -> Result<Interval, CurveError> {
        form::check_parameter(self.domain(), t)?;
        let (index, xi) = self.locate(t)?;
        let panel = self.panels.get(index).ok_or(CurveError::Defect)?;
        heading(self.heading_of(panel), xi, 0)
    }

    /// The curvature at station `t`, enclosed: π·h′(ξ)/L for the heading h in half-turns.
    ///
    /// # Errors
    ///
    /// As [`Transition::point`].
    pub fn curvature(&self, t: f64) -> Result<Interval, CurveError> {
        form::check_parameter(self.domain(), t)?;
        let (index, xi) = self.locate(t)?;
        let panel = self.panels.get(index).ok_or(CurveError::Defect)?;
        let slope = heading(self.heading_of(panel), xi, 1)?;
        Ok(form::pi()?.mul(slope)?.div(point(self.length)?)?)
    }

    /// The heading a panel uses.
    fn heading_of(&self, panel: &Panel) -> &Heading {
        match (&self.second, panel.second) {
            (Some(h), true) => h,
            _ => &self.first,
        }
    }

    /// The fast path's panel for station t: its panel, its start and ξ, in doubles.
    fn panel_of(&self, t: f64) -> Result<(&Panel, f64, f64), CurveError> {
        let count = self.panels.len();
        let xi = t / self.length;
        let index = form::floor_index(xi * form::count(count)?)?;
        let index = index.min(count.checked_sub(1).ok_or(CurveError::Defect)?);
        let panel = self.panels.get(index).ok_or(CurveError::Defect)?;
        Ok((panel, ratio(index, count)?, xi))
    }

    /// The panel holding station t, decided exactly, and the enclosure of ξ = t/L inside it.
    fn locate(&self, t: f64) -> Result<(usize, Interval), CurveError> {
        let count = self.panels.len();
        let xi = t / self.length;
        let last = count.checked_sub(1).ok_or(CurveError::Defect)?;
        let mut index = form::floor_index(xi * form::count(count)?)?.min(last);
        // ξ was rounded, so it may have reached a panel's start that t/L lies just before: at most
        // one, since a panel is at least 2⁻¹⁶ wide. Never the other way: a panel's end is a double,
        // so a quotient at or past it rounds to it or past it. Every t reaches the first panel's
        // start, 0.
        if !reaches(t, self.length, ratio(index, count)?) {
            index = index.checked_sub(1).ok_or(CurveError::Defect)?;
        }
        let a = ratio(index, count)?;
        let b = ratio(index.checked_add(1).ok_or(CurveError::Defect)?, count)?;
        let x = tight::div(point(t)?, point(self.length)?)?;
        Ok((
            index,
            Interval::new(tight::larger(x.lo(), a), tight::smaller(x.hi(), b))?,
        ))
    }

    /// The derivatives of the unit tangent e^{iθ(t)} to order `order` − 1 at ξ, from the heading's
    /// derivatives: the first to the `order`-th derivatives of the curve, before the start
    /// direction turns them.
    fn units<N: Num>(&self, h: &Heading, xi: N, order: usize) -> Result<Vec<[N; 2]>, CurveError> {
        if order == 0 {
            return Ok(Vec::new());
        }
        // θ⁽ʲ⁾(t) = π·h⁽ʲ⁾(ξ)/Lʲ.
        let over = N::exact(1.0)?.div(N::exact(self.length)?)?;
        let mut scale = N::pi()?;
        let mut rates = Vec::with_capacity(order);
        for j in 1..order {
            scale = scale.mul(over)?;
            rates.push(scale.mul(heading(h, xi, j)?)?);
        }
        unit_derivatives(heading(h, xi, 0)?, &rates)
    }

    /// A local vector turned by the start direction.
    fn turned<N: Num>(&self, [x, y]: [N; 2]) -> Result<[N; 2], CurveError> {
        let [u, v] = N::unit(self);
        Ok([u.mul(x)?.sub(v.mul(y)?)?, v.mul(x)?.add(u.mul(y)?)?])
    }

    /// A local point placed at the start, in doubles.
    fn placed(&self, local: [f64; 2]) -> [f64; 2] {
        let [u, v] = self.unit_doubles;
        [
            self.start[0] + (u * local[0] - v * local[1]),
            self.start[1] + (v * local[0] + u * local[1]),
        ]
    }
}

/// The arithmetic the evaluation runs in: doubles for the fast path, intervals for the certified
/// one.
trait Num: Copy {
    /// The double x, exactly.
    fn exact(x: f64) -> Result<Self, CurveError>;
    /// A real whose correctly rounded double is x: a rule's node or weight.
    fn near(x: f64) -> Result<Self, CurveError>;
    /// π.
    fn pi() -> Result<Self, CurveError>;
    fn add(self, b: Self) -> Result<Self, CurveError>;
    fn sub(self, b: Self) -> Result<Self, CurveError>;
    fn mul(self, b: Self) -> Result<Self, CurveError>;
    fn div(self, b: Self) -> Result<Self, CurveError>;
    fn sinpi(self) -> Result<Self, CurveError>;
    fn cospi(self) -> Result<Self, CurveError>;
    /// The heading's polynomial coefficients.
    fn poly(h: &Heading) -> &[Self; TERMS];
    /// A wave's amplitude, phase and rate.
    fn wave(w: &Wave) -> [Self; 3];
    /// The unit start direction.
    fn unit(c: &Transition) -> [Self; 2];
}

impl Num for f64 {
    fn exact(x: f64) -> Result<f64, CurveError> {
        Ok(x)
    }
    fn near(x: f64) -> Result<f64, CurveError> {
        Ok(x)
    }
    fn pi() -> Result<f64, CurveError> {
        Ok(core::f64::consts::PI)
    }
    fn add(self, b: f64) -> Result<f64, CurveError> {
        Ok(self + b)
    }
    fn sub(self, b: f64) -> Result<f64, CurveError> {
        Ok(self - b)
    }
    fn mul(self, b: f64) -> Result<f64, CurveError> {
        Ok(self * b)
    }
    fn div(self, b: f64) -> Result<f64, CurveError> {
        Ok(self / b)
    }
    fn sinpi(self) -> Result<f64, CurveError> {
        Ok(math::sinpi(self)?.get())
    }
    fn cospi(self) -> Result<f64, CurveError> {
        Ok(math::cospi(self)?.get())
    }
    fn poly(h: &Heading) -> &[f64; TERMS] {
        &h.doubles
    }
    fn wave(w: &Wave) -> [f64; 3] {
        w.doubles
    }
    fn unit(c: &Transition) -> [f64; 2] {
        c.unit_doubles
    }
}

impl Num for Interval {
    fn exact(x: f64) -> Result<Interval, CurveError> {
        point(x)
    }
    fn near(x: f64) -> Result<Interval, CurveError> {
        Ok(Interval::new(x.next_down(), x.next_up())?)
    }
    fn pi() -> Result<Interval, CurveError> {
        form::pi()
    }
    fn add(self, b: Interval) -> Result<Interval, CurveError> {
        Ok(Interval::add(self, b)?)
    }
    fn sub(self, b: Interval) -> Result<Interval, CurveError> {
        Ok(Interval::sub(self, b)?)
    }
    fn mul(self, b: Interval) -> Result<Interval, CurveError> {
        Ok(Interval::mul(self, b)?)
    }
    fn div(self, b: Interval) -> Result<Interval, CurveError> {
        Ok(Interval::div(self, b)?)
    }
    fn sinpi(self) -> Result<Interval, CurveError> {
        Ok(Interval::sinpi(self)?)
    }
    fn cospi(self) -> Result<Interval, CurveError> {
        Ok(Interval::cospi(self)?)
    }
    fn poly(h: &Heading) -> &[Interval; TERMS] {
        &h.poly
    }
    fn wave(w: &Wave) -> [Interval; 3] {
        [w.amplitude, w.phase, w.rate]
    }
    fn unit(c: &Transition) -> [Interval; 2] {
        c.unit
    }
}

/// A point that must be finite.
fn finite(p: [f64; 2]) -> Result<[f64; 2], CurveError> {
    if p.iter().all(|v| v.is_finite()) {
        Ok(p)
    } else {
        Err(CurveError::NotFinite)
    }
}

/// k/n as a double, exact for the powers of two n of the panel counts.
fn ratio(k: usize, n: usize) -> Result<f64, CurveError> {
    Ok(form::count(k)? / form::count(n)?)
}

/// Whether t ≥ L·b, decided exactly: the sign of t − (p + e) for L·b = p + e exactly (the error
/// term has its exact sign even where it underflows, `two_prod`'s contract, and it is then too
/// small to decide any other difference).
fn reaches(t: f64, length: f64, b: f64) -> bool {
    let (p, e) = two_prod(length, b);
    let mut sum = ExactSum::new();
    sum.add(t);
    sum.add(-p);
    sum.add(-e);
    sum.round_ieee() >= 0.0
}

/// h⁽ʲ⁾(ξ), the j-th derivative of the heading with respect to ξ, in half-turns.
fn heading<N: Num>(h: &Heading, xi: N, j: usize) -> Result<N, CurveError> {
    let mut sum = N::exact(0.0)?;
    // Horner on the derivative's coefficients k·(k − 1)…(k − j + 1)·cₖ.
    for (k, &c) in N::poly(h).iter().enumerate().take(h.len).skip(j).rev() {
        let c = if j == 0 {
            c
        } else {
            c.mul(N::exact(falling(k, j)?)?)?
        };
        sum = sum.mul(xi)?.add(c)?;
    }
    if let Some(w) = &h.wave {
        let [amplitude, phase, rate] = N::wave(w);
        let arg = phase.add(rate.mul(xi)?)?;
        // d/dx sinpi x = π·cospi x, and cospi x = sinpi(x + ½): the j-th derivative of the wave
        // is (π·rate)ʲ·amplitude times sinpi, cospi, −sinpi or −cospi as j (plus one for a cosine)
        // is 0, 1, 2 or 3 modulo 4.
        let mut factor = amplitude;
        let step = N::pi()?.mul(rate)?;
        for _ in 0..j {
            factor = factor.mul(step)?;
        }
        let value = match j.wrapping_add(usize::from(w.cosine)) & 3 {
            0 => arg.sinpi()?,
            1 => arg.cospi()?,
            2 => N::exact(0.0)?.sub(arg.sinpi()?)?,
            _ => N::exact(0.0)?.sub(arg.cospi()?)?,
        };
        sum = sum.add(factor.mul(value)?)?;
    }
    Ok(sum)
}

/// k·(k − 1)…(k − j + 1), exactly: at most 8!.
fn falling(k: usize, j: usize) -> Result<f64, CurveError> {
    let mut f = 1.0;
    for i in 0..j {
        f *= form::count(k.checked_sub(i).ok_or(CurveError::Defect)?)?;
    }
    Ok(f)
}

/// The integrals of cos πh and sin πh over [a, a + 2·half] in ξ by the rule (not yet times L).
fn integral<N: Num>(h: &Heading, a: N, half: N) -> Result<[N; 2], CurveError> {
    let one = N::exact(1.0)?;
    let (mut x, mut y) = (N::exact(0.0)?, N::exact(0.0)?);
    for (&node, &weight) in RULE.nodes.iter().zip(RULE.weights) {
        let xi = a.add(half.mul(one.add(N::near(node)?)?)?)?;
        let turn = heading(h, xi, 0)?;
        let w = N::near(weight)?;
        x = x.add(w.mul(turn.cospi()?)?)?;
        y = y.add(w.mul(turn.sinpi()?)?)?;
    }
    Ok([x.mul(half)?, y.mul(half)?])
}

/// The derivatives of g = e^{iθ} from g = (cos πh, sin πh) at h = `turn` and θ′, θ″, … in
/// `rates`: g′ = iθ′·g, so by Leibniz's rule g⁽ᵏ⁺¹⁾ = Σⱼ C(k, j)·iθ⁽ʲ⁺¹⁾·g⁽ᵏ⁻ʲ⁾. Returns g and
/// its derivatives to the order of `rates`' length.
fn unit_derivatives<N: Num>(turn: N, rates: &[N]) -> Result<Vec<[N; 2]>, CurveError> {
    let zero = N::exact(0.0)?;
    let mut g = Vec::with_capacity(rates.len().saturating_add(1));
    g.push([turn.cospi()?, turn.sinpi()?]);
    let mut row = vec![N::exact(1.0)?];
    for _ in rates {
        let mut next = [zero, zero];
        for ((&c, &rate), &[x, y]) in row.iter().zip(rates).zip(g.iter().rev()) {
            let a = c.mul(rate)?;
            // i·a·(x + iy) = −a·y + i·a·x.
            let [re, im] = next;
            next = [re.sub(a.mul(y)?)?, im.add(a.mul(x)?)?];
        }
        g.push(next);
        // Pascal's next row.
        let mut wider = Vec::with_capacity(row.len().saturating_add(1));
        wider.push(N::exact(1.0)?);
        for (&p, &q) in row.iter().zip(row.iter().skip(1)) {
            wider.push(p.add(q)?);
        }
        wider.push(N::exact(1.0)?);
        row = wider;
    }
    Ok(g)
}

/// The panels: the smallest power-of-two count, at most `cap`, whose remainders sum to at most
/// L·2⁻⁵⁶, with the integrals to each panel's start; and the sum of the remainders.
fn build(
    first: &Heading,
    second: Option<&Heading>,
    length: f64,
    cap: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<(Vec<Panel>, f64), CurveError> {
    let tolerance = length * TOLERANCE;
    let mut count = if second.is_some() { 2 } else { 1 };
    loop {
        meter.charge(u64::try_from(count).map_err(|_| CurveError::Defect)?)?;
        let width = ratio(1, count)?;
        let half = width * 0.5;
        let mut bounds = Vec::with_capacity(count);
        let mut total = point(0.0)?;
        for p in 0..count {
            let a = ratio(p, count)?;
            let h = piece(first, second, a);
            let r = remainder(h, a, half, length)?;
            if !r.is_finite() {
                break;
            }
            total = total.add(point(r)?)?;
            if total.hi() > tolerance {
                break;
            }
            bounds.push(r);
        }
        if bounds.len() == count {
            return Ok((integrate(first, second, length, &bounds)?, total.hi()));
        }
        count = count.checked_mul(2).ok_or(CurveError::Defect)?;
        if count > cap {
            return Err(CurveError::TooManyPanels);
        }
    }
}

/// Helmert's second half from ξ = ½ on; the first heading everywhere else.
fn piece<'h>(first: &'h Heading, second: Option<&'h Heading>, a: f64) -> &'h Heading {
    match second {
        Some(h) if a >= 0.5 => h,
        _ => first,
    }
}

/// The panels' starting integrals, each panel's integral enclosed and widened by its remainder.
fn integrate(
    first: &Heading,
    second: Option<&Heading>,
    length: f64,
    bounds: &[f64],
) -> Result<Vec<Panel>, CurveError> {
    let count = bounds.len();
    let half = point(ratio(1, count)? * 0.5)?;
    let l = point(length)?;
    let (mut x, mut y) = (point(0.0)?, point(0.0)?);
    let mut panels = Vec::with_capacity(count);
    for (p, &r) in bounds.iter().enumerate() {
        let a = ratio(p, count)?;
        let h = piece(first, second, a);
        panels.push(Panel {
            x,
            y,
            doubles: [tight::middle(x), tight::middle(y)],
            remainder: r,
            second: second.is_some() && a >= 0.5,
        });
        let [qx, qy] = integral(h, point(a)?, half)?;
        let e = Interval::new(-r, r)?;
        x = x.add(l.mul(qx)?)?.add(e)?;
        y = y.add(l.mul(qy)?)?.add(e)?;
    }
    Ok(panels)
}

/// A bound on the rule's error over the panel [a, a + 2·half] in ξ, for both integrands, in
/// length units: the least of Trefethen's bounds over the ellipses tried (module documentation),
/// or +∞ if none gives a finite one.
fn remainder(h: &Heading, a: f64, half: f64, length: f64) -> Result<f64, CurveError> {
    let mid = point(a)?.add(point(half)?)?;
    let mut best = f64::INFINITY;
    let mut rho = 1.0;
    for _ in 0..ELLIPSES {
        rho *= 2.0;
        // An ellipse whose integrand or bound overflows is skipped: a smaller one may serve.
        if let Ok(bound) = trefethen(h, mid, half, rho, length) {
            best = tight::smaller(best, bound);
        }
    }
    Ok(best)
}

/// Trefethen's bound for the panel of middle `mid` and half-width `half` (in ξ) on the ellipse
/// Eρ: L·half·(64/15)·M·ρ⁻²ⁿ/(ρ² − 1), rounded up.
fn trefethen(
    h: &Heading,
    mid: Interval,
    half: f64,
    rho: f64,
    length: f64,
) -> Result<f64, CurveError> {
    // Eρ's semi-axes, (ρ ± 1/ρ)/2, scaled to the panel: exact for ρ ≤ 2¹².
    let inverse = 1.0 / rho;
    let reach = point(half * f64::midpoint(rho, inverse))?;
    let height = half * ((rho - inverse) * 0.5);
    let re = Interval::new(mid.sub(reach)?.lo(), mid.add(reach)?.hi())?;
    let size = ellipse_size(h, re, height)?;
    let mut power = 1.0;
    for _ in 0..NODES {
        power *= rho * rho;
    }
    Ok(point(length)?
        .mul(point(half)?)?
        .mul(point(64.0)?.div(point(15.0)?)?)?
        .mul(point(size)?)?
        .div(point(power)?)?
        .div(point(rho * rho - 1.0)?)?
        .hi())
}

/// An upper bound on |cos πh| and |sin πh| over the rectangle `re` + i·[−`height`, `height`] in
/// ξ, which holds the panel's ellipse: cosh(π·sup |Im h|), with Im h enclosed by complex interval
/// arithmetic, and |Im(A·sinpi w)| and |Im(A·cospi w)| at most |A|·sinh(π·|Im w|).
fn ellipse_size(h: &Heading, re: Interval, height: f64) -> Result<f64, CurveError> {
    let im = Interval::new(-height, height)?;
    let (mut ar, mut ai) = (point(0.0)?, point(0.0)?);
    for &c in h.poly.iter().take(h.len).rev() {
        let r = ar.mul(re)?.sub(ai.mul(im)?)?.add(c)?;
        ai = ar.mul(im)?.add(ai.mul(re)?)?;
        ar = r;
    }
    let pi = form::pi()?;
    let mut imaginary = point(tight::magnitude(ai))?;
    if let Some(w) = &h.wave {
        let s = pi
            .mul(point(tight::magnitude(w.rate))?)?
            .mul(point(height)?)?
            .hi();
        let sinh = point(s)?.exp()?.sub(point(-s)?.exp()?)?.mul(point(0.5)?)?;
        imaginary = imaginary.add(point(tight::magnitude(w.amplitude))?.mul(sinh)?)?;
    }
    // cosh y ≤ (e^y + 1)/2 for y ≥ 0.
    let e = pi.mul(imaginary)?.exp()?;
    Ok(e.add(point(1.0)?)?.mul(point(0.5)?)?.hi())
}

/// The law's heading in half-turns as a function of ξ, and Helmert's second half.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per law, each its heading's closed form"
)]
fn headings(law: &CurvatureLaw, length: f64) -> Result<(Heading, Option<Heading>), CurveError> {
    let mut poly = [point(0.0)?; TERMS];
    let mut wave = None;
    let mut second = None;
    let l = point(length)?;
    let over_pi = tight::div(point(1.0)?, form::pi()?)?;
    match *law {
        CurvatureLaw::Clothoid { start, end } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            poly[1] = k1;
            poly[2] = scaled(dk, 0.5)?;
        }
        CurvatureLaw::Bloss { start, end } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            poly[1] = k1;
            poly[3] = dk;
            poly[4] = scaled(dk, -0.5)?;
        }
        CurvatureLaw::Helmert { start, end } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            let third = tight::div(dk, point(3.0)?)?;
            poly[1] = k1;
            poly[3] = scaled(third, 2.0)?;
            // The second half, continuous with the first at ξ = ½ (both turn by k₁/2 + Δk/12):
            // k₁ξ + Δk·(1/6 − ξ + 2ξ² − (2/3)ξ³).
            let mut rest = [point(0.0)?; TERMS];
            rest[0] = scaled(third, 0.5)?;
            rest[1] = tight::sub(k1, dk)?;
            rest[2] = scaled(dk, 2.0)?;
            rest[3] = scaled(third, -2.0)?;
            second = Some(finish(rest, None)?);
        }
        CurvatureLaw::Cosine { start, end } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            poly[1] = tight::add(k1, scaled(dk, 0.5)?)?;
            wave = Some(Wave::new(
                false,
                tight::mul(scaled(dk, -0.5)?, over_pi)?,
                point(0.0)?,
                point(1.0)?,
            ));
        }
        CurvatureLaw::Sine { start, end } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            let amplitude = tight::mul(scaled(dk, 0.25)?, tight::square(over_pi)?)?;
            poly[0] = amplitude.neg();
            poly[1] = k1;
            poly[2] = scaled(dk, 0.5)?;
            wave = Some(Wave::new(true, amplitude, point(0.0)?, point(2.0)?));
        }
        CurvatureLaw::CantCoupled {
            start,
            end,
            height,
            cant,
        } => {
            let (k1, dk) = rates(start, end, l, over_pi)?;
            let g = tight::mul(
                tight::div(tight::mul(point(height)?, point(cant)?)?, l)?,
                over_pi,
            )?;
            poly[1] = k1;
            poly[3] = scaled(g, -140.0)?;
            poly[4] = scaled(g, 420.0)?;
            poly[5] = tight::sub(scaled(dk, 7.0)?, scaled(g, 420.0)?)?;
            poly[6] = tight::add(scaled(dk, -14.0)?, scaled(g, 140.0)?)?;
            poly[7] = scaled(dk, 10.0)?;
            poly[8] = scaled(dk, -2.5)?;
        }
        CurvatureLaw::Polynomial { terms, from } => {
            let f = point(from)?;
            // (f + Lξ)ᵐ by repeated products, its constant fᵐ dropped: θ(f + Lξ) − θ(f).
            let mut power = [point(0.0)?; TERMS];
            power[0] = point(1.0)?;
            for (k, term) in terms.iter().enumerate() {
                power = times_affine(&power, f, l)?;
                let Some(a) = *term else {
                    continue;
                };
                let m = form::count(k.checked_add(1).ok_or(CurveError::Defect)?)?;
                // sign(A)/((k + 1)·π·|A|ᵏ⁺¹).
                let mut den = point(m)?;
                for _ in 0..=k {
                    den = tight::mul(den, point(nonzero(a)?.abs())?)?;
                }
                let c = tight::mul(tight::div(point(a.signum())?, den)?, over_pi)?;
                for (slot, &p) in poly.iter_mut().zip(&power).skip(1) {
                    *slot = tight::add(*slot, tight::mul(c, p)?)?;
                }
            }
        }
        CurvatureLaw::CosineSpiral {
            constant,
            cosine,
            length: lc,
            from,
        } => {
            let lc = positive(lc)?;
            if let Some(a0) = constant {
                poly[1] = linear(l, a0, over_pi)?;
            }
            // (Lc/(π²·A₁))·(sinpi((f + Lξ)/Lc) − sinpi(f/Lc)).
            let amplitude = tight::mul(
                tight::div(lc, point(nonzero(cosine)?)?)?,
                tight::square(over_pi)?,
            )?;
            let phase = tight::div(point(from)?, lc)?;
            poly[0] = tight::mul(amplitude, phase.sinpi()?)?.neg();
            wave = Some(Wave::new(false, amplitude, phase, tight::div(l, lc)?));
        }
        CurvatureLaw::SineSpiral {
            constant,
            linear: a1,
            sine,
            length: ls,
            from,
        } => {
            let ls = positive(ls)?;
            if let Some(a0) = constant {
                poly[1] = linear(l, a0, over_pi)?;
            }
            if let Some(a1) = a1 {
                // sign(A₁)/(2π·A₁²)·((f + Lξ)² − f²) = sign(A₁)/(2π·A₁²)·(2fLξ + L²ξ²).
                let c = tight::mul(
                    tight::div(
                        point(a1.signum())?,
                        scaled(tight::square(point(nonzero(a1)?)?)?, 2.0)?,
                    )?,
                    over_pi,
                )?;
                let fl = tight::mul(point(from)?, l)?;
                poly[1] = tight::add(poly[1], tight::mul(c, scaled(fl, 2.0)?)?)?;
                poly[2] = tight::mul(c, tight::square(l)?)?;
            }
            // −(Ls/(2π²·A₂))·(cospi(2(f + Lξ)/Ls) − cospi(2f/Ls)).
            let amplitude = tight::mul(
                tight::div(ls, point(nonzero(sine)?)?)?,
                scaled(tight::square(over_pi)?, -0.5)?,
            )?;
            let phase = scaled(tight::div(point(from)?, ls)?, 2.0)?;
            poly[0] = tight::mul(amplitude, phase.cospi()?)?.neg();
            wave = Some(Wave::new(
                true,
                amplitude,
                phase,
                scaled(tight::div(l, ls)?, 2.0)?,
            ));
        }
    }
    Ok((finish(poly, wave)?, second))
}

/// κ₁·L/π and (κ₂ − κ₁)·L/π: the start curvature and the change, as turns in half-turns over the
/// length.
fn rates(
    start: f64,
    end: f64,
    l: Interval,
    over_pi: Interval,
) -> Result<(Interval, Interval), CurveError> {
    let (k1, k2) = (point(start)?, point(end)?);
    Ok((
        tight::mul(tight::mul(k1, l)?, over_pi)?,
        tight::mul(tight::mul(tight::sub(k2, k1)?, l)?, over_pi)?,
    ))
}

/// L/(π·A₀): a constant term's turn per unit ξ.
fn linear(l: Interval, a0: f64, over_pi: Interval) -> Result<Interval, CurveError> {
    tight::mul(tight::div(l, point(nonzero(a0)?)?)?, over_pi)
}

/// x times the double s.
fn scaled(x: Interval, s: f64) -> Result<Interval, CurveError> {
    tight::mul(x, point(s)?)
}

/// An IFC term, refused when zero (its reciprocal is the curvature).
fn nonzero(a: f64) -> Result<f64, CurveError> {
    if a == 0.0 {
        Err(CurveError::Degenerate)
    } else if a.is_finite() {
        Ok(a)
    } else {
        Err(CurveError::NotFinite)
    }
}

/// An IFC formula length, refused unless positive.
fn positive(x: f64) -> Result<Interval, CurveError> {
    if !x.is_finite() {
        Err(CurveError::NotFinite)
    } else if x > 0.0 {
        point(x)
    } else {
        Err(CurveError::Degenerate)
    }
}

/// The polynomial p(ξ) times (f + Lξ), its top coefficient dropped (it is zero: callers stay
/// within degree 8).
fn times_affine(
    p: &[Interval; TERMS],
    f: Interval,
    l: Interval,
) -> Result<[Interval; TERMS], CurveError> {
    let mut out = [point(0.0)?; TERMS];
    let mut below = point(0.0)?;
    for (slot, &here) in out.iter_mut().zip(p) {
        *slot = tight::add(tight::mul(here, f)?, tight::mul(below, l)?)?;
        below = here;
    }
    Ok(out)
}

impl Wave {
    fn new(cosine: bool, amplitude: Interval, phase: Interval, rate: Interval) -> Wave {
        Wave {
            cosine,
            amplitude,
            phase,
            rate,
            doubles: [amplitude, phase, rate].map(tight::middle),
        }
    }
}

/// The heading with its doubles and degree.
fn finish(poly: [Interval; TERMS], wave: Option<Wave>) -> Result<Heading, CurveError> {
    let zero = point(0.0)?;
    let len = poly
        .iter()
        .rposition(|c| *c != zero)
        .map_or(Ok(0), |k| k.checked_add(1).ok_or(CurveError::Defect))?;
    Ok(Heading {
        poly,
        doubles: poly.map(tight::middle),
        len,
        wave,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        reason = "tests: a failure stops the test, and the formulas read as written"
    )]

    use super::*;

    /// The heading c·ξ² with no wave.
    fn quadratic(c: f64) -> Heading {
        let mut poly = [point(0.0).unwrap(); TERMS];
        poly[2] = point(c).unwrap();
        finish(poly, None).unwrap()
    }

    /// The integrals over [0, 1] by the rule on `n` panels, enclosed: far more accurate than one.
    fn fine(h: &Heading, n: usize) -> [Interval; 2] {
        let half = point(ratio(1, n).unwrap() * 0.5).unwrap();
        let (mut x, mut y) = (point(0.0).unwrap(), point(0.0).unwrap());
        for p in 0..n {
            let [qx, qy] = integral(h, point(ratio(p, n).unwrap()).unwrap(), half).unwrap();
            x = x.add(qx).unwrap();
            y = y.add(qy).unwrap();
        }
        [x, y]
    }

    #[test]
    fn the_remainder_bounds_a_visible_rule_error() {
        // cos and sin of π·cξ² over one panel [0, 1]: eight nodes leave an error far above the
        // rounding, which the bound must cover; 1024 panels give the integrals to about 10⁻¹³.
        for (c, pinned) in [
            (1.5, 0x3ef1_319e_7465_7d7d),
            (3.0, 0x3f8f_95e4_329b_dc55),
            (6.0, 0x4045_e7cd_e9bd_c798),
        ] {
            let h = quadratic(c);
            let r = remainder(&h, 0.0, 0.5, 1.0).unwrap();
            let one = integral(&h, point(0.0).unwrap(), point(0.5).unwrap()).unwrap();
            for (q, e) in one.iter().zip(fine(&h, 1024)) {
                let err = tight::magnitude(q.sub(e).unwrap());
                assert!(err <= r, "c = {c}: error {err:e} above the bound {r:e}");
                assert!(
                    err > 1e-12,
                    "c = {c}: error {err:e} too small to test the bound"
                );
            }
            // Pinned, so a change to the bound is seen even where it stays above the error.
            assert_eq!(r.to_bits(), pinned, "c = {c}: {r:e}");
        }
    }

    #[test]
    fn the_ellipse_size_bounds_the_integrand_on_the_rectangle() {
        // h = 0.3ξ + 0.7ξ² + 0.2·sinpi(0.25 + 1.5ξ): |cos πh| and |sin πh| at points of the
        // rectangle, by cos(π(x + iy)) = cospi x·cosh πy − i·sinpi x·sinh πy.
        let mut poly = [point(0.0).unwrap(); TERMS];
        poly[1] = point(0.3).unwrap();
        poly[2] = point(0.7).unwrap();
        let wave = Wave::new(
            false,
            point(0.2).unwrap(),
            point(0.25).unwrap(),
            point(1.5).unwrap(),
        );
        let h = finish(poly, Some(wave)).unwrap();
        let (re, height) = (Interval::new(-0.5, 1.5).unwrap(), 0.75);
        let size = ellipse_size(&h, re, height).unwrap();
        let pi = core::f64::consts::PI;
        for i in 0..=8 {
            for k in 0..=8 {
                let (x, y) = (-0.5 + 0.25 * f64::from(i), -0.75 + 0.1875 * f64::from(k));
                // h(x + iy) in complex doubles, its wave by
                // sin(a + ib) = sin a·cosh b + i·cos a·sinh b.
                let (zr, zi) = (x, y);
                let (sr, si) = (zr * zr - zi * zi, 2.0 * zr * zi);
                let (ar, ai) = (0.25 + 1.5 * zr, 1.5 * zi);
                let (eb, ebi) = (
                    math::exp(pi * ai).unwrap().get(),
                    math::exp(-pi * ai).unwrap().get(),
                );
                let (ch, sh) = (f64::midpoint(eb, ebi), (eb - ebi) * 0.5);
                let hr = 0.3 * zr + 0.7 * sr + 0.2 * math::sinpi(ar).unwrap().get() * ch;
                let hi = 0.3 * zi + 0.7 * si + 0.2 * math::cospi(ar).unwrap().get() * sh;
                let (e, ei) = (
                    math::exp(pi * hi).unwrap().get(),
                    math::exp(-pi * hi).unwrap().get(),
                );
                let (ch, sh) = (f64::midpoint(e, ei), (e - ei) * 0.5);
                let (c, s) = (
                    math::cospi(hr).unwrap().get(),
                    math::sinpi(hr).unwrap().get(),
                );
                let cos = (c * c * ch * ch + s * s * sh * sh).sqrt();
                let sin = (s * s * ch * ch + c * c * sh * sh).sqrt();
                assert!(
                    cos <= size && sin <= size,
                    "at {x} + {y}i: {cos}, {sin} above {size}"
                );
            }
        }
        assert_eq!(size.to_bits(), 0x4159_9827_b4f5_5741);
    }

    #[test]
    fn the_remainders_widen_the_enclosures() {
        // Remainders far above the rounding: each panel's start holds every earlier one's, and a
        // station's enclosure its own panel's too.
        let h = quadratic(0.25);
        let panels = integrate(&h, None, 2.0, &[0.5, 0.25, 0.125, 0.0625]).unwrap();
        for (p, widened) in panels.iter().zip([0.0, 0.5, 0.75, 0.875]) {
            for i in [p.x, p.y] {
                let width = i.hi() - i.lo();
                assert!(
                    width >= 2.0 * widened && width <= 2.0 * widened + 1e-12,
                    "{i:?}"
                );
            }
            assert!(!p.second);
        }
        let law = CurvatureLaw::Clothoid {
            start: 0.0,
            end: 0.5,
        };
        let mut meter = WorkMeter::new(geom_run::Budget::UNLIMITED);
        let mut t = Transition::new([0.0, 0.0], [1.0, 0.0], 2.0, law, &mut meter).unwrap();
        let narrow = t.enclose(1.0, 0, &mut meter).unwrap()[0];
        let last = t.panels.len() - 1;
        t.panels[last].remainder = 0.5;
        let wide = t.enclose(2.0, 0, &mut meter).unwrap()[0];
        for (n, w) in narrow.iter().zip(wide) {
            assert!(
                n.hi() - n.lo() < 1e-12 && w.hi() - w.lo() >= 1.0,
                "{n:?} {w:?}"
            );
        }
    }

    #[test]
    fn the_cap_is_the_last_count_tried() {
        // This Bloss transition needs four panels (attack_g63.rs pins it): a cap of four builds
        // them, a cap of two refuses.
        let law = CurvatureLaw::Bloss {
            start: 1.0 / 1000.0,
            end: 1.0 / 350.0,
        };
        let (h, _) = headings(&law, 200.0).unwrap();
        let mut meter = WorkMeter::new(geom_run::Budget::UNLIMITED);
        let (panels, _) = build(&h, None, 200.0, 4, &mut meter).unwrap();
        assert_eq!(panels.len(), 4);
        assert_eq!(
            build(&h, None, 200.0, 2, &mut meter),
            Err(CurveError::TooManyPanels)
        );
    }

    #[test]
    fn stations_are_placed_exactly() {
        // 0.1 is not a double: L·0.5 for L = 0.1 is the double 0.05, exactly.
        let (l, b) = (0.1, 0.5);
        assert!(reaches(0.05, l, b));
        assert!(!reaches(0.05f64.next_down(), l, b));
        // 3·(1/3 rounded) is not 1: the exact product decides, not its rounding.
        let third = 1.0 / 3.0;
        let (p, e) = two_prod(3.0, third);
        assert!(e != 0.0);
        assert_eq!(reaches(p, 3.0, third), e <= 0.0);
        assert!(reaches(p.next_up(), 3.0, third));
        assert!(!reaches(p.next_down(), 3.0, third));
        assert!(reaches(0.0, 1.0, 0.0));
    }

    #[test]
    fn the_powers_of_an_affine_term_are_binomial() {
        // (2 + 3ξ)³ = 8 + 36ξ + 54ξ² + 27ξ³.
        let mut p = [point(0.0).unwrap(); TERMS];
        p[0] = point(1.0).unwrap();
        for _ in 0..3 {
            p = times_affine(&p, point(2.0).unwrap(), point(3.0).unwrap()).unwrap();
        }
        let want = [8.0, 36.0, 54.0, 27.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for (c, w) in p.iter().zip(want) {
            assert_eq!(*c, point(w).unwrap());
        }
        assert_eq!(falling(8, 3).unwrap(), 336.0);
        assert_eq!(falling(5, 0).unwrap(), 1.0);
        assert_eq!(ratio(3, 8).unwrap(), 0.375);
    }
}
