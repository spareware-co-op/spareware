//! Audit probe: every curve kind against references the kinds do not share: numerical
//! differentiation of the points, dense sampling of bounds, the NURBS form at mapped parameters,
//! and the split, reversed and transformed curves at the same parameters. It reports every
//! disagreement it finds; the confirmed ones have their own tests in the other `audit_*` files.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "tests: native arithmetic and indexing, the formulas' names, and a failure stops the test"
)]

use geom_curve::{
    Axes, Basis, BulgePolyline, Circle, Composite, CurvatureLaw, CurveError, CurveKind, DerivativeBound,
    Ellipse, Frame, Helix, HelixAxes, Hyperbola, Involute, Line, Offset, OffsetLaw, Parabola,
    ParamCurve, Polynomial, Station, Transition, Trimmed,
};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::{Basis as Knots, Curve, Limit};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) % n as u64) as usize
    }
}

struct Case<const D: usize> {
    name: String,
    kind: CurveKind<D>,
    /// Parameters where the kind may not be smooth.
    breaks: Vec<f64>,
    /// Whether a refusal (an error) is expected of this case: only wrong values count.
    lenient: bool,
}

fn dist<const D: usize>(a: &[f64; D], b: &[f64; D]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

fn norm<const D: usize>(a: &[f64; D]) -> f64 {
    a.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn samples((a, b): (f64, f64), n: usize) -> Vec<f64> {
    (0..=n)
        .map(|i| (a + (b - a) * i as f64 / n as f64).clamp(a, b))
        .collect()
}

/// The image of a point under a typed transform, in doubles.
fn apply<const D: usize>(t: &TypedTransform, p: &[f64; D]) -> [f64; D] {
    let rows = t.to_transform().unwrap().rows();
    let mut x = [0.0; 3];
    for (o, v) in x.iter_mut().zip(p) {
        *o = *v;
    }
    let mut out = [0.0; D];
    for (o, r) in out.iter_mut().zip(&rows) {
        *o = r[0] * x[0] + r[1] * x[1] + r[2] * x[2] + r[3];
    }
    out
}

/// Runs every probe on one case and returns what it found.
fn probe<const D: usize>(case: &Case<D>, transforms: &[TypedTransform]) -> Vec<String> {
    let mut found = Vec::new();
    let k = &case.kind;
    let m = &mut meter();
    let (a, b) = k.domain();
    let width = b - a;
    let ts = samples((a, b), 48);
    let near_break = |t: f64, h: f64| case.breaks.iter().any(|&x| (x - t).abs() <= h * 1.01);
    let mut scale: f64 = 1.0;
    for &t in &ts {
        if let Ok(p) = k.point(t, Limit::FromRight) {
            scale = scale.max(norm(&p));
        }
    }
    // Doubles of the extreme cases lose digits honestly (their enclosures say so): compared to
    // 1e-6 of the curve's size, the others to 1e-9.
    let rel = if case.lenient { 1e-6 } else { 1e-9 };
    let mut seen: Vec<String> = Vec::new();
    let mut say = |s: String| {
        // The NURBS kind refuses its certified methods on unclamped knots: `audit_unclamped.rs`.
        // A lenient case may refuse; a `Defect` is never a fair refusal (it marks a bug).
        if s.contains("NotClamped") || (case.lenient && s.contains("failed") && !s.contains("Defect")) {
            return;
        }
        // One example of each sort of disagreement per case: the message with its numbers out.
        let key: String = s.chars().filter(|c| !c.is_ascii_digit() && *c != '-' && *c != '.').collect();
        if !seen.contains(&key) {
            seen.push(key);
            found.push(format!("{}: {s}", case.name));
        }
    };

    // 1. Doubles inside enclosures, to order 4; derivatives against central differences.
    let h = width * 2e-3;
    for &t in &ts {
        let mut d = [[0.0; D]; 5];
        let Ok(()) = k.derivatives(t, Limit::FromRight, &mut d) else {
            say(format!("derivatives({t}) failed: {:?}", k.derivatives(t, Limit::FromRight, &mut d)));
            continue;
        };
        if !d.iter().flatten().all(|x| x.is_finite()) {
            say(format!("derivatives({t}) returned a value that is not finite: {d:?}"));
            continue;
        }
        match k.enclose(t, Limit::FromRight, 4, m) {
            Ok(e) => {
                let largest = e
                    .iter()
                    .flatten()
                    .fold(0.0_f64, |s, i| s.max(i.lo().abs()).max(i.hi().abs()));
                for (order, (v, enc)) in d.iter().zip(&e).enumerate() {
                    for (x, i) in v.iter().zip(enc) {
                        let slack = 1e-9 * (1.0 + largest);
                        if !(i.lo() - slack <= *x && *x <= i.hi() + slack) {
                            say(format!(
                                "order {order} at {t}: double {x} outside enclosure [{}, {}]",
                                i.lo(),
                                i.hi()
                            ));
                        }
                    }
                }
            }
            Err(e) => say(format!("enclose({t}) failed: {e:?}")),
        }
        if t - 2.0 * h < a || t + 2.0 * h > b || near_break(t, 2.0 * h) {
            continue;
        }
        // Differences need steps the doubles resolve (a circle a long way round its parameter
        // does not give them) and a curve the steps resolve (a bulge of thousands runs nearly all
        // its circle inside a sliver of its parameter).
        if h < t.abs() * 1e-9 || case.name.starts_with("extreme bulge") {
            continue;
        }
        // A fourth-order central difference of each derivative gives the next one; taken at two
        // steps, the difference of the two says how far the finer one may be off.
        let numeric = |step: f64| -> Option<[[f64; D]; 5]> {
            let mut stencil = [[[0.0; D]; 4]; 4];
            for (slot, off) in stencil.iter_mut().zip([-2.0, -1.0, 1.0, 2.0]) {
                k.derivatives(t + off * step, Limit::FromRight, slot).ok()?;
            }
            let mut out = [[0.0; D]; 5];
            for order in 1..=4 {
                for i in 0..D {
                    out[order][i] = (stencil[0][order - 1][i] - 8.0 * stencil[1][order - 1][i]
                        + 8.0 * stencil[2][order - 1][i]
                        - stencil[3][order - 1][i])
                        / (12.0 * step);
                }
            }
            Some(out)
        };
        let (Some(coarse), Some(fine)) = (numeric(h), numeric(h / 2.0)) else {
            continue;
        };
        for order in 1..=4 {
            let size = norm(&d[order]).max(norm(&fine[order])).max(norm(&d[order - 1]) / width);
            let tol = 1e-6 * size
                + 2.0 * dist(&coarse[order], &fine[order])
                + 1e-8 * scale / width.powi(order as i32);
            if dist(&fine[order], &d[order]) > tol {
                say(format!(
                    "derivative {order} at {t}: {:?}, but differences of order {} give {:?} (and {:?} at twice the step)",
                    d[order],
                    order - 1,
                    fine[order],
                    coarse[order]
                ));
                break;
            }
        }
    }

    // 2. Bounds hold dense samples, over many spans.
    let mut rng = Lcg(0xa0d1_7000 ^ case.name.len() as u64);
    let mut spans = vec![(a, b), (a, a), (b, b)];
    // A span that is one break is left to `audit_spans.rs`, which found the kinds disagree there.
    for &x in &case.breaks {
        if a < x && x < b {
            spans.push((a, x));
            spans.push((x, b));
        }
    }
    for _ in 0..24 {
        let (x, y) = (rng.range(a, b), rng.range(a, b));
        spans.push((x.min(y), x.max(y)));
    }
    for &(lo, hi) in &spans {
        let inside = samples((lo, hi), 64);
        match k.bound((lo, hi), m) {
            Ok(bx) => {
                if !bx.lo.iter().zip(&bx.hi).all(|(l, h)| l.is_finite() && h.is_finite() && l <= h)
                {
                    say(format!("bound(({lo}, {hi})) is not a box: {bx:?}"));
                    continue;
                }
                for &t in &inside {
                    for side in [Limit::FromRight, Limit::FromLeft] {
                        let Ok(p) = k.point(t, side) else { continue };
                        let slack = 1e-9 * scale;
                        if !p
                            .iter()
                            .zip(bx.lo.iter().zip(&bx.hi))
                            .all(|(x, (l, h))| l - slack <= *x && *x <= h + slack)
                        {
                            say(format!(
                                "bound(({lo}, {hi})) = {bx:?} misses the point {p:?} at {t}"
                            ));
                        }
                    }
                }
            }
            Err(e) => say(format!("bound(({lo}, {hi})) failed: {e:?}")),
        }
        for order in 1..=3 {
            match k.derivative_bound((lo, hi), order, m) {
                Ok(DerivativeBound::Bounded(bx)) => {
                    if !bx
                        .lo
                        .iter()
                        .zip(&bx.hi)
                        .all(|(l, h)| l.is_finite() && h.is_finite() && l <= h)
                    {
                        say(format!(
                            "derivative_bound(({lo}, {hi}), {order}) is not a box: {bx:?}"
                        ));
                        continue;
                    }
                    for &t in &inside {
                        // Inside the span from both sides; at its ends from inside it (and from
                        // both sides of a degenerate span, as the NURBS kind states).
                        let sides: &[Limit] = if lo == hi {
                            &[Limit::FromRight, Limit::FromLeft]
                        } else if t == lo {
                            &[Limit::FromRight]
                        } else if t == hi {
                            &[Limit::FromLeft]
                        } else {
                            &[Limit::FromRight, Limit::FromLeft]
                        };
                        for &side in sides {
                            let mut d = [[0.0; D]; 4];
                            if k.derivatives(t, side, &mut d).is_err() {
                                continue;
                            }
                            let v = d[order];
                            let slack = 1e-9 * (norm(&v) + scale / width.powi(order as i32));
                            if !v
                                .iter()
                                .zip(bx.lo.iter().zip(&bx.hi))
                                .all(|(x, (l, h))| l - slack <= *x && *x <= h + slack)
                            {
                                say(format!(
                                    "derivative_bound(({lo}, {hi}), {order}) = {bx:?} misses {v:?} at {t} from {side:?}"
                                ));
                            }
                        }
                    }
                }
                Ok(DerivativeBound::Unbounded) => {}
                Err(e) => say(format!("derivative_bound(({lo}, {hi}), {order}) failed: {e:?}")),
            }
        }
    }

    // 3. The NURBS form at mapped parameters.
    match k.to_nurbs(m) {
        Ok(n) => {
            let c = &n.value.curve;
            let (c0, c1) = c.basis().domain();
            for &t in &samples((a, b), 200) {
                let Ok(p) = k.point(t, Limit::FromRight) else { continue };
                let u = match n.value.map.enclose(t) {
                    Ok(u) => u,
                    Err(e) => {
                        say(format!("the NURBS map failed at {t}: {e:?}"));
                        continue;
                    }
                };
                let at = |x: f64| c.point(x.clamp(c0, c1), Limit::FromRight).unwrap();
                let (q0, q1, qm) = (at(u.lo()), at(u.hi()), at(f64::midpoint(u.lo(), u.hi())));
                let d = dist(&p, &qm);
                let slack = dist(&q0, &q1) + 1e-9 * scale;
                if d > n.deviation + slack {
                    say(format!(
                        "the NURBS form at {t} is {d:e} from the kind, deviation {:e}",
                        n.deviation
                    ));
                }
            }
        }
        Err(e) => say(format!("to_nurbs failed: {e:?}")),
    }

    // 4. Split, reversal and transforms at the same parameters, with derivatives.
    let mid = a + width * 0.4375;
    match k.split(mid, m) {
        Ok((l, r)) => {
            for &t in &ts {
                let (half, side) = if t <= mid {
                    (&l, Limit::FromLeft)
                } else {
                    (&r, Limit::FromRight)
                };
                let (mut x, mut y) = ([[0.0; D]; 3], [[0.0; D]; 3]);
                if k.derivatives(t, side, &mut x).is_err() {
                    continue;
                }
                if let Err(e) = half.value.derivatives(t, side, &mut y) {
                    say(format!("a split half failed at {t}: {e:?}"));
                    continue;
                }
                if dist(&x[0], &y[0]) > half.deviation + rel * scale {
                    say(format!(
                        "a split half at {t} is {:e} away, deviation {:e}",
                        dist(&x[0], &y[0]),
                        half.deviation
                    ));
                }
                if half.deviation == 0.0 && !near_break(t, 0.0) {
                    for order in 1..3 {
                        if dist(&x[order], &y[order]) > 1e-6 * (norm(&x[order]) + scale / width) {
                            say(format!(
                                "a split half's derivative {order} at {t}: {:?} against {:?}",
                                y[order], x[order]
                            ));
                        }
                    }
                }
            }
        }
        Err(e) => say(format!("split failed: {e:?}")),
    }
    match k.reversed(m) {
        Ok(r) => {
            if r.value.domain() != (-b, -a) {
                say(format!("the reversed domain is {:?}", r.value.domain()));
            }
            for &t in &ts {
                let (mut x, mut y) = ([[0.0; D]; 3], [[0.0; D]; 3]);
                if k.derivatives(t, Limit::FromRight, &mut x).is_err() {
                    continue;
                }
                if let Err(e) = r.value.derivatives(-t, Limit::FromLeft, &mut y) {
                    say(format!("the reversed curve failed at {}: {e:?}", -t));
                    continue;
                }
                if dist(&x[0], &y[0]) > r.deviation + rel * scale {
                    say(format!(
                        "the reversed curve at {} is {:e} away, deviation {:e}",
                        -t,
                        dist(&x[0], &y[0]),
                        r.deviation
                    ));
                }
                if r.deviation == 0.0 {
                    let flipped = x[1].map(|v| -v);
                    if dist(&flipped, &y[1]) > 1e-6 * (norm(&x[1]) + scale / width) {
                        say(format!(
                            "the reversed curve's tangent at {}: {:?} against {:?}",
                            -t, y[1], flipped
                        ));
                    }
                    if dist(&x[2], &y[2]) > 1e-6 * (norm(&x[2]) + scale / (width * width)) {
                        say(format!(
                            "the reversed curve's second derivative at {}: {:?} against {:?}",
                            -t, y[2], x[2]
                        ));
                    }
                }
            }
        }
        Err(e) => say(format!("reversed failed: {e:?}")),
    }
    for (i, tr) in transforms.iter().enumerate() {
        match k.transformed(tr, m) {
            Ok(moved) => {
                let stretch = {
                    let rows = tr.to_transform().unwrap().rows();
                    rows.iter()
                        .flat_map(|r| r.iter().take(3))
                        .map(|x| x * x)
                        .sum::<f64>()
                        .sqrt()
                };
                for &t in &ts {
                    let Ok(p) = k.point(t, Limit::FromRight) else { continue };
                    let image = apply(tr, &p);
                    match moved.value.point(t, Limit::FromRight) {
                        Ok(q) => {
                            let d = dist(&image, &q);
                            if d > moved.deviation + rel * (scale * (1.0 + stretch) + norm(&q)) {
                                say(format!(
                                    "transform {i} at {t}: {d:e} from the image, deviation {:e}",
                                    moved.deviation
                                ));
                            }
                        }
                        Err(e) => say(format!("transform {i}: the moved curve failed at {t}: {e:?}")),
                    }
                }
            }
            Err(e) => say(format!("transform {i} failed: {e:?}")),
        }
    }
    found
}

fn planar() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Z, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::X)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Z, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(1.5, 1.5, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(-1.0, 1.0, 1.0).unwrap()),
        TypedTransform::from(Transform::scale(-2.5, 2.5, 1.0).unwrap()),
    ]
}

