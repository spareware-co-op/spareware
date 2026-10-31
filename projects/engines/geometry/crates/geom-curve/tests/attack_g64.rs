//! Attacks on piece G64's claims: the transition kind's conversion to NURBS, split, reversal and
//! typed transforms, and its bounds, against the same independent truth as piece G63's attacks (a
//! 256-bit fixed-point integral of each law's heading, worked from IFC 4.3's curvature formulas,
//! `tests/laws` and `tests/series`), and against each other: a split part and a reversed curve
//! must evaluate as the original does, bit for bit; a reversed curve must be the transition built
//! from the original's end with the law run backwards, which this file works out from the
//! curvature formulas itself; a NURBS form must stay within its deviation of the truth; a moved
//! curve within its deviation of the exact image of the truth. A call may refuse an attack; it
//! may never return a result outside its stated error.
//!
//! Covers: CurveKind::Transition.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop,
    reason = "tests written as the formulas read: native arithmetic and indexing, math names, and a failure stops the test"
)]

mod laws;
mod series;

use dashu_int::IBig;
use geom_curve::contract;
use geom_curve::taylor;
use geom_curve::{
    Basis, Composite, CurvatureLaw, CurveError, CurveKind, DerivativeBound, Offset, OffsetLaw,
    ParamCurve, ParamMap, Transition, Trimmed,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_num::interval::Interval;
use geom_nurbs::{Bounds, Changed, Limit};
use geom_run::{Budget, WorkMeter};
use laws::{Case, cases, truth};
use series::{F, Truth, fx, mul, one};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn build(c: &Case) -> Transition<2> {
    Transition::new(c.start, c.direction, c.length, c.law, &mut meter())
        .unwrap_or_else(|e| panic!("{}: {e}", c.name))
}

/// The truth's slack: its own truncation, far below a double's step.
fn slack() -> IBig {
    one() >> 200
}

/// Whether the interval, widened by `widen`, holds v.
fn holds(i: Interval, v: &IBig, widen: &IBig) -> bool {
    fx(i.lo()) - widen - slack() <= *v && *v <= fx(i.hi()) + widen + slack()
}

/// The truth's point at station s.
fn truth_point(truth: &Truth, s: f64) -> [IBig; 2] {
    truth.placed(truth.local(&fx(s)))
}

/// The whole transition, run forward and reversed, and a part of it each way.
fn variants(t: &Transition<2>) -> Vec<(&'static str, Transition<2>)> {
    let l = t.length();
    let window = (l / 7.0, 0.8 * l);
    vec![
        ("whole", t.clone()),
        ("whole, reversed", t.segment((0.0, l), true).unwrap()),
        ("part", t.segment(window, false).unwrap()),
        ("part, reversed", t.segment(window, true).unwrap()),
    ]
}

/// Parameters across a domain: its ends, inside, and Helmert's middle when the domain holds it.
fn params(t: &Transition<2>) -> Vec<f64> {
    let (a, b) = t.domain();
    let mut out: Vec<f64> = [0.0, 0.13, 0.5, 0.77, 1.0]
        .iter()
        .map(|&q| along(a, b, q))
        .collect();
    let middle = t.length() / 2.0;
    for m in [middle, -middle] {
        if a < m && m < b {
            out.extend([m.next_down(), m, m.next_up()]);
        }
    }
    out
}

/// The point a fraction q of the way from a to b, inside [a, b]: the ends exactly.
fn along(a: f64, b: f64, q: f64) -> f64 {
    let s = a + (b - a) * q;
    if q <= 0.0 || s < a {
        a
    } else if q >= 1.0 || s > b {
        b
    } else {
        s
    }
}

fn flip(side: Limit) -> Limit {
    match side {
        Limit::FromLeft => Limit::FromRight,
        Limit::FromRight => Limit::FromLeft,
    }
}

fn the_transition(k: CurveKind<2>) -> Transition<2> {
    match k {
        CurveKind::Transition(t) => t,
        other => panic!("not a transition: {other:?}"),
    }
}

fn bits(x: f64) -> u64 {
    x.to_bits()
}

fn iv_bits(i: Interval) -> (u64, u64) {
    (i.lo().to_bits(), i.hi().to_bits())
}

/// The transforms the attacks run under, each with whether it keeps lengths: exact, rational and
/// certified rigid motions (which keep a transition), and a uniform and a non-uniform scale (which
/// give the NURBS form).
fn transforms() -> Vec<(TypedTransform, bool)> {
    vec![
        (
            TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
            true,
        ),
        (TypedTransform::Exact(GridTransform::mirror(Axis::Y)), true),
        (
            TypedTransform::from(
                RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
            ),
            true,
        ),
        (
            TypedTransform::from(Transform::rotation(Axis::Z, 0.37).unwrap()),
            true,
        ),
        (
            TypedTransform::from(Transform::scale(1.25, 1.25, 1.0).unwrap()),
            false,
        ),
        (
            TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
            false,
        ),
    ]
}

/// Claim: every law, whole or in part, run either way, passes every law of the kind contract under
/// every kind of transform.
#[test]
fn every_variant_passes_the_kind_contract() {
    let all = transforms();
    for (i, c) in cases().iter().enumerate() {
        let t = build(c);
        for (j, (name, v)) in variants(&t).into_iter().enumerate() {
            let (tr, _) = &all[(i + j) % all.len()];
            contract::check(&v, &params(&v), tr, &mut meter())
                .unwrap_or_else(|e| panic!("{}, {name}, under {tr:?}: {e}", c.name));
        }
    }
}

/// Claim: the reversed curve at −t is the original at t, bit for bit: points, derivatives (odd
/// orders negated), enclosures and bounds; and reversing twice gives the original back.
#[test]
fn a_reversed_curve_is_the_original_run_backwards_bit_for_bit() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        for (name, v) in variants(&t) {
            let r = v.reversed(m).unwrap();
            assert_eq!(r.deviation, 0.0);
            let r = the_transition(r.value);
            let (a, b) = v.domain();
            assert_eq!(r.domain(), (-b, -a), "{} {name}", c.name);
            assert_eq!(the_transition(r.reversed(m).unwrap().value), v);
            for s in params(&v) {
                for side in [Limit::FromLeft, Limit::FromRight] {
                    let (p, q) = (v.point(s, side).unwrap(), r.point(-s, flip(side)).unwrap());
                    assert_eq!(p.map(bits), q.map(bits), "{} {name} at {s}", c.name);
                    let (mut dv, mut dr) = ([[0.0; 2]; 6], [[0.0; 2]; 6]);
                    v.derivatives(s, side, &mut dv).unwrap();
                    r.derivatives(-s, flip(side), &mut dr).unwrap();
                    let ev = v.enclose(s, side, 5, m).unwrap();
                    let er = r.enclose(-s, flip(side), 5, m).unwrap();
                    for k in 0..6 {
                        let sign = if k % 2 == 1 { -1.0 } else { 1.0 };
                        assert_eq!(
                            dv[k].map(|x| bits(sign * x)),
                            dr[k].map(bits),
                            "{} {name}: derivative {k} at {s}",
                            c.name
                        );
                        let want = ev[k].map(|i| if k % 2 == 1 { i.neg() } else { i });
                        assert_eq!(want.map(iv_bits), er[k].map(iv_bits));
                    }
                }
            }
            for (lo, hi) in [(0.0, 1.0), (0.2, 0.45), (0.6, 0.6)] {
                let span = (along(a, b, lo), along(a, b, hi));
                let back = (-span.1, -span.0);
                assert_eq!(v.bound(span, m).unwrap(), r.bound(back, m).unwrap());
                for k in 1..4 {
                    let (DerivativeBound::Bounded(x), DerivativeBound::Bounded(y)) = (
                        v.derivative_bound(span, k, m).unwrap(),
                        r.derivative_bound(back, k, m).unwrap(),
                    ) else {
                        panic!("a transition's derivatives are bounded");
                    };
                    let want = if k % 2 == 1 {
                        Bounds {
                            lo: x.hi.map(|v| -v),
                            hi: x.lo.map(|v| -v),
                        }
                    } else {
                        x
                    };
                    assert_eq!(want, y, "{} {name}: derivative bound {k}", c.name);
                }
            }
        }
    }
}

