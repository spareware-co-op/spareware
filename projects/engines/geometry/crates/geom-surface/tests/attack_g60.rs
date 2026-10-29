//! Attacks on piece G60's claims for surfaces: the kind contract's margins (each law fires just
//! past what it allows and not just inside it, normal signs included), its refusal probes one at a
//! time and on any domain, and the NURBS kind on regions that touch knots, at creases approached
//! from either side, at the domain's corners and outside it.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    reason = "tests: native arithmetic and indexing, and a failure stops the test"
)]

use geom_curve::{ContractError, CurveError, DerivativeBound, Law, ParamMap};
use geom_math::{Axis, GridTransform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Basis, Bounds, Changed, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::contract::check;
use geom_surface::{ParamBox, ParamSurface, SurfaceForm, SurfaceKind};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

/// The contract's relative slack between doubles and an enclosure, 2^−30.
const SLACK: f64 = f64::from_bits(0x3E10_0000_0000_0000);

fn quarter() -> TypedTransform {
    TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1))
}

/// A sheet of degrees (1, 2) on [0, 1]² near (100, 100, 100), its normal along +z (or −z when
/// `down`), where the slack's scale is about 300.
fn sheet(down: bool) -> Surface<3> {
    let y = if down { -1.0 } else { 1.0 };
    let mut pts = Vec::new();
    for i in 0..2 {
        for j in 0..3 {
            let (fi, fj) = (f64::from(i), f64::from(j));
            pts.push([100.0 + fi, 100.0 + y * fj, 100.0 + 0.25 * fi * fj]);
        }
    }
    Surface::new(
        Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
        Basis::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap(),
        &pts,
    )
    .unwrap()
}

fn params() -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for k in 0..=4 {
        for l in 0..=4 {
            out.push((f64::from(k) / 4.0, f64::from(l) / 4.0));
        }
    }
    out
}