fn spatial() -> Vec<TypedTransform> {
    let mut t = planar();
    t.extend([
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Z)),
        TypedTransform::from(Transform::rotation(Axis::Y, 0.71).unwrap()),
        TypedTransform::from(Transform::scale(1.5, 0.75, 3.0).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 2.0, -2.0).unwrap()),
    ]);
    t
}

fn frame2(rng: &mut Lcg) -> Frame<2> {
    let x = [rng.range(-2.0, 2.0), rng.range(0.5, 2.0)];
    let n = if rng.below(2) == 0 { 1.0 } else { -1.0 };
    Frame::new(x, [0.0, 0.0, n * rng.range(0.5, 3.0)]).unwrap()
}

fn frame3(rng: &mut Lcg) -> Frame<3> {
    loop {
        let x = [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)];
        let n = [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)];
        if let Ok(f) = Frame::new(x, n) {
            return f;
        }
    }
}

fn nurbs<const D: usize>(rng: &mut Lcg, rational: bool, clamped: bool) -> (Curve<D>, Vec<f64>) {
    let p = 1 + rng.below(4);
    let mut k: Vec<f64> = Vec::new();
    if clamped {
        k.extend(std::iter::repeat_n(0.0, p + 1));
    } else {
        for i in 0..=p {
            k.push(-1.0 + i as f64 * 0.2);
        }
        k.push(0.0);
    }
    let mut x = 0.0;
    for _ in 0..(1 + rng.below(4)) {
        x += rng.range(0.3, 1.5);
        let mult = 1 + rng.below(p);
        k.extend(std::iter::repeat_n(x, mult));
    }
    x += 1.0;
    if clamped {
        k.extend(std::iter::repeat_n(x, p + 1));
    } else {
        for i in 0..=p {
            k.push(x + i as f64 * 0.3);
        }
    }
    let basis = Knots::new(p, k.clone()).unwrap();
    let pts: Vec<[f64; D]> = (0..basis.count())
        .map(|_| {
            let mut q = [0.0; D];
            for v in &mut q {
                *v = rng.range(-9.0, 9.0);
            }
            q
        })
        .collect();
    let curve = if rational {
        let w: Vec<f64> = (0..basis.count()).map(|_| rng.range(0.2, 3.0)).collect();
        Curve::new_rational(basis, &pts, &w).unwrap()
    } else {
        Curve::new(basis, &pts).unwrap()
    };
    (curve, k)
}

