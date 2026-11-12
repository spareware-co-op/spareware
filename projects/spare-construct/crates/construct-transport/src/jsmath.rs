//! V8's `Math.exp`, `Math.pow`, `Math.sin`, `Math.cos` and `Math.asin`, bit for bit.
//!
//! Node and the browsers compute these with ports of FreeBSD's fdlibm (V8's `base::ieee754`),
//! whose results differ from the platform maths library's in the last bit for a few percent of
//! arguments. The assignment is an iterative method with decisions at the rounding-noise level
//! (the sign of a line-search derivative near zero), so to repeat the prototype's iterations
//! these are ported from fdlibm, with V8's changes to it (exp(1) is exactly e; results are
//! scaled by an exact power of two). They agree with Node bit for bit on every argument tried
//! (`tests/jsmath_truth.rs`). Arguments of sin and cos beyond 2^19 pi/2 (about 823,550) use the
//! platform's functions: the prototype only passes angles in radians of a map.
//!
//! `Math.pow` is the exception: see [`pow`].

#![allow(
    clippy::many_single_char_names,
    reason = "the names are fdlibm's, so the port reads line by line against its C source"
)]

// fdlibm's constants are written as it writes them (some equal std's constants, which must not stand in for them)
#![allow(clippy::excessive_precision, clippy::approx_constant)]

fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as u32 as i32
}

fn lo(x: f64) -> u32 {
    x.to_bits() as u32
}

fn with_lo(x: f64, l: u32) -> f64 {
    f64::from_bits((x.to_bits() & 0xffff_ffff_0000_0000) | u64::from(l))
}

fn words(h: i32, l: u32) -> f64 {
    f64::from_bits((u64::from(h as u32) << 32) | u64::from(l))
}

/// `Math.exp(x)`.
#[must_use]
pub fn exp(x: f64) -> f64 {
    const HALF: [f64; 2] = [0.5, -0.5];
    const O_THRESHOLD: f64 = 7.097_827_128_933_839_730_96e+02;
    const U_THRESHOLD: f64 = -7.451_332_191_019_411_084_20e+02;
    const LN2_HI: [f64; 2] = [6.931_471_803_691_238_164_90e-01, -6.931_471_803_691_238_164_90e-01];
    const LN2_LO: [f64; 2] = [1.908_214_929_270_587_700_02e-10, -1.908_214_929_270_587_700_02e-10];
    const INVLN2: f64 = 1.442_695_040_888_963_387_00e+00;
    const P1: f64 = 1.666_666_666_666_660_190_37e-01;
    const P2: f64 = -2.777_777_777_701_559_338_42e-03;
    const P3: f64 = 6.613_756_321_437_934_361_17e-05;
    const P4: f64 = -1.653_390_220_546_525_153_90e-06;
    const P5: f64 = 4.138_136_797_057_238_460_39e-08;
    const E: f64 = 2.718_281_828_459_045;
    const TWOM1000: f64 = 9.332_636_185_032_188_789_90e-302;
    const TWO1023: f64 = 8.988_465_674_311_579_539e307;
    let mut x = x;
    let (mut hh, mut ll) = (0.0f64, 0.0f64);
    let mut k: i32 = 0;
    let mut hx = hi(x) as u32;
    let xsb = ((hx >> 31) & 1) as usize;
    hx &= 0x7fff_ffff;
    // filter out non-finite argument
    if hx >= 0x4086_2E42 {
        if hx >= 0x7ff0_0000 {
            if ((hx & 0xfffff) | lo(x)) != 0 {
                return x + x; // NaN
            }
            return if xsb == 0 { x } else { 0.0 };
        }
        if x > O_THRESHOLD {
            return f64::INFINITY;
        }
        if x < U_THRESHOLD {
            return 0.0;
        }
    }
    // argument reduction
    if hx > 0x3fd6_2e42 {
        if hx < 0x3ff0_a2b2 {
            if x == 1.0 {
                return E;
            }
            hh = x - LN2_HI[xsb];
            ll = LN2_LO[xsb];
            k = 1 - xsb as i32 - xsb as i32;
        } else {
            k = (INVLN2 * x + HALF[xsb]) as i32;
            let t = f64::from(k);
            hh = x - t * LN2_HI[0];
            ll = t * LN2_LO[0];
        }
        x = hh - ll;
    } else if hx < 0x3e30_0000 {
        if 1.0e300 + x > 1.0 {
            return 1.0 + x;
        }
    } else {
        k = 0;
    }
    // x is now in primary range
    let t = x * x;
    let twopk = if k >= -1021 {
        words(0x3ff0_0000i32.wrapping_add(k << 20), 0)
    } else {
        words(0x3ff0_0000i32.wrapping_add((k + 1000) << 20), 0)
    };
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    if k == 0 {
        return 1.0 - ((x * c) / (c - 2.0) - x);
    }
    let y = 1.0 - ((ll - (x * c) / (2.0 - c)) - hh);
    if k >= -1021 {
        if k == 1024 {
            return y * 2.0 * TWO1023;
        }
        y * twopk
    } else {
        y * twopk * TWOM1000
    }
}