/// The law of the curve run from the other end, worked from the curvature formulas: the reversed
/// curve at its station σ′ is the original at L − σ′, turning the other way, so κ′(σ′) = −κ(L − σ′).
/// The business laws' shapes are symmetric (κ₁ + f(ξ)·Δκ with f(1 − ξ) = 1 − f(ξ), and the
/// cant-coupled term odd about the middle), so they give the same law from −κ₂ to −κ₁. A parent
/// spiral is read from −(f + L) instead of f, the curvature negated at −s: κ′(s) = −κ(−s). None when
/// −(f + L) is not exact.
fn reversed_law(law: CurvatureLaw, length: f64) -> Option<CurvatureLaw> {
    let back = |from: f64| {
        let end = from + length;
        (end - length == from && end - from == length).then_some(-end)
    };
    Some(match law {
        CurvatureLaw::Clothoid { start, end } => CurvatureLaw::Clothoid {
            start: -end,
            end: -start,
        },
        CurvatureLaw::Helmert { start, end } => CurvatureLaw::Helmert {
            start: -end,
            end: -start,
        },
        CurvatureLaw::Bloss { start, end } => CurvatureLaw::Bloss {
            start: -end,
            end: -start,
        },
        CurvatureLaw::Cosine { start, end } => CurvatureLaw::Cosine {
            start: -end,
            end: -start,
        },
        CurvatureLaw::Sine { start, end } => CurvatureLaw::Sine {
            start: -end,
            end: -start,
        },
        CurvatureLaw::CantCoupled {
            start,
            end,
            height,
            cant,
        } => CurvatureLaw::CantCoupled {
            start: -end,
            end: -start,
            height,
            cant,
        },
        // −κ(−s) = Σ (−1)^(k+1)·sign(Aₖ)·sᵏ/|Aₖ|^(k+1): the even terms change sign.
        CurvatureLaw::Polynomial { terms, from } => {
            let mut t = terms;
            for (k, a) in t.iter_mut().enumerate() {
                if k % 2 == 0 {
                    *a = a.map(|x| -x);
                }
            }
            CurvatureLaw::Polynomial {
                terms: t,
                from: back(from)?,
            }
        }
        // −(1/A₀ + cos(−πs/Lc)/A₁) = −1/A₀ − cos(πs/Lc)/A₁.
        CurvatureLaw::CosineSpiral {
            constant,
            cosine,
            length: lc,
            from,
        } => CurvatureLaw::CosineSpiral {
            constant: constant.map(|a| -a),
            cosine: -cosine,
            length: lc,
            from: back(from)?,
        },
        // −(1/A₀ − sign(A₁)s/A₁² − sin(2πs/Ls)/A₂) = −1/A₀ + sign(A₁)s/A₁² + sin(2πs/Ls)/A₂.
        CurvatureLaw::SineSpiral {
            constant,
            linear,
            sine,
            length: ls,
            from,
        } => CurvatureLaw::SineSpiral {
            constant: constant.map(|a| -a),
            linear,
            sine,
            length: ls,
            from: back(from)?,
        },
    })
}