fn laws() -> Vec<(CurvatureLaw, f64)> {
    vec![
        (CurvatureLaw::Clothoid { start: 0.0, end: 0.01 }, 80.0),
        (CurvatureLaw::Clothoid { start: -0.02, end: 0.03 }, 120.0),
        (CurvatureLaw::Helmert { start: 0.0, end: 0.004 }, 150.0),
        (CurvatureLaw::Helmert { start: 0.01, end: -0.02 }, 90.0),
        (CurvatureLaw::Bloss { start: 0.0, end: 0.005 }, 100.0),
        (CurvatureLaw::Cosine { start: 0.002, end: -0.008 }, 110.0),
        (CurvatureLaw::Sine { start: 0.0, end: 0.01 }, 70.0),
        (
            CurvatureLaw::CantCoupled { start: 0.0, end: 0.003, height: 1.8, cant: 0.1 },
            140.0,
        ),
        (
            CurvatureLaw::Polynomial {
                terms: [None, Some(120.0), None, None, None, None, None, None],
                from: 10.0,
            },
            90.0,
        ),
        (
            CurvatureLaw::Polynomial {
                terms: [Some(-500.0), Some(200.0), Some(-150.0), Some(140.0), None, None, None, Some(130.0)],
                from: 0.0,
            },
            60.0,
        ),
        (
            CurvatureLaw::CosineSpiral { constant: Some(400.0), cosine: -300.0, length: 90.0, from: 5.0 },
            80.0,
        ),
        (
            CurvatureLaw::SineSpiral {
                constant: Some(-800.0),
                linear: Some(150.0),
                sine: 250.0,
                length: 75.0,
                from: 3.0,
            },
            70.0,
        ),
    ]
}