/// `Math.pow(x, y)`: V8's special cases, otherwise the platform's `pow`.
///
/// V8 gives exactly x for y = 1, x * x for y = 2 and the square root for y = 0.5, and follows
/// JavaScript's rules where C's pow differs (NaN exponent, a base of 1 or -1 to an infinite
/// power). Its general case is not fdlibm's (that one was ported and measured: its results differ from
/// V8's in 7% of calls), and V8's is a very accurate algorithm that this crate cannot
/// repeat bit for bit: the platform's pow differs from V8's in the last bit for about 0.03% of
/// arguments (measured against Node 24 on Windows for exponents 0.7 to 4). Code that
/// iterates to a fixed point can show this as a different iteration count at the
/// rounding-noise level.
#[must_use]
pub fn pow(x: f64, y: f64) -> f64 {
    if y.is_nan() {
        return f64::NAN;
    }
    if y.is_infinite() && (x == 1.0 || x == -1.0) {
        return f64::NAN;
    }
    if y == 2.0 {
        return x * x;
    }
    if y == 1.0 {
        return x;
    }
    if y == 0.5 && x >= 0.0 && x.is_sign_positive() {
        return x.sqrt();
    }
    x.powf(y)
}

/* ---------------- sin and cos ---------------- */

const S1: f64 = -1.666_666_666_666_663_243_48e-01;
const S2: f64 = 8.333_333_333_322_489_461_24e-03;
const S3: f64 = -1.984_126_982_985_794_931_34e-04;
const S4: f64 = 2.755_731_370_707_006_767_89e-06;
const S5: f64 = -2.505_076_025_340_686_341_95e-08;
const S6: f64 = 1.589_690_995_211_550_102_21e-10;

