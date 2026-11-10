//! Audit probe: every surface kind against references the kinds do not share: numerical
//! differentiation of the points, dense sampling of bounds, the NURBS form at mapped parameters,
//! and the split, reversed (in u and in v) and transformed surfaces at the same parameters. It
//! reports every disagreement it finds; the confirmed ones have their own tests in the other
//! `audit_*` files.

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

use geom_curve::{Basis as Leaf, Circle, DerivativeBound, Frame, Line, Trimmed};
use geom_math::{Axis, GridTransform, RationalTransform, Rotation, Transform, TypedTransform};
use geom_nurbs::{Basis, Bounds, Curve, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};
use geom_surface::{
    Cone, Cylinder, Extrusion, Offset, ParamBox, ParamSurface, Plane, Revolution, Ruled,
    SecondDerivativeBound, Sphere, SurfaceKind, Torus, partial_index,
};

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

struct Case {
    name: String,
    kind: SurfaceKind,
}

type V3 = [f64; 3];

fn sub(a: &V3, b: &V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: &V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn dist(a: &V3, b: &V3) -> f64 {
    norm(&sub(a, b))
}

fn cross(a: &V3, b: &V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn inside(b: &Bounds<3>, p: &V3, slack: f64) -> bool {
    b.lo.iter()
        .zip(&b.hi)
        .zip(p)
        .all(|((l, h), x)| l - slack <= *x && *x <= h + slack)
}

fn is_box(b: &Bounds<3>) -> bool {
    b.lo.iter().zip(&b.hi).all(|(l, h)| !l.is_nan() && !h.is_nan() && l <= h)
}

/// Whether some positive multiple of `n` lies in the box (within a relative slack).
fn holds_direction(b: &Bounds<3>, n: &V3) -> bool {
    let size = norm(n);
    if size == 0.0 {
        return true;
    }
    let (mut lo, mut hi) = (0.0_f64, f64::INFINITY);
    for i in 0..3 {
        let x = n[i] / size;
        let (l, h) = (b.lo[i], b.hi[i]);
        let reach = l.abs().max(h.abs());
        let reach = if reach.is_finite() { reach } else { 1.0 };
        if x.abs() < 1e-9 {
            // A component that is zero to rounding: the box must come within rounding of zero.
            if !(l <= 1e-9 * (1.0 + reach) && h >= -1e-9 * (1.0 + reach)) {
                return false;
            }
        } else if x > 0.0 {
            lo = lo.max(l / x);
            hi = hi.min(h / x);
        } else {
            lo = lo.max(h / x);
            hi = hi.min(l / x);
        }
    }
    hi > 0.0 && lo <= hi * (1.0 + 1e-9) + 1e-300
}

fn grid(((a, b), (c, d)): ParamBox, n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            out.push((
                (a + (b - a) * i as f64 / n as f64).clamp(a, b),
                (c + (d - c) * j as f64 / n as f64).clamp(c, d),
            ));
        }
    }
    out
}

fn apply(t: &TypedTransform, p: &V3) -> V3 {
    let rows = t.to_transform().unwrap().rows();
    let mut out = [0.0; 3];
    for (o, r) in out.iter_mut().zip(&rows) {
        *o = r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3];
    }
    out
}

const SIDES: (Limit, Limit) = (Limit::FromRight, Limit::FromRight);