fn cases2() -> Vec<Case<2>> {
    let mut rng = Lcg(0x00a0_d172);
    let mut out: Vec<Case<2>> = Vec::new();
    let mut push = |name: &str, kind: CurveKind<2>, breaks: Vec<f64>| {
        out.push(Case { name: name.to_string(), kind, breaks, lenient: false });
    };
    for i in 0..4 {
        let f = frame2(&mut rng);
        let c = [rng.range(-5.0, 5.0), rng.range(-5.0, 5.0)];
        let dom = [(0.0, 2.0), (-0.75, 0.6), (-3.25, -1.5), (0.1, 0.35)][i];
        push(
            &format!("circle {i}"),
            CurveKind::Circle(Circle::new(c, rng.range(0.5, 4.0), f, dom).unwrap()),
            vec![],
        );
        push(
            &format!("ellipse frame {i}"),
            CurveKind::Ellipse(
                Ellipse::new(c, Axes::Frame { frame: f, a: rng.range(0.5, 4.0), b: rng.range(0.5, 4.0) }, dom)
                    .unwrap(),
            ),
            vec![],
        );
        push(
            &format!("ellipse vectors {i}"),
            CurveKind::Ellipse(
                Ellipse::new(
                    c,
                    Axes::Vectors { u: [rng.range(1.0, 3.0), rng.range(-1.0, 1.0)], v: [rng.range(-1.0, 1.0), rng.range(1.0, 3.0)] },
                    dom,
                )
                .unwrap(),
            ),
            vec![],
        );
        let hd = [(-1.5, 2.0), (0.0, 3.0), (-4.0, -1.0), (-0.01, 0.02)][i];
        push(
            &format!("hyperbola {i}"),
            CurveKind::Hyperbola(
                Hyperbola::new(c, Axes::Frame { frame: f, a: rng.range(0.5, 2.0), b: rng.range(0.5, 2.0) }, hd)
                    .unwrap(),
            ),
            vec![],
        );
        push(
            &format!("parabola {i}"),
            CurveKind::Parabola(Parabola::focal(c, rng.range(0.2, 2.0), f, hd).unwrap()),
            vec![],
        );
        let id = [(-0.5, 1.5), (0.25, 3.0), (-2.0, 0.0), (0.0, 0.5)][i];
        push(
            &format!("involute {i}"),
            CurveKind::Involute(Involute::new(c, rng.range(0.5, 2.0), f, id).unwrap()),
            vec![],
        );
        push(
            &format!("line {i}"),
            CurveKind::Line(
                Line::new(c, [rng.range(-2.0, 2.0), rng.range(0.5, 2.0)], hd).unwrap(),
            ),
            vec![],
        );
        push(
            &format!("polynomial {i}"),
            CurveKind::Polynomial(
                Polynomial::new(
                    (0..(2 + i)).map(|_| [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)]).collect(),
                    hd,
                )
                .unwrap(),
            ),
            vec![],
        );
        for (rational, clamped) in [(false, true), (true, true), (false, false), (true, false)] {
            let (c, k) = nurbs::<2>(&mut rng, rational, clamped);
            push(&format!("nurbs {i} rational {rational} clamped {clamped}"), CurveKind::Nurbs(c), k);
        }
    }
    // Bulge polylines.
    for (i, bulges) in [
        vec![0.0, 0.3, -1.0],
        vec![1.0, 2.5, -0.577],
        vec![-4.0, 0.0, 0.5, 7.0],
        vec![0.2],
    ]
    .into_iter()
    .enumerate()
    {
        for closed in [false, true] {
            let n = if closed { bulges.len() } else { bulges.len() + 1 };
            if n < 2 {
                continue;
            }
            let vertices: Vec<[f64; 2]> = (0..n)
                .map(|j| {
                    let ang = std::f64::consts::TAU * j as f64 / n as f64;
                    [10.0 * ang.cos() + rng.range(-1.0, 1.0), 10.0 * ang.sin() + rng.range(-1.0, 1.0)]
                })
                .collect();
            let p = BulgePolyline::new(vertices, bulges.clone(), None, closed).unwrap();
            let breaks = (0..=bulges.len()).map(|j| j as f64).collect();
            push(&format!("bulge {i} closed {closed}"), CurveKind::Bulge(p), breaks);
        }
    }
    // Trimmed, composite and offset curves.
    let arc = Circle::new([0.0, 0.0], 5.0, Frame::standard().unwrap(), (0.0, 1.0)).unwrap();
    let seg = Line::new([-5.0, 0.0], [0.0, -1.0], (0.0, 4.0)).unwrap();
    let para = Parabola::new(
        [-5.0, -4.0],
        Axes::Vectors { u: [1.0, 0.5], v: [2.0, -1.0] },
        (0.0, 1.5),
    )
    .unwrap();
    push(
        "trimmed arc",
        CurveKind::Trimmed(Trimmed::new(Basis::Circle(arc.clone()), (0.125, 0.875)).unwrap()),
        vec![],
    );
    let comp = Composite::new(vec![
        Trimmed::whole(Basis::Circle(arc.clone())).unwrap(),
        Trimmed::whole(Basis::Line(seg.clone())).unwrap(),
        Trimmed::whole(Basis::Parabola(para)).unwrap(),
    ])
    .unwrap();
    let starts = comp.starts().to_vec();
    push("composite", CurveKind::Composite(comp), starts);
    let comp2 = Composite::starting_at(
        vec![
            Trimmed::new(Basis::Circle(arc.clone()), (0.25, 1.0)).unwrap(),
            Trimmed::new(Basis::Line(seg), (0.0, 0.3)).unwrap(),
        ],
        -7.3,
    )
    .unwrap();
    let starts = comp2.starts().to_vec();
    push("composite shifted", CurveKind::Composite(comp2), starts);
    for (i, d) in [1.0, -2.0, 4.5, -7.0].into_iter().enumerate() {
        push(
            &format!("offset arc {i}"),
            CurveKind::Offset(
                Offset::new(
                    Trimmed::whole(Basis::Circle(arc.clone())).unwrap(),
                    OffsetLaw::Planar { distance: d },
                )
                .unwrap(),
            ),
            vec![],
        );
        let (c, k) = nurbs::<2>(&mut rng, i % 2 == 1, true);
        let p = c.degree();
        // An offset needs a tangent everywhere: keep only smooth curves.
        if p >= 2 && k.windows(p).all(|w| w[0] != w[p - 1] || w[0] == k[0] || w[0] == k[k.len() - 1]) {
            if let Ok(o) = Offset::new(
                Trimmed::whole(Basis::Nurbs(c)).unwrap(),
                OffsetLaw::Planar { distance: d * 0.01 },
            ) {
                push(&format!("offset nurbs {i}"), CurveKind::Offset(o), k);
            }
        }
        let e = Ellipse::new(
            [1.0, 2.0],
            Axes::Frame { frame: Frame::standard().unwrap(), a: 6.0, b: 3.0 },
            (0.0, 1.25),
        )
        .unwrap();
        push(
            &format!("offset ellipse by distances {i}"),
            CurveKind::Offset(
                Offset::new(
                    Trimmed::whole(Basis::Ellipse(e)).unwrap(),
                    OffsetLaw::Distances(vec![
                        Station { at: 0.25, lateral: d * 0.1, vertical: 0.0 },
                        Station { at: 0.5, lateral: -d * 0.2, vertical: 0.0 },
                        Station { at: 1.0, lateral: d * 0.3, vertical: 0.0 },
                    ]),
                )
                .unwrap(),
            ),
            vec![0.25, 0.5, 1.0],
        );
    }
    // Kinds built on a bulge polyline, a NURBS curve trimmed a hair inside its ends, and a NURBS
    // curve that jumps at a knot of full multiplicity.
    let poly = BulgePolyline::new(
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 5.0]],
        vec![0.4, -1.5, 0.0],
        None,
        false,
    )
    .unwrap();
    push(
        "trimmed bulge",
        CurveKind::Trimmed(Trimmed::new(Basis::Bulge(poly.clone()), (0.5, 2.0)).unwrap()),
        vec![1.0],
    );
    let after = Line::new([4.0, 3.0], [1.0, 2.0], (0.0, 1.5)).unwrap();
    let mixed = Composite::new(vec![
        Trimmed::new(Basis::Bulge(poly), (0.5, 2.0)).unwrap(),
        Trimmed::whole(Basis::Line(after)).unwrap(),
    ])
    .unwrap();
    let starts = mixed.starts().to_vec();
    push("composite of a bulge and a line", CurveKind::Composite(mixed), vec![0.5, starts[1]]);
    for rational in [false, true] {
        let (c, k) = nurbs::<2>(&mut rng, rational, true);
        let (a, b) = c.basis().domain();
        let w = b - a;
        push(
            &format!("nurbs trimmed a hair inside its ends, rational {rational}"),
            CurveKind::Trimmed(Trimmed::new(Basis::Nurbs(c), (a + w * 1e-9, b - w * 1e-9)).unwrap()),
            k,
        );
    }
    let jump = Curve::<2>::new(
        Knots::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.5, 2.5, 2.5]).unwrap(),
        &[[0.0, 0.0], [1.0, 1.0], [2.0, 0.0], [5.0, 5.0], [6.0, 6.0], [7.0, 5.0]],
    )
    .unwrap();
    push("nurbs with a jump", CurveKind::Nurbs(jump), vec![1.0]);
    // Transitions.
    for (i, (law, length)) in laws().into_iter().enumerate() {
        let t = Transition::<2>::new([3.0, -2.0], [0.6, 0.8], length, law, &mut meter()).unwrap();
        let breaks = if matches!(law, CurvatureLaw::Helmert { .. }) {
            vec![length / 2.0, -length / 2.0]
        } else {
            vec![]
        };
        push(&format!("transition {i}"), CurveKind::Transition(t.clone()), breaks.clone());
        let part = t.segment((length * 0.2, length * 0.9), true).unwrap();
        push(&format!("transition {i} part reversed"), CurveKind::Transition(part), breaks);
    }
    out
}