/// `s` moved by `dx` along x.
fn shifted(s: &Surface<3>, dx: f64) -> Surface<3> {
    let w = s.weights();
    let pts: Vec<[f64; 3]> = s
        .homogeneous()
        .iter()
        .enumerate()
        .map(|(i, h)| [h[0] + dx * w.map_or(1.0, |w| w[i]), h[1], h[2]])
        .collect();
    Surface::from_homogeneous(
        s.basis_u().clone(),
        s.basis_v().clone(),
        pts,
        w.map(<[f64]>::to_vec),
    )
    .unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fault {
    None,
    Nurbs(f64, f64),
    Evaluate(f64, bool),
    PointOutside,
    SpanOutside,
    SplitAtStart,
    LocalBound,
    DerivativeBound,
    /// The normal bound's z range replaced by this one.
    NormalZ(f64, f64),
}

#[derive(Clone)]
struct Faulty {
    inner: Surface<3>,
    fault: Fault,
}

impl ParamSurface for Faulty {
    fn domain(&self) -> ParamBox {
        ParamSurface::domain(&self.inner)
    }
    fn point(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[f64; 3], CurveError> {
        let ((_, b), _) = self.domain();
        if self.fault == Fault::PointOutside && u > b {
            return ParamSurface::point(&self.inner, b, v, sides);
        }
        ParamSurface::point(&self.inner, u, v, sides)
    }
    fn partials(&self, u: f64, v: f64, sides: (Limit, Limit)) -> Result<[[f64; 3]; 3], CurveError> {
        let mut out = self.inner.partials(u, v, sides)?;
        if let Fault::Evaluate(k, above) = self.fault {
            let e = self.inner.enclose(u, v, sides, &mut meter())?;
            let (lo, hi) = (e[0][0].lo(), e[0][0].hi());
            let end = if above { hi } else { lo };
            let scale = 1.0 + end.abs() + lo.abs() + hi.abs();
            out[0][0] = if above {
                hi + k * SLACK * scale
            } else {
                lo - k * SLACK * scale
            };
        }
        Ok(out)
    }
    fn enclose(
        &self,
        u: f64,
        v: f64,
        sides: (Limit, Limit),
        meter: &mut WorkMeter<'_>,
    ) -> Result<[[Interval; 3]; 3], CurveError> {
        self.inner.enclose(u, v, sides, meter)
    }
    fn bound(&self, region: ParamBox, meter: &mut WorkMeter<'_>) -> Result<Bounds<3>, CurveError> {
        let dom = self.domain();
        if self.fault == Fault::SpanOutside && region.0.0 < dom.0.0 {
            return self.inner.bound(dom, meter);
        }
        let mut b = self.inner.bound(region, meter)?;
        if self.fault == Fault::LocalBound && region != dom {
            for d in 0..3 {
                let mid = b.lo[d] * 0.5 + b.hi[d] * 0.5;
                b.lo[d] = mid;
                b.hi[d] = mid;
            }
        }
        Ok(b)
    }
    fn derivative_bound(
        &self,
        region: ParamBox,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<DerivativeBound<3>, CurveError> {
        let b = self.inner.derivative_bound(region, dir, meter)?;
        if self.fault == Fault::DerivativeBound {
            return Ok(DerivativeBound::Bounded(Bounds {
                lo: [1e9; 3],
                hi: [2e9; 3],
            }));
        }
        Ok(b)
    }
    fn normal_bound(
        &self,
        region: ParamBox,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Bounds<3>, CurveError> {
        let mut b = self.inner.normal_bound(region, meter)?;
        if let Fault::NormalZ(lo, hi) = self.fault {
            b.lo[2] = lo;
            b.hi[2] = hi;
        }
        Ok(b)
    }
    fn to_nurbs(&self, meter: &mut WorkMeter<'_>) -> Result<Changed<SurfaceForm>, CurveError> {
        if let Fault::Nurbs(dx, deviation) = self.fault {
            return Ok(Changed {
                value: SurfaceForm {
                    surface: shifted(&self.inner, dx),
                    u: ParamMap::Same,
                    v: ParamMap::Same,
                },
                deviation,
            });
        }
        self.inner.to_nurbs(meter)
    }
    fn split(
        &self,
        dir: Direction,
        t: f64,
        meter: &mut WorkMeter<'_>,
    ) -> Result<(Changed<SurfaceKind>, Changed<SurfaceKind>), CurveError> {
        let ((a, b), _) = self.domain();
        if self.fault == Fault::SplitAtStart && dir == Direction::U && t == a {
            return self.inner.split(dir, f64::midpoint(a, b), meter);
        }
        self.inner.split(dir, t, meter)
    }
    fn reversed(
        &self,
        dir: Direction,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.inner.reversed(dir, meter)
    }
    fn transformed(
        &self,
        transform: &TypedTransform,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Changed<SurfaceKind>, CurveError> {
        self.inner.transformed(transform, meter)
    }
}

fn run_on(down: bool, fault: Fault) -> Result<(), ContractError> {
    let kind = Faulty {
        inner: sheet(down),
        fault,
    };
    check(&kind, &params(), &quarter(), &mut meter())
}

fn run(fault: Fault) -> Result<(), ContractError> {
    run_on(false, fault)
}

/// Claim: a conversion within its stated deviation passes, whichever way it is off; one off by
/// more fails.
#[test]
fn a_conversion_passes_within_its_deviation_either_way_and_fails_beyond() {
    assert_eq!(run(Fault::None), Ok(()));
    for dx in [1e-3, -1e-3] {
        assert_eq!(run(Fault::Nurbs(dx, 1e-3)), Ok(()), "moved {dx}");
        let e = run(Fault::Nurbs(dx, 1e-4)).unwrap_err();
        assert_eq!(e.law, Law::NurbsWithinItsError, "moved {dx}: {e}");
    }
}

/// Claim: doubles may lie up to slack·(1 + |x| + |lo| + |hi|) outside the enclosure, on either
/// side, and not further.
#[test]
fn evaluation_in_doubles_has_exactly_its_slack() {
    for above in [true, false] {
        assert_eq!(run(Fault::Evaluate(0.999, above)), Ok(()), "above {above}");
        let e = run(Fault::Evaluate(1.001, above)).unwrap_err();
        assert_eq!(e.law, Law::EvaluationIsEnclosed, "above {above}: {e}");
    }
}

/// Claim: each refusal is probed on its own.
#[test]
fn each_missing_refusal_is_caught_alone() {
    for fault in [Fault::PointOutside, Fault::SpanOutside, Fault::SplitAtStart] {
        let e = run(fault).unwrap_err();
        assert_eq!(e.law, Law::RefusesOutside, "{fault:?}: {e}");
    }
}

/// Claim: a bound wrong only away from the whole domain, or a derivative bound holding nothing, is
/// caught by its law.
#[test]
fn a_local_bound_or_a_derivative_bound_that_misses_is_caught() {
    assert_eq!(
        run(Fault::LocalBound).unwrap_err().law,
        Law::BoundHoldsPoints
    );
    assert_eq!(
        run(Fault::DerivativeBound).unwrap_err().law,
        Law::DerivativeBoundHoldsDerivatives
    );
}

/// Claim: a normal bound decides a sign only by a strict inequality: a side at exactly 0 decides
/// nothing, a correct strict side passes, and a strict side against the normal's sign fails, on
/// surfaces facing either way.
#[test]
fn a_normal_bound_decides_a_sign_only_strictly() {
    // The sheet's normal is +z (or −z when turned down).
    assert_eq!(run_on(false, Fault::None), Ok(()));
    assert_eq!(run_on(true, Fault::None), Ok(()));
    assert_eq!(run_on(true, Fault::NormalZ(0.0, 1.0)), Ok(()));
    assert_eq!(run_on(false, Fault::NormalZ(-1.0, 0.0)), Ok(()));
    for (down, z) in [(true, (0.5, 1.0)), (false, (-1.0, -0.5))] {
        let e = run_on(down, Fault::NormalZ(z.0, z.1)).unwrap_err();
        assert_eq!(e.law, Law::NormalBoundHoldsDirections, "down {down}: {e}");
    }
}

/// Claim: the refusal probes lie outside any domain: on [3.3, 4], [−1, 4] and [−10, 1] a probe
/// built from the domain's ends without care could land inside it.
#[test]
fn the_probes_lie_outside_any_domain() {
    for (a, b) in [(3.3, 4.0), (-1.0, 4.0), (-10.0, 1.0)] {
        let s = Surface::new(
            Basis::new(1, vec![a, a, b, b]).unwrap(),
            Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap(),
            &[
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
            ],
        )
        .unwrap();
        let ps = [(a, 0.0), (f64::midpoint(a, b), 0.5), (b, 1.0)];
        check(&s, &ps, &quarter(), &mut meter()).unwrap_or_else(|e| panic!("[{a}, {b}]: {e}"));
    }
}

/// Degree 1 both ways, a crease along u = 1 (`across_u`) or v = 1: from (0, ·, 0) flat to x = 1,
/// then straight up in z. Domain [0, 2] × [0, 1] (or the transpose).
fn creased(across_u: bool) -> Surface<3> {
    let two = Basis::new(1, vec![0.0, 0.0, 1.0, 2.0, 2.0]).unwrap();
    let one = Basis::new(1, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    let path = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
    if across_u {
        let mut pts = Vec::new();
        for [x, z] in path {
            for y in [0.0, 1.0] {
                pts.push([x, y, z]);
            }
        }
        Surface::new(two, one, &pts).unwrap()
    } else {
        let mut pts = Vec::new();
        for y in [0.0, 1.0] {
            for [x, z] in path {
                pts.push([x, y, z]);
            }
        }
        Surface::new(one, two, &pts).unwrap()
    }
}

fn holds(b: &Bounds<3>, p: [f64; 3]) -> bool {
    (0..3).all(|d| b.lo[d] <= p[d] && p[d] <= b.hi[d])
}

/// Claim: at a crease, enclosure and partials take the side asked for in each direction, and at
/// the domain's first and last parameters each side is the one patch there.
#[test]
fn a_crease_is_approached_from_the_side_asked_for() {
    let m = &mut meter();
    for across_u in [true, false] {
        let s = creased(across_u);
        let at = |t: f64| if across_u { (t, 0.5) } else { (0.5, t) };
        let side = |l: Limit| {
            if across_u {
                (l, Limit::FromRight)
            } else {
                (Limit::FromRight, l)
            }
        };
        // The partial across the crease: (1, 0, 0) before it, (0, 0, 1) after.
        let k = if across_u { 1 } else { 2 };
        let (u, v) = at(1.0);
        let right = s.enclose(u, v, side(Limit::FromRight), m).unwrap();
        let left = s.enclose(u, v, side(Limit::FromLeft), m).unwrap();
        assert!(
            right[k][0].contains(0.0) && right[k][2].contains(1.0),
            "{right:?}"
        );
        assert!(
            left[k][0].contains(1.0) && left[k][2].contains(0.0),
            "{left:?}"
        );
        // The domain's ends, from the side that has no patch beyond them.
        for (t, l) in [(0.0, Limit::FromLeft), (2.0, Limit::FromRight)] {
            let (u, v) = at(t);
            assert!(s.enclose(u, v, side(l), m).is_ok(), "{t} {l:?}");
        }
        for (u, v) in [(0.0, 0.0), (0.0, 1.0)] {
            let sides = (Limit::FromLeft, Limit::FromLeft);
            assert!(s.enclose(u, v, sides, m).is_ok(), "({u}, {v})");
        }
    }
}

/// Claim: regions that touch a knot bound only the patches they cover: a region ending at the
/// crease or starting there is fine, a single parameter bounds the line there, and a region inside
/// one patch bounds that part of it, not the patch or its neighbour.
#[test]
fn regions_touching_a_knot_bound_what_they_cover() {
    let m = &mut meter();
    let s = creased(true);
    let v = (0.0, 1.0);
    for u in [(0.5, 1.0), (1.0, 1.5), (1.0, 1.0), (0.5, 0.5), (1.5, 1.5)] {
        let b = s.bound((u, v), m).unwrap_or_else(|e| panic!("{u:?}: {e}"));
        for t in [u.0, f64::midpoint(u.0, u.1), u.1] {
            let p = ParamSurface::point(&s, t, 0.5, (Limit::FromRight, Limit::FromRight)).unwrap();
            assert!(holds(&b, p), "{u:?} at {t}: {b:?}");
        }
        // Before the crease z stays 0; after it x stays 1. A single parameter at the crease bounds
        // both patches whole, by design.
        let at_crease = u == (1.0, 1.0);
        if u.1 <= 1.0 && !at_crease {
            assert!(b.hi[2] <= 1e-12, "{u:?}: {b:?}");
        }
        if u.0 >= 1.0 && !at_crease {
            assert!(b.lo[0] >= 1.0 - 1e-12, "{u:?}: {b:?}");
        }
        assert!(s.derivative_bound((u, v), Direction::U, m).is_ok(), "{u:?}");
        assert!(s.normal_bound((u, v), m).is_ok(), "{u:?}");
    }
    // A small region inside the first patch: x within [0.5, 0.75].
    let b = s.bound(((0.5, 0.75), (0.25, 0.5)), m).unwrap();
    assert!(b.lo[0] >= 0.5 - 1e-12 && b.hi[0] <= 0.75 + 1e-12, "{b:?}");
    assert!(b.lo[1] >= 0.25 - 1e-12 && b.hi[1] <= 0.5 + 1e-12, "{b:?}");
}

/// Claim: refusals come first and cost nothing; a split must lie strictly inside the domain.
#[test]
fn refusals_come_first() {
    let s = creased(true);
    let sides = (Limit::FromRight, Limit::FromRight);
    let mut m = meter();
    for (u, v) in [(-0.5, 0.5), (2.5, 0.5), (1.0, -0.5), (1.0, 1.5)] {
        assert_eq!(
            s.enclose(u, v, sides, &mut m),
            Err(CurveError::ParameterOutside),
            "({u}, {v})"
        );
    }
    assert_eq!(m.used().work, 0);
    for (dir, t) in [
        (Direction::U, 0.0),
        (Direction::U, 2.0),
        (Direction::V, 0.0),
        (Direction::V, 1.0),
    ] {
        assert_eq!(
            s.split(dir, t, &mut meter()).err(),
            Some(CurveError::ParameterOutside),
            "{dir:?} {t}"
        );
    }
}
