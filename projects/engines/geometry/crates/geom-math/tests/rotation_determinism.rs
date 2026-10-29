//! The replay hash of piece G46: rational rotations over a fixed corpus, hashed with SHA-256, must
//! give the same digest on every target (chapters 21 and 28.13; the golden store of piece G19).
//!
//! The corpus, from a fixed linear congruential sequence: 2D searches for directions of every
//! octant, axis and size at seven bounds, 3D searches for quaternion targets of every size at five
//! bounds, compositions that stay exact and compositions re-approximated at three bounds, images
//! of grid points at the frame's edges, the matrices, inverses, directions, rational and
//! floating-point forms of every result, the quaternions read back from rational transforms, and
//! typed transforms composed across every variant. Every result, success or error, is written in
//! a fixed little-endian encoding, the angles as their bits, so a target that finds one other
//! fraction, rounds one other component or certifies one bit differently changes the digest. The
//! digest lives in the store; a change is a bug, or a new definition with a golden of its own
//! (chapter 26.2).

#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "the corpus generator; the rules are for kernel code"
)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use geom_math::{
    Axis, F2, GridTransform, MathError, Pythagorean, RationalRotation, RationalTransform, Rotation,
    Rounded, Scale, Transform, TypedTransform, WithinAngle,
};
use geom_num::grid::{P2, P3};
use geom_num::{NumError, Real};
use geom_test::golden::{self, hex, sha256};

/// The corpus's golden in the golden store.
const GOLDEN: &str = "geom-math/rotations";

/// Knuth's MMIX linear congruential sequence.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// A double in [−1, 1), from 53 bits by exact operations.
    fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }

    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 16) % n
    }
}

/// 2^k for |k| ≤ 1000, by exact doubling and halving.
fn pow2(k: i32) -> f64 {
    let mut v = 1.0;
    for _ in 0..k.unsigned_abs() {
        v = if k > 0 { v * 2.0 } else { v * 0.5 };
    }
    v
}

#[derive(Default)]
struct Recorder {
    bytes: Vec<u8>,
    calls: u32,
    reapproximated: u32,
}