fn cases3() -> Vec<Case<3>> {
    let mut rng = Lcg(0x00a0_d173);
    let mut out: Vec<Case<3>> = Vec::new();
    let mut push = |name: &str, kind: CurveKind<3>, breaks: Vec<f64>| {
        out.push(Case { name: name.to_string(), kind, breaks, lenient: false });
    };
    for i in 0..4 {
        let f = frame3(&mut rng);
        let c = [rng.range(-5.0, 5.0), rng.range(-5.0, 5.0), rng.range(-5.0, 5.0)];
        let dom = [(0.0, 2.0), (-0.75, 0.6), (-3.25, -1.5), (0.1, 0.35)][i];
        push(
            &format!("circle {i}"),
            CurveKind::Circle(Circle::new(c, rng.range(0.5, 4.0), f, dom).unwrap()),
            vec![],
        );
        push(
            &format!("ellipse {i}"),
            CurveKind::Ellipse(
                Ellipse::new(c, Axes::Frame { frame: f, a: rng.range(0.5, 4.0), b: rng.range(0.5, 4.0) }, dom)
                    .unwrap(),
            ),
            vec![],
        );
        let hd = [(-1.5, 2.0), (0.0, 3.0), (-4.0, -1.0), (-0.01, 0.02)][i];
        push(
            &format!("hyperbola {i}"),
            CurveKind::Hyperbola(
                Hyperbola::new(
                    c,
                    Axes::Vectors { u: [1.0, 0.25, -0.5], v: [rng.range(-1.0, 1.0), 2.0, 0.5] },
                    hd,
                )
                .unwrap(),
            ),
            vec![],
        );
        push(
            &format!("parabola {i}"),
            CurveKind::Parabola(Parabola::focal(c, rng.range(0.2, 2.0), f, hd).unwrap()),
            vec![],
        );
        let helix_dom = [(0.0, 6.5), (-3.0, 1.0), (10.0, 10.75), (-0.3, 0.2)][i];
        push(
            &format!("helix frame {i}"),
            CurveKind::Helix(
                Helix::new(
                    c,
                    HelixAxes::Frame { frame: f, radius: rng.range(0.5, 3.0), pitch: rng.range(-3.0, 3.0) + 0.01 },
                    helix_dom,
                )
                .unwrap(),
            ),
            vec![],
        );
        push(
            &format!("helix vectors {i}"),
            CurveKind::Helix(
                Helix::new(
                    c,
                    HelixAxes::Vectors { u: [2.0, 0.5, 0.0], v: [-0.5, 1.0, 0.25], w: [0.1, rng.range(-1.0, 1.0), 0.75] },
                    helix_dom,
                )
                .unwrap(),
            ),
            vec![],
        );
        let id = [(-0.5, 1.5), (0.25, 3.0), (-2.0, 0.0), (0.0, 0.5)][i];
        push(
            &format!("involute {i}"),
            CurveKind::Involute(Involute::new(c, rng.range(0.5, 2.0), f, id).unwrap()),
            vec![],
        );
        push(
            &format!("polynomial {i}"),
            CurveKind::Polynomial(
                Polynomial::new(
                    (0..(2 + i))
                        .map(|_| [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)])
                        .collect(),
                    hd,
                )
                .unwrap(),
            ),
            vec![],
        );
        for (rational, clamped) in [(false, true), (true, true), (false, false), (true, false)] {
            let (c, k) = nurbs::<3>(&mut rng, rational, clamped);
            push(&format!("nurbs {i} rational {rational} clamped {clamped}"), CurveKind::Nurbs(c), k);
        }
        let h = Helix::new(
            [0.0, 0.0, 0.0],
            HelixAxes::Frame { frame: Frame::standard().unwrap(), radius: 3.0, pitch: 2.0 },
            (0.0, 3.0),
        )
        .unwrap();
        let d = [1.0, -0.5, 2.5, -4.0][i];
        push(
            &format!("offset helix by reference {i}"),
            CurveKind::Offset(
                Offset::new(
                    Trimmed::whole(Basis::Helix(h.clone())).unwrap(),
                    OffsetLaw::Reference { distance: d, direction: [0.0, 0.0, 1.0] },
                )
                .unwrap(),
            ),
            vec![],
        );
        push(
            &format!("offset helix by distances {i}"),
            CurveKind::Offset(
                Offset::new(
                    Trimmed::whole(Basis::Helix(h)).unwrap(),
                    OffsetLaw::Distances(vec![
                        Station { at: 0.5, lateral: d * 0.1, vertical: 0.3 },
                        Station { at: 2.0, lateral: -d * 0.2, vertical: -0.4 },
                    ]),
                )
                .unwrap(),
            ),
            vec![0.5, 2.0],
        );
    }
    out
}