fn probe(case: &Case, transforms: &[TypedTransform]) -> Vec<String> {
    let mut found = Vec::new();
    let k = &case.kind;
    let m = &mut meter();
    let dom = k.domain();
    let ((a, b), (c, d)) = dom;
    let (wu, wv) = (b - a, d - c);
    let (bu, bv) = k.breaks();
    let near = |x: f64, list: &[f64], h: f64| list.iter().any(|&y| (x - y).abs() <= h * 1.01);
    let mut scale: f64 = 1.0;
    for &(u, v) in &grid(dom, 8) {
        if let Ok(p) = k.point(u, v, SIDES) {
            scale = scale.max(norm(&p));
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut say = |s: String| {
        let key: String = s.chars().filter(|c| !c.is_ascii_digit() && *c != '-' && *c != '.').collect();
        if !seen.contains(&key) {
            seen.push(key);
            found.push(format!("{}: {s}", case.name));
        }
    };

    // 1. Partials to order 3 at points: doubles in enclosures, and each against the differences
    // of the order below.
    let (hu, hv) = (wu * 2e-3, wv * 2e-3);
    for &(u, v) in &grid(dom, 6) {
        let at = |uu: f64, vv: f64| -> Option<Vec<V3>> {
            k.partials_over(((uu, uu), (vv, vv)), SIDES, 3, &mut meter())
                .ok()
                .map(|p| p.iter().map(|e| e.map(|i| f64::midpoint(i.lo(), i.hi()))).collect())
        };
        let Some(here) = at(u, v) else {
            // A pole or a point where partials cannot be enclosed.
            continue;
        };
        match k.partials(u, v, SIDES) {
            Ok(p) => {
                for (i, x) in p.iter().enumerate() {
                    let tol = 1e-7 * (norm(x) + norm(&here[i]) + scale / wu.min(wv));
                    if dist(x, &here[i]) > tol {
                        say(format!("partials({u}, {v})[{i}] = {x:?} against partials_over {:?}", here[i]));
                    }
                }
            }
            Err(e) => say(format!("partials({u}, {v}) failed: {e:?}")),
        }
        match k.enclose(u, v, SIDES, m) {
            Ok(e) => {
                let p = k.partials(u, v, SIDES).unwrap();
                for (i, (x, enc)) in p.iter().zip(&e).enumerate() {
                    let slack = 1e-9 * (scale / wu.min(wv) + norm(x));
                    if !x.iter().zip(enc).all(|(x, i)| i.lo() - slack <= *x && *x <= i.hi() + slack) {
                        say(format!("enclose({u}, {v})[{i}] misses {x:?}"));
                    }
                }
            }
            Err(e) => say(format!("enclose({u}, {v}) failed: {e:?}")),
        }
        if u - 2.0 * hu < a || u + 2.0 * hu > b || v - 2.0 * hv < c || v + 2.0 * hv > d {
            continue;
        }
        if near(u, &bu, 2.0 * hu) || near(v, &bv, 2.0 * hv) {
            continue;
        }
        let stencil = |du: f64, dv: f64| -> Option<Vec<V3>> {
            let s: Vec<Vec<V3>> = [-2.0, -1.0, 1.0, 2.0]
                .iter()
                .map(|o| at(u + o * du, v + o * dv))
                .collect::<Option<_>>()?;
            let step = if du == 0.0 { dv } else { du };
            Some(
                (0..s[0].len())
                    .map(|i| {
                        let mut out = [0.0; 3];
                        for x in 0..3 {
                            out[x] = (s[0][i][x] - 8.0 * s[1][i][x] + 8.0 * s[2][i][x] - s[3][i][x])
                                / (12.0 * step);
                        }
                        out
                    })
                    .collect(),
            )
        };
        let (Some(dc_u), Some(df_u), Some(dc_v), Some(df_v)) =
            (stencil(hu, 0.0), stencil(hu / 2.0, 0.0), stencil(0.0, hv), stencil(0.0, hv / 2.0))
        else {
            continue;
        };
        for n in 1..=3_usize {
            for j in 0..=n {
                let i = n - j;
                let exact = here[partial_index(i, j).unwrap()];
                // From the order below: by u if i > 0, else by v.
                let (fine, coarse, from) = if i > 0 {
                    let idx = partial_index(i - 1, j).unwrap();
                    (df_u[idx], dc_u[idx], "u")
                } else {
                    let idx = partial_index(i, j - 1).unwrap();
                    (df_v[idx], dc_v[idx], "v")
                };
                let size = norm(&exact).max(norm(&fine));
                let w = wu.powi(i as i32) * wv.powi(j as i32);
                let tol = 1e-6 * size + 2.0 * dist(&fine, &coarse) + 1e-8 * scale / w;
                if dist(&fine, &exact) > tol {
                    say(format!(
                        "partial ({i}, {j}) at ({u}, {v}): {exact:?}, but differences in {from} give {fine:?} (and {coarse:?} at twice the step)"
                    ));
                }
            }
        }
        // The normal bound's sign at a point: the cross product of the partials.
        if let Ok(nb) = k.normal_bound(((u, u), (v, v)), m) {
            let n = cross(&here[1], &here[2]);
            if !is_box(&nb) || !holds_direction(&nb, &n) {
                say(format!("normal_bound at ({u}, {v}) = {nb:?} holds no positive multiple of {n:?}"));
            }
        }
    }

    // 2. Bounds over boxes hold dense samples.
    let mut rng = Lcg(0x5a_d172 ^ case.name.len() as u64);
    let mut boxes: Vec<ParamBox> = vec![dom, ((a, a), (c, d)), ((a, b), (d, d)), ((b, b), (c, c))];
    for &x in &bu {
        boxes.push(((x, x), (c, d)));
        boxes.push(((a, x), (c, d)));
    }
    for &y in &bv {
        boxes.push(((a, b), (y, y)));
        boxes.push(((a, b), (y, d)));
    }
    for _ in 0..16 {
        let (x0, x1, y0, y1) = (rng.range(a, b), rng.range(a, b), rng.range(c, d), rng.range(c, d));
        boxes.push(((x0.min(x1), x0.max(x1)), (y0.min(y1), y0.max(y1))));
    }
    for &bx in &boxes {
        let pts = grid(bx, 10);
        let ((x0, x1), (y0, y1)) = bx;
        match k.bound(bx, m) {
            Ok(bb) => {
                if !is_box(&bb) {
                    say(format!("bound({bx:?}) is not a box: {bb:?}"));
                } else {
                    for &(u, v) in &pts {
                        // At an end of the box, the side inside it: a surface may jump at a break
                        // (the offset of a creased surface does), and the box is of its inside.
                        let inner = |t: f64, lo: f64, hi: f64, other: Limit| {
                            if lo < hi && t == hi {
                                Limit::FromLeft
                            } else if lo < hi && t == lo {
                                Limit::FromRight
                            } else {
                                other
                            }
                        };
                        for other in [Limit::FromRight, Limit::FromLeft] {
                            let sides = (inner(u, x0, x1, other), inner(v, y0, y1, other));
                            let Ok(p) = k.point(u, v, sides) else { continue };
                            if !inside(&bb, &p, 1e-9 * scale) {
                                say(format!("bound({bx:?}) = {bb:?} misses the point {p:?} at ({u}, {v}) from {sides:?}"));
                            }
                        }
                    }
                }
            }
            Err(e) => say(format!("bound({bx:?}) failed: {e:?}")),
        }
        // Sides from inside the box at its ends.
        let side_of = |t: f64, lo: f64, hi: f64| {
            if lo < hi && t == hi { Limit::FromLeft } else { Limit::FromRight }
        };
        for (dir, idx) in [(Direction::U, 1), (Direction::V, 2)] {
            match k.derivative_bound(bx, dir, m) {
                Ok(DerivativeBound::Bounded(bb)) => {
                    if !is_box(&bb) {
                        say(format!("derivative_bound({bx:?}, {dir:?}) is not a box: {bb:?}"));
                        continue;
                    }
                    for &(u, v) in &pts {
                        let sides = (side_of(u, x0, x1), side_of(v, y0, y1));
                        let Ok(p) = k.partials(u, v, sides) else { continue };
                        let slack = 1e-9 * (norm(&p[idx]) + scale / wu.min(wv));
                        if !inside(&bb, &p[idx], slack) {
                            say(format!(
                                "derivative_bound({bx:?}, {dir:?}) = {bb:?} misses {:?} at ({u}, {v})",
                                p[idx]
                            ));
                        }
                    }
                }
                Ok(DerivativeBound::Unbounded) => {}
                Err(e) => say(format!("derivative_bound({bx:?}, {dir:?}) failed: {e:?}")),
            }
        }
        match k.normal_bound(bx, m) {
            Ok(nb) => {
                if !is_box(&nb) {
                    say(format!("normal_bound({bx:?}) is not a box: {nb:?}"));
                } else {
                    for &(u, v) in &pts {
                        let sides = (side_of(u, x0, x1), side_of(v, y0, y1));
                        let Ok(p) = k.partials(u, v, sides) else { continue };
                        let n = cross(&p[1], &p[2]);
                        // Skip poles, where the cross product is zero to rounding.
                        if norm(&n) <= 1e-9 * norm(&p[1]) * norm(&p[2]) {
                            continue;
                        }
                        if !holds_direction(&nb, &n) {
                            say(format!(
                                "normal_bound({bx:?}) = {nb:?} holds no positive multiple of {n:?} at ({u}, {v})"
                            ));
                        }
                    }
                }
            }
            Err(e) => say(format!("normal_bound({bx:?}) failed: {e:?}")),
        }
        // Partials to order 2 over the box, and the second-derivative bound from them.
        let over = k.partials_over(bx, SIDES, 2, m);
        if let Ok(over) = &over {
            for &(u, v) in &pts {
                let sides = (side_of(u, x0, x1), side_of(v, y0, y1));
                let Ok(at) = k.partials_over(((u, u), (v, v)), sides, 2, &mut meter()) else { continue };
                for (i, (e, big)) in at.iter().zip(over).enumerate() {
                    let mid: V3 = e.map(|x| f64::midpoint(x.lo(), x.hi()));
                    let slack = 1e-9 * (norm(&mid) + scale / (wu.min(wv) * wu.min(wv)));
                    if !mid.iter().zip(big).all(|(x, i)| i.lo() - slack <= *x && *x <= i.hi() + slack) {
                        say(format!("partials_over({bx:?})[{i}] misses {mid:?} at ({u}, {v})"));
                    }
                }
            }
        }
        // The unit normal's partials over the box, against differences of the unit normal.
        if let Ok([nu, nv]) = k.normal_derivative_bound(bx, SIDES, m) {
            let unit = |u: f64, v: f64| -> Option<V3> {
                let p = k.partials(u, v, SIDES).ok()?;
                let n = cross(&p[1], &p[2]);
                let len = norm(&n);
                (len > 1e-6 * norm(&p[1]) * norm(&p[2])).then(|| n.map(|x| x / len))
            };
            for &(u, v) in &pts {
                for (name, bound, du, dv) in [("u", nu, hu, 0.0), ("v", nv, 0.0, hv)] {
                    let DerivativeBound::Bounded(bb) = bound else { continue };
                    let step = if du == 0.0 { dv } else { du };
                    // Inside the box and the domain, clear of breaks.
                    let (lo, hi, t) = if du == 0.0 { (y0, y1, v) } else { (x0, x1, u) };
                    if t - 2.0 * step < lo || t + 2.0 * step > hi {
                        continue;
                    }
                    if near(u, &bu, 2.0 * du) || near(v, &bv, 2.0 * dv) {
                        continue;
                    }
                    let at = |scale: f64| -> Option<V3> {
                        let s: Vec<V3> = [-2.0, -1.0, 1.0, 2.0]
                            .iter()
                            .map(|o| unit(u + o * du * scale, v + o * dv * scale))
                            .collect::<Option<_>>()?;
                        let mut out = [0.0; 3];
                        for x in 0..3 {
                            out[x] = (s[0][x] - 8.0 * s[1][x] + 8.0 * s[2][x] - s[3][x]) / (12.0 * step * scale);
                        }
                        Some(out)
                    };
                    let (Some(coarse), Some(fine)) = (at(1.0), at(0.5)) else { continue };
                    let slack = 1e-6 * (norm(&fine) + 1.0 / step) * step + 2.0 * dist(&coarse, &fine) + 1e-7;
                    if !is_box(&bb) || !inside(&bb, &fine, slack) {
                        say(format!(
                            "normal_derivative_bound({bx:?}) in {name} = {bb:?} misses {fine:?} at ({u}, {v})"
                        ));
                    }
                }
            }
        }
        if let Ok(SecondDerivativeBound::Bounded(s)) = k.second_derivative_bound(bx, SIDES, m) {
            for &(u, v) in &pts {
                let sides = (side_of(u, x0, x1), side_of(v, y0, y1));
                let Ok(at) = k.partials_over(((u, u), (v, v)), sides, 2, &mut meter()) else { continue };
                for (name, bb, idx) in [("uu", s.uu, 3), ("uv", s.uv, 4), ("vv", s.vv, 5)] {
                    let mid: V3 = at[idx].map(|x| f64::midpoint(x.lo(), x.hi()));
                    let slack = 1e-9 * (norm(&mid) + scale / (wu.min(wv) * wu.min(wv)));
                    if !inside(&bb, &mid, slack) {
                        say(format!("second_derivative_bound({bx:?}).{name} = {bb:?} misses {mid:?} at ({u}, {v})"));
                    }
                }
            }
        }
    }

    // 3. The NURBS form at mapped parameters.
    match k.to_nurbs(m) {
        Ok(n) => {
            let s = &n.value.surface;
            let ((s0, s1), (t0, t1)) = (s.basis_u().domain(), s.basis_v().domain());
            for &(u, v) in &grid(dom, 24) {
                let Ok(p) = k.point(u, v, SIDES) else { continue };
                let (Ok(mu), Ok(mv)) = (n.value.u.enclose(u), n.value.v.enclose(v)) else {
                    say(format!("a NURBS map failed at ({u}, {v})"));
                    continue;
                };
                let at = |x: f64, y: f64| s.point(x.clamp(s0, s1), y.clamp(t0, t1), SIDES).unwrap();
                let qm = at(f64::midpoint(mu.lo(), mu.hi()), f64::midpoint(mv.lo(), mv.hi()));
                let spread = dist(&at(mu.lo(), mv.lo()), &at(mu.hi(), mv.hi()))
                    + dist(&at(mu.lo(), mv.hi()), &at(mu.hi(), mv.lo()));
                let dd = dist(&p, &qm);
                if dd > n.deviation + spread + 1e-9 * scale {
                    say(format!(
                        "the NURBS form at ({u}, {v}) is {dd:e} from the kind, deviation {:e}",
                        n.deviation
                    ));
                }
            }
        }
        Err(e) => say(format!("to_nurbs failed: {e:?}")),
    }

    // 4. Split, reversal (both directions) and transforms.
    let pts = grid(dom, 8);
    for dir in [Direction::U, Direction::V] {
        let (lo, hi) = if dir == Direction::U { (a, b) } else { (c, d) };
        let mid = lo + (hi - lo) * 0.4375;
        match k.split(dir, mid, m) {
            Ok((l, r)) => {
                for &(u, v) in &pts {
                    let t = if dir == Direction::U { u } else { v };
                    let (half, s) = if t <= mid { (&l, Limit::FromLeft) } else { (&r, Limit::FromRight) };
                    let sides = if dir == Direction::U { (s, Limit::FromRight) } else { (Limit::FromRight, s) };
                    let Ok(p) = k.partials(u, v, sides) else { continue };
                    match half.value.partials(u, v, sides) {
                        Ok(q) => {
                            if dist(&p[0], &q[0]) > half.deviation + 1e-9 * scale {
                                say(format!(
                                    "a half split in {dir:?} at ({u}, {v}) is {:e} away, deviation {:e}",
                                    dist(&p[0], &q[0]),
                                    half.deviation
                                ));
                            }
                            if half.deviation == 0.0 {
                                for i in 1..3 {
                                    if dist(&p[i], &q[i]) > 1e-6 * (norm(&p[i]) + scale / wu.min(wv)) {
                                        say(format!(
                                            "a half split in {dir:?}: partial {i} at ({u}, {v}) {:?} against {:?}",
                                            q[i], p[i]
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => say(format!("a half split in {dir:?} failed at ({u}, {v}): {e:?}")),
                    }
                }
            }
            Err(e) => say(format!("split in {dir:?} failed: {e:?}")),
        }
        match k.reversed(dir, m) {
            Ok(r) => {
                let want = if dir == Direction::U { ((-b, -a), (c, d)) } else { ((a, b), (-d, -c)) };
                if r.value.domain() != want {
                    say(format!("reversed in {dir:?}: domain {:?}", r.value.domain()));
                }
                for &(u, v) in &pts {
                    let Ok(p) = k.partials(u, v, SIDES) else { continue };
                    let (ru, rv, sides) = if dir == Direction::U {
                        (-u, v, (Limit::FromLeft, Limit::FromRight))
                    } else {
                        (u, -v, (Limit::FromRight, Limit::FromLeft))
                    };
                    match r.value.partials(ru, rv, sides) {
                        Ok(q) => {
                            if dist(&p[0], &q[0]) > r.deviation + 1e-9 * scale {
                                say(format!(
                                    "reversed in {dir:?} at ({ru}, {rv}): {:e} away, deviation {:e}",
                                    dist(&p[0], &q[0]),
                                    r.deviation
                                ));
                            }
                            if r.deviation == 0.0 {
                                let (wantu, wantv) = if dir == Direction::U {
                                    (p[1].map(|x| -x), p[2])
                                } else {
                                    (p[1], p[2].map(|x| -x))
                                };
                                let tol = 1e-6 * (norm(&p[1]) + norm(&p[2]) + scale / wu.min(wv));
                                if dist(&wantu, &q[1]) > tol || dist(&wantv, &q[2]) > tol {
                                    say(format!(
                                        "reversed in {dir:?} at ({ru}, {rv}): partials {:?}, {:?} against {wantu:?}, {wantv:?}",
                                        q[1], q[2]
                                    ));
                                }
                            }
                        }
                        Err(e) => say(format!("reversed in {dir:?} failed at ({ru}, {rv}): {e:?}")),
                    }
                }
            }
            Err(e) => say(format!("reversed in {dir:?} failed: {e:?}")),
        }
    }
    for (i, tr) in transforms.iter().enumerate() {
        match k.transformed(tr, m) {
            Ok(moved) => {
                let rows = tr.to_transform().unwrap().rows();
                let stretch = rows.iter().flat_map(|r| r.iter().take(3)).map(|x| x * x).sum::<f64>().sqrt();
                for &(u, v) in &pts {
                    let Ok(p) = k.point(u, v, SIDES) else { continue };
                    let image = apply(tr, &p);
                    match moved.value.point(u, v, SIDES) {
                        Ok(q) => {
                            let dd = dist(&image, &q);
                            if dd > moved.deviation + 1e-9 * (scale * (1.0 + stretch) + norm(&q)) {
                                say(format!(
                                    "transform {i} at ({u}, {v}): {dd:e} from the image, deviation {:e}",
                                    moved.deviation
                                ));
                            }
                        }
                        Err(e) => say(format!("transform {i}: the moved surface failed at ({u}, {v}): {e:?}")),
                    }
                }
                // The moved surface's normal bound against the moved partials.
                if let Ok(nb) = moved.value.normal_bound(moved.value.domain(), m) {
                    for &(u, v) in &pts {
                        let Ok(q) = moved.value.partials(u, v, SIDES) else { continue };
                        let n = cross(&q[1], &q[2]);
                        if norm(&n) <= 1e-9 * norm(&q[1]) * norm(&q[2]) {
                            continue;
                        }
                        if is_box(&nb) && !holds_direction(&nb, &n) {
                            say(format!(
                                "transform {i}: normal_bound {nb:?} holds no positive multiple of {n:?} at ({u}, {v})"
                            ));
                        }
                    }
                }
            }
            Err(e) => say(format!("transform {i} failed: {e:?}")),
        }
    }
    found
}

fn transforms() -> Vec<TypedTransform> {
    vec![
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::X, 1)),
        TypedTransform::Exact(GridTransform::mirror(Axis::Y)),
        TypedTransform::from(
            RationalTransform::rotation(Axis::Y, Rotation::direction(3, 4).unwrap()).unwrap(),
        ),
        TypedTransform::from(Transform::rotation(Axis::Z, 0.3).unwrap()),
        TypedTransform::from(Transform::scale(2.0, 0.5, 3.0).unwrap()),
        TypedTransform::from(Transform::scale(1.5, 1.5, 1.5).unwrap()),
        TypedTransform::from(Transform::scale(-2.0, 2.0, 2.0).unwrap()),
        TypedTransform::from(Transform::scale(1.0, 1.0, -1.0).unwrap()),
    ]
}

fn frame(rng: &mut Lcg) -> Frame<3> {
    loop {
        let x = [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)];
        let n = [rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)];
        if let Ok(f) = Frame::new(x, n) {
            return f;
        }
    }
}

fn line(a: V3, d: V3) -> Trimmed<3> {
    Trimmed::whole(Leaf::Line(Line::new(a, d, (0.0, 1.0)).unwrap())).unwrap()
}

fn arc(c: V3, r: f64, x: V3, n: V3, span: (f64, f64)) -> Trimmed<3> {
    let circle = Circle::new(c, r, Frame::new(x, n).unwrap(), span).unwrap();
    Trimmed::whole(Leaf::Circle(circle)).unwrap()
}

fn spline(rational: bool) -> Trimmed<3> {
    let basis = Basis::new(3, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]).unwrap();
    let pts = [
        [2.0, 0.0, 0.0],
        [3.0, 0.5, 1.0],
        [2.5, -0.5, 2.0],
        [4.0, 0.0, 3.0],
        [3.0, 1.0, 4.0],
    ];
    let w = [1.0, 2.0, 1.0, 0.5, 1.0];
    let c = if rational {
        Curve::new_rational(basis, &pts, &w).unwrap()
    } else {
        Curve::new(basis, &pts).unwrap()
    };
    Trimmed::whole(Leaf::Nurbs(c)).unwrap()
}

fn patch(rng: &mut Lcg, rational: bool) -> Surface<3> {
    let (p, q) = (1 + rng.below(3), 1 + rng.below(3));
    let knots = |deg: usize, rng: &mut Lcg| {
        let mut k = vec![0.0; deg + 1];
        let mut x = 0.0;
        for _ in 0..rng.below(3) {
            x += rng.range(0.3, 1.0);
            k.extend(std::iter::repeat_n(x, 1 + rng.below(deg)));
        }
        k.extend(std::iter::repeat_n(x + 1.0, deg + 1));
        Basis::new(deg, k).unwrap()
    };
    let (bu, bv) = (knots(p, rng), knots(q, rng));
    let (nu, nv) = (bu.count(), bv.count());
    let mut pts = Vec::new();
    let mut w = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            pts.push([
                i as f64 * 2.0 + rng.range(-0.4, 0.4),
                j as f64 * 2.0 + rng.range(-0.4, 0.4),
                rng.range(-1.5, 1.5),
            ]);
            w.push(rng.range(0.4, 2.5));
        }
    }
    if rational {
        Surface::new_rational(bu, bv, &pts, &w).unwrap()
    } else {
        Surface::new(bu, bv, &pts).unwrap()
    }
}

fn cases() -> Vec<Case> {
    let mut rng = Lcg(0x5a_0d17);
    let mut out = Vec::new();
    let mut push = |name: &str, kind: SurfaceKind| out.push(Case { name: name.to_string(), kind });
    for i in 0..3 {
        let f = frame(&mut rng);
        let c = [rng.range(-5.0, 5.0), rng.range(-5.0, 5.0), rng.range(-5.0, 5.0)];
        let du = [(0.0, 2.0), (-0.75, 0.6), (3.25, 3.5)][i];
        let dv = [(-1.0, 2.0), (0.5, 3.0), (-4.0, -3.5)][i];
        push(
            &format!("plane {i}"),
            SurfaceKind::Plane(
                Plane::new(c, [1.0, 0.5, rng.range(-1.0, 1.0)], [rng.range(-1.0, 1.0), 2.0, 0.25], (du, dv))
                    .unwrap(),
            ),
        );
        push(
            &format!("cylinder {i}"),
            SurfaceKind::Cylinder(Cylinder::new(c, rng.range(0.5, 3.0), f, (du, dv)).unwrap()),
        );
        // Cones: one clear of its apex, one through it, one with a negative slope.
        let (r0, slope) = [(2.0, 0.5), (0.5, -0.75), (3.0, -0.25)][i];
        push(
            &format!("cone {i}"),
            SurfaceKind::Cone(Cone::new(c, r0, slope, f, (du, dv)).unwrap()),
        );
        let lat = [(-0.5, 0.5), (-0.25, 0.4), (0.1, 0.5)][i];
        push(
            &format!("sphere {i}"),
            SurfaceKind::Sphere(Sphere::new(c, rng.range(0.5, 3.0), f, (du, lat)).unwrap()),
        );
        let tv = [(0.0, 2.0), (-0.6, 0.7), (0.9, 1.1)][i];
        push(
            &format!("ring torus {i}"),
            SurfaceKind::Torus(Torus::new(c, 3.0, rng.range(0.5, 2.0), f, (du, tv)).unwrap()),
        );
        // Spindles: the apple, the lemon, and a domain across both.
        let sv = [(-0.5, 0.5), (0.95, 1.05), (0.3, 1.0)][i];
        push(
            &format!("spindle torus {i}"),
            SurfaceKind::Torus(Torus::new(c, 1.0, 2.5, f, (du, sv)).unwrap()),
        );
        for rational in [false, true] {
            push(
                &format!("nurbs {i} rational {rational}"),
                SurfaceKind::Nurbs(patch(&mut rng, rational)),
            );
        }
    }
    let axis = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let tilted = Frame::new([1.0, 0.5, 0.0], [0.25, -0.5, 1.0]).unwrap();
    push(
        "revolution of a line",
        SurfaceKind::Revolution(
            Revolution::new(line([2.0, 0.0, 0.0], [1.0, 0.0, 2.0]), [0.0, 0.0, 0.0], axis, (0.0, 2.0)).unwrap(),
        ),
    );
    push(
        "revolution of an arc about a tilted axis",
        SurfaceKind::Revolution(
            Revolution::new(
                arc([3.0, 0.0, 0.0], 1.0, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], (0.1, 0.9)),
                [0.5, -1.0, 0.25],
                tilted,
                (-0.4, 1.1),
            )
            .unwrap(),
        ),
    );
    for rational in [false, true] {
        push(
            &format!("revolution of a spline rational {rational}"),
            SurfaceKind::Revolution(
                Revolution::new(spline(rational), [0.0, 0.0, 1.0], axis, (0.25, 1.75)).unwrap(),
            ),
        );
        push(
            &format!("extrusion of a spline rational {rational}"),
            SurfaceKind::Extrusion(Extrusion::new(spline(rational), [0.5, -1.0, 0.25], (-1.0, 2.5)).unwrap()),
        );
    }
    push(
        "extrusion of an arc",
        SurfaceKind::Extrusion(
            Extrusion::new(arc([0.0, 0.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 1.5)), [0.0, 0.25, 1.0], (0.0, 3.0))
                .unwrap(),
        ),
    );
    push(
        "ruled between lines",
        SurfaceKind::Ruled(
            Ruled::new(line([0.0, 0.0, 0.0], [4.0, 0.0, 0.0]), line([0.0, 3.0, 1.0], [4.0, 1.0, 2.0]), (0.0, 1.0)).unwrap(),
        ),
    );
    push(
        "ruled between an arc and a line",
        SurfaceKind::Ruled(
            Ruled::new(
                arc([0.0, 0.0, 0.0], 2.0, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (0.0, 1.0)),
                line([2.0, 0.0, 3.0], [-4.0, 0.5, 0.0]),
                (-0.5, 1.5),
            )
            .unwrap(),
        ),
    );
    // Offsets.
    let f = Frame::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let base_cyl = SurfaceKind::Cylinder(Cylinder::new([0.0; 3], 2.0, f, ((0.0, 1.5), (0.0, 3.0))).unwrap());
    let base_torus = SurfaceKind::Torus(Torus::new([1.0, 2.0, 3.0], 4.0, 1.0, f, ((0.0, 1.0), (-0.5, 0.75))).unwrap());
    for (i, d) in [0.5, -0.75, 3.5].into_iter().enumerate() {
        push(&format!("offset cylinder {i}"), SurfaceKind::Offset(Offset::new(base_cyl.clone(), d).unwrap()));
        push(&format!("offset torus {i}"), SurfaceKind::Offset(Offset::new(base_torus.clone(), d * 0.5).unwrap()));
        push(
            &format!("offset nurbs {i}"),
            SurfaceKind::Offset(Offset::new(SurfaceKind::Nurbs(patch(&mut rng, i == 1)), d * 0.05).unwrap()),
        );
    }
    let twice = Offset::new(SurfaceKind::Offset(Offset::new(base_torus, 0.25).unwrap()), -0.25).unwrap();
    push("offset of an offset torus", SurfaceKind::Offset(twice));
    out
}

fn run(filter: impl Fn(&str) -> bool) {
    let mut all = Vec::new();
    for case in cases() {
        if filter(&case.name) {
            let start = std::time::Instant::now();
            all.extend(probe(&case, &transforms()));
            eprintln!("{}: {:?}", case.name, start.elapsed());
        }
    }
    assert!(all.is_empty(), "
{}", all.join("
"));
}

fn is_swept(n: &str) -> bool {
    n.starts_with("revolution") || n.starts_with("extrusion") || n.starts_with("ruled")
}

#[test]
fn probe_analytic_kinds() {
    run(|n| !n.starts_with("offset") && !n.starts_with("nurbs") && !is_swept(n));
}

#[test]
fn probe_nurbs_kinds() {
    run(|n| n.starts_with("nurbs"));
}

#[test]
fn probe_swept_kinds() {
    run(is_swept);
}

#[test]
fn probe_offsets_of_analytic_kinds() {
    run(|n| n.starts_with("offset cylinder") || n.starts_with("offset torus"));
}

#[test]
fn probe_offsets_of_nurbs() {
    run(|n| n.starts_with("offset nurbs"));
}

#[test]
fn probe_an_offset_of_an_offset() {
    run(|n| n.starts_with("offset of an offset"));
}