impl Recorder {
    fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }

    fn i64(&mut self, v: i64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    fn i128(&mut self, v: i128) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    fn f64(&mut self, v: f64) {
        assert!(!v.is_nan(), "a NaN result");
        self.bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    fn error(&mut self, e: MathError) {
        assert_ne!(
            e,
            MathError::Num(NumError::BudgetExceeded),
            "a defect was reported"
        );
        self.u8(0xEE);
        let code = match e {
            MathError::Num(NumError::OutOfRange { value }) => {
                self.i128(value);
                1
            }
            MathError::Num(NumError::Degenerate) => 2,
            MathError::Num(NumError::NotFinite) => 3,
            MathError::Num(NumError::ToleranceExceeded) => 4,
            MathError::NotInPlane => 5,
            other => panic!("an undocumented error {other:?}"),
        };
        self.u8(code);
    }

    fn rotation(&mut self, r: &RationalRotation) {
        for v in r.quaternion() {
            self.i64(v);
        }
    }

    fn within(&mut self, r: Result<WithinAngle<RationalRotation>, MathError>) {
        self.calls += 1;
        match r {
            Ok(w) => {
                self.u8(1);
                self.rotation(&w.value());
                self.f64(w.angle().get());
                self.details(&w.value());
            }
            Err(e) => self.error(e),
        }
    }

    fn pythagorean(&mut self, p: &Pythagorean) {
        self.i128(p.a());
        self.i128(p.b());
        self.i128(p.c());
    }

    fn rational(&mut self, t: &RationalTransform) {
        for row in t.matrix() {
            for v in row {
                self.i64(v);
            }
        }
        for v in t.translation_part() {
            self.i128(v);
        }
        self.i64(t.denominator());
    }

    fn transform(&mut self, t: &Transform) {
        for row in t.rows() {
            for v in row {
                self.f64(v);
            }
        }
    }

    fn typed(&mut self, r: Result<TypedTransform, MathError>) {
        self.calls += 1;
        match r {
            Ok(TypedTransform::Exact(g)) => {
                self.u8(0);
                self.rational(&RationalTransform::from_grid(&g));
            }
            Ok(TypedTransform::Rational(t)) => {
                self.u8(1);
                self.rational(&t);
            }
            Ok(TypedTransform::Rotation(r)) => {
                self.u8(2);
                self.rotation(&r);
            }
            Ok(TypedTransform::Certified(t)) => {
                self.u8(3);
                self.transform(&t);
            }
            Err(e) => self.error(e),
        }
    }

    /// Everything readable from a rotation: |q|², M(q), the inverse, the direction, the rational
    /// and floating-point forms and the quaternion read back.
    fn details(&mut self, r: &RationalRotation) {
        self.i128(r.norm2() as i128);
        for row in r.matrix().unwrap() {
            for v in row {
                self.i128(v);
            }
        }
        self.rotation(&r.inverse());
        match r.direction() {
            Some(p) => {
                self.u8(1);
                self.pythagorean(&p);
            }
            None => self.u8(0),
        }
        match r.to_rational() {
            Some(t) => {
                self.u8(1);
                self.rational(&t);
                let back = RationalRotation::from_rational(&t).unwrap();
                self.rotation(&back);
            }
            None => self.u8(0),
        }
        self.transform(&r.to_transform().unwrap());
    }

    fn image(&mut self, r: Result<Rounded<P3>, MathError>) {
        self.calls += 1;
        match r {
            Ok(p) => {
                self.u8(1);
                let v = p.value();
                self.i64(v.x.get());
                self.i64(v.y.get());
                self.i64(v.z.get());
                self.u8(u8::from(p.is_exact()));
            }
            Err(e) => self.error(e),
        }
    }

    fn image_2d(&mut self, r: Result<Rounded<P2>, MathError>) {
        self.calls += 1;
        match r {
            Ok(p) => {
                self.u8(1);
                let v = p.value();
                self.i64(v.x.get());
                self.i64(v.y.get());
                self.u8(u8::from(p.is_exact()));
            }
            Err(e) => self.error(e),
        }
    }
}

const BOUNDS_2D: [i32; 7] = [-34, -30, -24, -16, -8, -3, -1];
const BOUNDS_3D: [i32; 5] = [-34, -28, -17, -5, -1];

fn real(x: f64) -> Real {
    Real::new(x).unwrap()
}

fn corpus() -> Recorder {
    let mut rng = Lcg(0x6460_4646_0000_0046);
    let mut rec = Recorder::default();

    // 2D: every octant's edges and axes, tiny, subnormal and huge components, random directions
    // of every size, and integer directions.
    let mut dirs = vec![
        [1.0, 0.0],
        [0.0, 1.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [1.0, 1.0],
        [-1.0, 1.0],
        [-1.0, -1.0],
        [1.0, -1.0],
        [3.0, 4.0],
        [1.0, 1e-300],
        [5e-324, 1.0],
        [1e308, -1e308],
        [f64::MAX, f64::MAX],
        [1.0, 1.5 * pow2(-34)],
        [0.0, 0.0],
    ];
    for _ in 0..120 {
        let s = pow2(rng.below(400) as i32 - 200);
        dirs.push([rng.signed() * s, rng.signed() * s]);
    }
    for _ in 0..40 {
        let x = (rng.next() >> 34) as i64 - (1 << 29);
        let y = (rng.next() >> 34) as i64 - (1 << 29);
        dirs.push([x as f64, y as f64]);
    }
    let mut plane = Vec::new();
    for d in &dirs {
        let d = F2::new(d[0], d[1]).unwrap();
        for k in BOUNDS_2D {
            let got = RationalRotation::plane_within(d, real(pow2(k)));
            if let Ok(w) = got {
                plane.push(w.value());
            }
            rec.within(got);
            match Pythagorean::within(d, real(pow2(k))) {
                Ok(p) => {
                    rec.pythagorean(&p.value());
                    rec.f64(p.angle().get());
                }
                Err(e) => rec.error(e),
            }
        }
    }
    // Refused bounds.
    let d = F2::new(1.0, 2.0).unwrap();
    for b in [pow2(-35), 0.75, 0.0] {
        rec.within(RationalRotation::plane_within(d, real(b)));
        rec.within(RationalRotation::within([1.0, 2.0, 3.0, 4.0], real(b)));
    }

    // 3D: quaternion targets of every size, with zeros, tiny and huge components.
    let mut targets = vec![
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, -1.0],
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 1e-300, -1e-300, 0.0],
        [5e-324, 0.0, 0.0, 1e-320],
        [f64::MAX, -f64::MAX, f64::MAX, f64::MAX],
        [0.0; 4],
        [1.0, f64::NAN, 0.0, 0.0],
        [f64::INFINITY, 0.0, 0.0, 0.0],
    ];
    for _ in 0..120 {
        let s = pow2(rng.below(400) as i32 - 200);
        let mut t = [rng.signed(), rng.signed(), rng.signed(), rng.signed()];
        if rng.below(5) == 0 {
            t[rng.below(4) as usize] = 0.0;
        }
        targets.push(t.map(|v| v * s));
    }
    let mut space = Vec::new();
    for t in &targets {
        for k in BOUNDS_3D {
            let got = RationalRotation::within(*t, real(pow2(k)));
            if let Ok(w) = got {
                space.push(w.value());
            }
            rec.within(got);
        }
    }

    // Compositions: plane with plane (mostly exact), space with space (mostly re-approximated).
    for pair in plane.chunks(2).take(150).chain(space.chunks(2).take(150)) {
        let [a, b] = [pair[0], pair[pair.len() - 1]];
        rec.i64(i64::from(a.compose_exact(&b).unwrap().is_some()));
        for k in [-34, -20, -2] {
            let got = a.compose(&b, real(pow2(k)));
            if let Ok(w) = got {
                rec.reapproximated += u32::from(!w.is_exact());
            }
            rec.within(got);
        }
        rec.typed(TypedTransform::from(a).compose(&TypedTransform::from(b)));
    }

    // Images at the frame's edges and inside, in 3D and in the plane.
    let edge = (1i64 << 40) - 1;
    let coords = [
        edge,
        -edge,
        0,
        1,
        -1,
        123_456_789,
        -987_654_321_012,
        edge / 3,
    ];
    for r in plane.iter().step_by(17).chain(space.iter().step_by(13)) {
        for _ in 0..12 {
            let pick = |rng: &mut Lcg| coords[rng.below(coords.len() as u64) as usize];
            let p = P3::new(pick(&mut rng), pick(&mut rng), pick(&mut rng)).unwrap();
            rec.image(r.apply(p).map_err(MathError::from));
            rec.image_2d(r.apply_2d(P2 { x: p.x, y: p.y }));
        }
    }

    // Typed transforms across every variant.
    let big = space.iter().copied().find(|r| r.norm2() > 1 << 40).unwrap();
    let typed = [
        TypedTransform::from(big),
        TypedTransform::from(plane[3]),
        TypedTransform::Exact(GridTransform::quarter_turns(Axis::Y, 1)),
        TypedTransform::rotation(Axis::X, Rotation::direction(20, 21).unwrap()).unwrap(),
        TypedTransform::from(RationalTransform::translation([3, -1, 2], 7).unwrap()),
        TypedTransform::from(RationalTransform::uniform_scale(Scale::new(5, 3).unwrap()).unwrap()),
        TypedTransform::from(Transform::uniform_scale(1.25).unwrap()),
    ];
    for a in &typed {
        rec.typed(a.inverse());
        for b in &typed {
            rec.typed(a.compose(b));
        }
    }

    // Quaternions and directions from their parts.
    for q in [
        [1, 2, 3, 4],
        [0, 0, 0, 0],
        [1 << 36, 1, 0, 0],
        [-6, 0, 0, -8],
        [0, -3, 4, 0],
    ] {
        match RationalRotation::new(q) {
            Ok(r) => {
                rec.u8(1);
                rec.rotation(&r);
                rec.details(&r);
            }
            Err(e) => rec.error(e.into()),
        }
    }
    for (a, b, c) in [(3, 4, 5), (-6, 8, -10), (1, 1, 1), (0, 0, 0), (0, -7, 7)] {
        match Pythagorean::new(a, b, c) {
            Ok(p) => {
                rec.pythagorean(&p);
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    rec.rotation(&RationalRotation::about(axis, &p).unwrap());
                }
            }
            Err(e) => rec.error(e.into()),
        }
    }
    rec
}

#[test]
fn rotation_golden_hash_is_the_same_on_every_target() {
    let checked = golden::check(GOLDEN, || {
        let rec = corpus();
        (sha256(&rec.bytes), rec)
    });
    let rec = &checked.value;
    println!(
        "geom-math rotation corpus: {} calls, {} re-approximated, {} bytes, golden hash {}",
        rec.calls,
        rec.reapproximated,
        rec.bytes.len(),
        hex(sha256(&rec.bytes))
    );
    assert!(rec.calls > 3000, "only {} calls", rec.calls);
    assert!(
        rec.reapproximated > 100,
        "only {} re-approximated",
        rec.reapproximated
    );
    checked.verdict().unwrap_or_else(|e| {
        panic!(
            "{e}. A changed digest is a bug: some target finds another fraction, rounds another \
             component or certifies another bit (compare this log's digest with another \
             target's), or a definition changed in place, which chapter 26.2 forbids"
        )
    });
}