fn kernel_sin(x: f64, y: f64, iy: i32) -> f64 {
    let ix = hi(x) & 0x7fff_ffff;
    if ix < 0x3e40_0000 && (x as i32) == 0 {
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    if iy == 0 {
        x + v * (S1 + z * r)
    } else {
        x - ((z * (0.5 * y - v * r) - y) - v * S1)
    }
}

const C1: f64 = 4.166_666_666_666_660_190_37e-02;
const C2: f64 = -1.388_888_888_887_410_957_49e-03;
const C3: f64 = 2.480_158_728_947_672_941_78e-05;
const C4: f64 = -2.755_731_435_139_066_330_35e-07;
const C5: f64 = 2.087_572_321_298_174_827_90e-09;
const C6: f64 = -1.135_964_755_778_819_482_65e-11;

fn kernel_cos(x: f64, y: f64) -> f64 {
    let ix = hi(x) & 0x7fff_ffff;
    if ix < 0x3e40_0000 && (x as i32) == 0 {
        return 1.0;
    }
    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    if ix < 0x3fd3_3333 {
        1.0 - (0.5 * z - (z * r - x * y))
    } else {
        let qx = if ix > 0x3fe9_0000 { 0.28125 } else { words(ix - 0x0020_0000, 0) };
        let hz = 0.5 * z - qx;
        let a = 1.0 - qx;
        a - (hz - (z * r - x * y))
    }
}

// __ieee754_rem_pio2 for |x| up to 2^19 pi/2: n (mod 4 matters) and the remainder as y0 + y1
fn rem_pio2(x: f64) -> Option<(i32, f64, f64)> {
    const INVPIO2: f64 = 6.366_197_723_675_813_824_33e-01;
    const PIO2_1: f64 = 1.570_796_326_734_125_614_17e+00;
    const PIO2_1T: f64 = 6.077_100_506_506_192_249_32e-11;
    const PIO2_2: f64 = 6.077_100_506_303_965_976_60e-11;
    const PIO2_2T: f64 = 2.022_266_248_795_950_631_54e-21;
    const PIO2_3: f64 = 2.022_266_248_711_166_455_80e-21;
    const PIO2_3T: f64 = 8.478_427_660_368_899_569_97e-32;
    let hx = hi(x);
    let ix = hx & 0x7fff_ffff;
    if ix <= 0x3fe9_21fb {
        return Some((0, x, 0.0));
    }
    if ix < 0x4002_d97c {
        // |x| < 3 pi / 4, n = +-1
        if hx > 0 {
            let mut z = x - PIO2_1;
            let (y0, y1);
            if ix == 0x3ff9_21fb {
                z -= PIO2_2;
                y0 = z - PIO2_2T;
                y1 = (z - y0) - PIO2_2T;
            } else {
                y0 = z - PIO2_1T;
                y1 = (z - y0) - PIO2_1T;
            }
            return Some((1, y0, y1));
        } else {
            let mut z = x + PIO2_1;
            let (y0, y1);
            if ix == 0x3ff9_21fb {
                z += PIO2_2;
                y0 = z + PIO2_2T;
                y1 = (z - y0) + PIO2_2T;
            } else {
                y0 = z + PIO2_1T;
                y1 = (z - y0) + PIO2_1T;
            }
            return Some((-1, y0, y1));
        }
    }
    if ix <= 0x4139_21fb {
        // medium size
        let t = x.abs();
        let n = (t * INVPIO2 + 0.5) as i32;
        let fn_ = f64::from(n);
        let mut r = t - fn_ * PIO2_1;
        let mut w = fn_ * PIO2_1T;
        let j = ix >> 20;
        let mut y0 = r - w;
        let mut i = j - ((hi(y0) >> 20) & 0x7ff);
        if i > 16 {
            // 2nd iteration needed, good to 118 bits
            let t = r;
            w = fn_ * PIO2_2;
            r = t - w;
            w = fn_ * PIO2_2T - ((t - r) - w);
            y0 = r - w;
            i = j - ((hi(y0) >> 20) & 0x7ff);
            if i > 49 {
                // 3rd iteration, 151 bits
                let t = r;
                w = fn_ * PIO2_3;
                r = t - w;
                w = fn_ * PIO2_3T - ((t - r) - w);
                y0 = r - w;
            }
        }
        let y1 = (r - y0) - w;
        return if hx < 0 { Some((-n, -y0, -y1)) } else { Some((n, y0, y1)) };
    }
    None
}

/// `Math.sin(x)`.
#[must_use]
pub fn sin(x: f64) -> f64 {
    let ix = hi(x) & 0x7fff_ffff;
    if ix <= 0x3fe9_21fb {
        return kernel_sin(x, 0.0, 0);
    }
    if ix >= 0x7ff0_0000 {
        return f64::NAN;
    }
    match rem_pio2(x) {
        Some((n, y0, y1)) => match n & 3 {
            0 => kernel_sin(y0, y1, 1),
            1 => kernel_cos(y0, y1),
            2 => -kernel_sin(y0, y1, 1),
            _ => -kernel_cos(y0, y1),
        },
        None => x.sin(),
    }
}

/// `Math.cos(x)`.
#[must_use]
pub fn cos(x: f64) -> f64 {
    let ix = hi(x) & 0x7fff_ffff;
    if ix <= 0x3fe9_21fb {
        return kernel_cos(x, 0.0);
    }
    if ix >= 0x7ff0_0000 {
        return f64::NAN;
    }
    match rem_pio2(x) {
        Some((n, y0, y1)) => match n & 3 {
            0 => kernel_cos(y0, y1),
            1 => -kernel_sin(y0, y1, 1),
            2 => -kernel_cos(y0, y1),
            _ => kernel_sin(y0, y1, 1),
        },
        None => x.cos(),
    }
}

/// `Math.asin(x)`.
#[must_use]
pub fn asin(x: f64) -> f64 {
    const PIO2_HI: f64 = 1.570_796_326_794_896_558_00e+00;
    const PIO2_LO: f64 = 6.123_233_995_736_766_035_87e-17;
    const PIO4_HI: f64 = 7.853_981_633_974_482_789_99e-01;
    const PS0: f64 = 1.666_666_666_666_666_574_15e-01;
    const PS1: f64 = -3.255_658_186_224_009_154_05e-01;
    const PS2: f64 = 2.012_125_321_348_629_258_81e-01;
    const PS3: f64 = -4.005_553_450_067_941_140_27e-02;
    const PS4: f64 = 7.915_349_942_898_145_321_76e-04;
    const PS5: f64 = 3.479_331_075_960_211_675_70e-05;
    const QS1: f64 = -2.403_394_911_734_414_218_78e+00;
    const QS2: f64 = 2.020_945_760_233_505_694_71e+00;
    const QS3: f64 = -6.882_839_716_054_532_930_30e-01;
    const QS4: f64 = 7.703_815_055_590_193_527_91e-02;
    let hx = hi(x);
    let ix = hx & 0x7fff_ffff;
    if ix >= 0x3ff0_0000 {
        if ((ix - 0x3ff0_0000) as u32 | lo(x)) == 0 {
            return x * PIO2_HI + x * PIO2_LO;
        }
        return f64::NAN;
    } else if ix < 0x3fe0_0000 {
        if ix < 0x3e40_0000 && 1.0e300 + x > 1.0 {
            return x;
        }
        let t = x * x;
        let p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
        let q = 1.0 + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
        let w = p / q;
        return x + x * w;
    }
    // 1 > |x| >= 0.5
    let w = 1.0 - x.abs();
    let t = w * 0.5;
    let p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
    let q = 1.0 + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
    let s = t.sqrt();
    let t = if ix >= 0x3fef_3333 {
        let w = p / q;
        PIO2_HI - (2.0 * (s + s * w) - PIO2_LO)
    } else {
        let w = with_lo(s, 0);
        let c = (t - w * w) / (s + w);
        let r = p / q;
        let p = 2.0 * s * r - (PIO2_LO - 2.0 * c);
        let q = PIO4_HI - 2.0 * w;
        PIO4_HI - (p - q)
    };
    if hx > 0 { t } else { -t }
}