fn width(i: Interval) -> f64 {
    i.hi() - i.lo()
}

/// Claim: reversing a transition gives the curve run backwards: the transition built from the
/// original's end, leaving against its end tangent, with the law run backwards (worked out here,
/// not taken from the kernel), lies on the reversed curve within the rounding of that end.
#[test]
fn a_reversed_curve_is_the_transition_built_from_the_end() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let law = reversed_law(c.law, c.length)
            .unwrap_or_else(|| panic!("{}: an inexact reversed station", c.name));
        let l = c.length;
        let end = t.enclose(l, Limit::FromLeft, 1, m).unwrap();
        let (p, d) = (end[0], end[1]);
        let mid = |i: Interval| f64::midpoint(i.lo(), i.hi());
        let back = Transition::new([mid(p[0]), mid(p[1])], [-mid(d[0]), -mid(d[1])], l, law, m)
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        // The start is off by at most the end's enclosure, the direction by an angle of at most
        // twice the tangent's; a point σ′ along moves by at most σ′ times that angle.
        let shift = width(p[0]) + width(p[1]);
        let turn = 2.0 * (width(d[0]) + width(d[1]));
        let r = the_transition(t.reversed(m).unwrap().value);
        for q in 0..=8 {
            let along = l * f64::from(q) / 8.0;
            let widen = fx(shift + turn * along + 1e-300);
            let x = back.enclose(along, Limit::FromRight, 0, m).unwrap()[0];
            let y = r.enclose(along - l, Limit::FromRight, 0, m).unwrap()[0];
            for k in 0..2 {
                assert!(
                    fx(x[k].lo()) - &widen <= fx(y[k].hi())
                        && fx(y[k].lo()) <= fx(x[k].hi()) + &widen,
                    "{} at {along} along the reversed curve: {x:?} against {y:?}",
                    c.name
                );
            }
        }
    }
}

/// Claim: a split leaves two parts that evaluate as the original does at their parameters, bit for
/// bit, with deviation 0, whole or in part, run either way; a split at an end is refused.
#[test]
fn a_split_keeps_every_evaluation() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        for (name, v) in variants(&t) {
            let (a, b) = v.domain();
            let mut cuts = vec![along(a, b, 0.37), along(a, b, 0.5)];
            cuts.extend(v.params_inside_middle());
            for at in cuts {
                let (left, right) = v.split(at, m).unwrap();
                assert_eq!((left.deviation, right.deviation), (0.0, 0.0));
                let (left, right) = (the_transition(left.value), the_transition(right.value));
                assert_eq!((left.domain(), right.domain()), ((a, at), (at, b)));
                for (part, lo, hi) in [(&left, a, at), (&right, at, b)] {
                    for q in [0.0, 0.3, 1.0] {
                        let s = along(lo, hi, q);
                        for side in [Limit::FromLeft, Limit::FromRight] {
                            assert_eq!(
                                part.point(s, side).unwrap().map(bits),
                                v.point(s, side).unwrap().map(bits),
                                "{} {name} split at {at}, at {s}",
                                c.name
                            );
                            let x = part.enclose(s, side, 4, m).unwrap();
                            let y = v.enclose(s, side, 4, m).unwrap();
                            for k in 0..5 {
                                assert_eq!(x[k].map(iv_bits), y[k].map(iv_bits));
                            }
                        }
                    }
                }
            }
            assert_eq!(v.split(a, m).err(), Some(CurveError::ParameterOutside));
            assert_eq!(v.split(b, m).err(), Some(CurveError::ParameterOutside));
        }
    }
}

/// Helmert's middle as a parameter, when it is inside the domain.
trait Middle {
    fn params_inside_middle(&self) -> Vec<f64>;
}

