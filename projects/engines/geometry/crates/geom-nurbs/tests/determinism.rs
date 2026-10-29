//! The same bits on every target: the outputs of each algorithm on fixed inputs, pinned. CI runs
//! this on every target of chapter 21's matrix, so a difference in any IEEE operation, its order
//! or a fused multiply-add shows here.

#![allow(
    clippy::unwrap_used,
    clippy::unreadable_literal,
    clippy::many_single_char_names,
    reason = "tests: a failure stops the test, and the pinned values are raw bit patterns"
)]

use geom_nurbs::{Basis, Curve, Direction, Limit, Surface};
use geom_run::{Budget, WorkMeter};

fn meter() -> WorkMeter<'static> {
    WorkMeter::new(Budget::UNLIMITED)
}

fn curve() -> Curve<3> {
    let basis = Basis::new(
        3,
        vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.55, 0.55, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let pts = [
        [0.0, 0.0, 0.1],
        [1.0, 2.0, -0.3],
        [2.5, 1.7, 0.2],
        [3.1, -0.4, 1.3],
        [4.2, 0.9, 0.0],
        [5.0, 2.2, -1.1],
        [6.3, 0.1, 0.7],
    ];
    Curve::new_rational(basis, &pts, &[1.0, 0.7, 1.9, 0.6, 1.3, 2.1, 1.0]).unwrap()
}

/// Every double the algorithms produce here, in order.
fn outputs() -> Vec<f64> {
    let c = curve();
    let mut out = Vec::new();
    let mut n = [0.0; 4];
    c.basis().eval(0.41, Limit::FromRight, &mut n).unwrap();
    out.extend(n);
    let mut nd = [0.0; 12];
    c.basis()
        .eval_derivs(0.55, Limit::FromLeft, 2, &mut nd)
        .unwrap();
    out.extend(nd);
    out.push(c.basis().eval_one(3, 0.7, Limit::FromRight).unwrap());
    out.extend(c.point(0.41, Limit::FromRight).unwrap());
    let mut d = [[0.0; 3]; 3];
    c.derivs(0.62, Limit::FromRight, &mut d).unwrap();
    out.extend(d.iter().flatten());
    let m = &mut meter();
    let ins = c.insert_knot(0.8, 2, m).unwrap();
    out.extend(ins.value.homogeneous().iter().flatten());
    out.push(ins.deviation);
    let up = c.elevate(1, m).unwrap();
    out.extend(up.value.homogeneous().iter().flatten());
    out.extend(up.value.weights().unwrap());
    let bz = c.to_bezier(m).unwrap();
    for piece in &bz.value {
        out.extend(piece.homogeneous().iter().flatten());
    }
    let (rm, _) = ins.value.remove_knot(0.8, 2, 1e-6, m).unwrap();
    out.extend(rm.value.homogeneous().iter().flatten());
    out.push(rm.deviation);
    let down = up.value.reduce_degree(1e-6, m).unwrap();
    out.extend(down.value.homogeneous().iter().flatten());
    out.push(down.deviation);
    let bu = Basis::new(2, vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
    let bv = Basis::new(1, vec![0.0, 0.0, 0.25, 1.0, 1.0]).unwrap();
    let net: Vec<[f64; 3]> = (0..12)
        .map(|i| {
            let x = f64::from(i);
            [x * 0.3, (x * 0.7).fract(), x * x * 0.01]
        })
        .collect();
    let s = Surface::new(bu, bv, &net).unwrap();
    let lim = (Limit::FromRight, Limit::FromRight);
    out.extend(s.point(0.3, 0.6, lim).unwrap());
    let mut sd = [[0.0; 3]; 6];
    s.derivs(0.7, 0.1, lim, (2, 1), &mut sd).unwrap();
    out.extend(sd.iter().flatten());
    let si = s.insert_knot(Direction::V, 0.6, 1, m).unwrap();
    out.extend(si.value.homogeneous().iter().flatten());
    out
}

/// The bits of [`outputs`], as computed on x86-64 Windows with the crate at piece G56.
const PINNED: [u64; 246] = [
    0x3fa293dafe6b4d3f,
    0x3fd19d91f0545fa2,
    0x3fe50ebffcdfc3b0,
    0x3f9f272b61eaf548,
    0x0000000000000000,
    0x0000000000000000,
    0x3fe4924924924924,
    0x3fd6db6db6db6db9,
    0x0000000000000000,
    0x0000000000000000,
    0xc011249249249249,
    0x4011249249249249,
    0x0000000000000000,
    0x4045d1745d1745d0,
    0xc0537b03531dec0c,
    0x4041249249249248,
    0x3fc861861861861d,
    0x40062a684c1754a9,
    0x3fea9e871a30641e,
    0x3fe3defca742b906,
    0x4010434efa995705,
    0x3fe8b618043aa868,
    0x3fc35ec0287cfd6c,
    0x4014596aa1368245,
    0x401a1813e50ee758,
    0xc017f7b8c574b176,
    0xc01ff6a232975cbe,
    0xc02f83878bae04c8,
    0x40322bf7740f235c,
    0x0000000000000000,
    0x0000000000000000,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3ff6666666666666,
    0xbfcae147ae147ae1,
    0x4013000000000000,
    0x4009d70a3d70a3d7,
    0x3fd851eb851eb852,
    0x3ffdc28f5c28f5c2,
    0xbfceb851eb851eb8,
    0x3fe8f5c28f5c28f6,
    0x4011b9c869536204,
    0x3fe88c6f2d593bfc,
    0x3fcc869536202ed0,
    0x401a3bd089d56a26,
    0x40007236125013f1,
    0xbfe3a531ca9420bb,
    0x40206a9264e209dd,
    0x400458ffdfa2f480,
    0xbfed971254b3841e,
    0x4020555555555555,
    0x4000df012345678a,
    0xbfe468acf13579c0,
    0x4019333333333333,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3d40d0f7a9f00d63,
    0x0000000000000000,
    0x0000000000000000,
    0x3fb999999999999a,
    0x3fe0cccccccccccc,
    0x3ff0cccccccccccc,
    0xbfc0f5c28f5c28f6,
    0x3ffcdf6b0df6b0e0,
    0x3ffe62ad262ad262,
    0xbfa9227192271924,
    0x400f2a5b3aa5b3ac,
    0x400464129e4129e2,
    0x3fd7851eb851eb84,
    0x4004227192271922,
    0x3fe18e6dd8e6dd8f,
    0x3fe60d0860d0860d,
    0x40017390d2a6c406,
    0xbfbd36202ecfb9c4,
    0x3fe6bb3ee721a54d,
    0x4013869536202ed1,
    0x3fee30463796aca0,
    0x3fc00bb3ee721a55,
    0x401feb851eb851ec,
    0x400728f5c28f5c2a,
    0xbff27ae147ae147c,
    0x4022e66666666666,
    0x400beb851eb851ed,
    0xbff8eb851eb851ed,
    0x4019333333333333,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3ff0000000000000,
    0x3fe8cccccccccccc,
    0x3ff06fb586fb586f,
    0x3ff961bed61bed60,
    0x3feca7904a7904a8,
    0x3fe5333333333333,
    0x3ff3000000000000,
    0x3ffb333333333334,
    0x3ffd333333333334,
    0x3ff0000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3ff6666666666666,
    0xbfcae147ae147ae1,
    0x400745d1745d1746,
    0x40032f79f2f79f2f,
    0x3fbca01dca01dc9e,
    0x40086d56e9be28cb,
    0x3ffd1cb05d6dee91,
    0x3fd822873c56ff68,
    0x40086d56e9be28cb,
    0x3ffd1cb05d6dee91,
    0x3fd822873c56ff68,
    0x4009639b7639b764,
    0x3ff5657816578166,
    0x3fe3244e3244e324,
    0x3ffdc28f5c28f5c2,
    0xbfceb851eb851eb8,
    0x3fe8f5c28f5c28f6,
    0x40092a6c405d9f75,
    0x3fd0de5ab277f450,
    0x3fe00bb3ee721a55,
    0x40092a6c405d9f75,
    0x3fd0de5ab277f450,
    0x3fe00bb3ee721a55,
    0x4015d70a3d70a3d8,
    0x3ff2b851eb851eb9,
    0x0000000000000000,
    0x4025000000000000,
    0x40127ae147ae147c,
    0xc0027ae147ae147c,
    0x4019333333333333,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x0000000000000000,
    0x0000000000000000,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3ff6666666666666,
    0xbfcae147ae147ae1,
    0x4013000000000000,
    0x4009d70a3d70a3d7,
    0x3fd851eb851eb852,
    0x3ffdc28f5c28f5c2,
    0xbfceb851eb851eb8,
    0x3fe8f5c28f5c28f6,
    0x4015d70a3d70a3d8,
    0x3ff2b851eb851eba,
    0x3c90000000000000,
    0x4025000000000000,
    0x40127ae147ae147c,
    0xc0027ae147ae147d,
    0x4019333333333333,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3d31f4f200a66546,
    0x0000000000000000,
    0x0000000000000000,
    0x3fb999999999999a,
    0x3fe6666666666665,
    0x3ff6666666666665,
    0xbfcae147ae147ae1,
    0x4013000000000002,
    0x4009d70a3d70a3d4,
    0x3fd851eb851eb851,
    0x3ffdc28f5c28f5c1,
    0xbfceb851eb851eb7,
    0x3fe8f5c28f5c28f6,
    0x4015d70a3d70a3d8,
    0x3ff2b851eb851eb9,
    0x3c98000000000000,
    0x4024ffffffffffff,
    0x40127ae147ae147c,
    0xc0027ae147ae147c,
    0x4019333333333333,
    0x3fb999999999999a,
    0x3fe6666666666666,
    0x3d46cb9336b218ed,
    0x3ff5ba5e353f7ced,
    0x3fe52f1a9fbe76c7,
    0x3fce757928e0c9d8,
    0x3ffe6e978d4fdf3b,
    0x3fda7ef9db22d0dd,
    0x3fdbd5bab21815a0,
    0x3ff3333333333338,
    0x400147ae147ae14a,
    0x3fe07c84b5dcc63e,
    0x400428f5c28f5c2a,
    0xbfd70a3d70a3d701,
    0x3ff1c779a6b50b0f,
    0x3cd4000000000000,
    0xc019999999999998,
    0x3fe5810624dd2f16,
    0x400cccccccccccd0,
    0xc00666666666665b,
    0x4004ed916872b01e,
    0xbcf0000000000000,
    0xc040000000000000,
    0x3feeb851eb851ea8,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x3fd3333333333333,
    0x3fe6666666666666,
    0x3f847ae147ae147b,
    0x3fdc28f5c28f5c29,
    0x3fe1eb851eb851ec,
    0x3f989374bc6a7efa,
    0x3fe3333333333333,
    0x3fd9999999999998,
    0x3fa47ae147ae147b,
    0x3feccccccccccccc,
    0x3fb9999999999980,
    0x3fb70a3d70a3d70a,
    0x3ff3333333333333,
    0x3fe9999999999998,
    0x3fc47ae147ae147b,
    0x3ff570a3d70a3d71,
    0x3fe51eb851eb851e,
    0x3fc9db22d0e56042,
    0x3ff8000000000000,
    0x3fe0000000000000,
    0x3fd0000000000000,
    0x3ffccccccccccccc,
    0x3fc9999999999980,
    0x3fd70a3d70a3d70a,
    0x4000cccccccccccd,
    0x3fecccccccccccc8,
    0x3fdf5c28f5c28f5c,
    0x4001eb851eb851ec,
    0x3fe851eb851eb84e,
    0x3fe1eb851eb851ec,
    0x4003333333333333,
    0x3fe3333333333330,
    0x3fe47ae147ae147b,
    0x4005999999999999,
    0x3fd3333333333330,
    0x3fe9eb851eb851ec,
    0x4008000000000000,
    0x0000000000000000,
    0x3ff0000000000000,
    0x40091eb851eb851f,
    0x3fd4e81b4e81b4e2,
    0x3ff1916872b020c4,
    0x400a666666666666,
    0x3fe6666666666660,
    0x3ff35c28f5c28f5c,
];

#[test]
fn outputs_have_the_pinned_bits() {
    let bits: Vec<u64> = outputs().iter().map(|x| x.to_bits()).collect();
    assert_eq!(bits.len(), PINNED.len());
    for (i, (got, want)) in bits.iter().zip(PINNED).enumerate() {
        assert_eq!(
            *got,
            want,
            "output {i}: {:e} where {:e} is pinned",
            f64::from_bits(*got),
            f64::from_bits(want)
        );
    }
}