/// Kinds at the edges of what their constructors take: tiny and huge sizes, far parameters,
/// flat and sharp shapes. A refusal is fine here; a wrong value is not.
fn extreme2() -> Vec<Case<2>> {
    let mut out: Vec<Case<2>> = Vec::new();
    let mut push = |name: &str, kind: Result<CurveKind<2>, CurveError>, breaks: Vec<f64>| {
        if let Ok(kind) = kind {
            out.push(Case { name: format!("extreme {name}"), kind, breaks, lenient: true });
        }
    };
    let std2 = Frame::<2>::standard().unwrap();
    for (i, (r, c)) in [(1e-300, 0.0), (1e300, 0.0), (1.0, 1e300), (1e-160, 1e-160), (3.0, 1e16), (1e150, -1e150)]
        .into_iter()
        .enumerate()
    {
        push(&format!("circle {i}"), Circle::new([c, -c], r, std2, (0.0, 2.0)).map(CurveKind::Circle), vec![]);
        push(
            &format!("ellipse {i}"),
            Ellipse::new([c, c], Axes::Frame { frame: std2, a: r, b: r * 0.5 }, (-0.3, 1.2)).map(CurveKind::Ellipse),
            vec![],
        );
        push(
            &format!("hyperbola {i}"),
            Hyperbola::new([c, c], Axes::Frame { frame: std2, a: r, b: r * 2.0 }, (-3.0, 3.0)).map(CurveKind::Hyperbola),
            vec![],
        );
        push(&format!("parabola {i}"), Parabola::focal([c, c], r, std2, (-2.0, 5.0)).map(CurveKind::Parabola), vec![]);
        push(&format!("involute {i}"), Involute::new([c, c], r, std2, (-1.0, 2.5)).map(CurveKind::Involute), vec![]);
        push(&format!("line {i}"), Line::new([c, c], [r, -r], (-2.0, 3.0)).map(CurveKind::Line), vec![]);
        push(
            &format!("offset circle {i}"),
            Circle::new([c, -c], r, std2, (0.0, 1.5))
                .and_then(|a| Offset::new(Trimmed::whole(Basis::Circle(a))?, OffsetLaw::Planar { distance: r * 0.25 }))
                .map(CurveKind::Offset),
            vec![],
        );
    }
    // Far parameters: a turn's worth of circle a long way round, a hyperbola at its reach.
    for (i, a) in [1e9, 4.0e15, -1e12, 3.0e15].into_iter().enumerate() {
        push(&format!("far circle {i}"), Circle::new([1.0, 2.0], 3.0, std2, (a, a + 1.5)).map(CurveKind::Circle), vec![]);
        push(
            &format!("far involute {i}"),
            Involute::new([1.0, 2.0], 3.0, std2, (a * 1e-7, a * 1e-7 + 1.5)).map(CurveKind::Involute),
            vec![],
        );
    }
    for (i, d) in [(-512.0, -500.0), (500.0, 512.0), (-512.0, 512.0), (0.0, 1e-9), (-1e-12, 1e-12)].into_iter().enumerate() {
        push(
            &format!("reaching hyperbola {i}"),
            Hyperbola::new([1.0, 2.0], Axes::Frame { frame: std2, a: 2.0, b: 3.0 }, d).map(CurveKind::Hyperbola),
            vec![],
        );
    }
    // Thin and skewed conjugate axes.
    for (i, e) in [1e-9, 1e-200, 1e9].into_iter().enumerate() {
        push(
            &format!("thin ellipse {i}"),
            Ellipse::new([0.0, 0.0], Axes::Vectors { u: [1.0, 0.0], v: [1.0, e] }, (0.0, 2.0)).map(CurveKind::Ellipse),
            vec![],
        );
    }
    // Offsets at and past the centre of curvature, and of nothing.
    let arc = Circle::new([0.0, 0.0], 5.0, std2, (0.0, 1.0)).unwrap();
    for (i, d) in [5.0, 5.0 + 1e-12, 50.0, 0.0, -0.0, 1e-300, -1e300].into_iter().enumerate() {
        push(
            &format!("offset to the centre {i}"),
            Offset::new(Trimmed::whole(Basis::Circle(arc.clone())).unwrap(), OffsetLaw::Planar { distance: d })
                .map(CurveKind::Offset),
            vec![],
        );
    }
    // Bulges from nearly straight to nearly a full circle, and segments of very different sizes.
    for (i, b) in [1e-300, 1e-9, 0.999_999_999, 1.0, 1e3, 1e8, -1e150].into_iter().enumerate() {
        push(
            &format!("bulge {i}"),
            BulgePolyline::new(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 1e-6], [1e6, 1e6]], vec![b, -b, b * 0.5], None, false)
                .map(CurveKind::Bulge),
            vec![0.0, 1.0, 2.0, 3.0],
        );
    }
    // Polynomials of the highest degree, with large coefficients.
    let coefficients: Vec<[f64; 2]> = (0..=28).map(|j| [1e10 / (1.0 + j as f64), (-1.0_f64).powi(j) * 1e-10]).collect();
    push("polynomial 28", Polynomial::new(coefficients, (-1.5, 1.5)).map(CurveKind::Polynomial), vec![]);
    // NURBS: weights from tiny to huge, coincident control points, a degree-28 arch.
    let cubic = |pts: &[[f64; 2]], w: Option<&[f64]>| {
        let basis = Knots::new(3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]).unwrap();
        match w {
            Some(w) => Curve::new_rational(basis, pts, w),
            None => Curve::new(basis, pts),
        }
        .map(CurveKind::Nurbs)
        .map_err(CurveError::from)
    };
    let p5 = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, -1.0], [6.0, 0.0]];
    let knots5 = vec![0.0, 1.0, 2.0];
    push("nurbs weights 1e-300 to 1e300", cubic(&p5, Some(&[1e-300, 1.0, 1e300, 1.0, 1e-300])), knots5.clone());
    push("nurbs weights 1e-150", cubic(&p5, Some(&[1e-150, 2e-150, 1e-150, 3e-150, 1e-150])), knots5.clone());
    push("nurbs one heavy weight", cubic(&p5, Some(&[1.0, 1.0, 1e12, 1.0, 1.0])), knots5.clone());
    push("nurbs one light weight", cubic(&p5, Some(&[1.0, 1.0, 1e-12, 1.0, 1.0])), knots5.clone());
    push("nurbs coincident points", cubic(&[[1.0, 1.0]; 5], None), knots5.clone());
    push(
        "nurbs coincident ends",
        cubic(&[[0.0, 0.0], [0.0, 0.0], [3.0, 2.0], [6.0, 0.0], [6.0, 0.0]], None),
        knots5.clone(),
    );
    push(
        "nurbs huge points",
        cubic(&[[1e300, 0.0], [-1e300, 2.0], [3e299, 2e300], [4.0, -1e300], [6e300, 0.0]], None),
        knots5.clone(),
    );
    push(
        "nurbs tiny points",
        cubic(&[[1e-300, 0.0], [-1e-300, 2e-300], [3e-299, 2e-300], [4e-300, -1e-300], [6e-300, 0.0]], None),
        knots5,
    );
    let arch: Vec<[f64; 2]> = (0..=28).map(|j| [j as f64, ((j * j) % 7) as f64]).collect();
    let mut k28 = vec![0.0; 29];
    k28.extend(vec![1.0; 29]);
    push(
        "nurbs degree 28",
        Knots::new(28, k28).and_then(|b| Curve::new(b, &arch)).map(CurveKind::Nurbs).map_err(CurveError::from),
        vec![],
    );
    // Close knots: spans of very different lengths.
    let close = Knots::new(2, vec![0.0, 0.0, 0.0, 1e-300, 1.0, 1.0 + 1e-15, 2.0, 2.0, 2.0]);
    push(
        "nurbs close knots",
        close
            .and_then(|b| Curve::new(b, &[[0.0, 0.0], [1.0, 1.0], [2.0, 0.0], [3.0, 1.0], [4.0, 0.0], [5.0, 1.0]]))
            .map(CurveKind::Nurbs)
            .map_err(CurveError::from),
        vec![1e-300, 1.0, 1.0 + 1e-15],
    );
    // Transitions: short, long, sharply curved and nearly straight.
    for (i, (law, length)) in [
        (CurvatureLaw::Clothoid { start: 0.0, end: 1e-12 }, 1e-3),
        (CurvatureLaw::Clothoid { start: 0.0, end: 50.0 }, 2.0),
        (CurvatureLaw::Clothoid { start: -1e-9, end: 1e-9 }, 1e7),
        (CurvatureLaw::Bloss { start: 5.0, end: -5.0 }, 10.0),
        (CurvatureLaw::Sine { start: 0.0, end: 1e-300 }, 1e-100),
        (CurvatureLaw::Helmert { start: 0.0, end: 3.0 }, 4.0),
    ]
    .into_iter()
    .enumerate()
    {
        let breaks = if matches!(law, CurvatureLaw::Helmert { .. }) { vec![length / 2.0] } else { vec![] };
        push(
            &format!("transition {i}"),
            Transition::<2>::new([1e6, -1e6], [-3.0, 4.0], length, law, &mut meter()).map(CurveKind::Transition),
            breaks,
        );
    }
    out
}