impl Middle for Transition<2> {
    fn params_inside_middle(&self) -> Vec<f64> {
        if !matches!(self.law(), CurvatureLaw::Helmert { .. }) {
            return Vec::new();
        }
        let (a, b) = self.domain();
        let m = self.length() / 2.0;
        [m, -m].into_iter().filter(|&x| a < x && x < b).collect()
    }
}

/// Claim: the NURBS form lies within its deviation of the truth at every parameter sampled, on the
/// transition's own parameter, whole or in part and run either way; the deviation is small, and
/// its pieces meet at Helmert's middle.
#[test]
fn the_nurbs_form_is_within_its_deviation_of_the_truth() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let truth = truth(&c.law, c.length, c.start, c.unit);
        let l = c.length;
        for (name, v) in [
            ("whole", t.clone()),
            (
                "part, reversed",
                t.segment((l / 7.0, 0.8 * l), true).unwrap(),
            ),
        ] {
            let n = v.to_nurbs(m).unwrap();
            assert_eq!(n.value.map, ParamMap::Same);
            assert_eq!(ParamCurve::domain(&n.value.curve), v.domain());
            let b = v.bound(v.domain(), m).unwrap();
            let scale =
                b.lo.iter()
                    .chain(&b.hi)
                    .fold(1.0f64, |s, x| if x.abs() > s { x.abs() } else { s });
            // The conversion refines until rounding takes over: of the enclosures it starts from
            // (a few dozen steps of the coordinates and the length) and of the control points.
            assert!(
                n.deviation > 0.0 && n.deviation <= (scale + l) / 1_099_511_627_776.0,
                "{} {name}: deviation {:e} at scale {scale}",
                c.name,
                n.deviation
            );
            for k in v.params_inside_middle() {
                let knots = n.value.curve.basis().knots();
                assert!(
                    knots.contains(&k),
                    "{} {name}: no knot at Helmert's middle",
                    c.name
                );
            }
            let widen = fx(n.deviation);
            let (a, bb) = v.domain();
            for q in [0.0, 0.06, 0.31, 0.5, 0.74, 0.93, 1.0] {
                let s = along(a, bb, q);
                let exact = truth_point(&truth, v.station(s).unwrap());
                let got =
                    ParamCurve::enclose(&n.value.curve, s, Limit::FromRight, 0, m).unwrap()[0];
                for k in 0..2 {
                    assert!(
                        holds(got[k], &exact[k], &widen),
                        "{} {name} at {s}: {got:?} misses the truth by more than {:e}",
                        c.name,
                        n.deviation
                    );
                }
            }
        }
    }
}

/// Claim: the conversion doubles its pieces while each doubling more than halves the certified
/// deviation, and returns the better of the last two, as the module documentation says; and the
/// remainder-dominated steps fall far faster than half.
#[test]
fn the_conversion_stops_where_doubling_stops_halving() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let n = t.to_nurbs(m).unwrap();
        let breaks: Vec<f64> = t.params_inside_middle();
        let mut seen: Vec<Changed<geom_curve::NurbsForm<2>>> = Vec::new();
        let mut per = 1;
        let chosen = loop {
            let cuts = taylor::cuts_between(t.domain(), &breaks, per).unwrap();
            if cuts.len() > 4097 {
                break seen.last().unwrap().clone();
            }
            if let Ok(f) = taylor::from_cuts(&t, 9, &cuts, m) {
                if let Some(last) = seen.last()
                    && f.deviation >= last.deviation * 0.5
                {
                    break if f.deviation < last.deviation {
                        f
                    } else {
                        last.clone()
                    };
                }
                seen.push(f);
            }
            per *= 2;
        };
        assert_eq!(n, chosen, "{}", c.name);
        // While the remainder dominates, a doubling divides the deviation by far more than two.
        if seen.len() >= 3 {
            assert!(
                seen[1].deviation < seen[0].deviation / 16.0,
                "{}: {:e} then {:e}",
                c.name,
                seen[0].deviation,
                seen[1].deviation
            );
        }
    }
}

/// The exact image of a fixed-point point under a transform's rows, with entries in fixed point.
fn image(rows: &[[IBig; 4]; 3], p: &[IBig; 2]) -> [IBig; 2] {
    core::array::from_fn(|i| &rows[i][3] + mul(&rows[i][0], &p[0]) + mul(&rows[i][1], &p[1]))
}

/// A transform's rows in fixed point: the exact grid and rational entries, and a certified
/// transform's own doubles (exact in them).
fn rows(t: &TypedTransform) -> [[IBig; 4]; 3] {
    let rational = |r: &RationalTransform| {
        let d = IBig::from(r.denominator());
        let mm = r.matrix();
        let tt = r.translation_part();
        core::array::from_fn(|i| {
            core::array::from_fn(|j| {
                let n = if j < 3 {
                    IBig::from(mm[i][j])
                } else {
                    IBig::from(tt[i])
                };
                (n << F) / &d
            })
        })
    };
    match t {
        TypedTransform::Exact(g) => rational(&RationalTransform::from_grid(g)),
        TypedTransform::Rational(r) => rational(r),
        TypedTransform::Certified(c) => c.rows().map(|row| row.map(fx)),
        TypedTransform::Rotation(_) => panic!("not used here"),
    }
}

