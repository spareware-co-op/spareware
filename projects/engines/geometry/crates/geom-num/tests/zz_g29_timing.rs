//! Scratch timing for G29's routing decisions (not committed).
#![allow(clippy::all, clippy::pedantic, missing_docs)]

use std::hint::black_box;
use std::time::Instant;

use geom_num::generated::{plane_side, tpi_side, tpi_weight};
use geom_num::grid::P3;
use geom_num::plane::{Plane, Tpi};
use geom_num::Sign;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn coord(&mut self, reach: i64) -> i64 {
        (self.next() % (2 * reach as u64 + 1)) as i64 - reach
    }
    fn p3(&mut self, reach: i64) -> P3 {
        P3::new(self.coord(reach), self.coord(reach), self.coord(reach)).unwrap()
    }
}

fn pass<T: Copy>(inputs: &[T], mut f: impl FnMut(T) -> Sign) -> f64 {
    let start = Instant::now();
    let mut acc = 0i64;
    for &x in inputs {
        acc += i64::from(f(black_box(x)).as_i8());
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / inputs.len() as f64
}

fn plane(rng: &mut Rng, reach: i64) -> Plane {
    loop {
        if let Ok(p) = Plane::through(rng.p3(reach), rng.p3(reach), rng.p3(reach)) {
            return p;
        }
    }
}

fn tside(p: &Plane, t: &Tpi) -> Option<Sign> {
    let [a, b, c] = *t.planes();
    let (n1, n2, n3, n4) = (a.n(), b.n(), c.n(), p.n());
    tpi_side::filter(
        n1[0], n1[1], n1[2], a.d(), n2[0], n2[1], n2[2], b.d(), n3[0], n3[1], n3[2], c.d(), n4[0],
        n4[1], n4[2], p.d(),
    )
}
fn tside_exact(p: &Plane, t: &Tpi) -> Option<Sign> {
    let [a, b, c] = *t.planes();
    let (n1, n2, n3, n4) = (a.n(), b.n(), c.n(), p.n());
    tpi_side::exact(
        n1[0], n1[1], n1[2], a.d(), n2[0], n2[1], n2[2], b.d(), n3[0], n3[1], n3[2], c.d(), n4[0],
        n4[1], n4[2], p.d(),
    )
}
fn tweight(t: &Tpi) -> Option<Sign> {
    let [a, b, c] = *t.planes();
    let (n1, n2, n3) = (a.n(), b.n(), c.n());
    tpi_weight::filter(n1[0], n1[1], n1[2], n2[0], n2[1], n2[2], n3[0], n3[1], n3[2])
}
fn tweight_exact(t: &Tpi) -> Option<Sign> {
    let [a, b, c] = *t.planes();
    let (n1, n2, n3) = (a.n(), b.n(), c.n());
    tpi_weight::exact(n1[0], n1[1], n1[2], n2[0], n2[1], n2[2], n3[0], n3[1], n3[2])
}

#[test]
#[ignore]
fn timing() {
    let mut rng = Rng(7);
    const N: usize = 1 << 13;
    // Typical: random planes and points within 2^30.
    let pts: Vec<(Plane, P3)> = (0..N).map(|_| (plane(&mut rng, 1 << 30), rng.p3(1 << 30))).collect();
    // Degenerate: the point on the plane (a + (b - a) + (c - a)).
    let deg: Vec<(Plane, P3)> = (0..N)
        .map(|_| loop {
            let (a, b, c) = (rng.p3(1 << 30), rng.p3(1 << 30), rng.p3(1 << 30));
            if let Ok(p) = Plane::through(a, b, c) {
                let q = P3::new(
                    b.x.get() + c.x.get() - a.x.get(),
                    b.y.get() + c.y.get() - a.y.get(),
                    b.z.get() + c.z.get() - a.z.get(),
                )
                .unwrap();
                break (p, q);
            }
        })
        .collect();
    let tp: Vec<(Plane, Tpi)> = (0..N / 4)
        .map(|_| loop {
            let t = Tpi::new(plane(&mut rng, 1 << 30), plane(&mut rng, 1 << 30), plane(&mut rng, 1 << 30));
            if let Ok(t) = t {
                break (plane(&mut rng, 1 << 30), t);
            }
        })
        .collect();
    // Degenerate tpi: four planes through one point.
    let tdeg: Vec<(Plane, Tpi)> = (0..N / 4)
        .map(|_| loop {
            let at = rng.p3(1 << 30);
            let mut pl = || loop {
                let off = |r: &mut Rng| {
                    P3::new(at.x.get() + r.coord(1 << 10), at.y.get() + r.coord(1 << 10), at.z.get() + r.coord(1 << 10)).unwrap()
                };
                let (u, v) = (off(&mut rng), off(&mut rng));
                if let Ok(p) = Plane::through(at, u, v) {
                    break p;
                }
            };
            let (a, b, c, d) = (pl(), pl(), pl(), pl());
            if let Ok(t) = Tpi::new(a, b, c) {
                break (d, t);
            }
        })
        .collect();
    let mut best = [f64::INFINITY; 16];
    for _ in 0..40 {
        let mut k = 0;
        let mut t = |ns: f64| {
            if ns < best[k] {
                best[k] = ns;
            }
            k += 1;
        };
        t(pass(&pts, |(p, q)| p.side(q).unwrap()));
        t(pass(&pts, |(p, q)| plane_side::filter(p.n()[0], p.n()[1], p.n()[2], p.d(), q).unwrap_or(Sign::Zero)));
        t(pass(&pts, |(p, q)| plane_side::exact(p.n()[0], p.n()[1], p.n()[2], p.d(), q).unwrap()));
        t(pass(&deg, |(p, q)| p.side(q).unwrap()));
        t(pass(&deg, |(p, q)| plane_side::filter(p.n()[0], p.n()[1], p.n()[2], p.d(), q).unwrap_or(Sign::Zero)));
        t(pass(&tp, |(p, x)| p.side_of_tpi(&x).unwrap()));
        t(pass(&tp, |(p, x)| tside(&p, &x).unwrap_or(Sign::Zero)));
        t(pass(&tp, |(p, x)| tside_exact(&p, &x).unwrap()));
        t(pass(&tp, |(_, x)| tweight(&x).unwrap_or(Sign::Zero)));
        t(pass(&tp, |(_, x)| tweight_exact(&x).unwrap()));
        t(pass(&tdeg, |(p, x)| p.side_of_tpi(&x).unwrap()));
        t(pass(&tdeg, |(p, x)| tside(&p, &x).unwrap_or(Sign::Zero)));
        t(pass(&tdeg, |(p, x)| tside_exact(&p, &x).unwrap()));
        t(pass(&tdeg, |(_, x)| tweight(&x).unwrap_or(Sign::Zero)));
    }
    let names = [
        "side typical: public", "side typical: filter", "side typical: gen exact", "side degenerate: public",
        "side degenerate: filter", "tpi typical: public", "tpi typical: side filter", "tpi typical: side exact",
        "tpi typical: weight filter", "tpi typical: weight exact", "tpi degenerate: public",
        "tpi degenerate: side filter", "tpi degenerate: side exact", "tpi degenerate: weight filter",
    ];
    for (n, b) in names.iter().zip(best) {
        println!("{n:<32} {b:8.2} ns");
    }
    let dec = tp.iter().filter(|(p, x)| tside(p, x).is_some()).count();
    let ddec = tdeg.iter().filter(|(p, x)| tside(p, x).is_some()).count();
    println!("tpi side filter decides {dec}/{} typical, {ddec}/{} degenerate", tp.len(), tdeg.len());
}