fn extreme3() -> Vec<Case<3>> {
    let mut out: Vec<Case<3>> = Vec::new();
    let mut push = |name: &str, kind: Result<CurveKind<3>, CurveError>, breaks: Vec<f64>| {
        if let Ok(kind) = kind {
            out.push(Case { name: format!("extreme {name}"), kind, breaks, lenient: true });
        }
    };
    // Frames from vectors of very different sizes and nearly parallel ones.
    let frames = [
        Frame::<3>::new([1e-100, 0.0, 0.0], [0.0, 0.0, 1e100]),
        Frame::<3>::new([1.0, 1e-9, 0.0], [1.0, 0.0, 0.0]),
        Frame::<3>::new([1e40, 2e40, -1e40], [3e-40, 1e-40, 2e-40]),
        Frame::<3>::new([0.6, 0.8, 0.0], [0.0, 0.0, -5.0]),
    ];
    for (i, f) in frames.into_iter().enumerate() {
        let Ok(f) = f else { continue };
        for (j, r) in [1e-200, 1.0, 1e200].into_iter().enumerate() {
            push(&format!("circle {i} {j}"), Circle::new([r, -r, r], r, f, (-0.5, 1.5)).map(CurveKind::Circle), vec![]);
            push(
                &format!("helix {i} {j}"),
                Helix::new([0.0, r, 0.0], HelixAxes::Frame { frame: f, radius: r, pitch: -r * 1e-6 }, (-2.25, 3.5))
                    .map(CurveKind::Helix),
                vec![],
            );
            push(
                &format!("flat helix offset {i} {j}"),
                Helix::new([0.0, 0.0, 0.0], HelixAxes::Frame { frame: f, radius: r, pitch: r * 1e-9 }, (0.0, 1.0))
                    .and_then(|h| {
                        Offset::new(
                            Trimmed::whole(Basis::Helix(h))?,
                            OffsetLaw::Reference { distance: r * 0.5, direction: f.normal() },
                        )
                    })
                    .map(CurveKind::Offset),
                vec![],
            );
        }
    }
    // A long helix: many turns.
    let std3 = Frame::<3>::standard().unwrap();
    push(
        "long helix",
        Helix::new([0.0; 3], HelixAxes::Frame { frame: std3, radius: 2.0, pitch: 0.5 }, (-40.0, 60.0)).map(CurveKind::Helix),
        vec![],
    );
    // An offset whose reference direction is nearly along the tangent somewhere.
    let line = Line::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], (0.0, 4.0)).unwrap();
    for (i, v) in [[1.0, 1e-9, 0.0], [1.0, 1e-300, 0.0], [0.0, 1e-300, 0.0], [1e300, 1e300, 1e300]].into_iter().enumerate() {
        push(
            &format!("offset line reference {i}"),
            Offset::new(
                Trimmed::whole(Basis::Line(line.clone())).unwrap(),
                OffsetLaw::Reference { distance: 1.0, direction: v },
            )
            .map(CurveKind::Offset),
            vec![],
        );
    }
    out
}

#[test]
fn probe_planar_kinds() {
    let mut all = Vec::new();
    for case in cases2() {
        all.extend(probe(&case, &planar()));
    }
    assert!(all.is_empty(), "\n{}", all.join("\n"));
}

#[test]
fn probe_spatial_kinds() {
    let mut all = Vec::new();
    for case in cases3() {
        all.extend(probe(&case, &spatial()));
    }
    assert!(all.is_empty(), "\n{}", all.join("\n"));
}

#[test]
fn probe_extreme_planar_kinds() {
    let mut all = Vec::new();
    for case in extreme2() {
        all.extend(probe(&case, &planar()));
    }
    assert!(all.is_empty(), "\n{}", all.join("\n"));
}

#[test]
fn probe_extreme_spatial_kinds() {
    let mut all = Vec::new();
    for case in extreme3() {
        all.extend(probe(&case, &spatial()));
    }
    assert!(all.is_empty(), "\n{}", all.join("\n"));
}