/// Claim: a transform that keeps lengths keeps a transition, exactly (deviation 0) for an exact one
/// on exact data, a mirror negating the law; any other gives the NURBS form; each image lies within
/// its deviation of the exact image of the truth; a transform out of the plane is refused.
#[test]
fn transforms_keep_a_transition_only_when_they_keep_lengths() {
    let m = &mut meter();
    let mut all = transforms();
    all.push((
        TypedTransform::Exact(GridTransform::translation([7, -3, 0]).unwrap()),
        true,
    ));
    let mirror = TypedTransform::Exact(GridTransform::mirror(Axis::Y));
    for c in cases() {
        let t = build(&c);
        let truth = truth(&c.law, c.length, c.start, c.unit);
        let bx = t.bound(t.domain(), m).unwrap();
        let scale = bx
            .lo
            .iter()
            .chain(&bx.hi)
            .fold(1.0f64, |s, x| if x.abs() > s { x.abs() } else { s });
        let samples: Vec<(f64, [IBig; 2])> = [0.0, 0.4, 1.0]
            .iter()
            .map(|q| (c.length * q, truth_point(&truth, c.length * q)))
            .collect();
        for (tr, rigid) in &all {
            let moved = t.transformed(tr, m).unwrap();
            match &moved.value {
                CurveKind::Transition(img) => {
                    assert!(*rigid, "{} under {tr:?} kept the kind", c.name);
                    assert_eq!(img.length(), t.length());
                    assert_eq!(img.window(), t.window());
                    let want = if *tr == mirror {
                        c.law.mirrored()
                    } else {
                        c.law
                    };
                    assert_eq!(img.law(), want, "{} under {tr:?}", c.name);
                }
                CurveKind::Nurbs(_) => assert!(!rigid, "{} under {tr:?} lost the kind", c.name),
                other => panic!("{other:?}"),
            }
            // Exact grid motions of these exact starts and directions are exact, and every rigid
            // motion's deviation stays near rounding: 2⁻³⁰ of the scale.
            if matches!(tr, TypedTransform::Exact(_)) {
                assert_eq!(moved.deviation, 0.0, "{} under {tr:?}", c.name);
            }
            if *rigid {
                assert!(moved.deviation <= (scale + c.length) / 1_073_741_824.0);
            }
            let r = rows(tr);
            let widen = fx(moved.deviation);
            for (s, p) in &samples {
                let exact = image(&r, p);
                let got = moved.value.enclose(*s, Limit::FromRight, 0, m).unwrap()[0];
                for k in 0..2 {
                    assert!(
                        holds(got[k], &exact[k], &widen),
                        "{} under {tr:?} at {s}: {got:?}",
                        c.name
                    );
                }
            }
        }
        let tilt = TypedTransform::from(Transform::rotation(Axis::X, 0.25).unwrap());
        assert_eq!(t.transformed(&tilt, m).err(), Some(CurveError::NotInPlane));
    }
}

/// Claim: a mirrored law turns the other way at every station: the turn and the curvature negated,
/// and the curve the mirror image; mirroring twice gives the law back.
#[test]
fn a_mirrored_law_turns_the_other_way() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let law = c.law.mirrored();
        assert_eq!(law.mirrored(), c.law);
        let image = Transition::new(
            [c.start[0], -c.start[1]],
            [c.direction[0], -c.direction[1]],
            c.length,
            law,
            m,
        )
        .unwrap();
        assert_eq!(image.panels(), t.panels(), "{}", c.name);
        for q in [0.0, 0.2, 0.5, 0.9, 1.0] {
            let s = c.length * q;
            for (x, y) in [
                (t.turn(s).unwrap(), image.turn(s).unwrap()),
                (t.curvature(s).unwrap(), image.curvature(s).unwrap()),
            ] {
                // −x and y enclose one value.
                assert!(-x.hi() <= y.hi() && y.lo() <= -x.lo(), "{} at {s}", c.name);
            }
            let p = t.enclose(s, Limit::FromRight, 0, m).unwrap()[0];
            let r = image.enclose(s, Limit::FromRight, 0, m).unwrap()[0];
            assert!(p[0].lo() <= r[0].hi() && r[0].lo() <= p[0].hi());
            assert!(p[1].lo() <= -r[1].lo() && -r[1].hi() <= p[1].hi());
        }
    }
}

/// Claim: a bound over any span holds the truth at every station of the span, and a derivative
/// bound the truth's derivatives, whole or in part, run either way.
#[test]
fn bounds_hold_the_truth() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let truth = truth(&c.law, c.length, c.start, c.unit);
        for (name, v) in variants(&t) {
            let (a, b) = v.domain();
            for (lo, hi) in [(0.0, 1.0), (0.1, 0.35), (0.5, 0.5)] {
                let span = (along(a, b, lo), along(a, b, hi));
                let bx = v.bound(span, m).unwrap();
                let d: Vec<Bounds<2>> = (1..4)
                    .map(|k| match v.derivative_bound(span, k, m).unwrap() {
                        DerivativeBound::Bounded(x) => x,
                        DerivativeBound::Unbounded => panic!("bounded"),
                    })
                    .collect();
                for q in [0.25, 1.0] {
                    let s = along(span.0, span.1, q);
                    let station = v.station(s).unwrap();
                    let jet = truth.jet(&fx(station));
                    for k in 0..2 {
                        let inside = |x: &Bounds<2>, v: &IBig| {
                            fx(x.lo[k]) - slack() <= *v && *v <= fx(x.hi[k]) + slack()
                        };
                        assert!(inside(&bx, &jet[0][k]), "{} {name}: bound at {s}", c.name);
                        for order in 1..4 {
                            // d/dt = −d/dσ reversed.
                            let want = if v.is_reversed() && order % 2 == 1 {
                                -jet[order][k].clone()
                            } else {
                                jet[order][k].clone()
                            };
                            assert!(
                                inside(&d[order - 1], &want),
                                "{} {name}: derivative {order} bound at {s}",
                                c.name
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Claim: at Helmert's middle the fourth derivative takes the side asked for: it jumps there by
/// −8·Δκ/L² along the normal (the curvature's second derivative jumps from 4Δκ/L² to −4Δκ/L²), and
/// a derivative bound over a span reaching the middle holds both sides.
#[test]
fn helmerts_middle_takes_its_side() {
    let m = &mut meter();
    for c in cases() {
        let CurvatureLaw::Helmert { start, end } = c.law else {
            continue;
        };
        let t = build(&c);
        let l = c.length;
        let middle = l / 2.0;
        let left = t.enclose(middle, Limit::FromLeft, 4, m).unwrap();
        let right = t.enclose(middle, Limit::FromRight, 4, m).unwrap();
        for k in 0..4 {
            for i in 0..2 {
                assert!(left[k][i].lo() <= right[k][i].hi() && right[k][i].lo() <= left[k][i].hi());
            }
        }
        let jump = -8.0 * (end - start) / (l * l);
        let tangent = right[1];
        let normal = [tangent[1].neg(), tangent[0]];
        for i in 0..2 {
            let got = right[4][i].sub(left[4][i]).unwrap();
            let want = normal[i].mul(Interval::point(jump).unwrap()).unwrap();
            let pad = 1e-9 * jump.abs();
            assert!(
                got.lo() - pad <= want.hi() && want.lo() <= got.hi() + pad,
                "{}: the jump {got:?}, against {want:?}",
                c.name
            );
        }
        let DerivativeBound::Bounded(b) = t.derivative_bound((l / 4.0, middle), 4, m).unwrap()
        else {
            panic!("bounded");
        };
        for e in [&left, &right] {
            for i in 0..2 {
                assert!(
                    b.lo[i] <= e[4][i].lo() && e[4][i].hi() <= b.hi[i],
                    "{}",
                    c.name
                );
            }
        }
        // Its middle is a break of the leaf, at −L/2 reversed, so the kinds that hold it cut there.
        assert_eq!(
            Trimmed::whole(Basis::Transition(t.clone()))
                .unwrap()
                .breaks(),
            vec![middle]
        );
        // Reversed, the sides swap.
        let r = the_transition(t.reversed(m).unwrap().value);
        assert_eq!(
            Trimmed::whole(Basis::Transition(r.clone()))
                .unwrap()
                .breaks(),
            vec![-middle]
        );
        let back = r.enclose(-middle, Limit::FromRight, 4, m).unwrap();
        assert_eq!(back[4].map(iv_bits), left[4].map(iv_bits));
    }
}

/// Claim: a transition is a leaf like the others: trimmed, joined from its own parts into a
/// composite, and offset (a corridor's edge), each passes the kind contract.
#[test]
fn a_transition_joins_the_kinds_that_hold_curves() {
    let m = &mut meter();
    let tr = TypedTransform::from(Transform::rotation(Axis::Z, 0.37).unwrap());
    for c in cases().iter().filter(|c| c.length >= 60.0) {
        let t = build(c);
        let l = c.length;
        // Only Helmert's transition has a break.
        if !matches!(c.law, CurvatureLaw::Helmert { .. }) {
            assert_eq!(
                Trimmed::whole(Basis::Transition(t.clone()))
                    .unwrap()
                    .breaks(),
                Vec::<f64>::new()
            );
        }
        let trimmed = Trimmed::new(Basis::Transition(t.clone()), (l / 5.0, 0.9 * l)).unwrap();
        let ps: Vec<f64> = (0..=6)
            .map(|i| l / 5.0 + 0.7 * l * f64::from(i) / 6.0)
            .collect();
        contract::check(&trimmed, &ps, &tr, m)
            .unwrap_or_else(|e| panic!("{} trimmed: {e}", c.name));
        let (left, right) = t.split(l / 3.0, m).unwrap();
        let joined = Composite::new(vec![
            Trimmed::whole(Basis::Transition(the_transition(left.value))).unwrap(),
            Trimmed::whole(Basis::Transition(the_transition(right.value))).unwrap(),
        ])
        .unwrap();
        let ps: Vec<f64> = (0..=6).map(|i| l * f64::from(i) / 6.0).collect();
        contract::check(&joined, &ps, &tr, m).unwrap_or_else(|e| panic!("{} joined: {e}", c.name));
        let edge = Offset::new(
            Trimmed::whole(Basis::Transition(t.clone())).unwrap(),
            OffsetLaw::Planar { distance: 3.5 },
        )
        .unwrap();
        contract::check(&edge, &ps, &tr, m).unwrap_or_else(|e| panic!("{} offset: {e}", c.name));
    }
}

/// Claim: bad data is refused with its own error: a transition outside the plane, a part outside
/// the parent, a parameter or span outside the domain.
#[test]
fn bad_parts_are_refused() {
    let m = &mut meter();
    let law = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.01,
    };
    assert_eq!(
        Transition::<3>::new([0.0; 3], [1.0, 0.0, 0.0], 10.0, law, m).err(),
        Some(CurveError::Dimension { dimension: 3 })
    );
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], 10.0, law, m).unwrap();
    for w in [
        (-1.0, 5.0),
        (2.0, 2.0),
        (3.0, 2.0),
        (0.0, 10.5),
        (f64::NAN, 1.0),
        (0.0, f64::INFINITY),
    ] {
        assert_eq!(
            t.segment(w, false).err(),
            Some(CurveError::SpanOutside),
            "{w:?}"
        );
    }
    assert_eq!((t.window(), t.is_reversed()), ((0.0, 10.0), false));
    let part = t.segment((2.0, 6.0), true).unwrap();
    assert_eq!((part.window(), part.is_reversed()), ((2.0, 6.0), true));
    assert_eq!(part.domain(), (-6.0, -2.0));
    assert_eq!(part.station(-5.0), Ok(5.0));
    for s in [-6.5, -1.0, 3.0, f64::NAN] {
        assert_eq!(
            part.point(s, Limit::FromRight),
            Err(CurveError::ParameterOutside)
        );
        assert_eq!(part.station(s), Err(CurveError::ParameterOutside));
    }
    assert_eq!(part.bound((-7.0, -3.0), m), Err(CurveError::SpanOutside));
    assert_eq!(
        part.derivative_bound((-5.0, -1.0), 2, m).err(),
        Some(CurveError::SpanOutside)
    );
    // The parent's own queries still take any station of it.
    assert!(part.turn(9.0).is_ok());
    assert_eq!(part.turn(10.5).err(), Some(CurveError::ParameterOutside));
}

/// Claim: the meter stops conversion, bounds and transforms.
#[test]
fn the_meter_stops_the_new_work() {
    let c = &cases()[0];
    let t = build(c);
    let stopped = |r: Result<(), CurveError>| matches!(r, Err(CurveError::Stopped(_)));
    let mut m = WorkMeter::new(Budget::of_work(0));
    assert!(stopped(t.to_nurbs(&mut m).map(|_| ())));
    let mut m = WorkMeter::new(Budget::of_work(0));
    assert!(stopped(t.bound(t.domain(), &mut m).map(|_| ())));
    let mut m = WorkMeter::new(Budget::of_work(0));
    assert!(stopped(
        t.derivative_bound(t.domain(), 2, &mut m).map(|_| ())
    ));
    let mut m = WorkMeter::new(Budget::of_work(0));
    let turn = TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1));
    assert!(stopped(t.transformed(&turn, &mut m).map(|_| ())));
}

/// Claim: the conversion never cuts more than `MAX_PANELS` pieces: a transition turning thousands
/// of half-turns stops at the cap with the best form it found, still within its deviation.
#[test]
fn the_conversion_stops_at_the_piece_cap() {
    let m = &mut meter();
    let law = CurvatureLaw::Clothoid {
        start: 1e4,
        end: 1e4,
    };
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], 1.0, law, m).unwrap();
    let n = t.to_nurbs(m).unwrap();
    // 4096 pieces of degree 9: 4096·9 + 1 control points, and ten more knots than that.
    assert_eq!(n.value.curve.basis().knots().len(), 4096 * 9 + 11);
    assert!(n.deviation.is_finite() && n.deviation > 0.0);
    for s in [0.0, 0.123, 0.5, 0.987, 1.0] {
        let x = t.enclose(s, Limit::FromRight, 0, m).unwrap()[0];
        let y = ParamCurve::enclose(&n.value.curve, s, Limit::FromRight, 0, m).unwrap()[0];
        for k in 0..2 {
            assert!(x[k].lo() - n.deviation <= y[k].hi() && y[k].lo() <= x[k].hi() + n.deviation);
        }
    }
}

/// Claim: a conversion whose first pieces are too wide for doubles refines past them, and one whose
/// every piece overflows is refused, never guessed. (A piece 2·10³¹ wide overflows in its Taylor
/// coefficients' powers of the width, so a straight transition 10³³ long fails up to 64 pieces.)
#[test]
fn a_conversion_refines_past_overflow_or_refuses() {
    let m = &mut meter();
    let straight = CurvatureLaw::Clothoid {
        start: 0.0,
        end: 0.0,
    };
    let t = Transition::new([0.0, 0.0], [1.0, 0.0], 1e33, straight, m).unwrap();
    let wide = taylor::cuts_between(t.domain(), &[], 64).unwrap();
    assert_eq!(
        taylor::from_cuts(&t, 9, &wide, m).err(),
        Some(CurveError::NotFinite)
    );
    let n = t.to_nurbs(m).unwrap();
    let pieces = (n.value.curve.basis().knots().len() - 11) / 9 + 1;
    assert!(pieces > 64, "{pieces} pieces");
    let x = t.enclose(5e32, Limit::FromRight, 0, m).unwrap()[0];
    let y = ParamCurve::enclose(&n.value.curve, 5e32, Limit::FromRight, 0, m).unwrap()[0];
    assert!(x[0].lo() - n.deviation <= y[0].hi() && y[0].lo() <= x[0].hi() + n.deviation);
    let far = Transition::new([f64::MAX, 0.0], [1.0, 0.0], 1e300, straight, m).unwrap();
    assert_eq!(far.to_nurbs(m).err(), Some(CurveError::NotFinite));
}

/// Claim: a transform within `ε_ang` of a rigid motion keeps a transition, and its deviation covers
/// how far it is from one: the image of the truth under the transform's own entries lies within
/// it, a mirror among them; and a rigid transform's deviation stays near rounding.
#[test]
fn a_nearly_rigid_transform_counts_its_stretch() {
    let m = &mut meter();
    // 1 + 2⁻³⁶: a stretch a quarter of `ε_ang` = 2⁻³⁴.
    let s = 1.0 + 1.0 / 68_719_476_736.0;
    let rows_of = |y: f64| {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]
    };
    let near = [
        (
            TypedTransform::from(Transform::from_rows(rows_of(s)).unwrap()),
            false,
        ),
        (
            TypedTransform::from(Transform::from_rows(rows_of(-s)).unwrap()),
            true,
        ),
        (
            TypedTransform::from(Transform::scale(1.0, -1.0, 1.0).unwrap()),
            true,
        ),
    ];
    for c in cases() {
        let t = build(&c);
        let truth = truth(&c.law, c.length, c.start, c.unit);
        let b = t.bound(t.domain(), m).unwrap();
        let scale =
            b.lo.iter()
                .chain(&b.hi)
                .fold(1.0f64, |s, x| if x.abs() > s { x.abs() } else { s });
        let samples: Vec<(f64, [IBig; 2])> = [0.3, 1.0]
            .iter()
            .map(|q| (c.length * q, truth_point(&truth, c.length * q)))
            .collect();
        for (tr, mirror) in &near {
            let moved = t.transformed(tr, m).unwrap();
            let CurveKind::Transition(img) = &moved.value else {
                panic!("{}: a nearly rigid transform keeps a transition", c.name);
            };
            let want = if *mirror { c.law.mirrored() } else { c.law };
            assert_eq!(img.law(), want);
            // 2⁻³⁰ of the scale: far above the stretch's 2⁻³⁶, far below a wrong frame.
            assert!(
                moved.deviation <= (scale + c.length) / 1_073_741_824.0,
                "{}: deviation {:e}",
                c.name,
                moved.deviation
            );
            let r = rows(tr);
            let widen = fx(moved.deviation);
            for (s, p) in &samples {
                let exact = image(&r, p);
                let got = moved.value.enclose(*s, Limit::FromRight, 0, m).unwrap()[0];
                for k in 0..2 {
                    assert!(
                        holds(got[k], &exact[k], &widen),
                        "{} under {tr:?} at {s}: {got:?}, deviation {:e}",
                        c.name,
                        moved.deviation
                    );
                }
            }
        }
    }
}

/// Claim: a winding spiral's bound hugs it: no side lies farther out than one panel's length (and
/// the spacing of the stations sampled) beyond the curve's own extent.
#[test]
fn a_winding_spirals_bound_hugs_it() {
    let m = &mut meter();
    for c in cases() {
        let t = build(&c);
        let b = t.bound(t.domain(), m).unwrap();
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for i in 0..=1024 {
            let s = along(0.0, c.length, f64::from(i) / 1024.0);
            let e = t.enclose(s, Limit::FromRight, 0, m).unwrap()[0];
            for k in 0..2 {
                if e[k].lo() < lo[k] {
                    lo[k] = e[k].lo();
                }
                if e[k].hi() > hi[k] {
                    hi[k] = e[k].hi();
                }
            }
        }
        let room = c.length / 1024.0 + 2.0 * c.length / t.panels() as f64;
        for k in 0..2 {
            assert!(
                lo[k] - room <= b.lo[k] && b.hi[k] <= hi[k] + room,
                "{}: the bound {b:?} against the extent {lo:?} to {hi:?}",
                c.name
            );
        }
    }
}
